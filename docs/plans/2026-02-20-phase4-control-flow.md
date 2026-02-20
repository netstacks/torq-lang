# Phase 4: Control Flow Codegen Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Compile `fibonacci.torq` to a working native binary — requires variables, arithmetic, user-defined blocks, match expressions, and each sequential with range.

**Architecture:** Phase 4 introduces a minimal C runtime (embedded as source, compiled at link time) for printing runtime values. The codegen is refactored from a flat print-only emitter to an expression-based compiler that returns typed Cranelift `Value`s. Each TORQ `::block` compiles to a Cranelift function. Match compiles to compare-and-branch basic blocks. Each sequential + range compiles to a counted for-loop.

**Tech Stack:** Cranelift 0.128, Rust, minimal C runtime (printf-based helpers)

---

## Context

**Current state (Phase 3):** The codegen handles only `::main` with `print` calls on literals. All literals are converted to strings at compile time and printed via libc `puts`. No variables, no arithmetic, no user blocks, no control flow.

**Phase 4 milestone from design doc:** `match`, `each sequential`, `loop` work.

**Target program — `fibonacci.torq`:**
```
::fibonacci $n
  $n | match
    0 -> 0
    1 -> 1
    _ -> ::fibonacci ($n - 1) + ::fibonacci ($n - 2)

::main
  range 1 20 | each $n sequential
    ::fibonacci $n | print
```

**AST structure (from `torqc parse`):**
- `::fibonacci` body: `Pipeline([Variable($n), Match(3 arms)])`
  - Arm 1: `Literal(Int(0))` → `Literal(Int(0))`
  - Arm 2: `Literal(Int(1))` → `Literal(Int(1))`
  - Arm 3: `Wildcard` → `BinOp(Add, BlockCall("fibonacci",[Group(BinOp(Sub,$n,1))]), BlockCall("fibonacci",[Group(BinOp(Sub,$n,2))]))`
- `::main` body: `Each { iterable: Call("range",[1,20]), binding: $n, sequential: true, body: [Pipeline([BlockCall("fibonacci",[$n]), Call("print",[])])] }`
- Note: `print` in the pipeline has **no args** — it receives the pipe value

**Key constraints:**
- Cranelift cannot call variadic C functions (like `printf`). Solution: embed a C runtime with non-variadic print helpers.
- All values are `i64` in Phase 4 (integers, booleans as 0/1). String literals remain as pointers to data sections.
- Type tracking is compile-time only — no runtime tags needed yet.
- `range` + `each sequential` compiles to a simple counted for-loop (no array allocation).

---

### Task 1: Embed C Runtime + Update Linker

**Goal:** Replace libc `puts` with our own runtime print functions. Existing tests must still pass after this change.

**Files:**
- Create: `compiler/crates/torqc-codegen/src/runtime.rs`
- Modify: `compiler/crates/torqc-codegen/src/lib.rs`
- Modify: `compiler/crates/torqc-codegen/src/linker.rs`
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`

**Step 1: Create `runtime.rs` with embedded C source**

```rust
// compiler/crates/torqc-codegen/src/runtime.rs

/// Minimal C runtime for TORQ programs.
/// Compiled alongside user object code at link time.
pub const RUNTIME_C_SOURCE: &str = r#"
#include <stdio.h>
#include <stdint.h>

void torq_print_int(int64_t n) {
    printf("%lld\n", (long long)n);
}

void torq_print_str(const char* s) {
    puts(s);
}

void torq_print_bool(int64_t b) {
    puts(b ? "true" : "false");
}

void torq_print_float(double f) {
    printf("%g\n", f);
}

void torq_print_null(void) {
    puts("null");
}
"#;
```

**Step 2: Update `lib.rs`**

Add `pub mod runtime;` to `compiler/crates/torqc-codegen/src/lib.rs`.

**Step 3: Update linker to compile runtime alongside user code**

Modify `linker::link()` to:
1. Write `runtime.c` to temp alongside `torq_output.o`
2. Pass both files to `cc`: `cc torq_output.o runtime.c -o output`

```rust
// In link():
let runtime_path = temp_dir.join("torq_runtime.c");
std::fs::write(&runtime_path, crate::runtime::RUNTIME_C_SOURCE)
    .map_err(|e| CodegenError::new(format!("failed to write runtime: {}", e)))?;

