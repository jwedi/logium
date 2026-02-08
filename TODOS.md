# Logium V2 Gaps

Outstanding items from the V2 design doc (`docs/plan_v2.md`) that are not yet implemented.

---

## 1. WebSocket Streaming for Analysis Results

**Status:** In progress (this branch)

The design doc specifies `HTTP / WS` communication between UI and server, and `logium-server` is described as an "Axum HTTP/WebSocket server." Currently, analysis runs synchronously via `POST /api/projects/{pid}/analyze` and returns a single JSON blob. WebSocket streaming would let the frontend display results incrementally and show progress.

**Implementation:** Add `AnalysisEvent` enum + `analyze_streaming()` to logium-core, WebSocket route on the server, and streaming UI updates in the frontend.

---

## 2. Rule Creation by Highlighting

The design doc's "Viewer-First" UI paradigm specifies:
> The user highlights text in a log line and creates a rule directly from the selection. Logium suggests regex patterns and lets the user refine match and extraction rules in-place.

Currently, rules are created through a separate form UI. There is no highlight-to-rule flow in the log viewer.

**Implementation:** Add text selection handling in `LogViewer.svelte`, surface the `suggest-rule` API endpoint on selection, and provide an inline rule editor popover.

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
