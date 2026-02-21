# Phase 2: Semantic Analysis Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the `torqc-semantic` crate that validates parsed TORQ ASTs for logical correctness, and wire it into `torqc check <path>`.

**Architecture:** A new `torqc-semantic` crate runs multiple validation passes over the AST. First pass collects a block registry (all block definitions). Subsequent passes walk the AST checking block resolution, variable scoping, parameter arity, control flow, and shared state safety. All diagnostics are collected (not fail-fast) so the developer sees every issue at once. The AI fix loop (Phase 7) will later wrap this analysis.

**Tech Stack:** Rust, `torqc-ast` crate (AST types), `torqc-lexer` + `torqc-parser` (for re-parsing in CLI), `clap` (CLI)

**Depends on:** Phase 1 complete (lexer, parser, AST, CLI with `torqc parse`)

---

### Task 1: Create torqc-semantic crate and diagnostic types

**Files:**
- Create: `compiler/crates/torqc-semantic/Cargo.toml`
- Create: `compiler/crates/torqc-semantic/src/lib.rs`
- Create: `compiler/crates/torqc-semantic/src/diagnostic.rs`
- Modify: `compiler/Cargo.toml` (add workspace member)

**Step 1: Write the failing test**

Create `compiler/crates/torqc-semantic/src/diagnostic.rs` with types and a test:

```rust
use torqc_ast::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(
            f,
            "{}:{}:{}: {}: {}",
            self.span.file, self.span.line, self.span.col, prefix, self.message
        )?;
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {}", hint)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_span() -> Span {
        Span { file: "test.torq".into(), line: 1, col: 1 }
    }

    #[test]
    fn error_diagnostic() {
        let d = Diagnostic::error("undefined block ::foo", test_span());
        assert!(d.is_error());
        assert!(d.to_string().contains("error"));
        assert!(d.to_string().contains("undefined block ::foo"));
    }

    #[test]
    fn warning_diagnostic() {
        let d = Diagnostic::warning("unused variable $x", test_span());
        assert!(!d.is_error());
        assert!(d.to_string().contains("warning"));
    }

    #[test]
    fn diagnostic_with_hint() {
        let d = Diagnostic::error("undefined block ::foo", test_span())
            .with_hint("did you mean ::foobar?");
        assert!(d.to_string().contains("hint: did you mean ::foobar?"));
    }
}
```

**Step 2: Create Cargo.toml and lib.rs**

`compiler/crates/torqc-semantic/Cargo.toml`:
```toml
[package]
name = "torqc-semantic"
version = "0.1.0"
edition = "2021"

[dependencies]
torqc-ast = { path = "../torqc-ast" }

[dev-dependencies]
torqc-lexer = { path = "../torqc-lexer" }
torqc-parser = { path = "../torqc-parser" }
pretty_assertions = "1"
```

`compiler/crates/torqc-semantic/src/lib.rs`:
```rust
pub mod diagnostic;
```

Add `"crates/torqc-semantic"` to the workspace members in `compiler/Cargo.toml`.

**Step 3: Run tests to verify they pass**

Run: `cargo test -p torqc-semantic`
Expected: 3 tests pass

**Step 4: Commit**

```bash
git add compiler/crates/torqc-semantic/ compiler/Cargo.toml
git commit -m "feat(semantic): create torqc-semantic crate with diagnostic types"
```

---

### Task 2: Block registry — collect block definitions, detect duplicates

**Files:**
- Create: `compiler/crates/torqc-semantic/src/registry.rs`
- Modify: `compiler/crates/torqc-semantic/src/lib.rs`

The block registry scans a `Program` and builds a map of block name -> block metadata. Duplicate names produce an error diagnostic.

**Step 1: Write the failing test**

```rust
use std::collections::HashMap;
use torqc_ast::ast::*;
use crate::diagnostic::Diagnostic;

#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub name: String,
    pub param_count: usize,
    pub span: Span,
}

#[derive(Debug)]
pub struct BlockRegistry {
    blocks: HashMap<String, BlockInfo>,
}

impl BlockRegistry {
    pub fn build(program: &Program) -> (Self, Vec<Diagnostic>) {
        let mut blocks = HashMap::new();
        let mut diagnostics = Vec::new();

        for block in &program.blocks {
            if let Some(existing) = blocks.get(&block.name) {
                let existing: &BlockInfo = existing;
                diagnostics.push(
                    Diagnostic::error(
                        format!("duplicate block ::{}",  block.name),
                        block.span.clone(),
                    )
                    .with_hint(format!(
                        "first defined at {}:{}",
                        existing.span.file, existing.span.line
                    )),
                );
            } else {
                blocks.insert(
                    block.name.clone(),
                    BlockInfo {
                        name: block.name.clone(),
                        param_count: block.params.len(),
                        span: block.span.clone(),
                    },
                );
            }
        }

        (Self { blocks }, diagnostics)
    }

    pub fn get(&self, name: &str) -> Option<&BlockInfo> {
        self.blocks.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.blocks.contains_key(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.blocks.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span { file: "test.torq".into(), line: 1, col: 1 }
    }

    fn make_block(name: &str, param_count: usize, line: usize) -> Block {
        Block {
            name: name.to_string(),
            params: (0..param_count)
                .map(|i| Param {
                    sigil: Sigil::Scalar,
                    name: format!("p{}", i),
                    span: Span { file: "test.torq".into(), line, col: 1 },
                })
                .collect(),
            body: vec![],
            doc_comments: vec![],
            span: Span { file: "test.torq".into(), line, col: 1 },
        }
    }

    #[test]
    fn registers_blocks() {
        let program = Program {
            blocks: vec![
                make_block("main", 0, 1),
                make_block("helper", 2, 5),
            ],
        };
        let (reg, diags) = BlockRegistry::build(&program);
        assert!(diags.is_empty());
        assert!(reg.contains("main"));
        assert!(reg.contains("helper"));
        assert!(!reg.contains("unknown"));
        assert_eq!(reg.get("helper").unwrap().param_count, 2);
    }

    #[test]
    fn detects_duplicate_blocks() {
        let program = Program {
            blocks: vec![
                make_block("main", 0, 1),
                make_block("main", 0, 10),
            ],
        };
        let (_, diags) = BlockRegistry::build(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].is_error());
        assert!(diags[0].message.contains("duplicate block ::main"));
    }

    #[test]
    fn empty_program() {
        let program = Program { blocks: vec![] };
        let (reg, diags) = BlockRegistry::build(&program);
        assert!(diags.is_empty());
        assert!(reg.names().is_empty());
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test -p torqc-semantic`
Expected: 6 tests pass (3 diagnostic + 3 registry)

