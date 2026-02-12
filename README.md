# Logium

Logium is a log analysis tool that detects known failure cases by correlating events across multiple log sources. You define **rules** that extract state from log lines, and **patterns** that fire when state conditions across sources are met in a specific order. Logium ingests log files, applies rules, evaluates patterns, and tells you which failure case occurred and where.

**The motivating example:** a game client attempts matchmaking and fails. To diagnose why, an engineer must read both client and server logs, establish a global timeline, track each service's state, and mentally match it against known scenarios (server full? wrong region? crashed?). Logium automates this. You define the failure patterns once, and Logium finds them instantly across any set of logs.

## Quick Start

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

## Architecture

```
                    +---------------------------+
                    |       Svelte 5 UI         |
                    |  (Viewer-first interface)  |
                    +------------+--------------+
                                 | HTTP / WebSocket
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

### Project Structure

```
logium/
  Cargo.toml                     # Workspace root
  run.sh                         # Build & run script
  AGENTS.md                      # Development guidelines
  crates/
    logium-core/                 # Pure logic engine (no IO, no framework deps)
      src/
        model.rs                 # All domain types (TimestampTemplate, SourceTemplate, etc.)
        engine.rs                # Streaming engine + unit tests
        lib.rs                   # Module exports
      tests/
        real_data_tests.rs       # Integration tests against real log data
        fixtures/                # Real-world log files for testing
          zookeeper/             # Zookeeper logs (timestamp at line start)
          nginx/                 # Nginx access logs (mid-line timestamps)
          syslog/                # Linux syslog (yearless timestamps)
      benches/
        analysis_benchmark.rs    # Criterion benchmarks
    logium-server/               # Axum HTTP server + SQLite
      src/
        main.rs                  # Server setup, CORS, static serving
        db.rs                    # Schema, migrations, CRUD
        routes/
          mod.rs                 # Error handling, ApiResult type
          projects.rs            # Project CRUD
          timestamp_templates.rs # Timestamp template CRUD
          templates.rs           # Source template CRUD
          sources.rs             # Source CRUD + file upload
          rules.rs               # Rule CRUD (with match/extraction rules)
          rulesets.rs            # Ruleset CRUD
          patterns.rs            # Pattern CRUD (with predicates)
          analysis.rs            # Analysis, template detection, rule suggestion
  ui/                            # Svelte 5 frontend
    src/
      App.svelte                 # App shell: sidebar nav + view routing
      app.css                    # Global styles (Tokyo Night dark theme)
      lib/
        api.ts                   # Typed API client (all endpoints)
        LogViewer.svelte         # Primary: virtual-scrolled log viewer
        RuleCreator.svelte       # Highlight-to-rule modal
        PatternEditor.svelte     # Ordered predicate builder
        AnalysisView.svelte      # Run analysis, display results
        ProjectManager.svelte    # Create/select/delete projects
        SourceManager.svelte     # Upload log files, manage sources
        TemplateManager.svelte   # View/edit source templates
        RuleList.svelte          # Browse and manage rules
        RulesetManager.svelte    # Group rules, bind to templates
  scripts/
    setup_test_data.sh           # Download and prepare test fixture files
    run_benchmark.sh             # Run benchmarks, save timestamped results
  benchmark/
    results/                     # Timestamped benchmark output files
  docs/
    plan_v2.md                   # Full design document
