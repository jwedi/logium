<script lang="ts">
  import { rules as rulesApi } from './api';

  let { projectId, selectedText, onClose, onCreated }: {
    projectId: number;
    selectedText: string;
    onClose: () => void;
    onCreated: () => void;
  } = $props();

  let ruleName = $state('');
  let regexPattern = $state('');
  let matchMode: 'Any' | 'All' = $state('Any');
  let captureGroups: { name: string; extractionType: 'Parsed' | 'Static' | 'Clear'; mode: 'Replace' | 'Accumulate' }[] = $state([]);
  let previewResult = $state('');
  let saving = $state(false);

  // Auto-generate regex from selected text
  function escapeRegex(text: string): string {
    return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  function generateInitialRegex(text: string): string {
    // Escape the text and replace common variable parts with capture groups
    let pattern = escapeRegex(text);
    // Replace numbers with a named capture group placeholder
    pattern = pattern.replace(/\d+/g, '(\\d+)');
    return pattern;
  }

  function detectGroups(pattern: string): string[] {
    const groups: string[] = [];
    const namedGroupRegex = /\((?:\?<([^>]+)>)?/g;
    let match;
    let idx = 0;
    while ((match = namedGroupRegex.exec(pattern)) !== null) {
      if (match[1]) {
        groups.push(match[1]);
      } else {
        groups.push(`group_${idx}`);
      }
      idx++;
    }
    return groups;
  }

  function updatePreview() {
    try {
      const re = new RegExp(regexPattern);
      const m = re.exec(selectedText);
      if (m) {
        previewResult = `Match: "${m[0]}"`;
        if (m.length > 1) {
          previewResult += ' | Groups: ' + m.slice(1).map((g, i) => `${i}: "${g}"`).join(', ');
        }
      } else {
        previewResult = 'No match on selected text';
      }
    } catch (e: any) {
      previewResult = `Invalid regex: ${e.message}`;
    }
  }

  $effect(() => {
    regexPattern = generateInitialRegex(selectedText);
  });

  $effect(() => {
    regexPattern;
    const groups = detectGroups(regexPattern);
    captureGroups = groups.map(name => ({
      name,
      extractionType: 'Parsed' as const,
      mode: 'Replace' as const,
    }));
    updatePreview();
  });

  async function save() {
    if (!ruleName.trim() || !regexPattern.trim()) return;
    saving = true;
    try {
      await rulesApi.create(projectId, {
        name: ruleName.trim(),
        match_mode: matchMode,
        match_rules: [{ id: 0, pattern: regexPattern }],
        extraction_rules: captureGroups.map((cg, i) => ({
          id: 0,
          extraction_type: cg.extractionType,
          state_key: cg.name,
          pattern: cg.extractionType === 'Parsed' ? regexPattern : null,
          static_value: null,
          mode: cg.mode,
        })),
      });
      onCreated();
    } catch (e: any) {
      alert(e.message);
    } finally {
      saving = false;
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-overlay" onclick={onClose} onkeydown={(e) => e.key === 'Escape' && onClose()}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <div class="modal-header">
      <h2>Create Rule from Selection</h2>
      <button class="close-btn" onclick={onClose}>x</button>
    </div>

    <div class="modal-body">
      <div class="selected-text-preview">
        <label>Selected Text</label>
        <code>{selectedText}</code>
      </div>

      <div class="field">
        <label>Rule Name</label>
        <input type="text" bind:value={ruleName} placeholder="My rule..." />
      </div>

      <div class="field">
        <label>Match Mode</label>
        <select bind:value={matchMode}>
          <option value="Any">Any</option>
          <option value="All">All</option>
        </select>
      </div>

      <div class="field">
        <label>Regex Pattern</label>
        <textarea rows="3" bind:value={regexPattern} oninput={updatePreview}></textarea>
        <div class="preview" class:error={previewResult.startsWith('Invalid') || previewResult.startsWith('No match')}>
          {previewResult}
        </div>
      </div>

      {#if captureGroups.length > 0}
        <div class="capture-groups">
          <label>Capture Groups (Extraction Rules)</label>
          {#each captureGroups as group, i}
            <div class="group-row">
              <div class="field">
                <label>State Key</label>
                <input type="text" bind:value={group.name} />
              </div>
              <div class="field">
                <label>Type</label>
                <select bind:value={group.extractionType}>
                  <option value="Parsed">Parsed</option>
                  <option value="Static">Static</option>
                  <option value="Clear">Clear</option>
                </select>
              </div>
              <div class="field">
                <label>Mode</label>
                <select bind:value={group.mode}>
                  <option value="Replace">Replace</option>
                  <option value="Accumulate">Accumulate</option>
                </select>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="modal-footer">
      <button onclick={onClose}>Cancel</button>
      <button class="primary" onclick={save} disabled={saving || !ruleName.trim() || !regexPattern.trim()}>
        {saving ? 'Saving...' : 'Create Rule'}
      </button>
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: 600px;
    max-width: 90vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 16px;
  }

  .close-btn {
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 18px;
    padding: 4px 8px;
  }

  .modal-body {
    padding: 20px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .selected-text-preview code {
    display: block;
    margin-top: 4px;
    padding: 8px 12px;
    background: var(--bg);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
    color: var(--yellow);
  }

  .preview {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--green);
    margin-top: 4px;
  }

  .preview.error {
    color: var(--red);
  }

  .capture-groups {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .group-row {
    display: flex;
    gap: 8px;
    padding: 8px;
    background: var(--bg);
    border-radius: var(--radius);
  }

  .group-row .field {
    flex: 1;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 16px 20px;
    border-top: 1px solid var(--border);
  }
</style>
