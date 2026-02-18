import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import LogViewer from '../LogViewer.svelte';
import { makeSource, makeRuleMatch, makeLogLine, makeStateChange } from './fixtures';

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
    stateChanges: [],
    ...overrides,
  };
  return render(LogViewer, { props });
}

function makeErrorRuleMatches() {
  return [
    makeRuleMatch({
      source_id: 1,
      rule_id: 1,
      log_line: makeLogLine({
        raw: 'ERROR Failed to connect to database',
        timestamp: '2024-01-15T10:30:02.000',
      }),
      extracted_state: { error_type: { String: 'connection_failure' } },
    }),
    makeRuleMatch({
      source_id: 1,
      rule_id: 2,
      log_line: makeLogLine({
        raw: 'ERROR Timeout waiting for response',
        timestamp: '2024-01-15T10:30:05.000',
      }),
      extracted_state: { error_type: { String: 'timeout' } },
    }),
  ];
}

function makeMultiSourceStateChanges() {
  return [
    makeStateChange({
      timestamp: '2024-01-15T10:30:01.000',
      source_name: 'app.log',
      state_key: 'status',
      new_value: { String: 'starting' },
    }),
    makeStateChange({
      timestamp: '2024-01-15T10:30:02.000',
      source_name: 'app.log',
      state_key: 'error_type',
      new_value: { String: 'connection_failure' },
    }),
    makeStateChange({
      timestamp: '2024-01-15T10:30:03.000',
      source_name: 'db.log',
      state_key: 'connections',
      new_value: { Integer: 0 },
    }),
    makeStateChange({
      timestamp: '2024-01-15T10:30:04.000',
      source_name: 'app.log',
      state_key: 'status',
      new_value: { String: 'retrying' },
    }),
    makeStateChange({
      timestamp: '2024-01-15T10:30:05.000',
      source_name: 'db.log',
      state_key: 'connections',
      new_value: { Integer: 1 },
    }),
  ];
}

describe('LogViewer Accumulated State', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    fetchMock.mockResolvedValue({
      ok: true,
      text: () => Promise.resolve(LOG_CONTENT),
    });
  });

  it('shows state panel with both extracted and accumulated state for matched line', async () => {
    const ruleMatches = makeErrorRuleMatches();
    const stateChanges = makeMultiSourceStateChanges();

    renderLogViewer({ ruleMatches, stateChanges });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // Click the first ERROR line (index 2)
    const logLines = document.querySelectorAll('.log-line');
    const errorLine = Array.from(logLines).find((el) => el.textContent?.includes('ERROR Failed'));
    expect(errorLine).toBeTruthy();
    await fireEvent.click(errorLine!);
    await tick();

    await waitFor(() => {
      // Should show "Extracted State" header
      expect(screen.getByText('Extracted State')).toBeInTheDocument();
      // Should show accumulated state section with timestamp
      expect(screen.getByText('State at 2024-01-15T10:30:02.000')).toBeInTheDocument();
    });
  });

  it('replays state changes only up to clicked timestamp', async () => {
    const ruleMatches = makeErrorRuleMatches();
    const stateChanges = makeMultiSourceStateChanges();

    renderLogViewer({ ruleMatches, stateChanges });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // Click the first ERROR line (timestamp 2024-01-15T10:30:02.000)
    const logLines = document.querySelectorAll('.log-line');
    const errorLine = Array.from(logLines).find((el) => el.textContent?.includes('ERROR Failed'));
    await fireEvent.click(errorLine!);
    await tick();

    await waitFor(() => {
      const panel = document.querySelector('.state-panel')!;
      const text = panel.textContent!;
      // Should include state changes at or before T10:30:02
      expect(text).toContain('starting'); // T10:30:01 app.log status=starting
      expect(text).toContain('connection_failure'); // T10:30:02 app.log error_type=connection_failure
      // Should NOT include state changes after T10:30:02
      expect(text).not.toContain('retrying'); // T10:30:04 — after clicked line
    });
  });

  it('groups accumulated state by source name', async () => {
    const ruleMatches = [
      makeRuleMatch({
        source_id: 1,
        rule_id: 1,
        log_line: makeLogLine({
          raw: 'ERROR Timeout waiting for response',
          timestamp: '2024-01-15T10:30:05.000',
        }),
      }),
    ];
    const stateChanges = makeMultiSourceStateChanges();

    renderLogViewer({ ruleMatches, stateChanges });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // Click the second ERROR line (timestamp 10:30:05)
    const logLines = document.querySelectorAll('.log-line');
    const errorLine = Array.from(logLines).find((el) => el.textContent?.includes('Timeout'));
    await fireEvent.click(errorLine!);
    await tick();

    await waitFor(() => {
      const sourceHeaders = document.querySelectorAll('.state-source-name');
      const names = Array.from(sourceHeaders).map((el) => el.textContent);
      // Current source (app.log) should appear first
      expect(names[0]).toBe('app.log');
      expect(names).toContain('db.log');
    });
  });

  it('shows accumulated state for non-matched line with no extracted section', async () => {
    const ruleMatches = makeErrorRuleMatches();
    const stateChanges = makeMultiSourceStateChanges();

    renderLogViewer({ ruleMatches, stateChanges });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // Click line index 3 "WARN Retrying..." — not a matched line, but after ERROR at index 2
    const logLines = document.querySelectorAll('.log-line');
    const warnLine = Array.from(logLines).find((el) => el.textContent?.includes('WARN Retrying'));
    expect(warnLine).toBeTruthy();
    await fireEvent.click(warnLine!);
    await tick();

    await waitFor(() => {
      const panel = document.querySelector('.state-panel');
      expect(panel).toBeTruthy();
      // Should show accumulated state (inherited timestamp from previous matched line)
      const text = panel!.textContent!;
      expect(text).toContain('State at');
      // Should NOT show "Extracted State" header (non-matched line)
      expect(text).not.toContain('Extracted State');
    });
  });

  it('shows no state panel when no timestamp is resolvable', async () => {
    // No rule matches at all, so no timestamps to resolve from
    const stateChanges = makeMultiSourceStateChanges();

    renderLogViewer({ ruleMatches: [], stateChanges });
    await tick();
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    await tick();

    // Click line 0 (before any matches)
    const logLines = document.querySelectorAll('.log-line');
    await fireEvent.click(logLines[0]);
    await tick();

    await waitFor(() => {
      const panel = document.querySelector('.state-panel');
      expect(panel).toBeNull();
    });
  });
});
