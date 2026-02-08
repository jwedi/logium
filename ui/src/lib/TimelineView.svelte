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
  }: {
    result: AnalysisResult;
    sourceList: Source[];
    ruleList: LogRule[];
    patternList: Pattern[];
  } = $props();

  const BASE_HEIGHT = 2000;
  const MIN_ZOOM = 0.1;
  const MAX_ZOOM = 50;

  let zoom = $state(1);
  let scrollTop = $state(0);
  let viewportHeight = $state(600);
  let scrollContainer: HTMLDivElement | undefined = $state();
  let selectedEvent: TimelineEvent | null = $state(null);

  function getSourceName(id: number): string {
    return sourceList.find((s) => s.id === id)?.name ?? `Source #${id}`;
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

  // Compute time domain
  let domain = $derived.by(() => {
    const timestamps = allEvents.map((e) => e.timestamp);
    if (timestamps.length === 0) return { minTime: 0, maxTime: 1000, span: 1000 };
    const minTime = Math.min(...timestamps);
    const maxTime = Math.max(...timestamps);
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
    selectedEvent = selectedEvent?.id === event.id ? null : event;
  }

  $effect(() => {
    if (scrollContainer) {
      const obs = new ResizeObserver((entries) => {
        for (const entry of entries) {
          viewportHeight = entry.contentRect.height;
        }
      });
      obs.observe(scrollContainer);
      return () => obs.disconnect();
    }
  });

  let laneWidth = $derived(
    sourceLanes.length > 0 ? Math.max(60, Math.min(120, 600 / sourceLanes.length)) : 100,
  );
  let swimlanesWidth = $derived(sourceLanes.length * laneWidth);
</script>

<div class="timeline-container" class:has-detail={selectedEvent !== null}>
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
          <span class="lane-count">{lane.events.length}</span>
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

          <!-- Pattern bands -->
          {#each patternEvents as pev}
            {@const y = (pev.timestamp - domain.minTime) / msPerPixel}
            <rect
              x="0"
              y={y - 1}
              width={swimlanesWidth}
              height="3"
              fill="var(--purple)"
              opacity="0.2"
              rx="1"
            />
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <g onclick={() => onEventClick(pev)} role="button" tabindex="0" style="cursor: pointer">
              <rect x="0" y={y - 8} width={swimlanesWidth} height="16" fill="transparent" />
              <text
                x="4"
                y={y - 4}
                fill="var(--purple)"
                font-size="9"
                font-family="var(--font-mono)"
                opacity="0.7">PM</text
              >
            </g>
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
        </svg>
      </div>
    </div>
  {/if}

  {#if selectedEvent}
    <TimelineDetailPanel
      event={selectedEvent}
      {sourceList}
      {ruleList}
      {patternList}
      onClose={() => (selectedEvent = null)}
    />
  {/if}
</div>

<style>
  .timeline-container {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 260px);
    min-height: 400px;
    position: relative;
  }

  .timeline-container.has-detail {
    flex-direction: row;
    flex-wrap: wrap;
  }

  .timeline-container.has-detail .zoom-controls {
    width: 100%;
  }

  .timeline-container.has-detail .lane-headers {
    width: calc(100% - 300px);
  }

  .timeline-container.has-detail .scroll-area {
    width: calc(100% - 300px);
  }

  .zoom-controls {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 8px 0;
    flex-shrink: 0;
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
    width: 80px;
    flex-shrink: 0;
  }

  .lane-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 6px 4px;
    gap: 2px;
  }

  .lane-name {
    font-size: 11px;
    font-weight: 600;
    color: var(--cyan);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
    text-align: center;
  }

  .lane-count {
    font-size: 10px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }

  .scroll-area {
    flex: 1;
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
    width: 80px;
    flex-shrink: 0;
    position: sticky;
    left: 0;
    z-index: 1;
    background: var(--bg);
  }

  .swimlanes-svg {
    display: block;
  }
</style>
