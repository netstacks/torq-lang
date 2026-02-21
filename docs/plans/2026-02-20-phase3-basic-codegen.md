# Phase 3: Basic Codegen Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** `torqc build hello.torq` produces a native binary that prints "hello world" when executed.

**Architecture:** A new `torqc-codegen` crate translates the AST into Cranelift IR, emits an object file (.o), then invokes the system C compiler (`cc`) to link against libc and produce a standalone native binary. For Phase 3, there is no custom runtime library — `print` maps directly to libc's `puts()`. The runtime crate (`torqc-runtime`) is deferred to Phase 5 when we need stdlib implementations beyond what libc provides.

**Tech Stack:** Rust, Cranelift 0.128.3 (cranelift-codegen, cranelift-frontend, cranelift-module, cranelift-object, cranelift-native), target-lexicon 0.13

**Depends on:** Phase 2 complete (lexer, parser, semantic analysis, CLI)

---

### Task 1: Create torqc-codegen crate with Cranelift dependencies

**Files:**
- Create: `compiler/crates/torqc-codegen/Cargo.toml`
- Create: `compiler/crates/torqc-codegen/src/lib.rs`
- Modify: `compiler/Cargo.toml` (add workspace member)

**Step 1: Create Cargo.toml**

```toml
[package]
name = "torqc-codegen"
version = "0.1.0"
edition = "2021"

[dependencies]
torqc-ast = { path = "../torqc-ast" }
cranelift-codegen = "0.128"
cranelift-frontend = "0.128"
cranelift-module = "0.128"
cranelift-object = "0.128"
cranelift-native = "0.128"
target-lexicon = "0.13"
```

**Step 2: Create initial lib.rs**

```rust
pub mod codegen;
```

Create `compiler/crates/torqc-codegen/src/codegen.rs` as an empty placeholder:

```rust
// Codegen module — will be populated in subsequent tasks.
```

**Step 3: Add workspace member**

Add `"crates/torqc-codegen"` to `compiler/Cargo.toml` members list.

**Step 4: Verify it builds**

Run: `cargo build -p torqc-codegen`
Expected: Compiles (Cranelift dependencies download and build — may take a minute)

**Step 5: Commit**

```bash
git add compiler/crates/torqc-codegen/ compiler/Cargo.toml
git commit -m "feat(codegen): create torqc-codegen crate with Cranelift dependencies"
```

---

### Task 2: Host ISA setup and module initialization

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`

Set up the Cranelift compiler context: detect host ISA (x86_64 or aarch64), create an ObjectModule, and provide a builder API.

**Step 1: Write the implementation with tests**

```rust
use std::fmt;

use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_module::Module;
use cranelift_object::{ObjectBuilder, ObjectModule};
use torqc_ast::ast::*;

#[derive(Debug)]
pub struct CodegenError {
    pub message: String,
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "codegen error: {}", self.message)
    }
}

impl std::error::Error for CodegenError {}

impl CodegenError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

pub struct Compiler {
    module: ObjectModule,
}

impl Compiler {
    /// Create a new compiler targeting the host platform.
    pub fn new() -> Result<Self, CodegenError> {
        let mut flag_builder = settings::builder();
        // Enable position-independent code for linking
        flag_builder
            .set("is_pic", "true")
            .map_err(|e| CodegenError::new(format!("failed to set is_pic: {}", e)))?;
        // Use standard optimizations
        flag_builder
            .set("opt_level", "speed")
            .map_err(|e| CodegenError::new(format!("failed to set opt_level: {}", e)))?;

        let isa_builder = cranelift_native::builder()
            .map_err(|e| CodegenError::new(format!("failed to detect host ISA: {}", e)))?;

        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| CodegenError::new(format!("failed to build ISA: {}", e)))?;

        let obj_builder = ObjectBuilder::new(
            isa,
            "torq_output",
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| CodegenError::new(format!("failed to create object builder: {}", e)))?;

        let module = ObjectModule::new(obj_builder);

