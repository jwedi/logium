import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TimelineView from '../TimelineView.svelte';
import {
  makeAnalysisResult,
  makeSource,
  makeRule,
  makePattern,
  makeRuleMatch,
  makePatternMatch,
  makeLogLine,
} from './fixtures';

function renderView(propsOverrides: Record<string, any> = {}) {
  const defaults = {
    result: makeAnalysisResult(),
    sourceList: [makeSource({ id: 1, name: 'app.log' })],
    ruleList: [makeRule({ id: 1, name: 'Error Rule' })],
    patternList: [makePattern({ id: 1, name: 'Failure Pattern' })],
  };
  return render(TimelineView, { props: { ...defaults, ...propsOverrides } });
}

describe('TimelineView', () => {
  // --- Behavioral ---

  it('shows empty message when analysis has no events', () => {
    renderView({
      result: makeAnalysisResult({
        rule_matches: [
          // Use an invalid timestamp that Date.parse returns NaN for
          makeRuleMatch({ log_line: makeLogLine({ timestamp: 'not-a-date' }) }),
        ],
        pattern_matches: [],
      }),
    });
    expect(screen.getByText('No timestamped events to display on timeline.')).toBeInTheDocument();
  });

  it('renders lane headers with source names and event counts', () => {
    const { container } = renderView({
      sourceList: [
        makeSource({ id: 1, name: 'nginx.log' }),
        makeSource({ id: 2, name: 'app.log' }),
      ],
      result: makeAnalysisResult({
        rule_matches: [
          makeRuleMatch({ source_id: 1 }),
          makeRuleMatch({ source_id: 1 }),
          makeRuleMatch({ source_id: 2 }),
        ],
        pattern_matches: [],
      }),
    });
    expect(screen.getByText('nginx.log')).toBeInTheDocument();
    expect(screen.getByText('app.log')).toBeInTheDocument();
    // Check lane counts via class selector to avoid collision with cluster badges
    const laneCounts = container.querySelectorAll('.lane-count');
    const countTexts = Array.from(laneCounts).map((el) => el.textContent);
    expect(countTexts).toContain('2');
    expect(countTexts).toContain('1');
  });

  it('zoom in changes label from 100% to 150%', async () => {
    renderView();
    expect(screen.getByText('100%')).toBeInTheDocument();
    await fireEvent.click(screen.getByTitle('Zoom in'));
    expect(screen.getByText('150%')).toBeInTheDocument();
  });

  it('zoom out changes label to 67%', async () => {
    renderView();
    await fireEvent.click(screen.getByTitle('Zoom out'));
    expect(screen.getByText('67%')).toBeInTheDocument();
  });

  it('reset returns to 100%', async () => {
    renderView();
    await fireEvent.click(screen.getByTitle('Zoom in'));
    expect(screen.getByText('150%')).toBeInTheDocument();
    await fireEvent.click(screen.getByTitle('Reset zoom'));
    expect(screen.getByText('100%')).toBeInTheDocument();
  });

  it('detail panel not rendered when no event is selected', () => {
    const { container } = renderView();
    expect(container.querySelector('.detail-panel')).not.toBeInTheDocument();
  });

  it('skips events with invalid timestamps (NaN)', () => {
    const { container } = renderView({
      result: makeAnalysisResult({
        rule_matches: [
          makeRuleMatch({
            source_id: 1,
            log_line: makeLogLine({ timestamp: 'invalid' }),
          }),
          makeRuleMatch({
            source_id: 1,
            log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
          }),
        ],
        pattern_matches: [],
      }),
    });
    // Only one valid event, so only one dot should exist
    // The component should still render (not show empty message) since one event is valid
    expect(container.querySelector('.empty')).not.toBeInTheDocument();
  });

  it('shows empty message when result has zero matches', () => {
    renderView({
      result: makeAnalysisResult({
        rule_matches: [],
        pattern_matches: [],
      }),
    });
    expect(screen.getByText('No timestamped events to display on timeline.')).toBeInTheDocument();
  });

  it('renders zoom controls', () => {
    renderView();
    expect(screen.getByTitle('Zoom in')).toBeInTheDocument();
    expect(screen.getByTitle('Zoom out')).toBeInTheDocument();
    expect(screen.getByTitle('Reset zoom')).toBeInTheDocument();
  });

  // --- Snapshot ---

  it('matches snapshot with two sources and a pattern match', () => {
    const { container } = renderView({
      sourceList: [makeSource({ id: 1, name: 'src-a' }), makeSource({ id: 2, name: 'src-b' })],
      result: makeAnalysisResult({
        rule_matches: [
          makeRuleMatch({
            source_id: 1,
            log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
          }),
          makeRuleMatch({
            source_id: 2,
            log_line: makeLogLine({ timestamp: '2024-01-15T10:30:01.000' }),
          }),
        ],
        pattern_matches: [makePatternMatch({ timestamp: '2024-01-15T10:30:02.000' })],
      }),
    });
    expect(container.innerHTML).toMatchSnapshot();
  });
});
