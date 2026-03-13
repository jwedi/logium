import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import RuleCreator from '../RuleCreator.svelte';
import {
  makeRule,
  makeRuleset,
  makeSource,
  makeRuleMatch,
  makeLogLine,
  makeSuggestRuleResponse,
} from './fixtures';
import * as invalidation from '../analysisInvalidation.svelte';

// vi.mock factory is hoisted — cannot reference imported helpers, so use plain objects
vi.mock('../api', () => ({
  rules: {
    create: vi.fn(),
    list: vi.fn().mockResolvedValue([]),
  },
  rulesets: {
    list: vi.fn(),
    update: vi.fn(),
  },
  sources: {
    list: vi.fn(),
  },
  analysis: {
    suggestRule: vi.fn(),
    run: vi.fn(),
    runStreaming: vi.fn(),
    detectTemplate: vi.fn(),
    dryRun: vi.fn(),
  },
}));

import {
  rules as rulesApi,
  rulesets as rulesetsApi,
  sources as sourcesApi,
  analysis as analysisApi,
} from '../api';

function renderRuleCreator(overrides: Record<string, any> = {}) {
  const props = {
    projectId: 1,
    selectedText: 'ERROR something failed',
    sourceTemplateId: 1,
    onClose: vi.fn(),
    onCreated: vi.fn(),
    ...overrides,
  };
  return { ...render(RuleCreator, { props }), props };
}

function getTextarea(): HTMLTextAreaElement {
  return document.querySelector('textarea') as HTMLTextAreaElement;
}

function getRulesetSelect(): HTMLSelectElement | null {
  // Find the select that follows the "Assign to Ruleset" label
  const labels = document.querySelectorAll('label');
  for (const label of labels) {
    if (label.textContent?.trim() === 'Assign to Ruleset') {
      const field = label.closest('.field');
      return field?.querySelector('select') ?? null;
    }
  }
  return null;
}

