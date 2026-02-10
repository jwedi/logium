import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import StateEvolutionView from '../StateEvolutionView.svelte';
import { makeStateChange, makeSource, makeRule } from './fixtures';

function renderView(
  stateChanges = [makeStateChange()],
  sourceList = [makeSource()],
  ruleList = [makeRule()],
) {
  return render(StateEvolutionView, {
    props: { stateChanges, sourceList, ruleList },
  });
}

describe('StateEvolutionView', () => {
  it('shows empty message when no state changes', () => {
    renderView([]);
    expect(screen.getByText('No state changes to display.')).toBeInTheDocument();
  });

  it('renders table rows with correct data', () => {
    const sc = makeStateChange({
      source_name: 'server',
      state_key: 'region',
      old_value: { String: 'us-west' },
      new_value: { String: 'us-east' },
      rule_id: 1,
    });
    renderView([sc], [makeSource({ name: 'server' })], [makeRule({ id: 1, name: 'Region Rule' })]);

    // Source name appears in both filter dropdown and table cell
    expect(screen.getAllByText('server').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('region').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('us-west')).toBeInTheDocument();
    expect(screen.getByText('us-east')).toBeInTheDocument();
    expect(screen.getByText('Region Rule')).toBeInTheDocument();
  });

  it('filters by source', async () => {
    const changes = [
      makeStateChange({ source_name: 'server', state_key: 'region' }),
      makeStateChange({ source_name: 'client', state_key: 'status' }),
    ];
    renderView(changes, [makeSource({ name: 'server' }), makeSource({ id: 2, name: 'client' })]);

    // Both visible initially
    expect(screen.getByText('2 changes')).toBeInTheDocument();

    // Filter to server
    const sourceSelect = screen.getAllByRole('combobox')[0];
    await fireEvent.change(sourceSelect, { target: { value: 'server' } });

    expect(screen.getByText('1 change')).toBeInTheDocument();
  });

  it('filters by key', async () => {
    const changes = [
      makeStateChange({ state_key: 'region', source_name: 'server' }),
      makeStateChange({ state_key: 'status', source_name: 'server' }),
    ];
    renderView(changes);

    // Both visible initially
    expect(screen.getByText('2 changes')).toBeInTheDocument();

    // Filter to region key
    const keySelect = screen.getAllByRole('combobox')[1];
    await fireEvent.change(keySelect, { target: { value: 'region' } });

    expect(screen.getByText('1 change')).toBeInTheDocument();
  });

  it('displays "(none)" for null old_value (first set)', () => {
    const sc = makeStateChange({ old_value: null, new_value: { String: 'val' } });
    renderView([sc]);

    const noneElements = screen.getAllByText('(none)');
    expect(noneElements.length).toBeGreaterThanOrEqual(1);
  });

  it('displays "(none)" for null new_value (clear)', () => {
    const sc = makeStateChange({
      old_value: { String: 'val' },
      new_value: null,
    });
    renderView([sc]);

    const noneElements = screen.getAllByText('(none)');
    expect(noneElements.length).toBeGreaterThanOrEqual(1);
  });

  it('shows rule names from ruleList', () => {
    const sc = makeStateChange({ rule_id: 42 });
    renderView([sc], [makeSource()], [makeRule({ id: 42, name: 'My Custom Rule' })]);

    expect(screen.getByText('My Custom Rule')).toBeInTheDocument();
  });

  it('matches snapshot', () => {
    const changes = [
      makeStateChange({
        source_name: 'server',
        state_key: 'region',
        old_value: null,
        new_value: { String: 'us-east' },
        rule_id: 1,
      }),
      makeStateChange({
        source_name: 'server',
        state_key: 'count',
        old_value: { Integer: 10 },
        new_value: { Integer: 42 },
        rule_id: 2,
      }),
    ];
    const { container } = renderView(
      changes,
      [makeSource({ name: 'server' })],
      [makeRule({ id: 1, name: 'Region Rule' }), makeRule({ id: 2, name: 'Count Rule' })],
    );

    expect(container.innerHTML).toMatchSnapshot();
  });
});