**Step 3: Commit**

```bash
git add compiler/crates/torqc-semantic/
git commit -m "feat(semantic): add block registry with duplicate detection"
```

---

### Task 3: Block resolution — verify all ::calls and &refs resolve

**Files:**
- Create: `compiler/crates/torqc-semantic/src/checks/mod.rs`
- Create: `compiler/crates/torqc-semantic/src/checks/resolve_blocks.rs`
- Modify: `compiler/crates/torqc-semantic/src/lib.rs`

This check walks every expression in the AST. For every `Expr::BlockCall` and `Expr::BlockRef`, it verifies the name exists in the registry. If not, it produces an error with a "did you mean?" hint using edit distance.

**Step 1: Write the failing test**

`compiler/crates/torqc-semantic/src/checks/resolve_blocks.rs`:

```rust
use torqc_ast::ast::*;
use crate::diagnostic::Diagnostic;
use crate::registry::BlockRegistry;

pub fn check(program: &Program, registry: &BlockRegistry) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for block in &program.blocks {
        check_statements(&block.body, registry, &mut diagnostics);
    }
    diagnostics
}

fn check_statements(stmts: &[Statement], reg: &BlockRegistry, diags: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Statement::Pipeline(p) => check_pipeline(p, reg, diags),
            Statement::Assignment(a) => check_expr(&a.value, reg, diags),
            Statement::Each(e) => {
                check_expr(&e.iterable, reg, diags);
                check_statements(&e.body, reg, diags);
            }
            Statement::Loop(l) => {
                if let Some(cond) = &l.condition {
                    check_expr(cond, reg, diags);
                }
                check_statements(&l.body, reg, diags);
            }
            Statement::Expression(e) => check_expr(e, reg, diags),
        }
    }
}

fn check_pipeline(pipeline: &Pipeline, reg: &BlockRegistry, diags: &mut Vec<Diagnostic>) {
    for stage in &pipeline.stages {
        check_expr(stage, reg, diags);
    }
}

fn check_expr(expr: &Expr, reg: &BlockRegistry, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::BlockCall(bc) => {
            if !reg.contains(&bc.name) {
                let mut d = Diagnostic::error(
                    format!("undefined block ::{}", bc.name),
                    bc.span.clone(),
                );
                if let Some(suggestion) = find_similar(&bc.name, reg) {
                    d = d.with_hint(format!("did you mean ::{}?", suggestion));
                }
                diags.push(d);
            }
            for arg in &bc.args {
                check_expr(arg, reg, diags);
            }
        }
        Expr::BlockRef(name, span) => {
            if !reg.contains(name) {
                let mut d = Diagnostic::error(
                    format!("undefined block reference &{}", name),
                    span.clone(),
                );
                if let Some(suggestion) = find_similar(name, reg) {
                    d = d.with_hint(format!("did you mean &{}?", suggestion));
                }
                diags.push(d);
            }
        }
        Expr::Pipeline(p) => check_pipeline(p, reg, diags),
        Expr::Call(c) => {
            for arg in &c.args {
                check_expr(arg, reg, diags);
            }
        }
        Expr::BinOp(b) => {
            check_expr(&b.left, reg, diags);
            check_expr(&b.right, reg, diags);
        }
        Expr::Match(m) => {
            for arm in &m.arms {
                check_expr(&arm.body, reg, diags);
            }
        }
        Expr::MemberAccess(ma) => check_expr(&ma.object, reg, diags),
        Expr::Ternary(t) => {
            check_expr(&t.condition, reg, diags);
            check_expr(&t.then_expr, reg, diags);
            check_expr(&t.else_expr, reg, diags);
        }
        Expr::Array(elems, _) => {
            for e in elems {
                check_expr(e, reg, diags);
            }
        }
        Expr::Dict(entries, _) => {
            for entry in entries {
                check_expr(&entry.value, reg, diags);
            }
        }
        Expr::Group(inner, _) => check_expr(inner, reg, diags),
        _ => {}
    }
}

/// Simple edit distance for "did you mean?" suggestions.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() { dp[i][0] = i; }
    for j in 0..=b.len() { dp[0][j] = j; }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[a.len()][b.len()]
}

fn find_similar(name: &str, reg: &BlockRegistry) -> Option<String> {
    let threshold = (name.len() / 2).max(1).min(3);
    reg.names()
        .into_iter()
        .filter_map(|n| {
            let d = edit_distance(name, n);
            if d <= threshold && d > 0 { Some((d, n.to_string())) } else { None }
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, n)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span { file: "test.torq".into(), line: 1, col: 1 }
    }

    fn make_program_with_call(defined: &[&str], called: &str) -> Program {
        let mut blocks: Vec<Block> = defined.iter().map(|name| Block {
            name: name.to_string(),
            params: vec![],
            body: vec![],
            doc_comments: vec![],
            span: span(),
        }).collect();

        // Add a block that calls `called`
        blocks.push(Block {
            name: "caller".to_string(),
            params: vec![],
            body: vec![Statement::Expression(Expr::BlockCall(BlockCall {
                name: called.to_string(),
                args: vec![],
                span: span(),
            }))],
            doc_comments: vec![],
            span: span(),
        });

        Program { blocks }
    }

    #[test]
    fn valid_block_call() {
        let program = make_program_with_call(&["helper"], "helper");
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        assert!(diags.is_empty());
    }

    #[test]
    fn undefined_block_call() {
        let program = make_program_with_call(&["helper"], "unknown");
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined block ::unknown"));
    }

    #[test]
    fn did_you_mean_suggestion() {
        let program = make_program_with_call(&["validate_cart"], "validate_car");
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].hint.as_ref().unwrap().contains("validate_cart"));
    }

    #[test]
    fn undefined_block_ref() {
        let mut blocks = vec![Block {
            name: "main".to_string(),
            params: vec![],
            body: vec![Statement::Expression(Expr::BlockRef("nonexistent".to_string(), span()))],
            doc_comments: vec![],
            span: span(),
        }];
        let program = Program { blocks };
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined block reference &nonexistent"));
    }

    #[test]
    fn nested_block_call_checked() {
        // Block call inside an each body
        let program = Program {
            blocks: vec![
                Block {
                    name: "main".to_string(),
                    params: vec![],
                    body: vec![Statement::Each(Each {
                        iterable: Box::new(Expr::Variable(Variable {
                            sigil: Sigil::Array,
                            name: "items".to_string(),
                            span: span(),
                        })),
                        binding: Variable {
                            sigil: Sigil::Scalar,
                            name: "item".to_string(),
                            span: span(),
                        },
                        sequential: false,
                        body: vec![Statement::Expression(Expr::BlockCall(BlockCall {
                            name: "missing".to_string(),
                            args: vec![],
                            span: span(),
                        }))],
                        span: span(),
                    })],
                    doc_comments: vec![],
                    span: span(),
                },
            ],
        };
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("::missing"));
    }
}
```

