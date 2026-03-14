import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import RuleList from '../RuleList.svelte';
import { makeRule } from './fixtures';

vi.mock('../api', () => ({
  rules: {
    list: vi.fn().mockResolvedValue([]),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    get: vi.fn(),
  },
  rulesets: {
    list: vi.fn().mockResolvedValue([]),
    update: vi.fn(),
  },
  sources: {
    list: vi.fn().mockResolvedValue([]),
  },
  analysis: {
    suggestRule: vi.fn(),
    run: vi.fn(),
    runStreaming: vi.fn(),
    detectTemplate: vi.fn(),
    dryRun: vi.fn().mockResolvedValue([]),
  },
}));

import { rules as rulesApi } from '../api';

const ruleAlpha = makeRule({
  id: 1,
  name: 'Alpha Rule',
  match_rules: [{ id: 1, pattern: 'ALPHA_PATTERN' }],
  extraction_rules: [],
});

const ruleBeta = makeRule({
  id: 2,
  name: 'Beta Rule',
  match_rules: [{ id: 2, pattern: 'BETA_PATTERN' }],
  extraction_rules: [
    {
      id: 1,
      extraction_type: 'Parsed',
      state_key: 'beta_key',
      pattern: 'BETA (.+)',
      static_value: null,
      mode: 'Replace',
      event_text_only: false,
    },
  ],
});

const ruleGamma = makeRule({
  id: 3,
  name: 'Gamma Rule',
  match_rules: [{ id: 3, pattern: 'GAMMA_PATTERN' }],
  extraction_rules: [
    {
      id: 2,
      extraction_type: 'Static',
      state_key: 'gamma_key',
      pattern: null,
      static_value: 'static_val',
      mode: 'Replace',
      event_text_only: false,
    },
  ],
});

function renderRuleList() {
  return render(RuleList, { props: { projectId: 1 } });
}

