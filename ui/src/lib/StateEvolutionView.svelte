<script lang="ts">
  import type { StateChange, Source, LogRule, StateValue } from './api';
  import { formatTimestamp } from './formatUtils';

  let {
    stateChanges,
    sourceList,
    ruleList,
  }: { stateChanges: StateChange[]; sourceList: Source[]; ruleList: LogRule[] } = $props();

  let filterSource: string = $state('all');
  let filterKey: string = $state('all');
  let selectedChange: StateChange | null = $state(null);

  function formatStateValue(sv: StateValue): string {
    if ('String' in sv) return sv.String;
    if ('Integer' in sv) return String(sv.Integer);
    if ('Float' in sv) return String(sv.Float);
    if ('Bool' in sv) return String(sv.Bool);
    return '?';
  }

  function getRuleName(id: number): string {
    return ruleList.find((r) => r.id === id)?.name ?? `Rule #${id}`;
  }

  function getSourceColor(name: string): string {
    return sourceList.find((s) => s.name === name)?.color ?? 'var(--accent)';
  }

  function closeModal() {
    selectedChange = null;
  }

  let sourceNames = $derived([...new Set(stateChanges.map((sc) => sc.source_name))].sort());
  let stateKeys = $derived([...new Set(stateChanges.map((sc) => sc.state_key))].sort());

  let filteredChanges = $derived(
    stateChanges.filter((sc) => {
      if (filterSource !== 'all' && sc.source_name !== filterSource) return false;
      if (filterKey !== 'all' && sc.state_key !== filterKey) return false;
      return true;
    }),
  );

  let stateSnapshot = $derived.by(() => {
    if (!selectedChange) return null;
    const ts = selectedChange.timestamp;
    const map = new Map<string, Map<string, StateValue>>();
    for (const sc of stateChanges) {
      if (sc.timestamp > ts) break;
      if (sc.new_value === null) {
        map.get(sc.source_name)?.delete(sc.state_key);
      } else {
        let src = map.get(sc.source_name);
        if (!src) {
          src = new Map();
          map.set(sc.source_name, src);
        }
        src.set(sc.state_key, sc.new_value);
      }
    }
    for (const [k, v] of map) if (v.size === 0) map.delete(k);
    return map;
  });
</script>