        Ok(Self { module })
    }

    /// Compile a TORQ Program and return the object file bytes.
    pub fn compile(self, _program: &Program) -> Result<Vec<u8>, CodegenError> {
        // Will be implemented in subsequent tasks.
        // For now, just finalize the empty module.
        let product = self.module.finish();
        let bytes = product.emit()
            .map_err(|e| CodegenError::new(format!("failed to emit object: {}", e)))?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_creates_successfully() {
        let compiler = Compiler::new();
        assert!(compiler.is_ok());
    }

    #[test]
    fn empty_program_emits_object() {
        let compiler = Compiler::new().unwrap();
        let program = Program { blocks: vec![] };
        let bytes = compiler.compile(&program);
        assert!(bytes.is_ok());
        assert!(!bytes.unwrap().is_empty());
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p torqc-codegen`
Expected: 2 tests pass

**Step 3: Commit**

```bash
git add compiler/crates/torqc-codegen/
git commit -m "feat(codegen): host ISA detection and ObjectModule initialization"
```

---

### Task 3: Codegen for main function — skeleton with exit code

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`

Generate a C-ABI `main` function from the `::main` block that returns 0. This is the skeleton that later tasks will fill with actual block body codegen.

**Step 1: Implement main function generation**

The `compile` method needs to:
1. Find the `::main` block in the program (error if missing)
2. Declare a function `main` with signature `() -> i32` using C calling convention
3. Build the function body: for now, just `return 0`
4. Define the function in the module
5. Finalize and emit

Key Cranelift APIs:
```rust
use cranelift_codegen::ir::{types, AbiParam, Function, Signature};
use cranelift_codegen::ir::InstBuilder;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{FuncId, Linkage, Module};
```

Steps inside `compile`:
```rust
// 1. Find ::main block
let main_block = program.blocks.iter()
    .find(|b| b.name == "main")
    .ok_or_else(|| CodegenError::new("no ::main block found"))?;

// 2. Declare main function
let mut sig = self.module.make_signature();
sig.returns.push(AbiParam::new(types::I32));
sig.call_conv = CallConv::SystemV; // or platform-appropriate

let func_id = self.module.declare_function("main", Linkage::Export, &sig)
    .map_err(|e| CodegenError::new(format!("declare main: {}", e)))?;

// 3. Build function body
let mut ctx = self.module.make_context();
ctx.func.signature = sig;

let mut func_ctx = FunctionBuilderContext::new();
let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

let entry_block = builder.create_block();
builder.switch_to_block(entry_block);
builder.seal_block(entry_block);

// Return 0
let zero = builder.ins().iconst(types::I32, 0);
builder.ins().return_(&[zero]);
builder.finalize();

// 4. Define function
self.module.define_function(func_id, &mut ctx)
    .map_err(|e| CodegenError::new(format!("define main: {}", e)))?;

// 5. Emit
let product = self.module.finish();
let bytes = product.emit()
    .map_err(|e| CodegenError::new(format!("emit: {}", e)))?;
Ok(bytes)
```

**Step 2: Write tests**

```rust
#[test]
fn missing_main_block_errors() {
    let compiler = Compiler::new().unwrap();
    let program = Program { blocks: vec![] };
    let result = compiler.compile(&program);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("no ::main block"));
}

#[test]
fn main_block_emits_object() {
    let compiler = Compiler::new().unwrap();
    let program = Program {
        blocks: vec![Block {
            name: "main".to_string(),
            params: vec![],
            body: vec![],
            doc_comments: vec![],
            span: Span { file: "test.torq".into(), line: 1, col: 1 },
        }],
    };
    let bytes = compiler.compile(&program).unwrap();
    assert!(!bytes.is_empty());
}
```

**Step 3: Run tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass

**Step 4: Commit**

```bash
git add compiler/crates/torqc-codegen/
git commit -m "feat(codegen): generate main function skeleton returning 0"
```

---

### Task 4: String data sections and print call generation

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`

This is the core task — handle `print "hello world"` by:
1. Storing the string as a null-terminated data section
2. Declaring libc's `puts` as an imported function
3. Generating a call to `puts` with a pointer to the string data

**Step 1: Add string data handling**

Add a helper to store a string constant:
```rust
use cranelift_module::DataDescription;

fn create_string_data(
    module: &mut ObjectModule,
    name: &str,
    value: &str,
) -> Result<cranelift_module::DataId, CodegenError> {
    let data_id = module
        .declare_data(name, Linkage::Local, false, false)
        .map_err(|e| CodegenError::new(format!("declare data: {}", e)))?;

    let mut data_desc = DataDescription::new();
    // Null-terminate for puts
    let mut bytes = value.as_bytes().to_vec();
    bytes.push(0);
    data_desc.define(bytes.into_boxed_slice());

    module.define_data(data_id, &data_desc)
        .map_err(|e| CodegenError::new(format!("define data: {}", e)))?;

    Ok(data_id)
}
```

**Step 2: Declare puts and generate call**

In the function body builder, for a `Statement::Expression(Expr::Call(call))` where `call.name == "print"` and the argument is a string literal:

```rust
// Declare puts: fn(ptr) -> i32
let mut puts_sig = self.module.make_signature();
puts_sig.params.push(AbiParam::new(self.module.target_config().pointer_type()));
puts_sig.returns.push(AbiParam::new(types::I32));

let puts_func = self.module.declare_function("puts", Linkage::Import, &puts_sig)
    .map_err(|e| CodegenError::new(format!("declare puts: {}", e)))?;

// For each print "string":
// 1. Create data section for the string
let data_id = create_string_data(&mut self.module, "str_0", "hello world")?;

// 2. In the function builder, get a pointer to the data
let data_gv = self.module.declare_data_in_func(data_id, builder.func);
let ptr = builder.ins().global_value(pointer_type, data_gv);

// 3. Call puts
let puts_ref = self.module.declare_func_in_func(puts_func, builder.func);
builder.ins().call(puts_ref, &[ptr]);
```

**Step 3: Walk the ::main block body**

Implement a basic statement codegen that handles:
- `Statement::Expression(Expr::Call(c))` where `c.name == "print"` with a string literal arg → calls puts
- Everything else → skip for now (or error)

**Step 4: Write tests**

```rust
#[test]
fn print_string_emits_object() {
    let compiler = Compiler::new().unwrap();
    let program = Program {
        blocks: vec![Block {
            name: "main".to_string(),
            params: vec![],
            body: vec![Statement::Expression(Expr::Call(Call {
                name: "print".to_string(),
                args: vec![Expr::Literal(Literal::String(
                    "hello world".to_string(),
                    Span { file: "test.torq".into(), line: 2, col: 3 },
                ))],
                span: Span { file: "test.torq".into(), line: 2, col: 3 },
            }))],
            doc_comments: vec![],
            span: Span { file: "test.torq".into(), line: 1, col: 1 },
        }],
    };
    let bytes = compiler.compile(&program).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn multiple_print_calls() {
    let compiler = Compiler::new().unwrap();
    let program = Program {
        blocks: vec![Block {
            name: "main".to_string(),
            params: vec![],
            body: vec![
                Statement::Expression(Expr::Call(Call {
                    name: "print".to_string(),
                    args: vec![Expr::Literal(Literal::String(
                        "line one".to_string(),
                        Span { file: "test.torq".into(), line: 2, col: 3 },
                    ))],
                    span: Span { file: "test.torq".into(), line: 2, col: 3 },
                })),
                Statement::Expression(Expr::Call(Call {
                    name: "print".to_string(),
                    args: vec![Expr::Literal(Literal::String(
                        "line two".to_string(),
                        Span { file: "test.torq".into(), line: 3, col: 3 },
                    ))],
                    span: Span { file: "test.torq".into(), line: 3, col: 3 },
                })),
            ],
            doc_comments: vec![],
            span: Span { file: "test.torq".into(), line: 1, col: 1 },
        }],
    };
    let bytes = compiler.compile(&program).unwrap();
    assert!(!bytes.is_empty());
}
```

**Step 5: Run tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass

**Step 6: Commit**

```bash
git add compiler/crates/torqc-codegen/
git commit -m "feat(codegen): string data sections and print via libc puts"
```

---

### Task 5: Integer literal printing

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`

Handle `print 42` by converting the integer to a string at compile time and calling `puts`. This is the simplest approach — no need for `printf` formatting yet.

**Step 1: Extend print handling**

When `print` is called with a `Literal::Int(n, _)`:
- Convert `n` to string at compile time: `n.to_string()`
- Store as a data section (same as string literals)
- Call `puts` with the pointer

When `print` is called with a `Literal::Bool(b, _)`:
- Store `"true"` or `"false"` as data
- Call `puts`

When `print` is called with a `Literal::Null(_)`:
- Store `"null"` as data
- Call `puts`

**Step 2: Write tests**

```rust
#[test]
fn print_integer() {
    let compiler = Compiler::new().unwrap();
    let program = Program {
        blocks: vec![Block {
            name: "main".to_string(),
            params: vec![],
            body: vec![Statement::Expression(Expr::Call(Call {
                name: "print".to_string(),
                args: vec![Expr::Literal(Literal::Int(42, Span { file: "t".into(), line: 1, col: 1 }))],
                span: Span { file: "t".into(), line: 1, col: 1 },
            }))],
            doc_comments: vec![],
            span: Span { file: "t".into(), line: 1, col: 1 },
        }],
    };
    let bytes = compiler.compile(&program).unwrap();
    assert!(!bytes.is_empty());
}
```

**Step 3: Run tests**

Run: `cargo test -p torqc-codegen`

**Step 4: Commit**

```bash
git add compiler/crates/torqc-codegen/
git commit -m "feat(codegen): print integer and boolean literals"
```

---

### Task 6: Object file emission and linking

**Files:**
- Create: `compiler/crates/torqc-codegen/src/linker.rs`
- Modify: `compiler/crates/torqc-codegen/src/lib.rs`

Provide a function that takes object file bytes, writes them to a temp file, invokes the system C compiler (`cc`) to link, and produces a native executable.

**Step 1: Implement the linker**

```rust
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::codegen::CodegenError;

/// Link an object file into a native executable using the system C compiler.
pub fn link(
    object_bytes: &[u8],
    output_path: &Path,
) -> Result<PathBuf, CodegenError> {
    // Write object bytes to a temp file
    let temp_dir = std::env::temp_dir();
    let obj_path = temp_dir.join("torq_output.o");
    std::fs::write(&obj_path, object_bytes)
        .map_err(|e| CodegenError::new(format!("failed to write object file: {}", e)))?;

    // Invoke cc to link
    let output = Command::new("cc")
        .arg(&obj_path)
        .arg("-o")
        .arg(output_path)
        .output()
        .map_err(|e| CodegenError::new(format!("failed to invoke linker (cc): {}", e)))?;

    // Clean up temp file
    let _ = std::fs::remove_file(&obj_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CodegenError::new(format!("linker failed: {}", stderr)));
    }

    Ok(output_path.to_path_buf())
}
```

**Step 2: Update lib.rs**

```rust
pub mod codegen;
pub mod linker;
```

**Step 3: Write a test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::Compiler;
    use torqc_ast::ast::*;

    #[test]
    fn link_produces_executable() {
        let compiler = Compiler::new().unwrap();
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![],
                doc_comments: vec![],
                span: Span { file: "t".into(), line: 1, col: 1 },
            }],
        };
        let bytes = compiler.compile(&program).unwrap();

        let temp = std::env::temp_dir().join("torq_test_binary");
        let result = link(&bytes, &temp);
        assert!(result.is_ok(), "link failed: {:?}", result.err());

        // Verify the file exists and is executable
        assert!(temp.exists());
        let _ = std::fs::remove_file(&temp);
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p torqc-codegen`

**Step 5: Commit**

```bash
git add compiler/crates/torqc-codegen/
git commit -m "feat(codegen): system linker integration for native binary output"
```

---

### Task 7: CLI `build` command

**Files:**
- Modify: `compiler/crates/torqc-cli/Cargo.toml` (add torqc-codegen dependency)
- Modify: `compiler/crates/torqc-cli/src/main.rs` (add Build subcommand)

**Step 1: Add dependency**

```toml
torqc-codegen = { path = "../torqc-codegen" }
```

**Step 2: Add Build subcommand**

```rust
/// Compile a .torq file into a native binary
Build {
    /// Path to the .torq source file
    file: String,
    /// Output binary path (defaults to filename without extension)
    #[arg(short, long)]
    output: Option<String>,
},
```

**Step 3: Add run_build function**

```rust
fn run_build(path: &str, output: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read '{}': {}", path, e))?;

    // Lex
    let tokens = Lexer::tokenize(&source, path)
        .map_err(|e| format!("lex error: {}", e))?;

    // Parse
    let program = parser::parse(tokens, path)
        .map_err(|e| format!("parse error: {}", e))?;

    // Semantic analysis
    let analysis = torqc_semantic::analyzer::analyze(&program);
    for diag in &analysis.diagnostics {
        eprintln!("{}", diag);
    }
    if analysis.has_errors() {
        return Err(format!(
            "{} error(s) found, cannot compile",
            analysis.error_count()
        ).into());
    }

    // Codegen
    eprintln!("\u{2713} Parsed {} block(s)", program.blocks.len());

    let compiler = torqc_codegen::codegen::Compiler::new()
        .map_err(|e| format!("codegen init: {}", e))?;

    let object_bytes = compiler.compile(&program)
        .map_err(|e| format!("codegen: {}", e))?;

    // Determine output path
    let output_path = match output {
        Some(o) => std::path::PathBuf::from(o),
        None => {
            let stem = std::path::Path::new(path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            std::path::PathBuf::from(stem)
        }
    };

    // Link
    torqc_codegen::linker::link(&object_bytes, &output_path)
        .map_err(|e| format!("link: {}", e))?;

    eprintln!("\u{2713} Built {}", output_path.display());

    Ok(())
}
```

**Step 4: Wire up match arm**

```rust
Commands::Build { file, output } => {
    if let Err(e) = run_build(&file, output.as_deref()) {
        eprintln!("error: {}", e);
        process::exit(1);
    }
}
```

**Step 5: Build and verify**

Run: `cargo build -p torqc-cli`
Expected: Compiles

**Step 6: Commit**

```bash
git add compiler/crates/torqc-cli/
git commit -m "feat(cli): add torqc build command for native binary compilation"
```

---

### Task 8: End-to-end test — hello.torq compiles and runs

**Files:**
- Create: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

This is the critical integration test: compile hello.torq, link it, run it, verify the output.

**Step 1: Write the integration test**

```rust
use std::path::PathBuf;
use std::process::Command;

use torqc_ast::ast::*;
use torqc_codegen::codegen::Compiler;
use torqc_codegen::linker;
use torqc_lexer::lexer::Lexer;
use torqc_parser::parser;

fn compile_and_run(source: &str) -> String {
    let tokens = Lexer::tokenize(source, "test.torq").expect("lex failed");
    let program = parser::parse(tokens, "test.torq").expect("parse failed");

    let compiler = Compiler::new().expect("compiler init failed");
    let bytes = compiler.compile(&program).expect("compile failed");

    let temp_bin = std::env::temp_dir().join("torq_test_run");
    linker::link(&bytes, &temp_bin).expect("link failed");

    let output = Command::new(&temp_bin)
        .output()
        .expect("failed to run binary");

    let _ = std::fs::remove_file(&temp_bin);

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn hello_world() {
    let source = "::main\n  print \"hello world\"\n";
    let output = compile_and_run(source);
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn multiple_prints() {
    let source = "::main\n  print \"line one\"\n  print \"line two\"\n";
    let output = compile_and_run(source);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines, vec!["line one", "line two"]);
}

#[test]
fn print_integer() {
    let source = "::main\n  print 42\n";
    let output = compile_and_run(source);
    assert_eq!(output.trim(), "42");
}

#[test]
fn hello_torq_example_file() {
    let source = std::fs::read_to_string("../../examples/hello.torq")
        .expect("read hello.torq");
    let output = compile_and_run(&source);
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn empty_main_runs_cleanly() {
    let source = "::main\n  print \"\"\n";
    let output = compile_and_run(source);
    // puts("") prints an empty line
    assert_eq!(output.trim(), "");
}
```

**Step 2: Add dev-dependencies to torqc-codegen Cargo.toml**

```toml
[dev-dependencies]
torqc-lexer = { path = "../torqc-lexer" }
torqc-parser = { path = "../torqc-parser" }
```

**Step 3: Run the integration tests**

Run: `cargo test -p torqc-codegen --test codegen_tests`
Expected: All tests pass (including hello_torq_example_file)

**Step 4: Also test via CLI**

Run: `cargo run -p torqc-cli -- build ../../examples/hello.torq -o /tmp/hello_torq && /tmp/hello_torq`
Expected output: `hello world`

**Step 5: Commit**

```bash
git add compiler/crates/torqc-codegen/
git commit -m "feat(codegen): end-to-end integration tests — hello.torq compiles and runs"
```

---

### Task 9: Pipeline expression codegen (basic)

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`

Handle basic pipeline statements where the pipeline ends with `print`. For example:
```torq
::main
  "hello" | print
```

This is parsed as `Statement::Pipeline { stages: [Literal::String("hello"), Call("print")] }`. The codegen needs to recognize that the last stage is `print` and treat the pipeline input as the print argument.

**Step 1: Add pipeline codegen**

When processing `Statement::Pipeline(p)`:
- If the pipeline has exactly 2 stages where the last stage is `Call("print")` and the first stage is a string/int literal, treat it as `print <literal>`.
- This is a simplified version — full pipeline codegen comes in later phases.

**Step 2: Write test**

```rust
#[test]
fn pipeline_to_print() {
    let source = "::main\n  \"hello pipeline\" | print\n";
    let output = compile_and_run(source);
    assert_eq!(output.trim(), "hello pipeline");
}
```

**Step 3: Run tests**

Run: `cargo test -p torqc-codegen`

**Step 4: Commit**

```bash
git add compiler/crates/torqc-codegen/
git commit -m "feat(codegen): basic pipeline-to-print codegen"
```

---

### Task 10: Final cleanup and CLAUDE.md update

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Fix any warnings.

**Step 2: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests pass

**Step 3: Verify hello.torq end-to-end**

Run:
```bash
cd compiler
cargo run -p torqc-cli -- build ../examples/hello.torq -o /tmp/hello_torq
/tmp/hello_torq
```
Expected: Prints "hello world"

**Step 4: Update CLAUDE.md**

Add:
- `cargo run -p torqc-cli -- build <file.torq>` command
- `torqc-codegen` crate description (Cranelift-based, generates native object files, links via system cc)
- Update crate count from 5 to 6
- Update test count

**Step 5: Commit**

```bash
git add -A
git commit -m "feat(codegen): phase 3 complete — torqc build produces native binaries"
```

---

## Summary

After Phase 3, `torqc build hello.torq` produces a native binary that:
- Finds the `::main` block
- Generates Cranelift IR for `print` calls (string literals, integers, booleans)
- Handles basic `literal | print` pipelines
- Emits an object file and links with the system C compiler
- Produces a standalone executable with no runtime dependencies (just libc)

**What works:** `print` with string/integer/boolean literals, basic pipelines ending in print, multiple print calls.

**What doesn't yet:** Variables, assignments, block calls, control flow (match/each/loop), arithmetic — those come in Phase 4+.
