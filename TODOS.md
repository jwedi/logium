# Logium V2 Gaps

Outstanding items from the V2 design doc (`docs/plan_v2.md`) that are not yet implemented.

---

## 1. WebSocket Streaming for Analysis Results

**Status:** Done

Implemented `AnalysisEvent` enum + `analyze_streaming()` in logium-core, WebSocket route (`GET /api/projects/:pid/analyze/ws`) on the server, and streaming UI updates in the frontend with buffered rendering and a live progress counter.

---

## 2. Rule Creation by Highlighting

**Status:** Done

The user highlights text in a log line → `RuleCreator` modal opens → the backend `suggest-rule` API generates a regex pattern (with fallback to client-side escaping) → the user refines the pattern, names the rule, and picks a ruleset → on save the rule is created and assigned to the selected ruleset so it takes effect on the next analysis run.

---

## 3. Real-Time Feedback / Live Re-evaluation

**Status:** Done

A module-scoped invalidation counter (`analysisInvalidation.svelte.ts`) is incremented after every successful CRUD operation in RuleList, RulesetManager, PatternEditor, and RuleCreator. AnalysisView watches the counter via `$effect` and auto-reruns analysis after a 500ms debounce, with cancellation of any in-flight streaming run. The button shows "Re-analyzing..." during auto-triggered runs.

---

## 4. Interactive Timeline (Phase 2)

**Status:** Done

Click-to-navigate: clicking a timeline event in the detail panel shows a "Go to line" button that switches to the table view, selects the correct source, and scrolls `LogViewer` to the matching log line. Per-source state evolution: each swimlane dot shows extracted state as an SVG tooltip on hover, and compact `key=value` labels alongside dots when zoomed in (hidden when `msPerPixel ≥ 50` to avoid clutter).

---

## 5. Stream/Live Log Source Support

The design doc mentions sources can be a `path` or `stream`:
> A concrete log source (file or stream) associated with a source template.

Currently, only file paths are supported. There is no mechanism for streaming/tailing live log sources.

**Implementation:** Add a file-watching mode (e.g., `notify` crate) or accept stdin/WebSocket-pushed log lines, feed new lines into the engine incrementally, and push new matches to the frontend via WebSocket.

---

## 6. Search / Grep in LogViewer

Search bar in `LogViewer` that lets the user find text across loaded log lines.

- Ctrl+F opens a search bar overlay inside `LogViewer`
- Supports plain-text and regex modes (toggle button)
- Jump between matches with prev/next buttons; show match counter ("3 of 47")
- Highlight all matches visible in the virtual-scroll viewport
- Frontend only — log content is already loaded client-side

**Implementation:** Add a `LogSearch` component rendered inside `LogViewer`. On input, scan the in-memory log lines, collect match indices, and pass them to the virtual-scroll renderer for highlighting. Prev/next buttons update a `currentMatchIndex` and scroll the viewport to that line.

---

## 7. Rule Editing

**Status:** Done

"Edit" button on each rule in `RuleList` opens an inline `RuleEditor` component with all fields (name, match_mode, match patterns, extraction rules). Calls `PUT /api/projects/:pid/rules/:id` on save and triggers `invalidateAnalysis()`. Shared regex helpers extracted to `regexUtils.ts` and reused by both `RuleCreator` and `RuleEditor`.

---

## 8. Project Import/Export

**Status:** Done

Export/import a project's configuration (timestamp templates, source templates, rules, rulesets, patterns) as a versioned JSON file. `GET /api/projects/:pid/export` returns a downloadable JSON with `Content-Disposition: attachment`. `POST /api/projects/:pid/import` accepts the JSON and creates entities in dependency order with full ID remapping. Frontend adds per-project Export and Import buttons in ProjectManager.

---

## 9. Rule Testing / Dry Run

**Status:** Done

