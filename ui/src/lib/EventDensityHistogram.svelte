<script lang="ts">
  import type { RuleMatch } from './api';

  let {
    ruleMatches,
    onBucketClick,
    onTimeRangeSelect,
  }: {
    ruleMatches: RuleMatch[];
    onBucketClick?: (match: RuleMatch) => void;
    onTimeRangeSelect?: (start: string, end: string) => void;
  } = $props();

  let containerWidth = $state(0);

  interface Bucket {
    startMs: number;
    endMs: number;
    count: number;
    firstMatch: RuleMatch;
  }

  let buckets: Bucket[] = $derived.by(() => {
    // Parse and filter valid timestamps
    const parsed: { ms: number; match: RuleMatch }[] = [];
    for (const rm of ruleMatches) {
      const ms = Date.parse(rm.log_line.timestamp + 'Z');
      if (!isNaN(ms)) parsed.push({ ms, match: rm });
    }
    if (parsed.length === 0) return [];

    let minMs = Infinity;
    let maxMs = -Infinity;
    for (const p of parsed) {
      if (p.ms < minMs) minMs = p.ms;
      if (p.ms > maxMs) maxMs = p.ms;
    }

    const span = maxMs - minMs;

    // Single timestamp: one bucket
    if (span === 0) {
      return [{ startMs: minMs, endMs: maxMs, count: parsed.length, firstMatch: parsed[0].match }];
    }

    const w = containerWidth > 0 ? containerWidth : 600;
    const numBuckets = Math.min(80, Math.max(10, Math.floor(w / 14)));
    const bucketSpan = span / numBuckets;

    const result: Bucket[] = Array.from({ length: numBuckets }, (_, i) => ({
      startMs: minMs + i * bucketSpan,
      endMs: minMs + (i + 1) * bucketSpan,
      count: 0,
      firstMatch: parsed[0].match, // placeholder
    }));

    // Track whether each bucket has had its firstMatch set
    const hasFirst = new Uint8Array(numBuckets);

    for (const p of parsed) {
      let idx = Math.floor((p.ms - minMs) / bucketSpan);
      if (idx >= numBuckets) idx = numBuckets - 1;
      result[idx].count++;
      if (!hasFirst[idx]) {
        result[idx].firstMatch = p.match;
        hasFirst[idx] = 1;
      }
    }

    return result;
  });

  let maxCount = $derived(buckets.reduce((m, b) => Math.max(m, b.count), 0));

  const Y_LABEL_WIDTH = 32;
  const BAR_AREA_HEIGHT = 60;
  const LABEL_HEIGHT = 20;
  const TOTAL_HEIGHT = BAR_AREA_HEIGHT + LABEL_HEIGHT;
  const GAP = 2;
  const PAD_X = 4;

  let svgEl: SVGSVGElement;

  let hoveredIdx: number | null = $state(null);
  let tooltipX = $state(0);
  let tooltipY = $state(0);

  let dragStartIdx: number | null = $state(null);
  let dragCurrentIdx: number | null = $state(null);

  function formatLabel(ms: number): string {
    const d = new Date(ms);
    const mo = String(d.getUTCMonth() + 1).padStart(2, '0');
    const dd = String(d.getUTCDate()).padStart(2, '0');
    const hh = String(d.getUTCHours()).padStart(2, '0');
    const mm = String(d.getUTCMinutes()).padStart(2, '0');
    return `${mo}/${dd} ${hh}:${mm}`;
  }

  function formatTooltipTime(ms: number): string {
    const d = new Date(ms);
    const mo = String(d.getUTCMonth() + 1).padStart(2, '0');
    const dd = String(d.getUTCDate()).padStart(2, '0');
    return `${mo}/${dd} ${d.toISOString().slice(11, 19)}`;
  }

  function msToDatetimeLocal(ms: number): string {
    const d = new Date(ms);
    const yyyy = d.getUTCFullYear();
    const mo = String(d.getUTCMonth() + 1).padStart(2, '0');
    const dd = String(d.getUTCDate()).padStart(2, '0');
    const hh = String(d.getUTCHours()).padStart(2, '0');
    const mm = String(d.getUTCMinutes()).padStart(2, '0');
    const ss = String(d.getUTCSeconds()).padStart(2, '0');
    return `${yyyy}-${mo}-${dd}T${hh}:${mm}:${ss}`;
  }

  function chartWidth(): number {
    const w = containerWidth > 0 ? containerWidth : 600;
    return w - Y_LABEL_WIDTH;
  }

  function barWidth(total: number): number {
    if (total <= 0) return 0;
    return Math.max(1, (chartWidth() - PAD_X * 2) / total - GAP);
  }

  function barX(idx: number, total: number): number {
    const bw = barWidth(total) + GAP;
    return Y_LABEL_WIDTH + PAD_X + idx * bw;
  }

  function barHeight(count: number): number {
    if (maxCount === 0) return 0;
    return Math.max(2, (count / maxCount) * BAR_AREA_HEIGHT);
  }

  function xToBucketIdx(clientX: number, svg: SVGSVGElement): number | null {
    if (buckets.length === 0) return null;
    const rect = svg.getBoundingClientRect();
    const x = clientX - rect.left;
    const firstBarX = barX(0, buckets.length);
    const bw = barWidth(buckets.length);
    const step = bw + GAP;
    const lastBarEnd = barX(buckets.length - 1, buckets.length) + bw;

    if (x < firstBarX) return 0;
    if (x >= lastBarEnd) return buckets.length - 1;

    const idx = Math.floor((x - firstBarX) / step);
    return Math.min(Math.max(0, idx), buckets.length - 1);
  }

  function cancelDrag() {
    window.removeEventListener('keydown', handleKeyDown);
    dragStartIdx = null;
    dragCurrentIdx = null;
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape' && dragStartIdx !== null) {
      cancelDrag();
    }
  }

  function handlePointerDown(e: PointerEvent) {
    if (!onTimeRangeSelect && !onBucketClick) return;
    if (!svgEl) return;
    const idx = xToBucketIdx(e.clientX, svgEl);
    if (idx === null) return;
    dragStartIdx = idx;
    dragCurrentIdx = idx;
    svgEl.setPointerCapture?.(e.pointerId);
    window.addEventListener('keydown', handleKeyDown);
  }

  function handlePointerMove(e: PointerEvent) {
    if (dragStartIdx === null) return;
    if (!svgEl) return;
    const idx = xToBucketIdx(e.clientX, svgEl);
    if (idx !== null) dragCurrentIdx = idx;
  }

  function handlePointerUp() {
    window.removeEventListener('keydown', handleKeyDown);

    const startIdx = dragStartIdx;
    const currentIdx = dragCurrentIdx;
    if (startIdx === null || currentIdx === null) {
      dragStartIdx = null;
      dragCurrentIdx = null;
      return;
    }

    if (startIdx === currentIdx) {
      // Single click — fire onBucketClick
      const bucket = buckets[startIdx];
      if (onBucketClick && bucket && bucket.count > 0) {
        onBucketClick(bucket.firstMatch);
      }
    } else {
      // Drag — fire onTimeRangeSelect
      if (onTimeRangeSelect) {
        const minIdx = Math.min(startIdx, currentIdx);
        const maxIdx = Math.max(startIdx, currentIdx);
        const fromBucket = buckets[minIdx];
        const toBucket = buckets[maxIdx];
        if (fromBucket && toBucket) {
          const fromStr = msToDatetimeLocal(fromBucket.startMs);
          const toStr = msToDatetimeLocal(toBucket.endMs);
          onTimeRangeSelect(fromStr, toStr);
        }
      }
    }

    dragStartIdx = null;
    dragCurrentIdx = null;
  }

  // Time labels: start, middle, end
  let timeLabels = $derived.by(() => {
    if (buckets.length === 0) return [];
    const first = buckets[0];
    const last = buckets[buckets.length - 1];
    const cw = chartWidth();
    if (buckets.length === 1) {
      return [{ x: Y_LABEL_WIDTH + PAD_X, text: formatLabel(first.startMs) }];
    }
    const mid = (first.startMs + last.endMs) / 2;
    return [
      { x: Y_LABEL_WIDTH + PAD_X, text: formatLabel(first.startMs) },
      { x: Y_LABEL_WIDTH + cw / 2, text: formatLabel(mid) },
      { x: Y_LABEL_WIDTH + cw - PAD_X, text: formatLabel(last.endMs) },
    ];
  });
