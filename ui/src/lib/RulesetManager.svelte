<script lang="ts">
  import {
    rulesets as rulesetsApi,
    rules as rulesApi,
    templates as templatesApi,
    type Ruleset,
    type LogRule,
    type SourceTemplate,
  } from './api';

  let { projectId }: { projectId: number } = $props();

  let rulesetList: Ruleset[] = $state([]);
  let ruleList: LogRule[] = $state([]);
  let templateList: SourceTemplate[] = $state([]);
  let loading = $state(false);
  let editing: Ruleset | null = $state(null);

  let newName = $state('');
  let newTemplateId = $state<number | ''>('');
  let newRuleIds: number[] = $state([]);

  async function load() {
    loading = true;
    try {
      [rulesetList, ruleList, templateList] = await Promise.all([
        rulesetsApi.list(projectId),
        rulesApi.list(projectId),
        templatesApi.list(projectId),
      ]);
    } catch (e: any) {
      alert(e.message);
    } finally {
      loading = false;
    }
  }

  function toggleRule(ruleId: number, list: number[]): number[] {
    return list.includes(ruleId) ? list.filter((id) => id !== ruleId) : [...list, ruleId];
  }

  async function createRuleset() {
    if (!newName.trim() || !newTemplateId) return;
    try {
      await rulesetsApi.create(projectId, {
        name: newName.trim(),
        template_id: Number(newTemplateId),
        rule_ids: newRuleIds,
      });
      newName = '';
      newTemplateId = '';
      newRuleIds = [];
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function updateRuleset() {
    if (!editing) return;
    try {
      await rulesetsApi.update(projectId, editing.id, {
        name: editing.name,
        template_id: editing.template_id,
        rule_ids: editing.rule_ids,
      });
      editing = null;
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function deleteRuleset(id: number) {
    if (!confirm('Delete this ruleset?')) return;
    try {
      await rulesetsApi.delete(projectId, id);
      if (editing?.id === id) editing = null;
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  function getTemplateName(id: number): string {
    return templateList.find((t) => t.id === id)?.name ?? `Template #${id}`;
  }

  function getRuleName(id: number): string {
    return ruleList.find((r) => r.id === id)?.name ?? `Rule #${id}`;
  }

  $effect(() => {
    projectId;
    load();
  });
</script>

<h2>Rulesets</h2>

<div class="create-form card">
  <h3>New Ruleset</h3>
  <div class="form-fields">
    <div class="row">
      <div class="field" style="flex:1">
        <label>Name</label>
        <input type="text" bind:value={newName} placeholder="Ruleset name..." />
      </div>
      <div class="field" style="flex:1">
        <label>Template</label>
        <select bind:value={newTemplateId}>
          <option value="">Select template...</option>
          {#each templateList as tmpl}
            <option value={tmpl.id}>{tmpl.name}</option>
          {/each}
        </select>
      </div>
    </div>
    {#if ruleList.length > 0}
      <div class="field">
        <label>Rules</label>
        <div class="rule-checkboxes">
          {#each ruleList as rule}
            <label class="checkbox-label">
              <input
                type="checkbox"
                checked={newRuleIds.includes(rule.id)}
                onchange={() => (newRuleIds = toggleRule(rule.id, newRuleIds))}
              />
              {rule.name}
            </label>
          {/each}
        </div>
      </div>
    {/if}
  </div>
  <div class="actions">
    <button class="primary" onclick={createRuleset} disabled={!newName.trim() || !newTemplateId}>
      Create Ruleset
    </button>
  </div>
</div>

{#if loading}
  <div class="empty">Loading...</div>
{:else if rulesetList.length === 0}
  <div class="empty">No rulesets yet.</div>
{:else}
  <div class="ruleset-list">
    {#each rulesetList as rs}
      <div class="ruleset-card card">
        {#if editing?.id === rs.id}
          <div class="form-fields">
            <div class="row">
              <div class="field" style="flex:1">
                <label>Name</label>
                <input type="text" bind:value={editing.name} />
              </div>
              <div class="field" style="flex:1">
                <label>Template</label>
                <select bind:value={editing.template_id}>
                  {#each templateList as tmpl}
                    <option value={tmpl.id}>{tmpl.name}</option>
                  {/each}
                </select>
              </div>
            </div>
            <div class="field">
              <label>Rules</label>
              <div class="rule-checkboxes">
                {#each ruleList as rule}
                  <label class="checkbox-label">
                    <input
                      type="checkbox"
                      checked={editing.rule_ids.includes(rule.id)}
                      onchange={() => {
                        if (editing) editing.rule_ids = toggleRule(rule.id, editing.rule_ids);
                      }}
                    />
                    {rule.name}
                  </label>
                {/each}
              </div>
            </div>
          </div>
          <div class="actions">
            <button class="primary" onclick={updateRuleset}>Save</button>
            <button onclick={() => (editing = null)}>Cancel</button>
          </div>
        {:else}
          <div class="ruleset-info">
            <div class="ruleset-header-row">
              <span class="ruleset-name">{rs.name}</span>
              <span class="badge">{getTemplateName(rs.template_id)}</span>
            </div>
            <div class="ruleset-rules">
              {#if rs.rule_ids.length === 0}
                <span class="text-muted">No rules assigned</span>
              {:else}
                {#each rs.rule_ids as ruleId}
                  <span class="badge">{getRuleName(ruleId)}</span>
                {/each}
              {/if}
            </div>
          </div>
          <div class="ruleset-actions">
            <button onclick={() => (editing = { ...rs, rule_ids: [...rs.rule_ids] })}>Edit</button>
            <button class="danger" onclick={() => deleteRuleset(rs.id)}>Delete</button>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .create-form {
    margin-bottom: 20px;
  }

  .form-fields {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .rule-checkboxes {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 8px;
    background: var(--bg);
    border-radius: var(--radius);
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 13px;
    color: var(--text);
    text-transform: none;
    letter-spacing: 0;
    cursor: pointer;
  }

  .checkbox-label input {
    width: auto;
    padding: 0;
  }

  .ruleset-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .ruleset-card {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
  }

  .ruleset-info {
    flex: 1;
  }

  .ruleset-header-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .ruleset-name {
    font-weight: 600;
    font-size: 15px;
  }

  .ruleset-rules {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .text-muted {
    color: var(--text-muted);
    font-size: 12px;
  }

  .ruleset-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }
</style>
