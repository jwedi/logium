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

The design doc's Phase 2 deliverables include:
> - Visual timeline component rendering log events and pattern matches on a time axis.
> - Interactive features: zoom, pan, click-to-navigate-to-log-line.
> - Source swimlanes showing per-source state evolution over time.

The current `TimelineView` has basic zoom/pan and swimlanes but does not support click-to-navigate-to-log-line or per-source state evolution display.

**Implementation:** Add click handlers on timeline events that scroll/highlight the corresponding log line in `LogViewer`, and add a state evolution track showing extracted state values over time per swimlane.

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

Allow editing existing rules inline in `RuleList`.

- "Edit" button on each rule row opens an inline edit form (same fields as create: name, match_mode, match patterns, extraction rules)
- Calls existing `PUT /api/projects/:pid/rules/:id` endpoint (already implemented server-side)
- Triggers `invalidateAnalysis()` on save so results stay in sync
- Frontend only — no backend changes needed

**Implementation:** Add an `editingRuleId` state to `RuleList`. When set, render inline inputs pre-filled with current values. On save, `PUT` the updated rule, reset `editingRuleId`, refetch the rule list, and call `invalidateAnalysis()`.

---

## 8. Project Import/Export

Export a project's configuration as JSON (templates, rules, rulesets, patterns) and import it into another project.

- "Export Project Config" button → downloads a JSON file containing all source templates, timestamp templates, rules, rulesets, and patterns (no log file data)
- "Import Project Config" button → file picker for a JSON file; creates all entities with ID remapping to avoid conflicts
- Backend: new `GET /api/projects/:pid/export` and `POST /api/projects/:pid/import` endpoints
- Frontend: buttons in project settings + file picker dialog

**Implementation:** Backend export endpoint queries all related entities and serializes them. Import endpoint deserializes, creates entities in dependency order (timestamp templates → source templates → rules → rulesets → patterns), maps old IDs to new IDs for foreign-key references. Frontend adds export/import buttons with fetch + Blob download / FileReader upload.

---

## 9. Rule Testing / Dry Run

Test a rule against a pasted log line without running a full analysis.

- "Test" button on each rule in `RuleList`
- Opens a text input area — paste a log line, see whether the rule matches and what state would be extracted
- Client-side regex evaluation reusing the `(?P<>)` → `(?<>)` named-group conversion already in `RuleCreator`
- Frontend only — no backend round-trip needed

**Implementation:** Add a `RuleTester` component. Convert the rule's patterns to JS-compatible regexes, run them against the input text, and display: match result (yes/no), matched substring highlight, and extracted named groups as a key-value table.

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

A dedicated view showing how extracted state changes over time across sources.

- New tab or panel in `AnalysisView`: "State Evolution"
- Table columns: timestamp | source | key | old_value → new_value
- Filterable by source and state key
- Backend: the engine already computes state mutations internally — expose them as `StateChange` events in the analysis output
- Frontend: new `StateEvolutionView` component consuming the events

**Implementation:** In logium-core, emit `AnalysisEvent::StateChange { timestamp, source, key, old_value, new_value }` alongside existing match events. Pipe through WebSocket. Frontend renders a sortable/filterable table with the state diff data.