let mut cmd = Command::new("cc");
cmd.arg(&obj_path).arg(&runtime_path).arg("-o").arg(output_path);

// ... existing arch detection ...

// Clean up both temp files
let _ = std::fs::remove_file(&obj_path);
let _ = std::fs::remove_file(&runtime_path);
```

**Step 4: Update codegen to use runtime print functions instead of `puts`**

Replace the `puts` import with runtime function imports:
- `torq_print_str(ptr)` — for string literals
- `torq_print_int(i64)` — for integer literals
- `torq_print_bool(i64)` — for boolean literals
- `torq_print_float(f64)` — for float literals (stretch)
- `torq_print_null()` — for null

In `Compiler::compile()`, replace the puts declaration with declarations for each runtime function. Update `emit_print_literal()` to call the type-specific runtime function instead of converting everything to strings and calling puts.

For string literals: create data section → call `torq_print_str(ptr)`
For integer literals: push iconst → call `torq_print_int(i64)`
For boolean literals: push iconst(0 or 1) → call `torq_print_bool(i64)`
For null: call `torq_print_null()` (no args)

**Step 5: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All 17 existing tests pass (7 unit + 10 integration). The output is identical — runtime functions produce the same text as the old puts-based approach.

**Step 6: Commit**

```bash
git add compiler/crates/torqc-codegen/src/runtime.rs \
        compiler/crates/torqc-codegen/src/lib.rs \
        compiler/crates/torqc-codegen/src/linker.rs \
        compiler/crates/torqc-codegen/src/codegen.rs
git commit -m "feat(codegen): embed C runtime with typed print functions

Replace libc puts with runtime helpers (torq_print_int, torq_print_str,
torq_print_bool, torq_print_null). Runtime C source is embedded in the
compiler and compiled alongside user code at link time."
```

---

### Task 2: Expression-Based Codegen Refactor

**Goal:** Refactor codegen from flat statement walking to expression-based compilation. Every expression returns a `(Value, TorqType)` pair. Pipelines thread a pipe value through stages. This is the foundation for all subsequent tasks.

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`

**Architecture:**

Add a value type enum:
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TorqType {
    I64,
    Ptr,   // pointer to null-terminated string
    Bool,
    F64,
    Void,
}
```

Add a `FuncCompiler` struct that handles compilation within a single function:
```rust
struct FuncCompiler<'a> {
    module: &'a mut ObjectModule,
    builder: FunctionBuilder<'a>,
    pointer_type: Type,
    str_counter: usize,
    variables: HashMap<String, (Variable, TorqType)>,
    var_index: usize,
    // Function IDs for runtime print functions
    print_int_id: FuncId,
    print_str_id: FuncId,
    print_bool_id: FuncId,
    print_null_id: FuncId,
}
```

Core methods:
```rust
impl FuncCompiler {
    fn compile_statement(&mut self, stmt: &Statement) -> Result<Option<(Value, TorqType)>, CodegenError>;
    fn compile_expr(&mut self, expr: &Expr, pipe_value: Option<(Value, TorqType)>) -> Result<(Value, TorqType), CodegenError>;
    fn compile_pipeline(&mut self, pipeline: &Pipeline) -> Result<Option<(Value, TorqType)>, CodegenError>;
    fn emit_print(&mut self, value: Value, ty: TorqType) -> Result<(), CodegenError>;
}
```

Pipeline compilation:
```rust
fn compile_pipeline(&mut self, stages: &[Expr]) -> Result<Option<(Value, TorqType)>, CodegenError> {
    let mut pipe_val: Option<(Value, TorqType)> = None;
    for stage in stages {
        match stage {
            Expr::Call(call) if call.name == "print" => {
                if let Some((val, ty)) = pipe_val.or_else(|| /* compile first arg */) {
                    self.emit_print(val, ty)?;
                }
                pipe_val = None; // print consumes the value
            }
            _ => {
                let result = self.compile_expr(stage, pipe_val)?;
                pipe_val = Some(result);
            }
        }
    }
    Ok(pipe_val)
}
```

Expression compilation handles:
- `Expr::Literal(Literal::Int(n, _))` → `builder.ins().iconst(I64, n)` returns `(val, TorqType::I64)`
- `Expr::Literal(Literal::String(s, _))` → create data section, global_value → `(ptr, TorqType::Ptr)`
- `Expr::Literal(Literal::Bool(b, _))` → iconst(I64, 0 or 1) → `(val, TorqType::Bool)`
- `Expr::Literal(Literal::Null(_))` → iconst(I64, 0) → `(val, TorqType::Void)`
- `Expr::Variable(var)` → look up in variables map, use_var → `(val, ty)`
- `Expr::Group(inner, _)` → compile_expr(inner, pipe_value)
- `Expr::Call(call) if call.name == "print"` → handled in pipeline
- Other expressions → `CodegenError("unsupported expression")` for now

The `print` builtin with explicit args (e.g., `print "hello"`, `print 42`) still works:
- `Statement::Expression(Expr::Call(call)) if call.name == "print"` → compile arg, emit_print

**Step 1: Add TorqType and refactor codegen**

Rewrite `Compiler::compile()` to:
1. Declare all runtime print functions upfront (already done in Task 1)
2. Build `::main` function using the new expression-based approach
3. Walk body using `compile_statement()` → `compile_pipeline()` → `compile_expr()`

**Step 2: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All 17 tests pass. Output is identical.

**Step 3: Commit**

```bash
git add compiler/crates/torqc-codegen/src/codegen.rs
git commit -m "refactor(codegen): expression-based compilation with TorqType

