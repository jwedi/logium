<script lang="ts">
  import type { StateChange, Source, LogRule, StateValue } from './api';

  let {
    stateChanges,
    sourceList,
    ruleList,
  }: { stateChanges: StateChange[]; sourceList: Source[]; ruleList: LogRule[] } = $props();

  let filterSource: string = $state('all');
  let filterKey: string = $state('all');

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

  let sourceNames = $derived([...new Set(stateChanges.map((sc) => sc.source_name))].sort());
  let stateKeys = $derived([...new Set(stateChanges.map((sc) => sc.state_key))].sort());

  let filteredChanges = $derived(
    stateChanges.filter((sc) => {
      if (filterSource !== 'all' && sc.source_name !== filterSource) return false;
      if (filterKey !== 'all' && sc.state_key !== filterKey) return false;
      return true;
    }),
  );
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
            <tr>
              <td class="ts">{sc.timestamp}</td>
              <td class="source">{sc.source_name}</td>
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

  .ts {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-dim);
    white-space: nowrap;
  }

  .source {
    color: var(--accent);
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
</style>
