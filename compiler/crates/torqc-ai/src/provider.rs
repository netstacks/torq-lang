use crate::config::{AiConfig, ProviderKind};
use serde_json::json;

/// Represents a compilation error that needs AI fixing.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub stage: String,
    pub message: String,
    pub source: String,
    pub file: String,
}

/// Response from the AI provider.
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub fixed_source: String,
    pub explanation: String,
}

/// Trait for AI providers — all providers implement this.
pub trait AiProvider: Send + Sync {
    fn fix_error(&self, error: &CompileError) -> Result<AiResponse, String>;
    fn name(&self) -> &str;
}

/// Anthropic Claude provider.
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    endpoint: String,
}

impl AnthropicProvider {
    pub fn new(config: &AiConfig) -> Result<Self, String> {
        let api_key = config.key.clone()
            .ok_or_else(|| "Anthropic API key not configured. Run `torqc ai setup`".to_string())?;
        Ok(Self {
            api_key,
            model: config.model.clone(),
            endpoint: config.endpoint.clone()
                .unwrap_or_else(|| "https://api.anthropic.com/v1/messages".to_string()),
        })
    }
}

impl AiProvider for AnthropicProvider {
    fn fix_error(&self, error: &CompileError) -> Result<AiResponse, String> {
        let prompt = format_fix_prompt(error);

        let body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "system": "You are a TORQ language expert. Fix the compilation error in the provided TORQ source code. Return ONLY the corrected TORQ source code, no explanations. If you must explain, put it after a line containing only '---'."
        });

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(&self.endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = resp.status();
        let text = resp.text().map_err(|e| format!("failed to read response: {}", e))?;

        if !status.is_success() {
            return Err(format!("API error ({}): {}", status, text));
        }

        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("failed to parse response: {}", e))?;

        let content = parsed["content"][0]["text"]
            .as_str()
            .ok_or_else(|| "no text in response".to_string())?;

        parse_ai_response(content)
    }

    fn name(&self) -> &str {
        "Anthropic"
    }
}

/// OpenAI provider.
pub struct OpenAIProvider {
    api_key: String,
    model: String,
    endpoint: String,
}

impl OpenAIProvider {
    pub fn new(config: &AiConfig) -> Result<Self, String> {
        let api_key = config.key.clone()
            .ok_or_else(|| "OpenAI API key not configured. Run `torqc ai setup`".to_string())?;
        Ok(Self {
            api_key,
            model: config.model.clone(),
            endpoint: config.endpoint.clone()
                .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".to_string()),
        })
    }
}

impl AiProvider for OpenAIProvider {
    fn fix_error(&self, error: &CompileError) -> Result<AiResponse, String> {
        let prompt = format_fix_prompt(error);

        let body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a TORQ language expert. Fix the compilation error in the provided TORQ source code. Return ONLY the corrected TORQ source code, no explanations. If you must explain, put it after a line containing only '---'."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 4096
        });

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = resp.status();
        let text = resp.text().map_err(|e| format!("failed to read response: {}", e))?;

        if !status.is_success() {
            return Err(format!("API error ({}): {}", status, text));
        }

        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("failed to parse response: {}", e))?;

        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| "no content in response".to_string())?;

        parse_ai_response(content)
    }

    fn name(&self) -> &str {
        "OpenAI"
    }
}

/// Mock provider for testing — returns pre-configured responses.
pub struct MockProvider {
    responses: Vec<AiResponse>,
    call_count: std::sync::atomic::AtomicUsize,
}

impl MockProvider {
    pub fn new(responses: Vec<AiResponse>) -> Self {
        Self {
            responses,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl AiProvider for MockProvider {
    fn fix_error(&self, _error: &CompileError) -> Result<AiResponse, String> {
        let idx = self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if idx < self.responses.len() {
            Ok(self.responses[idx].clone())
        } else {
            Err("mock provider: no more responses".to_string())
        }
    }

    fn name(&self) -> &str {
        "Mock"
    }
}

/// Create an AI provider from configuration.
pub fn create_provider(config: &AiConfig) -> Result<Box<dyn AiProvider>, String> {
    match config.provider {
        ProviderKind::Anthropic => Ok(Box::new(AnthropicProvider::new(config)?)),
        ProviderKind::OpenAI => Ok(Box::new(OpenAIProvider::new(config)?)),
        ProviderKind::Local => {
            // Local uses the OpenAI-compatible endpoint (Ollama)
            let mut local_config = config.clone();
            if local_config.endpoint.is_none() {
                local_config.endpoint = Some("http://localhost:11434/v1/chat/completions".to_string());
            }
            if local_config.key.is_none() {
                local_config.key = Some("ollama".to_string()); // Ollama doesn't need a real key
            }
            Ok(Box::new(OpenAIProvider::new(&local_config)?))
        }
        ProviderKind::Custom => {
            // Custom uses the OpenAI-compatible API format
            Ok(Box::new(OpenAIProvider::new(config)?))
        }
    }
}

// --- Internal helpers ---

fn format_fix_prompt(error: &CompileError) -> String {
    format!(
        "Fix the following TORQ compilation error.\n\n\
         Stage: {}\n\
         Error: {}\n\
         File: {}\n\n\
         Source code:\n```torq\n{}\n```\n\n\
         Return the fixed TORQ source code.",
        error.stage, error.message, error.file, error.source
    )
}

pub fn parse_ai_response(content: &str) -> Result<AiResponse, String> {
    // Strip markdown code fences if present
    let cleaned = content.trim();
    let cleaned = if cleaned.starts_with("```") {
        let after_first_fence = cleaned
            .find('\n')
            .map(|i| &cleaned[i + 1..])
            .unwrap_or(cleaned);
        after_first_fence
            .rfind("```")
            .map(|i| &after_first_fence[..i])
            .unwrap_or(after_first_fence)
            .trim()
    } else {
        cleaned
    };

    // Split on --- separator if present
    if let Some(idx) = cleaned.find("\n---\n") {
        Ok(AiResponse {
            fixed_source: cleaned[..idx].trim().to_string(),
            explanation: cleaned[idx + 5..].trim().to_string(),
        })
    } else {
        Ok(AiResponse {
            fixed_source: cleaned.to_string(),
            explanation: String::new(),
        })
    }
}
