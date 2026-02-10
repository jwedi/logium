<script lang="ts">
  import { rules as rulesApi, type LogRule, type ExtractionRule } from './api';
  import { invalidateAnalysis } from './analysisInvalidation.svelte';
  import { testPattern, toJsRegex } from './regexUtils';

  let {
    rule,
    projectId,
    onSave,
    onCancel,
  }: {
    rule: LogRule;
    projectId: number;
    onSave: () => void;
    onCancel: () => void;
  } = $props();

  // Mutable editing copies — destructure prop immediately to avoid Svelte warning
  const {
    name: initName,
    match_mode: initMode,
    match_rules: initMR,
    extraction_rules: initER,
  } = rule;
  let editName = $state(initName);
  let editMatchMode: 'Any' | 'All' = $state(initMode);
  let editMatchPatterns: { id: number; pattern: string }[] = $state(
    initMR.map((mr) => ({ id: mr.id, pattern: mr.pattern })),
  );
  let editExtractionRules: {
    id: number;
    state_key: string;
    extraction_type: 'Parsed' | 'Static' | 'Clear';
    pattern: string;
    static_value: string;
    mode: 'Replace' | 'Accumulate';
  }[] = $state(
    initER.map((er) => ({
      id: er.id,
      state_key: er.state_key,
      extraction_type: er.extraction_type,
      pattern: er.pattern ?? '',
      static_value: er.static_value ?? '',
      mode: er.mode,
    })),
  );

  let saving = $state(false);

  // Dry-run state
  let testLine = $state('');

  // Per-pattern test results
  let patternResults = $derived(
    testLine.trim() ? editMatchPatterns.map((mp) => testPattern(mp.pattern, testLine)) : [],
  );

  // Overall verdict
  let overallVerdict = $derived.by(() => {
    if (!testLine.trim() || patternResults.length === 0) return null;
    const matchCount = patternResults.filter((r) => r.status === 'match').length;
    const hasError = patternResults.some((r) => r.status === 'error');
    if (hasError) return { ok: false, text: 'One or more patterns have errors' };
    if (editMatchMode === 'Any') {
      return matchCount > 0
        ? {
            ok: true,
            text: `${matchCount} of ${patternResults.length} pattern${patternResults.length !== 1 ? 's' : ''} matched (Any mode)`,
          }
        : { ok: false, text: 'No patterns matched' };
    }
    // All mode
    return matchCount === patternResults.length
      ? { ok: true, text: `All ${matchCount} pattern${matchCount !== 1 ? 's' : ''} matched` }
      : {
          ok: false,
          text: `${matchCount} of ${patternResults.length} patterns matched (All mode requires all)`,
        };
  });

  // Extraction preview
  let extractionPreview = $derived.by(() => {
    if (!testLine.trim()) return [];
    return editExtractionRules.map((er) => {
      if (er.extraction_type === 'Clear') {
        return { key: er.state_key, value: '(cleared)', type: 'Clear' as const };
      }
      if (er.extraction_type === 'Static') {
        return { key: er.state_key, value: er.static_value || '(empty)', type: 'Static' as const };
      }
      // Parsed — run extraction pattern
      if (!er.pattern) {
        return { key: er.state_key, value: '(no pattern)', type: 'Parsed' as const };
      }
      try {
        const jsPattern = toJsRegex(er.pattern);
        const re = new RegExp(jsPattern);
        const m = re.exec(testLine);
        if (m?.groups) {
          // Try the state_key as a group name, else take the first group
          const val = m.groups[er.state_key] ?? Object.values(m.groups)[0] ?? m[0];
          return { key: er.state_key, value: val, type: 'Parsed' as const };
        }
        if (m && m.length > 1) {
          return { key: er.state_key, value: m[1], type: 'Parsed' as const };
        }
        if (m) {
          return { key: er.state_key, value: m[0], type: 'Parsed' as const };
        }
        return { key: er.state_key, value: '(no match)', type: 'Parsed' as const };
      } catch {
        return { key: er.state_key, value: '(invalid regex)', type: 'Parsed' as const };
      }
    });
  });

  function addMatchPattern() {
    editMatchPatterns = [...editMatchPatterns, { id: 0, pattern: '' }];
  }

  function removeMatchPattern(index: number) {
    editMatchPatterns = editMatchPatterns.filter((_, i) => i !== index);
  }

  function addExtractionRule() {
    editExtractionRules = [
      ...editExtractionRules,
      {
        id: 0,
        state_key: '',
        extraction_type: 'Parsed',
        pattern: '',
        static_value: '',
        mode: 'Replace',
      },
    ];
  }

  function removeExtractionRule(index: number) {
    editExtractionRules = editExtractionRules.filter((_, i) => i !== index);
  }

  let canSave = $derived(
    editName.trim() !== '' && editMatchPatterns.some((mp) => mp.pattern.trim() !== ''),
  );

  async function save() {
    if (!canSave) return;
    saving = true;
    try {
      const payload: Partial<LogRule> = {
        name: editName.trim(),
        match_mode: editMatchMode,
        match_rules: editMatchPatterns
          .filter((mp) => mp.pattern.trim())
          .map((mp) => ({ id: 0, pattern: mp.pattern })),
        extraction_rules: editExtractionRules.map(
          (er): ExtractionRule => ({
            id: 0,
            extraction_type: er.extraction_type,
            state_key: er.state_key,
            pattern: er.extraction_type === 'Parsed' ? er.pattern || null : null,
            static_value: er.extraction_type === 'Static' ? er.static_value || null : null,
            mode: er.mode,
          }),
        ),
      };
      await rulesApi.update(projectId, rule.id, payload);
      invalidateAnalysis();
      onSave();
    } catch (e: any) {
      alert(e.message);
    } finally {
      saving = false;
    }
  }
