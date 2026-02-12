<script lang="ts">
  import { tick } from 'svelte';
  import {
    rules as rulesApi,
    type Source,
    type LogRule,
    type RuleMatch,
    type PatternMatch,
    type StateValue,
  } from './api';
  import RuleCreator from './RuleCreator.svelte';

  let {
    source,
    projectId,
    ruleMatches = [],
    patternMatches = [],
    navigateTarget = null,
  }: {
    source: Source;
    projectId: number;
    ruleMatches?: RuleMatch[];
    patternMatches?: PatternMatch[];
    navigateTarget?: string | null;
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

  // Search state
  let searchOpen = $state(false);
  let searchQuery = $state('');
  let searchIsRegex = $state(false);
  let currentMatchIdx = $state(0);
  let searchInput: HTMLInputElement | undefined = $state();

  // Filter state
  let filterQuery = $state('');
  let filterIsRegex = $state(false);

  // Context expansion state
  let expandedLines: Set<number> = $state(new Set());
  let contextSize: number = $state(5);

  // Filter: compiled regex
  let filterRegex: RegExp | null = $derived.by(() => {
    if (!filterQuery) return null;
    try {
      if (filterIsRegex) return new RegExp(filterQuery, 'gi');
      return new RegExp(filterQuery.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi');
    } catch {
      return null;
    }
  });

  // Base filter: indices passing the text/regex filter
  let baseFilteredIndices: number[] = $derived.by(() => {
    if (!filterQuery || !filterRegex) {
      return Array.from({ length: lines.length }, (_, i) => i);
    }
    const result: number[] = [];
    for (let i = 0; i < lines.length; i++) {
      filterRegex.lastIndex = 0;
      if (filterRegex.test(lines[i])) result.push(i);
    }
    return result;
  });

  // Final filtered indices: base + context around expanded matched lines
  let filteredIndices: number[] = $derived.by(() => {
    if (expandedLines.size === 0) return baseFilteredIndices;
    const baseSet = new Set(baseFilteredIndices);
    const additions: number[] = [];
    for (const lineIdx of expandedLines) {
      if (!lineMatchMap.has(lineIdx) || !baseSet.has(lineIdx)) continue;
      const lo = Math.max(0, lineIdx - contextSize);
      const hi = Math.min(lines.length - 1, lineIdx + contextSize);
      for (let i = lo; i <= hi; i++) {
        if (!baseSet.has(i)) additions.push(i);
      }
    }
    if (additions.length === 0) return baseFilteredIndices;
    const merged = [...baseFilteredIndices, ...additions];
    merged.sort((a, b) => a - b);
    const deduped: number[] = [merged[0]];
    for (let i = 1; i < merged.length; i++) {
      if (merged[i] !== merged[i - 1]) deduped.push(merged[i]);
    }
    return deduped;
  });

  // Lines that are context-only (for CSS styling)
  let contextLineSet: Set<number> = $derived.by(() => {
    if (expandedLines.size === 0) return new Set();
    const baseSet = new Set(baseFilteredIndices);
    const ctx = new Set<number>();
    for (const idx of filteredIndices) {
      if (!baseSet.has(idx)) ctx.add(idx);
    }
    return ctx;
  });

  let totalHeight = $derived(filteredIndices.length * LINE_HEIGHT);
  let startIdx = $derived(Math.max(0, Math.floor(scrollTop / LINE_HEIGHT) - OVERSCAN));
  let endIdx = $derived(
    Math.min(
      filteredIndices.length,
      Math.ceil((scrollTop + containerHeight) / LINE_HEIGHT) + OVERSCAN,
    ),
  );
  let visibleLines = $derived(
    filteredIndices.slice(startIdx, endIdx).map((origIdx, i) => ({
      origIdx,
      text: lines[origIdx],
      gapBefore:
        i === 0
          ? startIdx > 0 && origIdx !== filteredIndices[startIdx - 1] + 1
          : origIdx !== filteredIndices[startIdx + i - 1] + 1,
    })),
  );
  let offsetY = $derived(startIdx * LINE_HEIGHT);

  // Search: compiled regex for matching and highlighting
  let searchRegex: RegExp | null = $derived.by(() => {
    if (!searchQuery) return null;
    try {
      if (searchIsRegex) {
        return new RegExp(searchQuery, 'gi');
      }
      return new RegExp(searchQuery.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi');
    } catch {
      return null;
    }
  });

  // Search: array of matching filtered positions
  let searchMatches: number[] = $derived.by(() => {
    if (!searchQuery || !searchRegex) return [];
    const result: number[] = [];
    for (let fi = 0; fi < filteredIndices.length; fi++) {
      searchRegex.lastIndex = 0;
      if (searchRegex.test(lines[filteredIndices[fi]])) result.push(fi);
    }
    return result;
  });

  let searchMatchSet = $derived(new Set(searchMatches.map((fi) => filteredIndices[fi])));

  // Build a map of line content -> rule matches for highlighting
  let lineMatchMap = $derived.by(() => {
    const map = new Map<number, { ruleId: number; match: RuleMatch }[]>();
    for (const m of ruleMatches) {
      if (m.source_id !== source.id) continue;
      // Try to find the line index by raw content match
      const idx = lines.findIndex((l) => l === m.log_line.raw);
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
    } catch {
      /* ignore */
    }
  }

  function onLineClick(globalIdx: number) {
    selectedLineIdx = selectedLineIdx === globalIdx ? null : globalIdx;
  }

  // Search functions
  function onSearchKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
      e.preventDefault();
      searchOpen = true;
      tick().then(() => searchInput?.focus());
    }
    if (e.key === 'Escape' && searchOpen) {
      closeSearch();
    }
  }

  function onSearchInputKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (e.shiftKey) prevMatch();
      else nextMatch();
    }
  }

  function nextMatch() {
    if (searchMatches.length === 0) return;
    currentMatchIdx = (currentMatchIdx + 1) % searchMatches.length;
    scrollToMatch();
  }

  function prevMatch() {
    if (searchMatches.length === 0) return;
    currentMatchIdx = (currentMatchIdx - 1 + searchMatches.length) % searchMatches.length;
    scrollToMatch();
  }

  function scrollToMatch() {
    const filteredPos = searchMatches[currentMatchIdx];
    if (container && filteredPos !== undefined) {
      container.scrollTop = filteredPos * LINE_HEIGHT - containerHeight / 2 + LINE_HEIGHT / 2;
    }
  }

  function closeSearch() {
    searchOpen = false;
    searchQuery = '';
    currentMatchIdx = 0;
  }

  function toggleExpand(lineIdx: number, event: MouseEvent) {
    event.stopPropagation();
    const next = new Set(expandedLines);
    if (next.has(lineIdx)) next.delete(lineIdx);
    else next.add(lineIdx);
    expandedLines = next;
  }

  function expandAll() {
    const next = new Set(expandedLines);
    for (const idx of baseFilteredIndices) {
      if (lineMatchMap.has(idx)) next.add(idx);
    }
    expandedLines = next;
  }

  function collapseAll() {
    expandedLines = new Set();
  }

  function splitLineByRegex(
    line: string,
    regex: RegExp | null,
  ): { text: string; isMatch: boolean }[] {
    if (!regex) return [{ text: line, isMatch: false }];
    const segments: { text: string; isMatch: boolean }[] = [];
    let lastIndex = 0;
    regex.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = regex.exec(line)) !== null) {
      if (m.index > lastIndex) {
        segments.push({ text: line.slice(lastIndex, m.index), isMatch: false });
      }
      segments.push({ text: m[0], isMatch: true });
      lastIndex = regex.lastIndex;
      if (m[0].length === 0) {
        regex.lastIndex++;
      }
    }
    if (lastIndex < line.length) {
      segments.push({ text: line.slice(lastIndex), isMatch: false });
    }
    return segments.length > 0 ? segments : [{ text: line, isMatch: false }];
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
    expandedLines = new Set();
    loadFileContent();
    loadRules();
  });

  $effect(() => {
    if (container) {
      const obs = new ResizeObserver((entries) => {
        for (const entry of entries) {
          containerHeight = entry.contentRect.height;
        }
      });
      obs.observe(container);
      return () => obs.disconnect();
    }
  });

  // Reset match index when search results change
  $effect(() => {
    searchMatches;
    currentMatchIdx = 0;
  });

  // Navigate to a specific line when navigateTarget is set (from timeline click)
  $effect(() => {
    if (navigateTarget && lines.length > 0) {
      const origIdx = lines.findIndex((l) => l === navigateTarget);
      if (origIdx >= 0 && container) {
        selectedLineIdx = origIdx;
        let filteredPos = filteredIndices.indexOf(origIdx);
        if (filteredPos < 0 && filterQuery) {
          filterQuery = '';
          tick().then(() => {
            if (container)
              container.scrollTop = origIdx * LINE_HEIGHT - containerHeight / 2 + LINE_HEIGHT / 2;
          });
          return;
        }
        if (filteredPos >= 0)
          container.scrollTop = filteredPos * LINE_HEIGHT - containerHeight / 2 + LINE_HEIGHT / 2;
      }
    }
  });

  // Reset scroll on filter change
  $effect(() => {
    filterQuery;
    filterIsRegex;
    expandedLines = new Set();
    if (container) {
      container.scrollTop = 0;
      scrollTop = 0;
    }
  });

  // Deselect line if filtered out
  $effect(() => {
    if (selectedLineIdx !== null && filterQuery) {
      const inFiltered = filteredIndices.includes(selectedLineIdx);
      if (!inFiltered) selectedLineIdx = null;
    }
  });
