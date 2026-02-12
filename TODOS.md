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

A time picker that constrains both the analysis engine and the LogViewer to a start/end timestamp window. When debugging, you usually know approximately when the issue occurred — currently you must process the entire file.

- Time picker UI (manual entry or click-to-select from timeline)
- Pass time bounds to `analyze_streaming()` on the backend to skip lines outside the window
- LogViewer also respects the time range when browsing raw content

**Inspiration:** Kibana's time picker, Datadog's time selector, Grafana's dashboard time range.

---

## 14. Structured Log Support (JSON Lines)

Auto-detect JSON log lines and make all top-level keys available as state fields without manual extraction rules. Modern services emit JSON logs — extracting fields with regex is tedious when the data is already structured.

- A JSON source template type that parses lines as JSON instead of using regex extraction
- Auto-detect JSON lines (starts with `{`) and surface all top-level keys
- Timestamp field configurable (e.g. `timestamp`, `ts`, `@timestamp`)

**Inspiration:** Kibana auto-indexes all JSON fields, Seq preserves structured data natively, lnav detects JSON lines.

---

## 15. Context Lines Around Matches

**Status:** Done

Click-to-expand on rule-matched lines in LogViewer to show N surrounding context lines (like `grep -C`). Expand/collapse individual matches or all at once. Context lines are visually distinct (dimmed, dashed border). Configurable context size (default 5). Filter count shows base matches, not context lines. Gap separators between non-consecutive groups.

---

## 16. Multi-line Log Entry Support

Stack traces, JSON payloads, and multi-line messages span multiple lines. The current line-by-line model splits these into separate entries, breaking timestamp parsing and rule matching.

- A `continuation_regex` on SourceTemplate — lines not matching the timestamp pattern are appended to the previous entry
- Common heuristic: if a line doesn't start with a timestamp, it's a continuation
- Merged entries are treated as a single LogLine for rule matching

**Inspiration:** Filebeat's `multiline` config, lnav's multi-line detection, Logstash's multiline codec.

---

## 17. Result Filtering / Faceted Browsing

Filter controls on analysis results: by rule, by source, by state key, by time range. After analysis produces hundreds of matches, there's no way to slice them.

- Click a rule name in the summary to filter to those matches
- Filter dropdowns for source, rule, state key
- Time-range sub-filter on results
- Applies across table view, timeline, and state evolution

**Inspiration:** Kibana's faceted filtering, Datadog's facets panel, Splunk's field sidebar.

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

Auto-cluster log lines by similarity (tokenize, group by template). Show the top N clusters with counts. Click a cluster to create a rule from it.

- Tokenize log lines and group by structure (e.g., Drain algorithm)
- Show cluster list sorted by frequency
- Click-to-create-rule from a cluster template

**Inspiration:** Datadog Log Patterns, Elasticsearch log categorization, Drain algorithm.

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