**Step 2: Create `checks/mod.rs`**

```rust
pub mod resolve_blocks;
```

Update `lib.rs`:
```rust
pub mod diagnostic;
pub mod registry;
pub mod checks;
```

**Step 3: Run tests to verify they pass**

Run: `cargo test -p torqc-semantic`
Expected: All tests pass (diagnostic + registry + resolve_blocks)

**Step 4: Commit**

```bash
git add compiler/crates/torqc-semantic/
git commit -m "feat(semantic): add block resolution check with did-you-mean hints"
```

---

### Task 4: Variable scope analysis — detect use before definition

**Files:**
- Create: `compiler/crates/torqc-semantic/src/checks/scope.rs`
- Modify: `compiler/crates/torqc-semantic/src/checks/mod.rs`

Variables must be defined before use. A variable is defined by:
- Block parameters
- Assignment (`$x = expr` or `expr -> $x`)
- `each` binding (`each $item`)
- `loop` is a new scope level

Variables from outer scopes are visible in inner scopes (each, loop).

**Step 1: Write the implementation with tests**

```rust
use std::collections::HashSet;
use torqc_ast::ast::*;
use crate::diagnostic::Diagnostic;

pub fn check(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for block in &program.blocks {
        let mut scope = Scope::new();
        // Block params are defined
        for param in &block.params {
            scope.define(&param.name);
        }
        check_statements(&block.body, &mut scope, &mut diagnostics);
    }
    diagnostics
}

struct Scope {
    /// Stack of sets — each level is a scope (block, each, loop).
    levels: Vec<HashSet<String>>,
}

impl Scope {
    fn new() -> Self {
        Self { levels: vec![HashSet::new()] }
    }

    fn define(&mut self, name: &str) {
        self.levels.last_mut().unwrap().insert(name.to_string());
    }

    fn is_defined(&self, name: &str) -> bool {
        self.levels.iter().rev().any(|level| level.contains(name))
    }

    fn push(&mut self) {
        self.levels.push(HashSet::new());
    }

    fn pop(&mut self) {
        self.levels.pop();
    }
}

fn check_statements(stmts: &[Statement], scope: &mut Scope, diags: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Statement::Assignment(a) => {
                // Check the value first (RHS may reference vars)
                check_expr(&a.value, scope, diags);
                // Then define the target variable
                scope.define(&a.target.name);
            }
            Statement::Pipeline(p) => {
                for stage in &p.stages {
                    check_expr(stage, scope, diags);
                }
            }
            Statement::Each(e) => {
                check_expr(&e.iterable, scope, diags);
                scope.push();
                scope.define(&e.binding.name);
                check_statements(&e.body, scope, diags);
                scope.pop();
            }
            Statement::Loop(l) => {
                if let Some(cond) = &l.condition {
                    check_expr(cond, scope, diags);
                }
                scope.push();
                check_statements(&l.body, scope, diags);
                scope.pop();
            }
            Statement::Expression(e) => check_expr(e, scope, diags),
        }
    }
}

fn check_expr(expr: &Expr, scope: &mut Scope, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Variable(v) => {
            // Shared (*) vars are globally accessible — skip scope check
            if v.sigil != Sigil::Shared && !scope.is_defined(&v.name) {
                diags.push(Diagnostic::error(
                    format!("variable '{}' used before definition", format_var(v)),
                    v.span.clone(),
                ));
            }
        }
        Expr::BlockCall(bc) => {
            for arg in &bc.args {
                check_expr(arg, scope, diags);
            }
        }
        Expr::Call(c) => {
            for arg in &c.args {
                check_expr(arg, scope, diags);
            }
        }
        Expr::Pipeline(p) => {
            for stage in &p.stages {
                check_expr(stage, scope, diags);
            }
        }
        Expr::BinOp(b) => {
            check_expr(&b.left, scope, diags);
            check_expr(&b.right, scope, diags);
        }
        Expr::Match(m) => {
            for arm in &m.arms {
                // Patterns can capture variables
                scope.push();
                define_pattern_captures(&arm.pattern, scope);
                check_expr(&arm.body, scope, diags);
                scope.pop();
            }
        }
        Expr::MemberAccess(ma) => check_expr(&ma.object, scope, diags),
        Expr::Ternary(t) => {
            check_expr(&t.condition, scope, diags);
            check_expr(&t.then_expr, scope, diags);
            check_expr(&t.else_expr, scope, diags);
        }
        Expr::Array(elems, _) => {
            for e in elems {
                check_expr(e, scope, diags);
            }
        }
        Expr::Dict(entries, _) => {
            for entry in entries {
                check_expr(&entry.value, scope, diags);
            }
        }
        Expr::Group(inner, _) => check_expr(inner, scope, diags),
        Expr::StringInterp(parts, _) => {
            for part in parts {
                if let StringPart::Interpolation(v) = part {
                    if v.sigil != Sigil::Shared && !scope.is_defined(&v.name) {
                        diags.push(Diagnostic::error(
                            format!("variable '{}' used before definition", format_var(v)),
                            v.span.clone(),
                        ));
                    }
                }
            }
        }
        _ => {}
    }
}

fn define_pattern_captures(pattern: &Pattern, scope: &mut Scope) {
    match pattern {
        Pattern::Variable(v) => { scope.define(&v.name); }
        Pattern::WithCaptures(_, captures) => {
            for v in captures {
                scope.define(&v.name);
            }
        }
        Pattern::And(patterns) => {
            for p in patterns {
                define_pattern_captures(p, scope);
            }
        }
        _ => {}
    }
}

fn format_var(v: &Variable) -> String {
    let sigil = match v.sigil {
        Sigil::Scalar => "$",
        Sigil::Array => "@",
        Sigil::Dict => "%",
        Sigil::Error => "!",
        Sigil::Regex => "~",
        Sigil::Shared => "*",
        Sigil::BlockRef => "&",
    };
    format!("{}{}", sigil, v.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span { file: "test.torq".into(), line: 1, col: 1 }
    }

    fn scalar_var(name: &str) -> Variable {
        Variable { sigil: Sigil::Scalar, name: name.to_string(), span: span() }
    }

    #[test]
    fn param_defines_variable() {
        let program = Program {
            blocks: vec![Block {
                name: "greet".to_string(),
                params: vec![Param { sigil: Sigil::Scalar, name: "name".to_string(), span: span() }],
                body: vec![Statement::Expression(Expr::Variable(scalar_var("name")))],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert!(diags.is_empty());
    }

    #[test]
    fn undefined_variable() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Expression(Expr::Variable(scalar_var("x")))],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$x"));
        assert!(diags[0].message.contains("used before definition"));
    }

    #[test]
    fn assignment_defines_variable() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![
                    Statement::Assignment(Assignment {
                        target: scalar_var("x"),
                        value: Box::new(Expr::Literal(Literal::Int(42, span()))),
                        span: span(),
                    }),
                    Statement::Expression(Expr::Variable(scalar_var("x"))),
                ],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert!(diags.is_empty());
    }

    #[test]
    fn each_binding_defines_variable() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![Param { sigil: Sigil::Array, name: "items".to_string(), span: span() }],
                body: vec![Statement::Each(Each {
                    iterable: Box::new(Expr::Variable(Variable {
                        sigil: Sigil::Array, name: "items".to_string(), span: span(),
                    })),
                    binding: scalar_var("item"),
                    sequential: false,
                    body: vec![Statement::Expression(Expr::Variable(scalar_var("item")))],
                    span: span(),
                })],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert!(diags.is_empty());
    }

    #[test]
    fn each_binding_not_visible_outside() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![Param { sigil: Sigil::Array, name: "items".to_string(), span: span() }],
                body: vec![
                    Statement::Each(Each {
                        iterable: Box::new(Expr::Variable(Variable {
                            sigil: Sigil::Array, name: "items".to_string(), span: span(),
                        })),
                        binding: scalar_var("item"),
                        sequential: false,
                        body: vec![],
                        span: span(),
                    }),
                    // $item used after each — should error
                    Statement::Expression(Expr::Variable(scalar_var("item"))),
                ],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$item"));
    }

    #[test]
    fn shared_vars_always_accessible() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Expression(Expr::Variable(Variable {
                    sigil: Sigil::Shared,
                    name: "counter".to_string(),
                    span: span(),
                }))],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert!(diags.is_empty());
    }
}
```

