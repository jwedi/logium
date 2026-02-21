import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import TimestampTemplateManager from '../TimestampTemplateManager.svelte';

vi.mock('../api', () => ({
  timestampTemplates: {
    list: vi.fn().mockResolvedValue([]),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
  },
  sources: {
    list: vi.fn().mockResolvedValue([]),
    rawLines: vi.fn().mockResolvedValue({ lines: [], total_lines: 0 }),
  },
}));

import { timestampTemplates as tsTemplatesApi } from '../api';

function renderTimestampTemplateManager() {
  return render(TimestampTemplateManager, { props: { projectId: 1 } });
}

describe('TimestampTemplateManager regex validation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tsTemplatesApi.list).mockResolvedValue([]);
  });

  it('shows error message for invalid extraction regex', async () => {
    renderTimestampTemplateManager();
    await tick();

    const input = screen.getByPlaceholderText('Regex to extract timestamp from line...');
    await fireEvent.input(input, { target: { value: '[abc' } });

    await waitFor(() => {
      const errors = document.querySelectorAll('.field-error');
      expect(errors.length).toBeGreaterThan(0);
      expect(errors[0].textContent).toMatch(/Invalid regular expression/);
    });
  });

  it('disables Create button when extraction regex is invalid', async () => {
    renderTimestampTemplateManager();
    await tick();

    // Fill required fields
    const nameInput = screen.getByPlaceholderText('e.g. ISO 8601, Syslog...');
    await fireEvent.input(nameInput, { target: { value: 'My TS Template' } });

    const formatInput = screen.getByPlaceholderText('e.g. %Y-%m-%dT%H:%M:%S');
    await fireEvent.input(formatInput, { target: { value: '%Y-%m-%d' } });

    // Type invalid regex
    const regexInput = screen.getByPlaceholderText('Regex to extract timestamp from line...');
    await fireEvent.input(regexInput, { target: { value: '[abc' } });

    await waitFor(() => {
      const createBtn = screen.getByText('Create');
      expect(createBtn).toBeDisabled();
    });
  });

  it('enables Create button when extraction regex is valid', async () => {
    renderTimestampTemplateManager();
    await tick();

    const nameInput = screen.getByPlaceholderText('e.g. ISO 8601, Syslog...');
    await fireEvent.input(nameInput, { target: { value: 'My TS Template' } });

    const formatInput = screen.getByPlaceholderText('e.g. %Y-%m-%dT%H:%M:%S');
    await fireEvent.input(formatInput, { target: { value: '%Y-%m-%d' } });

    const regexInput = screen.getByPlaceholderText('Regex to extract timestamp from line...');
    await fireEvent.input(regexInput, { target: { value: '\\d{4}-\\d{2}-\\d{2}' } });

    await waitFor(() => {
      const createBtn = screen.getByText('Create');
      expect(createBtn).not.toBeDisabled();
    });
  });

  it('shows no error for empty extraction regex', async () => {
    renderTimestampTemplateManager();
    await tick();

    await waitFor(() => {
      const errors = document.querySelectorAll('.field-error');
      expect(errors.length).toBe(0);
    });
  });
});

describe('TimestampTemplateManager regex test inputs', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tsTemplatesApi.list).mockResolvedValue([]);
  });

  it('shows 1 Test button in create form', async () => {
    renderTimestampTemplateManager();
    await tick();

    await waitFor(() => {
      const testButtons = screen.getAllByText('Test');
      expect(testButtons.length).toBe(1);
    });
  });
});

describe('TimestampTemplateManager regex presets', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tsTemplatesApi.list).mockResolvedValue([]);
  });

  it('shows 1 preset button in create form', async () => {
    renderTimestampTemplateManager();
    await tick();

    await waitFor(() => {
      const presetButtons = screen.getAllByText(/Presets/);
      expect(presetButtons.length).toBe(1);
    });
  });
});
