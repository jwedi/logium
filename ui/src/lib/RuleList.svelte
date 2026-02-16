<script lang="ts">
  import {
    rules as rulesApi,
    sources as sourcesApi,
    analysis as analysisApi,
    type LogRule,
    type Source,
    type RuleMatch,
    type DryRunRequest,
  } from './api';
  import { invalidateAnalysis } from './analysisInvalidation.svelte';
  import RuleEditor from './RuleEditor.svelte';

  let { projectId }: { projectId: number } = $props();

  let ruleList: LogRule[] = $state([]);
  let loading = $state(false);
  let expandedId: number | null = $state(null);
  let editingId: number | null = $state(null);

  // Create form state
  let newName = $state('');
  let newMatchMode: 'Any' | 'All' = $state('Any');
  let newMatchPattern = $state('');
  let showCreate = $state(false);
  let newExtractionRules: {
    state_key: string;
    extraction_type: 'Parsed' | 'Static' | 'Clear';
    pattern: string;
    static_value: string;
    mode: 'Replace' | 'Accumulate';
  }[] = $state([]);

  // Source-level dry run state (create form)
  let availableSources: Source[] = $state([]);
  let newSelectedSourceId: number | '' = $state('');
  let newDryRunLoading = $state(false);
  let newDryRunResults: RuleMatch[] = $state([]);
  let newDryRunError = $state('');

  async function load() {
    loading = true;
    try {
      ruleList = await rulesApi.list(projectId);
    } catch (e: any) {
      alert(e.message);
    } finally {
      loading = false;
    }
  }

  function addNewExtractionRule() {
    newExtractionRules = [
      ...newExtractionRules,
      { state_key: '', extraction_type: 'Parsed', pattern: '', static_value: '', mode: 'Replace' },
    ];
  }

  function removeNewExtractionRule(i: number) {
    newExtractionRules = newExtractionRules.filter((_, idx) => idx !== i);
  }

  async function loadSources() {
    try {
      availableSources = await sourcesApi.list(projectId);
    } catch {
      /* ignore */
    }
  }

  async function runNewDryRun() {
    if (newSelectedSourceId === '' || !newMatchPattern.trim()) return;
    newDryRunLoading = true;
    newDryRunError = '';
    newDryRunResults = [];
    try {
      const data: DryRunRequest = {
        source_id: newSelectedSourceId as number,
        match_mode: newMatchMode,
        match_rules: [{ pattern: newMatchPattern }],
        extraction_rules: newExtractionRules.map((er) => ({
          extraction_type: er.extraction_type,
          state_key: er.state_key,
          pattern: er.extraction_type === 'Parsed' ? er.pattern || null : null,
          static_value: er.extraction_type === 'Static' ? er.static_value || null : null,
          mode: er.mode,
        })),
      };
      newDryRunResults = await analysisApi.dryRun(projectId, data);
    } catch (e: any) {
      newDryRunError = e.message;
    } finally {
      newDryRunLoading = false;
    }
  }

  function formatStateValue(sv: Record<string, unknown>): string {
    const key = Object.keys(sv)[0];
    return String(sv[key]);
  }

  async function createRule() {
    if (!newName.trim() || !newMatchPattern.trim()) return;
    try {
      await rulesApi.create(projectId, {
        name: newName.trim(),
        match_mode: newMatchMode,
        match_rules: [{ id: 0, pattern: newMatchPattern }],
        extraction_rules: newExtractionRules.map((er) => ({
          id: 0,
          extraction_type: er.extraction_type,
          state_key: er.state_key,
          pattern: er.extraction_type === 'Parsed' ? er.pattern || null : null,
          static_value: er.extraction_type === 'Static' ? er.static_value || null : null,
          mode: er.mode,
        })),
      });
      newName = '';
      newMatchPattern = '';
      newExtractionRules = [];
      newSelectedSourceId = '';
      newDryRunResults = [];
      newDryRunError = '';
      showCreate = false;
      await load();
      invalidateAnalysis();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function deleteRule(id: number) {
    if (!confirm('Delete this rule?')) return;
    try {
      await rulesApi.delete(projectId, id);
      await load();
      invalidateAnalysis();
    } catch (e: any) {
      alert(e.message);
    }
  }

  $effect(() => {
    projectId;
    load();
    loadSources();
  });
</script>

<div class="header-row">
  <h2>Rules</h2>
  <button class="primary" onclick={() => (showCreate = !showCreate)}>
    {showCreate ? 'Cancel' : 'New Rule'}
  </button>
</div>

{#if showCreate}
  <div class="create-form card">
    <h3>New Rule</h3>
    <div class="form-fields">
      <div class="field">
        <label>Name</label>
        <input type="text" bind:value={newName} placeholder="Rule name..." />
      </div>
      <div class="field">
        <label>Match Mode</label>
        <select bind:value={newMatchMode}>
          <option value="Any">Any</option>
          <option value="All">All</option>
        </select>
      </div>
      <div class="field">
        <label>Match Pattern (regex)</label>
        <textarea rows="2" bind:value={newMatchPattern} placeholder="e\.g\. ERROR.*timeout"
        ></textarea>
      </div>
    </div>

    <div class="section-header">
      <h4>Extraction Rules</h4>
      <button class="small" onclick={addNewExtractionRule}>+ Add</button>
    </div>

    {#each newExtractionRules as er, i}
      <div class="new-extraction-row">
        <div class="field">
          <label>Key</label>
          <input type="text" bind:value={er.state_key} placeholder="state_key" />
        </div>
        <div class="field">
          <label>Type</label>
          <select bind:value={er.extraction_type}>
            <option value="Parsed">Parsed</option>
            <option value="Static">Static</option>
            <option value="Clear">Clear</option>
          </select>
        </div>
        <div class="field">
          <label>Mode</label>
          <select bind:value={er.mode}>
            <option value="Replace">Replace</option>
            <option value="Accumulate">Accumulate</option>
          </select>
        </div>
        {#if er.extraction_type === 'Parsed'}
          <div class="field" style="flex:2">
            <label>Pattern</label>
            <input type="text" bind:value={er.pattern} placeholder="e.g. ERROR (?P<message>.+)" />
          </div>
        {/if}
        {#if er.extraction_type === 'Static'}
          <div class="field" style="flex:2">
            <label>Value</label>
            <input type="text" bind:value={er.static_value} placeholder="static value..." />
          </div>
        {/if}
        <button class="small danger remove-extraction" onclick={() => removeNewExtractionRule(i)}
          >x</button
        >
      </div>
    {/each}

    <div class="dry-run-section">
      <h4>Test Against Source</h4>
      {#if availableSources.length === 0}
        <div class="hint">No sources available</div>
      {:else}
        <div class="dry-run-controls">
          <select bind:value={newSelectedSourceId}>
            <option value="">Select a source...</option>
            {#each availableSources as src}
              <option value={src.id}>{src.name}</option>
            {/each}
          </select>
          <button
            class="small"
            onclick={runNewDryRun}
            disabled={newDryRunLoading || newSelectedSourceId === '' || !newMatchPattern.trim()}
          >
            {newDryRunLoading ? 'Running...' : 'Run'}
          </button>
        </div>
        {#if newDryRunError}
          <div class="hint error">{newDryRunError}</div>
        {/if}
        {#if newDryRunResults.length > 0}
          <div class="dry-run-results">
            <div class="hint">
              {newDryRunResults.length} match{newDryRunResults.length === 1 ? '' : 'es'} found
            </div>
            {#each newDryRunResults as rm}
              <div class="dry-run-match">
                <code>{rm.log_line.raw}</code>
                {#if Object.keys(rm.extracted_state).length > 0}
                  <div class="extraction-chips">
                    {#each Object.entries(rm.extracted_state) as [key, value]}
                      <span class="chip"
                        >{key}: {formatStateValue(value as Record<string, unknown>)}</span
                      >
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>

    <div class="actions">
      <button
        class="primary"
        onclick={createRule}
        disabled={!newName.trim() || !newMatchPattern.trim()}
      >
        Create
      </button>
    </div>
  </div>
{/if}

{#if loading}
  <div class="empty">Loading...</div>
{:else if ruleList.length === 0}
  <div class="guidance">
    <strong>Rules</strong> define what to look for in your logs — a regex pattern that extracts
    state when matched. Create one with the form above, or run an analysis first and
    <strong>select text in the log viewer</strong> to generate a rule from a real log line.
  </div>
{:else}
  <div class="rule-list">
    {#each ruleList as rule}
      <div class="rule-card card">
        <div
          class="rule-header"
          onclick={() => (expandedId = expandedId === rule.id ? null : rule.id)}
          onkeydown={(e) =>
            e.key === 'Enter' && (expandedId = expandedId === rule.id ? null : rule.id)}
          role="button"
          tabindex="0"
        >
          <div class="rule-info">
            <span class="rule-name">{rule.name}</span>
            <span class="badge">{rule.match_mode}</span>
            <span class="badge"
              >{rule.match_rules.length} match{rule.match_rules.length !== 1 ? 'es' : ''}</span
            >
            <span class="badge"
              >{rule.extraction_rules.length} extraction{rule.extraction_rules.length !== 1
                ? 's'
                : ''}</span
            >
          </div>
          <div class="rule-actions">
            <button
              onclick={(e) => {
                e.stopPropagation();
                editingId = rule.id;
                expandedId = rule.id;
              }}>Edit</button
            >
            <button
              class="danger"
              onclick={(e) => {
                e.stopPropagation();
                deleteRule(rule.id);
              }}>Delete</button
            >
          </div>
        </div>

        {#if expandedId === rule.id}
          {#if editingId === rule.id}
            <RuleEditor
              rule={JSON.parse(JSON.stringify(rule))}
              {projectId}
              onSave={() => {
                editingId = null;
                load();
              }}
              onCancel={() => {
                editingId = null;
              }}
            />
          {:else}
            <div class="rule-details">
              <div class="detail-section">
                <h3>Match Rules</h3>
                {#each rule.match_rules as mr}
                  <code class="pattern">{mr.pattern}</code>
                {/each}
              </div>
              {#if rule.extraction_rules.length > 0}
                <div class="detail-section">
                  <h3>Extraction Rules</h3>
                  {#each rule.extraction_rules as er}
                    <div class="extraction-row">
                      <span class="state-key">{er.state_key}</span>
                      <span class="badge">{er.extraction_type}</span>
                      <span class="badge">{er.mode}</span>
                      {#if er.pattern}
                        <code class="pattern">{er.pattern}</code>
                      {/if}
                      {#if er.static_value}
                        <span class="static-val">= {er.static_value}</span>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .header-row h2 {
    margin: 0;
  }

  .create-form {
    margin-bottom: 20px;
  }

  .form-fields {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .rule-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .rule-card {
    padding: 0;
    overflow: hidden;
  }

  .rule-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    cursor: pointer;
  }

  .rule-header:hover {
    background: var(--bg-hover);
  }

  .rule-info {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .rule-name {
    font-weight: 600;
  }

  .rule-actions {
    display: flex;
    gap: 8px;
  }

  .rule-details {
    padding: 0 16px 16px;
    border-top: 1px solid var(--border);
  }

  .detail-section {
    margin-top: 12px;
  }

  .detail-section h3 {
    margin-bottom: 8px;
  }

  .pattern {
    display: block;
    font-family: var(--font-mono);
    font-size: 12px;
    background: var(--bg);
    padding: 6px 10px;
    border-radius: var(--radius);
    color: var(--yellow);
    margin-bottom: 4px;
  }

  .extraction-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    border-bottom: 1px solid var(--border);
    font-size: 13px;
  }

  .state-key {
    font-family: var(--font-mono);
    color: var(--cyan);
    font-weight: 600;
  }

  .static-val {
    font-family: var(--font-mono);
    color: var(--green);
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 12px;
  }

  .section-header h4 {
    margin: 0;
  }

  .new-extraction-row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 8px;
    background: var(--bg);
    border-radius: var(--radius);
    margin-top: 4px;
  }

  .new-extraction-row .field {
    flex: 1;
  }

  .remove-extraction {
    margin-bottom: 2px;
  }

  .dry-run-section {
    margin-top: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .dry-run-section h4 {
    margin: 0;
  }

  .dry-run-controls {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .dry-run-controls select {
    flex: 1;
  }

  .dry-run-results {
    max-height: 200px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .dry-run-match {
    padding: 6px 8px;
    background: var(--bg);
    border-radius: var(--radius);
  }

  .dry-run-match code {
    display: block;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .extraction-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
  }

  .chip {
    display: inline-block;
    padding: 1px 6px;
    background: var(--green);
    color: var(--bg);
    border-radius: 4px;
    font-size: 11px;
    font-family: var(--font-mono);
  }

  .hint {
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }

  .hint.error {
    color: var(--yellow);
  }

  button.small {
    padding: 2px 8px;
    font-size: 12px;
  }
</style>