```

### Why This Layering

**logium-core** is a pure library with zero framework dependencies. It takes data structures in, returns results out. This makes it trivially testable, reusable in other contexts (CLI, batch processing), and keeps the complex logic isolated from web/IO concerns.

**logium-server** handles the "real world": HTTP, SQLite persistence, file uploads, CORS. It converts between database rows and core model types, delegates analysis to the core engine via `tokio::task::spawn_blocking`, and serves the frontend.

**The frontend** communicates via REST and WebSockets. Analysis results stream over a WebSocket connection (`/api/projects/:pid/analyze/ws`), enabling incremental rendering with a live progress counter. In development, Vite proxies `/api` and WebSocket requests to the backend (configured in `vite.config.ts`). In production, the backend serves the built frontend from `ui/dist/` as static files.

---

## Key Design Decisions

### State-Based Model

Rules don't just match log lines — they produce **state mutations**. Each source maintains a `HashMap<String, StateValue>` of current state. When a rule matches a log line, its extraction rules modify the source's state (set a value, accumulate, or clear).

Patterns then match against this accumulated state, not against individual log lines. This means a pattern like "server is full AND client is connecting" works regardless of which specific log lines produced that state.

### All-Must-Hold, Ordered Activation Pattern Semantics

This is the core matching model, and it's more nuanced than simple sequential matching:

1. The evaluator watches for **predicate 1** to become true in the current state
2. Once true, it watches for **predicate 2** while continuously verifying predicate 1 **remains** true
3. If predicate 1 becomes false before predicate 2 activates, progress **resets** to step 1
4. When all predicates are simultaneously true and each became true in order → **match**
5. After matching, the pattern **resets and can re-fire**

This prevents false positives from transient states. If the server was briefly full but then had capacity by the time the client connected, the pattern won't match — the "full" predicate became false before the "connecting" predicate activated.

### Cross-Source State References

Predicate operands can be either literal values or references to another source's state:

```
server.region  !=  client.region      # StateRef operand
server.players  >  64                 # Literal operand
```

This is how Logium detects cross-source conditions like "client in a different region than server."

### Rulesets Map to Templates, Not Sources

A ruleset is bound to a **source template** (e.g., "server log format"), not to individual sources. When you add a new server log file that uses the same template, all the template's rulesets automatically apply. This avoids duplicating rule configuration per file.

### Timestamp Templates

Timestamp parsing is configured separately from source templates via `TimestampTemplate`. This allows:

- **`extraction_regex`**: For log formats where the timestamp isn't at the start of the line (e.g., nginx access logs: `93.180.71.3 - - [17/May/2015:08:05:32 +0000] ...`). The regex's capture group 1 extracts the timestamp substring before parsing.
- **`default_year`**: For yearless formats like syslog (`Jun 14 15:16:01 ...`). The engine automatically prepends the default year when the format string lacks `%Y`.

A `SourceTemplate` references a `TimestampTemplate` via `timestamp_template_id`, so multiple source templates with different content extraction patterns can share the same timestamp parsing configuration.

### Streaming Architecture

The engine never loads entire log files into memory. It uses:
- **`LogLineIterator`**: Reads lines lazily via `BufReader`, parsing timestamps on the fly using the associated `TimestampTemplate`. Supports multi-line log entries via `continuation_regex` — lines matching the regex are merged into the preceding logical entry. Supports JSON Lines via `json_timestamp_field` — when set, each line is parsed as JSON and the timestamp is extracted from the named field
- **`MergedLogStream`**: K-way merge via `BinaryHeap` (min-heap) — merges K source iterators in chronological order in O(N log K) time
- **`RegexSet`**: All match rules for a rule are compiled into a single regex automaton. One pass over the text tests all patterns simultaneously, instead of running regexes sequentially

### GUI-Only Configuration

There are no config files. All projects, rules, patterns, and templates are managed through the UI and persisted in SQLite. This is a deliberate choice: log analysis rules are inherently exploratory, and the viewer-first UI makes the feedback loop tight.

---

## How the Engine Works

The analysis pipeline in `logium-core` processes log files in a single streaming pass:

```
Sources (files)
    │
    ▼
Parse log lines per SourceTemplate + TimestampTemplate (lazy iterators)
    │  (extraction_regex → timestamp substring → parse with format + default_year)
    │
    ▼
K-way merge via min-heap on timestamp  →  Global chronological order
    │
    ▼
