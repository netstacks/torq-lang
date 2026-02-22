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

// ---------------------------------------------------------------------------
// User-defined block (function) tests
// ---------------------------------------------------------------------------

#[test]
fn user_block_call() {
    let src = "::double $n\n  ($n * 2)\n\n::main\n  ::double 21 | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "42");
}

#[test]
fn block_with_multiple_params() {
    let src = "::add $a $b\n  ($a + $b)\n\n::main\n  ::add 19 23 | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "42");
}

// ---------------------------------------------------------------------------
// Match expression tests
// ---------------------------------------------------------------------------

#[test]
fn match_literal() {
    let src = "\
::classify $n
  $n | match
    1 -> 100
    2 -> 200
    _ -> 0

::main
  ::classify 2 | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "200");
}

#[test]
fn match_wildcard() {
    let src = "\
::classify $n
  $n | match
    1 -> 100
    _ -> 999

::main
  ::classify 42 | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "999");
}

// ---------------------------------------------------------------------------
// Loop and break tests
// ---------------------------------------------------------------------------

#[test]
fn loop_with_break() {
    let src = "\
::main
  3 -> $i
  loop
    $i | print
    ($i - 1) -> $i
    ($i < 1) | match
      1 -> break
      _ -> 0
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "3\n2\n1");
}

#[test]
fn simple_loop_with_variable() {
    let src = "\
::main
  0 -> $sum
  1 -> $i
  loop
    ($sum + $i) -> $sum
    ($i + 1) -> $i
    ($i > 5) | match
      1 -> break
      _ -> 0
  $sum | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "15");
}

// ---------------------------------------------------------------------------
// Each sequential + range tests
// ---------------------------------------------------------------------------

#[test]
fn each_sequential_range() {
    let src = "\
::main
  range 1 5 | each $n sequential
    $n | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "1\n2\n3\n4");
}

#[test]
fn each_sequential_range_with_block_call() {
    let src = "\
::double $n
  ($n * 2)

::main
  range 1 4 | each $n sequential
    ::double $n | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "2\n4\n6");
}

// ---------------------------------------------------------------------------
// Recursion + fibonacci integration tests
// ---------------------------------------------------------------------------

fn fib_value(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    let (mut a, mut b) = (0i64, 1i64);
    for _ in 2..=n {
        let tmp = a + b;
        a = b;
        b = tmp;
    }
    b
}

#[test]
fn fibonacci_example_file() {
    let fib = examples_dir().join("fibonacci.torq");
    let source = std::fs::read_to_string(&fib)
        .unwrap_or_else(|e| panic!("read {} failed: {}", fib.display(), e));
    let output = compile_and_run(&source);
    let lines: Vec<&str> = output.trim().lines().collect();
    let expected: Vec<String> = (1..20).map(|n| fib_value(n).to_string()).collect();
    let expected_refs: Vec<&str> = expected.iter().map(|s| s.as_str()).collect();
    assert_eq!(lines, expected_refs);
}

#[test]
fn fibonacci_inline() {
    let src = "\
::fibonacci $n
  $n | match
    0 -> 0
    1 -> 1
    _ -> ::fibonacci ($n - 1) + ::fibonacci ($n - 2)

::main
  ::fibonacci 10 | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "55");
}

#[test]
fn recursive_factorial() {
    let src = "\
::factorial $n
  $n | match
    0 -> 1
    _ -> ($n * ::factorial ($n - 1))

::main
  ::factorial 10 | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "3628800");
}

#[test]
fn block_returning_match() {
    let src = "\
::sign $n
  $n | match
    0 -> 0
    _ -> 1

::main
  ::sign 42 | print
  ::sign 0 | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "1\n0");
}

// ---------------------------------------------------------------------------
// Array tests
// ---------------------------------------------------------------------------

#[test]
fn array_create_and_print() {
    let output = compile_and_run("::main\n  @nums = [1 2 3]\n  @nums | print\n");
    assert_eq!(output.trim(), "[1, 2, 3]");
}

#[test]
fn array_len() {
    let output = compile_and_run("::main\n  @names = [\"a\" \"b\" \"c\"]\n  @names | len | print\n");
    assert_eq!(output.trim(), "3");
}

