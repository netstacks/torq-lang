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
| Phase 5.5: Match Pattern Expansion | IN PROGRESS | — |
| Phase 6: Parallelism | NOT STARTED | — |
| Phase 7: AI Integration | NOT STARTED | — |
| Phase 8: Runtime Stdlib Full | NOT STARTED | — |
| Phase 9: Test Framework | NOT STARTED | — |
| Phase 10: Polish | NOT STARTED | — |

**Total tests passing: 257** (as of session start, after array_reverse bugfix)

---

## Phase 5.5: Match Pattern Expansion

### Goal
Expand `match` expression codegen beyond int literals and wildcards.

### Patterns to implement
- [x] `Pattern::Literal(Literal::Int)` — already works
- [x] `Pattern::Wildcard` — already works
- [ ] `Pattern::Literal(Literal::String)` — match on string values
- [ ] `Pattern::Literal(Literal::Bool)` — match on true/false
- [ ] `Pattern::Literal(Literal::Float)` — match on float values
- [ ] `Pattern::Comparison` — match `> 10`, `< 5`, `>= 0`, etc.
- [ ] `Pattern::Variable` — bind matched value to a variable

### Files modified
- `compiler/crates/torqc-codegen/src/codegen.rs` — match compilation in `compile_expr`

---

## Phase 6: Parallelism

### Goal
Implement parallel `each` (TORQ's default), `*shared` atomic variables, thread pool.

### Tasks
- [ ] Add `torq_thread_pool_init` / `torq_parallel_each` to C runtime
- [ ] Add `*shared` variable support (atomic read/write) to C runtime
- [ ] Codegen: emit parallel each (spawn threads via runtime)
- [ ] Codegen: `*shared` variable access through atomic runtime functions
- [ ] Tests for parallel execution correctness

### Key files
- `compiler/crates/torqc-codegen/runtime/torq_runtime.c` — thread pool, atomics
- `compiler/crates/torqc-codegen/src/codegen.rs` — parallel each codegen

---

## Phase 7: AI Integration

### Goal
Create `torqc-ai` crate for LLM-based error recovery.

### Tasks
- [ ] Create `compiler/crates/torqc-ai/` crate
- [ ] Define AI provider trait (Anthropic, OpenAI, Local, Custom)
- [ ] Implement error serialization (error + source context + stage)
- [ ] Implement fix loop (up to 3 retries)
- [ ] Add `--ai`, `--no-ai`, `--verbose` flags to CLI
- [ ] Mock LLM responses for CI testing
- [ ] Config: `~/.torqc/config.yaml` and `torq.yaml` override

### Key files
- `compiler/crates/torqc-ai/src/lib.rs` — new crate
- `compiler/crates/torqc-cli/src/main.rs` — CLI flags

---

## Phase 8: Runtime Stdlib Full

### Goal
Add HTTP client/server, database, crypto, time, scheduling to runtime.

### Tasks
- [ ] HTTP client: `http_get`, `http_post`, `http_put`, `http_delete`
- [ ] HTTP server: `http_listen`, `http_route`, `http_respond`
- [ ] Database: `db_connect`, `db_query`, `db_insert`, `db_update`, `db_delete`
- [ ] Crypto: `crypto_hash`, `crypto_hmac`, `crypto_uuid`, `jwt_sign`, `jwt_verify`
- [ ] Time: `time_now`, `time_parse`, `time_diff`, `time_add`, `time_sleep`
- [ ] Scheduling: `schedule_every`, `schedule_at`, `schedule_cron`
- [ ] Extended logging: `log_info`, `log_warn`, `log_err`, `log_debug`

### Key files
- `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- `compiler/crates/torqc-codegen/src/codegen.rs`

---

## Phase 9: Test Framework

### Goal
`torqc test` command that runs `::test_` prefixed blocks.

### Tasks
- [ ] Add `assert` builtin to runtime
- [ ] CLI `torqc test <file>` command
- [ ] Test runner: compile and execute `::test_` blocks
- [ ] Report pass/fail per test block
- [ ] `assert_eq` builtin

### Key files
- `compiler/crates/torqc-cli/src/main.rs`
- `compiler/crates/torqc-codegen/runtime/torq_runtime.c`

---

## Phase 10: Polish

### Goal
Production readiness features.

### Tasks
- [ ] `torqc run <file>` — compile + execute immediately
- [ ] `torq.yaml` config parsing
- [ ] Incremental compilation (block-level caching)
- [ ] OpenAPI service spec codegen to `torq_modules/`
- [ ] Packaging and distribution

### Key files
- `compiler/crates/torqc-cli/src/main.rs`

---

## Session Recovery Notes

If context is lost, start here:
1. Read this file for current status
2. Run `cargo test` from `compiler/` to verify baseline
3. Check git log for recent commits
4. Continue from the first incomplete phase above