**Step 2: Update `checks/mod.rs`**

```rust
pub mod resolve_blocks;
pub mod scope;
```

**Step 3: Run tests**

Run: `cargo test -p torqc-semantic`
Expected: All tests pass

**Step 4: Commit**

```bash
git add compiler/crates/torqc-semantic/
git commit -m "feat(semantic): add variable scope analysis"
```

---

### Task 5: Parameter arity check

**Files:**
- Create: `compiler/crates/torqc-semantic/src/checks/arity.rs`
- Modify: `compiler/crates/torqc-semantic/src/checks/mod.rs`

Checks that block calls pass the correct number of arguments matching the block's parameter list. Only checks user-defined blocks (not builtins).

**Step 1: Write the implementation with tests**

```rust
use torqc_ast::ast::*;
use crate::diagnostic::Diagnostic;
use crate::registry::BlockRegistry;

pub fn check(program: &Program, registry: &BlockRegistry) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for block in &program.blocks {
        check_statements(&block.body, registry, &mut diagnostics);
    }
    diagnostics
}

fn check_statements(stmts: &[Statement], reg: &BlockRegistry, diags: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Statement::Pipeline(p) => {
                for stage in &p.stages {
                    check_expr(stage, reg, diags);
                }
            }
            Statement::Assignment(a) => check_expr(&a.value, reg, diags),
            Statement::Each(e) => {
                check_expr(&e.iterable, reg, diags);
                check_statements(&e.body, reg, diags);
            }
            Statement::Loop(l) => {
                if let Some(cond) = &l.condition {
                    check_expr(cond, reg, diags);
                }
                check_statements(&l.body, reg, diags);
            }
            Statement::Expression(e) => check_expr(e, reg, diags),
        }
    }
}

fn check_expr(expr: &Expr, reg: &BlockRegistry, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::BlockCall(bc) => {
            if let Some(info) = reg.get(&bc.name) {
                if bc.args.len() != info.param_count {
                    diags.push(Diagnostic::error(
                        format!(
                            "block ::{} expects {} argument(s), but {} provided",
                            bc.name, info.param_count, bc.args.len()
                        ),
                        bc.span.clone(),
                    ));
                }
            }
            for arg in &bc.args {
                check_expr(arg, reg, diags);
            }
        }
        Expr::Pipeline(p) => {
            for stage in &p.stages {
                check_expr(stage, reg, diags);
            }
        }
        Expr::BinOp(b) => {
            check_expr(&b.left, reg, diags);
            check_expr(&b.right, reg, diags);
        }
        Expr::Match(m) => {
            for arm in &m.arms {
                check_expr(&arm.body, reg, diags);
            }
        }
        Expr::Call(c) => {
            for arg in &c.args {
                check_expr(arg, reg, diags);
            }
        }
        Expr::MemberAccess(ma) => check_expr(&ma.object, reg, diags),
        Expr::Ternary(t) => {
            check_expr(&t.condition, reg, diags);
            check_expr(&t.then_expr, reg, diags);
            check_expr(&t.else_expr, reg, diags);
        }
        Expr::Array(elems, _) => {
            for e in elems { check_expr(e, reg, diags); }
        }
        Expr::Dict(entries, _) => {
            for entry in entries { check_expr(&entry.value, reg, diags); }
        }
        Expr::Group(inner, _) => check_expr(inner, reg, diags),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::BlockRegistry;

    fn span() -> Span {
        Span { file: "test.torq".into(), line: 1, col: 1 }
    }

    fn make_block(name: &str, param_count: usize) -> Block {
        Block {
            name: name.to_string(),
            params: (0..param_count)
                .map(|i| Param {
                    sigil: Sigil::Scalar,
                    name: format!("p{}", i),
                    span: span(),
                })
                .collect(),
            body: vec![],
            doc_comments: vec![],
            span: span(),
        }
    }

    #[test]
    fn correct_arity() {
        let program = Program {
            blocks: vec![
                make_block("helper", 2),
                Block {
                    name: "main".to_string(),
                    params: vec![],
                    body: vec![Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "helper".to_string(),
                        args: vec![
                            Expr::Literal(Literal::Int(1, span())),
                            Expr::Literal(Literal::Int(2, span())),
                        ],
                        span: span(),
                    }))],
                    doc_comments: vec![],
                    span: span(),
                },
            ],
        };
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        assert!(diags.is_empty());
    }

    #[test]
    fn too_few_args() {
        let program = Program {
            blocks: vec![
                make_block("helper", 2),
                Block {
                    name: "main".to_string(),
                    params: vec![],
                    body: vec![Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "helper".to_string(),
                        args: vec![Expr::Literal(Literal::Int(1, span()))],
                        span: span(),
                    }))],
                    doc_comments: vec![],
                    span: span(),
                },
            ],
        };
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("expects 2"));
        assert!(diags[0].message.contains("1 provided"));
    }

    #[test]
    fn too_many_args() {
        let program = Program {
            blocks: vec![
                make_block("helper", 0),
                Block {
                    name: "main".to_string(),
                    params: vec![],
                    body: vec![Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "helper".to_string(),
                        args: vec![Expr::Literal(Literal::Int(1, span()))],
                        span: span(),
                    }))],
                    doc_comments: vec![],
                    span: span(),
                },
            ],
        };
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("expects 0"));
    }

    #[test]
    fn undefined_block_skipped() {
        // Arity check should not double-report for undefined blocks
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Expression(Expr::BlockCall(BlockCall {
                    name: "nonexistent".to_string(),
                    args: vec![],
                    span: span(),
                }))],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let (reg, _) = BlockRegistry::build(&program);
        let diags = check(&program, &reg);
        // No arity error for undefined block (resolve_blocks handles that)
        assert!(diags.is_empty());
    }
}
```