describe('RuleCreator', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(analysisApi.suggestRule).mockResolvedValue(makeSuggestRuleResponse());
    vi.mocked(rulesApi.create).mockResolvedValue(makeRule({ id: 42 }));
    vi.mocked(rulesetsApi.list).mockResolvedValue([
      makeRuleset({ id: 1, name: 'Server Rules', template_id: 1, rule_ids: [10, 20] }),
      makeRuleset({ id: 2, name: 'Client Rules', template_id: 2, rule_ids: [] }),
      makeRuleset({ id: 3, name: 'More Server Rules', template_id: 1, rule_ids: [30] }),
    ]);
    vi.mocked(rulesetsApi.update).mockResolvedValue({} as any);
    vi.mocked(sourcesApi.list).mockResolvedValue([
      makeSource({ id: 1, name: 'app.log', template_id: 1 }),
      makeSource({ id: 2, name: 'server.log', template_id: 1 }),
      makeSource({ id: 3, name: 'client.log', template_id: 2 }),
    ]);
  });

  it('renders modal with selected text displayed', async () => {
    renderRuleCreator();
    await tick();

    expect(screen.getByText('Create Rule from Selection')).toBeInTheDocument();
    expect(screen.getByText('ERROR something failed')).toBeInTheDocument();
  });

  it('calls suggest-rule API on mount and populates regex field', async () => {
    renderRuleCreator();
    await tick();

    await waitFor(() => {
      expect(analysisApi.suggestRule).toHaveBeenCalledWith(1, { text: 'ERROR something failed' });
    });

    await waitFor(() => {
      expect(getTextarea().value).toBe('ERROR (?P<message>.+)');
    });
  });

  it('falls back to escaped pattern when suggest-rule API fails', async () => {
    vi.mocked(analysisApi.suggestRule).mockRejectedValue(new Error('Network error'));

    renderRuleCreator({ selectedText: 'Count: 42' });
    await tick();

    await waitFor(() => {
      // Escaped text with numbers replaced by (\d+)
      expect(getTextarea().value).toBe('Count: (\\d+)');
    });
  });

  it('shows matching rulesets filtered by template_id', async () => {
    renderRuleCreator({ sourceTemplateId: 1 });
    await tick();

    await waitFor(() => {
      expect(screen.getByText('Assign to Ruleset')).toBeInTheDocument();
    });

    // Should show rulesets with template_id 1 only
    expect(screen.getByText('Server Rules')).toBeInTheDocument();
    expect(screen.getByText('More Server Rules')).toBeInTheDocument();
    // Client Rules has template_id 2, should NOT appear in the dropdown
    const options = screen.getAllByRole('option');
    const optionTexts = options.map((o) => o.textContent);
    expect(optionTexts).not.toContain('Client Rules');
  });

  it('auto-selects ruleset when only one matches template', async () => {
    vi.mocked(rulesetsApi.list).mockResolvedValue([
      makeRuleset({ id: 5, name: 'Only One', template_id: 3, rule_ids: [] }),
      makeRuleset({ id: 6, name: 'Other Template', template_id: 99, rule_ids: [] }),
    ]);

    renderRuleCreator({ sourceTemplateId: 3 });
    await tick();

    await waitFor(() => {
      expect(screen.getByText('Assign to Ruleset')).toBeInTheDocument();
    });

    const select = getRulesetSelect()!;
    expect(select).not.toBeNull();
    expect(select.value).toBe('5');
  });

  it('creates rule and assigns to selected ruleset on save', async () => {
    renderRuleCreator({ sourceTemplateId: 1 });
    await tick();

    // Wait for API calls to settle
    await waitFor(() => {
      expect(screen.getByText('Assign to Ruleset')).toBeInTheDocument();
    });

    // Fill in rule name
    const nameInput = screen.getByPlaceholderText('My rule...');
    await fireEvent.input(nameInput, { target: { value: 'My Error Rule' } });

    // Select the first ruleset
    const rulesetSelect = getRulesetSelect()!;
    await fireEvent.change(rulesetSelect, { target: { value: '1' } });

    // Click save
    await fireEvent.click(screen.getByText('Create Rule'));

    await waitFor(() => {
      expect(rulesApi.create).toHaveBeenCalledTimes(1);
    });

    // Verify extraction rules include pattern and static_value fields
    const createCall = vi.mocked(rulesApi.create).mock.calls[0];
    const payload = createCall[1];
    expect(payload.extraction_rules).toEqual([
      {
        id: 0,
        extraction_type: 'Parsed',
        state_key: 'message',
        pattern: 'ERROR (?P<message>.+)',
        static_value: null,
        mode: 'Replace',
        event_text_only: false,
      },
    ]);

    // Should update ruleset with the new rule ID appended
    await waitFor(() => {
      expect(rulesetsApi.update).toHaveBeenCalledWith(1, 1, {
        name: 'Server Rules',
        template_id: 1,
        rule_ids: [10, 20, 42],
      });
    });
  });

  it('creates rule without ruleset assignment when "None" selected', async () => {
    renderRuleCreator({ sourceTemplateId: 1 });
    await tick();

    await waitFor(() => {
      expect(screen.getByText('Assign to Ruleset')).toBeInTheDocument();
    });

    // Fill in rule name
    const nameInput = screen.getByPlaceholderText('My rule...');
    await fireEvent.input(nameInput, { target: { value: 'My Error Rule' } });

    // Ensure "None" is selected (default — value is '')
    const rulesetSelect = getRulesetSelect()!;
    await fireEvent.change(rulesetSelect, { target: { value: '' } });

    await fireEvent.click(screen.getByText('Create Rule'));

    await waitFor(() => {
      expect(rulesApi.create).toHaveBeenCalledTimes(1);
    });

    // Should NOT update any ruleset
    expect(rulesetsApi.update).not.toHaveBeenCalled();
  });

  it('disables save button when name is empty', async () => {
    renderRuleCreator();
    await tick();

    await waitFor(() => {
      expect(analysisApi.suggestRule).toHaveBeenCalled();
    });

    const saveBtn = screen.getByText('Create Rule');
    expect(saveBtn).toBeDisabled();
  });

  it('adds a manual extraction rule via "+ Add" button', async () => {
    renderRuleCreator();
    await tick();

    await waitFor(() => {
      expect(analysisApi.suggestRule).toHaveBeenCalled();
    });

    // Should already have one auto-seeded extraction rule from the suggested pattern
    await waitFor(() => {
      expect(screen.getAllByPlaceholderText('state_key').length).toBe(1);
    });

    // Click "+ Add" to add another extraction rule
    await fireEvent.click(screen.getByText('+ Add'));

    // Now there should be two state_key inputs
    expect(screen.getAllByPlaceholderText('state_key').length).toBe(2);
  });

  it('removes an extraction rule via the remove button', async () => {
    renderRuleCreator();
    await tick();

    await waitFor(() => {
      expect(analysisApi.suggestRule).toHaveBeenCalled();
    });

    // Should have one auto-seeded extraction rule
    await waitFor(() => {
      expect(screen.getAllByPlaceholderText('state_key').length).toBe(1);
    });

    // Click the "x" remove button
    const removeButtons = screen.getAllByText('x');
    // Filter to only the extraction rule remove button (not the modal close button)
    const extractionRemoveBtn = removeButtons.find(
      (btn) => btn.classList.contains('danger') || btn.closest('.group-row'),
    );
    expect(extractionRemoveBtn).toBeTruthy();
    await fireEvent.click(extractionRemoveBtn!);

    // No more state_key inputs
    expect(screen.queryAllByPlaceholderText('state_key').length).toBe(0);
  });

  it('calls invalidateAnalysis after successful save', async () => {
    const spy = vi.spyOn(invalidation, 'invalidateAnalysis');

    renderRuleCreator({ sourceTemplateId: 1 });
    await tick();

    await waitFor(() => {
      expect(screen.getByText('Assign to Ruleset')).toBeInTheDocument();
    });

    const nameInput = screen.getByPlaceholderText('My rule...');
    await fireEvent.input(nameInput, { target: { value: 'My Error Rule' } });

    await fireEvent.click(screen.getByText('Create Rule'));

    await waitFor(() => {
      expect(rulesApi.create).toHaveBeenCalledTimes(1);
    });

    await waitFor(() => {
      expect(spy).toHaveBeenCalled();
    });

    spy.mockRestore();
  });

  // -----------------------------------------------------------------------
  // Dry-run tests
  // -----------------------------------------------------------------------

  it('shows source dropdown filtered by template_id', async () => {
    renderRuleCreator({ sourceTemplateId: 1 });
    await tick();

    await waitFor(() => {
      expect(screen.getByText('Test Against Source')).toBeInTheDocument();
    });

    // Should show matching sources (template_id=1)
    expect(screen.getByText('app.log')).toBeInTheDocument();
    expect(screen.getByText('server.log')).toBeInTheDocument();
    // Should NOT show non-matching source (template_id=2)
    const options = document.querySelectorAll('.dry-run-controls option');
    const optionTexts = Array.from(options).map((o) => o.textContent);
    expect(optionTexts).not.toContain('client.log');
  });

  it('calls dry run API and displays results', async () => {
    vi.mocked(analysisApi.dryRun).mockResolvedValue([
      makeRuleMatch({
        log_line: makeLogLine({ raw: '2024-01-01 ERROR disk full', content: 'ERROR disk full' }),
        extracted_state: { message: { String: 'disk full' } as any },
      }),
    ]);

    renderRuleCreator({ sourceTemplateId: 1 });
    await tick();

    await waitFor(() => {
      expect(screen.getByText('Test Against Source')).toBeInTheDocument();
    });

    // Select a source
    const sourceSelect = document.querySelector('.dry-run-controls select') as HTMLSelectElement;
    await fireEvent.change(sourceSelect, { target: { value: '1' } });

    // Click Run
    await fireEvent.click(screen.getByText('Run'));

    await waitFor(() => {
      expect(analysisApi.dryRun).toHaveBeenCalledWith(
        1,
        expect.objectContaining({
          source_id: 1,
          match_mode: 'Any',
          match_rules: [{ pattern: 'ERROR (?P<message>.+)' }],
        }),
      );
    });

    await waitFor(() => {
      expect(screen.getByText('1 match found')).toBeInTheDocument();
      expect(screen.getByText('2024-01-01 ERROR disk full')).toBeInTheDocument();
      expect(screen.getByText('message: disk full')).toBeInTheDocument();
    });
  });

  it('shows error message when dry run fails', async () => {
    vi.mocked(analysisApi.dryRun).mockRejectedValue(new Error('invalid regex'));

    renderRuleCreator({ sourceTemplateId: 1 });
    await tick();

    await waitFor(() => {
      expect(screen.getByText('Test Against Source')).toBeInTheDocument();
    });

    const sourceSelect = document.querySelector('.dry-run-controls select') as HTMLSelectElement;
    await fireEvent.change(sourceSelect, { target: { value: '1' } });

    await fireEvent.click(screen.getByText('Run'));

    await waitFor(() => {
      expect(screen.getByText('invalid regex')).toBeInTheDocument();
    });
  });
});