Replace flat emit_print_literal with compile_expr/compile_statement.
Expressions return (Value, TorqType) pairs. Pipelines thread pipe values
through stages. Foundation for variables, arithmetic, and control flow."
```

---

### Task 3: Variable Support

**Goal:** Support `$variable` assignment (`-> $var`) and reading. Test with assign + print.

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Write the failing test**

```rust
// in codegen_tests.rs
#[test]
fn variable_assign_and_print() {
    let output = compile_and_run("::main\n  42 -> $x\n  $x | print\n");
    assert_eq!(output.trim(), "42");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p torqc-codegen --test codegen_tests variable_assign_and_print`
Expected: FAIL — variable support not implemented.

**Step 3: Implement variable assignment and reading**

In `FuncCompiler`:
- `variables: HashMap<String, (Variable, TorqType)>` — maps variable names to Cranelift Variables
- `var_index: usize` — counter for unique Cranelift Variable indices

Assignment statement:
```rust
Statement::Assignment(assign) => {
    let (val, ty) = self.compile_expr(&assign.value, None)?;
    let var_name = &assign.target.name;
    let cl_var = if let Some((existing, _)) = self.variables.get(var_name) {
        *existing
    } else {
        let idx = self.var_index;
        self.var_index += 1;
        let cl_var = Variable::new(idx);
        // Declare variable type based on TorqType
        let cl_type = match ty {
            TorqType::I64 | TorqType::Bool => types::I64,
            TorqType::Ptr => self.pointer_type,
            TorqType::F64 => types::F64,
            TorqType::Void => types::I64,
        };
        self.builder.declare_var(cl_var, cl_type);
        self.variables.insert(var_name.clone(), (cl_var, ty));
        cl_var
    };
    self.builder.def_var(cl_var, val);
    Ok(Some((val, ty)))
}
```

Variable read:
```rust
Expr::Variable(var) => {
    if let Some((cl_var, ty)) = self.variables.get(&var.name) {
        let val = self.builder.use_var(*cl_var);
        Ok((val, *ty))
    } else {
        Err(CodegenError::new(format!("undefined variable ${}", var.name)))
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p torqc-codegen --test codegen_tests variable_assign_and_print`
Expected: PASS, output = "42"

**Step 5: Add more variable tests**

```rust
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
```

**Step 6: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass (17 existing + 3 new = 20).

**Step 7: Commit**

```bash
git add compiler/crates/torqc-codegen/src/codegen.rs \
        compiler/crates/torqc-codegen/tests/codegen_tests.rs
git commit -m "feat(codegen): variable assignment and reading

$var assignments store typed Cranelift Variables. Variable reads
retrieve values with compile-time type tracking."
```

---

### Task 4: Arithmetic and Comparison Operators

**Goal:** Compile `BinOp` expressions (Add, Sub, Mul, Div, Mod, comparisons) and `Group` (parenthesized expressions).

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Write failing tests**

```rust
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
```

**Step 2: Run to verify they fail**

Run: `cargo test -p torqc-codegen --test codegen_tests arithmetic`
Expected: FAIL

**Step 3: Implement BinOp and Group codegen**

```rust
Expr::BinOp(binop) => {
    let (left, _) = self.compile_expr(&binop.left, None)?;
    let (right, _) = self.compile_expr(&binop.right, None)?;
    let result = match binop.op {
        BinOpKind::Add => self.builder.ins().iadd(left, right),
        BinOpKind::Sub => self.builder.ins().isub(left, right),
        BinOpKind::Mul => self.builder.ins().imul(left, right),
        BinOpKind::Div => self.builder.ins().sdiv(left, right),
        BinOpKind::Mod => self.builder.ins().srem(left, right),
        BinOpKind::Gt  => { let c = self.builder.ins().icmp(IntCC::SignedGreaterThan, left, right); self.builder.ins().uextend(types::I64, c) }
        BinOpKind::Lt  => { let c = self.builder.ins().icmp(IntCC::SignedLessThan, left, right); self.builder.ins().uextend(types::I64, c) }
        BinOpKind::GtEq => { let c = self.builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, left, right); self.builder.ins().uextend(types::I64, c) }
        BinOpKind::LtEq => { let c = self.builder.ins().icmp(IntCC::SignedLessThanOrEqual, left, right); self.builder.ins().uextend(types::I64, c) }
        BinOpKind::Eq  => { let c = self.builder.ins().icmp(IntCC::Equal, left, right); self.builder.ins().uextend(types::I64, c) }
        BinOpKind::NotEq => { let c = self.builder.ins().icmp(IntCC::NotEqual, left, right); self.builder.ins().uextend(types::I64, c) }
        _ => return Err(CodegenError::new(format!("unsupported operator {:?}", binop.op))),
    };
    Ok((result, TorqType::I64))
}

Expr::Group(inner, _) => {
    self.compile_expr(inner, pipe_value)
}
```

Note: Comparisons return i64 (0 or 1) via `uextend` from the i8 `icmp` result.

**Step 4: Run to verify they pass**

Run: `cargo test -p torqc-codegen --test codegen_tests arithmetic`
Expected: All 4 PASS

**Step 5: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass (20 + 4 = 24).

**Step 6: Commit**

```bash
git add compiler/crates/torqc-codegen/src/codegen.rs \
        compiler/crates/torqc-codegen/tests/codegen_tests.rs
git commit -m "feat(codegen): arithmetic and comparison operators

BinOp compiles to Cranelift iadd/isub/imul/sdiv/srem for arithmetic,
icmp + uextend for comparisons. Group expressions pass through."
```

---

### Task 5: User-Defined Blocks (Functions)

**Goal:** Each `::block` compiles to a Cranelift function. Block calls compile to function calls. Blocks return the value of their last expression.

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Write failing test**

```rust
#[test]
fn user_block_call() {
    let src = "::double $n\n  ($n * 2)\n\n::main\n  ::double 21 | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "42");
}
```

**Step 2: Run to verify it fails**

Run: `cargo test -p torqc-codegen --test codegen_tests user_block_call`
Expected: FAIL

**Step 3: Implement multi-function codegen**

**Architecture change:** `Compiler::compile()` must handle multiple blocks:

1. **First pass — declare all functions:**
   For each block in the program, declare a Cranelift function with appropriate signature:
   - `::main`: `fn() -> i32` (exported, C convention)
   - Other blocks: `fn(i64, i64, ...) -> i64` (one i64 param per block param, returns i64)

   Store a map: `block_name → (FuncId, param_count)`.

2. **Second pass — define all functions:**
   For each block, create a FunctionBuilder, compile the body, finalize.
   - Block parameters become function parameters. For each param, declare a Cranelift Variable, def_var with the block_param value.
   - The return value is the result of the last statement/expression in the body.
   - For `::main`, the existing `return 0` logic stays. But if the body produces a value and it's a non-main block, return that value.

3. **BlockCall expression:**
   ```rust
   Expr::BlockCall(call) => {
       let (func_id, _param_count) = self.block_funcs.get(&call.name)
           .ok_or_else(|| CodegenError::new(format!("undefined block ::{}", call.name)))?;
       let func_ref = self.module.declare_func_in_func(*func_id, self.builder.func);
       let mut args = Vec::new();
       for arg_expr in &call.args {
           let (val, _ty) = self.compile_expr(arg_expr, None)?;
           args.push(val);
       }
       let inst = self.builder.ins().call(func_ref, &args);
       let result = self.builder.inst_results(inst)[0];
       Ok((result, TorqType::I64))
   }
   ```

**Step 4: Run to verify it passes**

Run: `cargo test -p torqc-codegen --test codegen_tests user_block_call`
Expected: PASS, output = "42"

**Step 5: Add recursive call test**

```rust
#[test]
fn recursive_block_call() {
    let src = "\
::factorial $n
  ($n * 1) | match
    0 -> 1
    _ -> ($n * ::factorial ($n - 1))

::main
  ::factorial 5 | print
";
    // Note: this test will need match (Task 6) to pass.
    // Skip for now — it's tested in Task 9 integration.
}
```

Actually, since this needs match, add a simpler recursive test instead:

```rust
#[test]
fn block_with_multiple_params() {
    let src = "::add $a $b\n  ($a + $b)\n\n::main\n  ::add 19 23 | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "42");
}
```

**Step 6: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass (24 + 2 = 26).

**Step 7: Commit**

```bash
git add compiler/crates/torqc-codegen/src/codegen.rs \
        compiler/crates/torqc-codegen/tests/codegen_tests.rs
git commit -m "feat(codegen): user-defined blocks as Cranelift functions

Two-pass: declare all blocks as functions, then define them.
Block parameters map to function parameters. Block calls compile
to Cranelift call instructions. Return value from last expression."
```

---

### Task 6: Match Expression

**Goal:** Compile match expressions with literal int patterns and wildcard. Match is a value-producing expression.

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Write failing tests**

```rust
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
```

**Step 2: Run to verify they fail**

Run: `cargo test -p torqc-codegen --test codegen_tests match`
Expected: FAIL

**Step 3: Implement match codegen**

Match compilation strategy using Cranelift basic blocks:

```
[entry: compute match subject]
  │
  ├─ icmp subject == pattern_0 ──► [arm_0_block: compute body_0, jump merge(result)]
  │
  ├─ icmp subject == pattern_1 ──► [arm_1_block: compute body_1, jump merge(result)]
  │
  └─ (wildcard/fallthrough) ────► [wildcard_block: compute body_w, jump merge(result)]
                                        │
                                  [merge_block(result_param): continue]
```

Implementation:
```rust
Expr::Match(m) => {
    // Subject comes from pipe_value
    let (subject, _) = pipe_value
        .ok_or_else(|| CodegenError::new("match requires pipe input"))?;

    // Create merge block with a block parameter for the result
    let merge_block = self.builder.create_block();
    self.builder.append_block_param(merge_block, types::I64);

    for arm in &m.arms {
        match &arm.pattern {
            Pattern::Literal(Literal::Int(n, _)) => {
                let arm_block = self.builder.create_block();
                let next_block = self.builder.create_block();

                let pat_val = self.builder.ins().iconst(types::I64, *n);
                let cmp = self.builder.ins().icmp(IntCC::Equal, subject, pat_val);
                self.builder.ins().brif(cmp, arm_block, &[], next_block, &[]);

                // Arm body
                self.builder.switch_to_block(arm_block);
                self.builder.seal_block(arm_block);
                let (result, _) = self.compile_expr(&arm.body, None)?;
                self.builder.ins().jump(merge_block, &[result]);

                // Continue to next pattern test
                self.builder.switch_to_block(next_block);
                self.builder.seal_block(next_block);
            }
            Pattern::Wildcard => {
                // Wildcard — unconditional
                let (result, _) = self.compile_expr(&arm.body, pipe_value)?;
                self.builder.ins().jump(merge_block, &[result]);
            }
            _ => {
                // Unsupported pattern for Phase 4
                return Err(CodegenError::new("unsupported match pattern"));
            }
        }
    }

    // Switch to merge block
    self.builder.switch_to_block(merge_block);
    self.builder.seal_block(merge_block);
    let result = self.builder.block_params(merge_block)[0];
    Ok((result, TorqType::I64))
}
```

Important: The wildcard arm in fibonacci's match body references `pipe_value` (for `$n`). But actually, looking at the AST, the wildcard body is `BinOp(Add, BlockCall("fibonacci",[Group(Sub($n,1))]), BlockCall("fibonacci",[Group(Sub($n,2))]))`. The `$n` here is a Variable reference, not a pipe reference. So the wildcard arm body just needs access to variables — which it already has through the FuncCompiler's variable map. The pipe_value is only needed for the match subject comparison, not for arm bodies (except if the arm body itself references the pipe value, but in practice it uses named variables).

**Step 4: Run to verify they pass**

Run: `cargo test -p torqc-codegen --test codegen_tests match`
Expected: Both PASS

**Step 5: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass (26 + 2 = 28).

**Step 6: Commit**

```bash
git add compiler/crates/torqc-codegen/src/codegen.rs \
        compiler/crates/torqc-codegen/tests/codegen_tests.rs
git commit -m "feat(codegen): match expression with literal and wildcard patterns

Compare-and-branch basic blocks for literal int patterns. Wildcard
falls through. Merge block collects result via block parameter."
```

---

### Task 7: Loop and Break

**Goal:** Compile `loop` with optional condition and `break` statement.

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Write failing test**

```rust
#[test]
fn loop_with_break() {
    // Count down from 3: print 3, 2, 1, then break
    let src = "\
::main
  3 -> $i
  loop
    $i | print
    ($i - 1) -> $i
    ($i == 0) | match
      1 -> break
      _ -> 0
";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "3\n2\n1");
}
```

Note: This uses match to conditionally break. An alternative test with a simpler pattern:

```rust
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
    assert_eq!(output.trim(), "15"); // 1+2+3+4+5
}
```

**Step 2: Run to verify it fails**

Run: `cargo test -p torqc-codegen --test codegen_tests loop`
Expected: FAIL

**Step 3: Implement loop codegen**

Loop structure:
```
[current_block]
  jump → loop_header

[loop_header]    ← back-edge target
  ... body ...
  (break) → jump exit_block
  jump → loop_header   (implicit back-edge at end of body)

[exit_block]
  ... continue ...
```

Implementation:
```rust
Statement::Loop(loop_stmt) => {
    let loop_header = self.builder.create_block();
    let exit_block = self.builder.create_block();

    // Push loop context for break resolution
    self.loop_exit_stack.push(exit_block);

    // Jump to loop header
    self.builder.ins().jump(loop_header, &[]);
    self.builder.switch_to_block(loop_header);
    // Don't seal loop_header yet — it has a back-edge

    // Compile body
    for stmt in &loop_stmt.body {
        self.compile_statement(stmt)?;
    }

    // Back-edge: jump back to header (if not already terminated by break)
    if !self.builder.is_filled() {
        self.builder.ins().jump(loop_header, &[]);
    }

    // Now seal the loop header (all predecessors known)
    self.builder.seal_block(loop_header);

    // Switch to exit block
    self.builder.switch_to_block(exit_block);
    self.builder.seal_block(exit_block);

    self.loop_exit_stack.pop();
    Ok(None)
}
```

Break:
```rust
Expr::Break(_) => {
    let exit_block = self.loop_exit_stack.last()
        .ok_or_else(|| CodegenError::new("break outside of loop"))?;
    self.builder.ins().jump(*exit_block, &[]);
    // Create a new unreachable block so subsequent code in the match arm has somewhere to go
    let dead_block = self.builder.create_block();
    self.builder.switch_to_block(dead_block);
    self.builder.seal_block(dead_block);
    Ok((self.builder.ins().iconst(types::I64, 0), TorqType::Void))
}
```

Note: After a `break`, we need a dead block because Cranelift requires every instruction to be in a block. The dead block is unreachable but satisfies the builder.

Add `loop_exit_stack: Vec<Block>` to `FuncCompiler`.

**Step 4: Run to verify they pass**

Run: `cargo test -p torqc-codegen --test codegen_tests loop`
Expected: PASS

**Step 5: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass (28 + 2 = 30).

**Step 6: Commit**

```bash
git add compiler/crates/torqc-codegen/src/codegen.rs \
        compiler/crates/torqc-codegen/tests/codegen_tests.rs
git commit -m "feat(codegen): loop with break

Loop compiles to basic block with back-edge. Break jumps to exit
block. Loop exit stack tracks nesting for break resolution."
```

---

### Task 8: Each Sequential + Range

**Goal:** Compile `range $start $end | each $var sequential` as a counted for-loop. This is the loop pattern used in fibonacci.torq's main block.

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Write failing test**

```rust
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
```

Note: `range 1 5` produces 1, 2, 3, 4 (exclusive end, like most range functions).

**Step 2: Run to verify it fails**

Run: `cargo test -p torqc-codegen --test codegen_tests each_sequential`
Expected: FAIL

**Step 3: Implement each sequential codegen**

For Phase 4, `each sequential` only supports `Call("range", [start, end])` as iterable. It compiles to a counted for-loop:

```
[current_block]
  $start = compile(range.args[0])
  $end = compile(range.args[1])
  jump → loop_header($start)

[loop_header($i)]
  cmp $i >= $end → exit_block
  bind $var = $i
  ... body ...
  $next = $i + 1
  jump → loop_header($next)

[exit_block]
  ... continue ...
```

Implementation:
```rust
Statement::Each(each) => {
    if !each.sequential {
        return Err(CodegenError::new("parallel each not supported in Phase 4"));
    }

    // Only support range(...) as iterable
    let (start_val, end_val) = match &*each.iterable {
        Expr::Call(call) if call.name == "range" && call.args.len() == 2 => {
            let (start, _) = self.compile_expr(&call.args[0], None)?;
            let (end, _) = self.compile_expr(&call.args[1], None)?;
            (start, end)
        }
        _ => return Err(CodegenError::new("each sequential only supports range() iterable in Phase 4")),
    };

    let loop_header = self.builder.create_block();
    self.builder.append_block_param(loop_header, types::I64);
    let exit_block = self.builder.create_block();

    self.loop_exit_stack.push(exit_block);

    // Jump to header with start value
    self.builder.ins().jump(loop_header, &[start_val]);
    self.builder.switch_to_block(loop_header);

    let counter = self.builder.block_params(loop_header)[0];

    // Check: counter >= end → exit
    let cmp = self.builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, counter, end_val);
    let body_block = self.builder.create_block();
    self.builder.ins().brif(cmp, exit_block, &[], body_block, &[]);

    self.builder.switch_to_block(body_block);
    self.builder.seal_block(body_block);

    // Bind the iteration variable
    let binding_name = &each.binding.name;
    let idx = self.var_index;
    self.var_index += 1;
    let cl_var = Variable::new(idx);
    self.builder.declare_var(cl_var, types::I64);
    self.builder.def_var(cl_var, counter);
    self.variables.insert(binding_name.clone(), (cl_var, TorqType::I64));

    // Compile body
    for stmt in &each.body {
        self.compile_statement(stmt)?;
    }

    // Increment counter and loop back
    if !self.builder.is_filled() {
        let one = self.builder.ins().iconst(types::I64, 1);
        let next = self.builder.ins().iadd(counter, one);
        self.builder.ins().jump(loop_header, &[next]);
    }

    self.builder.seal_block(loop_header);
    self.builder.switch_to_block(exit_block);
    self.builder.seal_block(exit_block);

    self.loop_exit_stack.pop();
    Ok(None)
}
```

**Step 4: Run to verify it passes**

Run: `cargo test -p torqc-codegen --test codegen_tests each_sequential`
Expected: PASS, output = "1\n2\n3\n4"

**Step 5: Add more tests**

```rust
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
```

**Step 6: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass (30 + 2 = 32).

**Step 7: Commit**

```bash
git add compiler/crates/torqc-codegen/src/codegen.rs \
        compiler/crates/torqc-codegen/tests/codegen_tests.rs