Integrated into `RuleEditor` as a "Test Rule (dry run)" section. Paste a log line to see per-pattern match/no-match indicators, overall verdict respecting match mode (Any/All), and extraction preview showing captured values (Parsed), static values, and cleared keys. Client-side only — uses `regexUtils.ts` helpers with `(?P<>)` → `(?<>)` conversion.

---

## 10. Persistent Analysis Results

**Status:** Done

Module-scoped `$state` cache (`analysisCache.svelte.ts`) keyed by `projectId`. `AnalysisView` restores cached results on mount/remount, writes to the cache after each successful streaming run, and naturally clears stale results when the project changes (cache miss returns null).

---

## 11. State Evolution View

**Status:** Done

`StateChange` struct in logium-core captures old→new state diffs during `apply_mutations()`. Emitted as `AnalysisEvent::StateChange` through both `analyze()` and `analyze_streaming()`. Frontend `StateEvolutionView` component renders a filterable table (by source and state key) showing timestamp, source, key, old→new values, and triggering rule. Accessible via "State Evolution" tab in AnalysisView.

---

## 12. Ad-hoc Log Filtering

**Status:** Done

A filter bar in LogViewer that hides non-matching lines without creating a persistent rule. Type a string or regex, and the view immediately shows only matching lines. Filter bar is always visible with text/regex toggle, "N of M lines" counter, and clear button. Search (Ctrl+F) operates within filtered results. Virtual scroll uses `filteredIndices` indirection to preserve original line numbers.

---

## 13. Time-Range Filtering

**Status:** Done

Optional `TimeRange { start, end }` struct in `logium-core::engine` filters lines in both `analyze()` and `analyze_streaming()` — lines before `start` are skipped, lines after `end` break early (leveraging chronological K-way merge order). Server routes accept `?start=...&end=...` query parameters on both POST and WebSocket analysis endpoints. Frontend `AnalysisView` adds `datetime-local` pickers with a Clear button. LogViewer is not filtered (shows full file, only engine-matched lines are highlighted). 4 engine unit tests + 3 server parsing tests added.

---

## ~~14. Structured Log Support (JSON Lines)~~

**Status:** Done

Added `json_timestamp_field` to `SourceTemplate`. When set, `LogLineIterator` parses lines as JSON and extracts the timestamp from the configured field (bypassing `content_regex` and `extraction_regex`). The analysis loop auto-extracts all top-level JSON keys as state fields (Replace mode) before rule processing — no manual extraction rules needed. Integers, floats, booleans, and strings are auto-typed. The `detect-template` endpoint detects JSON samples and suggests common timestamp field names (`timestamp`, `ts`, `@timestamp`, `time`, `datetime`). DB schema, API, and frontend updated. 6 new tests (3 unit, 2 integration, 1 server detect-template).

---

## 15. Context Lines Around Matches

**Status:** Done

Click-to-expand on rule-matched lines in LogViewer to show N surrounding context lines (like `grep -C`). Expand/collapse individual matches or all at once. Context lines are visually distinct (dimmed, dashed border). Configurable context size (default 5). Filter count shows base matches, not context lines. Gap separators between non-consecutive groups.

---

## ~~16. Multi-line Log Entry Support~~

**Status:** Done

Added `continuation_regex` field to `SourceTemplate`. When set, `LogLineIterator` merges physical lines matching the regex into the preceding logical entry. Timestamps are parsed from the head line only. Content and raw fields span all merged lines (joined by `\n`). Continuation-aware iteration uses a `pending_line` buffer for lookahead. DB schema, API, and frontend updated. 4 new tests (2 unit, 2 integration) covering multi-line parsing, cross-source matching on merged entries, and passthrough behavior when `continuation_regex` is `None`.

---

## 17. Result Filtering / Faceted Browsing

**Status:** Done

Clickable facet chips in the results summary card for filtering by rule and source. Single-select toggle (click to filter, click again to clear). A centralized `filteredResult` derived value filters `result` by active filters and is passed to all child views (table, timeline, state evolution). Summary stats show unfiltered totals; source button match counts reflect filtered results. "Showing X of Y matches" status with "Clear filters" button when filters are active. Filters reset automatically on new analysis runs.