describe('RuleList search and expand-all', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(rulesApi.list).mockResolvedValue([ruleAlpha, ruleBeta, ruleGamma]);
  });

  it('shows all rules when search is empty', async () => {
    renderRuleList();
    await waitFor(() => {
      expect(screen.getByText('Alpha Rule')).toBeInTheDocument();
      expect(screen.getByText('Beta Rule')).toBeInTheDocument();
      expect(screen.getByText('Gamma Rule')).toBeInTheDocument();
    });
  });

  it('filters rules by name', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Alpha Rule')).toBeInTheDocument());

    const searchInput = screen.getByPlaceholderText('Search rules...');
    await fireEvent.input(searchInput, { target: { value: 'alpha' } });
    await tick();

    expect(screen.getByText('Alpha Rule')).toBeInTheDocument();
    expect(screen.queryByText('Beta Rule')).toBeNull();
    expect(screen.queryByText('Gamma Rule')).toBeNull();
  });

  it('filters rules by match pattern', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Beta Rule')).toBeInTheDocument());

    const searchInput = screen.getByPlaceholderText('Search rules...');
    await fireEvent.input(searchInput, { target: { value: 'BETA_PATTERN' } });
    await tick();

    expect(screen.queryByText('Alpha Rule')).toBeNull();
    expect(screen.getByText('Beta Rule')).toBeInTheDocument();
    expect(screen.queryByText('Gamma Rule')).toBeNull();
  });

  it('filters rules by extraction rule state_key', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Gamma Rule')).toBeInTheDocument());

    const searchInput = screen.getByPlaceholderText('Search rules...');
    await fireEvent.input(searchInput, { target: { value: 'gamma_key' } });
    await tick();

    expect(screen.queryByText('Alpha Rule')).toBeNull();
    expect(screen.queryByText('Beta Rule')).toBeNull();
    expect(screen.getByText('Gamma Rule')).toBeInTheDocument();
  });

  it('auto-expands matched rules when search is active', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Beta Rule')).toBeInTheDocument());

    // Details should not be visible before searching
    expect(screen.queryByText('BETA_PATTERN')).toBeNull();

    const searchInput = screen.getByPlaceholderText('Search rules...');
    await fireEvent.input(searchInput, { target: { value: 'beta' } });
    await tick();

    // Match pattern should now be visible (auto-expanded)
    expect(screen.getByText('BETA_PATTERN')).toBeInTheDocument();
  });

  it('hides non-matching rules from the DOM', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Alpha Rule')).toBeInTheDocument());

    const searchInput = screen.getByPlaceholderText('Search rules...');
    await fireEvent.input(searchInput, { target: { value: 'gamma' } });
    await tick();

    expect(screen.queryByText('Alpha Rule')).toBeNull();
    expect(screen.queryByText('Beta Rule')).toBeNull();
    expect(screen.getByText('Gamma Rule')).toBeInTheDocument();
  });

  it('restores all rules when search is cleared', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Alpha Rule')).toBeInTheDocument());

    const searchInput = screen.getByPlaceholderText('Search rules...');
    await fireEvent.input(searchInput, { target: { value: 'alpha' } });
    await tick();

    expect(screen.queryByText('Beta Rule')).toBeNull();

    await fireEvent.input(searchInput, { target: { value: '' } });
    await tick();

    expect(screen.getByText('Alpha Rule')).toBeInTheDocument();
    expect(screen.getByText('Beta Rule')).toBeInTheDocument();
    expect(screen.getByText('Gamma Rule')).toBeInTheDocument();
  });

  it('shows no-match message when search has no hits', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Alpha Rule')).toBeInTheDocument());

    const searchInput = screen.getByPlaceholderText('Search rules...');
    await fireEvent.input(searchInput, { target: { value: 'zzz_no_match' } });
    await tick();

    expect(screen.getByText(/No rules match/)).toBeInTheDocument();
    expect(screen.queryByText('Alpha Rule')).toBeNull();
  });

  it('expand-all checkbox makes all rule details visible', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Alpha Rule')).toBeInTheDocument());

    // Details should not be visible before expanding
    expect(screen.queryByText('ALPHA_PATTERN')).toBeNull();
    expect(screen.queryByText('BETA_PATTERN')).toBeNull();
    expect(screen.queryByText('GAMMA_PATTERN')).toBeNull();

    const expandAllCheckbox = screen.getByRole('checkbox');
    await fireEvent.click(expandAllCheckbox);
    await tick();

    expect(screen.getByText('ALPHA_PATTERN')).toBeInTheDocument();
    expect(screen.getByText('BETA_PATTERN')).toBeInTheDocument();
    expect(screen.getByText('GAMMA_PATTERN')).toBeInTheDocument();
  });

  it('unchecking expand-all hides details for non-manually-expanded rules', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Alpha Rule')).toBeInTheDocument());

    const expandAllCheckbox = screen.getByRole('checkbox');
    await fireEvent.click(expandAllCheckbox);
    await tick();

    expect(screen.getByText('ALPHA_PATTERN')).toBeInTheDocument();

    // Uncheck expand-all
    await fireEvent.click(expandAllCheckbox);
    await tick();

    expect(screen.queryByText('ALPHA_PATTERN')).toBeNull();
    expect(screen.queryByText('BETA_PATTERN')).toBeNull();
    expect(screen.queryByText('GAMMA_PATTERN')).toBeNull();
  });

  it('search and expand-all coexist: search filters, expand-all shows details of filtered rules', async () => {
    renderRuleList();
    await waitFor(() => expect(screen.getByText('Alpha Rule')).toBeInTheDocument());

    // Enable expand-all
    const expandAllCheckbox = screen.getByRole('checkbox');
    await fireEvent.click(expandAllCheckbox);
    await tick();

    // Now filter by "alpha"
    const searchInput = screen.getByPlaceholderText('Search rules...');
    await fireEvent.input(searchInput, { target: { value: 'alpha' } });
    await tick();

    // Only Alpha Rule should be visible, with details expanded
    expect(screen.getByText('Alpha Rule')).toBeInTheDocument();
    expect(screen.getByText('ALPHA_PATTERN')).toBeInTheDocument();
    expect(screen.queryByText('Beta Rule')).toBeNull();
    expect(screen.queryByText('Gamma Rule')).toBeNull();
  });
});
