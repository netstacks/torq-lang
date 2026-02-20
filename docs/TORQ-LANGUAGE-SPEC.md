# TORQ Language Specification v0.1.0

## The First Programming Language Designed for AI to Write

---

## 1. Identity & Philosophy

**TORQ** is a token-optimized, AI-authored, human-auditable compiled programming language.

**One-liner:** TORQ is the first general-purpose programming language designed for AI to write, humans to approve, and machines to run fast.

### Core Principles

1. **Token-minimal syntax** — Every design decision minimizes the tokens AI spends generating code. No commas, no semicolons, no brackets where whitespace works. No redundant keywords.

2. **Zero ambiguity** — AI never has to choose between multiple valid ways to express something. One way to loop. One way to branch. One way to handle errors.

3. **Native data fluency** — JSON, XML, YAML, CSV, TOML all parse with `as data`. No imports, no packages for structured data.

4. **Compiles to native** — TORQ compiles to machine code. Target performance: equal to or faster than Go. Arena-based memory, auto-parallelized pipelines, block-level incremental compilation.

5. **Human-auditable** — Code reads left-to-right, top-to-bottom. Pipe syntax makes data flow visible. Humans can learn to read and approve TORQ programs.

6. **AI-independent runtime** — AI writes TORQ. But TORQ runs without AI. The compiled binary is standalone, deterministic, and portable.

7. **Bootstrap on day one** — The grammar is small enough (~50 rules) to fit in a system prompt. AI can write TORQ today from a spec alone, no training required.

8. **Parallel by default** — All operations run concurrently unless explicitly marked sequential. The compiler auto-parallelizes pipe stages and loop iterations.

### File Extension: `.torq`
### Compiler: `torqc`
### CLI: `torq run program.torq`

---

## 2. Sigil Type System

TORQ uses sigil prefixes to denote types. No type keywords. No type declarations. The sigil IS the type.

| Sigil | Type | Mnemonic |
|-------|------|----------|
| `$` | Scalar (string, int, float, bool, null) | Dollar = single value |
| `@` | Array / List | At = "at index" |
| `%` | Dictionary / Map | Percent = key%value pairs |
| `::` | Block definition (function/service) | Double colon = label/scope |
| `&` | Block reference | Ampersand = "address of" |
| `!` | Error | Bang = something broke |
| `~` | Regex pattern | Tilde = pattern matching |
| `*` | Shared / concurrent value | Star = caution, visible |
| `#` | Prompt comment (AI directive) | Human writes, AI reads and deletes after acting |
| `##` | Documentation comment | Stays for human readers |

### Scalars `$`

```
$name = "alice"
$age = 42
$price = 19.99
$active = true
$nothing = null
```

The compiler infers the underlying type (string, int, float, bool, null) from the assigned value. No explicit type annotations ever.

### Strings

```
$greeting = "hello $name, you are $age years old"
$path = "$base/api/$version/users"
$multi = """
  this is a
  multiline string
  with $variables
"""
```

All strings support interpolation by default. No format strings, no concatenation operators needed.

### Arrays `@`

```
@numbers = [1 2 3 4 5]
@names = ["alice" "bob" "charlie"]
@mixed = [1 "two" true 3.14]
```

No commas. Whitespace separates elements.

### Dictionaries `%`

```
%user = {
  name "alice"
  age 30
  role "admin"
  tags @["dev" "lead"]
}
```

No colons. No commas. Key then value.

### Nested Structures

```
%config = {
  server {
    host "0.0.0.0"
    port 8080
  }
  db {
    url "postgres://localhost/app"
    pool 10
  }
}

%config.server.port -> 8080
```

### Errors `!`

```
!err = "not found"
!err = {code 404 msg "user not found"}
```

### Regex `~`

```
~email = "^[a-z]+@[a-z]+\\.[a-z]+$"
$input | match ~email -> true/false
```

### Shared Values `*`

```
*counter = 0
```

The compiler enforces atomic access to shared values.

### Block References `&`

```
&callback = ::process
@handlers = [&::auth &::validate &::process]
```

---

## 3. Blocks

