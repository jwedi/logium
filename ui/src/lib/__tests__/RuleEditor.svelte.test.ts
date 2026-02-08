import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import RuleEditor from '../RuleEditor.svelte';
import { makeRule, makeRuleWithExtractions } from './fixtures';
import * as invalidation from '../analysisInvalidation.svelte';

vi.mock('../api', () => ({
  rules: {
    update: vi.fn(),
    list: vi.fn().mockResolvedValue([]),
    create: vi.fn(),
    get: vi.fn(),
    delete: vi.fn(),
  },
  rulesets: {
    list: vi.fn().mockResolvedValue([]),
    update: vi.fn(),
  },
  analysis: {
    suggestRule: vi.fn(),
    run: vi.fn(),
    runStreaming: vi.fn(),
    detectTemplate: vi.fn(),
  },
}));

import { rules as rulesApi } from '../api';

function renderEditor(
  ruleOverrides: Record<string, any> = {},
  propOverrides: Record<string, any> = {},
) {
  const rule = makeRule(ruleOverrides);
  const props = {
    rule,
    projectId: 1,
    onSave: vi.fn(),
    onCancel: vi.fn(),
    ...propOverrides,
  };
  return { ...render(RuleEditor, { props }), props, rule };
}

function getAllTextareas(): HTMLTextAreaElement[] {
  return Array.from(document.querySelectorAll('textarea'));
}

function getNameInput(): HTMLInputElement {
  return screen.getByDisplayValue('Error Rule') as HTMLInputElement;
}

