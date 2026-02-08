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

The design doc states:
> As rules and patterns are defined, the viewer immediately highlights matching log lines and shows state changes. Pattern matches appear inline in the timeline.

Currently, analysis must be explicitly triggered via the "Run Analysis" button. Rule/pattern changes do not trigger automatic re-evaluation.

**Implementation:** Watch for rule/pattern mutations via the existing CRUD APIs (or WebSocket push), automatically re-run analysis on changes, and incrementally update the viewer.

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
