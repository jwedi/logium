<script lang="ts">
  import {
    projects as projectsApi,
    type ImportPreview,
    type EntitySelection,
    type ImportResult,
  } from './api';

  let {
    exportData,
    preview,
    projectId,
    onClose,
    onImported,
  }: {
    exportData: unknown;
    preview: ImportPreview;
    projectId: number;
    onClose: () => void;
    onImported: (result: ImportResult) => void;
  } = $props();

  type ItemState = {
    exportId: number;
    name: string;
    existingId: number | null;
    include: boolean;
    conflictAction: 'overwrite' | 'add';
  };

  function initItems(
    items: { export_id: number; name: string; existing_id: number | null }[],
  ): ItemState[] {
    return items.map((item) => ({
      exportId: item.export_id,
      name: item.name,
      existingId: item.existing_id,
      include: true,
      conflictAction: 'overwrite',
    }));
  }

  let tts = $state(initItems(preview.timestamp_templates));
  let sts = $state(initItems(preview.source_templates));
  let rules = $state(initItems(preview.rules));
  let rulesets = $state(initItems(preview.rulesets));
  let patterns = $state(initItems(preview.patterns));

  let importing = $state(false);

  let allSkipped = $derived(
    [...tts, ...sts, ...rules, ...rulesets, ...patterns].every((s) => !s.include),
  );

  function toSelection(items: ItemState[]): EntitySelection[] {
    return items.map((s) => ({
      export_id: s.exportId,
      action: !s.include
        ? 'skip'
        : s.existingId != null && s.conflictAction === 'overwrite'
          ? 'overwrite'
          : 'add',
      existing_id: s.existingId ?? undefined,
    }));
  }

  async function doImport() {
    importing = true;
    try {
      const result = await projectsApi.importSelective(projectId, {
        export: exportData,
        selections: {
          timestamp_templates: toSelection(tts),
          source_templates: toSelection(sts),
          rules: toSelection(rules),
          rulesets: toSelection(rulesets),
          patterns: toSelection(patterns),
        },
      });
      onImported(result);
    } catch (e: any) {
      alert(e.message);
    } finally {
      importing = false;
    }
  }
</script>

<div class="overlay" role="button" tabindex="-1" onclick={onClose} onkeydown={() => {}}></div>
<div class="modal">
  <div class="modal-header">
    <h3>Import Preview</h3>
    <button class="close-btn" onclick={onClose}>×</button>
  </div>
  <div class="modal-body">
    {#snippet section(title: string, items: ItemState[])}
      {#if items.length > 0}
        <div class="section">
          <h4>{title} ({items.length})</h4>
          {#each items as item}
            <div class="item-row">
              <label class="item-label">
                <input type="checkbox" bind:checked={item.include} />
                <span class="item-name">{item.name}</span>
              </label>
              {#if item.existingId != null}
                <span class="conflict-badge">conflict</span>
                {#if item.include}
                  <select bind:value={item.conflictAction}>
                    <option value="overwrite">Overwrite existing</option>
                    <option value="add">Add as duplicate</option>
                  </select>
                {/if}
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/snippet}
    {@render section('Timestamp Templates', tts)}
    {@render section('Source Templates', sts)}
    {@render section('Rules', rules)}
    {@render section('Rulesets', rulesets)}
    {@render section('Patterns', patterns)}
  </div>
  <div class="modal-footer">
    <button onclick={onClose}>Cancel</button>
    <button class="primary" onclick={doImport} disabled={allSkipped || importing}>
      {importing ? 'Importing...' : 'Import'}
    </button>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 100;
  }

  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 101;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: 520px;
    max-width: 95vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h3 {
    margin: 0;
    font-size: 16px;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 20px;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0 4px;
    line-height: 1;
  }

  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .section h4 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .item-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
  }

  .item-label {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    flex: 1;
  }

  .item-name {
    font-size: 14px;
  }

  .conflict-badge {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--color-warning, #f59e0b);
    color: #fff;
    font-weight: 600;
    white-space: nowrap;
  }

  .item-row select {
    font-size: 12px;
    padding: 2px 4px;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid var(--border);
  }
</style>
