<script lang="ts">
  import type {
    AnalysisResult,
    Source,
    LogRule,
    Pattern,
    RuleMatch,
    PatternMatch,
    StateValue,
  } from './api';
  import TimelineAxis from './TimelineAxis.svelte';
  import TimelineSwimlane from './TimelineSwimlane.svelte';
  import TimelineDetailPanel from './TimelineDetailPanel.svelte';

  interface TimelineEvent {
    id: number;
    type: 'rule' | 'pattern';
    timestamp: number;
    sourceId: number | null;
    ruleId?: number;
    patternId?: number;
    ruleMatch?: RuleMatch;
    patternMatch?: PatternMatch;
    colorIndex: number;
  }

  interface SourceLane {
    sourceId: number;
    sourceName: string;
    events: TimelineEvent[];
  }

  let {
    result,
    sourceList,
    ruleList,
    patternList,
    onNavigate,
  }: {
    result: AnalysisResult;
    sourceList: Source[];
    ruleList: LogRule[];
    patternList: Pattern[];
    onNavigate?: (sourceId: number, rawLine: string) => void;
  } = $props();

  const BASE_HEIGHT = 2000;
  const MIN_ZOOM = 0.1;
  const MAX_ZOOM = 50;

  let zoom = $state(1);
  let scrollTop = $state(0);
  let viewportHeight = $state(600);
  let viewportWidth = $state(800);
  let scrollContainer: HTMLDivElement | undefined = $state();
  let selectedEvent: TimelineEvent | null = $state(null);
  let detailPanelEl: HTMLDivElement | undefined = $state();

  function getSourceName(id: number): string {
    return sourceList.find((s) => s.id === id)?.name ?? `Source #${id}`;
  }

  function getPatternName(id: number): string {
    return patternList.find((p) => p.id === id)?.name ?? `Pattern #${id}`;
  }

  // Transform analysis result into timeline events
  let allEvents = $derived.by(() => {
    const events: TimelineEvent[] = [];
    let id = 0;

    for (const rm of result.rule_matches) {
      const ts = Date.parse(rm.log_line.timestamp + 'Z');
      if (!isNaN(ts)) {
        events.push({
          id: id++,
          type: 'rule',
          timestamp: ts,
          sourceId: rm.source_id,
          ruleId: rm.rule_id,
          ruleMatch: rm,
          colorIndex: rm.rule_id % 6,
        });
      }
    }

    for (const pm of result.pattern_matches) {
      const ts = Date.parse(pm.timestamp + 'Z');
      if (!isNaN(ts)) {
        events.push({
          id: id++,
          type: 'pattern',
          timestamp: ts,
          sourceId: null,
          patternId: pm.pattern_id,
          patternMatch: pm,
          colorIndex: -1,
        });
      }
    }

    return events;
  });

  // Compute time domain (single loop avoids temp array + spread stack overflow on >100k events)
  let domain = $derived.by(() => {
    if (allEvents.length === 0) return { minTime: 0, maxTime: 1000, span: 1000 };
    let minTime = allEvents[0].timestamp;
    let maxTime = allEvents[0].timestamp;
    for (let i = 1; i < allEvents.length; i++) {
      const t = allEvents[i].timestamp;
      if (t < minTime) minTime = t;
      if (t > maxTime) maxTime = t;
    }
    const span = Math.max(maxTime - minTime, 1000); // at least 1s span
    // Add 5% padding on each side
    const padding = span * 0.05;
    return { minTime: minTime - padding, maxTime: maxTime + padding, span: span + padding * 2 };
  });

  let msPerPixel = $derived(domain.span / (BASE_HEIGHT * zoom));
  let totalHeight = $derived(domain.span / msPerPixel);

  // Group rule events into source lanes
  let sourceLanes = $derived.by(() => {
    const laneMap = new Map<number, TimelineEvent[]>();

    for (const ev of allEvents) {
      if (ev.type !== 'rule' || ev.sourceId === null) continue;
      if (!laneMap.has(ev.sourceId)) laneMap.set(ev.sourceId, []);
      laneMap.get(ev.sourceId)!.push(ev);
    }

    const lanes: SourceLane[] = [];
    for (const src of sourceList) {
      const events = laneMap.get(src.id) ?? [];
      events.sort((a, b) => a.timestamp - b.timestamp);
      lanes.push({
        sourceId: src.id,
        sourceName: src.name,
        events,
      });
    }
    return lanes;
  });

  // Pattern events for cross-lane bands
  let patternEvents = $derived(
    allEvents.filter((e) => e.type === 'pattern').sort((a, b) => a.timestamp - b.timestamp),
  );

  // Viewport-filtered pattern events (binary search, same approach as TimelineSwimlane)
  let visiblePatternEvents = $derived.by(() => {
    if (patternEvents.length === 0) return [];
    const visMinY = scrollTop - 50;
    const visMaxY = scrollTop + viewportHeight + 50;

    // Binary search for first visible event
    let lo = 0,
      hi = patternEvents.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      const y = (patternEvents[mid].timestamp - domain.minTime) / msPerPixel;
      if (y < visMinY) lo = mid + 1;
      else hi = mid;
    }
    const startIdx = lo;

    // Binary search for last visible event
    lo = startIdx;
    hi = patternEvents.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      const y = (patternEvents[mid].timestamp - domain.minTime) / msPerPixel;
      if (y <= visMaxY) lo = mid + 1;
      else hi = mid;
    }

    return patternEvents.slice(startIdx, lo);
  });

  function onScroll() {
    if (scrollContainer) {
      scrollTop = scrollContainer.scrollTop;
    }
  }

  function onWheel(event: WheelEvent) {
    if (event.ctrlKey || event.metaKey) {
      event.preventDefault();
      const rect = scrollContainer?.getBoundingClientRect();
      if (!rect || !scrollContainer) return;

      const mouseY = event.clientY - rect.top + scrollTop;
      const mouseTimeMs = domain.minTime + mouseY * msPerPixel;

      const factor = event.deltaY > 0 ? 0.85 : 1.18;
      const newZoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom * factor));

      if (newZoom !== zoom) {
        zoom = newZoom;
        // After zoom, recompute where the mouse time would be and adjust scroll
        const newMsPerPixel = domain.span / (BASE_HEIGHT * newZoom);
        const newMouseY = (mouseTimeMs - domain.minTime) / newMsPerPixel;
        const newScrollTop = newMouseY - (event.clientY - rect.top);
        // Use tick to allow derived values to update
        requestAnimationFrame(() => {
          if (scrollContainer) {
            scrollContainer.scrollTop = Math.max(0, newScrollTop);
            scrollTop = scrollContainer.scrollTop;
          }
        });
      }
    }
  }

  function zoomIn() {
    zoom = Math.min(MAX_ZOOM, zoom * 1.5);
  }

  function zoomOut() {
    zoom = Math.max(MIN_ZOOM, zoom / 1.5);
  }

  function zoomReset() {
    zoom = 1;
  }

  function onEventClick(event: TimelineEvent) {
    const newSelection = selectedEvent?.id === event.id ? null : event;
    selectedEvent = newSelection;
    if (newSelection) {
      requestAnimationFrame(() => {
        detailPanelEl?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      });
    }
  }

  $effect(() => {
    if (scrollContainer) {
      const obs = new ResizeObserver((entries) => {
        for (const entry of entries) {
          viewportHeight = entry.contentRect.height;
          viewportWidth = entry.contentRect.width;
        }
      });
      obs.observe(scrollContainer);
      return () => obs.disconnect();
    }
  });

  const AXIS_WIDTH = 120;
  const MIN_LANE_WIDTH = 60;
  let laneWidth = $derived.by(() => {
    if (sourceLanes.length === 0) return 100;
    const available = viewportWidth - AXIS_WIDTH;
    return Math.max(MIN_LANE_WIDTH, Math.floor(available / sourceLanes.length));
  });
  let swimlanesWidth = $derived(sourceLanes.length * laneWidth);
