<script lang="ts">
  import type { RuleMatch, PatternMatch, StateValue } from './api';

  function formatStateValue(sv: StateValue): string {
    if ('String' in sv) return sv.String;
    if ('Integer' in sv) return String(sv.Integer);
    if ('Float' in sv) return String(sv.Float);
    if ('Bool' in sv) return String(sv.Bool);
    return '?';
  }

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

  let {
    events,
    sourceName,
    laneWidth,
    totalHeight,
    minTime,
    msPerPixel,
    scrollTop = 0,
    viewportHeight = 600,
    selectedEventId = null,
    onEventClick,
  }: {
    events: TimelineEvent[];
    sourceName: string;
    laneWidth: number;
    totalHeight: number;
    minTime: number;
    msPerPixel: number;
    scrollTop?: number;
    viewportHeight?: number;
    selectedEventId?: number | null;
    onEventClick: (event: TimelineEvent) => void;
  } = $props();

  const CLUSTER_THRESHOLD_PX = 3;
  const DOT_RADIUS = 5;

  interface ClusterOrDot {
    y: number;
    events: TimelineEvent[];
    isCluster: boolean;
  }

  let visibleItems = $derived.by(() => {
    const visMinY = scrollTop - 50;
    const visMaxY = scrollTop + viewportHeight + 50;

    // Binary search for start
    let lo = 0,
      hi = events.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      const y = (events[mid].timestamp - minTime) / msPerPixel;
      if (y < visMinY) lo = mid + 1;
      else hi = mid;
    }
    const startIdx = lo;

    // Binary search for end
    lo = startIdx;
    hi = events.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      const y = (events[mid].timestamp - minTime) / msPerPixel;
      if (y <= visMaxY) lo = mid + 1;
      else hi = mid;
    }
    const endIdx = lo;

    // Cluster nearby dots
    const items: ClusterOrDot[] = [];
    let i = startIdx;
    while (i < endIdx) {
      const ev = events[i];
      const y = (ev.timestamp - minTime) / msPerPixel;
      const group: TimelineEvent[] = [ev];
      let j = i + 1;
      while (j < endIdx) {
        const nextY = (events[j].timestamp - minTime) / msPerPixel;
        if (nextY - y > CLUSTER_THRESHOLD_PX) break;
        group.push(events[j]);
        j++;
      }
      items.push({
        y,
        events: group,
        isCluster: group.length > 1,
      });
      i = j;
    }
    return items;
  });

  let cx = $derived(laneWidth / 2);

  function handleClick(item: ClusterOrDot) {
    // For clusters, select the first event; for singles, select the event
    onEventClick(item.events[0]);
  }

  function clusterRadius(count: number): number {
    if (count <= 1) return DOT_RADIUS;
    return DOT_RADIUS + Math.min(Math.log2(count) * 2, 6);
  }

  function dotColor(item: ClusterOrDot): string {
    if (item.isCluster) {
      // Use rule color if all events share the same rule
      const firstRuleId = item.events[0].ruleId;
      if (firstRuleId !== undefined && item.events.every((e) => e.ruleId === firstRuleId)) {
        return `var(--rule-border-${item.events[0].colorIndex})`;
      }
      return 'var(--text-dim)';
    }
    const ev = item.events[0];
    if (ev.type === 'pattern') return 'var(--purple)';
    return `var(--rule-border-${ev.colorIndex})`;
  }

  function isSelected(item: ClusterOrDot): boolean {
    return selectedEventId !== null && item.events.some((e) => e.id === selectedEventId);
  }

  function getStateTooltip(item: ClusterOrDot): string | null {
    const ev = item.events[0];
    const state = ev.ruleMatch?.extracted_state;
    if (!state || Object.keys(state).length === 0) return null;
    return Object.entries(state)
      .map(([k, v]) => `${k}=${formatStateValue(v)}`)
      .join(', ');
  }

  function getStateLabel(item: ClusterOrDot): string | null {
    const ev = item.events[0];
    const state = ev.ruleMatch?.extracted_state;
    if (!state || Object.keys(state).length === 0) return null;
    const entries = Object.entries(state).slice(0, 2);
    return entries.map(([k, v]) => `${k}=${formatStateValue(v)}`).join(' ');
  }

  // Only show labels when zoomed in enough
  let showLabels = $derived(msPerPixel < 50);
</script>

<g class="swimlane">
  <!-- Header background -->
  <rect x="0" y="0" width={laneWidth} height={totalHeight} fill="transparent" />

  {#each visibleItems as item}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <g
      class="event-dot"
      onclick={() => handleClick(item)}
      role="button"
      tabindex="0"
      style="cursor: pointer"
    >
      {#if isSelected(item)}
        <circle
          {cx}
          cy={item.y}
          r={DOT_RADIUS + 3}
          fill="none"
          stroke="var(--accent)"
          stroke-width="2"
        />
      {/if}
      <circle
        class="hover-ring"
        {cx}
        cy={item.y}
        r={(item.isCluster ? clusterRadius(item.events.length) : DOT_RADIUS) + 2}
        fill="none"
        stroke="var(--accent)"
        stroke-width="1.5"
        opacity="0"
      />
      <circle
        {cx}
        cy={item.y}
        r={item.isCluster ? clusterRadius(item.events.length) : DOT_RADIUS}
        fill={dotColor(item)}
        opacity={item.isCluster ? 0.8 : 0.7}
      >
        {#if item.isCluster}
          <title>{item.events.length} events</title>
        {:else if getStateTooltip(item)}
          <title>{getStateTooltip(item)}</title>
        {/if}
      </circle>
      {#if showLabels && !item.isCluster && getStateLabel(item)}
        <text
          x={cx + DOT_RADIUS + 4}
          y={item.y + 3}
          fill="var(--text-muted)"
          font-size="8"
          font-family="var(--font-mono)"
          class="state-label">{getStateLabel(item)}</text
        >
      {/if}
    </g>
  {/each}
</g>

<style>
  .event-dot .hover-ring {
    transition: opacity 0.15s;
  }

  .event-dot:hover .hover-ring {
    opacity: 0.3 !important;
  }
</style>
