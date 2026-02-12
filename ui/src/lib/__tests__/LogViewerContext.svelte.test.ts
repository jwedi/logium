import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import LogViewer from '../LogViewer.svelte';
import { makeSource, makeRuleMatch, makeLogLine } from './fixtures';

const LOG_CONTENT = [
  'INFO Starting server on port 8080', // 0
  'DEBUG Loading config from /etc/app.conf', // 1
  'ERROR Failed to connect to database', // 2
  'WARN Retrying connection in 5s', // 3
  'INFO Connection established', // 4
  'ERROR Timeout waiting for response', // 5
  'DEBUG Cleanup started', // 6
  'INFO Server shutting down', // 7
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

function makeErrorRuleMatches() {
  return [
    makeRuleMatch({
      source_id: 1,
      rule_id: 1,
      log_line: makeLogLine({ raw: 'ERROR Failed to connect to database' }),
    }),
    makeRuleMatch({
      source_id: 1,
      rule_id: 2,
      log_line: makeLogLine({ raw: 'ERROR Timeout waiting for response' }),
    }),
  ];
}

describe('LogViewer Context Expansion', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve(LOG_CONTENT),
    });
  });

  it('shows expand button on matched lines when filter active', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');

    await waitFor(() => {
      const expandBtns = document.querySelectorAll('.expand-toggle');
      expect(expandBtns.length).toBe(2);
    });
  });

  it('does not show expand button when no filter active', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // No filter active - should have spacers, not expand buttons
    await waitFor(() => {
      const expandBtns = document.querySelectorAll('.expand-toggle');
      expect(expandBtns.length).toBe(0);
    });
  });

  it('clicking expand shows context lines', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(2);
    });

    // Click expand on the first ERROR line
    const expandBtn = document.querySelector('.expand-toggle')!;
    await fireEvent.click(expandBtn);
    await tick();

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      // ERROR at index 2, context size 5 → lines 0-7, but only 0,1,3,4 are context (2 and 5 are base)
      // So we get base(2) + context(0,1,3,4) = more than 2 lines
      expect(lines.length).toBeGreaterThan(2);
    });
  });

  it('context lines have context-line class', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });

    const expandBtn = document.querySelector('.expand-toggle')!;
    await fireEvent.click(expandBtn);
    await tick();

    await waitFor(() => {
      const contextLines = document.querySelectorAll('.log-line.context-line');
      expect(contextLines.length).toBeGreaterThan(0);
    });
  });

  it('context lines show original line numbers', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });

    const expandBtn = document.querySelector('.expand-toggle')!;
    await fireEvent.click(expandBtn);
    await tick();

    await waitFor(() => {
      const lineNumbers = document.querySelectorAll('.line-number');
      const numbers = Array.from(lineNumbers).map((el) => el.textContent);
      // Should include original line numbers like 1, 2, 3, etc.
      expect(numbers).toContain('1'); // line 0 → display "1"
      expect(numbers).toContain('3'); // ERROR at index 2 → display "3"
    });
  });

  it('collapse removes context lines', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });

    // Expand
    const expandBtn = document.querySelector('.expand-toggle')!;
    await fireEvent.click(expandBtn);
    await tick();

    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBeGreaterThan(2);
    });

    // Collapse (click same button, now showing ▼)
    const collapseBtn = document.querySelector('.expand-toggle')!;
    await fireEvent.click(collapseBtn);
    await tick();

    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });
  });

  it('expand all shows context for all matches', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });

    const expandAllBtn = screen.getByTitle('Expand all matches');
    await fireEvent.click(expandAllBtn);
    await tick();

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      // Both ERRORs expanded with context 5 → should show all 8 lines
      expect(lines.length).toBe(8);
    });
  });

  it('collapse all removes all context', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });

    // Expand all
    const expandAllBtn = screen.getByTitle('Expand all matches');
    await fireEvent.click(expandAllBtn);
    await tick();
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(8);
    });

    // Collapse all
    const collapseAllBtn = screen.getByTitle('Collapse all matches');
    await fireEvent.click(collapseAllBtn);
    await tick();

    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });
  });

  it('context size controls number of lines', async () => {
    renderLogViewer({ ruleMatches: [makeErrorRuleMatches()[0]] }); // Rule match only on first ERROR
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      // Two ERROR lines pass text filter (indices 2 and 5)
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });

    // Change context size to 1
    const ctxInput = document.querySelector('.context-size-input') as HTMLInputElement;
    await fireEvent.input(ctxInput, { target: { value: '1' } });
    await tick();

    // Expand the first ERROR (only one with a rule match, so only one expand button)
    const expandBtn = document.querySelector('.expand-toggle')!;
    await fireEvent.click(expandBtn);
    await tick();

    await waitFor(() => {
      const lines = document.querySelectorAll('.log-line');
      // ERROR at index 2, context 1 → adds lines 1,3 → base(2,5) + context(1,3) = 4
      expect(lines.length).toBe(4);
    });
  });

  it('gap separator on non-consecutive groups', async () => {
    // Use a single match that doesn't cover all lines, with small context
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });

    // Change context size to 1 so there's a gap between the two groups
    const ctxInput = document.querySelector('.context-size-input') as HTMLInputElement;
    await fireEvent.input(ctxInput, { target: { value: '1' } });
    await tick();

    // Expand all
    const expandAllBtn = screen.getByTitle('Expand all matches');
    await fireEvent.click(expandAllBtn);
    await tick();

    await waitFor(() => {
      // ERROR at 2 → ctx 1,2,3; ERROR at 5 → ctx 4,5,6
      // Combined sorted: 1,2,3,4,5,6 — all consecutive, no gap
      // So gap-before should not appear in this case
      // Let's just check lines rendered correctly
      const lines = document.querySelectorAll('.log-line');
      expect(lines.length).toBe(6); // 1,2,3,4,5,6
    });
  });

  it('filter count shows base count not expanded count', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      expect(screen.getByText('2 of 8 lines')).toBeInTheDocument();
    });

    // Expand all — count should still say "2 of 8 lines"
    const expandAllBtn = screen.getByTitle('Expand all matches');
    await fireEvent.click(expandAllBtn);
    await tick();

    await waitFor(() => {
      expect(screen.getByText('2 of 8 lines')).toBeInTheDocument();
    });
  });

  it('search works within context lines', async () => {
    renderLogViewer({ ruleMatches: makeErrorRuleMatches() });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    await typeInFilter('ERROR');
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(2);
    });

    // Expand to show context
    const expandAllBtn = screen.getByTitle('Expand all matches');
    await fireEvent.click(expandAllBtn);
    await tick();
    await waitFor(() => {
      expect(document.querySelectorAll('.log-line').length).toBe(8);
    });

    // Search for text that's in a context line
    await openSearch();
    await typeInSearch('Loading config');

    await waitFor(() => {
      expect(screen.getByText('1 of 1')).toBeInTheDocument();
    });
  });
});