Blocks are the fundamental unit of TORQ. They replace functions, methods, services, handlers, and modules.

```
::block_name
  $input | transform | output
```

### Block Arguments

```
::greet $name $greeting
  "$greeting, $name!" | print

::main
  ::greet "alice" "hello"
```

### Return Values

The last expression in a block is its return value. No `return` keyword.

```
::double $n
  $n * 2

::main
  ::double 21 | print    # prints 42
```

### Cross-File Resolution

Blocks are globally visible across all `.torq` files in a project. No imports.

```
# File: auth.torq
::validate_token $token
  $token | jwt.verify $secret

# File: main.torq
::main
  ::validate_token $req.header "authorization"
```

The compiler finds all `.torq` files and resolves block references automatically.

### Name Collisions

Duplicate block names across files produce a hard compiler error:

```
ERROR: duplicate block "::validate"
  -> auth.torq line 12
  -> payments.torq line 8
  -> rename one of them
```

---

## 4. Pipes

The pipe `|` is the core operator of TORQ. Data flows left to right through transformations.

```
$data | transform_a | transform_b | transform_c | output
```

### Pipe Chaining

```
::main
  file.read "data.csv"
  | as data
  | filter .age > 18
  | sort by .name
  | map .name | upper
  | join ", "
  | print
```

### Pipe to Block

```
$input | ::process | ::validate | ::save
```

### Pipe with Arguments

```
$data | retry 3 delay 1s | timeout 5s
```

### Pipe Branching

```
$result | match
  %success -> ::handle_success
  !err -> ::handle_error
```

---

## 5. Control Flow

TORQ has exactly three control flow constructs: `each`, `loop`, and `match`.

### `each` — Iteration (parallel by default)

```
# Parallel — distributes across all cores
@items | each $item
  $item | process | save

# Sequential — ordered, one at a time
@items | each $item sequential
  $balance = $balance + $item.amount
  update.ledger $balance
```

### Counted Iteration

```
range 1 10 | each $i
  print "number: $i"
```

### Nested Iteration

```
range 1 10 | each $x
  range 1 10 | each $y
    $x * $y | print
```

### `loop` — Conditional / Infinite Loop

```
# Conditional
$count = 0
loop $count < 100
  $count + 1 -> $count
  print $count

# Infinite with break
loop
  io.read stdin
  | match
    "quit" -> break
    $input -> process $input
```

### `match` — Pattern Matching (replaces if/else/switch)

```
# Simple branching
$age | match
  >= 21 -> serve.alcohol
  >= 18 -> serve.tobacco
  _ -> deny "underage"

# Type matching
$result | match
  %user -> respond 200 %user
  !err -> respond !err.code !err.msg

# Multi-condition
$request | match
  .method = "GET" & .auth = true -> ::handle_get
  .method = "POST" & .role = "admin" -> ::handle_post
  .auth = false -> respond 401
  _ -> respond 405

# Inline ternary
$user.active ? grant.access : deny.access
```

---

## 6. Native Data Parsing

TORQ natively understands all common structured data formats. No imports. No packages.

### Universal Parsing

```
$raw | as data       # auto-detect format (JSON, XML, YAML, CSV, TOML)
$raw | as json       # explicit JSON
$raw | as xml        # explicit XML
$raw | as yaml       # explicit YAML
$raw | as csv        # explicit CSV
$raw | as toml       # explicit TOML
```

### Universal Serialization

```
%data | to json      # serialize to JSON string
%data | to xml       # serialize to XML string
%data | to yaml      # serialize to YAML string
%data | to csv       # serialize to CSV string
```

### Validation

```
$raw | as data | valid?                # true/false
$raw | as data | validate %schema      # validate against schema
$raw | as data | strict                # error on malformed input
```

### Schema Validation

```
%schema = {
  name string required
  age int >= 0
  email string match "@"
}

$input | as data | validate %schema
  | fail -> respond 400 $errors
```

---

## 7. Comments — Prompts & Documentation

TORQ has two comment types with fundamentally different purposes.

### `#` — Prompts (AI Directives)

