<script lang="ts">
  import { rules as rulesApi, type LogRule } from './api';
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

  async function createRule() {
    if (!newName.trim() || !newMatchPattern.trim()) return;
    try {
      await rulesApi.create(projectId, {
        name: newName.trim(),
        match_mode: newMatchMode,
        match_rules: [{ id: 0, pattern: newMatchPattern }],
        extraction_rules: [],
      });
      newName = '';
      newMatchPattern = '';
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
</style>
