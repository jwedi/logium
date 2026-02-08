<script lang="ts">
  import type { RuleMatch, PatternMatch, StateValue, Source, LogRule, Pattern } from './api';

  interface TimelineEvent {
    id: number;
    type: 'rule' | 'pattern';
    timestamp: number;
    sourceId: number | null;
    ruleId?: number;
    patternId?: number;
    ruleMatch?: RuleMatch;
    patternMatch?: PatternMatch;
    colorIndex: number;
  }

  let {
    event,
    sourceList,
    ruleList,
    patternList,
    onClose,
  }: {
    event: TimelineEvent;
    sourceList: Source[];
    ruleList: LogRule[];
    patternList: Pattern[];
    onClose: () => void;
  } = $props();

  function getRuleName(id: number): string {
    return ruleList.find((r) => r.id === id)?.name ?? `Rule #${id}`;
  }

  function getPatternName(id: number): string {
    return patternList.find((p) => p.id === id)?.name ?? `Pattern #${id}`;
  }

  function getSourceName(id: number): string {
    return sourceList.find((s) => s.id === id)?.name ?? `Source #${id}`;
  }

  function formatStateValue(sv: StateValue): string {
    if ('String' in sv) return sv.String;
    if ('Integer' in sv) return String(sv.Integer);
    if ('Float' in sv) return String(sv.Float);
    if ('Bool' in sv) return String(sv.Bool);
    return '?';
  }

  function formatTimestamp(ms: number): string {
    const d = new Date(ms);
    return d.toISOString().replace('T', ' ').replace('Z', '');
  }
</script>

<div class="detail-panel">
  <div class="detail-header">
    <h3>{event.type === 'rule' ? 'Rule Match' : 'Pattern Match'}</h3>
    <button class="close-btn" onclick={onClose}>x</button>
  </div>

  <div class="detail-time">{formatTimestamp(event.timestamp)}</div>

  {#if event.type === 'rule' && event.ruleMatch}
    {@const rm = event.ruleMatch}
    <div class="detail-section">
      <div class="detail-label">Rule</div>
      <div class="detail-value rule-name" style="color: var(--rule-border-{event.colorIndex})">
        {getRuleName(rm.rule_id)}
      </div>
    </div>
    <div class="detail-section">
      <div class="detail-label">Source</div>
      <div class="detail-value source-name">{getSourceName(rm.source_id)}</div>
    </div>
    <div class="detail-section">
      <div class="detail-label">Log Line</div>
      <code class="detail-log-line">{rm.log_line.content || rm.log_line.raw}</code>
    </div>
    {#if Object.keys(rm.extracted_state).length > 0}
      <div class="detail-section">
        <div class="detail-label">Extracted State</div>
        <div class="state-table">
          {#each Object.entries(rm.extracted_state) as [key, val]}
            <div class="state-entry">
              <span class="state-key">{key}</span>
              <span class="state-value">{formatStateValue(val)}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {/if}

  {#if event.type === 'pattern' && event.patternMatch}
    {@const pm = event.patternMatch}
    <div class="detail-section">
      <div class="detail-label">Pattern</div>
      <div class="detail-value" style="color: var(--purple)">{getPatternName(pm.pattern_id)}</div>
    </div>
    <div class="detail-section">
      <div class="detail-label">State Snapshot</div>
      {#each Object.entries(pm.state_snapshot) as [sourceName, stateMap]}
        <div class="pm-source">
          <span class="pm-source-name">{sourceName}</span>
          {#each Object.entries(stateMap) as [key, val]}
            <div class="state-entry">
              <span class="state-key">{key}</span>
              <span class="state-value">{formatStateValue(val)}</span>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .detail-panel {
    width: 300px;
    min-width: 300px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-left: none;
    border-radius: 0 var(--radius) var(--radius) 0;
    padding: 16px;
    overflow-y: auto;
    position: relative;
  }

  .detail-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .detail-header h3 {
    margin: 0;
  }

  .close-btn {
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 16px;
    padding: 2px 6px;
  }

  .detail-time {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-dim);
    margin-bottom: 16px;
  }

  .detail-section {
    margin-bottom: 14px;
  }

  .detail-label {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 4px;
  }

  .detail-value {
    font-weight: 600;
    font-size: 14px;
  }

  .source-name {
    color: var(--cyan);
  }

  .detail-log-line {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
    display: block;
    background: var(--bg);
    border-radius: var(--radius);
    padding: 8px;
    overflow-x: auto;
    white-space: pre;
    word-break: break-all;
  }

  .state-table {
    background: var(--bg);
    border-radius: var(--radius);
    padding: 8px;
  }

  .state-entry {
    display: flex;
    justify-content: space-between;
    padding: 3px 0;
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }

  .state-entry:last-child {
    border-bottom: none;
  }

  .state-key {
    font-family: var(--font-mono);
    color: var(--cyan);
  }

  .state-value {
    font-family: var(--font-mono);
    color: var(--text);
  }

  .pm-source {
    margin-bottom: 10px;
  }

  .pm-source-name {
    font-weight: 600;
    font-size: 12px;
    color: var(--cyan);
    display: block;
    margin-bottom: 4px;
  }
</style>
