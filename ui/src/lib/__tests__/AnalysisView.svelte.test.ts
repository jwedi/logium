import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import AnalysisView from '../AnalysisView.svelte';
import { invalidateAnalysis } from '../analysisInvalidation.svelte';
import {
  makeAnalysisResult,
  makeSource,
  makeRule,
  makePattern,
  makeRuleMatch,
  makePatternMatch,
} from './fixtures';

const mockRuleMatch = {
  rule_id: 1,
  source_id: 1,
  log_line: {
    timestamp: '2024-01-15T10:30:00.000',
    source_id: 1,
    raw: 'ERROR test',
    content: 'test',
  },
  extracted_state: {},
};

const mockPatternMatch = {
  pattern_id: 1,
  timestamp: '2024-01-15T10:30:05.000',
  state_snapshot: {},
};

const mockStateChange = {
  timestamp: '2024-01-15T10:30:00.000',
  source_id: 1,
  source_name: 'app.log',
  state_key: 'status',
  old_value: null,
  new_value: { String: 'error_detected' },
  rule_id: 1,
};

// Default runStreaming: immediately calls callbacks then onComplete
function defaultRunStreaming(_pid: number, callbacks: any) {
  callbacks.onRuleMatch(mockRuleMatch);
  callbacks.onPatternMatch(mockPatternMatch);
  callbacks.onStateChange(mockStateChange);
  // Trigger flush via a microtask + timer to simulate the 100ms interval
  setTimeout(() => {
    callbacks.onComplete({
      total_lines: 5,
      total_rule_matches: 1,
      total_pattern_matches: 1,
      total_state_changes: 1,
    });
  }, 0);
  return { close: vi.fn() };
}

let runStreamingImpl = defaultRunStreaming;

// Mock the api module
vi.mock('../api', () => {
  const mockSources = [{ id: 1, name: 'app.log', template_id: 1, file_path: '/var/log/app.log' }];
  const mockRules = [
    { id: 1, name: 'Error Rule', match_mode: 'Any', match_rules: [], extraction_rules: [] },
  ];
  const mockPatterns = [{ id: 1, name: 'Failure Pattern', predicates: [] }];

  return {
    analysis: {
      run: vi.fn(),
      runStreaming: vi.fn((...args: any[]) => runStreamingImpl(args[0], args[1])),
      detectTemplate: vi.fn(),
      suggestRule: vi.fn(),
    },
    clustering: {
      run: vi.fn().mockResolvedValue({ clusters: [], total_lines: 0 }),
    },
    sources: {
      list: vi.fn().mockResolvedValue(mockSources),
    },
    rules: {
      list: vi.fn().mockResolvedValue(mockRules),
    },
    patterns: {
      list: vi.fn().mockResolvedValue(mockPatterns),
    },
    // Re-export types are just interfaces, not needed at runtime
  };
});

// Get a reference to the mocked module
import {
  analysis as analysisApi,
  sources as sourcesApi,
  rules as rulesApi,
  patterns as patternsApi,
} from '../api';

function renderAnalysis(projectId = 1) {
  return render(AnalysisView, { props: { projectId } });
}

