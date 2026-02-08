import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/svelte';
import TimelineAxis from '../TimelineAxis.svelte';

function renderAxis(propsOverrides: Record<string, any> = {}) {
  const defaults = {
    minTime: 0,
    maxTime: 60_000,
    msPerPixel: 30,
    scrollTop: 0,
    viewportHeight: 600,
  };
  return render(TimelineAxis, { props: { ...defaults, ...propsOverrides } });
}

describe('TimelineAxis', () => {
  // --- Behavioral ---

  it('picks sub-second interval when msPerPixel is very small', () => {
    // msPerPixel=0.1 → target=6ms → picks 100ms interval → labels have milliseconds
    const { container } = renderAxis({
      minTime: 0,
      maxTime: 10_000,
      msPerPixel: 0.1,
      scrollTop: 0,
      viewportHeight: 600,
    });
    const texts = container.querySelectorAll('text');
    expect(texts.length).toBeGreaterThan(0);
    // Sub-second ticks show HH:MM:SS.mmm
    const label = texts[0].textContent!;
    expect(label).toMatch(/^\d{2}:\d{2}:\d{2}\.\d{3}$/);
  });

  it('picks second-level interval at moderate zoom', () => {
    // msPerPixel=1 → target=60ms → picks 100ms interval → still sub-second
    // Use msPerPixel=10 → target=600ms → picks 1000ms → HH:MM:SS
    const { container } = renderAxis({
      minTime: 0,
      maxTime: 60_000,
      msPerPixel: 10,
      scrollTop: 0,
      viewportHeight: 600,
    });
    const texts = container.querySelectorAll('text');
    expect(texts.length).toBeGreaterThan(0);
    const label = texts[0].textContent!;
    // Second-level: HH:MM:SS (no milliseconds)
    expect(label).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });

  it('picks minute-level interval when zoomed out', () => {
    // msPerPixel=1000 → target=60000ms → picks 60000ms → HH:MM
    const { container } = renderAxis({
      minTime: 0,
      maxTime: 3_600_000,
      msPerPixel: 1000,
      scrollTop: 0,
      viewportHeight: 600,
    });
    const texts = container.querySelectorAll('text');
    expect(texts.length).toBeGreaterThan(0);
    const label = texts[0].textContent!;
    expect(label).toMatch(/^\d{2}:\d{2}$/);
  });

  it('renders at most one tick when time range is zero', () => {
    // When minTime === maxTime, totalHeight is 0 and the visible window is tiny.
    // The axis may render the single tick at the boundary point.
    const { container } = renderAxis({
      minTime: 5000,
      maxTime: 5000,
      msPerPixel: 1,
    });
    const texts = container.querySelectorAll('text');
    expect(texts.length).toBeLessThanOrEqual(1);
  });

  it('only renders ticks within visible viewport + buffer', () => {
    // Large range, small viewport → should only render ticks near viewport
    const { container } = renderAxis({
      minTime: 0,
      maxTime: 3_600_000,
      msPerPixel: 10,
      scrollTop: 0,
      viewportHeight: 200,
    });
    const texts = container.querySelectorAll('text');
    // With buffer of 2*interval, not all 3600 seconds of ticks should render
    // At msPerPixel=10, viewport sees 0-2000ms. Interval=1000ms, buffer=2000ms
    // So ticks from ~0ms to ~4000ms → maybe 4-5 ticks, definitely < 100
    expect(texts.length).toBeLessThan(100);
    expect(texts.length).toBeGreaterThan(0);
  });

  it('sets SVG height based on time range and msPerPixel', () => {
    const { container } = renderAxis({
      minTime: 0,
      maxTime: 10_000,
      msPerPixel: 5,
    });
    const svg = container.querySelector('svg')!;
    expect(svg.getAttribute('height')).toBe('2000');
  });

  // --- Snapshot ---

  it('matches snapshot with second-level ticks', () => {
    const { container } = renderAxis({
      minTime: 0,
      maxTime: 10_000,
      msPerPixel: 10,
      scrollTop: 0,
      viewportHeight: 600,
    });
    expect(container.innerHTML).toMatchSnapshot();
  });
});
