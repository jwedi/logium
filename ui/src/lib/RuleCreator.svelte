<script lang="ts">
  import {
    rules as rulesApi,
    rulesets as rulesetsApi,
    analysis as analysisApi,
    sources as sourcesApi,
    type Ruleset,
    type Source,
    type RuleMatch,
    type SuggestRuleResponse,
    type DryRunRequest,
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
  let extractionRules: {
    state_key: string;
    extraction_type: 'Parsed' | 'Static' | 'Clear';
    pattern: string;
    static_value: string;
    mode: 'Replace' | 'Accumulate';
    event_text_only: boolean;
  }[] = $state([]);
  let saving = $state(false);
  let suggestLoading = $state(false);
  let suggestError = $state('');
  let availableRulesets: Ruleset[] = $state([]);
  let selectedRulesetId: number | '' = $state('');
  let availableSources: Source[] = $state([]);
  let selectedSourceId: number | '' = $state('');
  let dryRunLoading = $state(false);
  let dryRunResults: RuleMatch[] = $state([]);
  let dryRunError = $state('');

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

  function seedExtractionRules(pattern: string) {
    const groups = detectGroups(pattern);
    extractionRules = groups.map((name) => ({
      state_key: name,
      extraction_type: 'Parsed' as const,
      pattern: pattern,
      static_value: '',
      mode: 'Replace' as const,
      event_text_only: false,
    }));
  }

  function addExtractionRule() {
    extractionRules = [
      ...extractionRules,
      {
        state_key: '',
        extraction_type: 'Parsed',
        pattern: '',
        static_value: '',
        mode: 'Replace',
        event_text_only: false,
      },
    ];
  }

  function removeExtractionRule(index: number) {
    extractionRules = extractionRules.filter((_, i) => i !== index);
  }

  async function fetchSuggestion() {
    suggestLoading = true;
    suggestError = '';
    try {
      const suggestion = await analysisApi.suggestRule(projectId, { text: selectedText });
      regexPattern = suggestion.pattern;
      seedExtractionRules(suggestion.pattern);
    } catch (e: any) {
      suggestError = e.message;
      // Fallback: escape text and replace numbers with capture groups
      let pattern = escapeRegex(selectedText);
      pattern = pattern.replace(/\d+/g, '(\\d+)');
      regexPattern = pattern;
      seedExtractionRules(pattern);
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

  async function loadSources() {
    try {
      const all = await sourcesApi.list(projectId);
      availableSources = all.filter((s) => s.template_id === sourceTemplateId);
    } catch {
      /* ignore */
    }
  }

  async function runDryRun() {
    if (selectedSourceId === '' || !regexPattern.trim()) return;
    dryRunLoading = true;
    dryRunError = '';
    dryRunResults = [];
    try {
      const data: DryRunRequest = {
        source_id: selectedSourceId as number,
        match_mode: matchMode,
        match_rules: [{ pattern: regexPattern }],
        extraction_rules: extractionRules.map((er) => ({
          extraction_type: er.extraction_type,
          state_key: er.state_key,
          pattern: er.extraction_type === 'Parsed' ? er.pattern || null : null,
          static_value: er.extraction_type === 'Static' ? er.static_value || null : null,
          mode: er.mode,
          event_text_only: er.event_text_only,
        })),
      };
      dryRunResults = await analysisApi.dryRun(projectId, data);
    } catch (e: any) {
      dryRunError = e.message;
    } finally {
      dryRunLoading = false;
    }
  }

  function formatStateValue(sv: Record<string, unknown>): string {
    const key = Object.keys(sv)[0];
    return String(sv[key]);
  }

  $effect(() => {
    sourceTemplateId;
    loadRulesets();
    loadSources();
  });

  async function save() {
    if (!ruleName.trim() || !regexPattern.trim()) return;
    saving = true;
    try {
      const createdRule = await rulesApi.create(projectId, {
        name: ruleName.trim(),
        match_mode: matchMode,
        match_rules: [{ id: 0, pattern: regexPattern }],
        extraction_rules: extractionRules.map((er) => ({
          id: 0,
          extraction_type: er.extraction_type,
          state_key: er.state_key,
          pattern: er.extraction_type === 'Parsed' ? er.pattern || null : null,
          static_value: er.extraction_type === 'Static' ? er.static_value || null : null,
          mode: er.mode,
          event_text_only: er.event_text_only,
        })),
        event_text: null,
      });

      // Assign rule to selected ruleset
      if (selectedRulesetId !== '') {
        const ruleset = availableRulesets.find((rs) => rs.id === selectedRulesetId);
        if (ruleset) {
          await rulesetsApi.update(projectId, ruleset.id, {
            name: ruleset.name,
            template_id: ruleset.template_id,
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

      <div class="extraction-rules">
        <div class="section-header">
          <label>Extraction Rules</label>
          <button class="small" onclick={addExtractionRule}>+ Add</button>
        </div>
        {#each extractionRules as er, i}
          <div class="group-row">
            <div class="group-row-top">
              <div class="field">
                <label>State Key</label>
                <input type="text" bind:value={er.state_key} placeholder="state_key" />
              </div>
              <div class="field">
                <label>Type</label>
                <select bind:value={er.extraction_type}>
                  <option value="Parsed">Parsed</option>
                  <option value="Static">Static</option>
                  <option value="Clear">Clear</option>
                </select>
              </div>
              <div class="field">
                <label>Mode</label>
                <select bind:value={er.mode}>
                  <option value="Replace">Replace</option>
                  <option value="Accumulate">Accumulate</option>
                </select>
              </div>
              <button class="small danger" onclick={() => removeExtractionRule(i)}>x</button>
            </div>
            {#if er.extraction_type === 'Parsed'}
              <div class="field">
                <label>Pattern</label>
                <input
                  type="text"
                  bind:value={er.pattern}
                  placeholder="e.g. ERROR (?P<message>.+)"
                />
              </div>
            {/if}
            {#if er.extraction_type === 'Static'}
              <div class="field">
                <label>Value</label>
                <input type="text" bind:value={er.static_value} placeholder="static value..." />
              </div>
            {/if}
          </div>
        {/each}
      </div>

      <div class="dry-run-section">
        <div class="section-header">
          <label>Test Against Source</label>
        </div>
        {#if availableSources.length === 0}
          <div class="hint">No sources use this template</div>
        {:else}
          <div class="dry-run-controls">
            <select bind:value={selectedSourceId}>
              <option value="">Select a source...</option>
              {#each availableSources as src}
                <option value={src.id}>{src.name}</option>
              {/each}
            </select>
            <button
              class="small"
              onclick={runDryRun}
              disabled={dryRunLoading || selectedSourceId === '' || !regexPattern.trim()}
            >
              {dryRunLoading ? 'Running...' : 'Run'}
            </button>
          </div>
          {#if dryRunError}
            <div class="hint error">{dryRunError}</div>
          {/if}
          {#if dryRunResults.length > 0}
            <div class="dry-run-results">
              <div class="hint">
                {dryRunResults.length} match{dryRunResults.length === 1 ? '' : 'es'} found
              </div>
              {#each dryRunResults as rm, i}
                <div class="dry-run-match">
                  <code>{rm.log_line.raw}</code>
                  {#if Object.keys(rm.extracted_state).length > 0}
                    <div class="extraction-chips">
                      {#each Object.entries(rm.extracted_state) as [key, value]}
                        <span class="chip"
                          >{key}: {formatStateValue(value as Record<string, unknown>)}</span
                        >
                      {/each}
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          {:else if !dryRunLoading && !dryRunError && selectedSourceId !== ''}
            <!-- only show after a run has been attempted -->
          {/if}
        {/if}
      </div>

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

  .extraction-rules {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .group-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    background: var(--bg);
    border-radius: var(--radius);
  }

  .group-row-top {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }

  .group-row-top .field {
    flex: 1;
  }

  button.small {
    padding: 2px 8px;
    font-size: 12px;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 16px 20px;
    border-top: 1px solid var(--border);
  }

  .dry-run-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .dry-run-controls {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .dry-run-controls select {
    flex: 1;
  }

  .dry-run-results {
    max-height: 200px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .dry-run-match {
    padding: 6px 8px;
    background: var(--bg);
    border-radius: var(--radius);
  }

  .dry-run-match code {
    display: block;
    font-family: var(--font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .extraction-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 4px;
  }

  .chip {
    display: inline-block;
    padding: 1px 6px;
    background: var(--green);
    color: var(--bg);
    border-radius: 4px;
    font-size: 11px;
    font-family: var(--font-mono);
  }
</style>