</script>

<svelte:window onkeydown={onSearchKeydown} />

<div class="log-viewer-wrapper">
  <div class="log-viewer-column">
    <div class="filter-bar">
      <input bind:value={filterQuery} placeholder="Filter lines..." />
      <button
        class="search-toggle"
        class:active={filterIsRegex}
        onclick={() => (filterIsRegex = !filterIsRegex)}
        title="Toggle filter regex">.*</button
      >
      <span class="filter-count">
        {filterQuery ? `${baseFilteredIndices.length} of ${lines.length} lines` : ''}
      </span>
      {#if filterQuery}
        <button
          onclick={() => {
            filterQuery = '';
          }}
          title="Clear filter">&#x2715;</button
        >
      {/if}
      {#if filterQuery && lineMatchMap.size > 0}
        <span class="context-controls">
          <label title="Context lines around expanded matches">
            Ctx:
            <input
              type="number"
              min="1"
              max="50"
              bind:value={contextSize}
              class="context-size-input"
            />
          </label>
          <button onclick={expandAll} title="Expand all matches">&#x25BC; All</button>
          <button onclick={collapseAll} title="Collapse all matches">&#x25B2; All</button>
        </span>
      {/if}
    </div>
    {#if searchOpen}
      <div class="search-bar">
        <input
          bind:this={searchInput}
          bind:value={searchQuery}
          onkeydown={onSearchInputKeydown}
          placeholder="Search..."
        />
        <button
          class="search-toggle"
          class:active={searchIsRegex}
          onclick={() => (searchIsRegex = !searchIsRegex)}
          title="Toggle regex">.*</button
        >
        <span class="match-count">
          {searchMatches.length > 0
            ? `${currentMatchIdx + 1} of ${searchMatches.length}`
            : searchQuery
              ? 'No matches'
              : ''}
        </span>
        <button onclick={prevMatch} title="Previous match">&#x2191;</button>
        <button onclick={nextMatch} title="Next match">&#x2193;</button>
        <button onclick={closeSearch} title="Close search">&#x2715;</button>
      </div>
    {/if}

    <div
      class="log-viewer"
      bind:this={container}
      onscroll={onScroll}
      onmouseup={onMouseUp}
      role="log"
    >
      <div class="scroll-spacer" style="height: {totalHeight}px">
        <div class="visible-lines" style="transform: translateY({offsetY}px)">
          {#each visibleLines as entry, i}
            {@const globalIdx = entry.origIdx}
            {@const matches = lineMatchMap.get(globalIdx)}
            <div
              class="log-line"
              class:highlighted={!!matches}
              class:selected={selectedLineIdx === globalIdx}
              class:context-line={contextLineSet.has(globalIdx)}
              class:gap-before={entry.gapBefore}
              class:current-search-match={searchMatches.length > 0 &&
                filteredIndices[searchMatches[currentMatchIdx]] === globalIdx}
              style={matches
                ? `background: var(--rule-color-${getRuleColor(matches[0].ruleId)}); border-left: 3px solid var(--rule-border-${getRuleColor(matches[0].ruleId)})`
                : ''}
              onclick={() => onLineClick(globalIdx)}
              role="button"
              tabindex="0"
            >
              {#if filterQuery && matches}
                <button
                  class="expand-toggle"
                  onclick={(e) => toggleExpand(globalIdx, e)}
                  title={expandedLines.has(globalIdx) ? 'Collapse context' : 'Expand context'}
                  >{expandedLines.has(globalIdx) ? '\u25BC' : '\u25B6'}</button
                >
              {:else}
                <span class="expand-spacer"></span>
              {/if}
              <span class="line-number">{globalIdx + 1}</span>
              <span class="line-content"
                >{#if filterQuery || searchMatchSet.has(globalIdx)}{#each splitLineByRegex(entry.text, filterRegex) as seg}{#if seg.isMatch}<mark
                        class="filter-highlight"
                        >{#if searchMatchSet.has(globalIdx)}{#each splitLineByRegex(seg.text, searchRegex) as sseg}{#if sseg.isMatch}<mark
                                class="search-highlight">{sseg.text}</mark
                              >{:else}{sseg.text}{/if}{/each}{:else}{seg.text}{/if}</mark
                      >{:else if searchMatchSet.has(globalIdx)}{#each splitLineByRegex(seg.text, searchRegex) as sseg}{#if sseg.isMatch}<mark
                            class="search-highlight">{sseg.text}</mark
                          >{:else}{sseg.text}{/if}{/each}{:else}{seg.text}{/if}{/each}{:else}{entry.text}{/if}</span
              >
            </div>
          {/each}
        </div>
      </div>
    </div>
  </div>

  {#if selectedLineIdx !== null && lineMatchMap.has(selectedLineIdx)}
    <div class="state-panel">
      <h3>Extracted State</h3>
      <button class="close-btn" onclick={() => (selectedLineIdx = null)}>x</button>
      {#each lineMatchMap.get(selectedLineIdx)! as { ruleId, match }}
        <div class="state-group">
          <div class="state-rule-name">
            Rule #{ruleId}
            {#each allRules as r}
              {#if r.id === ruleId}
                - {r.name}{/if}
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
    sourceTemplateId={source.template_id}
    onClose={() => (showRuleCreator = false)}
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

  .log-viewer-column {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
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
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
  }

  .filter-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .filter-bar input {
    flex: 1;
    min-width: 150px;
    font-family: var(--font-mono);
    font-size: 13px;
  }

  .filter-bar button {
    padding: 2px 8px;
    font-size: 13px;
  }

  .filter-bar button.active {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
  }

  .filter-count {
    font-size: 12px;
    color: var(--text-muted);
    min-width: 80px;
    text-align: center;
  }

  mark.filter-highlight {
    background: rgba(125, 207, 255, 0.15);
    border-radius: 2px;
    padding: 0 1px;
  }

  .search-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .search-bar input {
    flex: 1;
    min-width: 150px;
    font-family: var(--font-mono);
    font-size: 13px;
  }

  .search-bar button {
    padding: 2px 8px;
    font-size: 13px;
  }

  .search-bar button.active {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
  }

  .match-count {
    font-size: 12px;
    color: var(--text-muted);
    min-width: 80px;
    text-align: center;
  }

  mark.search-highlight {
    background: var(--yellow);
    color: var(--bg);
    border-radius: 2px;
    padding: 0 1px;
  }

  .log-line.current-search-match {
    background: rgba(224, 175, 104, 0.1) !important;
  }

  .expand-toggle {
    flex-shrink: 0;
    width: 18px;
    height: 22px;
    padding: 0;
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 10px;
    line-height: 22px;
    text-align: center;
    cursor: pointer;
    user-select: none;
  }
  .expand-toggle:hover {
    color: var(--accent);
  }

  .expand-spacer {
    flex-shrink: 0;
    width: 18px;
    display: inline-block;
  }

  .log-line.context-line {
    opacity: 0.65;
    border-left: 3px dashed var(--border) !important;
    background: none !important;
  }
  .log-line.context-line:hover {
    opacity: 1;
  }

  .log-line.gap-before {
    box-shadow: 0 -1px 0 0 var(--text-muted);
  }

  .context-controls {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: 8px;
  }
  .context-controls label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .context-size-input {
    width: 45px;
    padding: 2px 4px;
    font-size: 12px;
    text-align: center;
  }
</style>
