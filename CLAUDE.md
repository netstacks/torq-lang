# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.
You should be the primary developer, The human is only there to answer architecture questions and provide governance. 

- Be Honest and push back when need be. 
- DO NOT constantly tell the human "Your Right...."

## Project Overview

TORQ is a token-optimized programming language designed for AI to write while remaining human-auditable. It's currently in the **specification phase** — the language grammar and spec are complete, but the compiler (`torqc`) and standard library have not been implemented yet.

The `compiler/` and `stdlib/` directories are empty placeholders for future implementation.

## Repository Structure

- `docs/TORQ-LANGUAGE-SPEC.md` — Complete language specification (~1050 lines). The authoritative reference for all syntax, semantics, and design decisions.
- `docs/TORQ-AI-PROMPT.md` — System prompt for AI code generation (~115 lines). Condensed grammar rules ready to paste into any AI model.
- `examples/*.torq` — Seven example programs demonstrating language features (hello world through full e-commerce checkout service).
- `examples/torq.yaml` — Configuration file example showing service definitions and environment config.

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
