use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use torqc_codegen::codegen::Compiler;
use torqc_codegen::linker;
use torqc_lexer::lexer::Lexer;
use torqc_parser::parser;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Guard that serialises calls through the linker, which writes to a shared
/// temp file (`torq_output.o`).  Without this, parallel test threads race on
/// that single object-file path.
static LINK_MUTEX: Mutex<()> = Mutex::new(());

fn compile_and_run(source: &str) -> String {
    let tokens = Lexer::tokenize(source, "test.torq").expect("lex failed");
    let program = parser::parse(tokens, "test.torq").expect("parse failed");

    let compiler = Compiler::new().expect("compiler init failed");
    let bytes = compiler.compile(&program).expect("compile failed");

    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_bin = std::env::temp_dir().join(format!("torq_e2e_test_{}", n));

    // Hold the lock across link + run so no other test can clobber the
    // intermediate object file while we are linking.
    {
        let _guard = LINK_MUTEX.lock().unwrap();
        linker::link(&bytes, &temp_bin).expect("link failed");
    }

    let output = Command::new(&temp_bin)
        .output()
        .expect("failed to run binary");

    let _ = std::fs::remove_file(&temp_bin);

    assert!(
        output.status.success(),
        "binary exited with non-zero: {:?}",
        output.status
    );

    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Resolve the path to the repository-level `examples/` directory regardless
/// of the current working directory.  Uses `CARGO_MANIFEST_DIR` (set by Cargo
/// for every test binary) to anchor the path.
fn examples_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // manifest = <repo>/compiler/crates/torqc-codegen
    // examples = <repo>/examples
    manifest
        .parent() // crates/
        .unwrap()
        .parent() // compiler/
        .unwrap()
        .parent() // <repo>/
        .unwrap()
        .join("examples")
}

#[test]
fn hello_world() {
    let output = compile_and_run("::main\n  print \"hello world\"\n");
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn multiple_prints() {
    let output = compile_and_run("::main\n  print \"line one\"\n  print \"line two\"\n");
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines, vec!["line one", "line two"]);
}

#[test]
fn print_integer() {
    let output = compile_and_run("::main\n  print 42\n");
    assert_eq!(output.trim(), "42");
}

#[test]
fn print_boolean_true() {
    let output = compile_and_run("::main\n  print true\n");
    assert_eq!(output.trim(), "true");
}

#[test]
fn print_boolean_false() {
    let output = compile_and_run("::main\n  print false\n");
    assert_eq!(output.trim(), "false");
}

#[test]
fn print_null() {
    let output = compile_and_run("::main\n  print null\n");
    assert_eq!(output.trim(), "null");
}

#[test]
fn hello_torq_example_file() {
    let hello = examples_dir().join("hello.torq");
    let source = std::fs::read_to_string(&hello)
        .unwrap_or_else(|e| panic!("read {} failed: {}", hello.display(), e));
    let output = compile_and_run(&source);
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn pipeline_to_print() {
    let output = compile_and_run("::main\n  \"hello pipeline\" | print\n");
    assert_eq!(output.trim(), "hello pipeline");
}

#[test]
fn pipeline_integer_to_print() {
    let output = compile_and_run("::main\n  99 | print\n");
    assert_eq!(output.trim(), "99");
}

// ---------------------------------------------------------------------------
// Variable tests
// ---------------------------------------------------------------------------

#[test]
fn variable_assign_and_print() {
    let output = compile_and_run("::main\n  42 -> $x\n  $x | print\n");
    assert_eq!(output.trim(), "42");
}

#[test]
fn variable_reassign() {
    let output = compile_and_run("::main\n  10 -> $x\n  20 -> $x\n  $x | print\n");
    assert_eq!(output.trim(), "20");
}

#[test]
fn variable_string() {
    let output = compile_and_run("::main\n  \"hello\" -> $msg\n  $msg | print\n");
    assert_eq!(output.trim(), "hello");
}

// ---------------------------------------------------------------------------
// Arithmetic tests
// ---------------------------------------------------------------------------

#[test]
fn arithmetic_add() {
    let output = compile_and_run("::main\n  (3 + 4) | print\n");
    assert_eq!(output.trim(), "7");
}

#[test]
fn arithmetic_sub() {
    let output = compile_and_run("::main\n  (10 - 3) | print\n");
    assert_eq!(output.trim(), "7");
}

#[test]
fn arithmetic_mul() {
    let output = compile_and_run("::main\n  (6 * 7) | print\n");
    assert_eq!(output.trim(), "42");
}

#[test]
fn arithmetic_complex() {
    let output = compile_and_run("::main\n  ((2 + 3) * (10 - 4)) | print\n");
    assert_eq!(output.trim(), "30");
}

// ---------------------------------------------------------------------------

#[test]
fn empty_main_exits_zero() {
    let tokens = Lexer::tokenize("::main\n  print \"\"\n", "test.torq").expect("lex");
    let program = parser::parse(tokens, "test.torq").expect("parse");
    let compiler = Compiler::new().expect("compiler");
    let bytes = compiler.compile(&program).expect("compile");

    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp = std::env::temp_dir().join(format!("torq_e2e_empty_{}", n));

    {
        let _guard = LINK_MUTEX.lock().unwrap();
        linker::link(&bytes, &temp).expect("link");
    }

    let output = Command::new(&temp).output().expect("run");
    let _ = std::fs::remove_file(&temp);
    assert!(output.status.success());
}