**Step 2: Update `checks/mod.rs`**

```rust
pub mod resolve_blocks;
pub mod scope;
pub mod arity;
```

**Step 3: Run tests**

Run: `cargo test -p torqc-semantic`
Expected: All tests pass

**Step 4: Commit**

```bash
git add compiler/crates/torqc-semantic/
git commit -m "feat(semantic): add parameter arity checking"
```

---

### Task 6: Break validation and shared state safety

**Files:**
- Create: `compiler/crates/torqc-semantic/src/checks/control_flow.rs`
- Modify: `compiler/crates/torqc-semantic/src/checks/mod.rs`

Two checks in one module:
1. `break` can only appear inside a `loop` body (not in `each`, not at block level)
2. `*` shared variables modified inside a parallel `each` (non-sequential) produce a warning

**Step 1: Write the implementation with tests**

```rust
use torqc_ast::ast::*;
use crate::diagnostic::Diagnostic;

pub fn check(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for block in &program.blocks {
        check_statements(&block.body, false, false, &mut diagnostics);
    }
    diagnostics
}

/// `in_loop` tracks whether we're inside a loop body.
/// `in_parallel_each` tracks whether we're in a non-sequential each.
fn check_statements(
    stmts: &[Statement],
    in_loop: bool,
    in_parallel_each: bool,
    diags: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            Statement::Loop(l) => {
                if let Some(cond) = &l.condition {
                    check_expr(cond, in_loop, in_parallel_each, diags);
                }
                // Inside loop body, in_loop = true
                check_statements(&l.body, true, in_parallel_each, diags);
            }
            Statement::Each(e) => {
                check_expr(&e.iterable, in_loop, in_parallel_each, diags);
                let parallel = !e.sequential;
                check_statements(&e.body, in_loop, parallel, diags);
            }
            Statement::Pipeline(p) => {
                for stage in &p.stages {
                    check_expr(stage, in_loop, in_parallel_each, diags);
                }
            }
            Statement::Assignment(a) => {
                check_expr(&a.value, in_loop, in_parallel_each, diags);
                // Check if assigning to shared var in parallel each
                if in_parallel_each && a.target.sigil == Sigil::Shared {
                    diags.push(Diagnostic::warning(
                        format!("shared variable *{} modified inside parallel each", a.target.name),
                        a.span.clone(),
                    ).with_hint("consider adding 'sequential' to the each statement"));
                }
            }
            Statement::Expression(e) => {
                check_expr(e, in_loop, in_parallel_each, diags);
            }
        }
    }
}

fn check_expr(
    expr: &Expr,
    in_loop: bool,
    in_parallel_each: bool,
    diags: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Break(span) => {
            if !in_loop {
                diags.push(Diagnostic::error(
                    "break used outside of a loop".to_string(),
                    span.clone(),
                ));
            }
        }
        Expr::Pipeline(p) => {
            for stage in &p.stages {
                check_expr(stage, in_loop, in_parallel_each, diags);
            }
        }
        Expr::BinOp(b) => {
            check_expr(&b.left, in_loop, in_parallel_each, diags);
            check_expr(&b.right, in_loop, in_parallel_each, diags);
        }
        Expr::Match(m) => {
            for arm in &m.arms {
                check_expr(&arm.body, in_loop, in_parallel_each, diags);
            }
        }
        Expr::BlockCall(bc) => {
            for arg in &bc.args {
                check_expr(arg, in_loop, in_parallel_each, diags);
            }
        }
        Expr::Call(c) => {
            for arg in &c.args {
                check_expr(arg, in_loop, in_parallel_each, diags);
            }
        }
        Expr::MemberAccess(ma) => check_expr(&ma.object, in_loop, in_parallel_each, diags),
        Expr::Ternary(t) => {
            check_expr(&t.condition, in_loop, in_parallel_each, diags);
            check_expr(&t.then_expr, in_loop, in_parallel_each, diags);
            check_expr(&t.else_expr, in_loop, in_parallel_each, diags);
        }
        Expr::Array(elems, _) => {
            for e in elems { check_expr(e, in_loop, in_parallel_each, diags); }
        }
        Expr::Dict(entries, _) => {
            for entry in entries { check_expr(&entry.value, in_loop, in_parallel_each, diags); }
        }
        Expr::Group(inner, _) => check_expr(inner, in_loop, in_parallel_each, diags),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> Span {
        Span { file: "test.torq".into(), line: 1, col: 1 }
    }

    #[test]
    fn break_inside_loop_ok() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Loop(Loop {
                    condition: None,
                    body: vec![Statement::Expression(Expr::Break(span()))],
                    span: span(),
                })],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert!(diags.is_empty());
    }

    #[test]
    fn break_outside_loop_error() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Expression(Expr::Break(span()))],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].is_error());
        assert!(diags[0].message.contains("break used outside of a loop"));
    }

    #[test]
    fn break_in_each_error() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Each(Each {
                    iterable: Box::new(Expr::Variable(Variable {
                        sigil: Sigil::Array, name: "items".into(), span: span(),
                    })),
                    binding: Variable { sigil: Sigil::Scalar, name: "x".into(), span: span() },
                    sequential: false,
                    body: vec![Statement::Expression(Expr::Break(span()))],
                    span: span(),
                })],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("break used outside of a loop"));
    }

    #[test]
    fn shared_var_in_parallel_each_warns() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Each(Each {
                    iterable: Box::new(Expr::Variable(Variable {
                        sigil: Sigil::Array, name: "items".into(), span: span(),
                    })),
                    binding: Variable { sigil: Sigil::Scalar, name: "x".into(), span: span() },
                    sequential: false,
                    body: vec![Statement::Assignment(Assignment {
                        target: Variable { sigil: Sigil::Shared, name: "counter".into(), span: span() },
                        value: Box::new(Expr::Literal(Literal::Int(1, span()))),
                        span: span(),
                    })],
                    span: span(),
                })],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(!diags[0].is_error()); // warning, not error
        assert!(diags[0].message.contains("*counter"));
        assert!(diags[0].message.contains("parallel each"));
    }

    #[test]
    fn shared_var_in_sequential_each_ok() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Each(Each {
                    iterable: Box::new(Expr::Variable(Variable {
                        sigil: Sigil::Array, name: "items".into(), span: span(),
                    })),
                    binding: Variable { sigil: Sigil::Scalar, name: "x".into(), span: span() },
                    sequential: true,
                    body: vec![Statement::Assignment(Assignment {
                        target: Variable { sigil: Sigil::Shared, name: "counter".into(), span: span() },
                        value: Box::new(Expr::Literal(Literal::Int(1, span()))),
                        span: span(),
                    })],
                    span: span(),
                })],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let diags = check(&program);
        assert!(diags.is_empty());
    }
}
```

