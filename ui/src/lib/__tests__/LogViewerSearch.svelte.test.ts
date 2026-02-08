import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import LogViewer from '../LogViewer.svelte';
import { makeSource } from './fixtures';

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

// Mock fetch for file content
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

async function openSearch() {
  await fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
  await tick();
}

async function typeInSearch(text: string) {
  const input = screen.getByPlaceholderText('Search...');
  await fireEvent.input(input, { target: { value: text } });
  await tick();
}

describe('LogViewer Search', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve(LOG_CONTENT),
    });
  });

  // --- Open / Close ---

  it('opens search bar on Ctrl+F', async () => {
    renderLogViewer();
    await tick();

    expect(screen.queryByPlaceholderText('Search...')).not.toBeInTheDocument();

    await openSearch();

    expect(screen.getByPlaceholderText('Search...')).toBeInTheDocument();
  });

  it('opens search bar on Cmd+F (macOS)', async () => {
    renderLogViewer();
    await tick();

    await fireEvent.keyDown(window, { key: 'f', metaKey: true });
    await tick();

    expect(screen.getByPlaceholderText('Search...')).toBeInTheDocument();
  });

  it('closes search bar on Escape', async () => {
    renderLogViewer();
    await tick();
    await openSearch();

    expect(screen.getByPlaceholderText('Search...')).toBeInTheDocument();

    await fireEvent.keyDown(window, { key: 'Escape' });
    await tick();

    expect(screen.queryByPlaceholderText('Search...')).not.toBeInTheDocument();
  });

  it('closes search via close button', async () => {
    renderLogViewer();
    await tick();
    await openSearch();

    await fireEvent.click(screen.getByTitle('Close search'));
    await tick();

    expect(screen.queryByPlaceholderText('Search...')).not.toBeInTheDocument();
  });

  it('clears query when search is closed', async () => {
    renderLogViewer();
    await tick();
    await openSearch();
    await typeInSearch('ERROR');

    // Close and reopen
    await fireEvent.keyDown(window, { key: 'Escape' });
    await tick();
    await openSearch();

    const input = screen.getByPlaceholderText('Search...') as HTMLInputElement;
    expect(input.value).toBe('');
  });

  // --- Match counting ---

  it('shows match count for plain text search', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('ERROR');

    await waitFor(() => {
      expect(screen.getByText('1 of 2')).toBeInTheDocument();
    });
  });

  it('shows "No matches" when query has no results', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('FATAL');

    await waitFor(() => {
      expect(screen.getByText('No matches')).toBeInTheDocument();
    });
  });

  it('shows nothing in match count when query is empty', async () => {
    renderLogViewer();
    await tick();
    await openSearch();

    const matchCount = document.querySelector('.match-count');
    expect(matchCount?.textContent?.trim()).toBe('');
  });

  // --- Case insensitivity ---

  it('search is case-insensitive', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('error');

    await waitFor(() => {
      expect(screen.getByText('1 of 2')).toBeInTheDocument();
    });
  });

  // --- Highlighting ---

  it('highlights matching text with mark elements', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('ERROR');

    await waitFor(() => {
      const marks = document.querySelectorAll('mark.search-highlight');
      expect(marks.length).toBeGreaterThanOrEqual(1);
      expect(marks[0].textContent).toBe('ERROR');
    });
  });

  it('applies current-search-match class to the active match line', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('ERROR');

    await waitFor(() => {
      const currentLine = document.querySelector('.log-line.current-search-match');
      expect(currentLine).not.toBeNull();
      expect(currentLine!.textContent).toContain('ERROR');
    });
  });

  // --- Navigation ---

  it('navigates to next match on Enter', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('ERROR');

    await waitFor(() => {
      expect(screen.getByText('1 of 2')).toBeInTheDocument();
    });

    const input = screen.getByPlaceholderText('Search...');
    await fireEvent.keyDown(input, { key: 'Enter' });
    await tick();

    expect(screen.getByText('2 of 2')).toBeInTheDocument();
  });

  it('navigates to previous match on Shift+Enter', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('ERROR');

    await waitFor(() => {
      expect(screen.getByText('1 of 2')).toBeInTheDocument();
    });

    const input = screen.getByPlaceholderText('Search...');
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    await tick();

    // Wraps around: from index 0 to last match
    expect(screen.getByText('2 of 2')).toBeInTheDocument();
  });

  it('navigates via arrow buttons', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('ERROR');

    await waitFor(() => {
      expect(screen.getByText('1 of 2')).toBeInTheDocument();
    });

    await fireEvent.click(screen.getByTitle('Next match'));
    await tick();
    expect(screen.getByText('2 of 2')).toBeInTheDocument();

    await fireEvent.click(screen.getByTitle('Previous match'));
    await tick();
    expect(screen.getByText('1 of 2')).toBeInTheDocument();
  });

  it('wraps around when navigating past last match', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();
    await typeInSearch('ERROR');

    await waitFor(() => {
      expect(screen.getByText('1 of 2')).toBeInTheDocument();
    });

    await fireEvent.click(screen.getByTitle('Next match'));
    await tick();
    expect(screen.getByText('2 of 2')).toBeInTheDocument();

    await fireEvent.click(screen.getByTitle('Next match'));
    await tick();
    expect(screen.getByText('1 of 2')).toBeInTheDocument();
  });

  // --- Regex mode ---

  it('toggles regex mode', async () => {
    renderLogViewer();
    await tick();
    await openSearch();

    const regexBtn = screen.getByTitle('Toggle regex');
    expect(regexBtn.classList.contains('active')).toBe(false);

    await fireEvent.click(regexBtn);
    await tick();

    expect(regexBtn.classList.contains('active')).toBe(true);
  });

  it('matches regex patterns in regex mode', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();

    // Enable regex mode
    await fireEvent.click(screen.getByTitle('Toggle regex'));
    await tick();

    // Search for lines starting with ERROR or WARN
    await typeInSearch('(ERROR|WARN)');

    await waitFor(() => {
      expect(screen.getByText('1 of 3')).toBeInTheDocument();
    });
  });

  it('handles invalid regex gracefully', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await openSearch();

    // Enable regex mode
    await fireEvent.click(screen.getByTitle('Toggle regex'));
    await tick();

    // Type invalid regex — should not crash, should show no matches
    await typeInSearch('[invalid');

    await waitFor(() => {
      expect(screen.getByText('No matches')).toBeInTheDocument();
    });

    // Component should still be functional
    expect(screen.getByPlaceholderText('Search...')).toBeInTheDocument();
  });

  // --- Existing features still work ---

  it('does not interfere with text selection popup', async () => {
    renderLogViewer();
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // The selection popup element should exist in the DOM
    const popup = document.getElementById('selection-popup');
    expect(popup).not.toBeNull();
  });
});
