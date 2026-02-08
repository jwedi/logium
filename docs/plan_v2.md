# Logium -- Design Document (v2)

## Problem Statement

Investigating failures in distributed systems is repetitive and time-consuming. An operator typically must open logs from multiple services, establish a global chronological order of events, track the evolving state of each service, and mentally match that state against known failure scenarios.

**Example.** A game client attempts matchmaking and fails to join the expected server. To diagnose the failure an engineer must read both the client logs and the server logs, determine the server's state (e.g. "full", "wrong region") at the moment the client began matchmaking, and correlate the two. If the server was full, that is one known failure case; if the server was in a different region, that is another. Doing this manually for every incident does not scale.

Logium automates this process. Users define **rules** that extract state from individual log lines and **patterns** that match when state conditions across one or more log sources are met in a specific order. Logium ingests log files, applies rules, evaluates patterns, and reports matches -- telling the operator exactly which known failure case occurred and where.

---

## Architecture Overview

Logium is a web application with a Rust backend and a Svelte 5 frontend.

```
                        +---------------------------+
                        |       Svelte 5 UI         |
                        |  (Viewer-first interface)  |
                        +------------+--------------+
                                     | HTTP / WS
                        +------------+--------------+
                        |     logium-server          |
                        |  Axum  |  SQLite           |
                        +------------+--------------+
                                     |
                        +------------+--------------+
                        |     logium-core            |
                        |  Engine (pure logic)       |
                        |  - Streaming iterators     |
                        |  - K-way merge (min-heap)  |
                        |  - RegexSet matching       |
                        |  - Per-source state maps   |
                        |  - Pattern evaluator       |
                        +---------------------------+
```

### Cargo Workspace

```
logium/
  crates/
    logium-core/     # Pure logic engine, no IO, no framework deps
    logium-server/   # Axum HTTP server, SQLite persistence, serves UI
  ui/                # Svelte 5 frontend
```