**Step 2: Update `checks/mod.rs`**

```rust
pub mod resolve_blocks;
pub mod scope;
pub mod arity;
pub mod control_flow;
```

**Step 3: Run tests**

Run: `cargo test -p torqc-semantic`
Expected: All tests pass

**Step 4: Commit**

```bash
git add compiler/crates/torqc-semantic/
git commit -m "feat(semantic): add break validation and shared state safety checks"
```

---

### Task 7: Analyzer — run all checks and collect diagnostics

**Files:**
- Create: `compiler/crates/torqc-semantic/src/analyzer.rs`
- Modify: `compiler/crates/torqc-semantic/src/lib.rs`

The `Analyzer` is the public API. It takes a `Program`, runs all checks, and returns all diagnostics sorted by location.

**Step 1: Write the implementation with tests**

```rust
use torqc_ast::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::registry::BlockRegistry;
use crate::checks;

#[derive(Debug)]
pub struct AnalysisResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl AnalysisResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| !d.is_error()).count()
    }
}

pub fn analyze(program: &Program) -> AnalysisResult {
    let mut diagnostics = Vec::new();

    // Phase 1: Build block registry (also detects duplicates)
    let (registry, mut reg_diags) = BlockRegistry::build(program);
    diagnostics.append(&mut reg_diags);

    // Phase 2: Block resolution
    diagnostics.append(&mut checks::resolve_blocks::check(program, &registry));

    // Phase 3: Parameter arity
    diagnostics.append(&mut checks::arity::check(program, &registry));

    // Phase 4: Variable scope
    diagnostics.append(&mut checks::scope::check(program));

    // Phase 5: Control flow (break validation, shared state safety)
    diagnostics.append(&mut checks::control_flow::check(program));

    // Sort diagnostics by file, then line, then column
    diagnostics.sort_by(|a, b| {
        a.span.file.cmp(&b.span.file)
            .then(a.span.line.cmp(&b.span.line))
            .then(a.span.col.cmp(&b.span.col))
    });

    AnalysisResult { diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;
    use torqc_ast::ast::*;

    fn span() -> Span {
        Span { file: "test.torq".into(), line: 1, col: 1 }
    }

    #[test]
    fn clean_program() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![],
                body: vec![Statement::Expression(Expr::Literal(
                    Literal::String("hello".into(), span()),
                ))],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let result = analyze(&program);
        assert!(!result.has_errors());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warning_count(), 0);
    }

    #[test]
    fn multiple_errors_collected() {
        let program = Program {
            blocks: vec![
                Block {
                    name: "main".to_string(),
                    params: vec![],
                    body: vec![
                        // Undefined block call
                        Statement::Expression(Expr::BlockCall(BlockCall {
                            name: "missing".into(),
                            args: vec![],
                            span: Span { file: "test.torq".into(), line: 3, col: 3 },
                        })),
                        // Undefined variable
                        Statement::Expression(Expr::Variable(Variable {
                            sigil: Sigil::Scalar,
                            name: "x".into(),
                            span: Span { file: "test.torq".into(), line: 5, col: 3 },
                        })),
                        // Break outside loop
                        Statement::Expression(Expr::Break(
                            Span { file: "test.torq".into(), line: 7, col: 3 },
                        )),
                    ],
                    doc_comments: vec![],
                    span: span(),
                },
            ],
        };
        let result = analyze(&program);
        assert!(result.has_errors());
        assert_eq!(result.error_count(), 3);
        // Verify sorting by line
        assert!(result.diagnostics[0].span.line <= result.diagnostics[1].span.line);
        assert!(result.diagnostics[1].span.line <= result.diagnostics[2].span.line);
    }

    #[test]
    fn errors_and_warnings_together() {
        let program = Program {
            blocks: vec![Block {
                name: "main".to_string(),
                params: vec![Param {
                    sigil: Sigil::Array, name: "items".into(), span: span(),
                }],
                body: vec![
                    // Break outside loop (error)
                    Statement::Expression(Expr::Break(span())),
                    // Shared var in parallel each (warning)
                    Statement::Each(Each {
                        iterable: Box::new(Expr::Variable(Variable {
                            sigil: Sigil::Array, name: "items".into(), span: span(),
                        })),
                        binding: Variable { sigil: Sigil::Scalar, name: "x".into(), span: span() },
                        sequential: false,
                        body: vec![Statement::Assignment(Assignment {
                            target: Variable {
                                sigil: Sigil::Shared, name: "c".into(), span: span(),
                            },
                            value: Box::new(Expr::Literal(Literal::Int(1, span()))),
                            span: span(),
                        })],
                        span: span(),
                    }),
                ],
                doc_comments: vec![],
                span: span(),
            }],
        };
        let result = analyze(&program);
        assert!(result.has_errors());
        assert_eq!(result.error_count(), 1);
        assert_eq!(result.warning_count(), 1);
    }
}
```

