# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.
You should be the primary developer, The human is only there to answer architecture questions and provide governance. 

- Be Honest and push back when need be. 
- DO NOT constantly tell the human "Your Right...."

## Project Overview

TORQ is a token-optimized programming language designed for AI to write while remaining human-auditable. The language spec is complete and the compiler (lexer, parser, semantic analysis, codegen) is implemented in Rust with Cranelift backend. The `torqc build` command compiles `.torq` files to native binaries. All values are dynamically typed TorqValue pointers managed by the C runtime.

## Repository Structure

- `docs/TORQ-LANGUAGE-SPEC.md` — Complete language specification (~1050 lines). The authoritative reference for all syntax, semantics, and design decisions.
- `docs/TORQ-AI-PROMPT.md` — System prompt for AI code generation (~115 lines). Condensed grammar rules ready to paste into any AI model.
- `docs/plans/` — Design documents and implementation plans.
- `examples/*.torq` — Seven example programs demonstrating language features (hello world through full e-commerce checkout service).
- `examples/torq.yaml` — Configuration file example showing service definitions and environment config.
- `compiler/` — Rust workspace for the `torqc` compiler (see Build Commands below).

## Build Commands

All commands run from the `compiler/` directory.

- `cargo build` — build the compiler
- `cargo test` — run all tests (244 tests across lexer, parser, semantic, codegen, CLI)
- `cargo test -p torqc-lexer` — run lexer tests only
- `cargo test -p torqc-parser` — run parser tests only
- `cargo test -p torqc-semantic` — run semantic analysis tests only
- `cargo clippy` — lint
- `cargo run -p torqc-cli -- parse <file.torq>` — parse a TORQ file and print AST as JSON
- `cargo run -p torqc-cli -- check <file.torq>` — run semantic analysis (parse + 5 validation passes)
- `cargo run -p torqc-cli -- build <file.torq>` — compile to native binary

## Compiler Architecture

The compiler is a Rust workspace at `compiler/` with 6 crates:

- `torqc-ast` — AST type definitions shared across crates
- `torqc-lexer` — Tokenizer using logos, with indentation tracking (emits Indent/Dedent tokens)
- `torqc-parser` — Recursive descent parser producing AST from token stream
- `torqc-semantic` — Semantic analysis with 5 validation passes (see below)
- `torqc-codegen` — Cranelift-based code generator with embedded C runtime (`runtime/torq_runtime.c`); emits object files linked via system `cc` to produce native binaries. All values are TorqValue* pointers (tagged union: int, float, bool, string, array, dict, null). Runtime provides constructors, arithmetic, comparisons, string ops, array/dict ops, math, I/O, and JSON serialization. Codegen supports variables, arithmetic, comparisons, user-defined blocks with recursion, match expressions, loops, each sequential with range, arrays, dicts, member access, string interpolation, and the full stdlib pipeline.
- `torqc-cli` — CLI entry point (`torqc parse`, `torqc check`, and `torqc build` commands)

The `torqc-semantic` crate runs 5 passes over the parsed AST:

1. **Block registry** — builds a name-to-block map; detects duplicate block definitions
2. **Block resolution** — verifies all `::block_call` and `&block_ref` targets exist; suggests "did you mean?" for close misspellings (Levenshtein distance)
3. **Parameter arity** — checks that block calls pass the correct number of arguments
4. **Variable scope** — tracks variable definitions (assignments, params, each bindings, match patterns) and reports use of undefined variables; `*shared` variables are always in scope
5. **Control flow** — validates `break` only appears inside `loop` bodies; warns when `*shared` variables are assigned inside parallel `each` (suggests adding `sequential`)

Token types are fieldless enums — the actual source text is stored in `SpannedToken.text`. The parser extracts variable names, string contents, and numbers from this text field.

## Language Essentials

**Sigil type system** — types are single-character prefixes, not keywords:
- `$` scalar, `@` array, `%` dict, `::` block definition, `&` block reference, `!` error, `~` regex, `*` shared/concurrent value

**Syntax rules:**
- No imports, no type declarations, no commas, no semicolons
- Pipe-based data flow: `$data | transform | output`
- Only 3 control flow constructs: `each` (parallel iteration), `loop`, `match`
- 15 total reserved words
- Blocks (`::name`) are globally visible across all `.torq` files — no import resolution needed
- Last expression in a block is the return value

**Concurrency model:** Parallel by default, opt-out with `sequential` keyword.

**Memory model:** Arena-scoped per block, freed on block exit. No GC, no borrow checker. Forward-only pipe data flow guarantees no backward references.

## Design Principles to Follow

When writing or modifying TORQ spec, examples, or future compiler code:

1. **Token minimalism** — every syntax choice should minimize the tokens an AI spends generating code. If there's a shorter way that's still readable, prefer it.
2. **Zero ambiguity** — one canonical way to express each concept. Never add a second way to do the same thing.
3. **AI-first, human-auditable** — optimized for AI authorship but must remain readable by humans for review.
4. **170+ built-in functions** — the stdlib ships with the runtime (string, array, dict, math, time, crypto, http, db, fs, etc.). No package manager needed for core operations.
5. **Service integration via OpenAPI specs** — external services defined in `torq.yaml`, auto-generated into `torq_modules/` (gitignored).

## Working with Examples

Examples in `examples/` range from trivial (`hello.torq`) to complex (`checkout_service.torq`). They serve as both documentation and future integration tests. When modifying the spec, ensure examples remain consistent with spec changes.
