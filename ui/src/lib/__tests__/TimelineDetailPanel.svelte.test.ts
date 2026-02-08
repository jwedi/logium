import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TimelineDetailPanel from '../TimelineDetailPanel.svelte';
import {
  makeRuleTimelineEvent,
  makePatternTimelineEvent,
  makeRuleMatch,
  makePatternMatch,
  makeLogLine,
  makeSource,
  makeRule,
  makePattern,
} from './fixtures';

function renderPanel(propsOverrides: Record<string, any> = {}) {
  const defaults = {
    event: makeRuleTimelineEvent(),
    sourceList: [makeSource()],
    ruleList: [makeRule()],
    patternList: [makePattern()],
    onClose: vi.fn(),
  };
  return render(TimelineDetailPanel, { props: { ...defaults, ...propsOverrides } });
}

describe('TimelineDetailPanel', () => {
  // --- Behavioral ---

  it('shows "Rule Match" heading for rule events', () => {
    renderPanel({ event: makeRuleTimelineEvent() });
    expect(screen.getByText('Rule Match')).toBeInTheDocument();
  });

  it('shows "Pattern Match" heading for pattern events', () => {
    renderPanel({ event: makePatternTimelineEvent() });
    expect(screen.getByText('Pattern Match')).toBeInTheDocument();
  });

  it('looks up rule name from ruleList', () => {
    renderPanel({
      event: makeRuleTimelineEvent({ ruleId: 1, ruleMatch: makeRuleMatch({ rule_id: 1 }) }),
      ruleList: [makeRule({ id: 1, name: 'My Custom Rule' })],
    });
    expect(screen.getByText('My Custom Rule')).toBeInTheDocument();
  });

  it('falls back to Rule #N when rule not found', () => {
    renderPanel({
      event: makeRuleTimelineEvent({ ruleId: 99, ruleMatch: makeRuleMatch({ rule_id: 99 }) }),
      ruleList: [],
    });
    expect(screen.getByText('Rule #99')).toBeInTheDocument();
  });

  it('looks up source name from sourceList', () => {
    renderPanel({
      event: makeRuleTimelineEvent({ sourceId: 1, ruleMatch: makeRuleMatch({ source_id: 1 }) }),
      sourceList: [makeSource({ id: 1, name: 'my-source.log' })],
    });
    expect(screen.getByText('my-source.log')).toBeInTheDocument();
  });

  it('displays log line content when available', () => {
    renderPanel({
      event: makeRuleTimelineEvent({
        ruleMatch: makeRuleMatch({
          log_line: makeLogLine({ content: 'parsed content', raw: 'raw line' }),
        }),
      }),
    });
    expect(screen.getByText('parsed content')).toBeInTheDocument();
  });

  it('falls back to raw when content is empty', () => {
    renderPanel({
      event: makeRuleTimelineEvent({
        ruleMatch: makeRuleMatch({
          log_line: makeLogLine({ content: '', raw: 'raw fallback line' }),
        }),
      }),
    });
    expect(screen.getByText('raw fallback line')).toBeInTheDocument();
  });

  it('renders extracted state key-value pairs', () => {
    renderPanel({
      event: makeRuleTimelineEvent({
        ruleMatch: makeRuleMatch({
          extracted_state: {
            error_code: { String: 'E404' },
            count: { Integer: 42 },
          },
        }),
      }),
    });
    expect(screen.getByText('error_code')).toBeInTheDocument();
    expect(screen.getByText('E404')).toBeInTheDocument();
    expect(screen.getByText('count')).toBeInTheDocument();
    expect(screen.getByText('42')).toBeInTheDocument();
  });

  it('hides extracted state section when state is empty', () => {
    const { container } = renderPanel({
      event: makeRuleTimelineEvent({
        ruleMatch: makeRuleMatch({ extracted_state: {} }),
      }),
    });
    expect(container.querySelector('.state-table')).not.toBeInTheDocument();
  });

  it('renders pattern match state snapshot grouped by source name', () => {
    renderPanel({
      event: makePatternTimelineEvent({
        patternMatch: makePatternMatch({
          state_snapshot: {
            'nginx.log': { status: { Integer: 500 } },
            'app.log': { error: { String: 'timeout' } },
          },
        }),
      }),
    });
    expect(screen.getByText('nginx.log')).toBeInTheDocument();
    expect(screen.getByText('500')).toBeInTheDocument();
    expect(screen.getByText('app.log')).toBeInTheDocument();
    expect(screen.getByText('timeout')).toBeInTheDocument();
  });

  it('formats all StateValue variants', () => {
    renderPanel({
      event: makeRuleTimelineEvent({
        ruleMatch: makeRuleMatch({
          extracted_state: {
            str: { String: 'hello' },
            int: { Integer: 7 },
            flt: { Float: 3.14 },
            bool: { Bool: true },
          },
        }),
      }),
    });
    expect(screen.getByText('hello')).toBeInTheDocument();
    expect(screen.getByText('7')).toBeInTheDocument();
    expect(screen.getByText('3.14')).toBeInTheDocument();
    expect(screen.getByText('true')).toBeInTheDocument();
  });

  it('calls onClose callback when close button clicked', async () => {
    const onClose = vi.fn();
    renderPanel({ onClose });
    await fireEvent.click(screen.getByText('x'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('formats timestamp from ms to ISO-like string', () => {
    const ts = Date.parse('2024-01-15T10:30:00.000Z');
    renderPanel({ event: makeRuleTimelineEvent({ timestamp: ts }) });
    expect(screen.getByText('2024-01-15 10:30:00.000')).toBeInTheDocument();
  });

  it('looks up pattern name from patternList', () => {
    renderPanel({
      event: makePatternTimelineEvent({
        patternId: 1,
        patternMatch: makePatternMatch({ pattern_id: 1 }),
      }),
      patternList: [makePattern({ id: 1, name: 'My Pattern' })],
    });
    expect(screen.getByText('My Pattern')).toBeInTheDocument();
  });

  it('falls back to Pattern #N when pattern not found', () => {
    renderPanel({
      event: makePatternTimelineEvent({
        patternId: 77,
        patternMatch: makePatternMatch({ pattern_id: 77 }),
      }),
      patternList: [],
    });
    expect(screen.getByText('Pattern #77')).toBeInTheDocument();
  });

  // --- Snapshots ---

  it('matches snapshot for rule match panel', () => {
    const { container } = renderPanel({
      event: makeRuleTimelineEvent({
        ruleMatch: makeRuleMatch({
          extracted_state: { code: { String: '500' } },
        }),
      }),
    });
    expect(container.innerHTML).toMatchSnapshot();
  });

  it('matches snapshot for pattern match panel', () => {
    const { container } = renderPanel({
      event: makePatternTimelineEvent({
        patternMatch: makePatternMatch({
          state_snapshot: {
            src1: { key1: { String: 'val1' } },
          },
        }),
      }),
    });
    expect(container.innerHTML).toMatchSnapshot();
  });
});
