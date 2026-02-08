<script lang="ts">
  import { rules as rulesApi, type Source, type LogRule, type RuleMatch, type PatternMatch, type StateValue } from './api';
  import RuleCreator from './RuleCreator.svelte';

  let { source, projectId, ruleMatches = [], patternMatches = [] }: {
    source: Source;
    projectId: number;
    ruleMatches?: RuleMatch[];
    patternMatches?: PatternMatch[];
  } = $props();

  const LINE_HEIGHT = 22;
  const OVERSCAN = 10;

  let lines: string[] = $state([]);
  let container: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let containerHeight = $state(600);
  let showRuleCreator = $state(false);
  let selectedText = $state('');
  let popupPos = $state({ x: 0, y: 0 });
  let selectedLineIdx: number | null = $state(null);
  let allRules: LogRule[] = $state([]);

  let totalHeight = $derived(lines.length * LINE_HEIGHT);
  let startIdx = $derived(Math.max(0, Math.floor(scrollTop / LINE_HEIGHT) - OVERSCAN));
  let endIdx = $derived(Math.min(lines.length, Math.ceil((scrollTop + containerHeight) / LINE_HEIGHT) + OVERSCAN));
  let visibleLines = $derived(lines.slice(startIdx, endIdx));
  let offsetY = $derived(startIdx * LINE_HEIGHT);

  // Build a map of line content -> rule matches for highlighting
  let lineMatchMap = $derived.by(() => {
    const map = new Map<number, { ruleId: number; match: RuleMatch }[]>();
    for (const m of ruleMatches) {
      if (m.source_id !== source.id) continue;
      // Try to find the line index by raw content match
      const idx = lines.findIndex(l => l === m.log_line.raw);
      if (idx >= 0) {
        if (!map.has(idx)) map.set(idx, []);
        map.get(idx)!.push({ ruleId: m.rule_id, match: m });
      }
    }
    return map;
  });

  // Pattern match timestamps mapped to line indices
  let patternMatchLines = $derived.by(() => {
    const result: { lineIdx: number; match: PatternMatch }[] = [];
    for (const pm of patternMatches) {
      // Find nearest line by timestamp (simplified: just show at top)
      result.push({ lineIdx: 0, match: pm });
    }
    return result;
  });

  function getRuleColor(ruleId: number): number {
    return ruleId % 6;
  }

  function formatStateValue(sv: StateValue): string {
    if ('String' in sv) return sv.String;
    if ('Integer' in sv) return String(sv.Integer);
    if ('Float' in sv) return String(sv.Float);
    if ('Bool' in sv) return String(sv.Bool);
    return '?';
  }

  function onScroll() {
    if (container) {
      scrollTop = container.scrollTop;
    }
  }

  function onMouseUp(event: MouseEvent) {
    const sel = window.getSelection();
    const text = sel?.toString().trim();
    if (text && text.length > 0) {
      selectedText = text;
      popupPos = { x: event.clientX, y: event.clientY };
      showRuleCreator = false;
      // Show mini popup
      const popup = document.getElementById('selection-popup');
      if (popup) {
        popup.style.display = 'block';
        popup.style.left = `${event.clientX}px`;
        popup.style.top = `${event.clientY - 40}px`;
      }
    } else {
      const popup = document.getElementById('selection-popup');
      if (popup) popup.style.display = 'none';
    }
  }

  function openRuleCreator() {
    showRuleCreator = true;
    const popup = document.getElementById('selection-popup');
    if (popup) popup.style.display = 'none';
  }

  function onRuleCreated() {
    showRuleCreator = false;
    loadRules();
  }

  async function loadRules() {
    try {
      allRules = await rulesApi.list(projectId);
    } catch { /* ignore */ }
  }

  function onLineClick(globalIdx: number) {
    selectedLineIdx = selectedLineIdx === globalIdx ? null : globalIdx;
  }

  // Simulate reading file lines from the source file_path
  // In real usage, the backend would serve file content
  async function loadFileContent() {
    if (!source.file_path) {
      lines = ['(No file uploaded for this source)'];
      return;
    }
    try {
      const res = await fetch(`/api/projects/${projectId}/sources/${source.id}/content`);
      if (res.ok) {
        const text = await res.text();
        lines = text.split('\n');
      } else {
        lines = [`(Could not load file: ${res.status})`];
      }
    } catch {
      lines = ['(Could not load file content)'];
    }
  }

  $effect(() => {
    source;
    loadFileContent();
    loadRules();
  });

  $effect(() => {
    if (container) {
      const obs = new ResizeObserver(entries => {
        for (const entry of entries) {
          containerHeight = entry.contentRect.height;
        }
      });
      obs.observe(container);
      return () => obs.disconnect();
    }
  });