**Step 2: Update `lib.rs`**

```rust
pub mod diagnostic;
pub mod registry;
pub mod checks;
pub mod analyzer;
```

**Step 3: Run tests**

Run: `cargo test -p torqc-semantic`
Expected: All tests pass

**Step 4: Commit**

```bash
git add compiler/crates/torqc-semantic/
git commit -m "feat(semantic): add analyzer that runs all checks"
```

---

### Task 8: CLI — wire up `torqc check` command

**Files:**
- Modify: `compiler/crates/torqc-cli/Cargo.toml` (add torqc-semantic dependency)
- Modify: `compiler/crates/torqc-cli/src/main.rs` (add Check subcommand)

**Step 1: Add dependency**

Add to `compiler/crates/torqc-cli/Cargo.toml` under `[dependencies]`:
```toml
torqc-semantic = { path = "../torqc-semantic" }
```

**Step 2: Add Check subcommand**

In `main.rs`, add to the `Commands` enum:
```rust
/// Check a .torq file for semantic errors
Check {
    /// Path to the .torq source file
    file: String,
},
```

Add match arm:
```rust
Commands::Check { file } => {
    if let Err(e) = run_check(&file) {
        eprintln!("error: {}", e);
        process::exit(1);
    }
}
```

Add the `run_check` function:
```rust
fn run_check(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read '{}': {}", path, e))?;

    let tokens = Lexer::tokenize(&source, path)
        .map_err(|e| format!("lex error: {}", e))?;

    let program = parser::parse(tokens, path)
        .map_err(|e| format!("parse error: {}", e))?;

    let result = torqc_semantic::analyzer::analyze(&program);

    for diag in &result.diagnostics {
        eprintln!("{}", diag);
    }

    let errors = result.error_count();
    let warnings = result.warning_count();

    if errors > 0 {
        eprintln!(
            "\n\u{2717} {} error(s), {} warning(s)",
            errors, warnings
        );
        process::exit(1);
    } else if warnings > 0 {
        eprintln!(
            "\n\u{26A0} {} warning(s)",
            warnings
        );
    } else {
        eprintln!("\u{2713} No issues found");
    }

    Ok(())
}
```

**Step 3: Build and verify**

Run: `cargo build -p torqc-cli`
Expected: Compiles cleanly