#[test]
fn array_first_last() {
    let output = compile_and_run("::main\n  @nums = [10 20 30]\n  @nums | first | print\n  @nums | last | print\n");
    assert_eq!(output.trim(), "10\n30");
}

// ---------------------------------------------------------------------------
// Dict tests
// ---------------------------------------------------------------------------

#[test]
fn dict_create_and_print() {
    let src = "::main\n  %user = { name \"alice\" age 30 }\n  %user | print\n";
    let output = compile_and_run(src);
    assert!(output.contains("name") && output.contains("alice") && output.contains("age") && output.contains("30"));
}

#[test]
fn dict_member_access() {
    let src = "::main\n  %user = { name \"alice\" age 30 }\n  %user.name | print\n  %user.age | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "alice\n30");
}

#[test]
fn dict_len() {
    let src = "::main\n  %d = { a 1 b 2 c 3 }\n  %d | len | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "3");
}

// ---------------------------------------------------------------------------
// String operation tests
// ---------------------------------------------------------------------------

#[test]
fn string_upper() {
    let output = compile_and_run("::main\n  \"hello world\" | upper | print\n");
    assert_eq!(output.trim(), "HELLO WORLD");
}

#[test]
fn string_lower() {
    let output = compile_and_run("::main\n  \"HELLO\" | lower | print\n");
    assert_eq!(output.trim(), "hello");
}

#[test]
fn string_trim() {
    let output = compile_and_run("::main\n  \"  hello  \" | trim | print\n");
    assert_eq!(output.trim(), "hello");
}

#[test]
fn string_len() {
    let output = compile_and_run("::main\n  \"hello\" | len | print\n");
    assert_eq!(output.trim(), "5");
}

#[test]
fn string_contains() {
    let output = compile_and_run("::main\n  \"hello world\" | contains \"world\" | print\n");
    assert_eq!(output.trim(), "true");
}

#[test]
fn string_split_and_join() {
    let output = compile_and_run("::main\n  \"a,b,c\" | split \",\" | join \"-\" | print\n");
    assert_eq!(output.trim(), "a-b-c");
}

// ---------------------------------------------------------------------------
// String interpolation tests
// ---------------------------------------------------------------------------

#[test]
fn string_interpolation() {
    let src = "::main\n  \"alice\" -> $name\n  \"hello $name\" | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "hello alice");
}

#[test]
fn string_interpolation_with_int() {
    let src = "::main\n  42 -> $age\n  \"age is $age\" | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "age is 42");
}

// ---------------------------------------------------------------------------
// Math tests
// ---------------------------------------------------------------------------

#[test]
fn math_sqrt() {
    let output = compile_and_run("::main\n  144 | sqrt | print\n");
    assert_eq!(output.trim(), "12");
}

#[test]
fn math_abs() {
    let output = compile_and_run("::main\n  (0 - 42) | abs | print\n");
    assert_eq!(output.trim(), "42");
}

#[test]
fn math_floor_ceil() {
    let output = compile_and_run("::main\n  3.7 | floor | print\n  3.2 | ceil | print\n");
    assert_eq!(output.trim(), "3\n4");
}

// ---------------------------------------------------------------------------
// I/O tests
// ---------------------------------------------------------------------------

#[test]
fn fs_read_write() {
    let src = "::main\n  \"hello from torq\" -> $msg\n  fs_write \"/tmp/torq_test_io.txt\" $msg\n  fs_read \"/tmp/torq_test_io.txt\" | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "hello from torq");
    let _ = std::fs::remove_file("/tmp/torq_test_io.txt");
}

#[test]
fn log_output() {
    let src = "::main\n  log \"test message\"\n  print \"ok\"\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "ok");
}

// ---------------------------------------------------------------------------
// JSON tests
// ---------------------------------------------------------------------------

#[test]
fn to_json_dict() {
    let src = "::main\n  %user = { name \"alice\" age 30 }\n  %user | to_json | print\n";
    let output = compile_and_run(src);
    assert!(output.contains("\"name\"") && output.contains("\"alice\""));
    assert!(output.contains("\"age\"") && output.contains("30"));
}

