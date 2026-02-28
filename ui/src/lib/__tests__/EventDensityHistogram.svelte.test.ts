import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import EventDensityHistogram from '../EventDensityHistogram.svelte';
import { makeRuleMatch, makeLogLine } from './fixtures';

function makeMatchAtTime(timestamp: string, raw?: string) {
  return makeRuleMatch({
    log_line: makeLogLine({ timestamp, raw: raw ?? `line at ${timestamp}` }),
  });
}

function renderHistogram(propsOverrides: Record<string, any> = {}) {
  const defaults = {
    ruleMatches: [],
  };
  return render(EventDensityHistogram, { props: { ...defaults, ...propsOverrides } });
}

// jsdom lacks PointerEvent, and fireEvent.pointerDown creates events without clientX.
// Use MouseEvent (which supports clientX) with the correct event type name.
function createPointerEvent(type: string, clientX: number): MouseEvent {
  return new MouseEvent(type, { clientX, bubbles: true });
}

async function pointerDown(el: Element, clientX: number) {
  await fireEvent(el, createPointerEvent('pointerdown', clientX));
}
async function pointerMove(el: Element, clientX: number) {
  await fireEvent(el, createPointerEvent('pointermove', clientX));
}
async function pointerUp(el: Element, clientX: number) {
  await fireEvent(el, createPointerEvent('pointerup', clientX));
}