---

## 18. Request/Transaction Tracing

After extracting a request ID via a rule, click it to filter all sources to lines containing that ID. The core distributed debugging task: "show me everything that happened for request `abc123` across all services."

- Click an extracted state value to use it as a cross-source filter
- Shows all log lines across all sources matching the identifier
- Essentially a cross-source join on a common field

**Inspiration:** Grafana Loki's derived fields, Datadog's trace correlation, Honeycomb's trace view.

---

## 19. Aggregation / Statistics View

A stats panel showing match counts per rule, matches over time, and top extracted values by frequency. Helps answer "what are the top 10 most frequent error types?" or "how many timeouts per minute?"

- Match counts per rule (bar chart)
- Matches over time (histogram)
- Top extracted values by frequency
- Accessible as a new tab alongside Table/Timeline/State Evolution

**Inspiration:** Kibana Lens, Splunk's `stats` and `timechart` commands, Datadog's Log Analytics.

---

## 20. Analysis Result Export

Export analysis results (matches, state changes, pattern matches) as JSON or CSV. Results currently only exist in the browser session — can't share with teammates or save for later comparison.

- Download button on the results view
- JSON and CSV format options
- Includes rule matches, pattern matches, and state changes

**Inspiration:** Kibana CSV export, Splunk export, Datadog download.

---

## 21. Compressed File Support

Transparently decompress `.gz`/`.bz2`/`.zst` files in `LogLineIterator`. Archived logs are commonly compressed — users currently must manually decompress before loading.

- Detect compression by file extension
- Use `flate2`/`bzip2`/`zstd` crates for decompression
- Streaming decompression (no temp files)

**Inspiration:** lnav handles gzip/bzip2 transparently.

---

## 22. Bookmarks / Annotations

Click to bookmark a log line during investigation. Show a bookmarks sidebar to jump between marked lines, optionally with text notes.

- Bookmark toggle on each log line
- Bookmarks sidebar with jump-to navigation
- Optional text annotation per bookmark
- Frontend only — bookmarks are session-scoped

**Inspiration:** lnav's bookmark feature, IDE breakpoints.

---

## 23. Log Level / Severity Awareness

Extract severity as a built-in field via source template config or auto-detection. Add severity filter toggles to LogViewer and color-code lines by level.

- Configurable severity extraction (regex or JSON field)
- Filter toggles: ERROR, WARN, INFO, DEBUG, TRACE
- Color-coded lines by severity level

**Inspiration:** lnav color-codes by level, Kibana has level facets.

---

## 24. Automatic Error Clustering

**Status:** Done

Drain-inspired tokenization in `logium-core::engine::cluster_logs()`: splits log lines on whitespace, replaces variable tokens (numbers, IPs, UUIDs, hex, timestamps, paths, quoted strings) with `<*>`, groups by structural template. Streaming via `MergedLogStream` (never loads full files). `POST /api/projects/:pid/cluster` endpoint in `logium-server`. Frontend `ErrorClusteringView` component as a 4th tab in AnalysisView: shows clusters sorted by frequency with count badges, source chips, expand-for-samples, and "Create Rule" button (opens `RuleCreator` with a sample line). Source filter chips to narrow by source. 8 Rust unit tests (tokenizer + cluster_logs), 1 server integration test, 6 frontend component tests.

---

## 25. Diff Between Analysis Runs

Save analysis results and diff two runs: new matches, disappeared matches, state changes that differ. Useful for "this worked yesterday but not today" scenarios.

- Save/name analysis result snapshots
- Side-by-side or unified diff view
- Highlight new, removed, and changed matches

**Inspiration:** Splunk's compare time ranges, general diff tooling.

---

## 26. Event Density Histogram