</script>

<div class="log-viewer-wrapper">
  <div
    class="log-viewer"
    bind:this={container}
    onscroll={onScroll}
    onmouseup={onMouseUp}
    role="log"
  >
    <div class="scroll-spacer" style="height: {totalHeight}px">
      <div class="visible-lines" style="transform: translateY({offsetY}px)">
        {#each visibleLines as line, i}
          {@const globalIdx = startIdx + i}
          {@const matches = lineMatchMap.get(globalIdx)}
          <div
            class="log-line"
            class:highlighted={!!matches}
            class:selected={selectedLineIdx === globalIdx}
            style={matches ? `background: var(--rule-color-${getRuleColor(matches[0].ruleId)}); border-left: 3px solid var(--rule-border-${getRuleColor(matches[0].ruleId)})` : ''}
            onclick={() => onLineClick(globalIdx)}
            role="button"
            tabindex="0"
          >
            <span class="line-number">{globalIdx + 1}</span>
            <span class="line-content">{line}</span>
          </div>
        {/each}
      </div>
    </div>
  </div>

  {#if selectedLineIdx !== null && lineMatchMap.has(selectedLineIdx)}
    <div class="state-panel">
      <h3>Extracted State</h3>
      <button class="close-btn" onclick={() => selectedLineIdx = null}>x</button>
      {#each lineMatchMap.get(selectedLineIdx)! as { ruleId, match }}
        <div class="state-group">
          <div class="state-rule-name">
            Rule #{ruleId}
            {#each allRules as r}
              {#if r.id === ruleId} - {r.name}{/if}
            {/each}
          </div>
          {#each Object.entries(match.extracted_state) as [key, val]}
            <div class="state-entry">
              <span class="state-key">{key}</span>
              <span class="state-value">{formatStateValue(val)}</span>
            </div>
          {/each}
          {#if Object.keys(match.extracted_state).length === 0}
            <div class="state-empty">No state extracted</div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<div id="selection-popup" class="selection-popup" style="display: none">
  <button class="primary" onclick={openRuleCreator}>Create Rule from Selection</button>
</div>

{#if showRuleCreator}
  <RuleCreator
    {projectId}
    {selectedText}
    onClose={() => showRuleCreator = false}
    onCreated={onRuleCreated}
  />
{/if}

<style>
  .log-viewer-wrapper {
    display: flex;
    gap: 0;
    height: calc(100vh - 200px);
    min-height: 400px;
  }

  .log-viewer {
    flex: 1;
    overflow-y: auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 22px;
    position: relative;
    user-select: text;
  }

  .scroll-spacer {
    position: relative;
  }

  .visible-lines {
    position: absolute;
    left: 0;
    right: 0;
  }

  .log-line {
    display: flex;
    padding: 0 8px;
    border-left: 3px solid transparent;
    cursor: pointer;
    white-space: pre;
  }

  .log-line:hover {
    background: var(--bg-secondary) !important;
  }

  .log-line.selected {
    background: var(--bg-tertiary) !important;
  }

  .line-number {
    flex-shrink: 0;
    width: 50px;
    text-align: right;
    padding-right: 12px;
    color: var(--text-muted);
    user-select: none;
  }

  .line-content {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .state-panel {
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

  .close-btn {
    position: absolute;
    top: 12px;
    right: 12px;
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 16px;
    padding: 2px 6px;
  }

  .state-group {
    margin-bottom: 16px;
  }

  .state-rule-name {
    font-weight: 600;
    font-size: 13px;
    margin-bottom: 8px;
    color: var(--accent);
  }

  .state-entry {
    display: flex;
    justify-content: space-between;
    padding: 4px 0;
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }

  .state-key {
    font-family: var(--font-mono);
    color: var(--cyan);
  }

  .state-value {
    font-family: var(--font-mono);
    color: var(--text);
  }

  .state-empty {
    color: var(--text-muted);
    font-size: 12px;
    font-style: italic;
  }

  .selection-popup {
    position: fixed;
    z-index: 1000;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 4px;
    box-shadow: 0 4px 12px rgba(0,0,0,0.4);
  }
</style>
