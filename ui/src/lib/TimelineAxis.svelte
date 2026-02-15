<script lang="ts">
  let {
    minTime,
    maxTime,
    msPerPixel,
    scrollTop = 0,
    viewportHeight = 600,
  }: {
    minTime: number;
    maxTime: number;
    msPerPixel: number;
    scrollTop?: number;
    viewportHeight?: number;
  } = $props();

  const NICE_INTERVALS = [
    100, 250, 500, 1_000, 2_000, 5_000, 10_000, 15_000, 30_000, 60_000, 120_000, 300_000, 600_000,
    1_800_000, 3_600_000, 7_200_000, 14_400_000, 21_600_000, 43_200_000, 86_400_000,
  ];

  let totalHeight = $derived((maxTime - minTime) / msPerPixel);

  let ticks = $derived.by(() => {
    const targetSpacingPx = 80;
    const targetIntervalMs = targetSpacingPx * msPerPixel;

    let interval = NICE_INTERVALS[NICE_INTERVALS.length - 1];
    for (const ni of NICE_INTERVALS) {
      if (ni >= targetIntervalMs) {
        interval = ni;
        break;
      }
    }

    const visibleMinMs = minTime + scrollTop * msPerPixel;
    const visibleMaxMs = minTime + (scrollTop + viewportHeight) * msPerPixel;

    const bufferMs = interval * 2;
    const startMs = Math.floor((visibleMinMs - bufferMs) / interval) * interval;
    const endMs = visibleMaxMs + bufferMs;

    const result: { y: number; label: string; ms: number }[] = [];
    for (let ms = startMs; ms <= endMs; ms += interval) {
      if (ms < minTime || ms > maxTime) continue;
      const y = (ms - minTime) / msPerPixel;
      result.push({ y, label: formatTick(ms, interval), ms });
    }
    return result;
  });

  function formatTick(ms: number, interval: number): string {
    const d = new Date(ms);
    const h = d.getUTCHours().toString().padStart(2, '0');
    const m = d.getUTCMinutes().toString().padStart(2, '0');
    const s = d.getUTCSeconds().toString().padStart(2, '0');
    const mil = d.getUTCMilliseconds().toString().padStart(3, '0');

    if (interval < 1000) {
      return `${h}:${m}:${s}.${mil}`;
    } else if (interval < 60_000) {
      return `${h}:${m}:${s}`;
    } else if (interval < 3_600_000) {
      return `${h}:${m}`;
    } else if (interval < 86_400_000) {
      const mon = d.toLocaleString('en', { month: 'short', timeZone: 'UTC' });
      const day = d.getUTCDate().toString().padStart(2, '0');
      return `${mon} ${day} ${h}:${m}`;
    } else {
      const mon = d.toLocaleString('en', { month: 'short', timeZone: 'UTC' });
      const day = d.getUTCDate().toString().padStart(2, '0');
      return `${mon} ${day}`;
    }
  }
</script>

<svg class="timeline-axis" width="120" height={totalHeight}>
  {#each ticks as tick}
    <line x1="110" y1={tick.y} x2="120" y2={tick.y} stroke="var(--text-muted)" stroke-width="1" />
    <text
      x="106"
      y={tick.y + 4}
      text-anchor="end"
      fill="var(--text-dim)"
      font-family="var(--font-mono)"
      font-size="12">{tick.label}</text
    >
  {/each}
</svg>

<style>
  .timeline-axis {
    display: block;
    flex-shrink: 0;
  }
</style>