describe('RuleEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(rulesApi.update).mockResolvedValue(makeRule());
  });

  it('pre-populates fields from rule prop', () => {
    renderEditor();
    expect(screen.getByDisplayValue('Error Rule')).toBeInTheDocument();
    // Match pattern textarea
    const textareas = getAllTextareas();
    const patternTextarea = textareas.find((t) => t.value === 'ERROR');
    expect(patternTextarea).toBeTruthy();
  });

  it('pre-populates extraction rules', () => {
    renderEditor({
      extraction_rules: [
        {
          id: 1,
          extraction_type: 'Parsed',
          state_key: 'message',
          pattern: 'ERROR (.+)',
          static_value: null,
          mode: 'Replace',
        },
      ],
    });
    expect(screen.getByDisplayValue('message')).toBeInTheDocument();
  });

  it('saves edited name and match_mode', async () => {
    const { props } = renderEditor();

    const nameInput = getNameInput();
    await fireEvent.input(nameInput, { target: { value: 'Updated Rule' } });

    const matchModeSelect = screen.getAllByRole('combobox')[0];
    await fireEvent.change(matchModeSelect, { target: { value: 'All' } });

    await fireEvent.click(screen.getByText('Save'));

    await waitFor(() => {
      expect(rulesApi.update).toHaveBeenCalledTimes(1);
    });

    const call = vi.mocked(rulesApi.update).mock.calls[0];
    expect(call[0]).toBe(1); // projectId
    expect(call[1]).toBe(1); // rule.id
    expect(call[2]).toMatchObject({
      name: 'Updated Rule',
      match_mode: 'All',
    });
  });

  it('calls onSave after successful save', async () => {
    const { props } = renderEditor();
    await fireEvent.click(screen.getByText('Save'));

    await waitFor(() => {
      expect(props.onSave).toHaveBeenCalled();
    });
  });

  it('calls onCancel when Cancel clicked', async () => {
    const { props } = renderEditor();
    await fireEvent.click(screen.getByText('Cancel'));
    expect(props.onCancel).toHaveBeenCalled();
  });

  it('calls invalidateAnalysis on save', async () => {
    const spy = vi.spyOn(invalidation, 'invalidateAnalysis');
    renderEditor();

    await fireEvent.click(screen.getByText('Save'));

    await waitFor(() => {
      expect(spy).toHaveBeenCalled();
    });

    spy.mockRestore();
  });

  it('disables save when name is empty', async () => {
    renderEditor();
    const nameInput = getNameInput();
    await fireEvent.input(nameInput, { target: { value: '' } });
    await tick();

    expect(screen.getByText('Save')).toBeDisabled();
  });

  it('disables save when no patterns have content', async () => {
    renderEditor({ match_rules: [{ id: 1, pattern: '' }] });
    await tick();

    expect(screen.getByText('Save')).toBeDisabled();
  });

  it('adds a new match pattern row', async () => {
    renderEditor();
    const addButtons = screen.getAllByText('+ Add');
    await fireEvent.click(addButtons[0]); // First "+ Add" is match patterns

    const textareas = getAllTextareas();
    // Should have 2 pattern textareas + 1 test textarea = 3
    expect(textareas.length).toBe(3);
  });

  it('removes a match pattern row', async () => {
    renderEditor({
      match_rules: [
        { id: 1, pattern: 'ERROR' },
        { id: 2, pattern: 'WARN' },
      ],
    });

    const removeButtons = screen.getAllByText('x');
    await fireEvent.click(removeButtons[0]); // Remove first pattern
    await tick();

    // Only WARN should remain
    const textareas = getAllTextareas();
    const patternValues = textareas
      .filter((t) => t.placeholder === 'regex pattern...')
      .map((t) => t.value);
    expect(patternValues).toEqual(['WARN']);
  });

  it('adds an extraction rule row', async () => {
    renderEditor();
    const addButtons = screen.getAllByText('+ Add');
    await fireEvent.click(addButtons[1]); // Second "+ Add" is extraction rules
    await tick();

    // Should see a state_key input
    const inputs = screen.getAllByPlaceholderText('state_key');
    expect(inputs.length).toBe(1);
  });

  it('removes an extraction rule row', async () => {
    renderEditor({
      extraction_rules: [
        {
          id: 1,
          extraction_type: 'Parsed',
          state_key: 'msg',
          pattern: '.+',
          static_value: null,
          mode: 'Replace',
        },
        {
          id: 2,
          extraction_type: 'Static',
          state_key: 'level',
          pattern: null,
          static_value: 'err',
          mode: 'Replace',
        },
      ],
    });

    // The "x" buttons: first is match pattern remove, next two are extraction removes
    const removeButtons = screen.getAllByText('x');
    // Click the second "x" which is the first extraction rule remove
    await fireEvent.click(removeButtons[1]);
    await tick();

    // Only one extraction state_key input should remain
    expect(screen.queryByDisplayValue('msg')).toBeNull();
    expect(screen.getByDisplayValue('level')).toBeInTheDocument();
  });

  it('shows Parsed pattern input when type is Parsed', () => {
    renderEditor({
      extraction_rules: [
        {
          id: 1,
          extraction_type: 'Parsed',
          state_key: 'msg',
          pattern: 'ERROR (.+)',
          static_value: null,
          mode: 'Replace',
        },
      ],
    });
    expect(screen.getByPlaceholderText('regex with groups...')).toBeInTheDocument();
  });

  it('shows static value input when type is Static', () => {
    renderEditor({
      extraction_rules: [
        {
          id: 1,
          extraction_type: 'Static',
          state_key: 'level',
          pattern: null,
          static_value: 'error',
          mode: 'Replace',
        },
      ],
    });
    expect(screen.getByPlaceholderText('static value...')).toBeInTheDocument();
    expect(screen.getByDisplayValue('error')).toBeInTheDocument();
  });

  it('hides both pattern and value inputs for Clear type', () => {
    renderEditor({
      extraction_rules: [
        {
          id: 1,
          extraction_type: 'Clear',
          state_key: 'old_key',
          pattern: null,
          static_value: null,
          mode: 'Replace',
        },
      ],
    });
    expect(screen.queryByPlaceholderText('regex with groups...')).toBeNull();
    expect(screen.queryByPlaceholderText('static value...')).toBeNull();
  });

  // Dry-run tests
  it('shows match indicator for matching pattern', async () => {
    renderEditor({ match_rules: [{ id: 1, pattern: 'ERROR' }] });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'ERROR something failed' } });
    await tick();

    expect(screen.getByText('Match')).toBeInTheDocument();
  });

  it('shows no-match indicator for non-matching pattern', async () => {
    renderEditor({ match_rules: [{ id: 1, pattern: 'ERROR' }] });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'INFO all good' } });
    await tick();

    expect(screen.getByText('No match')).toBeInTheDocument();
  });

  it('shows error indicator for invalid regex', async () => {
    renderEditor({ match_rules: [{ id: 1, pattern: '(unclosed' }] });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'test' } });
    await tick();

    expect(screen.getByText('Error')).toBeInTheDocument();
  });

  it('shows overall verdict for Any mode with matches', async () => {
    renderEditor({
      match_mode: 'Any',
      match_rules: [
        { id: 1, pattern: 'ERROR' },
        { id: 2, pattern: 'WARN' },
      ],
    });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'ERROR something' } });
    await tick();

    expect(screen.getByText(/1 of 2 patterns matched \(Any mode\)/)).toBeInTheDocument();
  });

  it('shows overall verdict for All mode requiring all matches', async () => {
    renderEditor({
      match_mode: 'All',
      match_rules: [
        { id: 1, pattern: 'ERROR' },
        { id: 2, pattern: 'failed' },
      ],
    });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'ERROR something failed' } });
    await tick();

    expect(screen.getByText(/All 2 patterns matched/)).toBeInTheDocument();
  });

  it('shows extraction preview for Parsed type', async () => {
    renderEditor({
      match_rules: [{ id: 1, pattern: 'ERROR (?P<message>.+)' }],
      extraction_rules: [
        {
          id: 1,
          extraction_type: 'Parsed',
          state_key: 'message',
          pattern: 'ERROR (?P<message>.+)',
          static_value: null,
          mode: 'Replace',
        },
      ],
    });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'ERROR something failed' } });
    await tick();

    expect(screen.getByText('something failed')).toBeInTheDocument();
    expect(screen.getByText('(Parsed)')).toBeInTheDocument();
  });

  it('shows extraction preview for Static type', async () => {
    renderEditor({
      extraction_rules: [
        {
          id: 1,
          extraction_type: 'Static',
          state_key: 'level',
          pattern: null,
          static_value: 'error',
          mode: 'Replace',
        },
      ],
    });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'anything' } });
    await tick();

    expect(screen.getByText('error')).toBeInTheDocument();
    expect(screen.getByText('(Static)')).toBeInTheDocument();
  });

  it('shows extraction preview for Clear type', async () => {
    renderEditor({
      extraction_rules: [
        {
          id: 1,
          extraction_type: 'Clear',
          state_key: 'old_key',
          pattern: null,
          static_value: null,
          mode: 'Replace',
        },
      ],
    });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'anything' } });
    await tick();

    expect(screen.getByText('(cleared)')).toBeInTheDocument();
    expect(screen.getByText('(Clear)')).toBeInTheDocument();
  });

  it('handles (?P<name>) syntax in dry-run', async () => {
    renderEditor({
      match_rules: [{ id: 1, pattern: '(?P<level>\\w+): (?P<msg>.+)' }],
    });

    const testInput = screen.getByPlaceholderText('Paste a log line here to test...');
    await fireEvent.input(testInput, { target: { value: 'ERROR: bad stuff' } });
    await tick();

    expect(screen.getByText('Match')).toBeInTheDocument();
  });

  it('sends id: 0 for sub-rules in save payload', async () => {
    renderEditor({
      match_rules: [{ id: 99, pattern: 'ERROR' }],
      extraction_rules: [
        {
          id: 88,
          extraction_type: 'Static',
          state_key: 'level',
          pattern: null,
          static_value: 'err',
          mode: 'Replace',
        },
      ],
    });

    await fireEvent.click(screen.getByText('Save'));

    await waitFor(() => {
      expect(rulesApi.update).toHaveBeenCalledTimes(1);
    });

    const payload = vi.mocked(rulesApi.update).mock.calls[0][2];
    expect(payload.match_rules![0].id).toBe(0);
    expect(payload.extraction_rules![0].id).toBe(0);
  });
});
