# TORQ

**The first programming language designed for AI to write.**

TORQ is a token-optimized, compiled, general-purpose programming language. AI writes it. Humans approve it. Machines run it fast.

## Why TORQ?

Every programming language today was designed for humans to write. TORQ inverts this — it's optimized for AI authorship while remaining readable for human review.

| Problem | TORQ's Answer |
|---------|--------------|
| AI wastes tokens on boilerplate | Zero imports, zero type declarations, sigil-based types |
| AI picks wrong syntax patterns | One way to do each thing — zero ambiguity |
| Runtime performance | Compiles to native code, arena-scoped memory, zero GC |
| Concurrency is hard | Parallel by default, sequential opt-out |
| Data format juggling | Native JSON, XML, YAML, CSV, TOML parsing built in |
| Package management | External APIs resolve from OpenAPI/Swagger specs automatically |

## What It Looks Like

```
::checkout
  $req.body | as data
  | api.stripe.charge .amount .user
  | retry 3 delay 1s
  | fail -> log.err | respond 500 "payment failed"
  | log "charge complete" | respond 200

::main
  http.listen 8080
  | route post "/checkout" -> ::checkout
```

That entire checkout service is ~30 tokens for an AI to generate. The Python equivalent is 150+.

## Key Features

- **Token-minimal syntax** — No commas, no semicolons, no imports, no type keywords
- **Sigil type system** — `$` scalar, `@` array, `%` dict, `!` error, `~` regex, `*` shared
- **Pipe-based data flow** — `$data | transform | output`
- **Parallel by default** — Everything concurrent, use `sequential` to opt out
- **Arena-scoped memory** — No garbage collector, no borrow checker, zero overhead
- **Native data parsing** — `$raw | as data` handles JSON, XML, YAML, CSV, TOML
- **170 built-in functions** — Filesystem, HTTP, database, crypto, scheduling — no packages needed
- **Two comment types** — `#` prompts (human directs AI, AI deletes after acting) and `##` docs (permanent)
- **Block-level compilation** — Change one block, sub-second rebuild

## Quick Start

```
## hello.torq
::main
  print "hello world"
```

```bash
torq run hello.torq
```

## Project Structure

```
project/
  torq.yaml          # config, external services, secrets
  src/
    main.torq        # entry point
    *.torq           # organize however you want
  torq_modules/      # auto-generated (gitignored)
```

## Documentation

- [Language Specification](docs/TORQ-LANGUAGE-SPEC.md) — Complete language reference
- [AI System Prompt](docs/TORQ-AI-PROMPT.md) — Paste into any AI to enable TORQ generation
- [Examples](examples/) — Working TORQ programs

## Bootstrap Strategy

TORQ works with current AI models today — no retraining needed. The grammar is ~30 rules. Paste the [AI System Prompt](docs/TORQ-AI-PROMPT.md) into any model's context and it can write TORQ immediately.

## Status

TORQ is in early design and specification phase. The language spec is defined. The compiler is next.

## Contributing

TORQ is open source under the MIT License. Contributions welcome.

## License

[MIT](LICENSE)