git commit -m "feat(codegen): each sequential with range

range(start, end) + each sequential compiles to a counted for-loop
with block parameter for the counter. Binding variable accessible in body."
```

---

### Task 9: Integration Tests + Fibonacci

**Goal:** Compile and run `fibonacci.torq` end-to-end. Add comprehensive integration tests. Verify all existing tests still pass.

**Files:**
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Write fibonacci integration test**

```rust
#[test]
fn fibonacci_example_file() {
    let fib = examples_dir().join("fibonacci.torq");
    let source = std::fs::read_to_string(&fib)
        .unwrap_or_else(|e| panic!("read {} failed: {}", fib.display(), e));
    let output = compile_and_run(&source);
    let lines: Vec<&str> = output.trim().lines().collect();
    // fibonacci(1)=1, fibonacci(2)=1, fibonacci(3)=2, ..., fibonacci(19)=4181
    let expected: Vec<String> = (1..20).map(|n| fib_value(n).to_string()).collect();
    let expected_refs: Vec<&str> = expected.iter().map(|s| s.as_str()).collect();
    assert_eq!(lines, expected_refs);
}

fn fib_value(n: i64) -> i64 {
    if n <= 1 { return n; }
    let mut a = 0i64;
    let mut b = 1i64;
    for _ in 2..=n {
        let tmp = a + b;
        a = b;
        b = tmp;
    }
    b
}
```

**Step 2: Run to verify it passes**

Run: `cargo test -p torqc-codegen --test codegen_tests fibonacci_example_file`
Expected: PASS — fibonacci(1) through fibonacci(19) printed correctly.

**Step 3: Add inline fibonacci test**

```rust
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
```

**Step 4: Add additional integration tests**

```rust
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
fn match_in_pipeline() {
    let src = "\
::main
  5 | match
    1 -> 100
    5 -> 500
    _ -> 0
  | print
";
    // Note: this tests match as a pipeline stage that passes its result forward.
    // The pipeline is: 5 | match ... | print
    // If the parser produces this as Pipeline([Int(5), Match(...), Call("print")])
    // then match receives pipe value 5, returns 500, which feeds into print.
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "500");
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
```

**Step 5: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All tests pass.

**Step 6: Also run full workspace tests**

Run: `cargo test` (from compiler/)
Expected: All tests across all crates pass.

**Step 7: Commit**

```bash
git add compiler/crates/torqc-codegen/tests/codegen_tests.rs
git commit -m "test(codegen): fibonacci and comprehensive integration tests

fibonacci.torq compiles and runs correctly. Additional tests for
factorial, match-in-pipeline, and block return values."
```

---

### Task 10: Cleanup + CLAUDE.md Update

**Goal:** Run clippy, verify all tests, update documentation.

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Run clippy**

Run: `cargo clippy -p torqc-codegen -- -D warnings`
Fix any warnings.

**Step 2: Run full test suite**

Run: `cargo test` (from compiler/)
Count total tests. Expected: ~220+ tests.

**Step 3: Verify `torqc build` works on fibonacci**

Run: `cargo run -p torqc-cli -- build examples/fibonacci.torq -o /tmp/fibonacci && /tmp/fibonacci`
Expected: Prints fibonacci(1) through fibonacci(19), one per line.

**Step 4: Update CLAUDE.md**

Update the project overview to mention Phase 4 capabilities:
- Variables, arithmetic, user-defined blocks, match, loop, each sequential, range
- fibonacci.torq compiles to native binary
- Update test count

**Step 5: Commit**

```bash
git add CLAUDE.md compiler/
git commit -m "chore: phase 4 cleanup — clippy, docs, test count update"
```