describe('AnalysisView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    runStreamingImpl = defaultRunStreaming;
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // --- Behavioral ---

  it('renders "Run Analysis" button', async () => {
    renderAnalysis();
    await tick();
    expect(screen.getByText('Run Analysis')).toBeInTheDocument();
  });

  it('tab defaults to "Table" with active class', async () => {
    renderAnalysis();
    await tick();

    // Click run and wait for results
    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('Table')).toBeInTheDocument();
    });

    const tableBtn = screen.getByText('Table');
    expect(tableBtn.classList.contains('active')).toBe(true);
  });

  it('clicking "Timeline" tab switches view', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('Timeline')).toBeInTheDocument();
    });

    await fireEvent.click(screen.getByText('Timeline'));
    const timelineBtn = screen.getByText('Timeline');
    expect(timelineBtn.classList.contains('active')).toBe(true);

    // Zoom controls appear in timeline mode
    expect(screen.getByTitle('Zoom in')).toBeInTheDocument();
  });

  it('clicking "Table" switches back', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('Timeline')).toBeInTheDocument();
    });

    // Switch to timeline
    await fireEvent.click(screen.getByText('Timeline'));
    // Switch back to table
    await fireEvent.click(screen.getByText('Table'));

    const tableBtn = screen.getByText('Table');
    expect(tableBtn.classList.contains('active')).toBe(true);
  });

  it('shows result stats after analysis completes', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('Results')).toBeInTheDocument();
    });

    // Stats section contains stat-label elements
    expect(screen.getAllByText('Rule Matches').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Pattern Matches').length).toBeGreaterThanOrEqual(1);
  });

  it('shows error banner on streaming error', async () => {
    runStreamingImpl = (_pid: number, callbacks: any) => {
      setTimeout(() => {
        callbacks.onError('Server error');
      }, 0);
      return { close: vi.fn() };
    };
    vi.mocked(analysisApi.runStreaming).mockImplementation((...args: any[]) =>
      runStreamingImpl(args[0], args[1]),
    );

    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('Server error')).toBeInTheDocument();
    });
  });

  it('button shows "Running..." while analysis is in progress', async () => {
    // runStreaming that never completes
    runStreamingImpl = (_pid: number, _callbacks: any) => {
      return { close: vi.fn() };
    };
    vi.mocked(analysisApi.runStreaming).mockImplementation((...args: any[]) =>
      runStreamingImpl(args[0], args[1]),
    );

    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    await tick();

    expect(screen.getByText('Running...')).toBeInTheDocument();
  });

  // --- Auto-rerun ---

  it('auto-reruns on invalidation after debounce', async () => {
    renderAnalysis();
    await tick();

    invalidateAnalysis();
    await tick();

    // Before debounce fires, runStreaming should not have been called
    expect(analysisApi.runStreaming).not.toHaveBeenCalled();

    // Advance past the 500ms debounce
    vi.advanceTimersByTime(500);
    await tick();

    expect(analysisApi.runStreaming).toHaveBeenCalledTimes(1);
  });

  it('debounces rapid invalidations', async () => {
    renderAnalysis();
    await tick();

    invalidateAnalysis();
    await tick();
    vi.advanceTimersByTime(100);
    invalidateAnalysis();
    await tick();
    vi.advanceTimersByTime(100);
    invalidateAnalysis();
    await tick();

    // Advance past the debounce from the last invalidation
    vi.advanceTimersByTime(500);
    await tick();

    // Should only have been called once (debounced)
    expect(analysisApi.runStreaming).toHaveBeenCalledTimes(1);
  });

  it('cancels in-flight analysis on re-trigger', async () => {
    // First run: never completes
    const closeHandle = vi.fn();
    runStreamingImpl = (_pid: number, _callbacks: any) => {
      return { close: closeHandle };
    };
    vi.mocked(analysisApi.runStreaming).mockImplementation((...args: any[]) =>
      runStreamingImpl(args[0], args[1]),
    );

    renderAnalysis();
    await tick();

    // Manual run
    await fireEvent.click(screen.getByText('Run Analysis'));
    await tick();

    expect(analysisApi.runStreaming).toHaveBeenCalledTimes(1);
    expect(closeHandle).not.toHaveBeenCalled();

    // Invalidate to trigger auto-rerun
    invalidateAnalysis();
    await tick();
    vi.advanceTimersByTime(500);
    await tick();

    // The first handle should have been closed
    expect(closeHandle).toHaveBeenCalled();
    // A new run should have started
    expect(analysisApi.runStreaming).toHaveBeenCalledTimes(2);
  });

  it('shows "Re-analyzing..." text during auto-triggered run', async () => {
    // runStreaming that never completes
    runStreamingImpl = (_pid: number, _callbacks: any) => {
      return { close: vi.fn() };
    };
    vi.mocked(analysisApi.runStreaming).mockImplementation((...args: any[]) =>
      runStreamingImpl(args[0], args[1]),
    );

    renderAnalysis();
    await tick();

    invalidateAnalysis();
    await tick();
    vi.advanceTimersByTime(500);
    await tick();

    expect(screen.getByText('Re-analyzing...')).toBeInTheDocument();
  });

  it('"Clusters" tab appears and switches view', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('Clusters')).toBeInTheDocument();
    });

    await fireEvent.click(screen.getByText('Clusters'));
    const clustersBtn = screen.getByText('Clusters');
    expect(clustersBtn.classList.contains('active')).toBe(true);
  });

  it('"State Evolution" tab appears and switches view', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('State Evolution')).toBeInTheDocument();
    });

    await fireEvent.click(screen.getByText('State Evolution'));
    const stateBtn = screen.getByText('State Evolution');
    expect(stateBtn.classList.contains('active')).toBe(true);
  });

  // --- Facet Filtering ---

  it('renders rule and source facet chips after analysis', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('Results')).toBeInTheDocument();
    });

    // Rule chip and badge both show "Error Rule"
    expect(screen.getAllByText('Error Rule').length).toBeGreaterThanOrEqual(1);
    // Source chip: "app.log" — may appear multiple times (source button + facet chip)
    expect(screen.getAllByText('app.log').length).toBeGreaterThanOrEqual(1);
    // Facet chips should have count badges
    const chipCounts = screen.getAllByText('1', { selector: '.chip-count' });
    expect(chipCounts.length).toBeGreaterThanOrEqual(1);
  });

  it('clicking a rule facet chip filters results', async () => {
    // Set up 2 rules with matches
    const multiRunStreaming = (_pid: number, callbacks: any) => {
      callbacks.onRuleMatch({
        rule_id: 1,
        source_id: 1,
        log_line: {
          timestamp: '2024-01-15T10:30:00.000',
          source_id: 1,
          raw: 'ERROR test',
          content: 'test',
        },
        extracted_state: {},
      });
      callbacks.onRuleMatch({
        rule_id: 2,
        source_id: 1,
        log_line: {
          timestamp: '2024-01-15T10:30:01.000',
          source_id: 1,
          raw: 'WARN test',
          content: 'warn test',
        },
        extracted_state: {},
      });
      setTimeout(() => {
        callbacks.onComplete({
          total_lines: 10,
          total_rule_matches: 2,
          total_pattern_matches: 0,
          total_state_changes: 0,
        });
      }, 0);
      return { close: vi.fn() };
    };

    runStreamingImpl = multiRunStreaming;
    vi.mocked(analysisApi.runStreaming).mockImplementation((...args: any[]) =>
      runStreamingImpl(args[0], args[1]),
    );

    // Mock rules to include a second rule
    vi.mocked(rulesApi.list).mockResolvedValue([
      { id: 1, name: 'Error Rule', match_mode: 'Any', match_rules: [], extraction_rules: [] },
      { id: 2, name: 'Warn Rule', match_mode: 'Any', match_rules: [], extraction_rules: [] },
    ]);

    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await tick();
    await vi.waitFor(() => {
      expect(screen.getAllByText('Error Rule').length).toBeGreaterThanOrEqual(1);
    });

    // Click the "Error Rule" facet chip (the one inside .facet-chips)
    const errorChip = screen
      .getAllByText('Error Rule')
      .find((el) => el.closest('.facet-chip'))!
      .closest('button')!;
    await fireEvent.click(errorChip);
    await tick();

    // Should show filter status
    expect(screen.getByText(/Showing 1 of 2 matches/)).toBeInTheDocument();
  });

  it('clicking the same chip again clears the filter', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await tick();
    await vi.waitFor(() => {
      expect(screen.getAllByText('Error Rule').length).toBeGreaterThanOrEqual(1);
    });

    // Click to activate — find the facet chip specifically
    const chip = screen
      .getAllByText('Error Rule')
      .find((el) => el.closest('.facet-chip'))!
      .closest('button')!;
    await fireEvent.click(chip);
    await tick();

    expect(screen.getByText(/Showing/)).toBeInTheDocument();

    // Click again to deactivate
    await fireEvent.click(chip);
    await tick();

    expect(screen.queryByText(/Showing/)).not.toBeInTheDocument();
  });

  it('Clear filters button resets all filters', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await tick();
    await vi.waitFor(() => {
      expect(screen.getAllByText('Error Rule').length).toBeGreaterThanOrEqual(1);
    });

    // Activate a rule filter
    const chip = screen
      .getAllByText('Error Rule')
      .find((el) => el.closest('.facet-chip'))!
      .closest('button')!;
    await fireEvent.click(chip);
    await tick();

    expect(screen.getByText(/Showing/)).toBeInTheDocument();

    // Click "Clear filters"
    await fireEvent.click(screen.getByText('Clear filters'));
    await tick();

    expect(screen.queryByText(/Showing/)).not.toBeInTheDocument();
  });

  it('filters reset on new analysis run', async () => {
    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await tick();
    await vi.waitFor(() => {
      expect(screen.getAllByText('Error Rule').length).toBeGreaterThanOrEqual(1);
    });

    // Activate filter
    const chip = screen
      .getAllByText('Error Rule')
      .find((el) => el.closest('.facet-chip'))!
      .closest('button')!;
    await fireEvent.click(chip);
    await tick();

    expect(screen.getByText(/Showing/)).toBeInTheDocument();

    // Run analysis again
    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await tick();
    await vi.waitFor(() => {
      expect(screen.getByText('Results')).toBeInTheDocument();
    });

    // Filter should be cleared
    expect(screen.queryByText(/Showing/)).not.toBeInTheDocument();
  });

  // --- Snapshot ---

  it('matches snapshot with results in table mode', async () => {
    const { container } = renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    vi.advanceTimersByTime(200);
    await waitFor(() => {
      expect(screen.getByText('Results')).toBeInTheDocument();
    });

    expect(container.innerHTML).toMatchSnapshot();
  });
});
