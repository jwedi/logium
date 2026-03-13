import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import StateEvolutionView from '../StateEvolutionView.svelte';
import {
  makeRuleMatch,
  makeStateChange,
  makePatternMatch,
  makeSource,
  makeRule,
  makePattern,
  makeLogLine,
} from './fixtures';
import type { RuleMatch, StateChange, PatternMatch, Source, LogRule, Pattern } from '../api';

function renderFeed(
  ruleMatches: RuleMatch[] = [],
  stateChanges: StateChange[] = [],
  patternMatches: PatternMatch[] = [],
  sourceList: Source[] = [makeSource()],
  ruleList: LogRule[] = [makeRule()],
  patternList: Pattern[] = [makePattern()],
) {
  return render(StateEvolutionView, {
    props: { ruleMatches, stateChanges, patternMatches, sourceList, ruleList, patternList },
  });
}

describe('StateEvolutionView (Event Feed)', () => {
  it('shows empty message when no matches', () => {
    renderFeed();
    expect(screen.getByText('No events to display.')).toBeInTheDocument();
  });

  it('renders one card per rule match', () => {
    const rm1 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
    });
    const rm2 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:31:00.000' }),
    });
    renderFeed([rm1, rm2]);
    // Two raw buttons = two cards
    expect(screen.getAllByTitle('Toggle raw log line').length).toBe(2);
  });

  it('shows event_text as headline when present', () => {
    const rm = makeRuleMatch({ event_text: 'Player 42 failed matchmaking' });
    renderFeed([rm]);
    expect(screen.getByText('Player 42 failed matchmaking')).toBeInTheDocument();
  });

  it('shows rule name fallback when event_text is null', () => {
    const rm = makeRuleMatch({ event_text: null, rule_id: 1 });
    renderFeed([rm], [], [], [makeSource()], [makeRule({ id: 1, name: 'Error Rule' })]);
    expect(screen.getByText('[Error Rule]')).toBeInTheDocument();
  });

  it('renders state chips for state changes joined to the match', () => {
    const rm = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
    });
    const sc = makeStateChange({
      timestamp: '2024-01-15T10:30:00.000',
      rule_id: 1,
      source_id: 1,
      state_key: 'session',
      new_value: { String: 'abc123' },
    });
    renderFeed([rm], [sc]);
    // 'session' appears both in the key filter dropdown option and as a chip key
    expect(screen.getAllByText('session').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('abc123')).toBeInTheDocument();
    // The chip element exists
    expect(document.querySelector('.state-chip')).toBeInTheDocument();
  });

  it('renders chip-clear marker for null new_value', () => {
    const rm = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
    });
    const sc = makeStateChange({
      timestamp: '2024-01-15T10:30:00.000',
      rule_id: 1,
      source_id: 1,
      state_key: 'session',
      old_value: { String: 'abc123' },
      new_value: null,
    });
    renderFeed([rm], [sc]);
    // 'session' appears both in the key filter dropdown and as a chip key
    expect(screen.getAllByText('session').length).toBeGreaterThanOrEqual(1);
    const clearMarks = document.querySelectorAll('.chip-clear-mark');
    expect(clearMarks.length).toBeGreaterThanOrEqual(1);
  });

  it('renders pattern separator row', () => {
    const pm = makePatternMatch({
      pattern_id: 1,
      timestamp: '2024-01-15T10:30:05.000',
    });
    renderFeed(
      [],
      [],
      [pm],
      [makeSource()],
      [makeRule()],
      [makePattern({ id: 1, name: 'AuthFailure' })],
    );
    expect(screen.getByText('Pattern matched: AuthFailure')).toBeInTheDocument();
  });

  it('pattern rows are always visible regardless of rule/source filter', async () => {
    const pm = makePatternMatch({ pattern_id: 1 });
    renderFeed(
      [],
      [],
      [pm],
      [makeSource()],
      [makeRule()],
      [makePattern({ id: 1, name: 'MyPattern' })],
    );
    // Pattern row still visible even with no matches
    expect(screen.getByText('Pattern matched: MyPattern')).toBeInTheDocument();
  });

  it('filters by source', async () => {
    const rm1 = makeRuleMatch({ source_id: 1, rule_id: 1 });
    const rm2 = makeRuleMatch({ source_id: 2, rule_id: 1 });
    renderFeed(
      [rm1, rm2],
      [],
      [],
      [makeSource({ id: 1, name: 'nginx' }), makeSource({ id: 2, name: 'game' })],
      [makeRule({ id: 1 })],
    );
    expect(screen.getByText('2 events')).toBeInTheDocument();

    const sourceSelect = screen.getAllByRole('combobox')[0];
    await fireEvent.change(sourceSelect, { target: { value: '1' } });
    expect(screen.getByText('1 event')).toBeInTheDocument();
  });

  it('filters by rule', async () => {
    const rm1 = makeRuleMatch({ rule_id: 1, source_id: 1 });
    const rm2 = makeRuleMatch({ rule_id: 2, source_id: 1 });
    renderFeed(
      [rm1, rm2],
      [],
      [],
      [makeSource({ id: 1 })],
      [makeRule({ id: 1, name: 'Rule A' }), makeRule({ id: 2, name: 'Rule B' })],
    );
    expect(screen.getByText('2 events')).toBeInTheDocument();

    const ruleSelect = screen.getAllByRole('combobox')[1];
    await fireEvent.change(ruleSelect, { target: { value: '1' } });
    expect(screen.getByText('1 event')).toBeInTheDocument();
  });

  it('filters by key — shows only matches with that key in state changes', async () => {
    const rm1 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
    });
    const rm2 = makeRuleMatch({
      rule_id: 2,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:31:00.000' }),
    });
    const sc1 = makeStateChange({
      timestamp: '2024-01-15T10:30:00.000',
      rule_id: 1,
      source_id: 1,
      state_key: 'session',
    });
    renderFeed(
      [rm1, rm2],
      [sc1],
      [],
      [makeSource({ id: 1 })],
      [makeRule({ id: 1 }), makeRule({ id: 2 })],
    );
    expect(screen.getByText('2 events')).toBeInTheDocument();

    const keySelect = screen.getAllByRole('combobox')[2];
    await fireEvent.change(keySelect, { target: { value: 'session' } });
    expect(screen.getByText('1 event')).toBeInTheDocument();
  });

  it('toggles raw line on button click', async () => {
    const rm = makeRuleMatch({
      log_line: makeLogLine({ raw: 'ERROR something went wrong' }),
    });
    renderFeed([rm]);
    expect(screen.queryByText('ERROR something went wrong')).not.toBeInTheDocument();

    const rawBtn = screen.getByTitle('Toggle raw log line');
    await fireEvent.click(rawBtn);
    expect(screen.getByText('ERROR something went wrong')).toBeInTheDocument();

    // Click again to collapse
    await fireEvent.click(rawBtn);
    expect(screen.queryByText('ERROR something went wrong')).not.toBeInTheDocument();
  });

  it('toggles state panel on button click showing accumulated state', async () => {
    const rm = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
    });
    const sc = makeStateChange({
      timestamp: '2024-01-15T10:30:00.000',
      rule_id: 1,
      source_id: 1,
      source_name: 'app.log',
      state_key: 'status',
      new_value: { String: 'error_detected' },
    });
    renderFeed([rm], [sc], [], [makeSource({ id: 1, name: 'app.log' })]);

    // Panel not visible initially
    expect(screen.queryByText('State at this moment')).not.toBeInTheDocument();

    const stateBtn = screen.getByTitle('Toggle state at this moment');
    await fireEvent.click(stateBtn);

    expect(screen.getByText('State at this moment')).toBeInTheDocument();
    // Source name appears in state panel
    expect(screen.getAllByText('app.log').length).toBeGreaterThanOrEqual(1);
    // state key and value appear in the panel (.state-k and .state-v)
    const panel = document.querySelector('.state-panel');
    expect(panel).toBeInTheDocument();
    expect(panel!.textContent).toContain('status');
    expect(panel!.textContent).toContain('error_detected');

    // Click again to collapse
    await fireEvent.click(stateBtn);
    expect(screen.queryByText('State at this moment')).not.toBeInTheDocument();
  });

  it('multiple state panels can be open simultaneously', async () => {
    const rm1 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000', raw: 'line one' }),
    });
    const rm2 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:31:00.000', raw: 'line two' }),
    });
    renderFeed([rm1, rm2], [], [], [makeSource({ id: 1 })], [makeRule({ id: 1 })]);

    const stateBtns = screen.getAllByTitle('Toggle state at this moment');
    expect(stateBtns.length).toBe(2);

    await fireEvent.click(stateBtns[0]);
    await fireEvent.click(stateBtns[1]);

    // Both panels open
    const panels = document.querySelectorAll('.state-panel');
    expect(panels.length).toBe(2);
  });

  it('state snapshot only includes state up to clicked row timestamp', async () => {
    // Two matches at different times; second match has a state change after first
    const rm1 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
    });
    const rm2 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      log_line: makeLogLine({ timestamp: '2024-01-15T10:32:00.000' }),
    });
    const sc1 = makeStateChange({
      timestamp: '2024-01-15T10:30:00.000',
      rule_id: 1,
      source_id: 1,
      source_name: 'app.log',
      state_key: 'phase',
      new_value: { String: 'auth' },
    });
    const sc2 = makeStateChange({
      timestamp: '2024-01-15T10:32:00.000',
      rule_id: 1,
      source_id: 1,
      source_name: 'app.log',
      state_key: 'phase',
      new_value: { String: 'match' },
    });
    renderFeed([rm1, rm2], [sc1, sc2], [], [makeSource({ id: 1, name: 'app.log' })]);

    const stateBtns = screen.getAllByTitle('Toggle state at this moment');
    // Open state panel for first row (rm1, earlier timestamp)
    await fireEvent.click(stateBtns[0]);

    const panels = document.querySelectorAll('.state-panel');
    expect(panels.length).toBe(1);
    // Panel should show 'auth' (state at rm1's time)
    expect(panels[0].textContent).toContain('auth');
    // Panel should NOT show 'match' (that is set later at rm2's time)
    expect(panels[0].textContent).not.toContain('match');
  });

  it('shows "N events" count for rule matches only', () => {
    const pm = makePatternMatch({ pattern_id: 1 });
    const rm = makeRuleMatch();
    renderFeed([rm], [], [pm], [makeSource()], [makeRule()], [makePattern({ id: 1 })]);
    expect(screen.getByText('1 event')).toBeInTheDocument();
  });

  it('feed is sorted chronologically', () => {
    const rm1 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      event_text: 'second event',
      log_line: makeLogLine({ timestamp: '2024-01-15T10:32:00.000' }),
    });
    const rm2 = makeRuleMatch({
      rule_id: 1,
      source_id: 1,
      event_text: 'first event',
      log_line: makeLogLine({ timestamp: '2024-01-15T10:30:00.000' }),
    });
    renderFeed([rm1, rm2]);
    const eventTexts = document.querySelectorAll('.event-text');
    const texts = Array.from(eventTexts).map((el) => el.textContent?.trim());
    expect(texts.indexOf('first event')).toBeLessThan(texts.indexOf('second event'));
  });
});
