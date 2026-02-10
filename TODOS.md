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

Keep the last analysis result in memory so switching tabs doesn't lose it.

- Store the most recent analysis result in a module-scoped `$state` (same pattern as `analysisInvalidation.svelte.ts`)
- When `AnalysisView` remounts, restore the cached result instead of showing an empty state
- Clear the cache on a new manual run, auto-rerun, or project change
- Frontend only

**Implementation:** Create `analysisCache.svelte.ts` exporting `getAnalysisCache()` / `setAnalysisCache()` backed by module-scoped `$state`. `AnalysisView` writes to the cache after each successful run and reads from it on mount.

---

## 11. State Evolution View

**Status:** Done

`StateChange` struct in logium-core captures old→new state diffs during `apply_mutations()`. Emitted as `AnalysisEvent::StateChange` through both `analyze()` and `analyze_streaming()`. Frontend `StateEvolutionView` component renders a filterable table (by source and state key) showing timestamp, source, key, old→new values, and triggering rule. Accessible via "State Evolution" tab in AnalysisView.