For each line:
    ├─ Find applicable rulesets (via source's template_id)
    ├─ Evaluate rules (RegexSet match → extraction)
    ├─ Apply state mutations to per-source state
    └─ Evaluate all patterns against current global state
    │
    ▼
Results: rule matches + pattern matches with state snapshots
```

### Rule Evaluation

A `LogRule` contains **match rules** (regex patterns) and **extraction rules** (what state to produce).

Match rules are compiled into a `RegexSet` — a single automaton that tests all patterns in one pass over the text. The `MatchMode` determines whether any pattern matching is sufficient (`Any`) or all must match (`All`).

If the rule matches, extraction rules run:
- **Parsed**: Uses a regex with named capture groups (e.g., `Players: (?P<player_count>\d+)`). The captured value is auto-typed: tries `i64`, then `f64`, then `bool`, falls back to `String`.
- **Static**: Assigns a fixed value to a state key whenever the rule matches.
- **Clear**: Removes a state key.

Each extraction rule also has a **mode**: `Replace` (overwrite) or `Accumulate` (for strings: comma-separated append; for numbers: addition).

### Pattern Evaluation

The `PatternEvaluator` tracks a progress index per pattern. After every state mutation, it re-checks all patterns:

```rust
pub struct PatternEvaluator {
    progress: Vec<usize>,  // index into predicates for each pattern
}
```

For each pattern at progress index `i`:
1. Is predicate `i` satisfied? If not, do nothing.
2. If yes, are all predicates `0..i` still satisfied? If any became false, reset to 0.
3. If all hold, advance to `i+1`. If that completes all predicates, emit a match and reset to 0.

This simple state machine handles ordered activation, predicate invalidation, and re-firing.

---

## The Frontend in Detail

The UI is built with **Svelte 5** using its new **runes** reactivity system. If you're not familiar with Svelte 5, here's how the key patterns work in this codebase:

### Svelte 5 Runes (the Reactivity System)

Svelte 5 replaced the old `$:` reactive declarations with explicit **runes** — function-like constructs that the compiler transforms into efficient reactive code:

**`$state(initialValue)`** — Creates reactive state. When the value changes, anything that reads it re-renders. Used throughout for component-local state:
```svelte
let lines: string[] = $state([]);        // LogViewer: list of log lines
let scrollTop = $state(0);               // LogViewer: current scroll position
let showRuleCreator = $state(false);     // LogViewer: modal visibility toggle
let currentView: View = $state('projects'); // App: which view is active
```

**`$derived(expression)`** — Computed values that automatically update when their dependencies change. The compiler tracks which `$state` values are read:
```svelte
// LogViewer: virtual scrolling calculations
let totalHeight = $derived(lines.length * LINE_HEIGHT);
let startIdx = $derived(Math.max(0, Math.floor(scrollTop / LINE_HEIGHT) - OVERSCAN));
let endIdx = $derived(Math.min(lines.length, Math.ceil((scrollTop + containerHeight) / LINE_HEIGHT) + OVERSCAN));
let visibleLines = $derived(lines.slice(startIdx, endIdx));
```

**`$derived.by(() => { ... })`** — For derived values that need more complex logic than a single expression:
```svelte
// LogViewer: builds a Map from line index → rule matches for O(1) lookup during render
let lineMatchMap = $derived.by(() => {
    const map = new Map<number, { ruleId: number; match: RuleMatch }[]>();
    for (const m of ruleMatches) { ... }
    return map;
});
```

**`$effect(() => { ... })`** — Side effects that re-run when their dependencies change. Used for data loading, DOM observation, and API calls:
```svelte
// App.svelte: loads projects on mount
$effect(() => { loadProjects(); });

// LogViewer: re-loads file when the source prop changes
$effect(() => { source; loadFileContent(); loadRules(); });

// LogViewer: observes container resize for virtual scrolling
$effect(() => {
    if (container) {
        const obs = new ResizeObserver(entries => {
            containerHeight = entry.contentRect.height;
        });
        obs.observe(container);
        return () => obs.disconnect();  // cleanup function
    }
});
```

**`$props()`** — Declares component inputs. Replaces Svelte 4's `export let`:
```svelte
// LogViewer.svelte
let { source, projectId, ruleMatches = [], patternMatches = [] }: {
    source: Source;
    projectId: number;
    ruleMatches?: RuleMatch[];
    patternMatches?: PatternMatch[];
} = $props();
```

### Key Components

#### LogViewer — The Primary Interface

`LogViewer.svelte` is the most important component. It displays log file content with **virtual scrolling** for performance:

```
┌──────────────────────────────────────────┬────────────┐
│  1 │ 2024-01-01 00:00:01 Server started  │            │
│  2 │ 2024-01-01 00:00:02 Client connect  │ Extracted  │
│ ██ │ 2024-01-01 00:00:03 Players: 42     │ State      │
│  4 │ 2024-01-01 00:00:04 Region: us-east │ Panel      │
│  5 │ ...                                  │ (sidebar)  │
└──────────────────────────────────────────┴────────────┘
         ▲ scroll-spacer with translateY
```

**Virtual scrolling** works by only rendering visible lines. A tall spacer div creates the scrollbar, and visible lines are positioned with `translateY`. The scroll handler updates `scrollTop`, which triggers `$derived` recalculation of `startIdx`, `endIdx`, and `visibleLines`. An `OVERSCAN` of 10 lines prevents flicker during fast scrolling.

**Highlight-to-rule**: When the user selects text with the mouse, `onMouseUp` captures the selection and shows a floating "Create Rule from Selection" button. Clicking it opens the `RuleCreator` modal with the selected text pre-filled.

**Match highlighting**: After analysis runs, `ruleMatches` are passed as props. The `lineMatchMap` derived value builds a `Map<lineIndex, matches[]>` for O(1) lookup. Matched lines get colored backgrounds and a left border, with colors cycled per rule (6 colors defined as CSS variables `--rule-color-0` through `--rule-color-5`).

**State panel**: Clicking a matched line opens a sidebar showing the extracted state key-value pairs for that match.

#### RuleCreator — Highlight-to-Rule Flow

When you select text in the LogViewer:

1. The selected text and the source's `template_id` are passed to `RuleCreator`
2. The backend `suggest-rule` API (`POST /api/projects/:pid/suggest-rule`) generates a regex pattern with named capture groups. If the API call fails, a client-side fallback escapes the text and replaces numbers with `(\d+)` capture groups
3. `detectGroups()` parses the regex to find capture groups (supports both JS `(?<name>...)` and Rust/Python `(?P<name>...)` syntax)
4. Each group becomes an extraction rule row where you name the state key, pick the type (Parsed/Static/Clear), and the mode (Replace/Accumulate)
5. A live preview shows what the regex matches against the original text (Rust `(?P<>)` syntax is transparently converted to JS `(?<>)` for the browser preview)
6. A **ruleset picker** shows rulesets filtered to those matching the source's template. If exactly one matches, it's auto-selected
7. On save, the rule is created via the API and — if a ruleset is selected — automatically appended to that ruleset's `rule_ids` so it takes effect on the next analysis run

#### PatternEditor — Building Detection Patterns

The PatternEditor lets you build ordered predicate sequences. Each predicate row has:
- **Source** dropdown (populated from project sources)
- **State key** text input
- **Operator** dropdown (Eq, Neq, Gt, Lt, Gte, Lte, Contains, Exists)
- **Operand** toggle between Literal (type a value) and StateRef (pick source + key)
- **Move up/down** buttons for reordering

#### AnalysisView — Running and Viewing Results

The AnalysisView streams results over a WebSocket connection (`/api/projects/:pid/analyze/ws`). Events arrive incrementally — rule matches, pattern matches, and progress updates — and are buffered into batched UI updates every 100ms. A live progress counter shows lines processed during analysis. Results are displayed as:
- Summary cards (N rule matches, M pattern matches)
- Per-source LogViewer instances with match highlighting
- Pattern match cards with full state snapshots showing the state of every source at match time

**Live re-evaluation:** When rules, rulesets, or patterns are modified in any editor view, AnalysisView automatically re-runs analysis after a 500ms debounce. In-flight runs are cancelled before starting a new one. This is coordinated via a shared module-scoped invalidation counter (`analysisInvalidation.svelte.ts`) that persists across component mount/unmount cycles.

### Routing and Navigation

The app uses **state-based routing** (no router library). `App.svelte` maintains a `currentView` state variable and renders the active component with `{#if}/{:else if}` blocks. The sidebar navigation shows project-scoped views only when a project is selected.

### Styling

The UI uses a **Tokyo Night**-inspired dark theme defined entirely through CSS custom properties in `app.css`. Key variables:
- `--bg`, `--bg-secondary`, `--bg-tertiary` for layered backgrounds
- `--accent` (blue) for interactive elements
- `--green`, `--red`, `--yellow`, `--purple`, `--cyan` for semantic colors
- `--font-mono` for log content (JetBrains Mono > Fira Code > Cascadia Code > Consolas)
- `--rule-color-0` through `--rule-color-5` for match highlighting with transparent backgrounds

All component styles are **scoped** using Svelte's `<style>` blocks — CSS is automatically scoped to the component at compile time, so there are no class name collisions.

### API Client

`api.ts` exports namespace objects (`projects`, `timestampTemplates`, `templates`, `sources`, `rules`, `rulesets`, `patterns`, `analysis`) each with typed methods (`list`, `get`, `create`, `update`, `delete`). A shared `request<T>()` helper handles JSON serialization, error responses, and the base URL.

The Vite dev server proxies `/api` requests to `http://localhost:3000` (configured in `vite.config.ts`), so the frontend can use relative paths in development. In production, the backend serves the frontend directly.

---

## REST API Reference

All endpoints are scoped under `/api/`. The server runs on port 3000 (configurable via `PORT` env var).

| Method | Path | Description |
|--------|------|-------------|
| **Projects** | | |
| GET | `/api/projects` | List all projects |
| POST | `/api/projects` | Create project |
| GET | `/api/projects/:id` | Get project |
| PUT | `/api/projects/:id` | Update project |
| DELETE | `/api/projects/:id` | Delete project (cascades) |
| **Timestamp Templates** | | |
| GET | `/api/projects/:pid/timestamp-templates` | List timestamp templates |
| POST | `/api/projects/:pid/timestamp-templates` | Create timestamp template |
| GET | `/api/projects/:pid/timestamp-templates/:id` | Get timestamp template |
| PUT | `/api/projects/:pid/timestamp-templates/:id` | Update timestamp template |
| DELETE | `/api/projects/:pid/timestamp-templates/:id` | Delete timestamp template |
| **Source Templates** | | |
| GET | `/api/projects/:pid/templates` | List templates |
| POST | `/api/projects/:pid/templates` | Create template |
| GET | `/api/projects/:pid/templates/:id` | Get template |
| PUT | `/api/projects/:pid/templates/:id` | Update template |
| DELETE | `/api/projects/:pid/templates/:id` | Delete template |
| **Sources** | | |
| GET | `/api/projects/:pid/sources` | List sources |
| POST | `/api/projects/:pid/sources` | Create source |
| GET | `/api/projects/:pid/sources/:id` | Get source |
| DELETE | `/api/projects/:pid/sources/:id` | Delete source |
| POST | `/api/projects/:pid/sources/:id/upload` | Upload log file (multipart) |
| **Rules** | | |
| GET | `/api/projects/:pid/rules` | List rules (with match/extraction rules) |
| POST | `/api/projects/:pid/rules` | Create rule |
| GET | `/api/projects/:pid/rules/:id` | Get rule |
| PUT | `/api/projects/:pid/rules/:id` | Update rule |
| DELETE | `/api/projects/:pid/rules/:id` | Delete rule |
| **Rulesets** | | |
| GET | `/api/projects/:pid/rulesets` | List rulesets (with rule IDs) |
| POST | `/api/projects/:pid/rulesets` | Create ruleset |
| GET | `/api/projects/:pid/rulesets/:id` | Get ruleset |
| PUT | `/api/projects/:pid/rulesets/:id` | Update ruleset |
| DELETE | `/api/projects/:pid/rulesets/:id` | Delete ruleset |
| **Patterns** | | |
| GET | `/api/projects/:pid/patterns` | List patterns (with predicates) |
| POST | `/api/projects/:pid/patterns` | Create pattern |
| GET | `/api/projects/:pid/patterns/:id` | Get pattern |
| PUT | `/api/projects/:pid/patterns/:id` | Update pattern |
| DELETE | `/api/projects/:pid/patterns/:id` | Delete pattern |
| **Import/Export** | | |
| GET | `/api/projects/:pid/export` | Export project config (JSON download) |
| POST | `/api/projects/:pid/import` | Import project config (with ID remapping) |
| **Analysis** | | |
| POST | `/api/projects/:pid/analyze` | Run full analysis (batch JSON) |
| GET | `/api/projects/:pid/analyze/ws` | Run analysis (WebSocket streaming) |
| POST | `/api/projects/:pid/detect-template` | Auto-detect timestamp format |
| POST | `/api/projects/:pid/suggest-rule` | Suggest regex from text |

### Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `sqlite:logium.db?mode=rwc` | SQLite connection string |
| `PORT` | `3000` | Server port |
| `UPLOADS_DIR` | `./uploads` | Directory for uploaded log files |

---

## Domain Model

### The Entity Hierarchy

```
Project
  ├── TimestampTemplate[]        "how to parse timestamps"
  │     └── (format, extraction_regex?, default_year?)
  ├── SourceTemplate[]           "how to read this type of log"
  │     └── (timestamp_template_id, line_delimiter, content_regex, continuation_regex?, json_timestamp_field?)
  ├── Source[]                   "an actual log file"
  │     └── (name, template_id, file_path)
  ├── LogRule[]                  "what to look for, what state to produce"
  │     ├── MatchRule[]          regex patterns (Any/All mode)
  │     └── ExtractionRule[]     state mutations (Parsed/Static/Clear, Replace/Accumulate)
  ├── Ruleset[]                  "which rules apply to which template"
  │     └── (template_id, rule_ids[])
  └── Pattern[]                  "what failure case to detect"
        └── PatternPredicate[]   ordered conditions (source, key, operator, operand)
```

### StateValue

The engine uses a tagged union for state values:

```rust
enum StateValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
}
```

Supports cross-type numeric comparison (`Integer(10) == Float(10.0)`) and type-appropriate accumulation (strings concatenate, numbers add).

### Operators

`Eq`, `Neq`, `Gt`, `Lt`, `Gte`, `Lte` — standard comparisons using `PartialOrd`
`Contains` — substring check for strings
`Exists` — checks if the state key is present (ignores operand)

---

## Test Suite

Tests are spread across three layers:

- **logium-core unit tests**: Rule matching, state mutations, pattern evaluation, K-way merge, multi-line continuation, streaming analysis
- **logium-core integration tests**: Per-format parsing (Zookeeper, Nginx, Syslog), cross-source analysis, template reuse, state references, multi-line log entries
- **logium-server tests**: Database CRUD, import/export round-trips, analysis helpers, seed data

Test fixtures include real-world log formats in `crates/logium-core/tests/fixtures/` (downloaded via `scripts/setup_test_data.sh`). All server tests use in-memory SQLite for isolation.

Run benchmarks with `./scripts/run_benchmark.sh` — results are saved with timestamps to `benchmark/results/`.

---

## Future Work (Phase 2)

- **Click-to-navigate** — click timeline events to scroll to the corresponding log line in LogViewer
- **Source swimlanes** — per-source state evolution over time on the timeline
- **Live log sources** — file watching / stdin streaming for tailing live logs
- See `docs/plan_v2.md` for the full design document
