# Phase 1: Lexer + Parser Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the TORQ lexer and parser so that `torqc parse file.torq` prints a complete AST for any valid TORQ program.

**Architecture:** Rust workspace with three crates — `torqc-ast` (shared types), `torqc-lexer` (tokenization via logos), `torqc-parser` (recursive descent parser). A `torqc-cli` crate wires them together behind a `torqc parse` command.

**Tech Stack:** Rust, logos (lexer generator), serde + serde_json (AST serialization), clap (CLI arg parsing)

---

### Task 1: Initialize Rust Workspace

**Files:**
- Create: `compiler/Cargo.toml` (workspace root)
- Create: `compiler/crates/torqc-ast/Cargo.toml`
- Create: `compiler/crates/torqc-ast/src/lib.rs`
- Create: `compiler/crates/torqc-lexer/Cargo.toml`
- Create: `compiler/crates/torqc-lexer/src/lib.rs`
- Create: `compiler/crates/torqc-parser/Cargo.toml`
- Create: `compiler/crates/torqc-parser/src/lib.rs`
- Create: `compiler/crates/torqc-cli/Cargo.toml`
- Create: `compiler/crates/torqc-cli/src/main.rs`

**Step 1: Create workspace Cargo.toml**

```toml
# compiler/Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/torqc-ast",
    "crates/torqc-lexer",
    "crates/torqc-parser",
    "crates/torqc-cli",
]
```

**Step 2: Create torqc-ast crate**

```toml
# compiler/crates/torqc-ast/Cargo.toml
[package]
name = "torqc-ast"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
// compiler/crates/torqc-ast/src/lib.rs
pub mod ast;
```

**Step 3: Create torqc-lexer crate**

```toml
# compiler/crates/torqc-lexer/Cargo.toml
[package]
name = "torqc-lexer"
version = "0.1.0"
edition = "2021"

[dependencies]
logos = "0.14"

[dev-dependencies]
pretty_assertions = "1"
```

```rust
// compiler/crates/torqc-lexer/src/lib.rs
pub mod token;
pub mod lexer;
```

**Step 4: Create torqc-parser crate**

```toml
# compiler/crates/torqc-parser/Cargo.toml
[package]
name = "torqc-parser"
version = "0.1.0"
edition = "2021"

[dependencies]
torqc-ast = { path = "../torqc-ast" }
torqc-lexer = { path = "../torqc-lexer" }

[dev-dependencies]
pretty_assertions = "1"
```

```rust
// compiler/crates/torqc-parser/src/lib.rs
pub mod parser;
```

**Step 5: Create torqc-cli crate**

```toml
# compiler/crates/torqc-cli/Cargo.toml
[package]
name = "torqc-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "torqc"
path = "src/main.rs"

[dependencies]
torqc-ast = { path = "../torqc-ast" }
torqc-lexer = { path = "../torqc-lexer" }
torqc-parser = { path = "../torqc-parser" }
clap = { version = "4", features = ["derive"] }
serde_json = "1"
```

```rust
// compiler/crates/torqc-cli/src/main.rs
fn main() {
    println!("torqc - TORQ compiler");
}
```

**Step 6: Verify workspace compiles**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo build`
Expected: Compiles with no errors.

**Step 7: Commit**

```bash
git add compiler/
git commit -m "feat: initialize Rust workspace with ast, lexer, parser, cli crates"
```

---

### Task 2: Define AST Types

All AST node types that the parser will produce. These match the grammar in the spec (Appendix C).

**Files:**
- Create: `compiler/crates/torqc-ast/src/ast.rs`

**Step 1: Write the AST definitions**

```rust
// compiler/crates/torqc-ast/src/ast.rs
use serde::Serialize;

/// Source location for error reporting
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

/// A complete TORQ program — one or more blocks
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Program {
    pub blocks: Vec<Block>,
}

/// A block definition: ::name param1 param2 \n body
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Block {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Statement>,
    pub doc_comments: Vec<String>,
    pub span: Span,
}

