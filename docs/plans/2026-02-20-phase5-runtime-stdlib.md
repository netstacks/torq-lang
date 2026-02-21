# Phase 5: Runtime Stdlib Core Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add dynamic value system with arrays, dicts, string operations, math, I/O, and JSON — enabling TORQ programs that create/transform collections, manipulate strings, and read/write files.

**Architecture:** Replace the compile-time typed approach (TorqType I64/Ptr/Bool) with a unified runtime value system. All values become pointers to heap-allocated `TorqValue` structs with type tags. A C runtime library (~1000 LOC) handles value creation, arithmetic, comparison, collections, string ops, and I/O. Cranelift codegen simplifies to emitting function calls — no more type-specific instruction selection.

**Tech Stack:** Cranelift 0.128, C runtime library (embedded via `include_str!`), Rust codegen

**Target program after Phase 5:**
```
::main
  @names = ["alice" "bob" "charlie"]
  @names | len | print
  @names | first | upper | print

  %user = { name "alice" age 30 }
  %user.name | print
  %user | to json | print

  "hello world" | upper | print
  "  spaces  " | trim | print

  math.sqrt 144 | print
  sys.fs.read "test.txt" | print
```

---

## Context

**Current state (Phase 4):** Codegen uses compile-time type tracking (`TorqType` enum) with type-specific Cranelift instructions: `iadd` for ints, `icmp` for comparisons, separate `torq_print_int`/`torq_print_str`/etc. Works for scalar-only programs (fibonacci). Cannot represent arrays, dicts, or mixed-type operations.

**Why the refactor:** TORQ is dynamically typed. Arrays hold mixed types (`[1 "two" true]`), dict values can be any type, and pipeline stages need runtime type dispatch. A unified `TorqValue*` representation is the only clean way to support this.

**Key design decisions:**
- All values are `TorqValue*` (pointer-sized i64 in Cranelift). No compile-time type tracking.
- The C runtime handles all type checking, arithmetic, and operations.
- Memory uses simple `malloc`/`free` for now. Arena allocation deferred to Phase 6.
- The C runtime source is stored at `compiler/crates/torqc-codegen/runtime/torq_runtime.c` and embedded via `include_str!`.
- Dict implementation uses linear scan (simple, correct, fast enough for small dicts).

---

### Task 1: TorqValue C Runtime Foundation

**Goal:** Build the core value system in C. Structs, constructors, type checking, extraction, print, and memory management.

**Files:**
- Create: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- Modify: `compiler/crates/torqc-codegen/src/runtime.rs` — change to `include_str!`

**Step 1: Create the C runtime file**

Create `compiler/crates/torqc-codegen/runtime/torq_runtime.c` with:

```c
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>

// ===== Type system =====

typedef enum {
    TV_NULL = 0,
    TV_INT,
    TV_FLOAT,
    TV_BOOL,
    TV_STR,
    TV_ARRAY,
    TV_DICT
} TorqTypeTag;

typedef struct TorqValue TorqValue;
typedef struct TorqArray TorqArray;
typedef struct TorqDict TorqDict;

struct TorqArray {
    int64_t capacity;
    int64_t length;
    TorqValue** elements;
};

typedef struct {
    char* key;
    TorqValue* value;
} TorqDictEntry;

struct TorqDict {
    int64_t capacity;
    int64_t length;
    TorqDictEntry* entries;
};

struct TorqValue {
    TorqTypeTag type;
    union {
        int64_t integer;
        double floating;
        int64_t boolean;
        char* string;
        TorqArray* array;
        TorqDict* dict;
    };
};

// ===== Constructors =====

TorqValue* torq_int(int64_t n) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_INT;
    v->integer = n;
    return v;
}

TorqValue* torq_float(double f) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_FLOAT;
    v->floating = f;
    return v;
}

TorqValue* torq_bool(int64_t b) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_BOOL;
    v->boolean = b;
    return v;
}

TorqValue* torq_str(const char* s) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_STR;
    v->string = strdup(s);
    return v;
}

TorqValue* torq_null(void) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_NULL;
    v->integer = 0;
    return v;
}

// ===== Type checking =====

int64_t torq_is_truthy(TorqValue* v) {
    if (!v) return 0;
    switch (v->type) {
        case TV_NULL: return 0;
        case TV_INT: return v->integer != 0;
        case TV_FLOAT: return v->floating != 0.0;
        case TV_BOOL: return v->boolean;
        case TV_STR: return v->string && v->string[0] != '\0';
        case TV_ARRAY: return v->array && v->array->length > 0;
        case TV_DICT: return v->dict && v->dict->length > 0;
    }
    return 0;
}

// ===== Extraction =====

int64_t torq_as_int(TorqValue* v) {
    if (!v) return 0;
    switch (v->type) {
        case TV_INT: return v->integer;
        case TV_FLOAT: return (int64_t)v->floating;
        case TV_BOOL: return v->boolean;
        default: return 0;
    }
}

// ===== Print =====

// Forward declarations for collection printing
static void torq_print_array_value(TorqValue* v);
static void torq_print_dict_value(TorqValue* v);
static void torq_fprint_value(FILE* f, TorqValue* v);

void torq_print(TorqValue* v) {
    if (!v) { puts("null"); return; }
    switch (v->type) {
        case TV_NULL:  puts("null"); break;
        case TV_INT:   printf("%lld\n", (long long)v->integer); break;
        case TV_FLOAT: printf("%g\n", v->floating); break;
        case TV_BOOL:  puts(v->boolean ? "true" : "false"); break;
        case TV_STR:   puts(v->string); break;
        case TV_ARRAY: torq_print_array_value(v); putchar('\n'); break;
        case TV_DICT:  torq_print_dict_value(v); putchar('\n'); break;
    }
}
```

