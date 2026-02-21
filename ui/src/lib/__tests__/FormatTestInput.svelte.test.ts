import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import FormatTestInput from '../FormatTestInput.svelte';

vi.mock('../api', () => ({
  timestampTemplates: {
    testFormat: vi.fn().mockResolvedValue({ parsed_timestamp: '2024-01-15 10:30:45' }),
  },
  sources: {
    list: vi.fn().mockResolvedValue([]),
    rawLines: vi.fn().mockResolvedValue({ lines: [], total_lines: 0 }),
  },
}));

import { timestampTemplates as tsTemplatesApi } from '../api';

function renderFormatTest(
  props: Partial<{
    format: string;
    extractionRegex: string;
    defaultYear: string;
    projectId: number;
  }> = {},
) {
  return render(FormatTestInput, {
    props: {
      format: '%Y-%m-%dT%H:%M:%S',
      extractionRegex: '',
      defaultYear: '',
      projectId: 1,
      ...props,
    },
  });
}

describe('FormatTestInput', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tsTemplatesApi.testFormat).mockResolvedValue({
      parsed_timestamp: '2024-01-15 10:30:45',
    });
  });

  it('renders Test toggle button', () => {
    renderFormatTest();
    expect(screen.getByText('Test')).toBeTruthy();
  });

  it('shows input and Browse file button when expanded', async () => {
    renderFormatTest();
    await fireEvent.click(screen.getByText('Test'));
    await tick();

    expect(screen.getByPlaceholderText('Paste a sample line to test...')).toBeTruthy();
    expect(screen.getByText('Browse file')).toBeTruthy();
  });

  it('calls API and shows parsed timestamp on success', async () => {
    vi.useFakeTimers();
    renderFormatTest();
    await fireEvent.click(screen.getByText('Test'));
    await tick();

    const input = screen.getByPlaceholderText('Paste a sample line to test...');
    await fireEvent.input(input, { target: { value: '2024-01-15T10:30:45 log line' } });

    // Advance past debounce
    await vi.advanceTimersByTimeAsync(350);

    await waitFor(() => {
      expect(tsTemplatesApi.testFormat).toHaveBeenCalledWith(1, {
        format: '%Y-%m-%dT%H:%M:%S',
        sample: '2024-01-15T10:30:45 log line',
        extraction_regex: null,
        default_year: null,
      });
      const verdict = document.querySelector('.verdict.ok');
      expect(verdict).toBeTruthy();
      expect(verdict!.textContent).toContain('2024-01-15 10:30:45');
    });

    vi.useRealTimers();
  });

  it('shows error message on API failure', async () => {
    vi.useFakeTimers();
    vi.mocked(tsTemplatesApi.testFormat).mockRejectedValue(
      new Error('API 400: {"error":"failed to parse"}'),
    );
    renderFormatTest();
    await fireEvent.click(screen.getByText('Test'));
    await tick();

    const input = screen.getByPlaceholderText('Paste a sample line to test...');
    await fireEvent.input(input, { target: { value: 'bad input' } });

    await vi.advanceTimersByTimeAsync(350);

    await waitFor(() => {
      const verdict = document.querySelector('.verdict.fail');
      expect(verdict).toBeTruthy();
      expect(verdict!.textContent).toContain('failed to parse');
    });

    vi.useRealTimers();
  });
});