Written by humans. Read by AI. AI acts on them then **deletes them**. These are instructions from the human to the AI author.

```
# add retry with exponential backoff
# handle timeouts differently from auth errors
# optimize for memory not speed
```

**Lifecycle:** Human writes prompt -> AI acts on it -> AI removes the prompt. If the human doesn't like the result, they write a new prompt.

### `##` — Documentation (Human Readable)

Written by AI when requested, or by humans. Stays in the code. Extractable by tooling.

```
## Payment Processing Service
## Handles multi-region payment routing with PSD2 compliance
```

### Rules

| | `#` Prompt | `##` Documentation |
|---|---|---|
| **Author** | Human | AI (on request) or Human |
| **Audience** | AI | Humans |
| **Lifecycle** | Deleted after AI acts | Permanent until human removes |
| **In compiled binary** | No | No |
| **AI generates by default** | Never | Only when requested |

---

## 8. Memory Model — Arena-Scoped

### The Problem

| Language | Memory Model | Tradeoff |
|----------|-------------|----------|
| C | Manual malloc/free | Fast but unsafe |
| Rust | Borrow checker | Safe but complex, wastes AI tokens |
| Go | Garbage collector | Simple but GC pauses |
| Python | Reference counting + GC | Simple but slow |

### TORQ's Approach: Arena-Scoped Memory

Every `::` block gets its own memory arena. When the block finishes, the entire arena frees at once.

```
::process
  $data = file.read "big.csv" | as data
  $result = $data | filter .active | map .name
  $result | file.write "out.txt"
  # block ends -> entire arena freed instantly
```

### Why This Works

- Pipe syntax means data flows **forward only** — no backward references
- The compiler proves at compile time when each arena dies
- Zero runtime overhead — no GC thread, no reference counting
- AI never thinks about memory — leaks are structurally impossible

---

## 9. Concurrency — Parallel by Default

### The Inversion

Every other language: Everything is sequential. Opt IN to parallelism.

TORQ: **Everything is parallel. Opt OUT to sequential.**

### Pipe Auto-Parallelism

```
::main
  $users | fetch_profile | enrich_data | score | rank | save
```

The compiler analyzes the pipeline:
- `fetch_profile` — I/O bound, schedule on async pool
- `enrich_data` — CPU bound, schedule on compute pool
- `score` and `rank` — pure functions, can pipeline/stream
- `save` — I/O, back to async pool

### Parallel Iteration (default)

```
@users | each $user
  fetch $user.profile | enrich | save
```

1,000 users on 8 cores = automatic distribution across 8 chunks.

### Sequential Opt-Out

```
@transactions | each $tx sequential
  $balance = $balance + $tx.amount
  update.ledger $balance
```

### Shared State

```
*counter = 0

@items | each $item
  *counter + 1 -> *counter    # compiler enforces atomic access
  process $item
```

### Safety Guarantees

| Concern | TORQ's Answer |
|---------|--------------|
| Race conditions | Arena memory — each parallel branch gets its own arena |
| Data ordering | Pipes preserve order by default |
| Thread overhead | Compiler auto-tunes — trivial work stays single-threaded |
| Shared state | Explicit `*` sigil, compiler enforces atomic access |

---

## 10. Compilation

### Block-Level Incremental Compilation

```
::auth          # compiled once, cached
::payments      # changed one line — only this recompiles
::notify        # compiled once, cached
::main          # relinks automatically
```

Each `::` block compiles independently. Sub-second rebuilds.

### AI-Aware Optimization Pass

The compiler includes an optimization pass that understands common AI generation patterns:
- Dead code elimination for redundant AI-generated branches
- Constant folding for over-specified literals
- Pipeline fusion for unnecessarily split stages

### Compilation Targets

The compiler produces native machine code. The exact backend (custom, LLVM, or other) is an implementation decision, but the performance target is: **equal to or faster than Go**.

---

## 11. Project Structure

```
project/
  torq.yaml            # human-managed config, services, secrets
  src/
    main.torq          # entry point
    *.torq             # any organization the team wants
  torq_modules/        # auto-generated from API specs (gitignored)
  examples/            # optional
```

