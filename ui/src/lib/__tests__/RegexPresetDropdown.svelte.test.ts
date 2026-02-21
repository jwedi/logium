import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import RegexPresetDropdown from '../RegexPresetDropdown.svelte';

describe('RegexPresetDropdown', () => {
  it('renders Presets button for field type with presets', () => {
    render(RegexPresetDropdown, {
      props: { fieldType: 'content_regex', onSelect: vi.fn() },
    });
    expect(screen.getByText(/Presets/)).toBeInTheDocument();
  });

  it('opens dropdown on click and shows preset items', async () => {
    render(RegexPresetDropdown, {
      props: { fieldType: 'continuation_regex', onSelect: vi.fn() },
    });

    await fireEvent.click(screen.getByText(/Presets/));

    expect(screen.getByText('Indented')).toBeInTheDocument();
    expect(screen.getByText('Tab-indented')).toBeInTheDocument();
    expect(screen.getByText('Caused by')).toBeInTheDocument();
    expect(screen.getByText('Java stack trace')).toBeInTheDocument();
  });

  it('calls onSelect with correct pattern on item click', async () => {
    const onSelect = vi.fn();
    render(RegexPresetDropdown, {
      props: { fieldType: 'continuation_regex', onSelect },
    });

    await fireEvent.click(screen.getByText(/Presets/));
    await fireEvent.click(screen.getByText('Indented'));

    expect(onSelect).toHaveBeenCalledWith('^\\s+');
  });

  it('closes dropdown after selection', async () => {
    render(RegexPresetDropdown, {
      props: { fieldType: 'continuation_regex', onSelect: vi.fn() },
    });

    await fireEvent.click(screen.getByText(/Presets/));
    expect(screen.getByText('Indented')).toBeInTheDocument();

    await fireEvent.click(screen.getByText('Indented'));
    expect(screen.queryByText('Indented')).not.toBeInTheDocument();
  });

  it('closes dropdown on outside click', async () => {
    render(RegexPresetDropdown, {
      props: { fieldType: 'content_regex', onSelect: vi.fn() },
    });

    await fireEvent.click(screen.getByText(/Presets/));
    expect(screen.getByText('Skip first token')).toBeInTheDocument();

    await fireEvent.mouseDown(document.body);
    expect(screen.queryByText('Skip first token')).not.toBeInTheDocument();
  });
});
