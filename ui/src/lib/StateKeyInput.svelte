<script lang="ts">
  let {
    value = $bindable(''),
    knownKeys = [],
    placeholder = 'state_key',
  }: {
    value: string;
    knownKeys: string[];
    placeholder?: string;
  } = $props();

  let open = $state(false);
  let highlightedIndex = $state(-1);
  let inputEl: HTMLInputElement;
  let wrapperEl: HTMLDivElement;

  let filtered = $derived.by(() => {
    if (!open) return [];
    const q = value.trim().toLowerCase();
    return knownKeys.filter((k) => k !== value && (q === '' || k.toLowerCase().includes(q)));
  });

  function select(key: string) {
    value = key;
    open = false;
    highlightedIndex = -1;
  }

  function onFocus() {
    open = true;
    highlightedIndex = -1;
  }

  function onInput() {
    open = true;
    highlightedIndex = -1;
  }

  function onKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      open = false;
      highlightedIndex = -1;
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      highlightedIndex = Math.min(highlightedIndex + 1, filtered.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      highlightedIndex = Math.max(highlightedIndex - 1, -1);
    } else if (e.key === 'Enter' && highlightedIndex >= 0) {
      e.preventDefault();
      select(filtered[highlightedIndex]);
    }
  }

  function onFocusout(e: FocusEvent) {
    const related = e.relatedTarget as Node | null;
    if (!wrapperEl?.contains(related)) {
      open = false;
      highlightedIndex = -1;
    }
  }
</script>

<div class="wrapper" bind:this={wrapperEl} onfocusout={onFocusout}>
  <input
    bind:this={inputEl}
    bind:value
    type="text"
    {placeholder}
    onfocus={onFocus}
    oninput={onInput}
    onkeydown={onKeydown}
    autocomplete="off"
  />
  {#if open && filtered.length > 0}
    <ul class="dropdown">
      {#each filtered as key, i}
        <li
          class:highlighted={i === highlightedIndex}
          onmousedown={(e) => {
            e.preventDefault();
            select(key);
          }}
        >
          {key}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .wrapper {
    position: relative;
    display: block;
    width: 100%;
  }

  input {
    width: 100%;
    box-sizing: border-box;
  }

  .dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin: 0;
    padding: 0;
    list-style: none;
    background: var(--bg-secondary, #1e1e2e);
    border: 1px solid var(--border, #45475a);
    border-top: none;
    border-radius: 0 0 4px 4px;
    max-height: 160px;
    overflow-y: auto;
    z-index: 100;
  }

  li {
    padding: 4px 8px;
    cursor: pointer;
    font-size: 0.85rem;
    color: var(--text-primary, #cdd6f4);
  }

  li:hover,
  li.highlighted {
    background: var(--bg-hover, #313244);
  }
</style>