### torq.yaml — Project Configuration

Human-written YAML file declaring project metadata, external services, and environment config.

```yaml
name: my-app
version: 1.0.0

services:
  stripe:
    spec: https://raw.githubusercontent.com/stripe/openapi/master/openapi/spec3.json
    type: openapi
    auth: ${STRIPE_KEY}

  sendgrid:
    spec: https://api.sendgrid.com/v3/openapi.json
    type: openapi
    auth: ${SENDGRID_KEY}

database:
  url: ${DATABASE_URL}
  pool: 10

environment:
  production:
    log_level: error
  development:
    log_level: debug
```

### Service Resolution

When the compiler encounters `api.stripe.charge`:
1. Looks up `stripe` in `torq.yaml` services
2. Fetches the OpenAPI spec from the declared URL
3. Auto-generates a TORQ wrapper module in `torq_modules/`
4. Links against it at compile time

Unknown services produce a hard compiler error:
```
ERROR: unknown service "blah"
  -> not found in torq.yaml services
```

---

## 12. Standard Library

Everything below ships with TORQ. Zero config. Zero resolving. Always available. No imports.

### Filesystem — `sys.fs`

```
sys.fs.read "file.txt"                    # read file to string
sys.fs.read "data.bin" binary             # read as bytes
sys.fs.write "out.txt" $content           # write file
sys.fs.append "log.txt" $line             # append
sys.fs.delete "tmp.txt"                   # delete
sys.fs.exists "file.txt"                  # true/false
sys.fs.list "/path"                       # array of filenames
sys.fs.list "/path" recursive             # deep list
sys.fs.move "a.txt" "b.txt"              # rename/move
sys.fs.copy "a.txt" "b.txt"              # copy
sys.fs.size "file.txt"                    # bytes
sys.fs.watch "/path" -> ::on_change       # file watcher
```

### HTTP Server — `http`

```
http.listen 8080
| route get "/path" -> ::handler
| route post "/path" -> ::handler
| route put "/path" -> ::handler
| route delete "/path" -> ::handler
| route ws "/socket" -> ::ws_handler
| middleware ::auth
| cors "*"
| static "/public" "./static"
| tls $cert $key

$req.body                                 # request body
$req.header "name"                        # header value
$req.param "id"                           # URL param :id
$req.query "page"                         # query string
$req.method                               # GET POST etc
$req.path                                 # URL path

respond 200 $data                         # response
respond 200 json $data                    # JSON response
respond 301 redirect "/new"               # redirect
respond 404 "not found"                   # error response
```

### HTTP Client — `http`

```
http.get "https://api.example.com/users"
http.post "https://api.example.com/users" $body
http.put "https://url" $body
http.delete "https://url"

http.get "https://url"
| header "Authorization" "Bearer $token"
| timeout 5s
| retry 3
| fail -> !err
```

### Database — `db`

```
db.connect $url
db.query "select * from users"
db.query "select * from users where id=$id"
db.insert "users" %{name "alice" age 30}
db.update "users" %{age 31} where %{name "alice"}
db.delete "users" where %{name "alice"}
db.count "users"
db.tables
db.migrate "migrations/"
db.transaction
  db.insert "orders" $order
  db.update "inventory" %{stock $stock - 1}
  | fail -> rollback
```

### Strings

```
$str | upper                              # HELLO
$str | lower                              # hello
$str | upper_first                        # Hello
$str | trim                               # strip whitespace
$str | split ","                          # to array
$str | replace "old" "new"               # replace first
$str | replace_all "old" "new"           # replace all
$str | contains "word"                    # true/false
$str | starts "pre"                       # true/false
$str | ends "suf"                         # true/false
$str | len                                # length
$str | slice 0 5                          # substring
$str | reverse                            # reverse
$str | match ~"pattern"                   # regex match
$str | extract ~"(\\d+)"                  # regex capture
$str | pad_left 10 "0"                    # "0000000042"
$str | encode base64                      # encode
$str | decode base64                      # decode
$str | hash sha256                        # hash
```

### Math

