use crate::config::AiConfig;
use crate::provider::{AiProvider, CompileError};

/// AI compilation fixer — wraps an AI provider and handles the retry loop.
pub struct AiFixer {
    provider: Box<dyn AiProvider>,
    max_retries: u32,
    verbose: bool,
}

/// Result of an AI fix attempt.
#[derive(Debug)]
pub enum FixResult {
    /// The AI successfully fixed the source.
    Fixed {
        source: String,
        explanation: String,
        attempts: u32,
    },
    /// The AI could not fix the issue after max retries.
    Failed {
        original_error: String,
        attempts: u32,
    },
}

impl AiFixer {
    pub fn new(provider: Box<dyn AiProvider>, config: &AiConfig, verbose: bool) -> Self {
        Self {
            provider,
            max_retries: config.max_retries,
            verbose,
        }
    }

    /// Attempt to fix a compilation error by sending it to the AI provider.
    /// The `recompile` closure takes the fixed source and returns Ok(()) on success
    /// or Err(error_message) if the fix didn't work.
    pub fn try_fix<F>(
        &self,
        error: &CompileError,
        mut recompile: F,
    ) -> FixResult
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        let mut last_error = error.clone();

        for attempt in 1..=self.max_retries {
            if self.verbose {
                eprintln!(
                    "  \u{2192} AI fix attempt {}/{} via {}",
                    attempt,
                    self.max_retries,
                    self.provider.name()
                );
            }

            match self.provider.fix_error(&last_error) {
                Ok(response) => {
                    if self.verbose && !response.explanation.is_empty() {
                        eprintln!("    Explanation: {}", response.explanation);
                    }

                    // Try recompiling with the fixed source
                    match recompile(&response.fixed_source) {
                        Ok(()) => {
                            return FixResult::Fixed {
                                source: response.fixed_source,
                                explanation: response.explanation,
                                attempts: attempt,
                            };
                        }
                        Err(new_error) => {
                            if self.verbose {
                                eprintln!("    Fix did not resolve: {}", new_error);
                            }
                            // Update the error for the next attempt
                            last_error = CompileError {
                                stage: error.stage.clone(),
                                message: new_error,
                                source: response.fixed_source,
                                file: error.file.clone(),
                            };
                        }
                    }
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("    AI provider error: {}", e);
                    }
                    return FixResult::Failed {
                        original_error: error.message.clone(),
                        attempts: attempt,
                    };
                }
            }
        }

        FixResult::Failed {
            original_error: error.message.clone(),
            attempts: self.max_retries,
        }
    }
}

/// Convenience: create a fixer from config, returns None if AI is disabled.
pub fn create_fixer(config: &AiConfig, verbose: bool) -> Result<AiFixer, String> {
    let provider = crate::provider::create_provider(config)?;
    Ok(AiFixer::new(provider, config, verbose))
}