/// Block parameter — a sigil variable like $name, @items, %config
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Param {
    pub sigil: Sigil,
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Sigil {
    Scalar,     // $
    Array,      // @
    Dict,       // %
    Error,      // !
    Regex,      // ~
    Shared,     // *
    BlockRef,   // &
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Statement {
    Pipeline(Pipeline),
    Assignment(Assignment),
    Each(Each),
    Loop(Loop),
    Expression(Expr),
}

/// A chain of expressions connected by |
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pipeline {
    pub stages: Vec<Expr>,
    pub span: Span,
}

/// sigil_var = expr  OR  expr -> sigil_var
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Assignment {
    pub target: Variable,
    pub value: Box<Expr>,
    pub span: Span,
}

/// expr | each $var [sequential] \n body
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Each {
    pub iterable: Box<Expr>,
    pub binding: Variable,
    pub sequential: bool,
    pub body: Vec<Statement>,
    pub span: Span,
}

/// loop [condition] \n body
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Loop {
    pub condition: Option<Box<Expr>>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Expr {
    /// Literal values: 42, 3.14, "hello", true, false, null
    Literal(Literal),
    /// Sigil variable: $name, @items, %config, !err, *counter
    Variable(Variable),
    /// Array literal: [1 2 3]
    Array(Vec<Expr>, Span),
    /// Dict literal: { key "value" }
    Dict(Vec<DictEntry>, Span),
    /// Block call: ::name arg1 arg2
    BlockCall(BlockCall),
    /// Block reference: &::name
    BlockRef(String, Span),
    /// Builtin/function call: print, filter, sort, http.get, etc.
    Call(Call),
    /// Match expression
    Match(Match),
    /// Binary operation: $a + $b, $a > $b, etc.
    BinOp(BinOp),
    /// Member access: %user.name, $req.body
    MemberAccess(MemberAccess),
    /// String interpolation: "hello $name"
    StringInterp(Vec<StringPart>, Span),
    /// Ternary: $cond ? $a : $b
    Ternary(Ternary),
    /// Pipe chain (used within expressions)
    Pipeline(Pipeline),
    /// Break statement (inside loops)
    Break(Span),
    /// Grouped expression: ($n - 1)
    Group(Box<Expr>, Span),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Variable {
    pub sigil: Sigil,
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Literal {
    Int(i64, Span),
    Float(f64, Span),
    String(String, Span),
    Bool(bool, Span),
    Null(Span),
    Duration(Duration, Span),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Duration {
    pub value: u64,
    pub unit: DurationUnit,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum DurationUnit {
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DictEntry {
    pub key: String,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BlockCall {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Call {
    pub name: String, // dotted: "http.get", "sys.fs.read", "print"
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Match {
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Pattern {
    /// Literal: 0, 1, "quit", true
    Literal(Literal),
    /// Comparison: >= 21, < 10, != "admin"
    Comparison(ComparisonOp, Box<Expr>),
    /// Type pattern: %user, !err, $value
    Variable(Variable),
    /// Field condition: .method = "GET"
    FieldMatch(FieldMatch),
    /// Multi-condition: .method = "GET" & .auth = true
    And(Vec<Pattern>),
    /// Wildcard: _
    Wildcard,
    /// Destructuring args: "build" $target
    WithCaptures(Box<Pattern>, Vec<Variable>),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FieldMatch {
    pub field: String,
    pub op: ComparisonOp,
    pub value: Box<Expr>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ComparisonOp {
    Eq,         // =
    NotEq,      // !=
    Gt,         // >
    Lt,         // <
    GtEq,       // >=
    LtEq,       // <=
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum BinOpKind {
    Add,        // +
    Sub,        // -
    Mul,        // *
    Div,        // /
    Mod,        // %
    Pow,        // **
    Gt,         // >
    Lt,         // <
    GtEq,       // >=
    LtEq,       // <=
    Eq,         // =
    NotEq,      // !=
    And,        // &
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BinOp {
    pub left: Box<Expr>,
    pub op: BinOpKind,
    pub right: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MemberAccess {
    pub object: Box<Expr>,
    pub field: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum StringPart {
    Literal(String),
    Interpolation(Variable),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Ternary {
    pub condition: Box<Expr>,
    pub then_expr: Box<Expr>,
    pub else_expr: Box<Expr>,
    pub span: Span,
}
```

**Step 2: Verify it compiles**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo build`
Expected: Compiles with no errors.

**Step 3: Commit**

```bash
git add compiler/crates/torqc-ast/
git commit -m "feat: define complete AST types for TORQ grammar"
```

---

### Task 3: Implement Token Types

Define every token the lexer can produce.

**Files:**
- Create: `compiler/crates/torqc-lexer/src/token.rs`

**Step 1: Write the token definitions**

```rust
// compiler/crates/torqc-lexer/src/token.rs
use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]  // skip spaces and tabs, but NOT newlines
pub enum Token {
    // --- Sigils / Variables ---
    // $name — scalar variable
    #[regex(r"\$[a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*)*", |lex| lex.slice().to_string())]
    ScalarVar(String),

    // @name — array variable
    #[regex(r"@[a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*)*", |lex| lex.slice().to_string())]
    ArrayVar(String),

    // %name — dict variable
    #[regex(r"%[a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*)*", |lex| lex.slice().to_string())]
    DictVar(String),

    // !name — error variable
    #[regex(r"![a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*)*", |lex| lex.slice().to_string())]
    ErrorVar(String),

    // *name — shared variable
    #[regex(r"\*[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    SharedVar(String),

    // ~name — regex variable (when used as variable, not literal)
    #[regex(r"~[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    RegexVar(String),

    // &::name — block reference
    #[regex(r"&::[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    BlockRef(String),

    // ::name — block definition or call
    #[regex(r"::[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    BlockName(String),

    // --- Literals ---
    // Float must come before Int to match greedily
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    // Integer
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok(), priority = 2)]
    Int(i64),

    // Triple-quoted string (multiline)
    #[regex(r#"""""#, priority = 3)]
    TripleQuote,

    // Double-quoted string
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    StringLit(String),

    // Duration literals: 100ms, 1s, 5m, 2h, 7d
    #[regex(r"[0-9]+ms", |lex| lex.slice().to_string())]
    DurationMs(String),
    #[regex(r"[0-9]+s", |lex| lex.slice().to_string(), priority = 3)]
    DurationS(String),
    #[regex(r"[0-9]+m", |lex| lex.slice().to_string(), priority = 3)]
    DurationM(String),
    #[regex(r"[0-9]+h", |lex| lex.slice().to_string())]
    DurationH(String),
    #[regex(r"[0-9]+d", |lex| lex.slice().to_string())]
    DurationD(String),

    // --- Keywords ---
    #[token("each")]
    Each,
    #[token("sequential")]
    Sequential,
    #[token("loop")]
    Loop,
    #[token("match")]
    Match,
    #[token("break")]
    Break,
    #[token("range")]
    Range,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("null")]
    Null,
    #[token("as")]
    As,
    #[token("to")]
    To,
    #[token("fail")]
    Fail,
    #[token("retry")]
    Retry,
    #[token("respond")]
    Respond,
    #[token("delay")]
    Delay,
    #[token("backoff")]
    Backoff,
    #[token("exponential")]
    Exponential,
    #[token("where")]
    Where,
    #[token("by")]
    By,
    #[token("desc")]
    Desc,
    #[token("template")]
    Template,
    #[token("validate")]
    Validate,
    #[token("required")]
    Required,
    #[token("binary")]
    Binary,
    #[token("recursive")]
    Recursive,
    #[token("redirect")]
    Redirect,
    #[token("json")]
    Json,
    #[token("xml")]
    Xml,
    #[token("yaml")]
    Yaml,
    #[token("csv")]
    Csv,
    #[token("toml")]
    Toml,
    #[token("data")]
    Data,
    #[token("rollback")]
    Rollback,
    #[token("timeout")]
    Timeout,

    // --- Operators ---
    #[token("|")]
    Pipe,
    #[token("->")]
    Arrow,
    #[token("=")]
    Eq,
    #[token("!=")]
    NotEq,
    #[token(">=")]
    GtEq,
    #[token("<=")]
    LtEq,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("**")]
    Power,
    #[token("/")]
    Slash,
    #[token("?")]
    Question,
    #[token(":")]
    Colon,
    #[token("&")]
    Ampersand,
    #[token(".")]
    Dot,

    // --- Delimiters ---
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,

    // --- Structure ---
    #[regex(r"\n")]
    Newline,

    // ## doc comment
    #[regex(r"##[^\n]*", |lex| lex.slice().to_string())]
    DocComment(String),

    // # prompt comment (will be stripped)
    #[regex(r"#[^\n]*", |lex| lex.slice().to_string(), priority = 1)]
    PromptComment(String),

    // Bare identifiers: print, filter, sort, http, sys, string, int, etc.
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*)*", |lex| lex.slice().to_string(), priority = 1)]
    Ident(String),
}
```

**Step 2: Verify it compiles**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo build`
Expected: Compiles (may have warnings about unused code, that's fine).

**Step 3: Commit**

```bash
git add compiler/crates/torqc-lexer/
git commit -m "feat: define TORQ token types with logos lexer"
```

---

### Task 4: Implement Lexer with Indentation Tracking

The lexer wraps logos tokenization and adds indent/dedent tracking plus span information.

**Files:**
- Create: `compiler/crates/torqc-lexer/src/lexer.rs`

**Step 1: Write the lexer**

```rust
// compiler/crates/torqc-lexer/src/lexer.rs
use logos::Logos;
use crate::token::Token;

/// A token with source location
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: LexToken,
    pub line: usize,
    pub col: usize,
}

/// Extended token type that includes indent/dedent
#[derive(Debug, Clone, PartialEq)]
pub enum LexToken {
    Token(Token),
    Indent,
    Dedent,
    Eof,
}

pub struct Lexer {
    tokens: Vec<SpannedToken>,
}

impl Lexer {
    pub fn tokenize(source: &str, file: &str) -> Result<Vec<SpannedToken>, LexError> {
        let mut result = Vec::new();
        let mut indent_stack: Vec<usize> = vec![0];
        let mut line = 1usize;

        for raw_line in source.lines() {
            // Skip blank lines
            if raw_line.trim().is_empty() {
                line += 1;
                continue;
            }

            // Skip prompt comments (# lines) — they get stripped entirely
            let trimmed = raw_line.trim();
            if trimmed.starts_with('#') && !trimmed.starts_with("##") {
                line += 1;
                continue;
            }

            // Calculate indentation (number of leading spaces)
            let indent = raw_line.len() - raw_line.trim_start().len();
            let current_indent = *indent_stack.last().unwrap();

            if indent > current_indent {
                indent_stack.push(indent);
                result.push(SpannedToken {
                    token: LexToken::Indent,
                    line,
                    col: 0,
                });
            } else {
                while indent < *indent_stack.last().unwrap() {
                    indent_stack.pop();
                    result.push(SpannedToken {
                        token: LexToken::Dedent,
                        line,
                        col: 0,
                    });
                }
            }

            // Tokenize the line content (trimmed of leading whitespace)
            let line_content = raw_line.trim_start();
            let mut lex = Token::lexer(line_content);

            while let Some(tok_result) = lex.next() {
                match tok_result {
                    Ok(token) => {
                        let span = lex.span();
                        result.push(SpannedToken {
                            token: LexToken::Token(token),
                            line,
                            col: indent + span.start + 1,
                        });
                    }
                    Err(()) => {
                        return Err(LexError {
                            file: file.to_string(),
                            line,
                            col: indent + lex.span().start + 1,
                            message: format!(
                                "unexpected character: {:?}",
                                &line_content[lex.span().start..lex.span().end]
                            ),
                        });
                    }
                }
            }

            // Add newline at end of each line
            result.push(SpannedToken {
                token: LexToken::Token(Token::Newline),
                line,
                col: raw_line.len() + 1,
            });

            line += 1;
        }

        // Close any remaining indentation
        while indent_stack.len() > 1 {
            indent_stack.pop();
            result.push(SpannedToken {
                token: LexToken::Dedent,
                line,
                col: 0,
            });
        }

        result.push(SpannedToken {
            token: LexToken::Eof,
            line,
            col: 0,
        });

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}: {}", self.file, self.line, self.col, self.message)
    }
}
```

**Step 2: Verify it compiles**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo build`
Expected: Compiles with no errors.

**Step 3: Commit**

```bash
git add compiler/crates/torqc-lexer/
git commit -m "feat: implement lexer with indentation tracking"
```

---

### Task 5: Write Lexer Tests

Test the lexer against real TORQ syntax from the examples.

**Files:**
- Create: `compiler/crates/torqc-lexer/tests/lexer_tests.rs`

**Step 1: Write failing tests**

```rust
// compiler/crates/torqc-lexer/tests/lexer_tests.rs
use torqc_lexer::lexer::{Lexer, LexToken, SpannedToken};
use torqc_lexer::token::Token;

fn lex(source: &str) -> Vec<SpannedToken> {
    Lexer::tokenize(source, "test.torq").expect("lexing failed")
}

fn tokens_only(source: &str) -> Vec<LexToken> {
    lex(source).into_iter().map(|s| s.token).collect()
}

#[test]
fn test_hello_world() {
    let tokens = tokens_only("::main\n  print \"hello world\"");
    assert!(tokens.contains(&LexToken::Token(Token::BlockName("::main".to_string()))));
    assert!(tokens.contains(&LexToken::Indent));
    assert!(tokens.contains(&LexToken::Token(Token::Ident("print".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::StringLit("\"hello world\"".to_string()))));
}

#[test]
fn test_scalar_variables() {
    let tokens = tokens_only("$name = \"alice\"");
    assert!(tokens.contains(&LexToken::Token(Token::ScalarVar("$name".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::Eq)));
    assert!(tokens.contains(&LexToken::Token(Token::StringLit("\"alice\"".to_string()))));
}

#[test]
fn test_array_literal() {
    let tokens = tokens_only("@numbers = [1 2 3 4 5]");
    assert!(tokens.contains(&LexToken::Token(Token::ArrayVar("@numbers".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::LBracket)));
    assert!(tokens.contains(&LexToken::Token(Token::Int(1))));
    assert!(tokens.contains(&LexToken::Token(Token::Int(5))));
    assert!(tokens.contains(&LexToken::Token(Token::RBracket)));
}

#[test]
fn test_dict_literal() {
    let tokens = tokens_only("%user = {\n  name \"alice\"\n  age 30\n}");
    assert!(tokens.contains(&LexToken::Token(Token::DictVar("%user".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::LBrace)));
    assert!(tokens.contains(&LexToken::Token(Token::Ident("name".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::Int(30))));
    assert!(tokens.contains(&LexToken::Token(Token::RBrace)));
}

#[test]
fn test_pipe_chain() {
    let tokens = tokens_only("$data | filter | sort | print");
    assert!(tokens.contains(&LexToken::Token(Token::ScalarVar("$data".to_string()))));
    assert!(tokens.iter().filter(|t| t.token == LexToken::Token(Token::Pipe)).count() == 3);
    assert!(tokens.contains(&LexToken::Token(Token::Ident("filter".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::Ident("sort".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::Ident("print".to_string()))));
}

#[test]
fn test_block_with_params() {
    let tokens = tokens_only("::greet $name $greeting");
    assert!(tokens.contains(&LexToken::Token(Token::BlockName("::greet".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::ScalarVar("$name".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::ScalarVar("$greeting".to_string()))));
}

#[test]
fn test_arrow_assignment() {
    let tokens = tokens_only("$result -> $output");
    assert!(tokens.contains(&LexToken::Token(Token::Arrow)));
}

#[test]
fn test_keywords() {
    let tokens = tokens_only("each sequential loop match break range true false null as to fail retry respond");
    assert!(tokens.contains(&LexToken::Token(Token::Each)));
    assert!(tokens.contains(&LexToken::Token(Token::Sequential)));
    assert!(tokens.contains(&LexToken::Token(Token::Loop)));
    assert!(tokens.contains(&LexToken::Token(Token::Match)));
    assert!(tokens.contains(&LexToken::Token(Token::Break)));
    assert!(tokens.contains(&LexToken::Token(Token::Range)));
    assert!(tokens.contains(&LexToken::Token(Token::True)));
    assert!(tokens.contains(&LexToken::Token(Token::False)));
    assert!(tokens.contains(&LexToken::Token(Token::Null)));
    assert!(tokens.contains(&LexToken::Token(Token::As)));
    assert!(tokens.contains(&LexToken::Token(Token::To)));
    assert!(tokens.contains(&LexToken::Token(Token::Fail)));
    assert!(tokens.contains(&LexToken::Token(Token::Retry)));
    assert!(tokens.contains(&LexToken::Token(Token::Respond)));
}

#[test]
fn test_comparison_operators() {
    let tokens = tokens_only(">= 21 <= 10 != 0 > 5 < 3");
    assert!(tokens.contains(&LexToken::Token(Token::GtEq)));
    assert!(tokens.contains(&LexToken::Token(Token::LtEq)));
    assert!(tokens.contains(&LexToken::Token(Token::NotEq)));
    assert!(tokens.contains(&LexToken::Token(Token::Gt)));
    assert!(tokens.contains(&LexToken::Token(Token::Lt)));
}

#[test]
fn test_math_operators() {
    let tokens = tokens_only("$a + $b - $c * $d / $e % $f ** $g");
    assert!(tokens.contains(&LexToken::Token(Token::Plus)));
    assert!(tokens.contains(&LexToken::Token(Token::Minus)));
    assert!(tokens.contains(&LexToken::Token(Token::Power)));
    assert!(tokens.contains(&LexToken::Token(Token::Slash)));
}

#[test]
fn test_duration_literals() {
    let tokens = tokens_only("100ms 1s 5m 2h 7d");
    assert!(tokens.contains(&LexToken::Token(Token::DurationMs("100ms".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::DurationS("1s".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::DurationM("5m".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::DurationH("2h".to_string()))));
    assert!(tokens.contains(&LexToken::Token(Token::DurationD("7d".to_string()))));
}

#[test]
fn test_float_literal() {
    let tokens = tokens_only("$price = 19.99");
    assert!(tokens.contains(&LexToken::Token(Token::Float(19.99))));
}

#[test]
fn test_block_ref() {
    let tokens = tokens_only("&::process");
    assert!(tokens.contains(&LexToken::Token(Token::BlockRef("&::process".to_string()))));
}

#[test]
fn test_shared_var() {
    let tokens = tokens_only("*counter = 0");
    assert!(tokens.contains(&LexToken::Token(Token::SharedVar("*counter".to_string()))));
}

#[test]
fn test_error_var() {
    let tokens = tokens_only("!err = \"not found\"");
    assert!(tokens.contains(&LexToken::Token(Token::ErrorVar("!err".to_string()))));
}

#[test]
fn test_doc_comment() {
    let tokens = tokens_only("## Payment Processing Service");
    assert!(tokens.iter().any(|t| matches!(&t.token, LexToken::Token(Token::DocComment(s)) if s.contains("Payment"))));
}

#[test]
fn test_prompt_comment_stripped() {
    // Prompt comments are stripped by the lexer entirely
    let tokens = tokens_only("# add retry logic\n$x = 1");
    assert!(!tokens.iter().any(|t| matches!(&t.token, LexToken::Token(Token::PromptComment(_)))));
    assert!(tokens.contains(&LexToken::Token(Token::ScalarVar("$x".to_string()))));
}

#[test]
fn test_indentation() {
    let source = "::main\n  print \"hello\"\n  print \"world\"";
    let tokens = tokens_only(source);
    assert!(tokens.contains(&LexToken::Indent));
    // After the block, we should get a dedent at EOF
    assert!(tokens.contains(&LexToken::Dedent));
}

#[test]
fn test_nested_indentation() {
    let source = "::main\n  @items | each $item\n    print $item";
    let tokens = tokens_only(source);
    let indent_count = tokens.iter().filter(|t| t.token == LexToken::Indent).count();
    let dedent_count = tokens.iter().filter(|t| t.token == LexToken::Dedent).count();
    assert_eq!(indent_count, 2);
    assert_eq!(dedent_count, 2);
}

#[test]
fn test_member_access_in_variable() {
    let tokens = tokens_only("$req.body");
    assert!(tokens.contains(&LexToken::Token(Token::ScalarVar("$req.body".to_string()))));
}

#[test]
fn test_dotted_function_call() {
    let tokens = tokens_only("sys.fs.read \"file.txt\"");
    assert!(tokens.contains(&LexToken::Token(Token::Ident("sys.fs.read".to_string()))));
}

#[test]
fn test_ternary() {
    let tokens = tokens_only("$active ? grant : deny");
    assert!(tokens.contains(&LexToken::Token(Token::Question)));
    assert!(tokens.contains(&LexToken::Token(Token::Colon)));
}

#[test]
fn test_lex_hello_example() {
    let source = "::main\n  print \"hello world\"";
    let result = Lexer::tokenize(source, "hello.torq");
    assert!(result.is_ok());
}

#[test]
fn test_lex_fibonacci_example() {
    let source = r#"::fibonacci $n
  $n | match
    0 -> 0
    1 -> 1
    _ -> ::fibonacci ($n - 1) + ::fibonacci ($n - 2)

::main
  range 1 20 | each $n sequential
    ::fibonacci $n | print"#;
    let result = Lexer::tokenize(source, "fibonacci.torq");
    assert!(result.is_ok());
}
```

**Step 2: Run the tests**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo test -p torqc-lexer`
Expected: Some tests may fail — this is the TDD red phase. Fix issues in the lexer until all pass.

**Step 3: Iterate until all tests pass**

Fix any token definition issues, priority conflicts, or indentation tracking bugs. Common issues:
- `*` as multiply operator vs `*name` as shared variable — the regex requires a letter after `*`
- `%` as modulo vs `%name` as dict variable — same pattern
- Duration literals may conflict with bare integers followed by identifiers
- Keyword tokens may conflict with identifiers (logos priority handles this)

**Step 4: Run tests to verify all pass**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo test -p torqc-lexer`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add compiler/crates/torqc-lexer/
git commit -m "feat: add lexer tests covering all TORQ syntax"
```

---

### Task 6: Implement Parser — Core Structure

Build the recursive descent parser that produces an AST from the lexer's token stream. Start with blocks, statements, and pipelines.

**Files:**
- Create: `compiler/crates/torqc-parser/src/parser.rs`

**Step 1: Write the parser core**

```rust
// compiler/crates/torqc-parser/src/parser.rs
use torqc_ast::ast::*;
use torqc_lexer::lexer::{LexToken, SpannedToken};
use torqc_lexer::token::Token;

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    file: String,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}: {}", self.file, self.line, self.col, self.message)
    }
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>, file: String) -> Self {
        Self { tokens, pos: 0, file }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut blocks = Vec::new();
        self.skip_newlines();

        while !self.is_at_end() {
            // Collect doc comments before block
            let doc_comments = self.collect_doc_comments();
            self.skip_newlines();

            if self.is_at_end() {
                break;
            }

            let mut block = self.parse_block()?;
            block.doc_comments = doc_comments;
            blocks.push(block);
            self.skip_newlines();
        }

        Ok(Program { blocks })
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let span = self.current_span();

        // Expect ::block_name
        let name = match self.peek_token() {
            Some(Token::BlockName(n)) => {
                let name = n[2..].to_string(); // strip "::"
                self.advance();
                name
            }
            other => return Err(self.error(format!(
                "expected block definition (::name), got {:?}", other
            ))),
        };

        // Collect parameters (sigil variables on the same line)
        let mut params = Vec::new();
        while !self.is_newline_or_end() {
            match self.peek_token() {
                Some(Token::ScalarVar(v)) => {
                    let pspan = self.current_span();
                    params.push(Param { sigil: Sigil::Scalar, name: v[1..].to_string(), span: pspan });
                    self.advance();
                }
                Some(Token::ArrayVar(v)) => {
                    let pspan = self.current_span();
                    params.push(Param { sigil: Sigil::Array, name: v[1..].to_string(), span: pspan });
                    self.advance();
                }
                Some(Token::DictVar(v)) => {
                    let pspan = self.current_span();
                    params.push(Param { sigil: Sigil::Dict, name: v[1..].to_string(), span: pspan });
                    self.advance();
                }
                _ => break,
            }
        }

        self.expect_newline()?;
        self.expect_indent()?;
        let body = self.parse_body()?;
        self.expect_dedent()?;

        Ok(Block { name, params, body, doc_comments: vec![], span })
    }

    fn parse_body(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();
        self.skip_newlines();

        loop {
            if self.check_dedent() || self.is_at_end() {
                break;
            }
            self.skip_newlines();
            if self.check_dedent() || self.is_at_end() {
                break;
            }
            let stmt = self.parse_statement()?;
            stmts.push(stmt);
            self.skip_newlines();
        }

        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        // Look ahead to determine statement type
        match self.peek_token() {
            Some(Token::Loop) => self.parse_loop_stmt(),
            _ => {
                // Parse expression, then check if it's an assignment or pipeline
                let expr = self.parse_pipeline()?;

                // Check for = assignment: $var = expr
                // This is handled differently — we check if the expr is a variable
                // followed by = (already parsed as part of expression)

                // Check for -> assignment: expr -> $var
                if self.check_token(&Token::Arrow) {
                    self.advance(); // consume ->
                    let target = self.parse_variable()?;
                    self.skip_to_newline();
                    return Ok(Statement::Assignment(Assignment {
                        target,
                        value: Box::new(expr),
                        span: self.current_span(),
                    }));
                }

                // Check if the pipeline starts with `each`
                if let Expr::Pipeline(ref p) = expr {
                    if p.stages.len() >= 2 {
                        // Could be an each statement handled during pipeline parsing
                    }
                }

                match expr {
                    Expr::Pipeline(p) if p.stages.len() > 1 => {
                        Ok(Statement::Pipeline(p))
                    }
                    other => Ok(Statement::Expression(other)),
                }
            }
        }
    }

    fn parse_pipeline(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        let mut stages = vec![self.parse_expression()?];

        while self.check_token(&Token::Pipe) {
            self.advance(); // consume |

            // Check for `each` — this makes it an each statement
            if self.check_token(&Token::Each) {
                self.advance(); // consume each
                let binding = self.parse_variable()?;
                let sequential = self.check_token(&Token::Sequential);
                if sequential {
                    self.advance();
                }
                self.expect_newline()?;
                self.expect_indent()?;
                let body = self.parse_body()?;
                self.expect_dedent()?;

                let iterable = if stages.len() == 1 {
                    stages.pop().unwrap()
                } else {
                    Expr::Pipeline(Pipeline { stages, span: span.clone() })
                };

                return Ok(Expr::Pipeline(Pipeline {
                    stages: vec![Expr::Variable(Variable {
                        sigil: Sigil::Scalar,
                        name: "__each__".to_string(),
                        span: span.clone(),
                    })],
                    span: span.clone(),
                }));
                // TODO: return Each properly as a statement — this needs
                // refactoring so parse_statement handles each directly.
                // For now, this is a placeholder.
            }

            // Check for `match`
            if self.check_token(&Token::Match) {
                self.advance(); // consume match
                self.expect_newline()?;
                self.expect_indent()?;
                let arms = self.parse_match_arms()?;
                self.expect_dedent()?;
                stages.push(Expr::Match(Match { arms, span: self.current_span() }));
                continue;
            }

            stages.push(self.parse_expression()?);
        }

        if stages.len() == 1 {
            Ok(stages.pop().unwrap())
        } else {
            Ok(Expr::Pipeline(Pipeline { stages, span }))
        }
    }

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_primary()?;

        // Check for binary operators
        if let Some(op) = self.peek_binop() {
            self.advance();
            let right = self.parse_expression()?;
            return Ok(Expr::BinOp(BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span: self.current_span(),
            }));
        }

        // Check for ternary
        if self.check_token(&Token::Question) {
            self.advance();
            let then_expr = self.parse_expression()?;
            self.expect_token(&Token::Colon)?;
            let else_expr = self.parse_expression()?;
            return Ok(Expr::Ternary(Ternary {
                condition: Box::new(left),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span: self.current_span(),
            }));
        }

        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek_token() {
            // Block call: ::name
            Some(Token::BlockName(n)) => {
                let span = self.current_span();
                let name = n[2..].to_string();
                self.advance();
                let args = self.collect_call_args()?;
                Ok(Expr::BlockCall(BlockCall { name, args, span }))
            }

            // Block reference: &::name
            Some(Token::BlockRef(n)) => {
                let span = self.current_span();
                let name = n[3..].to_string(); // strip "&::"
                self.advance();
                Ok(Expr::BlockRef(name, span))
            }

            // Scalar variable: $name
            Some(Token::ScalarVar(v)) => {
                let span = self.current_span();
                let name = v[1..].to_string();
                self.advance();
                Ok(Expr::Variable(Variable { sigil: Sigil::Scalar, name, span }))
            }

            // Array variable: @name
            Some(Token::ArrayVar(v)) => {
                let span = self.current_span();
                let name = v[1..].to_string();
                self.advance();
                Ok(Expr::Variable(Variable { sigil: Sigil::Array, name, span }))
            }

            // Dict variable: %name
            Some(Token::DictVar(v)) => {
                let span = self.current_span();
                let name = v[1..].to_string();
                self.advance();
                Ok(Expr::Variable(Variable { sigil: Sigil::Dict, name, span }))
            }

            // Error variable: !name
            Some(Token::ErrorVar(v)) => {
                let span = self.current_span();
                let name = v[1..].to_string();
                self.advance();
                Ok(Expr::Variable(Variable { sigil: Sigil::Error, name, span }))
            }

            // Shared variable: *name
            Some(Token::SharedVar(v)) => {
                let span = self.current_span();
                let name = v[1..].to_string();
                self.advance();
                Ok(Expr::Variable(Variable { sigil: Sigil::Shared, name, span }))
            }

            // Integer literal
            Some(Token::Int(n)) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Literal(Literal::Int(n, span)))
            }

            // Float literal
            Some(Token::Float(n)) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Literal(Literal::Float(n, span)))
            }

            // String literal
            Some(Token::StringLit(s)) => {
                let span = self.current_span();
                let content = s[1..s.len()-1].to_string(); // strip quotes
                self.advance();
                // Check for string interpolation
                if content.contains('$') {
                    let parts = self.parse_string_parts(&content, &span);
                    Ok(Expr::StringInterp(parts, span))
                } else {
                    Ok(Expr::Literal(Literal::String(content, span)))
                }
            }

            // Boolean true
            Some(Token::True) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true, span)))
            }

            // Boolean false
            Some(Token::False) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false, span)))
            }

            // Null
            Some(Token::Null) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Literal(Literal::Null(span)))
            }

            // Array literal: [1 2 3]
            Some(Token::LBracket) => self.parse_array_literal(),

            // Dict literal: { key "val" }
            Some(Token::LBrace) => self.parse_dict_literal(),

            // Grouped expression: (expr)
            Some(Token::LParen) => {
                let span = self.current_span();
                self.advance();
                let expr = self.parse_pipeline()?;
                self.expect_token(&Token::RParen)?;
                Ok(Expr::Group(Box::new(expr), span))
            }

            // Break
            Some(Token::Break) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Break(span))
            }

            // Respond keyword
            Some(Token::Respond) => {
                let span = self.current_span();
                self.advance();
                let args = self.collect_call_args()?;
                Ok(Expr::Call(Call { name: "respond".to_string(), args, span }))
            }

            // Fail keyword
            Some(Token::Fail) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Call(Call { name: "fail".to_string(), args: vec![], span }))
            }

            // Range keyword
            Some(Token::Range) => {
                let span = self.current_span();
                self.advance();
                let args = self.collect_call_args()?;
                Ok(Expr::Call(Call { name: "range".to_string(), args, span }))
            }

            // Retry keyword
            Some(Token::Retry) => {
                let span = self.current_span();
                self.advance();
                let args = self.collect_call_args()?;
                Ok(Expr::Call(Call { name: "retry".to_string(), args, span }))
            }

            // Duration literals
            Some(Token::DurationMs(d)) => {
                let span = self.current_span();
                let value = d.trim_end_matches("ms").parse::<u64>().unwrap();
                self.advance();
                Ok(Expr::Literal(Literal::Duration(Duration { value, unit: DurationUnit::Milliseconds }, span)))
            }
            Some(Token::DurationS(d)) => {
                let span = self.current_span();
                let value = d.trim_end_matches('s').parse::<u64>().unwrap();
                self.advance();
                Ok(Expr::Literal(Literal::Duration(Duration { value, unit: DurationUnit::Seconds }, span)))
            }
            Some(Token::DurationM(d)) => {
                let span = self.current_span();
                let value = d.trim_end_matches('m').parse::<u64>().unwrap();
                self.advance();
                Ok(Expr::Literal(Literal::Duration(Duration { value, unit: DurationUnit::Minutes }, span)))
            }
            Some(Token::DurationH(d)) => {
                let span = self.current_span();
                let value = d.trim_end_matches('h').parse::<u64>().unwrap();
                self.advance();
                Ok(Expr::Literal(Literal::Duration(Duration { value, unit: DurationUnit::Hours }, span)))
            }
            Some(Token::DurationD(d)) => {
                let span = self.current_span();
                let value = d.trim_end_matches('d').parse::<u64>().unwrap();
                self.advance();
                Ok(Expr::Literal(Literal::Duration(Duration { value, unit: DurationUnit::Days }, span)))
            }

            // Identifiers — builtin/function calls: print, filter, http.get, etc.
            Some(Token::Ident(name)) => {
                let span = self.current_span();
                self.advance();
                let args = self.collect_call_args()?;
                if args.is_empty() {
                    // Could be just a reference or a no-arg call
                    Ok(Expr::Call(Call { name, args, span }))
                } else {
                    Ok(Expr::Call(Call { name, args, span }))
                }
            }

            // Member access starting with dot: .field
            Some(Token::Dot) => {
                let span = self.current_span();
                self.advance();
                match self.peek_token() {
                    Some(Token::Ident(field)) => {
                        self.advance();
                        Ok(Expr::MemberAccess(MemberAccess {
                            object: Box::new(Expr::Variable(Variable {
                                sigil: Sigil::Scalar,
                                name: "_pipe_input".to_string(),
                                span: span.clone(),
                            })),
                            field,
                            span,
                        }))
                    }
                    _ => Err(self.error("expected field name after '.'".to_string())),
                }
            }

            // Keyword tokens that can act as identifiers in certain contexts
            Some(Token::Json) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Call(Call { name: "json".to_string(), args: vec![], span }))
            }
            Some(Token::Data) => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Call(Call { name: "data".to_string(), args: vec![], span }))
            }

            // Comparison operators at start of expression (in match arms)
            Some(Token::GtEq) | Some(Token::LtEq) | Some(Token::Gt) | Some(Token::Lt) => {
                // This happens in match arms: >= 21 -> "adult"
                // Return as-is, match arm parsing handles this
                Err(self.error("unexpected operator at start of expression".to_string()))
            }

            // Wildcard
            Some(Token::Ident(ref s)) if s == "_" => {
                let span = self.current_span();
                self.advance();
                Ok(Expr::Literal(Literal::Null(span))) // placeholder for wildcard
            }

            other => Err(self.error(format!("unexpected token: {:?}", other))),
        }
    }

    fn parse_array_literal(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.advance(); // consume [
        let mut elements = Vec::new();
        while !self.check_token(&Token::RBracket) && !self.is_at_end() {
            self.skip_newlines();
            elements.push(self.parse_expression()?);
        }
        self.expect_token(&Token::RBracket)?;
        Ok(Expr::Array(elements, span))
    }

    fn parse_dict_literal(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.advance(); // consume {
        let mut entries = Vec::new();
        self.skip_newlines();
        while !self.check_token(&Token::RBrace) && !self.is_at_end() {
            self.skip_newlines();
            if self.check_token(&Token::RBrace) {
                break;
            }
            let entry_span = self.current_span();
            let key = match self.peek_token() {
                Some(Token::Ident(k)) => { self.advance(); k }
                _ => return Err(self.error("expected dict key (identifier)".to_string())),
            };
            let value = self.parse_expression()?;
            entries.push(DictEntry { key, value, span: entry_span });
            self.skip_newlines();
        }
        self.expect_token(&Token::RBrace)?;
        Ok(Expr::Dict(entries, span))
    }

    fn parse_match_arms(&mut self) -> Result<Vec<MatchArm>, ParseError> {
        let mut arms = Vec::new();
        self.skip_newlines();
        while !self.check_dedent() && !self.is_at_end() {
            self.skip_newlines();
            if self.check_dedent() || self.is_at_end() {
                break;
            }
            let arm = self.parse_match_arm()?;
            arms.push(arm);
            self.skip_newlines();
        }
        Ok(arms)
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let span = self.current_span();
        let pattern = self.parse_pattern()?;
        self.expect_token(&Token::Arrow)?;

        // Body can be inline expression or block call
        let body = self.parse_pipeline()?;

        Ok(MatchArm {
            pattern,
            body: Box::new(body),
            span,
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        match self.peek_token() {
            // Wildcard _
            Some(Token::Ident(ref s)) if s == "_" => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            // Comparison: >= 21, < 10
            Some(Token::GtEq) => {
                self.advance();
                let expr = self.parse_expression()?;
                Ok(Pattern::Comparison(ComparisonOp::GtEq, Box::new(expr)))
            }
            Some(Token::LtEq) => {
                self.advance();
                let expr = self.parse_expression()?;
                Ok(Pattern::Comparison(ComparisonOp::LtEq, Box::new(expr)))
            }
            Some(Token::Gt) => {
                self.advance();
                let expr = self.parse_expression()?;
                Ok(Pattern::Comparison(ComparisonOp::Gt, Box::new(expr)))
            }
            Some(Token::Lt) => {
                self.advance();
                let expr = self.parse_expression()?;
                Ok(Pattern::Comparison(ComparisonOp::Lt, Box::new(expr)))
            }
            // Field match: .method = "GET"
            Some(Token::Dot) => {
                self.advance();
                let field = match self.peek_token() {
                    Some(Token::Ident(f)) => { self.advance(); f }
                    _ => return Err(self.error("expected field name".to_string())),
                };
                let op = self.parse_comparison_op()?;
                let value = self.parse_expression()?;
                let pattern = Pattern::FieldMatch(FieldMatch {
                    field,
                    op,
                    value: Box::new(value),
                });
                // Check for & (AND patterns)
                if self.check_token(&Token::Ampersand) {
                    let mut patterns = vec![pattern];
                    while self.check_token(&Token::Ampersand) {
                        self.advance();
                        patterns.push(self.parse_pattern()?);
                    }
                    Ok(Pattern::And(patterns))
                } else {
                    Ok(pattern)
                }
            }
            // Variable patterns: %user, !err, $value
            Some(Token::ScalarVar(_)) | Some(Token::DictVar(_)) | Some(Token::ErrorVar(_)) => {
                let var = self.parse_variable()?;
                Ok(Pattern::Variable(var))
            }
            // Literal patterns: 0, 1, "quit", true
            Some(Token::Int(_)) | Some(Token::Float(_)) | Some(Token::StringLit(_))
            | Some(Token::True) | Some(Token::False) | Some(Token::Null) => {
                let expr = self.parse_primary()?;
                match expr {
                    Expr::Literal(lit) => {
                        // Check for captures after literal: "build" $target
                        let mut captures = Vec::new();
                        while matches!(self.peek_token(), Some(Token::ScalarVar(_)) | Some(Token::ArrayVar(_)) | Some(Token::DictVar(_))) {
                            captures.push(self.parse_variable()?);
                        }
                        if captures.is_empty() {
                            Ok(Pattern::Literal(lit))
                        } else {
                            Ok(Pattern::WithCaptures(Box::new(Pattern::Literal(lit)), captures))
                        }
                    }
                    _ => Err(self.error("expected pattern".to_string())),
                }
            }
            _ => Err(self.error(format!("unexpected token in pattern: {:?}", self.peek_token()))),
        }
    }

    fn parse_loop_stmt(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `loop`

        // Optional condition
        let condition = if self.is_newline_or_end() {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };

        self.expect_newline()?;
        self.expect_indent()?;
        let body = self.parse_body()?;
        self.expect_dedent()?;

        Ok(Statement::Loop(Loop { condition, body, span }))
    }

    // --- Helper methods ---

    fn parse_variable(&mut self) -> Result<Variable, ParseError> {
        let span = self.current_span();
        match self.peek_token() {
            Some(Token::ScalarVar(v)) => {
                let name = v[1..].to_string();
                self.advance();
                Ok(Variable { sigil: Sigil::Scalar, name, span })
            }
            Some(Token::ArrayVar(v)) => {
                let name = v[1..].to_string();
                self.advance();
                Ok(Variable { sigil: Sigil::Array, name, span })
            }
            Some(Token::DictVar(v)) => {
                let name = v[1..].to_string();
                self.advance();
                Ok(Variable { sigil: Sigil::Dict, name, span })
            }
            Some(Token::ErrorVar(v)) => {
                let name = v[1..].to_string();
                self.advance();
                Ok(Variable { sigil: Sigil::Error, name, span })
            }
            Some(Token::SharedVar(v)) => {
                let name = v[1..].to_string();
                self.advance();
                Ok(Variable { sigil: Sigil::Shared, name, span })
            }
            _ => Err(self.error("expected variable ($, @, %, !, *)".to_string())),
        }
    }

    fn parse_comparison_op(&mut self) -> Result<ComparisonOp, ParseError> {
        match self.peek_token() {
            Some(Token::Eq) => { self.advance(); Ok(ComparisonOp::Eq) }
            Some(Token::NotEq) => { self.advance(); Ok(ComparisonOp::NotEq) }
            Some(Token::Gt) => { self.advance(); Ok(ComparisonOp::Gt) }
            Some(Token::Lt) => { self.advance(); Ok(ComparisonOp::Lt) }
            Some(Token::GtEq) => { self.advance(); Ok(ComparisonOp::GtEq) }
            Some(Token::LtEq) => { self.advance(); Ok(ComparisonOp::LtEq) }
            _ => Err(self.error("expected comparison operator".to_string())),
        }
    }

    fn collect_call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        while !self.is_newline_or_end() && !self.check_token(&Token::Pipe) && !self.check_token(&Token::Arrow) {
            // Stop on tokens that end an argument list
            match self.peek_token() {
                Some(Token::RBracket) | Some(Token::RBrace) | Some(Token::RParen) => break,
                Some(Token::Question) | Some(Token::Colon) => break,
                // Comparison operators end arg collection in match contexts
                Some(Token::GtEq) | Some(Token::LtEq) | Some(Token::Gt) | Some(Token::Lt)
                | Some(Token::NotEq) => break,
                Some(Token::Ampersand) => break,
                _ => {}
            }
            args.push(self.parse_primary()?);
        }
        Ok(args)
    }

    fn collect_doc_comments(&mut self) -> Vec<String> {
        let mut comments = Vec::new();
        while let Some(Token::DocComment(c)) = self.peek_token() {
            comments.push(c[2..].trim().to_string()); // strip "##"
            self.advance();
            self.skip_newlines();
        }
        comments
    }

    fn parse_string_parts(&self, content: &str, _span: &Span) -> Vec<StringPart> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = content.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' {
                if !current.is_empty() {
                    parts.push(StringPart::Literal(current.clone()));
                    current.clear();
                }
                let mut var_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '.' {
                        var_name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !var_name.is_empty() {
                    parts.push(StringPart::Interpolation(Variable {
                        sigil: Sigil::Scalar,
                        name: var_name,
                        span: Span { file: String::new(), line: 0, col: 0 },
                    }));
                }
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            parts.push(StringPart::Literal(current));
        }
        parts
    }

    fn peek_binop(&self) -> Option<BinOpKind> {
        match self.peek_token() {
            Some(Token::Plus) => Some(BinOpKind::Add),
            Some(Token::Minus) => Some(BinOpKind::Sub),
            Some(Token::Power) => Some(BinOpKind::Pow),
            Some(Token::Slash) => Some(BinOpKind::Div),
            _ => None,
        }
    }

    // --- Token navigation ---

    fn peek_token(&self) -> Option<Token> {
        self.tokens.get(self.pos).and_then(|st| {
            match &st.token {
                LexToken::Token(t) => Some(t.clone()),
                _ => None,
            }
        })
    }

    fn peek_lex_token(&self) -> Option<&LexToken> {
        self.tokens.get(self.pos).map(|st| &st.token)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn check_token(&self, expected: &Token) -> bool {
        matches!(self.peek_token(), Some(ref t) if t == expected)
    }

    fn expect_token(&mut self, expected: &Token) -> Result<(), ParseError> {
        if self.check_token(expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!("expected {:?}, got {:?}", expected, self.peek_token())))
        }
    }

    fn check_dedent(&self) -> bool {
        matches!(self.peek_lex_token(), Some(LexToken::Dedent))
    }

    fn is_newline_or_end(&self) -> bool {
        matches!(self.peek_lex_token(), Some(LexToken::Token(Token::Newline)) | Some(LexToken::Eof) | None)
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_lex_token(), Some(LexToken::Eof) | None)
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek_lex_token(), Some(LexToken::Token(Token::Newline))) {
            self.advance();
        }
    }

    fn skip_to_newline(&mut self) {
        while !self.is_newline_or_end() {
            self.advance();
        }
    }

    fn expect_newline(&mut self) -> Result<(), ParseError> {
        if matches!(self.peek_lex_token(), Some(LexToken::Token(Token::Newline))) {
            self.advance();
            Ok(())
        } else {
            // Be lenient — might be EOF
            Ok(())
        }
    }

    fn expect_indent(&mut self) -> Result<(), ParseError> {
        self.skip_newlines();
        if matches!(self.peek_lex_token(), Some(LexToken::Indent)) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!("expected indent, got {:?}", self.peek_lex_token())))
        }
    }

    fn expect_dedent(&mut self) -> Result<(), ParseError> {
        self.skip_newlines();
        if matches!(self.peek_lex_token(), Some(LexToken::Dedent)) {
            self.advance();
            Ok(())
        } else {
            // Be lenient at EOF
            if self.is_at_end() {
                Ok(())
            } else {
                Err(self.error(format!("expected dedent, got {:?}", self.peek_lex_token())))
            }
        }
    }

    fn current_span(&self) -> Span {
        if let Some(st) = self.tokens.get(self.pos) {
            Span { file: self.file.clone(), line: st.line, col: st.col }
        } else {
            Span { file: self.file.clone(), line: 0, col: 0 }
        }
    }

    fn error(&self, message: String) -> ParseError {
        let span = self.current_span();
        ParseError {
            file: span.file,
            line: span.line,
            col: span.col,
            message,
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo build`
Expected: Compiles. Warnings about unused fields/methods are fine.

**Step 3: Commit**

```bash
git add compiler/crates/torqc-parser/
git commit -m "feat: implement recursive descent parser for TORQ grammar"
```

---

### Task 7: Write Parser Tests

Test the parser against the example TORQ programs.

**Files:**
- Create: `compiler/crates/torqc-parser/tests/parser_tests.rs`

**Step 1: Write parser tests**

```rust
// compiler/crates/torqc-parser/tests/parser_tests.rs
use torqc_lexer::lexer::Lexer;
use torqc_parser::parser::Parser;
use torqc_ast::ast::*;

fn parse(source: &str) -> Program {
    let tokens = Lexer::tokenize(source, "test.torq").expect("lexing failed");
    let mut parser = Parser::new(tokens, "test.torq".to_string());
    parser.parse_program().expect("parsing failed")
}

#[test]
fn test_parse_hello_world() {
    let program = parse("::main\n  print \"hello world\"");
    assert_eq!(program.blocks.len(), 1);
    assert_eq!(program.blocks[0].name, "main");
    assert_eq!(program.blocks[0].params.len(), 0);
    assert_eq!(program.blocks[0].body.len(), 1);
}

#[test]
fn test_parse_block_with_params() {
    let program = parse("::greet $name $greeting\n  print $name");
    assert_eq!(program.blocks[0].name, "greet");
    assert_eq!(program.blocks[0].params.len(), 2);
    assert_eq!(program.blocks[0].params[0].name, "name");
    assert_eq!(program.blocks[0].params[1].name, "greeting");
}

#[test]
fn test_parse_scalar_assignment() {
    let program = parse("::main\n  $name = \"alice\"");
    assert_eq!(program.blocks.len(), 1);
    assert_eq!(program.blocks[0].body.len(), 1);
}

#[test]
fn test_parse_pipeline() {
    let program = parse("::main\n  $data | filter | sort | print");
    let body = &program.blocks[0].body;
    assert_eq!(body.len(), 1);
    match &body[0] {
        Statement::Pipeline(p) => {
            assert_eq!(p.stages.len(), 4);
        }
        other => panic!("expected Pipeline, got {:?}", other),
    }
}

#[test]
fn test_parse_multiple_blocks() {
    let source = "::add $a $b\n  $a + $b\n\n::main\n  ::add 1 2 | print";
    let program = parse(source);
    assert_eq!(program.blocks.len(), 2);
    assert_eq!(program.blocks[0].name, "add");
    assert_eq!(program.blocks[1].name, "main");
}

#[test]
fn test_parse_array_literal() {
    let program = parse("::main\n  @nums = [1 2 3 4 5]");
    assert_eq!(program.blocks[0].body.len(), 1);
}

#[test]
fn test_parse_dict_literal() {
    let source = "::main\n  %user = {\n    name \"alice\"\n    age 30\n  }";
    let program = parse(source);
    assert_eq!(program.blocks[0].body.len(), 1);
}

#[test]
fn test_parse_match_expression() {
    let source = "::main\n  $age | match\n    >= 21 -> \"adult\"\n    >= 18 -> \"teen\"\n    _ -> \"child\"";
    let program = parse(source);
    assert_eq!(program.blocks[0].body.len(), 1);
}

#[test]
fn test_parse_loop() {
    let source = "::main\n  $count = 0\n  loop $count < 100\n    $count + 1 -> $count";
    let program = parse(source);
    let body = &program.blocks[0].body;
    assert!(body.len() >= 2);
    assert!(matches!(&body[1], Statement::Loop(_)));
}

#[test]
fn test_parse_block_call_with_args() {
    let program = parse("::main\n  ::greet \"alice\" \"hello\"");
    let body = &program.blocks[0].body;
    assert_eq!(body.len(), 1);
    match &body[0] {
        Statement::Expression(Expr::BlockCall(bc)) => {
            assert_eq!(bc.name, "greet");
            assert_eq!(bc.args.len(), 2);
        }
        other => panic!("expected BlockCall expression, got {:?}", other),
    }
}

#[test]
fn test_parse_arrow_assignment() {
    let source = "::main\n  $a + $b -> $result";
    let program = parse(source);
    assert!(matches!(&program.blocks[0].body[0], Statement::Assignment(_)));
}

#[test]
fn test_parse_doc_comments() {
    let source = "## Payment Service\n## Handles payments\n::pay $amount\n  process $amount";
    let program = parse(source);
    assert_eq!(program.blocks[0].doc_comments.len(), 2);
    assert!(program.blocks[0].doc_comments[0].contains("Payment Service"));
}

#[test]
fn test_parse_fibonacci_example() {
    let source = r#"::fibonacci $n
  $n | match
    0 -> 0
    1 -> 1
    _ -> ::fibonacci ($n - 1) + ::fibonacci ($n - 2)

::main
  range 1 20 | each $n sequential
    ::fibonacci $n | print"#;
    let program = parse(source);
    assert_eq!(program.blocks.len(), 2);
    assert_eq!(program.blocks[0].name, "fibonacci");
    assert_eq!(program.blocks[1].name, "main");
}

#[test]
fn test_parse_web_server_example() {
    let source = r#"::health
  respond 200 %{status "ok"}

::main
  http.listen 8080
  | route get "/health" -> ::health"#;
    let program = parse(source);
    assert_eq!(program.blocks.len(), 2);
}
```

**Step 2: Run tests**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo test -p torqc-parser`
Expected: Some tests will fail initially. This is the TDD red phase.

**Step 3: Fix parser issues until all tests pass**

Common issues to fix:
- `=` assignment parsing (currently only handles `->` assignment)
- `each` as part of a pipeline needs special handling
- Match arms inside pipes need correct indentation tracking
- Nested blocks/indentation edge cases

**Step 4: Run tests to verify all pass**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo test -p torqc-parser`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add compiler/crates/torqc-parser/
git commit -m "feat: add parser tests, fix parsing edge cases"
```

---

### Task 8: Wire Up CLI — `torqc parse`

Connect lexer and parser to the CLI so `torqc parse file.torq` prints the AST as JSON.

**Files:**
- Modify: `compiler/crates/torqc-cli/src/main.rs`

**Step 1: Implement the CLI**

```rust
// compiler/crates/torqc-cli/src/main.rs
use clap::{Parser as ClapParser, Subcommand};
use std::path::PathBuf;

#[derive(ClapParser)]
#[command(name = "torqc", about = "TORQ compiler")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a TORQ file and print the AST
    Parse {
        /// Path to .torq file
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file } => cmd_parse(file),
    }
}

fn cmd_parse(file: PathBuf) {
    let source = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✗ Could not read {}: {}", file.display(), e);
            std::process::exit(1);
        }
    };

    let filename = file.to_string_lossy().to_string();

    let tokens = match torqc_lexer::lexer::Lexer::tokenize(&source, &filename) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("✗ Lex error: {}", e);
            std::process::exit(1);
        }
    };

    let mut parser = torqc_parser::parser::Parser::new(tokens, filename);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("✗ Parse error: {}", e);
            std::process::exit(1);
        }
    };

    let json = serde_json::to_string_pretty(&program).unwrap();
    println!("{}", json);
    eprintln!("✓ Parsed {} block(s)", program.blocks.len());
}
```

**Step 2: Build and test with hello.torq**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo build && cargo run -- parse ../examples/hello.torq`
Expected: Prints JSON AST with one block named "main" containing a print call.

**Step 3: Test with fibonacci.torq**

Run: `cargo run -- parse ../examples/fibonacci.torq`
Expected: Prints JSON AST with two blocks: "fibonacci" and "main".

**Step 4: Test with all examples**

Run each example file — fix any parsing failures that come up. These are the first real integration tests.

```bash
cargo run -- parse ../examples/hello.torq
cargo run -- parse ../examples/fibonacci.torq
cargo run -- parse ../examples/data_processing.torq
cargo run -- parse ../examples/cli_tool.torq
cargo run -- parse ../examples/web_server.torq
cargo run -- parse ../examples/checkout_service.torq
```

**Step 5: Commit**

```bash
git add compiler/crates/torqc-cli/
git commit -m "feat: wire up torqc parse CLI command with JSON AST output"
```

---

### Task 9: Integration Tests — All Examples Must Parse

Create integration tests that parse every example file and verify the output.

**Files:**
- Create: `compiler/tests/integration/parse_examples.rs`
- Create: `compiler/tests/integration/mod.rs`

**Step 1: Write integration tests**

```rust
// compiler/tests/integration/parse_examples.rs
use torqc_lexer::lexer::Lexer;
use torqc_parser::parser::Parser;
use std::path::Path;

fn parse_file(path: &str) -> Result<torqc_ast::ast::Program, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("read error: {}", e))?;
    let tokens = Lexer::tokenize(&source, path)
        .map_err(|e| format!("lex error: {}", e))?;
    let mut parser = Parser::new(tokens, path.to_string());
    parser.parse_program()
        .map_err(|e| format!("parse error: {}", e))
}

