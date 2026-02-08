<script lang="ts">
  import {
    patterns as patternsApi,
    sources as sourcesApi,
    type Pattern,
    type PatternPredicate,
    type Source,
    type StateValue,
  } from './api';

  let { projectId }: { projectId: number } = $props();

  let patternList: Pattern[] = $state([]);
  let sourceList: Source[] = $state([]);
  let loading = $state(false);
  let editingPattern = $state<Pattern | null>(null);

  const OPERATORS = ['Eq', 'Neq', 'Gt', 'Lt', 'Gte', 'Lte', 'Contains', 'Exists'];

  // New pattern form
  let newName = $state('');
  let newPredicates: PatternPredicate[] = $state([]);

  function emptyPredicate(): PatternPredicate {
    return {
      source_name: '',
      state_key: '',
      operator: 'Eq',
      operand: { Literal: { String: '' } },
    };
  }

  function isLiteral(op: PatternPredicate['operand']): op is { Literal: StateValue } {
    return 'Literal' in op;
  }

  function getStateRef(op: PatternPredicate['operand']): {
    source_name: string;
    state_key: string;
  } {
    if ('StateRef' in op) return op.StateRef;
    return { source_name: '', state_key: '' };
  }

  function getLiteralString(op: PatternPredicate['operand']): string {
    if (!isLiteral(op)) return '';
    const val = op.Literal;
    if ('String' in val) return val.String;
    if ('Integer' in val) return String(val.Integer);
    if ('Float' in val) return String(val.Float);
    if ('Bool' in val) return String(val.Bool);
    return '';
  }

  function setLiteralValue(pred: PatternPredicate, value: string) {
    pred.operand = { Literal: { String: value } };
  }

  function setStateRef(pred: PatternPredicate, srcName: string, stateKey: string) {
    pred.operand = { StateRef: { source_name: srcName, state_key: stateKey } };
  }

  function toggleOperandType(pred: PatternPredicate) {
    if (isLiteral(pred.operand)) {
      pred.operand = { StateRef: { source_name: '', state_key: '' } };
    } else {
      pred.operand = { Literal: { String: '' } };
    }
  }

  function addPredicate(list: PatternPredicate[]): PatternPredicate[] {
    return [...list, emptyPredicate()];
  }

  function removePredicate(list: PatternPredicate[], idx: number): PatternPredicate[] {
    return list.filter((_, i) => i !== idx);
  }

  function movePredicate(list: PatternPredicate[], from: number, to: number): PatternPredicate[] {
    if (to < 0 || to >= list.length) return list;
    const copy = [...list];
    const [item] = copy.splice(from, 1);
    copy.splice(to, 0, item);
    return copy;
  }

  async function load() {
    loading = true;
    try {
      [patternList, sourceList] = await Promise.all([
        patternsApi.list(projectId),
        sourcesApi.list(projectId),
      ]);
    } catch (e: any) {
      alert(e.message);
    } finally {
      loading = false;
    }
  }

  async function createPattern() {
    if (!newName.trim()) return;
    try {
      await patternsApi.create(projectId, {
        name: newName.trim(),
        predicates: newPredicates,
      });
      newName = '';
      newPredicates = [];
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function updatePattern() {
    if (!editingPattern) return;
    try {
      await patternsApi.update(projectId, editingPattern.id, {
        name: editingPattern.name,
        predicates: editingPattern.predicates,
      });
      editingPattern = null;
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  async function deletePattern(id: number) {
    if (!confirm('Delete this pattern?')) return;
    try {
      await patternsApi.delete(projectId, id);
      if (editingPattern?.id === id) editingPattern = null;
      await load();
    } catch (e: any) {
      alert(e.message);
    }
  }

  let activePredicates = $derived(editingPattern ? editingPattern.predicates : newPredicates);

  $effect(() => {
    projectId;
    load();
  });
</script>

<h2>Patterns</h2>

<div class="create-form card">
  <h3>{editingPattern ? `Edit Pattern: ${editingPattern.name}` : 'New Pattern'}</h3>

  <div class="field">
    <label>Name</label>
    {#if editingPattern}
      <input type="text" bind:value={editingPattern.name} />
    {:else}
      <input type="text" bind:value={newName} placeholder="Pattern name..." />
    {/if}
  </div>

  <div class="predicates-section">
    <div class="predicates-header">
      <label>Predicates (ordered)</label>
      <button
        onclick={() => {
          if (editingPattern) {
            editingPattern.predicates = addPredicate(editingPattern.predicates);
          } else {
            newPredicates = addPredicate(newPredicates);
          }
        }}
      >
        Add Predicate
      </button>
    </div>

    {#if activePredicates.length === 0}
      <div class="empty">No predicates. Click "Add Predicate" to start.</div>
    {:else}
      {#each activePredicates as pred, i}
        <div class="predicate-row">
          <div class="predicate-order">
            <button
              class="move-btn"
              onclick={() => {
                if (editingPattern)
                  editingPattern.predicates = movePredicate(editingPattern.predicates, i, i - 1);
                else newPredicates = movePredicate(newPredicates, i, i - 1);
              }}
              disabled={i === 0}>^</button
            >
            <span class="order-num">{i + 1}</span>
            <button
              class="move-btn"
              onclick={() => {
                if (editingPattern)
                  editingPattern.predicates = movePredicate(editingPattern.predicates, i, i + 1);
                else newPredicates = movePredicate(newPredicates, i, i + 1);
              }}
              disabled={i === activePredicates.length - 1}>v</button
            >
          </div>

          <div class="predicate-fields">
            <div class="field">
              <label>Source</label>
              <select bind:value={pred.source_name}>
                <option value="">Select...</option>
                {#each sourceList as src}
                  <option value={src.name}>{src.name}</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label>State Key</label>
              <input type="text" bind:value={pred.state_key} placeholder="key..." />
            </div>
            <div class="field">
              <label>Operator</label>
              <select bind:value={pred.operator}>
                {#each OPERATORS as op}
                  <option value={op}>{op}</option>
                {/each}
              </select>
            </div>
            <div class="field">
              <label>
                Operand
                <button class="toggle-btn" onclick={() => toggleOperandType(pred)}>
                  {isLiteral(pred.operand) ? 'Literal' : 'State Ref'} (click to toggle)
                </button>
              </label>
              {#if isLiteral(pred.operand)}
                <input
                  type="text"
                  value={getLiteralString(pred.operand)}
                  oninput={(e) => setLiteralValue(pred, (e.target as HTMLInputElement).value)}
                  placeholder="value..."
                />
              {:else}
                <div class="row">
                  <input
                    type="text"
                    value={getStateRef(pred.operand).source_name}
                    oninput={(e) =>
                      setStateRef(
                        pred,
                        (e.target as HTMLInputElement).value,
                        getStateRef(pred.operand).state_key,
                      )}
                    placeholder="source..."
                    style="flex:1"
                  />
                  <input
                    type="text"
                    value={getStateRef(pred.operand).state_key}
                    oninput={(e) =>
                      setStateRef(
                        pred,
                        getStateRef(pred.operand).source_name,
                        (e.target as HTMLInputElement).value,
                      )}
                    placeholder="key..."
                    style="flex:1"
                  />
                </div>
              {/if}
            </div>
          </div>

          <button
            class="remove-btn danger"
            onclick={() => {
              if (editingPattern)
                editingPattern.predicates = removePredicate(editingPattern.predicates, i);
              else newPredicates = removePredicate(newPredicates, i);
            }}>x</button
          >
        </div>
      {/each}
    {/if}
  </div>

  <div class="actions">
    {#if editingPattern}
      <button class="primary" onclick={updatePattern}>Save</button>
      <button onclick={() => (editingPattern = null)}>Cancel</button>
    {:else}
      <button class="primary" onclick={createPattern} disabled={!newName.trim()}
        >Create Pattern</button
      >
    {/if}
  </div>
</div>

{#if !editingPattern && patternList.length > 0}
  <div class="pattern-list">
    {#each patternList as pattern}
      <div class="pattern-card card">
        <div class="pattern-info">
          <span class="pattern-name">{pattern.name}</span>
          <span class="badge"
            >{pattern.predicates.length} predicate{pattern.predicates.length !== 1 ? 's' : ''}</span
          >
        </div>
        <div class="pattern-actions">
          <button
            onclick={() =>
              (editingPattern = {
                ...pattern,
                predicates: pattern.predicates.map((p) => ({ ...p })),
              })}>Edit</button
          >
          <button class="danger" onclick={() => deletePattern(pattern.id)}>Delete</button>
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .create-form {
    margin-bottom: 20px;
  }

  .predicates-section {
    margin-top: 16px;
  }

  .predicates-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .predicate-row {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 12px;
    background: var(--bg);
    border-radius: var(--radius);
    margin-bottom: 8px;
  }

  .predicate-order {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding-top: 16px;
  }

  .order-num {
    font-weight: 700;
    color: var(--accent);
    font-size: 14px;
  }

  .move-btn {
    padding: 2px 6px;
    font-size: 11px;
    border: none;
    background: var(--bg-tertiary);
  }

  .predicate-fields {
    flex: 1;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .toggle-btn {
    display: inline;
    border: none;
    background: none;
    color: var(--accent);
    font-size: 10px;
    padding: 0;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
  }

  .remove-btn {
    padding: 4px 8px;
    font-size: 14px;
    margin-top: 16px;
  }

  .pattern-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .pattern-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .pattern-info {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pattern-name {
    font-weight: 600;
  }

  .pattern-actions {
    display: flex;
    gap: 8px;
  }
</style>
