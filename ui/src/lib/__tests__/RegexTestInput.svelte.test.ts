import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import RegexTestInput from '../RegexTestInput.svelte';

vi.mock('../api', () => ({
  sources: {
    list: vi.fn().mockResolvedValue([]),
    rawLines: vi.fn().mockResolvedValue({ lines: [], total_lines: 0 }),
  },
}));

function renderInput(pattern = '\\d+') {
  return render(RegexTestInput, { props: { pattern, projectId: 1 } });
}

describe('RegexTestInput', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Test toggle button', () => {
    renderInput();
    expect(screen.getByText('Test')).toBeTruthy();
  });

  it('shows test input when expanded', async () => {
    renderInput();
    await fireEvent.click(screen.getByText('Test'));
    await tick();

    expect(screen.getByPlaceholderText('Paste a sample line to test...')).toBeTruthy();
  });

  it('shows "Match" verdict when pattern matches sample', async () => {
    renderInput('\\d+');
    await fireEvent.click(screen.getByText('Test'));
    await tick();

    const input = screen.getByPlaceholderText('Paste a sample line to test...');
    await fireEvent.input(input, { target: { value: 'error code 42' } });

    await waitFor(() => {
      const verdict = document.querySelector('.verdict.ok');
      expect(verdict).toBeTruthy();
      expect(verdict!.textContent).toContain('Match');
    });
  });

  it('shows "No match" verdict when pattern does not match', async () => {
    renderInput('\\d+');
    await fireEvent.click(screen.getByText('Test'));
    await tick();

    const input = screen.getByPlaceholderText('Paste a sample line to test...');
    await fireEvent.input(input, { target: { value: 'no numbers here' } });

    await waitFor(() => {
      const verdict = document.querySelector('.verdict.fail');
      expect(verdict).toBeTruthy();
      expect(verdict!.textContent).toContain('No match');
    });
  });

  it('shows captured groups on match', async () => {
    renderInput('(?P<code>\\d+)');
    await fireEvent.click(screen.getByText('Test'));
    await tick();

    const input = screen.getByPlaceholderText('Paste a sample line to test...');
    await fireEvent.input(input, { target: { value: 'error 42' } });

    await waitFor(() => {
      expect(screen.getByText('code:')).toBeTruthy();
      expect(screen.getByText('"42"')).toBeTruthy();
    });
  });

  it('shows Browse file button when expanded', async () => {
    renderInput();
    await fireEvent.click(screen.getByText('Test'));
    await tick();

    expect(screen.getByText('Browse file')).toBeTruthy();
  });
});
