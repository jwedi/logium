# Logium

Logium is a log analysis tool that solves the following use cases for you:
1. You have multiple different log files with different structure and you need to establish the order of events across them.
    - For example 1 client log and N server logs.
2. You're repeatedly analysing similar log files and you know what you're looking for and would like to automate the extraction of key information to make your analysis faster.
    - Automatically extract user ids, launch parameters, server network responses, client errors
    - "State parameters" i.e key-value attributes are extracted from log files using regexes and capture groups.
3. You're repeatedly analysing similar log files and you know common failure cases or scenarios that you would like to automatically detect.
    - If the server logs X and Y before the client logs Z then that's a known failure case.
    - If the client has state parameters X, Y then that's a known failure case.

**The motivating example:** a game client attempts matchmaking and fails. To diagnose why, an engineer must read both client and server logs, establish a global timeline, track each service's state, and mentally match it against known scenarios (server full? wrong region? crashed? game not joinable?). Logium automates this. You define the failure patterns once, and Logium finds them instantly across any set of logs.

## Quick Start

Download a pre-built binary from [github.com/jwedi/logium/releases](https://github.com/jwedi/logium/releases) and run it directly — no Rust or Node.js required.

Or build from source:

```bash
# Prerequisites: Rust (cargo), Node.js (npm)

# Clone and run
./run.sh          # builds everything, starts backend + frontend dev server
```

This gives you:
- Frontend at **http://localhost:5173** (Vite dev server with hot reload)
- Backend API at **http://localhost:3000/api**

Other modes:
```bash
./run.sh build    # build everything for production, run tests
./run.sh prod     # build and serve from the backend (single process)
```

### Manual Setup

If you prefer to run things individually:

```bash
# 1. Install frontend dependencies
cd ui && npm install && cd ..

# 2. Build and start the backend
cargo run -p logium-server
# Server starts on http://localhost:3000

# 3. Start the frontend dev server (in another terminal)
cd ui && npm run dev
# Dev server starts on http://localhost:5173, proxies /api to :3000
```

### Running Tests

```bash
cargo test --workspace
```

---

### Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `sqlite:logium.db?mode=rwc` | SQLite connection string |
| `PORT` | `3000` | Server port |
| `UPLOADS_DIR` | `./uploads` | Directory for uploaded log files |

---

## Concepts

Logium is built around a small set of entities. Understanding them makes configuration straightforward.

### Timestamp Templates

A timestamp template tells Logium how to parse timestamps out of log lines.

| Field | Description |
|-------|-------------|
| `format` | A chrono format string, e.g. `%Y-%m-%d %H:%M:%S` |
| `extraction_regex` | Optional regex to pull the timestamp substring from the middle of a line (e.g. nginx wraps timestamps in `[...]`) |
| `default_year` | For yearless formats like syslog — Logium prepends the current year automatically |

Source templates reference a timestamp template by ID.

### Source Templates

A source template describes the structure of a log file format. It is reusable — many actual log files can share one template.

| Field | Description |
|-------|-------------|
| `timestamp_template_id` | Which timestamp template to use |
| `line_delimiter` | How to split entries (default: `\n`) |
| `content_regex` | Optional regex to extract just the "message" part of a line |
| `continuation_regex` | Optional regex — if a line matches, it is merged with the preceding entry (e.g. stack traces) |
| `json_timestamp_field` | If set, lines are parsed as JSON and the timestamp is read from this field; all other JSON fields are auto-extracted as state |

### Sources

A source is an actual log file bound to a source template.

| Field | Description |
|-------|-------------|
| `file_path` | Path to the log file |
| `name` | Human-readable label used in the UI and in predicate references |
| `color` | UI color for the timeline |

### Rules

Rules match individual log lines and optionally extract state from them.

- **`match_rules`**: one or more regex patterns; `match_mode` controls whether `Any` or `All` must match.
- **`extraction_rules`**: what state to record when the rule fires. Each extraction rule can:
  - `Parsed` — extract a value via a named capture group
  - `Static` — write a hardcoded value
  - `Clear` — delete a key from state
  - `Accumulate` mode — append to a list or increment a counter rather than overwriting
- **`event_text`**: optional template string with `{key}` substitutions; shown as the event label in the timeline.

### Rulesets

A ruleset binds a set of rules to a source template. It determines which rules are evaluated on which log sources. The ruleset name is used in pattern predicates to reference state from that source.

### Patterns

Patterns are ordered state machines that detect multi-step failure scenarios, potentially across multiple sources.

- **`predicates`**: an ordered list of conditions. Each predicate names a ruleset, a state key, an operator, and an operand.
- **Operators**: `Eq`, `Neq`, `Gt`, `Lt`, `Gte`, `Lte`, `Contains`, `Exists`
- **Operands**: a literal value, or a `StateRef` pointing to another ruleset's state key for cross-source comparisons.
- **Semantics**: predicates must be satisfied in order. All previously matched predicates must still hold when the next one fires. If an earlier predicate becomes false, progress resets to 0. When all predicates hold simultaneously, a `PatternMatch` is emitted and the pattern can re-fire immediately.

### How Everything Fits Together

Define timestamp/source templates → add sources → create rules and group them into rulesets → define patterns → run analysis. Logium merges all sources into a global timeline and emits pattern matches as events.

```
Timestamp Template
    └── Source Template  (references one timestamp template)
            ├── Ruleset  (binds rules to this source template)
            └── Sources  (actual log files using this template)
                    └── Rule matches → per-source state map

Pattern
    └── Predicates  (reference rulesets + state keys; can cross sources via StateRef)
```

---

## Architecture & Performance

### Two-Phase Analysis Pipeline

Analysis runs in two phases:

1. **Phase 1 — parallel**: each source file is processed independently using rayon. Rules are evaluated per-chunk (10 000 lines) across CPU cores. No shared state, no lock contention.
2. **Phase 2 — sequential**: a K-way merge produces a single global timeline. State mutations and pattern evaluation happen in timestamp order. This phase is I/O-free and fast.

### Streaming Iterators (Constant Memory)

`LogLineIterator` wraps a `BufReader` with a 64 KB buffer. Lines are parsed on demand — the full file is never loaded into memory. Memory usage is O(1) with respect to file size, enabling analysis of multi-gigabyte logs.

### K-Way Merge (BinaryHeap)

`MergedLogStream` uses a `BinaryHeap` to merge N sorted per-source streams in O(log N) time per line. Per-source ordering is guaranteed by the streaming iterator; the heap picks the globally earliest line at each step. Ties are broken by source ID for determinism.

### Batch Regex Matching (RegexSet)

Each `CompiledRule` uses `regex::RegexSet` to test all patterns in a single pass over the input string, rather than calling `Regex::is_match` N times. This is especially effective when many rules share log sources.

### Chunk-Based Parallelism

Rules are evaluated in chunks of 10 000 lines using rayon's parallel iterators. Chunk size balances cache locality (large chunks = fewer allocations) against parallelism granularity. Results are collected as `ProcessedLine` objects and fed into Phase 2 sequentially.

---
