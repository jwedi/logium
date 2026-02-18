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

  it('calls onBucketClick with the RuleMatch when a bar is clicked', async () => {
    const onClick = vi.fn();
    const matches = [
      makeMatchAtTime('2024-01-15T10:00:00.000', 'first-line'),
      makeMatchAtTime('2024-01-15T10:05:00.000', 'second-line'),
    ];
    const { container } = renderHistogram({
      ruleMatches: matches,
      onBucketClick: onClick,
    });
    const bars = container.querySelectorAll('rect.bar');
    const nonZero = Array.from(bars).find((b) => parseFloat(b.getAttribute('height') ?? '0') > 0);
    expect(nonZero).toBeTruthy();
    await fireEvent.click(nonZero!);
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
    expect(bar!.getAttribute('role')).toBeNull();
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
    // Verify date (MM/DD) is shown, not just time
    expect(tooltip!.textContent).toContain('01/15');
  });

  it('skips invalid timestamps', () => {
    const matches = [
      makeMatchAtTime('2024-01-15T10:00:00.000'),
      makeMatchAtTime('not-a-date'),
      makeMatchAtTime('2024-01-15T10:05:00.000'),
    ];
    const { container } = renderHistogram({ ruleMatches: matches });
    // Should still render (2 valid matches)
    expect(container.querySelector('.density-histogram')).toBeInTheDocument();
    const bars = container.querySelectorAll('rect.bar');
    expect(bars.length).toBeGreaterThan(0);
  });
});