Note: `torq_print_array_value` and `torq_print_dict_value` will be implemented in Tasks 4 and 5 respectively. For now they can be stubs that print `[array]` and `{dict}`.

**Step 2: Update runtime.rs to use include_str!**

```rust
// compiler/crates/torqc-codegen/src/runtime.rs
pub const RUNTIME_C_SOURCE: &str = include_str!("../runtime/torq_runtime.c");
```

**Step 3: Verify existing tests still compile and link**

The old print functions (`torq_print_int`, etc.) still need to exist in the runtime during the transition. Add them as wrappers:

```c
// Legacy compatibility — remove after Task 3 codegen refactor
void torq_print_int(int64_t n) { printf("%lld\n", (long long)n); }
void torq_print_str(const char* s) { puts(s); }
void torq_print_bool(int64_t b) { puts(b ? "true" : "false"); }
void torq_print_float(double f) { printf("%g\n", f); }
void torq_print_null(void) { puts("null"); }
```

**Step 4: Run all tests**

Run: `cargo test -p torqc-codegen`
Expected: All 36 tests pass (the legacy functions are preserved).

**Step 5: Commit**

```bash
git commit -m "feat(runtime): TorqValue system with tagged union and constructors

Move C runtime to standalone file at runtime/torq_runtime.c, embedded
via include_str!. Define TorqValue struct with type tags (null, int,
float, bool, str, array, dict). Add constructors, type checking,
extraction, and universal print."
```

---

### Task 2: Runtime Arithmetic and Comparison

**Goal:** Add runtime functions for arithmetic and comparison that work on TorqValue pointers.

**Files:**
- Modify: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`

**Add to the C runtime:**

```c
// ===== Arithmetic =====

TorqValue* torq_add(TorqValue* a, TorqValue* b) {
    if (a->type == TV_INT && b->type == TV_INT)
        return torq_int(a->integer + b->integer);
    if (a->type == TV_FLOAT || b->type == TV_FLOAT) {
        double fa = (a->type == TV_FLOAT) ? a->floating : (double)a->integer;
        double fb = (b->type == TV_FLOAT) ? b->floating : (double)b->integer;
        return torq_float(fa + fb);
    }
    if (a->type == TV_STR && b->type == TV_STR) {
        // String concatenation
        size_t la = strlen(a->string), lb = strlen(b->string);
        char* s = (char*)malloc(la + lb + 1);
        memcpy(s, a->string, la);
        memcpy(s + la, b->string, lb + 1);
        TorqValue* v = torq_str(s);
        free(s);
        return v;
    }
    return torq_int(0); // fallback
}

TorqValue* torq_sub(TorqValue* a, TorqValue* b) { /* int/float sub */ }
TorqValue* torq_mul(TorqValue* a, TorqValue* b) { /* int/float mul */ }
TorqValue* torq_div(TorqValue* a, TorqValue* b) { /* int/float div, check zero */ }
TorqValue* torq_mod(TorqValue* a, TorqValue* b) { /* int mod */ }

