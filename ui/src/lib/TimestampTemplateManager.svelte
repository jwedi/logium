<script lang="ts">
  import { timestampTemplates as tsTemplatesApi, type TimestampTemplate } from './api';
  import { validateRegex } from './regexUtils';

  let { projectId }: { projectId: number } = $props();

  let templateList: TimestampTemplate[] = $state([]);
  let loading = $state(false);
  let editing: TimestampTemplate | null = $state(null);

  let newName = $state('');
  let newFormat = $state('');
  let newExtractionRegex = $state('');
  let newDefaultYear = $state('');

  let newExtractionRegexError = $derived(validateRegex(newExtractionRegex));
  let editExtractionRegexError = $derived.by(() => {
    const e = editing;
    return e ? validateRegex(e.extraction_regex ?? '') : null;
  });

  async function load() {
    loading = true;
    try {
      templateList = await tsTemplatesApi.list(projectId);
    } catch (e: any) {
      alert(e.message);
    } finally {
      loading = false;
    }
  }

  async function create() {
    if (!newName.trim() || !newFormat.trim()) return;
    try {
      await tsTemplatesApi.create(projectId, {
        name: newName.trim(),
        format: newFormat.trim(),
        extraction_regex: newExtractionRegex || null,
        default_year: newDefaultYear ? Number(newDefaultYear) : null,
      });
      newName = '';
      newFormat = '';
      newExtractionRegex = '';
      newDefaultYear = '';
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function update() {
    if (!editing) return;
    try {
      await tsTemplatesApi.update(projectId, editing.id, {
        name: editing.name,
        format: editing.format,
        extraction_regex: editing.extraction_regex,
        default_year: editing.default_year,
      });
      editing = null;
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function remove(id: number) {
    if (!confirm('Delete this timestamp template? Source templates using it will be affected.'))
      return;
    try {
      await tsTemplatesApi.delete(projectId, id);
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

<h2>Timestamp Templates</h2>

<div class="create-form card">
  <h3>New Timestamp Template</h3>
  <div class="form-grid">
    <div class="field">
      <label>Name</label>
      <input type="text" bind:value={newName} placeholder="e.g. ISO 8601, Syslog..." />
    </div>
    <div class="field">
      <label>Format (strftime)</label>
      <input type="text" bind:value={newFormat} placeholder="e.g. %Y-%m-%dT%H:%M:%S" />
    </div>
    <div class="field">
      <label>Extraction Regex (optional)</label>
      <input
        type="text"
        bind:value={newExtractionRegex}
        placeholder="Regex to extract timestamp from line..."
      />
      {#if newExtractionRegexError}<span class="field-error">{newExtractionRegexError}</span>{/if}
    </div>
    <div class="field">
      <label>Default Year (optional)</label>
      <input
        type="text"
        bind:value={newDefaultYear}
        placeholder="e.g. 2026 (for yearless formats)"
      />
    </div>
  </div>
  <div class="actions">
    <button
      class="primary"
      onclick={create}
      disabled={!newName.trim() || !newFormat.trim() || !!newExtractionRegexError}>Create</button
    >
  </div>
</div>

{#if loading}
  <div class="empty">Loading...</div>
{:else if templateList.length === 0}
  <div class="guidance">
    <strong>Timestamp templates</strong> define how to parse timestamps from log lines — the strftime
    format string, an optional extraction regex for mid-line timestamps (e.g. nginx), and an optional
    default year for formats without one (e.g. syslog). Default templates are created automatically for
    new projects.
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
              <label>Format (strftime)</label>
              <input type="text" bind:value={editing.format} />
            </div>
            <div class="field">
              <label>Extraction Regex</label>
              <input type="text" bind:value={editing.extraction_regex} />
              {#if editExtractionRegexError}<span class="field-error"
                  >{editExtractionRegexError}</span
                >{/if}
            </div>
            <div class="field">
              <label>Default Year</label>
              <input type="number" bind:value={editing.default_year} placeholder="e.g. 2026" />
            </div>
          </div>
          <div class="actions">
            <button class="primary" onclick={update} disabled={!!editExtractionRegexError}
              >Save</button
            >
            <button onclick={() => (editing = null)}>Cancel</button>
          </div>
        {:else}
          <div class="template-info">
            <span class="template-name">{tmpl.name}</span>
            <div class="template-details">
              <span><strong>Format:</strong> <code>{tmpl.format}</code></span>
              {#if tmpl.extraction_regex}
                <span><strong>Extraction Regex:</strong> <code>{tmpl.extraction_regex}</code></span>
              {/if}
              {#if tmpl.default_year}
                <span><strong>Default Year:</strong> {tmpl.default_year}</span>
              {/if}
            </div>
          </div>
          <div class="template-actions">
            <button onclick={() => (editing = { ...tmpl })}>Edit</button>
            <button class="danger" onclick={() => remove(tmpl.id)}>Delete</button>
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

  .field-error {
    font-size: 11px;
    color: var(--red);
    margin-top: 2px;
    font-family: var(--font-mono);
  }
</style>
