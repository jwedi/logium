<script lang="ts">
  import type {
    RuleMatch,
    StateChange,
    PatternMatch,
    Source,
    LogRule,
    Pattern,
    StateValue,
  } from './api';
  import { formatTimestamp } from './formatUtils';

  let {
    ruleMatches,
    stateChanges,
    patternMatches,
    sourceList,
    ruleList,
    patternList,
  }: {
    ruleMatches: RuleMatch[];
    stateChanges: StateChange[];
    patternMatches: PatternMatch[];
    sourceList: Source[];
    ruleList: LogRule[];
    patternList: Pattern[];
  } = $props();

  let filterSource: string = $state('all');
  let filterRule: string = $state('all');
  let filterKey: string = $state('all');

  // Panels open for inline state snapshot, keyed by `${timestamp}|${rule_id}|${source_id}`
  let openStatePanels: Set<string> = $state(new Set());
  // Raw line expanded, keyed same way
  let openRawLines: Set<string> = $state(new Set());
  // Rule panel expanded, keyed same way
  let openRulePanels: Set<string> = $state(new Set());

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

  function getSourceName(id: number): string {
    return sourceList.find((s) => s.id === id)?.name ?? `Source #${id}`;
  }

  function getSourceColor(id: number): string {
    return sourceList.find((s) => s.id === id)?.color ?? 'var(--accent)';
  }

  function getSourceColorByName(name: string): string {
    return sourceList.find((s) => s.name === name)?.color ?? 'var(--accent)';
  }

  function getPatternName(id: number): string {
    return patternList.find((p) => p.id === id)?.name ?? `Pattern #${id}`;
  }

  // Join key for looking up state changes (same fields as StateChange has)
  function joinKey(rm: RuleMatch): string {
    return `${rm.log_line.timestamp}|${rm.rule_id}|${rm.source_id}`;
  }

  // Toggle key for tracking open panels — includes raw line to disambiguate
  // multiple matches from the same source/rule at the same timestamp
  function rowKey(rm: RuleMatch): string {
    return `${rm.log_line.timestamp}|${rm.rule_id}|${rm.source_id}|${rm.log_line.raw}`;
  }

  function toggleStatePanel(key: string) {
    const next = new Set(openStatePanels);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    openStatePanels = next;
  }

  function toggleRawLine(key: string) {
    const next = new Set(openRawLines);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    openRawLines = next;
  }

  function toggleRulePanel(key: string) {
    const next = new Set(openRulePanels);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    openRulePanels = next;
  }

  // Build lookup: (timestamp, rule_id, source_id) -> StateChange[]
  // StateChange has source_id? Let's check: StateChange has source_id and rule_id and timestamp
  // We need to match stateChanges to ruleMatches
  let stateChangesByKey = $derived.by(() => {
    const map = new Map<string, StateChange[]>();
    for (const sc of stateChanges) {
      // StateChange has source_id, rule_id, timestamp
      const key = `${sc.timestamp}|${sc.rule_id}|${sc.source_id}`;
      let arr = map.get(key);
      if (!arr) {
        arr = [];
        map.set(key, arr);
      }
      arr.push(sc);
    }
    return map;
  });

  // Derived: unique source names from ruleMatches (by source_id)
  let sourceOptions = $derived(
    [...new Set(ruleMatches.map((rm) => rm.source_id))].map((id) => ({
      id,
      name: getSourceName(id),
    })),
  );

  // Derived: unique state keys from stateChanges
  let stateKeys = $derived([...new Set(stateChanges.map((sc) => sc.state_key))].sort());

  // Filtered rule matches
  let filteredMatches = $derived(
    ruleMatches.filter((rm) => {
      if (filterSource !== 'all' && String(rm.source_id) !== filterSource) return false;
      if (filterRule !== 'all' && String(rm.rule_id) !== filterRule) return false;
      if (filterKey !== 'all') {
        // Only include if this match has a state change with this key
        const scs = stateChangesByKey.get(joinKey(rm)) ?? [];
        if (!scs.some((sc) => sc.state_key === filterKey)) return false;
      }
      return true;
    }),
  );

  type MatchRow = { kind: 'match'; rm: RuleMatch };
  type PatternRow = { kind: 'pattern'; pm: PatternMatch };
  type FeedRow = MatchRow | PatternRow;

  // Merge and sort by timestamp ascending
  let feedRows: FeedRow[] = $derived.by(() => {
    const rows: FeedRow[] = [
      ...filteredMatches.map((rm): MatchRow => ({ kind: 'match', rm })),
      ...patternMatches.map((pm): PatternRow => ({ kind: 'pattern', pm })),
    ];
    rows.sort((a, b) => {
      const ta = a.kind === 'match' ? a.rm.log_line.timestamp : a.pm.timestamp;
      const tb = b.kind === 'match' ? b.rm.log_line.timestamp : b.pm.timestamp;
      return ta < tb ? -1 : ta > tb ? 1 : 0;
    });
    return rows;
  });

  // Compute state snapshot up to (and including) a given timestamp
  function computeStateSnapshot(upToTimestamp: string): Map<string, Map<string, StateValue>> {
    const map = new Map<string, Map<string, StateValue>>();
    for (const sc of stateChanges) {
      if (sc.timestamp > upToTimestamp) break;
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
  }
</script>

<div class="event-feed">
  <div class="filter-bar">
    <label>
      Source
      <select bind:value={filterSource}>
        <option value="all">All sources</option>
        {#each sourceOptions as src}
          <option value={String(src.id)}>{src.name}</option>
        {/each}
      </select>
    </label>
    <label>
      Rule
      <select bind:value={filterRule}>
        <option value="all">All rules</option>
        {#each ruleList as rule}
          <option value={String(rule.id)}>{rule.name}</option>
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
      >{filteredMatches.length} event{filteredMatches.length !== 1 ? 's' : ''}</span
    >
  </div>

  {#if feedRows.length === 0}
    <div class="empty">No events to display.</div>
  {:else}
    <div class="feed">
      {#each feedRows as row, rowIdx}
        {#if row.kind === 'pattern'}
          {@const pm = row.pm}
          {@const pmKey = `pm_${rowIdx}|${pm.pattern_id}|${pm.timestamp}`}
          {@const pmStateOpen = openStatePanels.has(pmKey)}
          <div class="event-card">
            <div class="color-bar" style="background:var(--purple)"></div>
            <div class="card-body">
              <div class="card-headline">
                <span class="ts" title={pm.timestamp}>{formatTimestamp(pm.timestamp)}</span>
                <span style="color:var(--purple);font-size:12px">&#9670;</span>
                <span style="color:var(--purple);font-weight:600"
                  >Pattern matched: {getPatternName(pm.pattern_id)}</span
                >
              </div>
              <div class="card-chips-row">
                <div class="card-actions">
                  <button
                    class="action-btn"
                    class:active={pmStateOpen}
                    onclick={() => toggleStatePanel(pmKey)}
                    type="button"
                    title="Toggle pattern state snapshot"
                    >&#8801; state {pmStateOpen ? '&#9650;' : ''}</button
                  >
                </div>
              </div>
              {#if pmStateOpen}
                <div class="state-panel">
                  <div class="state-panel-header">State at match time</div>
                  {#if Object.keys(pm.state_snapshot).length === 0}
                    <div class="state-panel-empty">No state at match time.</div>
                  {:else}
                    {#each Object.entries(pm.state_snapshot).sort( ([a], [b]) => a.localeCompare(b), ) as [srcName, entries]}
                      <div class="state-source-group">
                        <span
                          class="state-source-name"
                          style="color:{getSourceColorByName(srcName)}">{srcName}</span
                        >
                        <div class="state-entries">
                          {#each Object.entries(entries) as [k, tv]}
                            <span class="state-kv">
                              <span class="state-k">{k}</span>
                              <span class="state-eq">=</span>
                              <span class="state-v">{formatStateValue(tv.value)}</span>
                              <span class="pm-set-at">({formatTimestamp(tv.set_at)})</span>
                            </span>
                          {/each}
                        </div>
                      </div>
                    {/each}
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        {:else}
          {@const rm = row.rm}
          {@const key = rowKey(rm)}
          {@const scs = stateChangesByKey.get(joinKey(rm)) ?? []}
          {@const stateOpen = openStatePanels.has(key)}
          {@const rawOpen = openRawLines.has(key)}
          <div class="event-card">
            <div class="color-bar" style="background:{getSourceColor(rm.source_id)}"></div>
            <div class="card-body">
              <div class="card-headline">
                <span class="ts" title={rm.log_line.timestamp}
                  >{formatTimestamp(rm.log_line.timestamp)}</span
                >
                <span class="source-name" style="color:{getSourceColor(rm.source_id)}"
                  >{getSourceName(rm.source_id)}</span
                >
                <span class="event-text">
                  {#if rm.event_text}
                    {rm.event_text}
                  {:else}
                    <span class="rule-name-fallback">[{getRuleName(rm.rule_id)}]</span>
                  {/if}
                </span>
              </div>
              <div class="card-chips-row">
                {#if scs.length > 0}
                  <span class="chips-label">&#8627;</span>
                  {#each scs as sc}
                    <span
                      class="state-chip"
                      class:chip-clear={sc.new_value === null}
                      title={sc.old_value !== null
                        ? `was: ${formatStateValue(sc.old_value)}`
                        : 'new key'}
                    >
                      {sc.state_key}
                      {#if sc.new_value !== null}
                        <span class="chip-arrow">&#8594;</span>
                        <span class="chip-value">{formatStateValue(sc.new_value)}</span>
                      {:else}
                        <span class="chip-clear-mark">&#x2715;</span>
                      {/if}
                    </span>
                  {/each}
                {/if}
                <div class="card-actions">
                  <button
                    class="action-btn"
                    class:active={rawOpen}
                    onclick={() => toggleRawLine(key)}
                    type="button"
                    title="Toggle raw log line">&#9654; raw</button
                  >
                  <button
                    class="action-btn"
                    class:active={stateOpen}
                    onclick={() => toggleStatePanel(key)}
                    type="button"
                    title="Toggle state at this moment"
                    >&#8801; state {stateOpen ? '&#9650;' : ''}</button
                  >
                  <button
                    class="action-btn"
                    class:active={openRulePanels.has(key)}
                    onclick={() => toggleRulePanel(key)}
                    type="button"
                    title="Toggle rule details"
                    >&#8862; rule{openRulePanels.has(key) ? ' &#9650;' : ''}</button
                  >
                </div>
              </div>
              {#if rawOpen}
                <div class="raw-line">{rm.log_line.raw}</div>
              {/if}
              {#if openRulePanels.has(key)}
                {@const rule = ruleList.find((r) => r.id === rm.rule_id)}
                <div class="rule-panel">
                  {#if rule}
                    <div class="rule-panel-name">{rule.name}</div>
                    <div class="rule-panel-mode">
                      match mode: <span class="rule-mode-badge">{rule.match_mode}</span>
                    </div>
                    {#if rule.match_rules.length > 0}
                      <div class="rule-panel-patterns">
                        {#each rule.match_rules as mr}
                          <code class="rule-pattern">{mr.pattern}</code>
                        {/each}
                      </div>
                    {/if}
                  {:else}
                    <span class="rule-panel-missing">Rule #{rm.rule_id}</span>
                  {/if}
                </div>
              {/if}
              {#if stateOpen}
                {@const snapshot = computeStateSnapshot(rm.log_line.timestamp)}
                <div class="state-panel">
                  <div class="state-panel-header">State at this moment</div>
                  {#if snapshot.size === 0}
                    <div class="state-panel-empty">No accumulated state at this point.</div>
                  {:else}
                    {#each [...snapshot.entries()].sort( ([a], [b]) => a.localeCompare(b), ) as [srcName, entries]}
                      <div class="state-source-group">
                        <span
                          class="state-source-name"
                          style="color:{getSourceColorByName(srcName)}">{srcName}</span
                        >
                        <div class="state-entries">
                          {#each [...entries.entries()] as [k, val]}
                            <span class="state-kv">
                              <span class="state-k">{k}</span>
                              <span class="state-eq">=</span>
                              <span class="state-v">{formatStateValue(val)}</span>
                            </span>
                          {/each}
                        </div>
                      </div>
                    {/each}
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .event-feed {
    margin-top: 8px;
  }

  .filter-bar {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 12px;
    flex-wrap: wrap;
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

  .feed {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  /* Event card */
  .event-card {
    display: flex;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .color-bar {
    width: 4px;
    flex-shrink: 0;
  }

  .card-body {
    flex: 1;
    min-width: 0;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .card-headline {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }

  .ts {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-dim);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .source-name {
    font-size: 13px;
    font-weight: 600;
    flex-shrink: 0;
  }

  .event-text {
    font-size: 13px;
    color: var(--text);
    flex: 1;
    min-width: 0;
  }

  .rule-name-fallback {
    color: var(--text-dim);
    font-style: italic;
  }

  .card-chips-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .chips-label {
    font-size: 13px;
    color: var(--text-dim);
    flex-shrink: 0;
  }

  .state-chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 7px;
    background: rgba(122, 162, 247, 0.1);
    border: 1px solid rgba(122, 162, 247, 0.25);
    border-radius: 10px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--cyan);
    cursor: default;
  }

  .state-chip.chip-clear {
    background: rgba(247, 118, 142, 0.1);
    border-color: rgba(247, 118, 142, 0.25);
    color: var(--red);
  }

  .chip-arrow {
    color: var(--text-dim);
    font-size: 10px;
  }

  .chip-value {
    color: var(--text);
  }

  .chip-clear-mark {
    color: var(--red);
  }

  .card-actions {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }

  .action-btn {
    padding: 2px 8px;
    font-size: 11px;
    border-radius: 10px;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text-dim);
    cursor: pointer;
    white-space: nowrap;
  }

  .action-btn:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  .action-btn.active {
    background: rgba(122, 162, 247, 0.12);
    border-color: rgba(122, 162, 247, 0.3);
    color: var(--accent);
  }

  /* Raw log line */
  .raw-line {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-dim);
    background: var(--bg);
    border-radius: var(--radius);
    padding: 6px 10px;
    white-space: pre;
    overflow-x: auto;
    margin-top: 2px;
  }

  /* Inline state panel */
  .state-panel {
    margin-top: 4px;
    padding: 8px 10px;
    background: var(--bg);
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }

  .state-panel-header {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-dim);
    margin-bottom: 6px;
  }

  .state-panel-empty {
    font-size: 12px;
    color: var(--text-dim);
  }

  .state-source-group {
    margin-bottom: 8px;
  }

  .state-source-name {
    font-size: 12px;
    font-weight: 600;
    display: block;
    margin-bottom: 3px;
  }

  .state-entries {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .state-kv {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .state-k {
    color: var(--cyan);
  }

  .state-eq {
    color: var(--text-dim);
  }

  .state-v {
    color: var(--text);
  }

  .pm-set-at {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-dim);
    margin-left: 4px;
  }

  .rule-panel {
    margin-top: 6px;
    padding: 8px 12px;
    background: var(--bg-secondary);
    border-radius: var(--radius);
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .rule-panel-name {
    font-weight: 600;
    color: var(--accent);
  }

  .rule-panel-mode {
    color: var(--text-dim);
  }

  .rule-mode-badge {
    font-family: var(--font-mono);
    color: var(--cyan);
  }

  .rule-panel-patterns {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-top: 2px;
  }

  .rule-pattern {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    background: var(--bg);
    padding: 2px 6px;
    border-radius: 3px;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .rule-panel-missing {
    color: var(--text-muted);
    font-style: italic;
  }
</style>