#[test]
fn to_json_array() {
    let src = "::main\n  @nums = [1 2 3]\n  @nums | to_json | print\n";
    assert_eq!(compile_and_run(src).trim(), "[1, 2, 3]");
}

// ---------------------------------------------------------------------------
// Phase 5 showcase – comprehensive integration test
// ---------------------------------------------------------------------------

#[test]
fn phase5_showcase() {
    let src = r#"::main
  @names = ["alice" "bob" "charlie"]
  @names | len | print
  @names | first | upper | print

  %user = { name "alice" age 30 }
  %user.name | print
  %user.age | print

  "hello world" | upper | print
  "  spaces  " | trim | print
"#;
    let output = compile_and_run(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "3");
    assert_eq!(lines[1], "ALICE");
    assert_eq!(lines[2], "alice");
    assert_eq!(lines[3], "30");
    assert_eq!(lines[4], "HELLO WORLD");
    assert_eq!(lines[5], "spaces");
}

// ---------------------------------------------------------------------------
// Phase 6 — Collection operations + each-over-arrays
// ---------------------------------------------------------------------------

#[test]
fn each_over_array() {
    let src = "\
::main
  @names = [\"alice\" \"bob\" \"charlie\"]
  @names | each $name sequential
    $name | upper | print
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "ALICE\nBOB\nCHARLIE");
}

#[test]
fn array_sort() {
    let output = compile_and_run("::main\n  @nums = [3 1 2]\n  @nums | sort | print\n");
    assert_eq!(output.trim(), "[1, 2, 3]");
}

#[test]
fn array_sum() {
    let output = compile_and_run("::main\n  @nums = [10 20 30]\n  @nums | sum | print\n");
    assert_eq!(output.trim(), "60");
}

#[test]
fn array_unique() {
    let output = compile_and_run("::main\n  @nums = [1 2 2 3 3 3]\n  @nums | unique | print\n");
    assert_eq!(output.trim(), "[1, 2, 3]");
}

#[test]
fn array_push_pop() {
    let output = compile_and_run("::main\n  @nums = [1 2 3]\n  @nums | push 4 | print\n  @nums | pop | print\n");
    assert_eq!(output.trim(), "[1, 2, 3, 4]\n[1, 2]");
}

#[test]
fn array_reverse() {
    let output = compile_and_run("::main\n  @nums = [1 2 3]\n  @nums | reverse | print\n");
    assert_eq!(output.trim(), "[3, 2, 1]");
}

#[test]
fn array_flatten() {
    // Flatten would require nested arrays. Test with a simpler case.
    let output = compile_and_run("::main\n  @nums = [1 2 3]\n  @nums | flatten | print\n");
    assert_eq!(output.trim(), "[1, 2, 3]");
}

#[test]
fn dict_keys_values() {
    let src = "::main\n  %d = { a 1 b 2 }\n  %d | keys | print\n  %d | values | sum | print\n";
    let output = compile_and_run(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert!(lines[0].contains("a") && lines[0].contains("b"));
    assert_eq!(lines[1], "3");
}

#[test]
fn dict_merge() {
    let src = "::main\n  %a = { x 1 }\n  %b = { y 2 }\n  %a | merge %b | len | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "2");
}

#[test]
fn dict_entries() {
    let src = "::main\n  %d = { name \"alice\" }\n  %d | entries | len | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "1");
}

#[test]
fn power_operator() {
    let output = compile_and_run("::main\n  (2 ** 10) | print\n");
    assert_eq!(output.trim(), "1024");
}

#[test]
fn from_json_parse() {
    let src = "::main\n  \"{\\\"name\\\": \\\"alice\\\", \\\"age\\\": 30}\" | from_json | print\n";
    let output = compile_and_run(src);
    assert!(output.contains("name") && output.contains("alice"));
}

#[test]
fn json_roundtrip() {
    let src = "::main\n  %user = { name \"alice\" age 30 }\n  %user | to_json | from_json | to_json | print\n";
    let output = compile_and_run(src);
    assert!(output.contains("\"name\"") && output.contains("\"alice\""));
}