</script>

<div class="rule-editor">
  <div class="editor-row">
    <div class="field" style="flex:2">
      <label>Name</label>
      <input type="text" bind:value={editName} />
    </div>
    <div class="field" style="flex:1">
      <label>
        Match Mode
        <span
          class="info-icon"
          data-tooltip="Any: rule fires if at least one match pattern matches the log line. All: rule fires only if every match pattern matches the log line."
          >?</span
        >
      </label>
      <select bind:value={editMatchMode}>
        <option value="Any">Any</option>
        <option value="All">All</option>
      </select>
    </div>
  </div>

  <div class="section-header">
    <h3>Match Patterns</h3>
    <button class="small" onclick={addMatchPattern}>+ Add</button>
  </div>

  {#each editMatchPatterns as mp, i}
    <div class="pattern-row">
      <textarea
        rows="1"
        bind:value={mp.pattern}
        placeholder="regex pattern..."
        class="pattern-input"
      ></textarea>
      <button class="small danger" onclick={() => removeMatchPattern(i)}>x</button>
      {#if testLine.trim() && patternResults[i]}
        <span
          class="test-indicator"
          class:match={patternResults[i].status === 'match'}
          class:no-match={patternResults[i].status !== 'match'}
        >
          {patternResults[i].status === 'match'
            ? 'Match'
            : patternResults[i].status === 'error'
              ? 'Error'
              : 'No match'}
        </span>
      {/if}
    </div>
  {/each}

  <div class="section-header">
    <h3>Extraction Rules</h3>
    <button class="small" onclick={addExtractionRule}>+ Add</button>
  </div>

  {#each editExtractionRules as er, i}
    <div class="extraction-row">
      <div class="field">
        <label>Key</label>
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
      {#if er.extraction_type === 'Parsed'}
        <div class="field" style="flex:2">
          <label>Pattern</label>
          <input type="text" bind:value={er.pattern} placeholder="regex with groups..." />
        </div>
      {/if}
      {#if er.extraction_type === 'Static'}
        <div class="field" style="flex:2">
          <label>Value</label>
          <input type="text" bind:value={er.static_value} placeholder="static value..." />
        </div>
      {/if}
      <button class="small danger remove-extraction" onclick={() => removeExtractionRule(i)}
        >x</button
      >
    </div>
  {/each}

  <div class="dry-run-section">
    <h3>Test Rule (dry run)</h3>
    <textarea
      rows="2"
      bind:value={testLine}
      placeholder="Paste a log line here to test..."
      class="test-input"
    ></textarea>

    {#if overallVerdict}
      <div class="verdict" class:ok={overallVerdict.ok} class:fail={!overallVerdict.ok}>
        {overallVerdict.text}
      </div>
    {/if}

    {#if extractionPreview.length > 0 && testLine.trim()}
      <div class="extraction-preview">
        <strong>Extracted State:</strong>
        {#each extractionPreview as ep}
          <div class="preview-row">
            <span class="preview-key">{ep.key}</span>
            <span class="preview-eq">=</span>
            <span class="preview-value">{ep.value}</span>
            <span class="preview-type">({ep.type})</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="editor-footer">
    <button onclick={onCancel}>Cancel</button>
    <button class="primary" onclick={save} disabled={saving || !canSave}>
      {saving ? 'Saving...' : 'Save'}
    </button>
  </div>
</div>

<style>
  .rule-editor {
    padding: 16px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .editor-row {
    display: flex;
    gap: 12px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .section-header h3 {
    margin: 0;
  }

  .pattern-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pattern-input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 12px;
    resize: vertical;
  }

  .test-indicator {
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
  }

  .test-indicator.match {
    color: var(--green);
  }

  .test-indicator.no-match {
    color: var(--red);
  }

  .extraction-row {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 8px;
    background: var(--bg);
    border-radius: var(--radius);
  }

  .extraction-row .field {
    flex: 1;
  }

  .remove-extraction {
    margin-bottom: 2px;
  }

  .dry-run-section {
    border-top: 1px solid var(--border);
    padding-top: 12px;
  }

  .dry-run-section h3 {
    margin: 0 0 8px;
  }

  .test-input {
    width: 100%;
    font-family: var(--font-mono);
    font-size: 12px;
    resize: vertical;
  }

  .verdict {
    margin-top: 8px;
    padding: 6px 10px;
    border-radius: var(--radius);
    font-size: 13px;
    font-weight: 600;
  }

  .verdict.ok {
    color: var(--green);
    background: rgba(158, 206, 106, 0.1);
  }

  .verdict.fail {
    color: var(--red);
    background: rgba(247, 118, 142, 0.1);
  }

  .extraction-preview {
    margin-top: 8px;
    font-size: 13px;
  }

  .preview-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 0;
  }

  .preview-key {
    font-family: var(--font-mono);
    color: var(--cyan);
    font-weight: 600;
  }

  .preview-eq {
    color: var(--text-muted);
  }

  .preview-value {
    font-family: var(--font-mono);
    color: var(--green);
  }

  .preview-type {
    font-size: 11px;
    color: var(--text-muted);
  }

  .editor-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  button.small {
    padding: 2px 8px;
    font-size: 12px;
  }

  .info-icon {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text-muted);
    color: var(--bg);
    font-size: 10px;
    font-weight: 700;
    cursor: help;
    vertical-align: middle;
    margin-left: 4px;
  }

  .info-icon::after {
    content: attr(data-tooltip);
    position: absolute;
    left: 50%;
    top: calc(100% + 6px);
    transform: translateX(-50%);
    background: var(--bg-tertiary, #333);
    color: var(--text, #eee);
    font-size: 12px;
    font-weight: 400;
    line-height: 1.4;
    padding: 6px 10px;
    border-radius: var(--radius, 4px);
    white-space: normal;
    width: 260px;
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.15s;
    z-index: 10;
  }

  .info-icon:hover::after {
    opacity: 1;
  }
</style>
