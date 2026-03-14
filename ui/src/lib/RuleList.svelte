<script lang="ts">
  import { rules as rulesApi, type LogRule } from './api';
  import { invalidateAnalysis } from './analysisInvalidation.svelte';
  import RuleEditor from './RuleEditor.svelte';

  let { projectId }: { projectId: number } = $props();

  let ruleList: LogRule[] = $state([]);
  let loading = $state(false);
  let expandedId: number | null = $state(null);
  let editingId: number | null = $state(null);
  let searchQuery = $state('');
  let expandAll = $state(false);

  let showCreate = $state(false);

  let allStateKeys = $derived.by(() => {
    const keys = new Set<string>();
    for (const r of ruleList) {
      for (const er of r.extraction_rules) {
        if (er.state_key.trim()) keys.add(er.state_key.trim());
      }
    }
    return [...keys].sort();
  });

  let isSearchActive = $derived(searchQuery.trim() !== '');

  let filteredRules = $derived.by(() => {
    if (!isSearchActive) return ruleList;
    const q = searchQuery.trim().toLowerCase();
    return ruleList.filter((rule) => {
      if (rule.name.toLowerCase().includes(q)) return true;
      if (rule.match_rules.some((mr) => mr.pattern.toLowerCase().includes(q))) return true;
      if (
        rule.extraction_rules.some(
          (er) =>
            er.state_key.toLowerCase().includes(q) || (er.pattern ?? '').toLowerCase().includes(q),
        )
      )
        return true;
      return false;
    });
  });

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

{#if ruleList.length > 0}
  <div class="search-bar">
    <input
      type="search"
      bind:value={searchQuery}
      placeholder="Search rules..."
      class="rule-search"
    />
    <label class="expand-all-label">
      <input type="checkbox" bind:checked={expandAll} />
      Expand all
    </label>
  </div>
{/if}

{#if showCreate}
  <RuleEditor
    {projectId}
    knownKeys={allStateKeys}
    onSave={() => {
      showCreate = false;
      load();
    }}
    onCancel={() => (showCreate = false)}
  />
{/if}

{#if loading}
  <div class="empty">Loading...</div>
{:else if ruleList.length === 0}
  <div class="guidance">
    <strong>Rules</strong> define what to look for in your logs — a regex pattern that extracts
    state when matched. Create one with the form above, or run an analysis first and
    <strong>select text in the log viewer</strong> to generate a rule from a real log line.
  </div>
{:else if filteredRules.length === 0}
  <div class="empty">No rules match "{searchQuery}".</div>
{:else}
  <div class="rule-list">
    {#each filteredRules as rule}
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

        {#if expandedId === rule.id || expandAll || isSearchActive}
          {#if editingId === rule.id}
            <RuleEditor
              rule={JSON.parse(JSON.stringify(rule))}
              {projectId}
              knownKeys={allStateKeys}
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

  button.small {
    padding: 2px 8px;
    font-size: 12px;
  }

  .search-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }

  .rule-search {
    flex: 1;
    max-width: 320px;
  }

  .expand-all-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    cursor: pointer;
    white-space: nowrap;
  }
</style>
