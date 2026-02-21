<script lang="ts">
  import { timestampTemplates as tsTemplatesApi } from './api';
  import { formatTimestamp } from './formatUtils';
  import RawFileViewer from './RawFileViewer.svelte';

  let {
    format,
    extractionRegex,
    defaultYear,
    projectId,
  }: {
    format: string;
    extractionRegex: string;
    defaultYear: string;
    projectId: number;
  } = $props();

  let expanded = $state(false);
  let sampleLine = $state('');
  let showFileViewer = $state(false);
  let loading = $state(false);
  let result: { parsed_timestamp: string } | null = $state(null);
  let error: string | null = $state(null);

  let debounceTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    // Track reactive dependencies
    const _f = format;
    const _e = extractionRegex;
    const _d = defaultYear;
    const _s = sampleLine;

    clearTimeout(debounceTimer);
    result = null;
    error = null;

    if (!_f || !_s) {
      loading = false;
      return;
    }

    loading = true;
    debounceTimer = setTimeout(async () => {
      try {
        const data: {
          format: string;
          sample: string;
          extraction_regex?: string | null;
          default_year?: number | null;
        } = {
          format: _f,
          sample: _s,
          extraction_regex: _e || null,
          default_year: _d ? Number(_d) : null,
        };
        result = await tsTemplatesApi.testFormat(projectId, data);
        error = null;
      } catch (e: any) {
        result = null;
        error = e.message || 'Failed to test format';
      } finally {
        loading = false;
      }
    }, 300);

    return () => clearTimeout(debounceTimer);
  });
</script>

<div class="format-test">
  <button class="toggle-btn" onclick={() => (expanded = !expanded)} type="button">
    {expanded ? 'Hide test' : 'Test'}
  </button>

  {#if expanded}
    <div class="test-area">
      <div class="test-row">
        <input
          class="test-input"
          type="text"
          bind:value={sampleLine}
          placeholder="Paste a sample line to test..."
        />
        <button class="browse-btn" type="button" onclick={() => (showFileViewer = true)}
          >Browse file</button
        >
      </div>

      {#if loading}
        <div class="verdict loading">Testing...</div>
      {:else if result}
        <div class="verdict ok">
          Parsed: {formatTimestamp(result.parsed_timestamp)}
          <span class="raw-ts" title={result.parsed_timestamp}>({result.parsed_timestamp})</span>
        </div>
      {:else if error}
        <div class="verdict fail">{error}</div>
      {/if}
    </div>
  {/if}

  {#if showFileViewer}
    <RawFileViewer
      {projectId}
      onSelect={(text) => {
        sampleLine = text;
        showFileViewer = false;
      }}
      onClose={() => (showFileViewer = false)}
    />
  {/if}
</div>

<style>
  .format-test {
    margin-top: 4px;
  }

  .toggle-btn {
    font-size: 11px;
    padding: 1px 6px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--text-dim);
    cursor: pointer;
  }

  .toggle-btn:hover {
    background: var(--bg-secondary);
  }

  .test-area {
    margin-top: 6px;
  }

  .test-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .test-input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 4px 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-primary);
    color: var(--text-primary);
  }

  .browse-btn {
    font-size: 11px;
    padding: 4px 8px;
    white-space: nowrap;
  }

  .verdict {
    margin-top: 6px;
    padding: 4px 8px;
    border-radius: var(--radius);
    font-size: 12px;
    font-weight: 600;
  }

  .verdict.ok {
    color: var(--green);
    background: rgba(158, 206, 106, 0.1);
  }

  .verdict.fail {
    color: var(--red);
    background: rgba(247, 118, 142, 0.1);
  }

  .verdict.loading {
    color: var(--text-dim);
    background: var(--bg-tertiary);
  }

  .raw-ts {
    font-weight: 400;
    font-size: 11px;
    font-family: var(--font-mono);
    opacity: 0.7;
  }
</style>
