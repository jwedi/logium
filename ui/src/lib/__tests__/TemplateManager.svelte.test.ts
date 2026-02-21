import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import TemplateManager from '../TemplateManager.svelte';

vi.mock('../api', () => ({
  templates: {
    list: vi.fn().mockResolvedValue([]),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
  },
  timestampTemplates: {
    list: vi.fn().mockResolvedValue([{ id: 1, name: 'ISO', format: '%Y-%m-%d' }]),
  },
}));

import { templates as templatesApi, timestampTemplates as tsTemplatesApi } from '../api';

function renderTemplateManager() {
  return render(TemplateManager, { props: { projectId: 1 } });
}

describe('TemplateManager regex validation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(templatesApi.list).mockResolvedValue([]);
    vi.mocked(tsTemplatesApi.list).mockResolvedValue([
      { id: 1, name: 'ISO', format: '%Y-%m-%d', extraction_regex: null, default_year: null },
    ]);
  });

  it('shows error message for invalid content regex', async () => {
    renderTemplateManager();
    await tick();

    const inputs = screen.getAllByPlaceholderText('Regex to extract content...');
    await fireEvent.input(inputs[0], { target: { value: '(unclosed' } });

    await waitFor(() => {
      const errors = document.querySelectorAll('.field-error');
      expect(errors.length).toBeGreaterThan(0);
      expect(errors[0].textContent).toMatch(/Invalid regular expression/);
    });
  });

  it('disables Create button when regex field has error', async () => {
    renderTemplateManager();
    await tick();

    // Fill in required fields
    const nameInput = screen.getByPlaceholderText('Template name...');
    await fireEvent.input(nameInput, { target: { value: 'My Template' } });

    // Type invalid regex
    const regexInput = screen.getByPlaceholderText('Regex to extract content...');
    await fireEvent.input(regexInput, { target: { value: '(unclosed' } });

    await waitFor(() => {
      const createBtn = screen.getByText('Create Template');
      expect(createBtn).toBeDisabled();
    });
  });

  it('enables Create button when regex is valid', async () => {
    renderTemplateManager();
    await tick();

    const nameInput = screen.getByPlaceholderText('Template name...');
    await fireEvent.input(nameInput, { target: { value: 'My Template' } });

    const regexInput = screen.getByPlaceholderText('Regex to extract content...');
    await fireEvent.input(regexInput, { target: { value: 'ERROR \\d+' } });

    await waitFor(() => {
      const createBtn = screen.getByText('Create Template');
      expect(createBtn).not.toBeDisabled();
    });
  });

  it('shows no error for empty regex', async () => {
    renderTemplateManager();
    await tick();

    await waitFor(() => {
      const errors = document.querySelectorAll('.field-error');
      expect(errors.length).toBe(0);
    });
  });

  it('clears error when regex is corrected', async () => {
    renderTemplateManager();
    await tick();

    const regexInput = screen.getByPlaceholderText('Regex to extract content...');
    await fireEvent.input(regexInput, { target: { value: '(unclosed' } });

    await waitFor(() => {
      expect(document.querySelectorAll('.field-error').length).toBeGreaterThan(0);
    });

    await fireEvent.input(regexInput, { target: { value: '(closed)' } });

    await waitFor(() => {
      expect(document.querySelectorAll('.field-error').length).toBe(0);
    });
  });
});

describe('TemplateManager regex presets', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(templatesApi.list).mockResolvedValue([]);
    vi.mocked(tsTemplatesApi.list).mockResolvedValue([
      { id: 1, name: 'ISO', format: '%Y-%m-%d', extraction_regex: null, default_year: null },
    ]);
  });

  it('shows 4 preset buttons in create form', async () => {
    renderTemplateManager();
    await tick();

    await waitFor(() => {
      const presetButtons = screen.getAllByText(/Presets/);
      expect(presetButtons.length).toBe(4);
    });
  });

  it('selecting a preset fills the input value', async () => {
    renderTemplateManager();
    await tick();

    const presetButtons = screen.getAllByText(/Presets/);
    // Click the first preset button (Content Regex)
    await fireEvent.click(presetButtons[0]);

    // Click the first preset item
    const presetItem = screen.getByText('Skip first token');
    await fireEvent.click(presetItem);

    await waitFor(() => {
      const input = screen.getByPlaceholderText('Regex to extract content...');
      expect(input).toHaveValue('^\\S+\\s+(.*)');
    });
  });
});