```
$a + $b                                   # add
$a - $b                                   # subtract
$a * $b                                   # multiply
$a / $b                                   # divide
$a % $b                                   # modulo
$a ** $b                                  # power

math.abs $n
math.round $n 2
math.floor $n
math.ceil $n
math.min @values
math.max @values
math.sqrt $n
math.random
math.random 1 100
math.pi
math.e
```

### Arrays

```
@arr | len                                # length
@arr | first                              # first element
@arr | last                               # last element
@arr | at 3                               # element at index
@arr | push $val                          # append
@arr | pop                                # remove last
@arr | shift                              # remove first
@arr | unshift $val                       # prepend
@arr | slice 1 5                          # sub-array
@arr | reverse                            # reverse
@arr | sort                               # sort ascending
@arr | sort desc                          # descending
@arr | sort by .name                      # sort by field
@arr | unique                             # deduplicate
@arr | flatten                            # flatten nested
@arr | zip @other                         # zip two arrays
@arr | chunk 3                            # groups of 3
@arr | filter > 10                        # filter by condition
@arr | filter .active                     # filter by field
@arr | map * 2                            # transform each
@arr | map .name                          # extract field
@arr | reduce + 0                         # reduce
@arr | sum                                # sum shortcut
@arr | any > 100                          # any match?
@arr | all > 0                            # all match?
@arr | find .name = "alice"               # first match
@arr | count .active                      # count matching
@arr | join ","                           # to string
@arr | empty?                             # true/false
```

### Dictionaries

```
%dict | keys                              # array of keys
%dict | values                            # array of values
%dict | entries                           # array of [key value]
%dict | get "key"                         # get value
%dict | set "key" $val                    # set value
%dict | drop "key"                        # remove key
%dict | has "key"                         # true/false
%dict | merge %other                      # merge two dicts
%dict | pick @["name" "age"]              # subset of keys
%dict | omit @["password" "secret"]       # exclude keys
%dict | empty?                            # true/false
%dict | len                               # number of keys
```

### Time & Date

```
time.now                                  # current timestamp
time.now | format "YYYY-MM-DD"           # format
time.parse "2024-01-15"                  # parse string
time.diff $a $b                           # difference
time.add $t 5m                            # add duration
time.add $t 2h                            # hours
time.add $t 7d                            # days
time.zone "US/Eastern"                    # timezone
time.unix                                 # epoch seconds
time.sleep 1s                             # pause
```

### Duration Literals

```
100ms    # milliseconds
1s       # seconds
5m       # minutes
2h       # hours
7d       # days
```

### Crypto & Security

```
crypto.hash sha256 $data
crypto.hmac sha256 $key $data
crypto.encrypt aes256 $key $data
crypto.decrypt aes256 $key $data
crypto.random 32
crypto.uuid
jwt.sign $payload $secret
jwt.verify $token $secret
```

### Logging & Observability

```
log $message                              # info
log.info $message
log.warn $message
log.err $message
log.debug $message

trace.start "operation_name"
trace.end
trace.tag "key" "value"

metric.count "requests" 1
metric.gauge "memory" $bytes
metric.time "response" $duration
```

### Process & System

```
sys.env "VAR_NAME"                        # env variable
sys.exec "command"                        # shell command
sys.exec "command" | as data              # capture output
sys.pid                                   # process ID
sys.hostname                              # hostname
sys.cores                                 # CPU cores
sys.memory                                # available memory
sys.exit 0                                # exit code
sys.args                                  # CLI arguments
```

### Scheduling

```
schedule.every 5m -> ::health_check
schedule.every 1h -> ::cleanup
schedule.at "09:00" -> ::morning_report
schedule.cron "0 */6 * * *" -> ::job
schedule.after 30s -> ::delayed_task
```

### Networking

```
net.tcp.listen 9000 -> ::handler
net.tcp.connect "host" 9000
net.udp.listen 5000 -> ::handler
net.udp.send "host" 5000 $data
net.dns.lookup "example.com"
net.ping "host"
```

---

## 13. Error Handling

Errors use the `!` sigil and flow through pipes naturally.