<div class="state-evolution">
  <div class="filter-bar">
    <label>
      Source
      <select bind:value={filterSource}>
        <option value="all">All sources</option>
        {#each sourceNames as name}
          <option value={name}>{name}</option>
        {/each}
      </select>
    </label>
    <label>
      Key
      <select bind:value={filterKey}>
        <option value="all">All keys</option>
        {#each stateKeys as key}
          <option value={key}>{key}</option>
        {/each}
      </select>
    </label>
    <span class="count"
      >{filteredChanges.length} change{filteredChanges.length !== 1 ? 's' : ''}</span
    >
  </div>

  {#if filteredChanges.length === 0}
    <div class="empty">No state changes to display.</div>
  {:else}
    <div class="table-container">
      <table>
        <thead>
          <tr>
            <th>Timestamp</th>
            <th>Source</th>
            <th>Key</th>
            <th>Change</th>
            <th>Rule</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredChanges as sc}
            <tr onclick={() => (selectedChange = sc)}>
              <td class="ts" title={sc.timestamp}>{formatTimestamp(sc.timestamp)}</td>
              <td class="source" style="color:{getSourceColor(sc.source_name)}">{sc.source_name}</td
              >
              <td class="key">{sc.state_key}</td>
              <td class="change">
                <span class="old-value"
                  >{sc.old_value ? formatStateValue(sc.old_value) : '(none)'}</span
                >
                <span class="arrow">&rarr;</span>
                <span class="new-value"
                  >{sc.new_value ? formatStateValue(sc.new_value) : '(none)'}</span
                >
              </td>
              <td class="rule">{getRuleName(sc.rule_id)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if selectedChange}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={closeModal}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h2>State Change Detail</h2>
        <button class="close-btn" onclick={closeModal} type="button">&times;</button>
      </div>
      <div class="modal-body">
        <div class="detail-section">
          <div class="detail-row">
            <span class="label">Timestamp</span><span class="mono">{selectedChange.timestamp}</span>
          </div>
          <div class="detail-row">
            <span class="label">Source</span><span
              style="color:{getSourceColor(selectedChange.source_name)}"
              >{selectedChange.source_name}</span
            >
          </div>
          <div class="detail-row">
            <span class="label">Key</span><span class="mono cyan">{selectedChange.state_key}</span>
          </div>
          <div class="detail-row">
            <span class="label">Change</span>
            <span class="mono">
              <span class="old-value"
                >{selectedChange.old_value
                  ? formatStateValue(selectedChange.old_value)
                  : '(none)'}</span
              >
              <span class="arrow"> → </span>
              <span class="new-value"
                >{selectedChange.new_value
                  ? formatStateValue(selectedChange.new_value)
                  : '(none)'}</span
              >
            </span>
          </div>
          <div class="detail-row">
            <span class="label">Rule</span><span>{getRuleName(selectedChange.rule_id)}</span>
          </div>
        </div>

        {#if stateSnapshot && stateSnapshot.size > 0}
          <div class="snapshot-section">
            <h3>State at this point</h3>
            {#each [...stateSnapshot.entries()].sort( ([a], [b]) => a.localeCompare(b), ) as [srcName, entries]}
              <div class="source-group">
                <div class="source-label" style="color:{getSourceColor(srcName)}">
                  Current state of {srcName}
                </div>
                {#each [...entries.entries()] as [key, val]}
                  <div class="state-entry">
                    <span class="state-key">{key}</span>
                    <span class="state-value">{formatStateValue(val)}</span>
                  </div>
                {/each}
              </div>
            {/each}
          </div>
        {:else}
          <div class="snapshot-section">
            <div class="empty-state">No accumulated state at this point.</div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .state-evolution {
    margin-top: 8px;
  }

  .filter-bar {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 12px;
  }

  .filter-bar label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text-dim);
  }

  .filter-bar select {
    padding: 4px 8px;
    font-size: 13px;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .count {
    font-size: 12px;
    color: var(--text-dim);
    margin-left: auto;
  }

  .empty {
    color: var(--text-dim);
    text-align: center;
    padding: 32px;
  }

  .table-container {
    overflow-x: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th {
    text-align: left;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    color: var(--text-dim);
    font-weight: 600;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  td {
    padding: 6px 12px;
    border-bottom: 1px solid var(--bg-secondary);
  }

  tbody tr {
    cursor: pointer;
  }

  tbody tr:hover {
    background: var(--bg-secondary);
  }

  .ts {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-dim);
    white-space: nowrap;
  }

  .source {
    font-weight: 500;
  }

  .key {
    font-family: var(--font-mono);
    color: var(--cyan);
  }

  .change {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .old-value {
    color: var(--text-dim);
  }

  .arrow {
    color: var(--text-dim);
    margin: 0 6px;
  }

  .new-value {
    color: var(--text);
  }

  .rule {
    font-size: 12px;
    color: var(--text-dim);
  }

  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    width: 520px;
    max-width: 90vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 16px;
  }

  .close-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-dim);
    font-size: 20px;
    line-height: 1;
    padding: 0 4px;
  }

  .modal-body {
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .detail-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .detail-row {
    display: flex;
    gap: 12px;
    font-size: 13px;
  }

  .label {
    width: 80px;
    flex-shrink: 0;
    color: var(--text-dim);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .accent {
    color: var(--accent);
  }

  .cyan {
    color: var(--cyan);
  }

  .snapshot-section h3 {
    margin: 0 0 12px;
    font-size: 13px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .source-group {
    margin-bottom: 16px;
  }

  .source-label {
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
    margin-bottom: 6px;
  }

  .state-entry {
    display: flex;
    gap: 12px;
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 2px 0;
  }

  .state-key {
    color: var(--cyan);
    min-width: 120px;
  }

  .state-value {
    color: var(--text);
  }

  .empty-state {
    color: var(--text-dim);
    font-size: 13px;
  }
</style>
