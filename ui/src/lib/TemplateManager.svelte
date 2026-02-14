<script lang="ts">
  import {
    templates as templatesApi,
    timestampTemplates as tsTemplatesApi,
    type SourceTemplate,
    type TimestampTemplate,
  } from './api';

  let { projectId }: { projectId: number } = $props();

  let templateList: SourceTemplate[] = $state([]);
  let tsTemplateList: TimestampTemplate[] = $state([]);
  let loading = $state(false);
  let editing: SourceTemplate | null = $state(null);

  let newName = $state('');
  let newTimestampTemplateId: number | null = $state(null);
  let newLineDelimiter = $state('\\n');
  let newContentRegex = $state('');
  let newContinuationRegex = $state('');
  let newJsonTimestampField = $state('');

  function tsTemplateName(id: number): string {
    return tsTemplateList.find((t) => t.id === id)?.name ?? `#${id}`;
  }

  async function load() {
    loading = true;
    try {
      [templateList, tsTemplateList] = await Promise.all([
        templatesApi.list(projectId),
        tsTemplatesApi.list(projectId),
      ]);
      if (tsTemplateList.length > 0 && newTimestampTemplateId == null) {
        newTimestampTemplateId = tsTemplateList[0].id;
      }
    } catch (e: any) {
      alert(e.message);
    } finally {
      loading = false;
    }
  }

  async function createTemplate() {
    if (!newName.trim() || newTimestampTemplateId == null) return;
    try {
      await templatesApi.create(projectId, {
        name: newName.trim(),
        timestamp_template_id: newTimestampTemplateId,
        line_delimiter: newLineDelimiter,
        content_regex: newContentRegex || null,
        continuation_regex: newContinuationRegex || null,
        json_timestamp_field: newJsonTimestampField || null,
      });
      newName = '';
      newLineDelimiter = '\\n';
      newContentRegex = '';
      newContinuationRegex = '';
      newJsonTimestampField = '';
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function updateTemplate() {
    if (!editing) return;
    try {
      await templatesApi.update(projectId, editing.id, editing);
      editing = null;
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function deleteTemplate(id: number) {
    if (!confirm('Delete this template?')) return;
    try {
      await templatesApi.delete(projectId, id);
      if (editing?.id === id) editing = null;
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  $effect(() => {
    projectId;
    load();
  });
</script>

<h2>Source Templates</h2>

<div class="create-form card">
  <h3>New Template</h3>
  <div class="form-grid">
    <div class="field">
      <label>Name</label>
      <input type="text" bind:value={newName} placeholder="Template name..." />
    </div>
    <div class="field">
      <label>Timestamp Template</label>
      {#if tsTemplateList.length === 0}
        <span class="hint">No timestamp templates available</span>
      {:else}
        <select bind:value={newTimestampTemplateId}>
          {#each tsTemplateList as ts}
            <option value={ts.id}>{ts.name} ({ts.format})</option>
          {/each}
        </select>
      {/if}
    </div>
    <div class="field">
      <label>Line Delimiter</label>
      <input type="text" bind:value={newLineDelimiter} />
    </div>
    <div class="field">
      <label>Content Regex (optional)</label>
      <input type="text" bind:value={newContentRegex} placeholder="Regex to extract content..." />
    </div>
    <div class="field">
      <label>Continuation Regex (optional)</label>
      <input
        type="text"
        bind:value={newContinuationRegex}
        placeholder="Regex for multi-line continuation..."
      />
    </div>
    <div class="field">
      <label>JSON Timestamp Field (optional)</label>
      <input
        type="text"
        bind:value={newJsonTimestampField}
        placeholder="e.g. timestamp, ts, @timestamp"
      />
    </div>
  </div>
  <div class="actions">
    <button
      class="primary"
      onclick={createTemplate}
      disabled={!newName.trim() || newTimestampTemplateId == null}>Create Template</button
    >
  </div>
</div>

{#if loading}
  <div class="empty">Loading...</div>
{:else if templateList.length === 0}
  <div class="guidance">
    <strong>Source templates</strong> describe how to parse a log format — timestamp pattern, line delimiter,
    and content regex. Fill in the form above to create one, or upload a log file in the Sources tab to
    auto-detect the format.
  </div>
{:else}
  <div class="template-list">
    {#each templateList as tmpl}
      <div class="template-card card">
        {#if editing?.id === tmpl.id}
          <div class="form-grid">
            <div class="field">
              <label>Name</label>
              <input type="text" bind:value={editing.name} />
            </div>
            <div class="field">
              <label>Timestamp Template</label>
              <select bind:value={editing.timestamp_template_id}>
                {#each tsTemplateList as ts}
                  <option value={ts.id}>{ts.name} ({ts.format})</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label>Line Delimiter</label>
              <input type="text" bind:value={editing.line_delimiter} />
            </div>
            <div class="field">
              <label>Content Regex</label>
              <input type="text" bind:value={editing.content_regex} />
            </div>
            <div class="field">
              <label>Continuation Regex</label>
              <input type="text" bind:value={editing.continuation_regex} />
            </div>
            <div class="field">
              <label>JSON Timestamp Field</label>
              <input
                type="text"
                bind:value={editing.json_timestamp_field}
                placeholder="e.g. timestamp, ts, @timestamp"
              />
            </div>
          </div>
          <div class="actions">
            <button class="primary" onclick={updateTemplate}>Save</button>
            <button onclick={() => (editing = null)}>Cancel</button>
          </div>
        {:else}
          <div class="template-info">
            <span class="template-name">{tmpl.name}</span>
            <div class="template-details">
              <span><strong>Timestamp:</strong> {tsTemplateName(tmpl.timestamp_template_id)}</span>
              <span><strong>Delimiter:</strong> {tmpl.line_delimiter}</span>
              {#if tmpl.content_regex}
                <span><strong>Regex:</strong> <code>{tmpl.content_regex}</code></span>
              {/if}
              {#if tmpl.continuation_regex}
                <span><strong>Continuation:</strong> <code>{tmpl.continuation_regex}</code></span>
              {/if}
              {#if tmpl.json_timestamp_field}
                <span
                  ><strong>JSON Timestamp Field:</strong>
                  <code>{tmpl.json_timestamp_field}</code></span
                >
              {/if}
            </div>
          </div>
          <div class="template-actions">
            <button onclick={() => (editing = { ...tmpl })}>Edit</button>
            <button class="danger" onclick={() => deleteTemplate(tmpl.id)}>Delete</button>
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

  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .template-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .template-card {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
  }

  .template-info {
    flex: 1;
  }

  .template-name {
    font-weight: 600;
    font-size: 15px;
  }

  .template-details {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 4px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .template-details code {
    font-family: var(--font-mono);
    font-size: 11px;
    background: var(--bg-tertiary);
    padding: 1px 4px;
    border-radius: 3px;
  }

  .template-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .hint {
    font-size: 12px;
    color: var(--text-dim);
    font-style: italic;
  }
</style>
