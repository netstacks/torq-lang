# TORQ Compiler Design

## Overview

`torqc` is an AI-enhanced compiler for the TORQ programming language. Written in Rust, it uses Cranelift for native code generation and integrates LLM-based error recovery at every compilation stage. The compiler autonomously fixes issues before surfacing errors to the developer.

## Architecture

### Compilation Pipeline

```
TORQ Source (.torq)
    │
    ▼
┌──────────────────┐    fail    ┌──────────┐
│  1. Lexer        │──────────▶│  AI Fix  │──┐
│     (logos)       │           └──────────┘  │
└────────┬─────────┘              retry      │
         │◀──────────────────────────────────┘
         ▼
┌──────────────────────┐    fail    ┌──────────┐
│  2. Parser           │──────────▶│  AI Fix  │──┐
│     (lalrpop)        │           └──────────┘  │
└────────┬─────────────┘              retry      │
         │◀──────────────────────────────────────┘
         ▼
┌──────────────────────┐    fail    ┌──────────┐
│  3. Semantic         │──────────▶│  AI Fix  │──┐
│     Analysis         │           └──────────┘  │
└────────┬─────────────┘              retry      │
         │◀──────────────────────────────────────┘
         ▼
┌──────────────────────┐    fail    ┌──────────┐
│  4. Codegen          │──────────▶│  AI Fix  │──┐
│     (Cranelift)      │           └──────────┘  │
└────────┬─────────────┘              retry      │
         │◀──────────────────────────────────────┘
         ▼
    Native Binary
```

Each AI checkpoint retries up to 3 times (configurable). If the AI cannot fix the issue, the original error surfaces to the developer. Every stage is deterministic on its own — AI acts as an error interceptor between stages, not inside them.

### Stage 1: Lexer

Tokenizes TORQ's sigil-based syntax using the `logos` crate. Sigils make tokenization unambiguous — the first character determines token type.

**Token types:**

| Category | Tokens |
|---|---|
| Sigils | `$name` `@name` `%name` `&name` `!name` `~pattern` `*name` |
| Block def | `::name` |
| Comments | `#` (prompt, stripped) `##` (doc, preserved) |
| Operators | `\|` (pipe) `->` (assignment) |
| Keywords | `each` `loop` `match` `fail` `sequential` `as` `to` `retry` `delay` `backoff` `true` `false` `null` `_` |
| Literals | `"strings"` `42` `3.14` `~regex~` |
| Whitespace | Significant for indentation (emit indent/dedent tokens) |