#[test]
fn test_parse_hello() {
    let prog = parse_file("../examples/hello.torq").unwrap();
    assert_eq!(prog.blocks.len(), 1);
    assert_eq!(prog.blocks[0].name, "main");
}

#[test]
fn test_parse_fibonacci() {
    let prog = parse_file("../examples/fibonacci.torq").unwrap();
    assert_eq!(prog.blocks.len(), 2);
    assert_eq!(prog.blocks[0].name, "fibonacci");
    assert_eq!(prog.blocks[1].name, "main");
}

#[test]
fn test_parse_data_processing() {
    let prog = parse_file("../examples/data_processing.torq").unwrap();
    assert!(prog.blocks.len() >= 2);
}

#[test]
fn test_parse_cli_tool() {
    let prog = parse_file("../examples/cli_tool.torq").unwrap();
    assert!(prog.blocks.len() >= 4); // build, test, deploy, version, main
}

#[test]
fn test_parse_web_server() {
    let prog = parse_file("../examples/web_server.torq").unwrap();
    assert!(prog.blocks.len() >= 5); // health, get_users, get_user, create_user, delete_user, main
}

#[test]
fn test_parse_checkout_service() {
    let prog = parse_file("../examples/checkout_service.torq").unwrap();
    assert!(prog.blocks.len() >= 5); // validate_cart, calculate_total, process_payment, send_receipt, checkout, main
}

