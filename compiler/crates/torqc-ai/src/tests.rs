use crate::config::{AiConfig, ProviderKind};
use crate::fixer::{AiFixer, FixResult};
use crate::provider::{AiProvider, AiResponse, CompileError, MockProvider};

fn mock_config() -> AiConfig {
    AiConfig {
        provider: ProviderKind::Anthropic,
        key: Some("test-key".into()),
        model: "test-model".into(),
        max_retries: 3,
        endpoint: None,
    }
}

fn sample_error() -> CompileError {
    CompileError {
        stage: "semantic".into(),
        message: "undefined block: ::validate_cart".into(),
        source: "::main\n  ::validate_cart\n".into(),
        file: "test.torq".into(),
    }
}

#[test]
fn mock_provider_returns_fixed_source() {
    let provider = MockProvider::new(vec![AiResponse {
        fixed_source: "::main\n  ::validate_items\n".into(),
        explanation: "renamed block".into(),
    }]);

    let error = sample_error();
    let result = provider.fix_error(&error);
    assert!(result.is_ok());
    let resp = result.unwrap();
    assert_eq!(resp.fixed_source, "::main\n  ::validate_items\n");
    assert_eq!(resp.explanation, "renamed block");
    assert_eq!(provider.call_count(), 1);
}

#[test]
fn mock_provider_exhausts_responses() {
    let provider = MockProvider::new(vec![]);
    let error = sample_error();
    let result = provider.fix_error(&error);
    assert!(result.is_err());
}

#[test]
fn fixer_succeeds_on_first_try() {
    let provider = Box::new(MockProvider::new(vec![AiResponse {
        fixed_source: "fixed source".into(),
        explanation: "".into(),
    }]));

    let config = mock_config();
    let fixer = AiFixer::new(provider, &config, false);

    let error = sample_error();
    let result = fixer.try_fix(&error, |_source| Ok(()));

    match result {
        FixResult::Fixed { attempts, .. } => assert_eq!(attempts, 1),
        FixResult::Failed { .. } => panic!("expected Fixed"),
    }
}

#[test]
fn fixer_retries_on_recompile_failure() {
    let provider = Box::new(MockProvider::new(vec![
        AiResponse {
            fixed_source: "still broken".into(),
            explanation: "".into(),
        },
        AiResponse {
            fixed_source: "fixed source".into(),
            explanation: "".into(),
        },
    ]));

    let config = mock_config();
    let fixer = AiFixer::new(provider, &config, false);

    let error = sample_error();
    let mut call = 0;
    let result = fixer.try_fix(&error, |source| {
        call += 1;
        if source == "still broken" {
            Err("still broken".into())
        } else {
            Ok(())
        }
    });

    match result {
        FixResult::Fixed { attempts, .. } => assert_eq!(attempts, 2),
        FixResult::Failed { .. } => panic!("expected Fixed on attempt 2"),
    }
}

#[test]
fn fixer_fails_after_max_retries() {
    let provider = Box::new(MockProvider::new(vec![
        AiResponse { fixed_source: "bad1".into(), explanation: "".into() },
        AiResponse { fixed_source: "bad2".into(), explanation: "".into() },
        AiResponse { fixed_source: "bad3".into(), explanation: "".into() },
    ]));

    let config = mock_config();
    let fixer = AiFixer::new(provider, &config, false);

    let error = sample_error();
    let result = fixer.try_fix(&error, |_| Err("still broken".into()));

    match result {
        FixResult::Failed { attempts, .. } => assert_eq!(attempts, 3),
        FixResult::Fixed { .. } => panic!("expected Failed"),
    }
}

#[test]
fn config_default_values() {
    let config = AiConfig::default();
    assert_eq!(config.provider, ProviderKind::Anthropic);
    assert_eq!(config.model, "claude-sonnet-4-6");
    assert_eq!(config.max_retries, 3);
    assert!(config.key.is_none());
}

#[test]
fn config_deserialize_yaml() {
    let yaml = r#"
provider: openai
key: sk-test
model: gpt-4o
max_retries: 5
"#;
    let config: AiConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.provider, ProviderKind::OpenAI);
    assert_eq!(config.key.unwrap(), "sk-test");
    assert_eq!(config.model, "gpt-4o");
    assert_eq!(config.max_retries, 5);
}

#[test]
fn parse_ai_response_with_code_fences() {
    let content = "```torq\n::main\n  print \"hello\"\n```";
    let result = crate::provider::parse_ai_response(content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().fixed_source, "::main\n  print \"hello\"");
}

#[test]
fn parse_ai_response_with_separator() {
    let content = "::main\n  print \"hello\"\n---\nFixed the typo";
    let result = crate::provider::parse_ai_response(content);
    assert!(result.is_ok());
    let resp = result.unwrap();
    assert_eq!(resp.fixed_source, "::main\n  print \"hello\"");
    assert_eq!(resp.explanation, "Fixed the typo");
}
