# Logium UI

Svelte 5 + TypeScript + Vite frontend for Logium, a log analysis tool that detects known failure cases by correlating events across multiple log sources.

## Getting Started

```bash
npm install
npm run dev    # dev server with HMR
npm run build  # production build to dist/
```

The dev server proxies API requests to the Rust backend on port 3000.

## Architecture

The UI is a single-page app with client-side routing. Key views:

| Component | Purpose |
|-----------|---------|
| `ProjectManager.svelte` | Project CRUD, top-level navigation |
| `SourceManager.svelte` | Manage log sources and file uploads |
| `TemplateManager.svelte` | Configure source templates and timestamp templates |
| `RuleList.svelte` / `RuleCreator.svelte` | Manage log rules (match + extraction) |
| `RulesetManager.svelte` | Group rules into rulesets |
| `PatternEditor.svelte` | Define cross-source failure patterns |
| `AnalysisView.svelte` | Run analysis, view results in Table or Timeline mode |
| `LogViewer.svelte` | Virtualized log file viewer with rule match highlighting |

## Analysis Views

After running analysis, results can be viewed in two modes via a tab bar:

### Table View

The original flat view: source selector, virtualized LogViewer with match highlighting, pattern match cards, and a rule match table.

### Timeline View

A graphical timeline visualizing events on a time axis with per-source swimlanes.

**Components:**

```
AnalysisView.svelte  (tab bar: Table | Timeline)
  └─ TimelineView.svelte        (orchestrator: zoom/pan state, data transform)
       ├─ TimelineAxis.svelte    (SVG time axis with adaptive ticks)
       ├─ TimelineSwimlane.svelte (one SVG column per source: event dots + clustering)
       └─ TimelineDetailPanel.svelte (right panel: event details on click)
```

**How it works:**

1. **Data transform** -- `AnalysisResult` is transformed into `TimelineEvent[]` via `$derived.by()`. Timestamps are parsed from ISO strings, events are grouped by source into sorted `SourceLane[]` arrays.
2. **Coordinate system** -- A `msPerPixel` ratio maps timestamps to Y pixel positions. Total virtual height scales with zoom level.
3. **Virtual rendering** -- Each swimlane binary-searches its sorted events to find the visible range (O(log n)). Events within 3px are clustered into count-badged dots.
4. **Adaptive axis** -- Tick intervals (100ms to 1h) auto-select targeting ~60px spacing. Format adapts from `HH:MM:SS.mmm` to `HH:MM` based on zoom.
5. **Zoom/Pan** -- Pan via native scroll. Zoom via Ctrl/Cmd+scroll wheel (0.1x-50x), anchored on cursor position.
6. **Click-to-inspect** -- Clicking an event dot opens a 300px detail panel showing rule match info (rule name, log line, extracted state) or pattern match info (state snapshot across sources).

## Styling

Uses a Tokyo Night-inspired dark theme defined in `app.css`. Key design tokens:

- 6 rule colors (`--rule-color-N` / `--rule-border-N`) for distinguishing rules
- `--purple` for pattern matches
- `--cyan` for source names
- `--font-mono` (JetBrains Mono) for code/data, `--font-sans` for UI

## API

All API calls go through `api.ts`, which provides typed clients for projects, sources, templates, rules, rulesets, patterns, and analysis. The backend serves at `/api/`.
