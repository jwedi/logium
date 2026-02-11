import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import LogViewer from '../LogViewer.svelte';
import { makeSource, makeRuleMatch, makeLogLine } from './fixtures';

const LOG_CONTENT = [
  'INFO Starting server on port 8080',
  'DEBUG Loading config from /etc/app.conf',
  'ERROR Failed to connect to database',
  'WARN Retrying connection in 5s',
  'ERROR Timeout waiting for response',
  'INFO Server ready',
].join('\n');

vi.mock('../api', () => ({
  rules: {
    list: vi.fn().mockResolvedValue([]),
  },
}));

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

function renderLogViewer(overrides: Record<string, any> = {}) {
  const props = {
    source: makeSource(),
    projectId: 1,
    ruleMatches: [],
    patternMatches: [],
    ...overrides,
  };
  return render(LogViewer, { props });
}

async function typeInFilter(text: string) {
  const input = screen.getByPlaceholderText('Filter lines...');
  await fireEvent.input(input, { target: { value: text } });
  await tick();
}

async function openSearch() {
  await fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
  await tick();
}

async function typeInSearch(text: string) {
  const input = screen.getByPlaceholderText('Search...');
  await fireEvent.input(input, { target: { value: text } });
  await tick();
}

describe('LogViewer Filter', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve(LOG_CONTENT),
    });
  });

  it('filter bar is always visible', async () => {
    renderLogViewer();
    await tick();

    expect(screen.getByPlaceholderText('Filter lines...')).toBeInTheDocument();
  });

  it('shows all lines when filter is empty', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(6);
    });
  });

  it('hides non-matching lines', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(2);
    });
  });

  it('filter is case-insensitive', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('error');

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(2);
    });
  });

  it('displays original line numbers', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');

    await waitFor(() => {
      const lineNumbers = document.querySelectorAll('.line-number');
      // ERROR lines are at original indices 2 and 4 (1-based: 3 and 5)
      expect(lineNumbers.length).toBe(2);
      expect(lineNumbers[0].textContent).toBe('3');
      expect(lineNumbers[1].textContent).toBe('5');
    });
  });

  it('shows "N of M lines" count', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');

    await waitFor(() => {
      expect(screen.getByText('2 of 6 lines')).toBeInTheDocument();
    });
  });

  it('shows nothing when filter empty', async () => {
    renderLogViewer();
    await tick();

    const filterCount = document.querySelector('.filter-count');
    expect(filterCount?.textContent?.trim()).toBe('');
  });

  it('regex mode toggle', async () => {
    renderLogViewer();
    await tick();

    const regexBtn = screen.getByTitle('Toggle filter regex');
    expect(regexBtn.classList.contains('active')).toBe(false);

    await fireEvent.click(regexBtn);
    await tick();

    expect(regexBtn.classList.contains('active')).toBe(true);
  });

  it('filters with regex', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // Enable regex mode
    await fireEvent.click(screen.getByTitle('Toggle filter regex'));
    await tick();

    await typeInFilter('(ERROR|WARN)');

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(3);
    });
  });

  it('invalid regex handled gracefully', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // Enable regex mode
    await fireEvent.click(screen.getByTitle('Toggle filter regex'));
    await tick();

    await typeInFilter('[invalid');

    await waitFor(() => {
      // Invalid regex → filterRegex is null → filteredIndices falls through to identity array
      // filterQuery is non-empty so counter shows, but all lines pass through
      expect(screen.getByText('6 of 6 lines')).toBeInTheDocument();
    });

    // Component should still be functional
    expect(screen.getByPlaceholderText('Filter lines...')).toBeInTheDocument();
  });

  it('highlights filter matches', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');

    await waitFor(() => {
      const marks = document.querySelectorAll('mark.filter-highlight');
      expect(marks.length).toBeGreaterThanOrEqual(1);
    });
  });

  it('clear button clears filter', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(2);
    });

    const clearBtn = screen.getByTitle('Clear filter');
    await fireEvent.click(clearBtn);
    await tick();

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(6);
    });
  });

  it('search within filtered results', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // First filter to ERROR lines only
    await typeInFilter('ERROR');
    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(2);
    });

    // Then search for "Timeout" within filtered lines
    await openSearch();
    await typeInSearch('Timeout');

    await waitFor(() => {
      expect(screen.getByText('1 of 1')).toBeInTheDocument();
    });
  });

  it('rule match highlighting on filtered lines', async () => {
    const ruleMatch = makeRuleMatch({
      source_id: 1,
      rule_id: 1,
      log_line: makeLogLine({ raw: 'ERROR Failed to connect to database' }),
    });

    renderLogViewer({ ruleMatches: [ruleMatch] });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // Filter to ERROR lines
    await typeInFilter('ERROR');

    await waitFor(() => {
      const highlighted = document.querySelectorAll('.log-line.highlighted');
      expect(highlighted.length).toBe(1);
    });
  });
});
