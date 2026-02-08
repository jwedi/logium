<script lang="ts">
  import { sources as sourcesApi, templates as templatesApi, type Source, type SourceTemplate } from './api';
  import LogViewer from './LogViewer.svelte';

  let { projectId }: { projectId: number } = $props();

  let sourceList: Source[] = $state([]);
  let templateList: SourceTemplate[] = $state([]);
  let selectedSource: Source | null = $state(null);
  let loading = $state(false);

  let newName = $state('');
  let newTemplateId = $state<number | ''>('');
  let fileInput: HTMLInputElement | undefined = $state();

  async function load() {
    loading = true;
    try {
      [sourceList, templateList] = await Promise.all([
        sourcesApi.list(projectId),
        templatesApi.list(projectId),
      ]);
    } catch (e: any) {
      alert(e.message);
    } finally {
      loading = false;
    }
  }

  async function createSource() {
    if (!newName.trim() || !newTemplateId) return;
    try {
      const source = await sourcesApi.create(projectId, {
        name: newName.trim(),
        template_id: Number(newTemplateId),
        file_path: '',
      });

      if (fileInput?.files?.[0]) {
        await sourcesApi.upload(projectId, source.id, fileInput.files[0]);
      }

      newName = '';
      newTemplateId = '';
      if (fileInput) fileInput.value = '';
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function deleteSource(id: number) {
    if (!confirm('Delete this source?')) return;
    try {
      await sourcesApi.delete(projectId, id);
      if (selectedSource?.id === id) selectedSource = null;
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

<h2>Sources</h2>

{#if selectedSource}
  <div class="viewer-header">
    <button onclick={() => selectedSource = null}>Back to list</button>
    <span class="source-title">{selectedSource.name}</span>
    <span class="badge">{selectedSource.file_path || 'no file'}</span>
  </div>
  <LogViewer source={selectedSource} {projectId} />
{:else}
  <div class="create-form card">
    <h3>Add Source</h3>
    <div class="form-row">
      <div class="field">
        <label>Name</label>
        <input type="text" bind:value={newName} placeholder="Source name..." />
      </div>
      <div class="field">
        <label>Template</label>
        <select bind:value={newTemplateId}>
          <option value="">Select template...</option>
          {#each templateList as tmpl}
            <option value={tmpl.id}>{tmpl.name}</option>
          {/each}
        </select>
      </div>
      <div class="field">
        <label>Log file</label>
        <input type="file" bind:this={fileInput} />
      </div>
      <button class="primary" onclick={createSource} disabled={!newName.trim() || !newTemplateId}>
        Add
      </button>
    </div>
  </div>

  {#if loading}
    <div class="empty">Loading...</div>
  {:else if sourceList.length === 0}
    <div class="empty">No sources yet. Add a template first, then create a source.</div>
  {:else}
    <div class="source-list">
      {#each sourceList as source}
        <div class="source-card card">
          <div class="source-info">
            <span class="source-name">{source.name}</span>
            <span class="source-meta">
              Template #{source.template_id} &middot; {source.file_path || 'no file'}
            </span>
          </div>
          <div class="source-actions">
            <button onclick={() => selectedSource = source}>View logs</button>
            <button class="danger" onclick={() => deleteSource(source.id)}>Delete</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .viewer-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }

  .source-title {
    font-weight: 600;
    font-size: 16px;
  }

  .create-form {
    margin-bottom: 20px;
  }

  .form-row {
    display: flex;
    gap: 12px;
    align-items: flex-end;
    flex-wrap: wrap;
  }

  .form-row .field {
    flex: 1;
    min-width: 150px;
  }

  .source-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .source-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .source-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .source-name {
    font-weight: 600;
  }

  .source-meta {
    font-size: 12px;
    color: var(--text-muted);
  }

  .source-actions {
    display: flex;
    gap: 8px;
  }
</style>
