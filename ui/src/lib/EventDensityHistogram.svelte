<script lang="ts">
  import type { RuleMatch } from './api';

  let {
    ruleMatches,
    onBucketClick,
  }: {
    ruleMatches: RuleMatch[];
    onBucketClick?: (match: RuleMatch) => void;
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

  const BAR_AREA_HEIGHT = 60;
  const LABEL_HEIGHT = 20;
  const TOTAL_HEIGHT = BAR_AREA_HEIGHT + LABEL_HEIGHT;
  const GAP = 2;
  const PAD_X = 4;

  let hoveredIdx: number | null = $state(null);
  let tooltipX = $state(0);
  let tooltipY = $state(0);

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

  function barWidth(total: number): number {
    if (total <= 0) return 0;
    const w = containerWidth > 0 ? containerWidth : 600;
    return Math.max(1, (w - PAD_X * 2) / total - GAP);
  }

  function barX(idx: number, total: number): number {
    const bw = barWidth(total) + GAP;
    return PAD_X + idx * bw;
  }

  function barHeight(count: number): number {
    if (maxCount === 0) return 0;
    return Math.max(2, (count / maxCount) * BAR_AREA_HEIGHT);
  }

  function handleClick(bucket: Bucket) {
    if (onBucketClick && bucket.count > 0) {
      onBucketClick(bucket.firstMatch);
    }
  }

  // Time labels: start, middle, end
  let timeLabels = $derived.by(() => {
    if (buckets.length === 0) return [];
    const first = buckets[0];
    const last = buckets[buckets.length - 1];
    if (buckets.length === 1) {
      return [{ x: PAD_X, text: formatLabel(first.startMs) }];
    }
    const w = containerWidth > 0 ? containerWidth : 600;
    const mid = (first.startMs + last.endMs) / 2;
    return [
      { x: PAD_X, text: formatLabel(first.startMs) },
      { x: w / 2, text: formatLabel(mid) },
      { x: w - PAD_X, text: formatLabel(last.endMs) },
    ];
  });
</script>

{#if buckets.length > 0}
  <div class="density-histogram" bind:clientWidth={containerWidth}>
    <svg width="100%" height={TOTAL_HEIGHT}>
      {#each buckets as bucket, i}
        {@const bw = barWidth(buckets.length)}
        {@const bx = barX(i, buckets.length)}
        {@const bh = barHeight(bucket.count)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <rect
          class="bar"
          class:clickable={!!onBucketClick}
          x={bx}
          y={BAR_AREA_HEIGHT - bh}
          width={bw}
          height={bh}
          opacity={hoveredIdx === i ? 1.0 : 0.7}
          role={onBucketClick ? 'button' : undefined}
          tabindex={onBucketClick ? 0 : undefined}
          onclick={() => handleClick(bucket)}
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
          text-anchor={label.x <= PAD_X + 1
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
          barWidth(buckets.length) / 2}px; top: {BAR_AREA_HEIGHT - barHeight(hb.count) - 4}px;"
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
  }

  .bar {
    fill: var(--accent);
    transition: opacity 0.1s;
  }

  .bar.clickable {
    cursor: pointer;
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
