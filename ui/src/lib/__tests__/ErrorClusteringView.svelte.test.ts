import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import ErrorClusteringView from '../ErrorClusteringView.svelte';

const mockClusterResult = {
  clusters: [
    {
      template: 'ERROR <*> timeout after <*> ms',
      count: 42,
      source_ids: [1],
      sample_lines: [
        'ERROR db timeout after 100 ms',
        'ERROR api timeout after 200 ms',
        'ERROR cache timeout after 300 ms',
      ],
    },
    {
      template: 'INFO started successfully',
      count: 10,
      source_ids: [1, 2],
      sample_lines: ['INFO started successfully'],
    },
  ],
  total_lines: 52,
};

const mockSources = [
  { id: 1, name: 'app.log', template_id: 1, file_path: '/var/log/app.log' },
  { id: 2, name: 'system.log', template_id: 1, file_path: '/var/log/system.log' },
];

let clusterRunImpl: () => Promise<typeof mockClusterResult>;

vi.mock('../api', () => ({
  clustering: {
    run: vi.fn((..._args: any[]) => clusterRunImpl()),
  },
  rules: { list: vi.fn().mockResolvedValue([]) },
  rulesets: { list: vi.fn().mockResolvedValue([]) },
  analysis: { suggestRule: vi.fn().mockResolvedValue({ pattern: 'ERROR', capture_groups: [] }) },
}));

function renderView(projectId = 1) {
  return render(ErrorClusteringView, {
    props: { projectId, sourceList: mockSources },
  });
}

describe('ErrorClusteringView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clusterRunImpl = () => Promise.resolve(mockClusterResult);
  });

  it('shows loading state while fetching', async () => {
    let resolve: (v: typeof mockClusterResult) => void;
    clusterRunImpl = () =>
      new Promise((r) => {
        resolve = r;
      });

    renderView();
    await tick();

    expect(screen.getByText('Clustering log lines...')).toBeInTheDocument();

    // Resolve to avoid unhandled promise
    resolve!(mockClusterResult);
    await tick();
  });

  it('displays clusters after loading', async () => {
    renderView();
    await waitFor(() => {
      expect(screen.getByText('2 clusters from 52 lines')).toBeInTheDocument();
    });

    expect(screen.getByText('ERROR <*> timeout after <*> ms')).toBeInTheDocument();
    expect(screen.getByText('INFO started successfully')).toBeInTheDocument();
    expect(screen.getByText('42')).toBeInTheDocument();
    expect(screen.getByText('10')).toBeInTheDocument();
  });

  it('expands cluster to show samples', async () => {
    renderView();
    await waitFor(() => {
      expect(screen.getByText('ERROR <*> timeout after <*> ms')).toBeInTheDocument();
    });

    // Click the cluster summary to expand
    await fireEvent.click(screen.getByText('ERROR <*> timeout after <*> ms'));
    await tick();

    expect(screen.getByText('ERROR db timeout after 100 ms')).toBeInTheDocument();
    expect(screen.getByText('ERROR api timeout after 200 ms')).toBeInTheDocument();
    expect(screen.getByText('ERROR cache timeout after 300 ms')).toBeInTheDocument();
  });

  it('Create Rule opens RuleCreator', async () => {
    renderView();
    await waitFor(() => {
      expect(screen.getAllByText('Create Rule').length).toBeGreaterThanOrEqual(1);
    });

    // Click the first "Create Rule" button
    await fireEvent.click(screen.getAllByText('Create Rule')[0]);
    await tick();

    // RuleCreator modal should be visible
    expect(screen.getByText('Create Rule from Selection')).toBeInTheDocument();
  });

  it('shows error state on API failure', async () => {
    clusterRunImpl = () => Promise.reject(new Error('Server error'));

    renderView();
    await waitFor(() => {
      expect(screen.getByText('Server error')).toBeInTheDocument();
    });
  });

  it('matches snapshot', async () => {
    const { container } = renderView();
    await waitFor(() => {
      expect(screen.getByText('2 clusters from 52 lines')).toBeInTheDocument();
    });
    expect(container.innerHTML).toMatchSnapshot();
  });
});