// ===== Comparison =====

TorqValue* torq_eq(TorqValue* a, TorqValue* b) {
    if (a->type != b->type) return torq_bool(0);
    switch (a->type) {
        case TV_INT: return torq_bool(a->integer == b->integer);
        case TV_FLOAT: return torq_bool(a->floating == b->floating);
        case TV_BOOL: return torq_bool(a->boolean == b->boolean);
        case TV_STR: return torq_bool(strcmp(a->string, b->string) == 0);
        case TV_NULL: return torq_bool(1);
        default: return torq_bool(0);
    }
}

TorqValue* torq_neq(TorqValue* a, TorqValue* b) { /* !eq */ }
TorqValue* torq_gt(TorqValue* a, TorqValue* b)  { /* int/float > */ }
TorqValue* torq_lt(TorqValue* a, TorqValue* b)  { /* int/float < */ }
TorqValue* torq_gte(TorqValue* a, TorqValue* b) { /* int/float >= */ }
TorqValue* torq_lte(TorqValue* a, TorqValue* b) { /* int/float <= */ }
```

**Step 1: Implement all arithmetic and comparison functions**

**Step 2: Run tests (still using legacy functions, should pass)**

Run: `cargo test -p torqc-codegen`
Expected: All 36 tests pass.

**Step 3: Commit**

---

### Task 3: Codegen Refactor to TorqValue Pointers

**Goal:** This is the critical refactor. ALL codegen switches from native Cranelift types to TorqValue pointers. Every expression produces a `TorqValue*` (i64 pointer). All 222 existing tests must pass after this change.

**Files:**
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs` (major refactor)

**What changes:**

1. **Remove `TorqType` enum** — no longer needed. All values are pointers.

