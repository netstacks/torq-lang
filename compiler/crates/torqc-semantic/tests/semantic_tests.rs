use std::path::PathBuf;

use torqc_lexer::lexer::Lexer;
use torqc_parser::parser;
use torqc_semantic::analyzer;

fn check_source(source: &str) -> torqc_semantic::analyzer::AnalysisResult {
    let tokens = Lexer::tokenize(source, "test.torq").expect("lex failed");
    let program = parser::parse(tokens, "test.torq").expect("parse failed");
    analyzer::analyze(&program)
}

fn check_file(path: &str) -> torqc_semantic::analyzer::AnalysisResult {
    let source = std::fs::read_to_string(path).expect(&format!("failed to read {}", path));
    let tokens = Lexer::tokenize(&source, path).expect("lex failed");
    let program = parser::parse(tokens, path).expect("parse failed");
    analyzer::analyze(&program)
}

/// Build an absolute path to an example file using CARGO_MANIFEST_DIR.
fn example_path(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crate is at compiler/crates/torqc-semantic, examples are at repo root
    path.pop(); // -> compiler/crates
    path.pop(); // -> compiler
    path.pop(); // -> repo root
    path.push("examples");
    path.push(name);
    path.to_string_lossy().to_string()
}

// -- Example files should have no errors --------------------------------------

#[test]
fn hello_clean() {
    let result = check_file(&example_path("hello.torq"));
    assert!(
        !result.has_errors(),
        "hello.torq has errors: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn fibonacci_clean() {
    let result = check_file(&example_path("fibonacci.torq"));
    assert!(
        !result.has_errors(),
        "fibonacci.torq has errors: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn data_processing_clean() {
    let result = check_file(&example_path("data_processing.torq"));
    assert!(
        !result.has_errors(),
        "data_processing.torq has errors: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn cli_tool_clean() {
    let result = check_file(&example_path("cli_tool.torq"));
    assert!(
        !result.has_errors(),
        "cli_tool.torq has errors: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn web_server_clean() {
    let result = check_file(&example_path("web_server.torq"));
    assert!(
        !result.has_errors(),
        "web_server.torq has errors: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn checkout_service_clean() {
    let result = check_file(&example_path("checkout_service.torq"));
    assert!(
        !result.has_errors(),
        "checkout_service.torq has errors: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
    );
}

// -- Negative tests -----------------------------------------------------------

#[test]
fn detects_undefined_block() {
    let result = check_source("::main\n  ::nonexistent\n");
    assert!(result.has_errors());
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("undefined block")));
}

#[test]
fn detects_duplicate_block() {
    let result = check_source("::main\n  print \"a\"\n\n::main\n  print \"b\"\n");
    assert!(result.has_errors());
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("duplicate block")));
}

#[test]
fn detects_break_outside_loop() {
    let result = check_source("::main\n  break\n");
    assert!(result.has_errors());
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("break")));
}

#[test]
fn detects_arity_mismatch_too_few() {
    // Too few args is a warning (parser may absorb trailing args via member access).
    let result = check_source("::helper $a $b\n  print $a\n\n::main\n  ::helper 1\n");
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("expects 2") && !d.is_error()));
}

#[test]
fn detects_arity_mismatch_too_many() {
    // Too many args is an error.
    let result = check_source("::helper $a\n  print $a\n\n::main\n  ::helper 1 2 3\n");
    assert!(result.has_errors());
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("expects 1") && d.is_error()));
}