#[test]
fn test_all_examples_parse_without_error() {
    let examples_dir = Path::new("../examples");
    for entry in std::fs::read_dir(examples_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "torq") {
            let result = parse_file(path.to_str().unwrap());
            assert!(result.is_ok(), "Failed to parse {}: {:?}", path.display(), result.err());
        }
    }
}
```

**Step 2: Run integration tests**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo test --test integration`
Note: The test binary name depends on file layout. If using a `tests/` directory at workspace root, you may need to create a standalone test crate or put these in a crate's test directory.

Alternative — place in `compiler/crates/torqc-cli/tests/integration.rs` since it depends on all crates:

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo test -p torqc-cli`
Expected: All examples parse successfully.

**Step 3: Fix any failures**

Iterate on parser until every example file produces a valid AST.

**Step 4: Commit**

```bash
git add compiler/
git commit -m "feat: add integration tests — all TORQ examples parse successfully"
```

---

### Task 10: Final Cleanup and Phase 1 Completion

Ensure everything builds cleanly, all tests pass, and the milestone is met.

**Step 1: Run full test suite**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo test`
Expected: All tests pass across all crates.

**Step 2: Run clippy for lint checks**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo clippy -- -D warnings`
Expected: No warnings.

**Step 3: Verify the milestone**

Run: `cd /Users/cwdavis/torq-lang/compiler && cargo run -- parse ../examples/checkout_service.torq`
Expected: Full JSON AST output for the most complex example, with correct block count.

**Step 4: Update CLAUDE.md with build commands**

Add to CLAUDE.md:
```
## Build Commands

All commands run from the `compiler/` directory.

- `cargo build` — build the compiler
- `cargo test` — run all tests
- `cargo test -p torqc-lexer` — run lexer tests only
- `cargo test -p torqc-parser` — run parser tests only
- `cargo test -p torqc-cli` — run integration tests
- `cargo clippy -- -D warnings` — lint
- `cargo run -- parse <file.torq>` — parse a TORQ file and print AST as JSON
```

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: complete Phase 1 — lexer + parser producing AST for all TORQ examples"
```