2. **RuntimeFuncs** — declare ALL runtime functions:
   - Constructors: `torq_int(i64)->ptr`, `torq_float(f64)->ptr`, `torq_bool(i64)->ptr`, `torq_str(ptr)->ptr`, `torq_null()->ptr`
   - Print: `torq_print(ptr)->void`
   - Arithmetic: `torq_add(ptr,ptr)->ptr`, `torq_sub(ptr,ptr)->ptr`, `torq_mul(ptr,ptr)->ptr`, `torq_div(ptr,ptr)->ptr`, `torq_mod(ptr,ptr)->ptr`
   - Comparison: `torq_eq(ptr,ptr)->ptr`, `torq_neq(ptr,ptr)->ptr`, `torq_gt(ptr,ptr)->ptr`, `torq_lt(ptr,ptr)->ptr`, `torq_gte(ptr,ptr)->ptr`, `torq_lte(ptr,ptr)->ptr`
   - Truthiness: `torq_is_truthy(ptr)->i64` (returns raw i64 for branching)
   - Extraction: `torq_as_int(ptr)->i64` (for main's return value)

3. **Literal compilation:**
   - `Literal::Int(n)` → `call torq_int(iconst n)` → returns ptr
   - `Literal::String(s)` → create data section for string, `call torq_str(global_value)` → returns ptr
   - `Literal::Bool(b)` → `call torq_bool(iconst 0/1)` → returns ptr
   - `Literal::Null` → `call torq_null()` → returns ptr
   - `Literal::Float(f)` → `call torq_float(f64const f)` → returns ptr

4. **BinOp compilation:**
   - `Add` → `call torq_add(left, right)` → returns ptr
   - `Sub` → `call torq_sub(left, right)` → returns ptr
   - `Gt` → `call torq_gt(left, right)` → returns ptr (TorqValue* bool)
   - etc.

5. **Print:**
   - Single `torq_print(value)` call. No more type dispatch in codegen.

6. **Match:**
   - Compare: `call torq_eq(subject, pattern_value)` → returns TorqValue* bool
   - Branch: `call torq_is_truthy(eq_result)` → returns i64, then `brif`

7. **Block return values:**
   - Non-main blocks return `TorqValue*` (i64 pointer)
   - `::main` returns `i32 0` (unchanged). If main's body has no explicit return, just return 0.

8. **Variables:**
   - Cranelift Variables hold pointers (i64). All variables are the same type.
   - `declare_var(types::I64)` for all variables.

9. **Each sequential + range:**
   - The counter is a raw i64 (not boxed) for efficiency
   - At each iteration, box the counter: `call torq_int(counter)` → bind to variable
   - The range comparison uses raw i64 icmp (counter >= end), NOT boxed comparison

10. **compile_expr return type:**
    - Changes from `Result<(Value, TorqType), CodegenError>` to `Result<Value, CodegenError>`
    - All Values are i64 (pointers to TorqValue)

**Step 1: Declare all runtime functions in `declare_runtime_funcs()`**

Extend `RuntimeFuncs` struct to hold FuncIds for all constructor, arithmetic, comparison, and utility functions.

**Step 2: Refactor compile_expr**

Change signature to `fn compile_expr(&mut self, expr: &Expr, rt: &RuntimeFuncs, builder: &mut FunctionBuilder, pipe_value: Option<Value>) -> Result<Value, CodegenError>`

Implement each expression variant using runtime function calls.

**Step 3: Refactor compile_statement and compile_pipeline**

Update to use the new `Option<Value>` (no TorqType).

**Step 4: Refactor match**

Match subject comparison: `call torq_eq(subject, lit_value)` then `call torq_is_truthy(eq_result)` then `brif`.

**Step 5: Refactor each sequential**

Keep counter as raw i64 (not boxed). Box only when binding to the loop variable.

**Step 6: Update unit tests in codegen.rs**

The unit tests that construct AST directly need to compile correctly. Output must be identical.

**Step 7: Run ALL tests**

Run: `cargo test` (full workspace)
Expected: All 222 tests pass. Output is identical to Phase 4.

**Step 8: Remove legacy print functions from runtime**

Delete `torq_print_int`, `torq_print_str`, `torq_print_bool`, `torq_print_float`, `torq_print_null` from the C runtime since they're no longer used.

**Step 9: Commit**

```bash
git commit -m "refactor(codegen): all values are TorqValue pointers

Replace compile-time TorqType dispatch with runtime TorqValue system.
All expressions produce TorqValue* pointers. Arithmetic, comparison,
and print dispatch to C runtime functions. All 222 tests pass."
```

---

### Task 4: Array Runtime + Codegen

**Goal:** Create and print arrays. Array operations: len, first, last, push.

**Files:**
- Modify: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Implement array functions in C runtime**

```c
TorqValue* torq_array_new(void) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_ARRAY;
    v->array = (TorqArray*)malloc(sizeof(TorqArray));
    v->array->capacity = 8;
    v->array->length = 0;
    v->array->elements = (TorqValue**)calloc(8, sizeof(TorqValue*));
    return v;
}

void torq_array_push_mut(TorqValue* arr, TorqValue* val) {
    // Push val onto arr (mutates arr)
    TorqArray* a = arr->array;
    if (a->length >= a->capacity) {
        a->capacity *= 2;
        a->elements = (TorqValue**)realloc(a->elements, a->capacity * sizeof(TorqValue*));
    }
    a->elements[a->length++] = val;
}

TorqValue* torq_array_len(TorqValue* arr) {
    if (arr->type != TV_ARRAY) return torq_int(0);
    return torq_int(arr->array->length);
}

TorqValue* torq_array_first(TorqValue* arr) {
    if (arr->type != TV_ARRAY || arr->array->length == 0) return torq_null();
    return arr->array->elements[0];
}

TorqValue* torq_array_last(TorqValue* arr) {
    if (arr->type != TV_ARRAY || arr->array->length == 0) return torq_null();
    return arr->array->elements[arr->array->length - 1];
}

TorqValue* torq_array_get(TorqValue* arr, TorqValue* index) {
    if (arr->type != TV_ARRAY) return torq_null();
    int64_t i = torq_as_int(index);
    if (i < 0 || i >= arr->array->length) return torq_null();
    return arr->array->elements[i];
}
```

Also implement `torq_print_array_value` for printing arrays like `[1, "two", true]`.

**Step 2: Add array codegen**

In `compile_expr`, handle `Expr::Array(elements, _span)`:
```rust
Expr::Array(elements, _) => {
    // Create new array
    let arr = call torq_array_new();
    // Push each element
    for elem in elements {
        let val = self.compile_expr(elem, rt, builder, None)?;
        call torq_array_push_mut(arr, val);
    }
    Ok(arr)
}
```

Declare `torq_array_new`, `torq_array_push_mut`, `torq_array_len`, `torq_array_first`, `torq_array_last`, `torq_array_get` in RuntimeFuncs.

**Step 3: Add stdlib dispatch for array operations**

In pipeline compilation, when a Call like `len`, `first`, `last` is encountered with a pipe value, dispatch to the runtime:
- `| len` → `call torq_array_len(pipe_val)` (also works for strings — add torq_str_len later)
- `| first` → `call torq_array_first(pipe_val)`
- `| last` → `call torq_array_last(pipe_val)`

**Step 4: Write tests**

```rust
#[test]
fn array_create_and_print() {
    let output = compile_and_run("::main\n  @nums = [1 2 3]\n  @nums | print\n");
    assert_eq!(output.trim(), "[1, 2, 3]");
}

#[test]
fn array_len() {
    let output = compile_and_run("::main\n  @names = [\"a\" \"b\" \"c\"]\n  @names | len | print\n");
    assert_eq!(output.trim(), "3");
}

#[test]
fn array_first_last() {
    let output = compile_and_run("::main\n  @nums = [10 20 30]\n  @nums | first | print\n  @nums | last | print\n");
    assert_eq!(output.trim(), "10\n30");
}
```

**Step 5: Run all tests, commit**

---

### Task 5: Dict Runtime + Codegen + MemberAccess

**Goal:** Create and print dicts. Dict field access via MemberAccess (`.field`).

**Files:**
- Modify: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Implement dict functions in C runtime**

```c
TorqValue* torq_dict_new(void) {
    TorqValue* v = (TorqValue*)malloc(sizeof(TorqValue));
    v->type = TV_DICT;
    v->dict = (TorqDict*)malloc(sizeof(TorqDict));
    v->dict->capacity = 16;
    v->dict->length = 0;
    v->dict->entries = (TorqDictEntry*)calloc(16, sizeof(TorqDictEntry));
    return v;
}

void torq_dict_set_mut(TorqValue* d, const char* key, TorqValue* val) {
    TorqDict* dict = d->dict;
    // Check if key exists
    for (int64_t i = 0; i < dict->length; i++) {
        if (strcmp(dict->entries[i].key, key) == 0) {
            dict->entries[i].value = val;
            return;
        }
    }
    // Add new entry
    if (dict->length >= dict->capacity) {
        dict->capacity *= 2;
        dict->entries = (TorqDictEntry*)realloc(dict->entries, dict->capacity * sizeof(TorqDictEntry));
    }
    dict->entries[dict->length].key = strdup(key);
    dict->entries[dict->length].value = val;
    dict->length++;
}

TorqValue* torq_dict_get(TorqValue* d, const char* key) {
    if (d->type != TV_DICT) return torq_null();
    TorqDict* dict = d->dict;
    for (int64_t i = 0; i < dict->length; i++) {
        if (strcmp(dict->entries[i].key, key) == 0) {
            return dict->entries[i].value;
        }
    }
    return torq_null();
}
```

Also implement `torq_dict_has`, `torq_dict_keys`, `torq_dict_values`, `torq_dict_len`.
Implement `torq_print_dict_value` for printing dicts like `{name: "alice", age: 30}`.

**Step 2: Add dict codegen**

Handle `Expr::Dict(entries, _span)`:
```rust
Expr::Dict(entries, _) => {
    let dict = call torq_dict_new();
    for entry in entries {
        let key_data = create_string_data(&entry.key);  // null-terminated key
        let key_ptr = global_value(key_data);
        let val = self.compile_expr(&entry.value, ...)?;
        call torq_dict_set_mut(dict, key_ptr, val);
    }
    Ok(dict)
}
```

**Step 3: Add MemberAccess codegen**

Handle `Expr::MemberAccess(access)`:
```rust
Expr::MemberAccess(access) => {
    let obj = self.compile_expr(&access.object, ...)?;
    let key_data = create_string_data(&access.field);
    let key_ptr = global_value(key_data);
    let result = call torq_dict_get(obj, key_ptr);
    Ok(result)
}
```

**Step 4: Write tests**

```rust
#[test]
fn dict_create_and_print() {
    let src = "::main\n  %user = { name \"alice\" age 30 }\n  %user | print\n";
    let output = compile_and_run(src);
    assert!(output.contains("name") && output.contains("alice") && output.contains("age") && output.contains("30"));
}

#[test]
fn dict_member_access() {
    let src = "::main\n  %user = { name \"alice\" age 30 }\n  %user.name | print\n  %user.age | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "alice\n30");
}

#[test]
fn dict_len() {
    let src = "::main\n  %d = { a 1 b 2 c 3 }\n  %d | len | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "3");
}
```

**Step 5: Run all tests, commit**

---

### Task 6: String Operations

**Goal:** Pipeline-based string operations: upper, lower, trim, len, split, join, contains, replace, starts, ends.

**Files:**
- Modify: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Implement string functions in C runtime**

```c
TorqValue* torq_str_upper(TorqValue* v) {
    if (v->type != TV_STR) return v;
    char* s = strdup(v->string);
    for (char* p = s; *p; p++) *p = toupper((unsigned char)*p);
    TorqValue* result = torq_str(s);
    free(s);
    return result;
}

TorqValue* torq_str_lower(TorqValue* v) { /* similar */ }
TorqValue* torq_str_trim(TorqValue* v) { /* strip leading/trailing whitespace */ }
TorqValue* torq_str_len(TorqValue* v) { /* return torq_int(strlen(v->string)) */ }
TorqValue* torq_str_contains(TorqValue* v, TorqValue* substr) { /* strstr */ }
TorqValue* torq_str_replace(TorqValue* v, TorqValue* old, TorqValue* new_) { /* replace first */ }
TorqValue* torq_str_replace_all(TorqValue* v, TorqValue* old, TorqValue* new_) { /* replace all */ }
TorqValue* torq_str_split(TorqValue* v, TorqValue* delim) { /* returns array of strings */ }
TorqValue* torq_str_starts(TorqValue* v, TorqValue* prefix) { /* strncmp */ }
TorqValue* torq_str_ends(TorqValue* v, TorqValue* suffix) { /* compare tail */ }
TorqValue* torq_str_slice(TorqValue* v, TorqValue* start, TorqValue* end_) { /* substring */ }
TorqValue* torq_str_reverse(TorqValue* v) { /* reverse string */ }
```

Also add `torq_join(TorqValue* arr, TorqValue* delim)` — joins an array of strings.

**Step 2: Add stdlib dispatch in codegen**

In `compile_pipeline` and `compile_expr`, when encountering `Call` expressions for known stdlib names, dispatch to the corresponding runtime function:

```rust
// In compile_expr or a new compile_call method:
match call.name.as_str() {
    "upper" => call torq_str_upper(pipe_value or first arg),
    "lower" => call torq_str_lower(...),
    "trim"  => call torq_str_trim(...),
    "len"   => {
        // len works on strings AND arrays
        // The runtime can check the type
        call torq_len(pipe_value)  // unified len function
    },
    "split"    => call torq_str_split(pipe, arg0),
    "join"     => call torq_join(pipe, arg0),
    "contains" => call torq_str_contains(pipe, arg0),
    "replace"  => call torq_str_replace(pipe, arg0, arg1),
    "starts"   => call torq_str_starts(pipe, arg0),
    "ends"     => call torq_str_ends(pipe, arg0),
    // ... etc
}
```

Add a unified `torq_len(TorqValue*)` that handles strings (strlen), arrays (length), and dicts (length).

**Step 3: Write tests**

```rust
#[test]
fn string_upper() {
    let output = compile_and_run("::main\n  \"hello world\" | upper | print\n");
    assert_eq!(output.trim(), "HELLO WORLD");
}

#[test]
fn string_lower() {
    let output = compile_and_run("::main\n  \"HELLO\" | lower | print\n");
    assert_eq!(output.trim(), "hello");
}

#[test]
fn string_trim() {
    let output = compile_and_run("::main\n  \"  hello  \" | trim | print\n");
    assert_eq!(output.trim(), "hello");
}

#[test]
fn string_len() {
    let output = compile_and_run("::main\n  \"hello\" | len | print\n");
    assert_eq!(output.trim(), "5");
}

#[test]
fn string_contains() {
    let output = compile_and_run("::main\n  \"hello world\" | contains \"world\" | print\n");
    assert_eq!(output.trim(), "true");
}

#[test]
fn string_split_and_join() {
    let output = compile_and_run("::main\n  \"a,b,c\" | split \",\" | join \"-\" | print\n");
    assert_eq!(output.trim(), "a-b-c");
}
```

**Step 4: Run all tests, commit**

---

### Task 7: StringInterp Codegen

**Goal:** Compile string interpolation: `"hello $name, age $age"`.

**Files:**
- Modify: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Add string building functions to runtime**

```c
TorqValue* torq_str_concat(TorqValue* a, TorqValue* b) {
    // Convert both to string representation, concatenate
    // torq_to_string(v) returns a string representation of any value
}

TorqValue* torq_to_string(TorqValue* v) {
    // Convert any TorqValue to its string representation
    // Int → "42", Bool → "true", String → itself, etc.
}
```

**Step 2: Implement StringInterp codegen**

Handle `Expr::StringInterp(parts, _span)`:
```rust
Expr::StringInterp(parts, _) => {
    // Build string by concatenating parts
    let mut result = None;
    for part in parts {
        let part_val = match part {
            StringPart::Literal(s) => call torq_str(data_section(s)),
            StringPart::Interpolation(var) => {
                let var_val = compile_expr(Expr::Variable(var), ...)?;
                call torq_to_string(var_val)
            }
        };
        result = match result {
            None => Some(part_val),
            Some(prev) => Some(call torq_str_concat(prev, part_val)),
        };
    }
    Ok(result.unwrap_or_else(|| call torq_str(empty_string)))
}
```

**Step 3: Write tests**

```rust
#[test]
fn string_interpolation() {
    let src = "::main\n  \"alice\" -> $name\n  \"hello $name\" | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "hello alice");
}

#[test]
fn string_interpolation_with_int() {
    let src = "::main\n  42 -> $age\n  \"age is $age\" | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "age is 42");
}
```

**Step 4: Run all tests, commit**

---

### Task 8: Math Functions

**Goal:** Basic math built-in functions.

**Files:**
- Modify: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Add math functions to runtime**

```c
TorqValue* torq_math_abs(TorqValue* v) {
    if (v->type == TV_INT) return torq_int(llabs(v->integer));
    if (v->type == TV_FLOAT) return torq_float(fabs(v->floating));
    return v;
}

TorqValue* torq_math_sqrt(TorqValue* v) {
    double n = (v->type == TV_FLOAT) ? v->floating : (double)v->integer;
    return torq_float(sqrt(n));
}

TorqValue* torq_math_floor(TorqValue* v) { /* floor → int */ }
TorqValue* torq_math_ceil(TorqValue* v) { /* ceil → int */ }
TorqValue* torq_math_round(TorqValue* v) { /* round → int */ }
TorqValue* torq_math_min(TorqValue* a, TorqValue* b) { /* min of two values */ }
TorqValue* torq_math_max(TorqValue* a, TorqValue* b) { /* max of two values */ }
TorqValue* torq_math_random(void) { /* random float 0..1 */ }
TorqValue* torq_math_random_range(TorqValue* lo, TorqValue* hi) { /* random int in range */ }
```

**Step 2: Dispatch `math.X` calls**

For `Call { name: "math.sqrt", args: [v] }`, dispatch to `torq_math_sqrt`.
The `.` in the name is part of the function name string from the parser.

**Step 3: Write tests**

```rust
#[test]
fn math_sqrt() {
    let output = compile_and_run("::main\n  math.sqrt 144 | print\n");
    assert_eq!(output.trim(), "12");
}

#[test]
fn math_abs() {
    let output = compile_and_run("::main\n  math.abs -42 | print\n");
    // Note: parser may parse this as math.abs with arg -42, or as math.abs (42) with unary minus
    // Adjust test based on actual parsing
}
```

**Step 4: Run all tests, commit**

---

### Task 9: I/O Functions

**Goal:** File I/O (sys.fs.read, sys.fs.write), logging (log), sys.env, sys.args, sys.exit.

**Files:**
- Modify: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Add I/O functions to runtime**

```c
TorqValue* torq_fs_read(TorqValue* path) {
    if (path->type != TV_STR) return torq_null();
    FILE* f = fopen(path->string, "r");
    if (!f) return torq_null();
    fseek(f, 0, SEEK_END);
    long len = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc(len + 1);
    fread(buf, 1, len, f);
    buf[len] = '\0';
    fclose(f);
    TorqValue* result = torq_str(buf);
    free(buf);
    return result;
}

void torq_fs_write(TorqValue* path, TorqValue* content) {
    if (path->type != TV_STR || content->type != TV_STR) return;
    FILE* f = fopen(path->string, "w");
    if (f) { fputs(content->string, f); fclose(f); }
}

TorqValue* torq_env(TorqValue* name) {
    if (name->type != TV_STR) return torq_null();
    const char* val = getenv(name->string);
    return val ? torq_str(val) : torq_null();
}

void torq_log(TorqValue* msg) {
    // Print to stderr
    torq_fprint_value(stderr, msg);
    fputc('\n', stderr);
}

void torq_exit(TorqValue* code) {
    exit((int)torq_as_int(code));
}
```

**Step 2: Dispatch calls**

Map `sys.fs.read`, `sys.fs.write`, `log`, `sys.env`, `sys.exit` to runtime functions.

**Step 3: Write tests**

```rust
#[test]
fn fs_read_write() {
    let src = r#"::main
  "hello from torq" | sys.fs.write "/tmp/torq_test_io.txt"
  sys.fs.read "/tmp/torq_test_io.txt" | print
"#;
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "hello from torq");
    let _ = std::fs::remove_file("/tmp/torq_test_io.txt");
}

#[test]
fn log_output() {
    // log writes to stderr, not stdout
    // This test verifies the program runs without crashing
    let src = "::main\n  log \"test message\"\n  print \"ok\"\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "ok");
}
```

**Step 4: Run all tests, commit**

---

### Task 10: JSON Serialization

**Goal:** `to json` converts any TorqValue to a JSON string. `as json` parses a JSON string to a TorqValue.

**Files:**
- Modify: `compiler/crates/torqc-codegen/runtime/torq_runtime.c`
- Modify: `compiler/crates/torqc-codegen/src/codegen.rs`
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`

**Step 1: Implement JSON serializer in C runtime**

```c
TorqValue* torq_to_json(TorqValue* v) {
    // Recursive JSON serializer
    // Int → "42", Float → "3.14", Bool → "true"/"false", Null → "null"
    // String → "\"escaped_string\""
    // Array → "[elem1, elem2]"
    // Dict → "{\"key1\": val1, \"key2\": val2}"
    // Returns a TorqValue string
}
```

JSON parser (as json) is complex — implement a minimal one that handles basic JSON, or defer to Phase 8. For Phase 5, `to json` is the priority.

**Step 2: Dispatch `to` and `as` calls**

In pipeline, `| to json` maps to `torq_to_json(pipe_value)`.

**Step 3: Write tests**

```rust
#[test]
fn to_json_dict() {
    let src = "::main\n  %user = { name \"alice\" age 30 }\n  %user | to json | print\n";
    let output = compile_and_run(src);
    // Should be valid JSON
    assert!(output.contains("\"name\"") && output.contains("\"alice\""));
    assert!(output.contains("\"age\"") && output.contains("30"));
}

#[test]
fn to_json_array() {
    let src = "::main\n  @nums = [1 2 3]\n  @nums | to json | print\n";
    let output = compile_and_run(src);
    assert_eq!(output.trim(), "[1, 2, 3]");
}
```

**Step 4: Run all tests, commit**

---

### Task 11: Integration Tests + Cleanup

**Goal:** Comprehensive end-to-end tests, clippy, CLAUDE.md update.

**Files:**
- Modify: `compiler/crates/torqc-codegen/tests/codegen_tests.rs`
- Modify: `CLAUDE.md`

**Step 1: Add comprehensive integration tests**

```rust
#[test]
fn phase5_showcase() {
    let src = r#"::main
  @names = ["alice" "bob" "charlie"]
  @names | len | print
  @names | first | upper | print

  %user = { name "alice" age 30 }
  %user.name | print
  %user.age | print

  "hello world" | upper | print
  "  spaces  " | trim | print
"#;
    let output = compile_and_run(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "3");
    assert_eq!(lines[1], "ALICE");
    assert_eq!(lines[2], "alice");
    assert_eq!(lines[3], "30");
    assert_eq!(lines[4], "HELLO WORLD");
    assert_eq!(lines[5], "spaces");
}
```

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`

**Step 3: Run full workspace tests**

Run: `cargo test`
Count total tests.

**Step 4: Update CLAUDE.md**

Update with Phase 5 capabilities, new test count.

**Step 5: Commit**
