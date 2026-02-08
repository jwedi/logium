<script lang="ts">
  import {
    rules as rulesApi,
    rulesets as rulesetsApi,
    analysis as analysisApi,
    type Ruleset,
    type SuggestRuleResponse,
  } from './api';
  import { invalidateAnalysis } from './analysisInvalidation.svelte';
  import { detectGroups, testPattern } from './regexUtils';

  let {
    projectId,
    selectedText,
    sourceTemplateId,
    onClose,
    onCreated,
  }: {
    projectId: number;
    selectedText: string;
    sourceTemplateId: number;
    onClose: () => void;
    onCreated: () => void;
  } = $props();

  let ruleName = $state('');
  let regexPattern = $state('');
  let matchMode: 'Any' | 'All' = $state('Any');
  let captureGroups: {
    name: string;
    extractionType: 'Parsed' | 'Static' | 'Clear';
    mode: 'Replace' | 'Accumulate';
  }[] = $state([]);
  let saving = $state(false);
  let suggestLoading = $state(false);
  let suggestError = $state('');
  let availableRulesets: Ruleset[] = $state([]);
  let selectedRulesetId: number | '' = $state('');

  function escapeRegex(text: string): string {
    return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  let previewResult = $derived.by(() => {
    const result = testPattern(regexPattern, selectedText);
    if (result.status === 'match') {
      let text = result.message;
      const groupEntries = Object.entries(result.groups);
      if (groupEntries.length > 0) {
        text += ' | Groups: ' + groupEntries.map(([k, v], i) => `${i}: "${v}"`).join(', ');
      }
      return text;
    }
    if (result.status === 'error') return result.message;
    return 'No match on selected text';
  });

  async function fetchSuggestion() {
    suggestLoading = true;
    suggestError = '';
    try {
      const suggestion = await analysisApi.suggestRule(projectId, { text: selectedText });
      regexPattern = suggestion.pattern;
    } catch (e: any) {
      suggestError = e.message;
      // Fallback: escape text and replace numbers with capture groups
      let pattern = escapeRegex(selectedText);
      pattern = pattern.replace(/\d+/g, '(\\d+)');
      regexPattern = pattern;
    } finally {
      suggestLoading = false;
    }
  }

  async function loadRulesets() {
    try {
      const all = await rulesetsApi.list(projectId);
      availableRulesets = all.filter((rs) => rs.template_id === sourceTemplateId);
      if (availableRulesets.length === 1) {
        selectedRulesetId = availableRulesets[0].id;
      }
    } catch {
      /* ignore */
    }
  }

  $effect(() => {
    selectedText;
    fetchSuggestion();
  });

  $effect(() => {
    sourceTemplateId;
    loadRulesets();
  });

  $effect(() => {
    regexPattern;
    const groups = detectGroups(regexPattern);
    captureGroups = groups.map((name) => ({
      name,
      extractionType: 'Parsed' as const,
      mode: 'Replace' as const,
    }));
  });

  async function save() {
    if (!ruleName.trim() || !regexPattern.trim()) return;
    saving = true;
    try {
      const createdRule = await rulesApi.create(projectId, {
        name: ruleName.trim(),
        match_mode: matchMode,
        match_rules: [{ id: 0, pattern: regexPattern }],
        extraction_rules: captureGroups.map((cg) => ({
          id: 0,
          extraction_type: cg.extractionType,
          state_key: cg.name,
          pattern: cg.extractionType === 'Parsed' ? regexPattern : null,
          static_value: null,
          mode: cg.mode,
        })),
      });

      // Assign rule to selected ruleset
      if (selectedRulesetId !== '') {
        const ruleset = availableRulesets.find((rs) => rs.id === selectedRulesetId);
        if (ruleset) {
          await rulesetsApi.update(projectId, ruleset.id, {
            rule_ids: [...ruleset.rule_ids, createdRule.id],
          });
        }
      }

      invalidateAnalysis();
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
        <textarea rows="3" bind:value={regexPattern}></textarea>
        {#if suggestLoading}
          <div class="hint">Generating pattern...</div>
        {/if}
        {#if suggestError}
          <div class="hint error">{suggestError} (using fallback pattern)</div>
        {/if}
        <div
          class="preview"
          class:error={previewResult.startsWith('Invalid') || previewResult.startsWith('No match')}
        >
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

      {#if availableRulesets.length > 0}
        <div class="field">
          <label>Assign to Ruleset</label>
          <select bind:value={selectedRulesetId}>
            <option value="">None (assign later)</option>
            {#each availableRulesets as rs}
              <option value={rs.id}>{rs.name}</option>
            {/each}
          </select>
        </div>
      {/if}
    </div>

    <div class="modal-footer">
      <button onclick={onClose}>Cancel</button>
      <button
        class="primary"
        onclick={save}
        disabled={saving || !ruleName.trim() || !regexPattern.trim()}
      >
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

  .hint {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 4px;
    font-style: italic;
  }

  .hint.error {
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