### Error Creation

```
!err = "something failed"
!err = {code 404 msg "not found" detail "user 123"}
```

### Error Propagation

```
::fetch_user $id
  db.query "select * from users where id=$id"
  | fail -> !{code 404 msg "user $id not found"}
```

### Error Handling in Pipes

```
::main
  ::fetch_user $id | match
    %user -> respond 200 %user
    !err -> respond !err.code !err.msg
```

### Retry on Failure

```
api.stripe.charge $amount
| retry 3 delay 1s backoff exponential
| fail -> !{code 500 msg "payment failed after retries"}
```

### Fallback

```
api.primary.fetch $data
| fail -> api.secondary.fetch $data
| fail -> cache.get $data
| fail -> !{code 503 msg "all sources unavailable"}
```

---

## 14. Bootstrap Strategy

### Phase 1: Grammar in System Prompt (Day One)

The TORQ grammar is ~50 rules. The entire spec fits in a system prompt. Any AI model can write TORQ today by reading this specification.

### Phase 2: Compiler + Example Corpus

Build the `torqc` compiler. Create a corpus of example TORQ programs covering every construct. AI reads spec + examples and improves.

### Phase 3: Transpiler-Generated Corpus

Build Python-to-TORQ and Go-to-TORQ transpilers. Feed existing open source projects through them. Generate a massive TORQ corpus for future AI training data.

---

## 15. Token Economics

### Why TORQ Exists

Every token an AI generates costs money and time. TORQ minimizes both.

### Comparison: Checkout Service

**Python (~150 tokens):**
```python
import stripe
from flask import Flask, request, jsonify
import logging

app = Flask(__name__)
logger = logging.getLogger(__name__)

@app.route('/checkout', methods=['POST'])
def checkout():
    try:
        data = request.get_json()
        amount = data['amount']
        user = data['user']
        charge = stripe.Charge.create(
            amount=amount,
            currency='usd',
            source=user['payment_source']
        )
        logger.info(f"Charge complete: {charge.id}")
        return jsonify({"status": "ok", "charge_id": charge.id}), 200
    except stripe.error.CardError as e:
        logger.error(f"Card error: {e}")
        return jsonify({"error": str(e)}), 400
    except Exception as e:
        logger.error(f"Payment failed: {e}")
        return jsonify({"error": "payment failed"}), 500
```

**TORQ (~30 tokens):**
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

**5x fewer tokens. Same functionality. Faster runtime.**

---

## Appendix A: Reserved Words

TORQ has minimal reserved words:

```
each        # iteration
sequential  # opt-out of parallel
loop        # conditional/infinite loop
match       # pattern matching
break       # exit loop
range       # number sequence
true false null  # literals
as to       # data conversion
fail        # error pipe
retry       # retry logic
respond     # HTTP response
```

**15 reserved words total.** Compare: Python has 35. JavaScript has 64. Java has 67.

---

## Appendix B: Operator Reference

```
|       pipe (data flow)
->      assignment / direction
=       binding
?       ternary condition
:       ternary else
&       logical AND in match
.       member access
+       add
-       subtract
*       multiply / shared sigil
/       divide
%       modulo / dict sigil
**      power
>       greater than
<       less than
>=      greater or equal
<=      less or equal
!=      not equal
```

---

## Appendix C: Complete Grammar Summary

```
program      = block*
block        = "::" name param* newline body
body         = (statement newline)*
statement    = pipe | assignment | loop_stmt | each_stmt
pipe         = expr ("|" expr)*
expr         = literal | sigil_var | call | match_expr
sigil_var    = "$" name | "@" name | "%" name | "!" name | "~" name | "*" name
literal      = string | number | bool | null | array | dict
array        = "[" value* "]"
dict         = "{" (name value)* "}"
assignment   = sigil_var "=" expr
call         = name ("." name)* arg*
match_expr   = "match" newline (pattern "->" expr newline)*
each_stmt    = expr "|" "each" sigil_var "sequential"? newline body
loop_stmt    = "loop" expr? newline body
```

~30 grammar rules. Fits in any AI system prompt.
