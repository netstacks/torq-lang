# TORQ Compiler — Build Progress

Last updated: 2026-02-22

## Status Summary

| Phase | Status | Tests |
|-------|--------|-------|
| Phase 1: Lexer + Parser | COMPLETE | 27 lexer + 14 parser |
| Phase 2: Semantic Analysis | COMPLETE | 11 tests |
| Phase 3: Basic Codegen | COMPLETE | Cranelift + C runtime |
| Phase 4: Control Flow | COMPLETE | Blocks, recursion, loop/break, match (int+wildcard), each sequential |
| Phase 5: Runtime Stdlib Core | COMPLETE | 64 codegen + 48 lexer-extra + 42 parser-extra + 44 semantic-extra |
| Phase 5.5: Match Pattern Expansion | COMPLETE | 8 codegen tests added |
| Phase 6: Parallelism | COMPLETE | 3 codegen tests added |
| Phase 7: AI Integration | COMPLETE | 9 tests (torqc-ai crate) |
| Phase 8: Runtime Stdlib Full | COMPLETE | 7 codegen tests added |
| Phase 9: Test Framework | COMPLETE | CLI `torqc test` command |
| Phase 10: Polish | COMPLETE | `torqc init`, `torqc run`, `--version` |

**Total tests passing: 284** (all green, 0 failures)

---

## Phase 5.5: Match Pattern Expansion — COMPLETE

### What was done
- Expanded `match` codegen from only int literals + wildcards to full pattern support
- All literal types: string, bool, float, null
- Comparison patterns: `> 10`, `< 5`, `>= 0`, `<= 100`, `== "yes"`, `!= 0`
- Variable binding: capture matched value into a named variable

### Files modified
- `compiler/crates/torqc-codegen/src/codegen.rs` — unified `Pattern::Literal(lit)` handler, added `Pattern::Comparison` and `Pattern::Variable`
- `compiler/crates/torqc-codegen/tests/codegen_tests.rs` — 8 new integration tests

---

## Phase 6: Parallelism — COMPLETE

### What was done
- pthreads-based parallel `each` for arrays and ranges
- `*shared` variable support with mutex-protected read/write/add
- Loop body compiled as separate Cranelift function, function pointer passed to runtime
- Sequential `each` still works as before (opt-out with `sequential` keyword)

### Files modified
- `compiler/crates/torqc-codegen/runtime/torq_runtime.c` — `torq_parallel_each_array`, `torq_parallel_each_range`, `TorqShared` struct, `torq_shared_new/read/write/add`
- `compiler/crates/torqc-codegen/src/codegen.rs` — parallel each body compilation, `each_body_counter`, shared variable codegen
- `compiler/crates/torqc-codegen/tests/codegen_tests.rs` — 3 new tests

---

## Phase 7: AI Integration — COMPLETE

### What was done
- New `torqc-ai` crate with provider trait pattern
- Providers: Anthropic, OpenAI, Local (Ollama), Custom endpoint, Mock (for CI)
- Fixer: retry loop up to `max_retries` with recompilation verification
- Config: `~/.torqc/config.yaml` with project-level `torq.yaml` overrides
- CLI flags: `--ai`, `--no-ai`, `--verbose` on `build` command
- `torqc run <file>` command (compile + execute + cleanup)

### Files modified
- `compiler/crates/torqc-ai/` — new crate (config.rs, provider.rs, fixer.rs, tests.rs)
- `compiler/crates/torqc-cli/src/main.rs` — AI flags, `run` command
- `compiler/Cargo.toml` — workspace member added

---

## Phase 8: Runtime Stdlib Full — COMPLETE

### What was done
- Time: `time_now`, `time_unix`, `time_format`, `time_parse`, `time_diff`, `time_add`, `time_sleep`
- HTTP client: `http_get`, `http_post`, `http_put`, `http_delete` (via curl subprocess)
- Crypto: `crypto_hash` (SHA-256 pure C), `crypto_uuid` (v4 via /dev/urandom)
- Logging: `log_info`, `log_warn`, `log_err`, `log_debug`
- Math: `math_random`
- String extras: `str_repeat`, `str_pad_left`, `str_pad_right`
- Array extras: `array_zip`, `array_index_of`, `array_reduce`
- Testing: `assert`, `assert_eq`

### Files modified
- `compiler/crates/torqc-codegen/runtime/torq_runtime.c` — 21 new runtime functions
- `compiler/crates/torqc-codegen/src/codegen.rs` — RuntimeFuncs fields, declarations, pipeline dispatch, expression calls
- `compiler/crates/torqc-codegen/tests/codegen_tests.rs` — 7 new tests

---

## Phase 9: Test Framework — COMPLETE

### What was done
- `torqc test <file>` CLI command
- Discovers `::test_` prefixed blocks, compiles each as standalone binary
- Reports pass/fail per test with timing
- `--filter` (`-f`) flag for running subset of tests
- Exit code 1 on any failure
- Example test files: `examples/test_math.torq`, `examples/test_fail.torq`

### Files modified
- `compiler/crates/torqc-cli/src/main.rs` — `run_test()` function, `test` subcommand

---

## Phase 10: Polish — COMPLETE

### What was done
- `torqc run <file>` — compile to temp file, execute, cleanup (implemented in Phase 7)
- `torqc init [name]` — project scaffolding (torq.yaml, src/main.torq, .gitignore)
- `--version` flag — shows `torqc 0.1.0`

### Files modified
- `compiler/crates/torqc-cli/src/main.rs` — `run_init()` function, `init` subcommand, version flag

---

## Compiler Commands Reference

```
torqc parse <file>     — Parse .torq file, print AST as JSON
torqc check <file>     — Semantic analysis (5 validation passes)
torqc build <file>     — Compile to native binary
torqc run <file>       — Compile + execute immediately
torqc test <file>      — Run ::test_ blocks with pass/fail reporting
torqc init [name]      — Scaffold new TORQ project
torqc --version        — Show version
```

### Build flags
- `--ai` / `--no-ai` — Enable/disable AI error recovery
- `--verbose` — Show detailed compilation info
- `-f` / `--filter` — Filter test names (for `torqc test`)

---

## Crate Breakdown (7 crates)

| Crate | Purpose | Tests |
|-------|---------|-------|
| torqc-ast | AST type definitions | 0 (shared types) |
| torqc-lexer | Tokenizer (logos + indentation) | 75 (27 + 48) |
| torqc-parser | Recursive descent parser | 56 (14 + 42) |
| torqc-semantic | 5 validation passes | 55 (11 + 44) |
| torqc-codegen | Cranelift + C runtime codegen | 89 (7 unit + 82 integration) |
| torqc-ai | LLM error recovery | 9 |
| torqc-cli | CLI entry point | 0 (tested via integration) |
| **Total** | | **284** |

---

## Session Recovery Notes

If context is lost, start here:
1. Read this file for current status
2. Run `cargo test` from `compiler/` to verify baseline (expect 284 passing)
3. Check `git log --oneline -10` for recent commits
4. All 10 phases are COMPLETE
5. Future work: HTTP server, database connectors, scheduling, incremental compilation, OpenAPI service spec codegen