describe('EventDensityHistogram', () => {
  it('renders nothing when empty', () => {
    const { container } = renderHistogram({ ruleMatches: [] });
    expect(container.querySelectorAll('rect.bar')).toHaveLength(0);
    expect(container.querySelector('.density-histogram')).not.toBeInTheDocument();
  });

  it('renders bars for spread matches', () => {
    const matches = Array.from({ length: 10 }, (_, i) => {
      const min = i.toString().padStart(2, '0');
      return makeMatchAtTime(`2024-01-15T10:${min}:00.000`);
    });
    const { container } = renderHistogram({ ruleMatches: matches });
    const bars = container.querySelectorAll('rect.bar');
    expect(bars.length).toBeGreaterThan(0);
  });

  it('handles single timestamp edge case', () => {
    const matches = Array.from({ length: 5 }, () => makeMatchAtTime('2024-01-15T10:30:00.000'));
    const { container } = renderHistogram({ ruleMatches: matches });
    const bars = container.querySelectorAll('rect.bar');
    expect(bars.length).toBe(1);
  });

  it('calls onBucketClick with the RuleMatch on single pointer click', async () => {
    const onClick = vi.fn();
    const matches = [
      makeMatchAtTime('2024-01-15T10:00:00.000', 'first-line'),
      makeMatchAtTime('2024-01-15T10:05:00.000', 'second-line'),
    ];
    const { container } = renderHistogram({
      ruleMatches: matches,
      onBucketClick: onClick,
    });
    const svg = container.querySelector('svg')!;
    expect(svg).toBeTruthy();
    // clientX=36 maps to bucket 0 which has count=1 (first match)
    await pointerDown(svg, 36);
    await pointerUp(svg, 36);
    expect(onClick).toHaveBeenCalledWith(
      expect.objectContaining({ log_line: expect.objectContaining({ raw: expect.any(String) }) }),
    );
  });

  it('does not show pointer cursor when onBucketClick is not provided', () => {
    const matches = [
      makeMatchAtTime('2024-01-15T10:00:00.000'),
      makeMatchAtTime('2024-01-15T10:05:00.000'),
    ];
    const { container } = renderHistogram({ ruleMatches: matches });
    const bar = container.querySelector('rect.bar');
    expect(bar).toBeTruthy();
    expect(bar!.classList.contains('clickable')).toBe(false);
  });

  it('shows tooltip with date on hover', async () => {
    const matches = [
      makeMatchAtTime('2024-01-15T10:00:00.000'),
      makeMatchAtTime('2024-01-15T10:05:00.000'),
    ];
    const { container } = renderHistogram({ ruleMatches: matches });
    const bars = container.querySelectorAll('rect.bar');
    const bar = Array.from(bars).find((b) => parseFloat(b.getAttribute('height') ?? '0') > 0);
    expect(bar).toBeTruthy();
    await fireEvent.mouseEnter(bar!);
    const tooltip = container.querySelector('.tooltip');
    expect(tooltip).toBeInTheDocument();
    expect(tooltip!.textContent).toContain('match');
    expect(tooltip!.textContent).toContain('01/15');
  });

  it('skips invalid timestamps', () => {
    const matches = [
      makeMatchAtTime('2024-01-15T10:00:00.000'),
      makeMatchAtTime('not-a-date'),
      makeMatchAtTime('2024-01-15T10:05:00.000'),
    ];
    const { container } = renderHistogram({ ruleMatches: matches });
    expect(container.querySelector('.density-histogram')).toBeInTheDocument();
    const bars = container.querySelectorAll('rect.bar');
    expect(bars.length).toBeGreaterThan(0);
  });

  it('drag selection calls onTimeRangeSelect with correct range', async () => {
    const onSelect = vi.fn();
    const matches = Array.from({ length: 10 }, (_, i) => {
      const min = i.toString().padStart(2, '0');
      return makeMatchAtTime(`2024-01-15T10:${min}:00.000`);
    });
    const { container } = renderHistogram({
      ruleMatches: matches,
      onTimeRangeSelect: onSelect,
    });
    const svg = container.querySelector('svg')!;
    expect(svg).toBeTruthy();
    // clientX=36 → bucket 0, clientX=200 → bucket ~12 (different buckets)
    await pointerDown(svg, 36);
    await pointerMove(svg, 200);
    await pointerUp(svg, 200);
    expect(onSelect).toHaveBeenCalledTimes(1);
    const [start, end] = onSelect.mock.calls[0];
    expect(start).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}$/);
    expect(end).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}$/);
    expect(start < end).toBe(true);
  });

  it('backward drag (right-to-left) still produces correct start < end', async () => {
    const onSelect = vi.fn();
    const matches = Array.from({ length: 10 }, (_, i) => {
      const min = i.toString().padStart(2, '0');
      return makeMatchAtTime(`2024-01-15T10:${min}:00.000`);
    });
    const { container } = renderHistogram({
      ruleMatches: matches,
      onTimeRangeSelect: onSelect,
    });
    const svg = container.querySelector('svg')!;
    await pointerDown(svg, 200);
    await pointerMove(svg, 36);
    await pointerUp(svg, 36);
    expect(onSelect).toHaveBeenCalledTimes(1);
    const [start, end] = onSelect.mock.calls[0];
    expect(start < end).toBe(true);
  });

  it('single click fires onBucketClick, not onTimeRangeSelect', async () => {
    const onClick = vi.fn();
    const onSelect = vi.fn();
    const matches = [
      makeMatchAtTime('2024-01-15T10:00:00.000', 'first-line'),
      makeMatchAtTime('2024-01-15T10:05:00.000', 'second-line'),
    ];
    const { container } = renderHistogram({
      ruleMatches: matches,
      onBucketClick: onClick,
      onTimeRangeSelect: onSelect,
    });
    const svg = container.querySelector('svg')!;
    // clientX=36 maps to bucket 0 (has a match)
    await pointerDown(svg, 36);
    await pointerUp(svg, 36);
    expect(onClick).toHaveBeenCalled();
    expect(onSelect).not.toHaveBeenCalled();
  });

  it('shows selection overlay during drag', async () => {
    const matches = Array.from({ length: 10 }, (_, i) => {
      const min = i.toString().padStart(2, '0');
      return makeMatchAtTime(`2024-01-15T10:${min}:00.000`);
    });
    const { container } = renderHistogram({
      ruleMatches: matches,
      onTimeRangeSelect: vi.fn(),
    });
    const svg = container.querySelector('svg')!;
    await pointerDown(svg, 36);
    await pointerMove(svg, 200);
    const overlay = container.querySelector('.selection-overlay');
    expect(overlay).toBeInTheDocument();
  });

  it('Escape cancels drag selection without firing callbacks', async () => {
    const onSelect = vi.fn();
    const onClick = vi.fn();
    const matches = Array.from({ length: 10 }, (_, i) => {
      const min = i.toString().padStart(2, '0');
      return makeMatchAtTime(`2024-01-15T10:${min}:00.000`);
    });
    const { container } = renderHistogram({
      ruleMatches: matches,
      onTimeRangeSelect: onSelect,
      onBucketClick: onClick,
    });
    const svg = container.querySelector('svg')!;
    await pointerDown(svg, 36);
    await pointerMove(svg, 200);
    // Selection overlay should be visible mid-drag
    expect(container.querySelector('.selection-overlay')).toBeInTheDocument();
    // Press Escape to cancel
    await fireEvent.keyDown(window, { key: 'Escape' });
    // Overlay should be gone
    expect(container.querySelector('.selection-overlay')).not.toBeInTheDocument();
    // Neither callback should have fired
    expect(onSelect).not.toHaveBeenCalled();
    expect(onClick).not.toHaveBeenCalled();
    // Subsequent pointerUp should be a no-op
    await pointerUp(svg, 200);
    expect(onSelect).not.toHaveBeenCalled();
    expect(onClick).not.toHaveBeenCalled();
  });

  it('shows crosshair cursor when onTimeRangeSelect is provided', () => {
    const matches = [
      makeMatchAtTime('2024-01-15T10:00:00.000'),
      makeMatchAtTime('2024-01-15T10:05:00.000'),
    ];
    const { container } = renderHistogram({
      ruleMatches: matches,
      onTimeRangeSelect: vi.fn(),
    });
    const svg = container.querySelector('svg')!;
    expect(svg.classList.contains('selectable')).toBe(true);
  });
});
