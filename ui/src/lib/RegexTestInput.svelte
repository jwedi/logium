<script lang="ts">
  import { testPattern, type RegexTestResult } from './regexUtils';
  import RawFileViewer from './RawFileViewer.svelte';

  let { pattern, projectId }: { pattern: string; projectId: number } = $props();

  let expanded = $state(false);
  let sampleLine = $state('');
  let showFileViewer = $state(false);

  let result: RegexTestResult | null = $derived(
    pattern && sampleLine ? testPattern(pattern, sampleLine) : null,
  );
</script>

<div class="regex-test">
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

      {#if result}
        <div
          class="verdict"
          class:ok={result.status === 'match'}
          class:fail={result.status !== 'match'}
        >
          {result.message}
        </div>
        {#if result.status === 'match' && Object.keys(result.groups).length > 0}
          <div class="groups">
            {#each Object.entries(result.groups) as [name, value]}
              <span class="group-item"><strong>{name}:</strong> "{value}"</span>
            {/each}
          </div>
        {/if}
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
  .regex-test {
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

  .groups {
    margin-top: 4px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .group-item {
    background: var(--bg-tertiary);
    padding: 2px 6px;
    border-radius: 3px;
  }
</style>