- **logium-core** -- Defines all domain types and the evaluation engine. Has no dependency on any web framework or database. Designed for deterministic, testable logic.
- **logium-server** -- Axum-based HTTP/WebSocket server. Owns SQLite persistence and serves the compiled Svelte UI as static assets.
- **ui/** -- Svelte 5 single-page application. Communicates with the server over REST and WebSocket.

### Key Technical Choices

| Concern | Decision |
|---------|----------|
| Backend language | Rust |
| Web framework | Axum |
| Persistence | SQLite |
| Frontend framework | Svelte 5 |
| Configuration | GUI-only (no config files) |
| Regex strategy | `RegexSet` for fast multi-rule matching |
| Log merging | K-way merge via min-heap on timestamp |
| State storage | `HashMap<String, String>` per source |

---

## Domain Model

### Project

Top-level organizational unit. A project owns a set of source templates, rulesets, and patterns.

### SourceTemplate

Describes how to interpret a class of log sources.

| Field | Description |
|-------|-------------|
| `name` | Human-readable identifier |
| `timestamp_format` | Auto-detected or user-specified format string for parsing timestamps |
| `line_delimiter` | Character or sequence that separates log lines (default: newline) |

Timestamp format and line delimiter support auto-detection: Logium samples the first N lines and infers the format. The user can override.

### Source

A concrete log source (file or stream) associated with a source template.

| Field | Description |
|-------|-------------|
| `name` | Human-readable identifier |
| `template` | Reference to a `SourceTemplate` |
| `path` / `stream` | Location of the log data |

### LogLine

A single parsed log entry.

| Field | Description |
|-------|-------------|
| `timestamp` | Parsed timestamp |
| `source_id` | Identifier of the originating source |
| `content` | Raw text content of the log line |

---

## Rules

Rules operate on individual log lines to produce state mutations. A rule is composed of match rules (to determine applicability) and extraction rules (to mutate state).

### MatchRule

A regex pattern tested against a log line's content.

| Field | Description |
|-------|-------------|
| `pattern` | Regex pattern |

A log rule specifies a **match mode** for its collection of match rules:

- **All** -- Every match rule must match the log line.
- **Any** -- At least one match rule must match.

### ExtractionRule

Determines how state is produced when a log rule matches. Three variants:

| Variant | Description |
|---------|-------------|
| **Parsed** | Regex with named capture groups. Captured values become state key-value pairs. |
| **Static** | Fixed key-value pair added to state unconditionally on match. |
| **Clear** | Removes a specific key (or all keys) from the source's state. |

Each extraction rule has a **write mode**:

- **Replace** -- The extracted value overwrites any existing value for that key.
- **Accumulate** -- The extracted value is appended (e.g. to a list) for that key.

### LogRule

A named, reusable unit combining match rules and extraction rules.

| Field | Description |
|-------|-------------|
| `name` | Identifier |
| `match_rules` | One or more `MatchRule` instances |
| `match_mode` | `All` or `Any` |
| `extraction_rules` | Zero or more `ExtractionRule` instances |

When a log line is processed against a log rule:

1. Evaluate all match rules according to the match mode.
2. If matched, execute each extraction rule to mutate the source's state.
3. Emit the match event (timestamp, source, log line, resulting state snapshot).

### Ruleset

A named collection of log rule references, mapped to a **source template** (not to individual sources). Every source using that template automatically inherits the ruleset's rules.

| Field | Description |
|-------|-------------|
| `name` | Identifier |
| `source_template` | The template this ruleset applies to |
| `rules` | Ordered list of `LogRule` references |

Rules are reusable: the same log rule can appear in multiple rulesets.

---

## Patterns

Patterns detect known failure cases by matching ordered state conditions across sources.

### PatternPredicate

A single boolean condition over the global state (which spans all sources).

| Field | Description |
|-------|-------------|
| `state_key` | Qualified key: `<source>.<key>` (e.g. `server.region`) |
| `operator` | Comparison operator |
| `operand` | Literal value **or** cross-source state reference (e.g. `client.region`) |

**Operators:**

| Operator | Description |
|----------|-------------|
| `Eq` | Equal |
| `Neq` | Not equal |
| `Gt` | Greater than |
| `Lt` | Less than |
| `Gte` | Greater than or equal |
| `Lte` | Less than or equal |
| `Contains` | String/collection contains |
| `Exists` | Key is present in state |

**Operand types:**

- **Literal** -- A fixed value (string, number). Example: `server.status Eq "full"`.
- **StateRef** -- A reference to another source's state key. Example: `client.region Neq server.region`.

### Pattern

A named, ordered sequence of predicates that identifies a specific failure case.

| Field | Description |
|-------|-------------|
| `name` | Human-readable identifier (e.g. "Server Full During Matchmaking") |
| `predicates` | Ordered list of `PatternPredicate` instances |

### Pattern Matching Semantics: All-Must-Hold, Ordered Activation

The pattern evaluator tracks predicate progress per pattern instance:

1. The evaluator watches for **predicate 1** to become true (its state condition is satisfied).
2. Once predicate 1 is true, the evaluator begins watching for **predicate 2**, while continuously verifying that predicate 1 **remains** true.
3. If predicate 1 becomes false before predicate 2 activates, the pattern **resets** to step 1.
4. This continues for each subsequent predicate: the evaluator advances only while all previously-activated predicates remain true.
5. When **all predicates are simultaneously true** and each became true in the specified order: the pattern **matches**.
6. On match, the pattern emits a match event (timestamp, matched predicates with state snapshots) and **resets**. The pattern can re-fire on subsequent state changes.

This model ensures that a pattern represents a sustained, ordered convergence of conditions -- not merely a sequence of transient events.

---

## Engine

The engine is the core processing pipeline in `logium-core`. It is stateless with respect to persistence and operates over streaming iterators.

### Processing Pipeline

```
  Sources (files/streams)
        |
        v
  [Parse log lines per SourceTemplate]
        |
        v
  [K-way merge via min-heap on timestamp]  -->  Global chronological order
        |
        v
  [Apply Rulesets: RegexSet matching + extraction]  -->  Per-source state mutations
        |
        v
  [Evaluate Patterns against global state]  -->  Match events
        |
        v
  [Emit results: matches, timeline events]
```

### Key Implementation Details

- **Streaming iterators** -- Log lines are processed lazily; no need to load entire files into memory.
- **K-way merge min-heap** -- Merges log lines from N sources into a single chronologically ordered stream. The heap holds one "next" line per source.
- **RegexSet** -- All match rules for a given ruleset are compiled into a single `RegexSet`, enabling a single pass over each log line to determine which rules match.
- **Per-source state** -- Each source maintains a `HashMap<String, String>` representing its current state. Extraction rules mutate this map.
- **Pattern evaluator with progress tracking** -- Each pattern instance tracks its current "progress index" (how many predicates have been activated in order). On every state mutation, all pattern instances are re-evaluated.

---

## UI Paradigm: Viewer-First

Logium's UI is built around the **log file viewer** as the primary interface. The workflow:

1. **Load log files** -- The user opens one or more log files into the viewer.
2. **Browse logs** -- Logs are displayed in merged chronological order with source coloring. The user scrolls and reads log lines.
3. **Create rules by highlighting** -- The user highlights text in a log line and creates a rule directly from the selection. Logium suggests regex patterns and lets the user refine match and extraction rules in-place.
4. **Real-time feedback** -- As rules and patterns are defined, the viewer immediately highlights matching log lines and shows state changes. Pattern matches appear inline in the timeline.
5. **Iterate** -- The user refines rules and patterns while seeing results update live.

This approach grounds the configuration experience in the actual data, reducing the gap between "reading logs" and "writing detection rules."

### Timeline View

A merged chronological view showing:

- All log rule match events with extracted state values.
- Pattern match events with their associated predicates and state snapshots.
- Source-colored entries for quick visual identification.

---

## Data Model and Persistence

All configuration (projects, source templates, rulesets, rules, patterns) is stored in SQLite. There are no configuration files; all setup is performed through the GUI.

### Entity Relationships

```
Project
  |-- SourceTemplate[]
  |     |-- (auto-detected timestamp format, line delimiter)
  |-- Ruleset[]
  |     |-- maps to one SourceTemplate
  |     |-- LogRule[] (references, reusable across rulesets)
  |           |-- MatchRule[]
  |           |-- ExtractionRule[]
  |-- Pattern[]
        |-- PatternPredicate[]
```

Sources are runtime entities loaded by the user (file paths/streams); they reference a source template but are not persisted as part of the project configuration.

---

## Phasing

### Phase 1: Core + Server + Viewer UI

Deliverables:

- **logium-core**: All domain types, rule evaluation engine, pattern matching engine, streaming pipeline with K-way merge.
- **logium-server**: Axum server, SQLite schema and persistence layer, REST API for CRUD operations on projects/templates/rulesets/rules/patterns, WebSocket for live evaluation results.
- **ui/**: Svelte 5 viewer-first interface -- file loading, log browsing, rule creation via highlight, real-time match display, basic timeline.

### Phase 2: Graphical Timeline

Deliverables:

- Visual timeline component rendering log events and pattern matches on a time axis.
- Interactive features: zoom, pan, click-to-navigate-to-log-line.
- Source swimlanes showing per-source state evolution over time.