Run: `cargo run -p torqc-cli -- check examples/hello.torq`
Expected: `✓ No issues found` (or may show scope warnings for implicit variables — we'll handle in Task 9)

**Step 4: Commit**

```bash
git add compiler/crates/torqc-cli/
git commit -m "feat(cli): add torqc check command for semantic analysis"
```

---

### Task 9: Integration tests with examples

**Files:**
- Create: `compiler/crates/torqc-semantic/tests/semantic_tests.rs`

Test that all 6 example files pass semantic analysis without errors. Also create test programs with intentional errors to verify each check works end-to-end through the parser.

**Important context:** The example programs use many implicit variables and builtin function calls. The scope checker will likely flag variables like `$req`, `$errors`, `$count`, `$balance`, etc. that are implicitly available (from pipe input or match captures). We may need to adjust the scope checker to be more lenient about variables used in pipe contexts. This task may require refining the scope checker to avoid false positives on the examples.

**Step 1: Write integration tests**

```rust
use torqc_lexer::lexer::Lexer;
use torqc_parser::parser;
use torqc_semantic::analyzer;

fn check_source(source: &str) -> torqc_semantic::analyzer::AnalysisResult {
    let tokens = Lexer::tokenize(source, "test.torq").expect("lex failed");
    let program = parser::parse(tokens, "test.torq").expect("parse failed");
    analyzer::analyze(&program)
}

fn check_file(path: &str) -> torqc_semantic::analyzer::AnalysisResult {
    let source = std::fs::read_to_string(path).expect("read failed");
    let tokens = Lexer::tokenize(&source, path).expect("lex failed");
    let program = parser::parse(tokens, path).expect("parse failed");
    analyzer::analyze(&program)
}

// ── Example files should have no errors ─────────────────────────

#[test]
fn hello_clean() {
    let result = check_file("../../examples/hello.torq");
    assert!(!result.has_errors(), "hello.torq has errors: {:?}",
        result.diagnostics.iter().filter(|d| d.is_error()).collect::<Vec<_>>());
}

#[test]
fn fibonacci_clean() {
    let result = check_file("../../examples/fibonacci.torq");
    assert!(!result.has_errors(), "fibonacci.torq has errors: {:?}",
        result.diagnostics.iter().filter(|d| d.is_error()).collect::<Vec<_>>());
}

#[test]
fn data_processing_clean() {
    let result = check_file("../../examples/data_processing.torq");
    assert!(!result.has_errors(), "data_processing.torq has errors: {:?}",
        result.diagnostics.iter().filter(|d| d.is_error()).collect::<Vec<_>>());
}

#[test]
fn cli_tool_clean() {
    let result = check_file("../../examples/cli_tool.torq");
    assert!(!result.has_errors(), "cli_tool.torq has errors: {:?}",
        result.diagnostics.iter().filter(|d| d.is_error()).collect::<Vec<_>>());
}

#[test]
fn web_server_clean() {
    let result = check_file("../../examples/web_server.torq");
    assert!(!result.has_errors(), "web_server.torq has errors: {:?}",
        result.diagnostics.iter().filter(|d| d.is_error()).collect::<Vec<_>>());
}

#[test]
fn checkout_service_clean() {
    let result = check_file("../../examples/checkout_service.torq");
    assert!(!result.has_errors(), "checkout_service.torq has errors: {:?}",
        result.diagnostics.iter().filter(|d| d.is_error()).collect::<Vec<_>>());
}

// ── Negative tests — intentional errors ──────────────────────────

#[test]
fn detects_undefined_block() {
    let result = check_source("::main\n  ::nonexistent\n");
    assert!(result.has_errors());
    assert!(result.diagnostics.iter().any(|d| d.message.contains("undefined block")));
}

#[test]
fn detects_duplicate_block() {
    let result = check_source("::main\n  print \"a\"\n\n::main\n  print \"b\"\n");
    assert!(result.has_errors());
    assert!(result.diagnostics.iter().any(|d| d.message.contains("duplicate block")));
}

#[test]
fn detects_break_outside_loop() {
    let result = check_source("::main\n  break\n");
    assert!(result.has_errors());
    assert!(result.diagnostics.iter().any(|d| d.message.contains("break")));
}

#[test]
fn detects_arity_mismatch() {
    let result = check_source("::helper $a $b\n  print $a\n\n::main\n  ::helper 1\n");
    assert!(result.has_errors());
    assert!(result.diagnostics.iter().any(|d| d.message.contains("expects 2")));
}
```

**Step 2: Run tests and fix scope checker if needed**

Run: `cargo test -p torqc-semantic --test semantic_tests`

If example files produce false-positive scope errors (expected), adjust the scope checker to handle:
- Variables from pipe input context (like `$req` in HTTP handlers)
- Match pattern captures that define variables
- Variables from `->` arrow assignment in pipelines

The scope checker may need to treat the first variable usage in a pipe context more leniently, or we may need to mark certain patterns as "implicit definitions." Fix iteratively until all 6 examples pass clean.

**Step 3: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests pass (Phase 1 + Phase 2)

**Step 4: Commit**

```bash
git add compiler/crates/torqc-semantic/
git commit -m "feat(semantic): add integration tests for all examples"
```

---

### Task 10: Final cleanup and CLAUDE.md update

**Files:**
- Modify: `CLAUDE.md` (add `torqc check` command, update architecture notes)
- Run clippy and fix any warnings

**Step 1: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Fix any warnings.

**Step 2: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests pass

**Step 3: Update CLAUDE.md**

Add to the build commands section:
```
cargo run -p torqc-cli -- check <file.torq>  # Semantic analysis
```

Add to the architecture notes that `torqc-semantic` exists and what checks it performs.

**Step 4: Commit**

```bash
git add -A
git commit -m "feat(semantic): phase 2 complete — torqc check with 5 validation passes"
```

---

## Summary

After Phase 2, `torqc check <file.torq>` runs 5 semantic validation passes:

| Check | Type | What it catches |
|---|---|---|
| Block registry | Error | Duplicate block names |
| Block resolution | Error | Undefined `::call` and `&ref` |
| Parameter arity | Error | Wrong number of arguments to blocks |
| Variable scope | Error | Variables used before definition |
| Control flow | Error/Warning | `break` outside loops, `*` vars in parallel `each` |

All 6 examples pass clean. The analyzer collects all diagnostics (doesn't fail fast) and reports them sorted by location.