</script>

{#if buckets.length > 0}
  <div class="density-histogram card" bind:clientWidth={containerWidth}>
    <span class="histogram-title">Match Density</span>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg
      bind:this={svgEl}
      width="100%"
      height={TOTAL_HEIGHT}
      class:selectable={!!onTimeRangeSelect}
      onpointerdown={handlePointerDown}
      onpointermove={handlePointerMove}
      onpointerup={handlePointerUp}
    >
      <!-- Y-axis max count label -->
      <text
        x={Y_LABEL_WIDTH - 4}
        y="10"
        fill="var(--text-muted)"
        font-size="10"
        font-family="var(--font-mono)"
        text-anchor="end">{maxCount}</text
      >
      <text
        x={Y_LABEL_WIDTH - 4}
        y={BAR_AREA_HEIGHT}
        fill="var(--text-muted)"
        font-size="10"
        font-family="var(--font-mono)"
        text-anchor="end">0</text
      >
      <!-- Y-axis line -->
      <line
        x1={Y_LABEL_WIDTH}
        y1="0"
        x2={Y_LABEL_WIDTH}
        y2={BAR_AREA_HEIGHT}
        stroke="var(--border)"
        stroke-width="1"
      />
      <!-- Selection overlay -->
      {#if dragStartIdx !== null && dragCurrentIdx !== null && dragStartIdx !== dragCurrentIdx}
        {@const minIdx = Math.min(dragStartIdx, dragCurrentIdx)}
        {@const maxIdx = Math.max(dragStartIdx, dragCurrentIdx)}
        {@const x1 = barX(minIdx, buckets.length)}
        {@const x2 = barX(maxIdx, buckets.length) + barWidth(buckets.length)}
        <rect
          class="selection-overlay"
          x={x1}
          y={0}
          width={x2 - x1}
          height={BAR_AREA_HEIGHT}
          fill="var(--accent)"
          opacity="0.15"
          pointer-events="none"
        />
      {/if}
      {#each buckets as bucket, i}
        {@const bw = barWidth(buckets.length)}
        {@const bx = barX(i, buckets.length)}
        {@const bh = barHeight(bucket.count)}
        <rect
          class="bar"
          class:clickable={!!onBucketClick || !!onTimeRangeSelect}
          x={bx}
          y={BAR_AREA_HEIGHT - bh}
          width={bw}
          height={bh}
          opacity={hoveredIdx === i ? 1.0 : 0.7}
          onmouseenter={(e) => {
            hoveredIdx = i;
            const rect = e.currentTarget.getBoundingClientRect();
            tooltipX = rect.left + rect.width / 2;
            tooltipY = rect.top;
          }}
          onmouseleave={() => {
            hoveredIdx = null;
          }}
        />
      {/each}
      {#each timeLabels as label}
        <text
          x={label.x}
          y={TOTAL_HEIGHT - 4}
          fill="var(--text-muted)"
          font-size="10"
          font-family="var(--font-mono)"
          text-anchor={label.x <= Y_LABEL_WIDTH + PAD_X + 1
            ? 'start'
            : label.x >= (containerWidth || 600) - PAD_X - 1
              ? 'end'
              : 'middle'}>{label.text}</text
        >
      {/each}
    </svg>
    {#if hoveredIdx !== null && buckets[hoveredIdx]}
      {@const hb = buckets[hoveredIdx]}
      <div
        class="tooltip"
        style="left: {barX(hoveredIdx, buckets.length) +
          barWidth(buckets.length) / 2}px; top: {BAR_AREA_HEIGHT - barHeight(hb.count) + 16}px;"
      >
        <div>{formatTooltipTime(hb.startMs)} – {formatTooltipTime(hb.endMs)}</div>
        <div>{hb.count} match{hb.count !== 1 ? 'es' : ''}</div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .density-histogram {
    position: relative;
    margin-bottom: 8px;
    padding: 10px 12px 8px;
  }

  .histogram-title {
    display: block;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }

  .bar {
    fill: var(--accent);
    transition: opacity 0.1s;
  }

  .bar.clickable {
    cursor: pointer;
  }

  svg.selectable {
    cursor: crosshair;
  }

  .tooltip {
    position: absolute;
    transform: translate(-50%, -100%);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 4px 8px;
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--text);
    white-space: nowrap;
    pointer-events: none;
    z-index: 10;
  }
</style>