</script>

<div class="timeline-container">
  <div class="zoom-controls">
    <button onclick={zoomIn} title="Zoom in">+</button>
    <button onclick={zoomOut} title="Zoom out">-</button>
    <button onclick={zoomReset} title="Reset zoom">Reset</button>
    <span class="zoom-label">{Math.round(zoom * 100)}%</span>
  </div>

  {#if allEvents.length === 0}
    <div class="empty">No timestamped events to display on timeline.</div>
  {:else}
    <!-- Swimlane headers -->
    <div class="lane-headers">
      <div class="axis-header"></div>
      {#each sourceLanes as lane}
        <div class="lane-header" style="width: {laneWidth}px">
          <span class="lane-name">{lane.sourceName}</span>
          <span class="lane-count-badge">{lane.events.length}</span>
        </div>
      {/each}
    </div>

    <!-- Scrollable timeline area -->
    <div class="scroll-area" bind:this={scrollContainer} onscroll={onScroll} onwheel={onWheel}>
      <div class="scroll-content" style="height: {totalHeight}px">
        <!-- Time axis -->
        <div class="axis-column">
          <TimelineAxis
            minTime={domain.minTime}
            maxTime={domain.maxTime}
            {msPerPixel}
            {scrollTop}
            {viewportHeight}
          />
          <!-- Pattern labels in axis gutter -->
          {#each visiblePatternEvents as pev}
            {@const y = (pev.timestamp - domain.minTime) / msPerPixel}
            <button class="pattern-label" style="top: {y - 10}px" onclick={() => onEventClick(pev)}
              >{getPatternName(pev.patternId!)}</button
            >
          {/each}
        </div>

        <!-- Swimlanes SVG -->
        <svg class="swimlanes-svg" width={swimlanesWidth} height={totalHeight}>
          <!-- Alternating lane backgrounds -->
          {#each sourceLanes as lane, i}
            <rect
              x={i * laneWidth}
              y="0"
              width={laneWidth}
              height={totalHeight}
              fill={i % 2 === 0 ? 'var(--bg)' : 'var(--bg-secondary)'}
              opacity="0.3"
            />
          {/each}

          <!-- Lane divider lines -->
          {#each sourceLanes as _, i}
            {#if i > 0}
              <line
                x1={i * laneWidth}
                y1="0"
                x2={i * laneWidth}
                y2={totalHeight}
                stroke="var(--border)"
                stroke-width="1"
                opacity="0.3"
              />
            {/if}
          {/each}

          <!-- Pattern bands (visual only, behind swimlanes) -->
          {#each visiblePatternEvents as pev}
            {@const y = (pev.timestamp - domain.minTime) / msPerPixel}
            <rect
              x="0"
              y={y - 2}
              width={swimlanesWidth}
              height="5"
              fill="var(--purple)"
              opacity="0.35"
              rx="2"
            />
            <!-- Diamond markers at lane intersections -->
            {#each sourceLanes as _, li}
              {@const dx = li * laneWidth + laneWidth / 2}
              <polygon
                points="{dx},{y - 4} {dx + 4},{y} {dx},{y + 4} {dx - 4},{y}"
                fill="var(--purple)"
                opacity="0.3"
              />
            {/each}
          {/each}

          <!-- Per-source swimlanes -->
          {#each sourceLanes as lane, i}
            <g transform="translate({i * laneWidth}, 0)">
              <TimelineSwimlane
                events={lane.events}
                sourceName={lane.sourceName}
                {laneWidth}
                {totalHeight}
                minTime={domain.minTime}
                {msPerPixel}
                {scrollTop}
                {viewportHeight}
                selectedEventId={selectedEvent?.id ?? null}
                {onEventClick}
              />
            </g>
          {/each}

          <!-- Pattern click targets (on top of swimlanes to receive clicks) -->
          {#each visiblePatternEvents as pev}
            {@const y = (pev.timestamp - domain.minTime) / msPerPixel}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <g onclick={() => onEventClick(pev)} role="button" tabindex="0" style="cursor: pointer">
              <rect x="0" y={y - 8} width={swimlanesWidth} height="16" fill="transparent" />
            </g>
          {/each}
        </svg>
      </div>
    </div>
  {/if}

  {#if selectedEvent}
    <div bind:this={detailPanelEl}>
      <TimelineDetailPanel
        event={selectedEvent}
        {sourceList}
        {ruleList}
        {patternList}
        onClose={() => (selectedEvent = null)}
        {onNavigate}
      />
    </div>
  {/if}
</div>

<style>
  .timeline-container {
    display: flex;
    flex-direction: column;
    min-height: 400px;
    position: relative;
  }

  .zoom-controls {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 6px 8px;
    flex-shrink: 0;
    position: sticky;
    right: 0;
    align-self: flex-end;
    z-index: 2;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .zoom-controls button {
    width: 28px;
    height: 28px;
    padding: 0;
    font-size: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .zoom-controls button:last-of-type {
    width: auto;
    padding: 0 8px;
    font-size: 11px;
  }

  .zoom-label {
    font-size: 11px;
    color: var(--text-muted);
    margin-left: 4px;
    font-family: var(--font-mono);
  }

  .lane-headers {
    display: flex;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
    width: 100%;
  }

  .axis-header {
    width: 120px;
    flex-shrink: 0;
  }

  .lane-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 8px 4px;
    gap: 4px;
    border-bottom: 1px solid var(--border);
  }

  .lane-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--cyan);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
    text-align: center;
  }

  .lane-count-badge {
    font-size: 10px;
    color: var(--text-dim);
    font-family: var(--font-mono);
    background: var(--bg-tertiary);
    padding: 1px 6px;
    border-radius: 8px;
    line-height: 1.4;
  }

  .scroll-area {
    height: calc(100vh - 340px);
    min-height: 300px;
    overflow-y: auto;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
    width: 100%;
  }

  .scroll-content {
    display: flex;
    position: relative;
    min-width: fit-content;
  }

  .axis-column {
    width: 120px;
    flex-shrink: 0;
    position: sticky;
    left: 0;
    z-index: 1;
    background: var(--bg);
    overflow: visible;
  }

  .swimlanes-svg {
    display: block;
  }

  .pattern-label {
    position: absolute;
    left: 4px;
    right: 4px;
    background: rgba(187, 154, 247, 0.15);
    color: var(--purple);
    font-size: 9px;
    font-family: var(--font-mono);
    padding: 2px 6px;
    border-radius: 4px;
    border: none;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
  }

  .pattern-label:hover {
    background: rgba(187, 154, 247, 0.25);
  }
</style>
