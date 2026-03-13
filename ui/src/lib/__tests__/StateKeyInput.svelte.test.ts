import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import StateKeyInput from '../StateKeyInput.svelte';

function renderInput(value = '', knownKeys: string[] = []) {
  return render(StateKeyInput, { props: { value, knownKeys } });
}

describe('StateKeyInput', () => {
  it('renders with empty value and no dropdown visible initially', () => {
    renderInput('', ['foo', 'bar']);
    const input = screen.getByRole('textbox') as HTMLInputElement;
    expect(input.value).toBe('');
    expect(screen.queryByRole('listbox')).toBeNull();
    expect(screen.queryByText('foo')).toBeNull();
  });

  it('shows dropdown on focus when knownKeys is non-empty', async () => {
    renderInput('', ['foo', 'bar', 'baz']);
    const input = screen.getByRole('textbox');
    await fireEvent.focus(input);
    await tick();

    expect(screen.getByText('foo')).toBeInTheDocument();
    expect(screen.getByText('bar')).toBeInTheDocument();
    expect(screen.getByText('baz')).toBeInTheDocument();
  });

  it('does not show dropdown when knownKeys is empty', async () => {
    renderInput('', []);
    const input = screen.getByRole('textbox');
    await fireEvent.focus(input);
    await tick();

    expect(screen.queryByRole('list')).toBeNull();
  });

  it('filters suggestions by substring match as user types', async () => {
    renderInput('', ['foo', 'bar', 'foobar']);
    const input = screen.getByRole('textbox');
    await fireEvent.focus(input);
    await fireEvent.input(input, { target: { value: 'foo' } });
    await tick();

    // 'foo' itself is excluded since value === key; 'foobar' matches; 'bar' does not
    expect(screen.queryByText('foo')).toBeNull();
    expect(screen.getByText('foobar')).toBeInTheDocument();
    expect(screen.queryByText('bar')).toBeNull();
  });

  it('shows all keys when query is empty string on focus', async () => {
    renderInput('', ['alpha', 'beta']);
    const input = screen.getByRole('textbox');
    await fireEvent.focus(input);
    await tick();

    expect(screen.getByText('alpha')).toBeInTheDocument();
    expect(screen.getByText('beta')).toBeInTheDocument();
  });

  it('selecting an option sets value and closes dropdown', async () => {
    renderInput('', ['foo', 'bar']);
    const input = screen.getByRole('textbox') as HTMLInputElement;
    await fireEvent.focus(input);
    await tick();

    const fooItem = screen.getByText('foo');
    await fireEvent.mouseDown(fooItem);
    await tick();

    expect(input.value).toBe('foo');
    expect(screen.queryByText('bar')).toBeNull();
  });

  it('typing a new key without selecting preserves the typed value', async () => {
    renderInput('', ['foo', 'bar']);
    const input = screen.getByRole('textbox') as HTMLInputElement;
    await fireEvent.focus(input);
    await fireEvent.input(input, { target: { value: 'custom_key' } });
    await tick();

    // No matching suggestions — dropdown hidden
    expect(screen.queryByText('foo')).toBeNull();
    expect(input.value).toBe('custom_key');
  });

  it('Escape closes the dropdown', async () => {
    renderInput('', ['foo', 'bar']);
    const input = screen.getByRole('textbox');
    await fireEvent.focus(input);
    await tick();
    expect(screen.getByText('foo')).toBeInTheDocument();

    await fireEvent.keyDown(input, { key: 'Escape' });
    await tick();

    expect(screen.queryByText('foo')).toBeNull();
  });

  it('ArrowDown highlights the first item', async () => {
    renderInput('', ['alpha', 'beta']);
    const input = screen.getByRole('textbox');
    await fireEvent.focus(input);
    await tick();

    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await tick();

    const items = screen.getAllByRole('listitem');
    expect(items[0].classList.contains('highlighted')).toBe(true);
    expect(items[1].classList.contains('highlighted')).toBe(false);
  });

  it('ArrowDown then Enter selects the highlighted item', async () => {
    renderInput('', ['alpha', 'beta']);
    const input = screen.getByRole('textbox') as HTMLInputElement;
    await fireEvent.focus(input);
    await tick();

    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await tick();
    await fireEvent.keyDown(input, { key: 'Enter' });
    await tick();

    expect(input.value).toBe('alpha');
    expect(screen.queryByText('beta')).toBeNull();
  });

  it('ArrowDown ArrowDown ArrowUp navigates correctly', async () => {
    renderInput('', ['alpha', 'beta', 'gamma']);
    const input = screen.getByRole('textbox');
    await fireEvent.focus(input);
    await tick();

    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await fireEvent.keyDown(input, { key: 'ArrowUp' });
    await tick();

    const items = screen.getAllByRole('listitem');
    // Should be back at index 0
    expect(items[0].classList.contains('highlighted')).toBe(true);
    expect(items[1].classList.contains('highlighted')).toBe(false);
  });
});
