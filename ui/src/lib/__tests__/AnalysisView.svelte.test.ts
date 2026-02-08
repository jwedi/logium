import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import AnalysisView from '../AnalysisView.svelte';
import { makeAnalysisResult, makeSource, makeRule, makePattern, makeRuleMatch, makePatternMatch } from './fixtures';

const mockRuleMatch = {
  rule_id: 1,
  source_id: 1,
  log_line: { timestamp: '2024-01-15T10:30:00.000', source_id: 1, raw: 'ERROR test', content: 'test' },
  extracted_state: {},
};

const mockPatternMatch = {
  pattern_id: 1,
  timestamp: '2024-01-15T10:30:05.000',
  state_snapshot: {},
};

// Default runStreaming: immediately calls callbacks then onComplete
function defaultRunStreaming(_pid: number, callbacks: any) {
  callbacks.onRuleMatch(mockRuleMatch);
  callbacks.onPatternMatch(mockPatternMatch);
  // Trigger flush via a microtask + timer to simulate the 100ms interval
  setTimeout(() => {
    callbacks.onComplete({ total_lines: 5, total_rule_matches: 1, total_pattern_matches: 1 });
  }, 0);
  return { close: vi.fn() };
}

let runStreamingImpl = defaultRunStreaming;

// Mock the api module
vi.mock('../api', () => {
  const mockSources = [
    { id: 1, name: 'app.log', template_id: 1, file_path: '/var/log/app.log' },
  ];
  const mockRules = [
    { id: 1, name: 'Error Rule', match_mode: 'Any', match_rules: [], extraction_rules: [] },
  ];
  const mockPatterns = [
    { id: 1, name: 'Failure Pattern', predicates: [] },
  ];

  return {
    analysis: {
      run: vi.fn(),
      runStreaming: vi.fn((...args: any[]) => runStreamingImpl(args[0], args[1])),
      detectTemplate: vi.fn(),
      suggestRule: vi.fn(),
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
import { analysis as analysisApi, sources as sourcesApi, rules as rulesApi, patterns as patternsApi } from '../api';

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
    vi.mocked(analysisApi.runStreaming).mockImplementation((...args: any[]) => runStreamingImpl(args[0], args[1]));

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
    vi.mocked(analysisApi.runStreaming).mockImplementation((...args: any[]) => runStreamingImpl(args[0], args[1]));

    renderAnalysis();
    await tick();

    await fireEvent.click(screen.getByText('Run Analysis'));
    await tick();

    expect(screen.getByText('Running...')).toBeInTheDocument();
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
