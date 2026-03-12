import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import RawFileViewer from '../RawFileViewer.svelte';

vi.mock('../api', () => ({
  sources: {
    list: vi.fn().mockResolvedValue([
      {
        id: 1,
        name: 'server.log',
        template_id: 1,
        file_path: '/var/log/server.log',
        color: '#60a5fa',
      },
      { id: 2, name: 'app.log', template_id: 1, file_path: '/var/log/app.log', color: '#34d399' },
    ]),
    rawLines: vi.fn().mockResolvedValue({
      lines: ['line one', 'line two', 'line three'],
      total_lines: 5,
    }),
  },
}));

import { sources as sourcesApi } from '../api';

function renderViewer(onSelect = vi.fn(), onClose = vi.fn()) {
  return {
    ...render(RawFileViewer, { props: { projectId: 1, onSelect, onClose } }),
    onSelect,
    onClose,
  };
}

describe('RawFileViewer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(sourcesApi.list).mockResolvedValue([
      {
        id: 1,
        name: 'server.log',
        template_id: 1,
        file_path: '/var/log/server.log',
        color: '#60a5fa',
      },
      { id: 2, name: 'app.log', template_id: 1, file_path: '/var/log/app.log', color: '#34d399' },
    ]);
    vi.mocked(sourcesApi.rawLines).mockResolvedValue({
      lines: ['line one', 'line two', 'line three'],
      total_lines: 5,
    });
  });

  it('renders source buttons from API', async () => {
    renderViewer();
    await tick();

    await waitFor(() => {
      expect(screen.getByText('server.log')).toBeTruthy();
      expect(screen.getByText('app.log')).toBeTruthy();
    });
  });

  it('loads lines when source is clicked', async () => {
    renderViewer();
    await tick();

    await waitFor(() => {
      expect(screen.getByText('server.log')).toBeTruthy();
    });

    await fireEvent.click(screen.getByText('server.log'));

    await waitFor(() => {
      expect(sourcesApi.rawLines).toHaveBeenCalledWith(1, 1, 0, 200);
      expect(screen.getByText('line one')).toBeTruthy();
      expect(screen.getByText('line two')).toBeTruthy();
    });
  });

  it('shows line numbers', async () => {
    renderViewer();
    await tick();

    await waitFor(() => {
      expect(screen.getByText('server.log')).toBeTruthy();
    });

    await fireEvent.click(screen.getByText('server.log'));

    await waitFor(() => {
      expect(screen.getByText('1')).toBeTruthy();
      expect(screen.getByText('2')).toBeTruthy();
      expect(screen.getByText('3')).toBeTruthy();
    });
  });

  it('calls onSelect when Use button is clicked', async () => {
    const { onSelect } = renderViewer();
    await tick();

    await waitFor(() => {
      expect(screen.getByText('server.log')).toBeTruthy();
    });

    await fireEvent.click(screen.getByText('server.log'));

    await waitFor(() => {
      expect(screen.getByText('line one')).toBeTruthy();
    });

    const useButtons = screen.getAllByText('Use');
    await fireEvent.click(useButtons[0]);

    expect(onSelect).toHaveBeenCalledWith('line one');
  });

  it('calls onClose when close button is clicked', async () => {
    const { onClose } = renderViewer();
    await tick();

    const closeBtn = screen.getByText('\u00D7');
    await fireEvent.click(closeBtn);

    expect(onClose).toHaveBeenCalled();
  });

  it('shows Load more when more lines available', async () => {
    renderViewer();
    await tick();

    await waitFor(() => {
      expect(screen.getByText('server.log')).toBeTruthy();
    });

    await fireEvent.click(screen.getByText('server.log'));

    await waitFor(() => {
      expect(screen.getByText(/Load more/)).toBeTruthy();
      expect(screen.getByText(/3 \/ 5/)).toBeTruthy();
    });
  });

  it('renders Open local file button', async () => {
    renderViewer();
    await tick();

    expect(screen.getByText('Open local file')).toBeTruthy();
  });

  it('displays lines from a local file', async () => {
    renderViewer();
    await tick();

    const fileContent = 'first line\nsecond line\nthird line\n';
    const file = new File([fileContent], 'test.log', { type: 'text/plain' });

    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    expect(input).toBeTruthy();

    Object.defineProperty(input, 'files', { value: [file] });
    await fireEvent.change(input);

    await waitFor(() => {
      expect(screen.getByText('first line')).toBeTruthy();
      expect(screen.getByText('second line')).toBeTruthy();
      expect(screen.getByText('third line')).toBeTruthy();
      expect(screen.getByText('test.log')).toBeTruthy();
    });

    // Line numbers should be present
    expect(screen.getByText('1')).toBeTruthy();
    expect(screen.getByText('2')).toBeTruthy();
    expect(screen.getByText('3')).toBeTruthy();
  });

  it('local file clears source selection', async () => {
    renderViewer();
    await tick();

    await waitFor(() => {
      expect(screen.getByText('server.log')).toBeTruthy();
    });

    // Select a source first
    await fireEvent.click(screen.getByText('server.log'));
    await waitFor(() => {
      expect(screen.getByText('line one')).toBeTruthy();
    });

    // Now load a local file
    const file = new File(['local content\n'], 'local.log', { type: 'text/plain' });
    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    Object.defineProperty(input, 'files', { value: [file] });
    await fireEvent.change(input);

    await waitFor(() => {
      expect(screen.getByText('local content')).toBeTruthy();
      expect(screen.getByText('local.log')).toBeTruthy();
    });

    // Source lines should be replaced
    expect(screen.queryByText('line one')).toBeNull();

    // No "Load more" for local files
    expect(screen.queryByText(/Load more/)).toBeNull();
  });
});
