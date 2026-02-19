import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import TimelineSwimlane from '../TimelineSwimlane.svelte';
import {
  makeRuleTimelineEvent,
  makePatternTimelineEvent,
  makeRuleMatch,
  type TimelineEvent,
} from './fixtures';

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
    // Each non-clustered event gets a hover-ring + main circle = 2 per event
    const circles = container.querySelectorAll('circle');
    expect(circles.length).toBe(10);
  });

  it('clusters events within 3px into a single scaled dot with tooltip', () => {
    const events = makeClusteredEvents(4, 100); // 100, 101, 102, 103ms → all within 3px
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    // Cluster = 1 hover-ring + 1 main circle
    const circles = container.querySelectorAll('circle');
    expect(circles.length).toBe(2);
    // No count text — scaled dots use tooltips instead
    const texts = container.querySelectorAll('text');
    expect(texts.length).toBe(0);
    // Tooltip shows event count
    const title = container.querySelector('title');
    expect(title).toBeInTheDocument();
    expect(title!.textContent).toBe('4 events');
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
    // Selected event should have a selection ring (stroke-width=2, not hover ring stroke-width=1.5)
    const selectionRings = container.querySelectorAll(
      'circle[stroke="var(--accent)"][stroke-width="2"]',
    );
    expect(selectionRings.length).toBe(1);
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
    // Only hover rings (stroke-width=1.5) should exist, no selection rings (stroke-width=2)
    const selectionRings = container.querySelectorAll(
      'circle[stroke="var(--accent)"][stroke-width="2"]',
    );
    expect(selectionRings.length).toBe(0);
  });

  it('pattern events use purple fill', () => {
    const events = [makePatternTimelineEvent({ id: 0, timestamp: 100 })];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    // 2 circles: hover-ring (fill=none) + main (fill=purple)
    const mainCircle = container.querySelector('circle[fill="var(--purple)"]');
    expect(mainCircle).toBeInTheDocument();
  });

  it('rule events use rule-border color based on colorIndex', () => {
    const events = [makeRuleTimelineEvent({ id: 0, timestamp: 100, colorIndex: 3 })];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    // 2 circles: hover-ring (fill=none) + main (fill=rule-border-3)
    const mainCircle = container.querySelector('circle[fill="var(--rule-border-3)"]');
    expect(mainCircle).toBeInTheDocument();
  });

  // --- State annotations ---

  it('renders tooltip with extracted state values', () => {
    const events = [
      makeRuleTimelineEvent({
        id: 0,
        timestamp: 100,
        ruleMatch: makeRuleMatch({
          extracted_state: {
            error_code: { String: 'E404' },
            count: { Integer: 42 },
          },
        }),
      }),
    ];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    const title = container.querySelector('title');
    expect(title).toBeInTheDocument();
    expect(title!.textContent).toContain('error_code=E404');
    expect(title!.textContent).toContain('count=42');
  });

  it('renders state label text when zoomed in', () => {
    const events = [
      makeRuleTimelineEvent({
        id: 0,
        timestamp: 100,
        ruleMatch: makeRuleMatch({
          extracted_state: { status: { String: 'failed' } },
        }),
      }),
    ];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1, // well under threshold of 50
      viewportHeight: 600,
    });
    const labels = container.querySelectorAll('.state-label');
    expect(labels.length).toBe(1);
    expect(labels[0].textContent).toBe('status=failed');
  });

  it('hides state labels when zoomed out', () => {
    const events = [
      makeRuleTimelineEvent({
        id: 0,
        timestamp: 100,
        ruleMatch: makeRuleMatch({
          extracted_state: { status: { String: 'failed' } },
        }),
      }),
    ];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 100, // above threshold
      viewportHeight: 600,
    });
    const labels = container.querySelectorAll('.state-label');
    expect(labels.length).toBe(0);
  });

  it('does not render tooltip or label when no extracted state', () => {
    const events = [
      makeRuleTimelineEvent({
        id: 0,
        timestamp: 100,
        ruleMatch: makeRuleMatch({ extracted_state: {} }),
      }),
    ];
    const { container } = renderSwimlane({
      events,
      minTime: 0,
      msPerPixel: 1,
      viewportHeight: 600,
    });
    expect(container.querySelector('title')).not.toBeInTheDocument();
    expect(container.querySelector('.state-label')).not.toBeInTheDocument();
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
