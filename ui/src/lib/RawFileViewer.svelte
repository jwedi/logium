<script lang="ts">
  import { sources as sourcesApi, type Source, type RawLinesResponse } from './api';

  let {
    projectId,
    onSelect,
    onClose,
  }: { projectId: number; onSelect: (text: string) => void; onClose: () => void } = $props();

  let sourceList: Source[] = $state([]);
  let selectedSourceId: number | null = $state(null);
  let rawData: RawLinesResponse | null = $state(null);
  let offset = $state(0);
  let loading = $state(false);
  let error: string | null = $state(null);
  let localFileName: string | null = $state(null);
  let fileInput: HTMLInputElement;

  const LIMIT = 200;

  async function loadSources() {
    try {
      sourceList = await sourcesApi.list(projectId);
    } catch (e: any) {
      error = e.message;
    }
  }

  async function selectSource(id: number) {
    selectedSourceId = id;
    localFileName = null;
    offset = 0;
    rawData = null;
    await loadLines();
  }

  function handleFileSelect(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = () => {
      const text = reader.result as string;
      const lines = text.split('\n');
      // Remove trailing empty line from final newline
      if (lines.length > 0 && lines[lines.length - 1] === '') {
        lines.pop();
      }
      selectedSourceId = null;
      localFileName = file.name;
      rawData = { lines, total_lines: lines.length };
      error = null;
    };
    reader.onerror = () => {
      error = 'Failed to read file';
    };
    reader.readAsText(file);
  }

  async function loadLines() {
    if (selectedSourceId == null) return;
    loading = true;
    error = null;
    try {
      const resp = await sourcesApi.rawLines(projectId, selectedSourceId, offset, LIMIT);
      if (rawData && offset > 0) {
        rawData = { lines: [...rawData.lines, ...resp.lines], total_lines: resp.total_lines };
      } else {
        rawData = resp;
      }
    } catch (e: any) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  function loadMore() {
    if (!rawData) return;
    offset = rawData.lines.length;
    loadLines();
  }

  function useLine(line: string) {
    onSelect(line);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  $effect(() => {
    loadSources();
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-overlay" onclick={onClose}>
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <h2>Browse Raw File</h2>
      <button class="close-btn" onclick={onClose} type="button">&times;</button>
    </div>

    <div class="modal-body">
      <div class="source-picker">
        {#each sourceList as src}
          <button
            class="source-btn"
            class:active={selectedSourceId === src.id}
            onclick={() => selectSource(src.id)}
            type="button"
          >
            {src.name}
          </button>
        {/each}
        <input
          bind:this={fileInput}
          type="file"
          accept=".log,.txt,.csv,*"
          class="file-input-hidden"
          onchange={handleFileSelect}
        />
        <button
          class="source-btn local-file-btn"
          class:active={localFileName != null}
          onclick={() => fileInput.click()}
          type="button"
        >
          {localFileName ?? 'Open local file'}
        </button>
        {#if sourceList.length === 0 && !localFileName}
          <span class="hint">No sources in this project</span>
        {/if}
      </div>

      {#if error}
        <div class="error">{error}</div>
      {/if}

      {#if rawData}
        <div class="lines-container">
          {#each rawData.lines as line, i}
            <div class="line-row">
              <span class="line-num">{i + 1}</span>
              <span class="line-text">{line}</span>
              <button class="use-btn" type="button" onclick={() => useLine(line)}>Use</button>
            </div>
          {/each}
        </div>

        {#if selectedSourceId != null && rawData.lines.length < rawData.total_lines}
          <button class="load-more" type="button" onclick={loadMore} disabled={loading}>
            {loading
              ? 'Loading...'
              : `Load more (${rawData.lines.length} / ${rawData.total_lines})`}
          </button>
        {/if}
      {/if}

      {#if loading && !rawData}
        <div class="hint">Loading...</div>
      {/if}
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: 800px;
    max-width: 90vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 16px;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 20px;
    color: var(--text-dim);
    cursor: pointer;
    padding: 0 4px;
  }

  .modal-body {
    padding: 12px 16px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .source-picker {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .source-btn {
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--radius);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    cursor: pointer;
    color: var(--text-primary);
  }

  .source-btn.active {
    background: var(--accent);
    color: var(--bg-primary);
    border-color: var(--accent);
  }

  .file-input-hidden {
    display: none;
  }

  .local-file-btn {
    border-style: dashed;
  }

  .lines-container {
    max-height: 50vh;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .line-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 2px 8px;
    font-family: var(--font-mono);
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }

  .line-row:hover {
    background: var(--bg-tertiary);
  }

  .line-row:hover .use-btn {
    opacity: 1;
  }

  .line-num {
    color: var(--text-dim);
    min-width: 3ch;
    text-align: right;
    user-select: none;
    flex-shrink: 0;
  }

  .line-text {
    flex: 1;
    white-space: pre;
    overflow-x: auto;
  }

  .use-btn {
    opacity: 0;
    font-size: 10px;
    padding: 1px 6px;
    flex-shrink: 0;
    cursor: pointer;
    transition: opacity 0.1s;
  }

  .load-more {
    align-self: center;
    padding: 6px 16px;
    font-size: 13px;
  }

  .hint {
    font-size: 12px;
    color: var(--text-dim);
    font-style: italic;
  }

  .error {
    color: var(--red);
    font-size: 12px;
  }
</style>
