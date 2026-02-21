<script lang="ts">
  import { FORMAT_PRESETS } from './regexPresets';

  let { onSelect }: { onSelect: (pattern: string) => void } = $props();

  let open = $state(false);

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    open = !open;
  }

  function select(pattern: string) {
    onSelect(pattern);
    open = false;
  }

  $effect(() => {
    if (!open) return;
    function onMouseDown(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (!target.closest('.preset-dropdown-wrapper')) {
        open = false;
      }
    }
    document.addEventListener('mousedown', onMouseDown);
    return () => document.removeEventListener('mousedown', onMouseDown);
  });
</script>

<span class="preset-dropdown-wrapper">
  <button type="button" class="preset-toggle" onclick={toggle}>Presets &#9662;</button>
  {#if open}
    <div class="preset-menu">
      {#each FORMAT_PRESETS as preset}
        <button type="button" class="preset-item" onclick={() => select(preset.pattern)}>
          <span class="preset-label">{preset.label}</span>
          <code class="preset-pattern">{preset.pattern}</code>
          <span class="preset-desc">{preset.description}</span>
        </button>
      {/each}
    </div>
  {/if}
</span>

<style>
  .preset-dropdown-wrapper {
    position: relative;
    display: inline-block;
  }

  .preset-toggle {
    font-size: 11px;
    padding: 1px 6px;
    background: transparent;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 3px;
    cursor: pointer;
    margin-left: 6px;
    vertical-align: baseline;
  }

  .preset-toggle:hover {
    background: var(--accent);
    color: var(--bg-primary);
  }

  .preset-menu {
    position: absolute;
    top: 100%;
    left: 0;
    z-index: 100;
    margin-top: 4px;
    min-width: 280px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    display: flex;
    flex-direction: column;
    padding: 4px 0;
  }

  .preset-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 10px;
    background: transparent;
    border: none;
    text-align: left;
    cursor: pointer;
    color: var(--text-primary);
    font-size: 12px;
  }

  .preset-item:hover {
    background: var(--bg-hover);
  }

  .preset-label {
    font-weight: 600;
  }

  .preset-pattern {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--accent);
  }

  .preset-desc {
    color: var(--text-dim);
    font-size: 11px;
  }
</style>