Small histogram above LogViewer showing event/match density over time. Click a time bucket to jump to that region. Helps visually identify "when did things go wrong."

- Time-bucketed bar chart of match counts
- Click a bucket to scroll LogViewer to that time
- Updates live during streaming analysis

**Inspiration:** Kibana Discover's histogram, Grafana's log volume chart.

---

## 27. CLI for AI Agents and Pipelines

Add a `logium` CLI binary so AI agents and CI/CD pipelines can run ad-hoc log analysis without the web UI. The CLI is stateless and single-shot: take a JSON config with everything needed (rules, templates, sources, patterns), run analysis, return JSON results. No SQLite, no persistence — just `stdin → analyze → stdout`.

**Design principles** (inspired by OpenClaw's CLI-first Skills pattern, `gh`, `ripgrep`):
- JSON on stdout — the machine-readable contract
- Stderr for progress, warnings, errors (never pollute stdout)
- Non-zero exit codes with structured error JSON on failure
- Composable with `jq` and other CLI tools

**Two commands only:**

```
# Export a project's config (rules, templates, patterns) as JSON
logium export <project-id>

# Run analysis from a self-contained JSON config
logium analyze -c config.json
logium analyze < config.json   # or via stdin
```

**Input format** — the existing `ProjectExport` shape plus a `sources` array:

```json
{
  "version": 1,
  "timestamp_templates": [...],
  "source_templates": [...],
  "rules": [...],
  "rulesets": [...],
  "patterns": [...],
  "sources": [
    { "id": 1, "name": "app", "file_path": "/var/log/app.log", "source_template_id": 1 }
  ]
}
```

**Output** — `AnalysisResult` as JSON on stdout:

```bash
logium analyze -c config.json | jq '.rule_matches[] | select(.rule_id == 3)'
```

**Agent workflow:**
1. `logium export 1 > config.json` — grab rules/templates from an existing project
2. Edit `config.json` to add `sources` with file paths to analyze
3. `logium analyze -c config.json` — run analysis, parse JSON output
4. Or construct the entire config from scratch (no project needed)

**Built-in help with full examples:**

`logium analyze --help` should include a worked example showing the minimum JSON needed for a basic analysis, so a new user can get started without reading docs:

```
EXAMPLES:
    # Minimal config to analyze a single log file:
    cat <<'EOF' | logium analyze
    {
      "version": 1,
      "timestamp_templates": [{
        "id": 1,
        "name": "syslog",
        "format": "%b %d %H:%M:%S",
        "default_year": 2025
      }],
      "source_templates": [{
        "id": 1,
        "name": "syslog",
        "line_regex": "^(?P<timestamp>\\w+ \\d+ [\\d:]+) (?P<host>\\S+) (?P<message>.+)$",
        "timestamp_template_id": 1
      }],
      "rules": [{
        "id": 1,
        "name": "OOM Killer",
        "source_template_id": 1,
        "match_field": "message",
        "match_regex": "Out of memory",
        "ruleset_id": 1
      }],
      "rulesets": [{ "id": 1, "name": "default" }],
      "patterns": [],
      "sources": [{
        "id": 1,
        "name": "syslog",
        "file_path": "/var/log/syslog",
        "source_template_id": 1
      }]
    }
    EOF

    # Export from an existing project, add sources, and analyze:
    logium export 1 > config.json
    # edit config.json to add "sources" array
    logium analyze -c config.json
```

Use `clap`'s `after_long_help` to embed these examples so they appear in `--help` output.

**Implementation:**
- New crate: `crates/logium-cli/` with `clap` derive API
- Depends only on `logium-core` (no HTTP, no SQLite for `analyze`)
- `export` command connects to the server's SQLite DB read-only
- All model types already derive `Serialize + Deserialize` — just `serde_json::to_writer(stdout)`
- `analyze()` already takes flat slices, so JSON → deserialize → call `analyze()` → serialize result

**Inspiration:** OpenClaw's CLI-first Skills pattern ("works with agents that didn't exist when we wrote the code"), `gh --json`, `ripgrep --json`, 12 Factor CLI Apps.

---

## Performance Optimizations

Prioritized optimizations for handling large log files (hundreds of MB).

### Critical Priority

#### P1. Increase BufReader buffer size
**Status:** Done

`BufReader::with_capacity(64 * 1024, file)` replaces the default 8KB buffer in `LogLineIterator::new()`.

#### P2. Eliminate LogLine cloning in hot loop
**Status:** Done

`LogLine.raw` and `LogLine.content` changed from `String` to `Arc<str>`. Iterator construction shares a single `Arc` when `raw == content` (common case — no `content_regex`). Hot-loop `line.clone()` into `RuleMatch` is now two atomic ref bumps instead of two heap allocations+copies. Benchmarks: ~9% faster on cross-source workload, ~1% on single-source 51k lines.

#### P3. Lazy state snapshot cloning — Done
**File:** `crates/logium-core/src/engine.rs` — `StateManager`
**Change:** Wrapped per-source inner state maps in `Arc` for COW semantics. `snapshot()` now clones `Arc` pointers (O(1) per source) instead of deep-cloning inner maps. Mutations use `Arc::make_mut()` — only clones the single source's map when a snapshot holds a reference.
**Result:** Large benchmark (51k lines) improved from 93.6ms → 88.8ms (~5% faster). Cross-source within noise.

#### P4. Frontend: replace array spread with push on flush — Done
**File:** `ui/src/lib/AnalysisView.svelte`
**Change:** Replaced `result = { ...spread }` with in-place `result!.*.push(...buffer)` in all three flush sites (periodic, onComplete, onError). Svelte 5 `$state` deep reactivity triggers updates from in-place mutations — no reassignment needed.
**Result:** Flush cost reduced from O(total matches) to O(buffer size) per flush; O(n²) cumulative overhead eliminated.

#### P5. Batch DB queries in load_project_data
**File:** `crates/logium-server/src/db.rs`
**Issue:** N+1 query pattern — `build_log_rule` called per rule, `get_predicates` called per pattern.
**Fix:** Use `WHERE id IN (...)` batch queries.
**Est. impact:** 30-50+ queries → 3-5; sub-second project load.

#### P4b. Parallel analysis engine (rayon)
**Status:** Done

Two-phase parallel architecture: Phase 1 uses `rayon::par_iter` to read and evaluate rules across sources in parallel (nested `par_iter` for per-line rule evaluation within each source). Phase 2 merges results via `ProcessedLineMerger` (K-way merge over pre-processed Vecs) and applies state mutations + pattern evaluation sequentially. Benchmarks: cross-source 6.3ms → 4.4ms (1.43×), single-source 51k lines 88.8ms → 79.2ms (1.12×).

### High Priority

#### P6. Avoid JSON double-parse — Done
Added `cached_json: Option<serde_json::Value>` field to `LogLine`. The JSON branch of `LogLineIterator::next()` now stores the parsed value, and `process_source()` uses `take()` instead of re-parsing.

#### P7. Optimize parse_timestamp_prefix
**Status:** Done

Added `estimate_timestamp_len(fmt)` that computes the expected (min, max) output length of a chrono format string from its specifiers (e.g., `%Y-%m-%d %H:%M:%S` → exactly 19 chars). `parse_timestamp_prefix` now tries a narrow window around the estimate (~3-5 positions) before falling back to a full scan. For formats without `extraction_regex` (zookeeper, syslog), this reduces per-line parse attempts from O(line_length) to O(1). No impact on nginx benchmarks (which use extraction_regex and never call `parse_timestamp_prefix`). 8 new tests: 6 for estimate accuracy across common formats, 2 for prefix parsing correctness.

#### P8. Virtualize pattern matches section
**Status:** Done

Content-based height estimation virtual scroll for pattern match cards. `estimatePmHeight()` computes each card's height from its data (sources × keys), prefix-sum offsets array for O(1) position lookup, binary search for visible range with overscan of 5. Scroll container capped at 600px with `translateY` positioning. Heading shows total count. No external dependencies — same scroll-spacer + translateY pattern used by `LogViewer`.

#### P9. Fix O(n*m) findIndex in LogViewer — Done
**File:** `ui/src/lib/LogViewer.svelte`
**Change:** Added `lineContentIndex` derived Map (line content → first occurrence index) for O(1) lookups, replacing O(M) `findIndex` calls in `lineMatchMap` and navigate effect. Added `filteredIndexSet` derived Set replacing O(F) `filteredIndices.includes()`. Total cost of `lineMatchMap` reduced from O(N*M) to O(N).

#### P10. Streaming export endpoint
**File:** `crates/logium-server/src/routes/analysis.rs`
**Issue:** Export materializes the full result in memory before sending.
**Fix:** Stream results directly using Axum's streaming body.
**Est. impact:** Enables arbitrarily large exports without OOM.

### Medium Priority

#### P11. Cache source_name in hot loop
**File:** `crates/logium-core/src/engine.rs`
**Issue:** `source_name` looked up and cloned from HashMap per line.
**Fix:** Cache per-source outside the inner loop.
**Est. impact:** Minor — fewer HashMap lookups.

#### P12. Deduplicate derived filter chains
**File:** `ui/src/lib/AnalysisView.svelte`
**Issue:** `filteredResult`, `ruleBreakdown`, `sourceBreakdown` each independently filter the full result.
**Fix:** Compute the filtered result once and derive breakdowns from it.
**Est. impact:** 2-3x fewer iterations over large arrays.

#### P13. Viewport-filter timeline events
**File:** `ui/src/lib/TimelineView.svelte`
**Issue:** `allEvents` creates objects for ALL matches regardless of visible range.
**Fix:** Filter to the visible time range before creating event objects.
**Est. impact:** Less GC pressure for large analyses.

#### P14. Virtualize StateEvolutionView
**File:** `ui/src/lib/StateEvolutionView.svelte`
**Issue:** All state change rows rendered without virtualization.
**Fix:** Add virtual scrolling for the state changes list.
**Est. impact:** Prevents lag with 10k+ changes.

#### P15. WebSocket backpressure tuning
**File:** `crates/logium-server/src/routes/analysis.rs`
**Issue:** 256-item channel with `blocking_send` can stall on slow clients.
**Fix:** Consider adaptive channel capacity or dropping stale messages.
**Est. impact:** Prevents stalling on slow clients.

---

## Usability & Onboarding

Improvements to reduce time-to-first-analysis, automate manual steps, and make existing features discoverable. Currently a new user must complete ~7 manual steps before seeing their first result. These items surface existing backend automation in the UI and smooth the onboarding curve.

Suggested priority order (impact-to-effort ratio): #29 → #30 → #28 → #36 → #37 → #31 → #32 → #35 → #34 → #33.

---

### 28. Upload-First Source Creation with Auto-Detection

When a user uploads a log file in SourceManager, auto-detect the format using the existing `detect-template` endpoint (already wired in `api.ts` but never called from the UI), match against the 6 auto-seeded timestamp templates, auto-create a source template if needed, and create the source — all in one flow. Show detected format and confidence to the user. This collapses 3 manual steps (create timestamp template config → create source template → create source) into a single file upload action.

**Key files:** `ui/src/lib/SourceManager.svelte`, `ui/src/lib/api.ts` (line ~406, `detectTemplate` already defined)

---

### 29. Auto-Create Default Ruleset on Source Template Creation — Done

`create_template()` in `db.rs` now auto-inserts a "Default — {name}" ruleset. `delete_template()` cascades to delete associated rulesets. Tests updated for the new behavior.

---

### 30. Actionable Empty States with Contextual Guidance — Done

Replaced bare "No X yet" messages with `.guidance` cards across all manager components. Each card explains the entity, the next step, and prerequisites. AnalysisView now shows a setup checklist with live source/rule/ruleset counts. Added `.guidance` CSS class in `app.css`, updated SourceManager, TemplateManager, RuleList, RulesetManager, and AnalysisView. Updated test mocks and snapshot.

---

### 31. Starter Project Configs (One-Click Demo)

**Status:** Done

Bundled 3 pre-built `.logium.json` config files (Nginx, Syslog, Zookeeper) in `ui/public/starters/` with matching 50-line sample log files in `ui/public/starters/samples/`. Added a "Load Starter" dropdown button on each project card in `ProjectManager.svelte` that fetches the static JSON and imports it via `projectsApi.importConfig()`. Each config includes a timestamp template, source template, 2 rules, 1 ruleset, and 1 pattern.

---

### 32. Timestamp Template Management UI

Add a CRUD section for timestamp templates in TemplateManager (or as a collapsible panel). The 6 auto-seeded templates cover common formats but users with custom timestamp formats have no way to create new ones through the UI — the `tsTemplatesApi` CRUD is fully wired in `api.ts` but only the list endpoint is called (for the dropdown). Include a "test" input where users can paste a sample line and verify the format parses correctly.

**Key files:** `ui/src/lib/TemplateManager.svelte`, `ui/src/lib/api.ts` (lines ~196-212, `tsTemplatesApi` CRUD already defined)

---

### 33. Guided Setup Wizard for Empty Projects

When a project has zero sources and zero rules, show a 3-step wizard overlay instead of the raw tab interface:
1. **Add your log files** — drag-and-drop zone with auto-detection (from #28)
2. **Define what to look for** — show error clustering results with "Create Rule" buttons (reuse `ErrorClusteringView`), plus manual rule creation
3. **Run analysis** — single button, inline results

Wizard is skippable ("Show full UI") and hides permanently once the project has at least one source and one rule. The normal tab UI takes over after that.

**Key files:** `ui/src/App.svelte`, new `ui/src/lib/SetupWizard.svelte` component, reuses `ErrorClusteringView.svelte` and `RuleCreator.svelte`

---

### 34. Setup Progress Indicator

Show a compact progress checklist in the project header or sidebar: Templates ✓ → Sources ✓ → Rules ✗ → Analysis ✗. Helps users understand the dependency chain and what steps remain. Disappears once all steps are complete and an analysis has been run. Uses entity counts already loaded by each component.

**Key files:** `ui/src/App.svelte`

---

### 35. Source Replace (Re-associate File)

In the source management view, add a "Replace" button on each source. Clicking it opens the file picker; selecting a file re-associates the source with the new file path and discards the old one. All parse settings (source template, name, etc.) are inherited — only the `file_path` changes. This supports the common workflow of receiving a new version of the same log (e.g., rotated logs, updated captures from a teammate) without recreating the source and its template/ruleset bindings.

**Key files:** `ui/src/lib/SourceManager.svelte`, `ui/src/lib/api.ts`, `crates/logium-server/src/routes/` (PUT source endpoint already exists)

---

### 36. Default Project Bootstrap — Done

Auto-creates a "Default project" when the project list is empty on first load, and auto-selects it. Added inline rename UI to ProjectManager (Rename button → input with Save/Cancel, Enter/Escape keys). Uses the existing `PUT /api/projects/:id` endpoint — no backend changes needed.

---

### 37. Source Template Auto-Selection via File Name / Log Content Regex — Done

Added `file_name_regex` and `log_content_regex` optional fields to `SourceTemplate` across the full stack (model, DB migration, CRUD, routes, API types, UI). `SourceManager.onFileSelected()` now runs a three-phase detection: (1) file name regex match, (2) log content regex match against first 1000 lines, (3) existing `detect-template` fallback. `TemplateManager` exposes create/edit fields and card display for both regexes. New DB test verifies round-trip persistence and clearing.
