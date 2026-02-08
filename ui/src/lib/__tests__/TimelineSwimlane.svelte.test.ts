import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import TimelineSwimlane from '../TimelineSwimlane.svelte';
import { makeRuleTimelineEvent, makePatternTimelineEvent, type TimelineEvent } from './fixtures';

function renderSwimlane(propsOverrides: Record<string, any> = {}) {
  const defaults = {
    events: [] as TimelineEvent[],
    sourceName: 'test-source',
    laneWidth: 100,
    totalHeight: 2000,
    minTime: 0,
    msPerPixel: 1,
    scrollTop: 0,
    viewportHeight: 600,
    selectedEventId: null,
    onEventClick: vi.fn(),
  };
  return render(TimelineSwimlane, { props: { ...defaults, ...propsOverrides } });
}

function makeSpreadEvents(count: number, spacingMs: number): TimelineEvent[] {
  return Array.from({ length: count }, (_, i) =>
    makeRuleTimelineEvent({
      id: i,
      timestamp: i * spacingMs,
      ruleId: i % 6,
      colorIndex: i % 6,
    }),
  );
}

function makeClusteredEvents(count: number, baseTimestamp: number): TimelineEvent[] {
  // With msPerPixel=1, events within 3px = within 3ms
  return Array.from({ length: count }, (_, i) =>
    makeRuleTimelineEvent({
      id: i,
      timestamp: baseTimestamp + i, // 1ms apart → within 3px cluster
      colorIndex: i % 6,
    }),
  );
}

describe('TimelineSwimlane', () => {
  // --- Behavioral ---

  it('renders one circle per visible event when events are spread apart', () => {
    const events = makeSpreadEvents(5, 100); // 0ms, 100ms, 200ms, 300ms, 400ms
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    // Each non-clustered event gets exactly one main circle
    const circles = container.querySelectorAll('circle');
    // 5 events, each with one circle
    expect(circles.length).toBe(5);
  });

  it('clusters events within 3px into a single dot with count badge', () => {
    const events = makeClusteredEvents(4, 100); // 100, 101, 102, 103ms → all within 3px
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    // Cluster = 1 main circle + no selection ring
    const circles = container.querySelectorAll('circle');
    expect(circles.length).toBe(1);
    // Count badge text
    const texts = container.querySelectorAll('text');
    expect(texts.length).toBe(1);
    expect(texts[0].textContent).toBe('4');
  });

  it('binary search limits rendering to visible events only', () => {
    // 100 events spread across 10000ms, viewport only shows 0-600px
    const events = makeSpreadEvents(100, 100);
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 200,
      scrollTop: 0,
    });
    const circles = container.querySelectorAll('circle');
    // viewport 0-200px + 50px buffer → events 0-250ms → ~3 events visible
    // Far fewer than 100
    expect(circles.length).toBeLessThan(100);
    expect(circles.length).toBeGreaterThan(0);
  });

  it('onEventClick receives first event of cluster on click', async () => {
    const onEventClick = vi.fn();
    const events = makeClusteredEvents(3, 100);
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
      onEventClick,
    });
    const dot = container.querySelector('.event-dot')!;
    await fireEvent.click(dot);
    expect(onEventClick).toHaveBeenCalledOnce();
    expect(onEventClick.mock.calls[0][0].id).toBe(events[0].id);
  });

  it('onEventClick receives the single event for non-clusters', async () => {
    const onEventClick = vi.fn();
    const events = makeSpreadEvents(1, 100);
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
      onEventClick,
    });
    const dot = container.querySelector('.event-dot')!;
    await fireEvent.click(dot);
    expect(onEventClick).toHaveBeenCalledOnce();
    expect(onEventClick.mock.calls[0][0].id).toBe(events[0].id);
  });

  it('shows selection ring when selectedEventId matches', () => {
    const events = makeSpreadEvents(3, 100);
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
      selectedEventId: events[1].id,
    });
    // Selected event should have an extra circle with accent stroke
    const accentCircles = container.querySelectorAll('circle[stroke="var(--accent)"]');
    expect(accentCircles.length).toBe(1);
  });

  it('no selection ring when selectedEventId is null', () => {
    const events = makeSpreadEvents(3, 100);
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
      selectedEventId: null,
    });
    const accentCircles = container.querySelectorAll('circle[stroke="var(--accent)"]');
    expect(accentCircles.length).toBe(0);
  });

  it('pattern events use purple fill', () => {
    const events = [makePatternTimelineEvent({ id: 0, timestamp: 100 })];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    const circles = container.querySelectorAll('circle');
    expect(circles.length).toBe(1);
    expect(circles[0].getAttribute('fill')).toBe('var(--purple)');
  });

  it('rule events use rule-border color based on colorIndex', () => {
    const events = [makeRuleTimelineEvent({ id: 0, timestamp: 100, colorIndex: 3 })];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    const circles = container.querySelectorAll('circle');
    expect(circles.length).toBe(1);
    expect(circles[0].getAttribute('fill')).toBe('var(--rule-border-3)');
  });

  // --- Snapshot ---

  it('matches snapshot with mixed events including a cluster', () => {
    const events = [
      makeRuleTimelineEvent({ id: 0, timestamp: 100, colorIndex: 0 }),
      makeRuleTimelineEvent({ id: 1, timestamp: 101, colorIndex: 1 }), // cluster with id 0
      makePatternTimelineEvent({ id: 2, timestamp: 300 }),
      makeRuleTimelineEvent({ id: 3, timestamp: 500, colorIndex: 2 }),
    ];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    expect(container.innerHTML).toMatchSnapshot();
  });
});