**Design notes:**
- No commas, semicolons, or brackets to track.
- `#` vs `##` must be distinguished: prompt comments are deleted entirely, doc comments are preserved.
- Indentation sensitivity requires emitting indent/dedent tokens (similar to Python's tokenizer).
- Regex literals `~pattern~` must not conflict with the `~` sigil for regex-typed variables.

### Stage 2: Parser

Builds an AST from the token stream using `lalrpop`. TORQ's grammar is small — roughly 50 grammar rules.

**Core AST node types:**

```
Program         → list of Blocks
Block           → name, parameters, body (list of Statements)
Statement       → Pipeline | Loop | Each | Match | Assignment
Pipeline        → chain of Expressions connected by |
Expression      → Literal | Variable | BlockCall | BuiltinCall
Assignment      → expression -> $variable
Match           → expression | match + list of Arms
Each            → expression | each $var + body
Loop            → loop condition + body
```

**Block resolution is global.** Parsing is a two-pass operation:

1. **First pass**: scan all `.torq` files, collect `::block_name` definitions and their signatures.
2. **Second pass**: parse block bodies, resolve `::block_name` calls and `&block_name` references against the collected names.

Duplicate block names across any files are a hard compile error — no ambiguity, no shadowing.

**Pipe chains** are the most important structure. A pipeline like `$data | filter | ::transform | to json` becomes a linked chain of expressions where each node's output feeds the next node's input.

### Stage 3: Semantic Analysis

Validates that the AST is logically correct. Runs traditional deterministic checks first, then AI fixes any issues found.

**Deterministic checks:**
- **Type consistency** — a `$` scalar piped into a function expecting `@` array is an error.
- **Block existence** — every `::name` call and `&name` reference resolves to a real block.
- **Pipe compatibility** — each step in a pipeline produces output the next step can consume.
- **Shared state safety** — `*` variables are only accessed through valid patterns.
- **Fail coverage** — operations that can produce `!` errors have `fail` handlers or propagate explicitly.
- **No circular pipes** — data flows forward only, no cycles.

**AI fix loop (on by default):**

When semantic analysis finds issues, the AI fixes them automatically:

```
Semantic check fails
    → send issue + AST + source to LLM
    → LLM returns fixed TORQ source
    → re-parse and re-check
    → repeat up to 3 attempts
    → if still broken, surface error to developer
```

With `--ai` flag, the AI also performs deeper analysis:
- Missing timeout/retry on HTTP calls
- Dead code detection (blocks defined but never called)
- Race conditions (`each` modifying `*` vars without `sequential`)
- Data parsed but never validated against a schema

### Stage 4: Codegen (Cranelift)

Translates the validated AST into Cranelift IR, which Cranelift compiles to native machine code.

**TORQ to Cranelift IR mapping:**

| TORQ Concept | Cranelift IR |
|---|---|
| `::block` | Function definition |
| `$scalar` | Single SSA value (i64, f64, or pointer) |
| `@array` | Pointer to heap-allocated array struct |
| `%dict` | Pointer to heap-allocated hashmap struct |
| Pipe `\|` | Chain of function calls, output → input |
| `each` (parallel) | Spawns OS threads or async tasks |
| `each sequential` | Simple loop |
| `loop` | Basic block with back-edge |
| `match` | Branch/switch IR |
| `-> $var` | SSA variable assignment |

**Arena memory model:** Each `::block` invocation allocates a memory arena. All `@` and `%` allocations within that block come from its arena. When the block returns, the entire arena frees at once. No garbage collector, no reference counting — one deallocation per block exit.

**Output:** A statically linked native binary. One file, no dependencies, runs anywhere.

### Runtime Library (libtorq)

A Rust library statically linked into every compiled TORQ binary. Contains implementations for TORQ's 170+ stdlib functions and core runtime machinery.

**Core runtime:**
- Arena allocator
- Thread pool for parallel `each`
- Pipe execution engine
- Error (`!`) propagation

**Stdlib implementations:**
- String operations (upper, lower, trim, split, replace, contains, etc.)
- Array operations (first, last, push, pop, sort, filter, map, reduce, etc.)
- Dict operations (keys, values, get, set, merge, pick, omit)
- Math (abs, round, floor, ceil, min, max, sqrt, random)
- Time (now, parse, diff, add, zone, unix, sleep)
- Filesystem (read, write, delete, exists, list, move, copy, watch)
- HTTP client + server (listen, route, respond, get, post, put, delete)
- Database (connect, query, insert, update, delete, transaction, migrate)
- Crypto (hash, hmac, encrypt, decrypt, uuid, jwt.sign, jwt.verify)
- Data parsing (JSON, YAML, CSV, XML, TOML — `as` and `to` operators)
- Logging (log, log.info, log.warn, log.err, log.debug)
- System (env, exec, args, exit, pid, hostname)
- Scheduling (every, at, cron, after)

### AI Integration

LLM integration is handled by the `torqc-ai` crate. It provides a unified interface used by all pipeline stages for error recovery.

**AI fix flow (all stages):**

```
Stage fails with error
    → torqc-ai serializes: error message + source context + stage info
    → sends to configured LLM provider
    → LLM returns fixed TORQ source
    → re-run failed stage with fixed source
    → up to max_retries attempts (default 3)
    → on success: log what was fixed, continue compilation
    → on failure: surface original error to developer
```

**AI tests use mocked LLM responses.** Record real responses once, replay them in CI. Deterministic and free.

## AI Provider Configuration

**Setup:**

```bash
$ torqc ai setup
AI Provider:
  1. Anthropic (recommended)
  2. OpenAI
  3. Local (Ollama)
  4. Custom endpoint

> 1

API Key: sk-ant-...
✓ Saved to ~/.torqc/config.yaml
```

**Global config (`~/.torqc/config.yaml`):**

```yaml
ai:
  provider: anthropic
  key: sk-ant-xxxxx
  model: claude-sonnet-4-6
  max_retries: 3
```

**Per-project override (optional, in `torq.yaml`):**

```yaml
ai:
  provider: openai
  model: gpt-4o
  max_retries: 5
```

**Three modes:**

| Mode | Flag | Behavior |
|---|---|---|
| Default | (none) | AI fixes compilation errors only |
| Full | `--ai` | Error fixing + semantic analysis + suggestions |
| Off | `--no-ai` | Traditional compiler, no LLM calls |

Local mode via Ollama works offline — smaller model, less capable fixes, but no API costs or latency. Without any AI configured, `torqc` works as a traditional compiler.

## User Experience

**Happy path:**
```bash
$ torqc build src/
✓ Parsed 4 files (12 blocks)
✓ Built checkout-service (1.2s)

$ ./checkout-service
Server listening on :8080
```

**AI fixes issues automatically:**
```bash
$ torqc build src/
✓ Parsed 4 files (12 blocks)
⚠ Go compilation failed: type mismatch in ::calculate_tax
  → AI fix applied (attempt 1/3)
✓ Built checkout-service (3.8s)
```

**AI can't fix (falls back to developer):**
```bash
$ torqc build src/
✗ Parse error in src/main.torq:14
  → AI fix failed after 3 attempts

  Block ::process_order references undefined block ::validate_cart
  Did you mean ::validate_items?
```

**Full AI analysis:**
```bash
$ torqc build --ai src/
✓ Parsed 4 files (12 blocks)
⚠ AI fixes (2 applied):
  src/checkout.torq:22 — ::charge_card had no fail handler;
    added fail with retry 3 delay 1s
  src/notify.torq:8 — each loop modified *counter without
    sequential; added sequential flag
✓ Built checkout-service (5.1s)
```

**`--verbose` shows full AI diffs** for human auditing.

## CLI Commands

```bash
torqc build <path>           # Compile to native binary
torqc build --ai <path>      # Compile with full AI analysis
torqc build --no-ai <path>   # Compile without AI
torqc build --verbose <path> # Show AI fix details
torqc run <file.torq>        # Compile and run immediately
torqc check <path>           # Semantic analysis only, no binary
torqc parse <file.torq>      # Print AST (debugging)
torqc test <path>            # Run ::test_ blocks
torqc ai setup               # Configure AI provider
```

## Testing Infrastructure

### Layer 1: Compiler Tests (Rust)

Validate that `torqc` itself works correctly. Run with `cargo test`.

```
compiler/tests/
  lexer/          # Token output for known inputs
  parser/         # AST output for known inputs
  semantic/       # Error detection and type checking
  codegen/        # Generated binary produces correct output
  ai/             # AI fix loop with mocked LLM responses
  integration/    # Full .torq → binary → run → check output
```

**Integration tests** use every file in `examples/` as a test case. Each example must always compile and produce expected output — this is the compiler's regression suite.

**AI tests** use mocked/recorded LLM responses. No real API calls in CI.

### Layer 2: TORQ Test Framework

Built into the language for TORQ developers to test their own programs.

Test blocks are prefixed with `::test_`:

```
::test_calculate_tax
  %order = { "subtotal" 100.00  "state" "CA" }
  %result = %order | ::calculate_tax
  %result.tax | assert 8.25
  %result.total | assert 108.25
```

Run with `torqc test`:

```bash
$ torqc test src/
✓ test_calculate_tax (2ms)
✓ test_process_order (15ms)
✗ test_send_receipt — expected "sent" got !timeout
  → AI fix: added retry to http.post in ::send_receipt
  → rerun: ✓ test_send_receipt (340ms)

3/3 passed (1 AI-fixed)
```

Test blocks do not compile into the production binary.

## Project Structure

```
torq-lang/
  docs/                         # Spec, AI prompt, design docs
  examples/                     # .torq examples (also integration tests)
  compiler/
    Cargo.toml                  # Rust workspace root
    crates/
      torqc-lexer/              # Lexer (logos)
      torqc-parser/             # Parser (lalrpop)
      torqc-ast/                # AST type definitions, shared
      torqc-semantic/           # Semantic analysis
      torqc-codegen/            # Cranelift IR generation
      torqc-ai/                 # LLM integration, error recovery
      torqc-runtime/            # libtorq — arena, stdlib
      torqc-cli/                # CLI entry point
    tests/
      lexer/
      parser/
      semantic/
      codegen/
      ai/
      integration/
```

Cargo workspace with one crate per compiler stage. Each crate compiles and tests independently.

## Build Phases

| Phase | Milestone | What works after |
|---|---|---|
| **1** | Lexer + Parser | `torqc parse file.torq` prints AST |
| **2** | Semantic analysis | `torqc check file.torq` reports errors |
| **3** | Codegen (basic) | `torqc build hello.torq` produces a binary that prints a string |
| **4** | Codegen (control flow) | `match`, `each sequential`, `loop` work |
| **5** | Runtime (stdlib core) | String ops, arrays, dicts, math, I/O |
| **6** | Parallelism | `each` (parallel default), `*` shared state |
| **7** | AI integration | Error recovery and semantic fixing |
| **8** | Runtime (stdlib full) | HTTP, DB, crypto, scheduling |
| **9** | Test framework | `torqc test` with `::test_` blocks |
| **10** | Polish | `torq.yaml` config, incremental compilation, packaging |

Each phase produces a usable checkpoint. After phase 3, TORQ can compile and run real programs.
