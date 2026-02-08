<script lang="ts">
  import { templates as templatesApi, type SourceTemplate } from './api';

  let { projectId }: { projectId: number } = $props();

  let templateList: SourceTemplate[] = $state([]);
  let loading = $state(false);
  let editing: SourceTemplate | null = $state(null);

  let newName = $state('');
  let newTimestampFormat = $state('%Y-%m-%d %H:%M:%S');
  let newLineDelimiter = $state('\\n');
  let newContentRegex = $state('');

  async function load() {
    loading = true;
    try {
      templateList = await templatesApi.list(projectId);
    } catch (e: any) {
      alert(e.message);
    } finally {
      loading = false;
    }
  }

  async function createTemplate() {
    if (!newName.trim()) return;
    try {
      await templatesApi.create(projectId, {
        name: newName.trim(),
        timestamp_format: newTimestampFormat,
        line_delimiter: newLineDelimiter,
        content_regex: newContentRegex || null,
      });
      newName = '';
      newTimestampFormat = '%Y-%m-%d %H:%M:%S';
      newLineDelimiter = '\\n';
      newContentRegex = '';
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
      <label>Timestamp Format</label>
      <input type="text" bind:value={newTimestampFormat} placeholder="%Y-%m-%d %H:%M:%S" />
    </div>
    <div class="field">
      <label>Line Delimiter</label>
      <input type="text" bind:value={newLineDelimiter} />
    </div>
    <div class="field">
      <label>Content Regex (optional)</label>
      <input type="text" bind:value={newContentRegex} placeholder="Regex to extract content..." />
    </div>
  </div>
  <div class="actions">
    <button class="primary" onclick={createTemplate} disabled={!newName.trim()}>Create Template</button>
  </div>
</div>

{#if loading}
  <div class="empty">Loading...</div>
{:else if templateList.length === 0}
  <div class="empty">No templates yet.</div>
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
              <label>Timestamp Format</label>
              <input type="text" bind:value={editing.timestamp_format} />
            </div>
            <div class="field">
              <label>Line Delimiter</label>
              <input type="text" bind:value={editing.line_delimiter} />
            </div>
            <div class="field">
              <label>Content Regex</label>
              <input type="text" bind:value={editing.content_regex} />
            </div>
          </div>
          <div class="actions">
            <button class="primary" onclick={updateTemplate}>Save</button>
            <button onclick={() => editing = null}>Cancel</button>
          </div>
        {:else}
          <div class="template-info">
            <span class="template-name">{tmpl.name}</span>
            <div class="template-details">
              <span><strong>Format:</strong> {tmpl.timestamp_format}</span>
              <span><strong>Delimiter:</strong> {tmpl.line_delimiter}</span>
              {#if tmpl.content_regex}
                <span><strong>Regex:</strong> <code>{tmpl.content_regex}</code></span>
              {/if}
            </div>
          </div>
          <div class="template-actions">
            <button onclick={() => editing = { ...tmpl }}>Edit</button>
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
</style>
