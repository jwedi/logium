<script lang="ts">
  import { tick } from 'svelte';
  import {
    rules as rulesApi,
    sources as sourcesApi,
    type Source,
    type LogRule,
    type RuleMatch,
    type PatternMatch,
    type StateChange,
    type StateValue,
  } from './api';
  import RuleCreator from './RuleCreator.svelte';
  import { formatTimestamp } from './formatUtils';
  import { validateRegex } from './regexUtils';

  let {
    source,
    projectId,
    ruleMatches = [],
    patternMatches = [],
    stateChanges = [],
    navigateTarget = null,
    sourceList = [],
  }: {
    source: Source;
    projectId: number;
    ruleMatches?: RuleMatch[];
    patternMatches?: PatternMatch[];
    stateChanges?: StateChange[];
    navigateTarget?: { raw: string; seq: number } | null;
    sourceList?: Source[];
  } = $props();

  const LINE_HEIGHT = 22;
  const OVERSCAN = 10;

  const PAGE_SIZE = 500;

  let totalLines: number = $state(0);
  let pageMap: Map<number, string[]> = $state(new Map());
  let loadingPages: boolean = $state(false);
  let abortController: AbortController | null = null;

  function getLine(idx: number): string {
    const page = pageMap.get(Math.floor(idx / PAGE_SIZE));
    return page?.[idx % PAGE_SIZE] ?? '';
  }

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

  // Whether full data is needed (filter, search, or analysis active)
  let needsAllLines: boolean = $derived(!!filterQuery || !!searchQuery || ruleMatches.length > 0);

  let filterError: string | null = $derived(
    filterIsRegex && filterQuery ? validateRegex(filterQuery) : null,
  );
  let searchError: string | null = $derived(
    searchIsRegex && searchQuery ? validateRegex(searchQuery) : null,
  );

  // Context expansion state
  let expandedLines: Set<number> = $state(new Set());
  let contextSize: number = $state(5);

  // Guard: when true, the filter-reset effect should not scroll to top
  let navigateInProgress = false;

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
      return Array.from({ length: totalLines }, (_, i) => i);
    }
    const result: number[] = [];
    const sortedPages = [...pageMap.keys()].sort((a, b) => a - b);
    for (const pageNum of sortedPages) {
      const page = pageMap.get(pageNum)!;
      const offset = pageNum * PAGE_SIZE;
      for (let i = 0; i < page.length; i++) {
        filterRegex.lastIndex = 0;
        if (filterRegex.test(page[i])) result.push(offset + i);
      }
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
      const hi = Math.min(totalLines - 1, lineIdx + contextSize);
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

  // Reverse lookup: line content -> first occurrence index (O(M) build, O(1) lookup)
  // Strip trailing \r so Windows-formatted files match engine output (which strips \r)
  let lineContentIndex = $derived.by(() => {
    const map = new Map<string, number>();
    for (const [pageNum, page] of pageMap) {
      const offset = pageNum * PAGE_SIZE;
      for (let i = 0; i < page.length; i++) {
        const key = page[i].replace(/\r$/, '');
        if (!map.has(key)) map.set(key, offset + i);
      }
    }
    return map;
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
      text: getLine(origIdx),
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
      if (searchRegex.test(getLine(filteredIndices[fi]))) result.push(fi);
    }
    return result;
  });

  let searchMatchSet = $derived(new Set(searchMatches.map((fi) => filteredIndices[fi])));

  // Build a map of line content -> rule matches for highlighting
  let lineMatchMap = $derived.by(() => {
    const map = new Map<number, { ruleId: number; match: RuleMatch }[]>();
    for (const m of ruleMatches) {
      if (m.source_id !== source.id) continue;
      // Strip trailing \r to match lineContentIndex keys
      const idx = lineContentIndex.get(m.log_line.raw.replace(/\r$/, '')) ?? -1;
      if (idx >= 0) {
        if (!map.has(idx)) map.set(idx, []);
        map.get(idx)!.push({ ruleId: m.rule_id, match: m });
      }
    }
    return map;
  });

  // Sorted array of { lineIndex, timestamp } for timestamp resolution via binary search
  let matchTimestampIndex = $derived.by(() => {
    const entries: { lineIndex: number; timestamp: string }[] = [];
    for (const m of ruleMatches) {
      if (m.source_id !== source.id) continue;
      const idx = lineContentIndex.get(m.log_line.raw.replace(/\r$/, '')) ?? -1;
      if (idx >= 0) {
        entries.push({ lineIndex: idx, timestamp: m.log_line.timestamp });
      }
    }
    entries.sort((a, b) => a.lineIndex - b.lineIndex);
    return entries;
  });

  function resolveTimestamp(lineIdx: number): string | null {
    // Matched line: use exact timestamp
    const matches = lineMatchMap.get(lineIdx);
    if (matches && matches.length > 0) {
      return matches[0].match.log_line.timestamp;
    }
    // Unmatched line: binary search for nearest matched line at or before this position
    const arr = matchTimestampIndex;
    if (arr.length === 0) return null;
    let lo = 0;
    let hi = arr.length - 1;
    let best = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >>> 1;
      if (arr[mid].lineIndex <= lineIdx) {
        best = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    return best >= 0 ? arr[best].timestamp : null;
  }

  // Accumulated state at the selected line's timestamp
  let accumulatedState = $derived.by(() => {
    if (selectedLineIdx === null || stateChanges.length === 0) return null;
    const ts = resolveTimestamp(selectedLineIdx);
    if (ts === null) return null;
    const stateMap = new Map<string, Map<string, StateValue>>();
    for (const sc of stateChanges) {
      if (sc.timestamp > ts) break;
      if (sc.new_value === null) {
        // Deletion
        const sourceMap = stateMap.get(sc.source_name);
        if (sourceMap) sourceMap.delete(sc.state_key);
      } else {
        let sourceMap = stateMap.get(sc.source_name);
        if (!sourceMap) {
          sourceMap = new Map();
          stateMap.set(sc.source_name, sourceMap);
        }
        sourceMap.set(sc.state_key, sc.new_value);
      }
    }
    // Remove empty sources
    for (const [k, v] of stateMap) {
      if (v.size === 0) stateMap.delete(k);
    }
    return { timestamp: ts, state: stateMap };
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

  function getSourceColor(name: string): string {
    return sourceList.find((s) => s.name === name)?.color ?? 'var(--text-dim)';
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

  async function loadFirstPage() {
    abortController?.abort();
    abortController = new AbortController();
    const signal = abortController.signal;

    pageMap = new Map();
    totalLines = 0;
    loadingPages = false;

    if (!source.file_path) {
      totalLines = 1;
      pageMap = new Map([[0, ['(No file uploaded for this source)']]]);
      return;
    }
    try {
      const resp = await sourcesApi.rawLines(projectId, source.id, 0, PAGE_SIZE, signal);
      if (signal.aborted) return;
      totalLines = resp.total_lines;
      pageMap = new Map([[0, resp.lines]]);
    } catch {
      if (abortController?.signal.aborted) return;
      totalLines = 1;
      pageMap = new Map([[0, ['(Could not load file content)']]]);
    }
  }

  async function loadAllRemainingPages(signal: AbortSignal) {
    const totalPages = Math.ceil(totalLines / PAGE_SIZE);
    if (pageMap.size >= totalPages) return;
    loadingPages = true;
    const CONCURRENCY = 3;
    for (let batch = 0; batch < totalPages; batch += CONCURRENCY) {
      if (signal.aborted) break;
      const fetches = [];
      for (let p = batch; p < Math.min(batch + CONCURRENCY, totalPages); p++) {
        if (pageMap.has(p)) continue;
        fetches.push(
          sourcesApi
            .rawLines(projectId, source.id, p * PAGE_SIZE, PAGE_SIZE, signal)
            .then((resp) => ({ page: p, lines: resp.lines })),
        );
      }
      if (fetches.length === 0) continue;
      try {
        const results = await Promise.all(fetches);
        if (signal.aborted) break;
        const next = new Map(pageMap);
        for (const { page, lines } of results) next.set(page, lines);
        pageMap = next;
      } catch {
        break;
      }
    }
    if (!signal.aborted) loadingPages = false;
  }

  async function ensureViewportPages(signal: AbortSignal) {
    if (totalLines === 0 || needsAllLines) return;
    const viewStart = Math.max(0, Math.floor(scrollTop / LINE_HEIGHT) - OVERSCAN);
    const viewEnd = Math.ceil((scrollTop + containerHeight) / LINE_HEIGHT) + OVERSCAN;
    const startPage = Math.max(0, Math.floor(viewStart / PAGE_SIZE) - 1);
    const endPage = Math.min(Math.ceil(totalLines / PAGE_SIZE) - 1, Math.ceil(viewEnd / PAGE_SIZE));
    const missing = [];
    for (let p = startPage; p <= endPage; p++) {
      if (!pageMap.has(p)) missing.push(p);
    }
    if (missing.length === 0) return;
    try {
      const results = await Promise.all(
        missing.map((p) =>
          sourcesApi
            .rawLines(projectId, source.id, p * PAGE_SIZE, PAGE_SIZE, signal)
            .then((resp) => ({ page: p, lines: resp.lines })),
        ),
      );
      if (signal.aborted) return;
      const next = new Map(pageMap);
      for (const { page, lines } of results) next.set(page, lines);
      pageMap = next;
    } catch {
      /* ignore */
    }
  }

  $effect(() => {
    source;
    expandedLines = new Set();
    loadFirstPage();
    loadRules();
  });

  // Scroll-driven page fetching (debounced, windowed mode only)
  let scrollFetchTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    scrollTop;
    containerHeight;
    if (needsAllLines || !abortController) return;
    if (scrollFetchTimer) clearTimeout(scrollFetchTimer);
    scrollFetchTimer = setTimeout(() => {
      if (abortController && !abortController.signal.aborted) {
        ensureViewportPages(abortController.signal);
      }
    }, 100);
  });

  // Load all pages when filter/search/analysis activates
  $effect(() => {
    if (
      needsAllLines &&
      totalLines > PAGE_SIZE &&
      abortController &&
      !abortController.signal.aborted
    ) {
      loadAllRemainingPages(abortController.signal);
    }
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

  // Navigate to a specific line when navigateTarget is set (from timeline/histogram click)
  $effect(() => {
    if (!navigateTarget || totalLines === 0) return;
    const _seq = navigateTarget.seq; // read seq so Svelte re-fires on every new request
    const raw = navigateTarget.raw;
    const origIdx = lineContentIndex.get(raw) ?? -1;
    if (origIdx < 0 || !container) return;

    selectedLineIdx = origIdx;
    let filteredPos = filteredIndices.indexOf(origIdx);
    if (filteredPos < 0 && filterQuery) {
      navigateInProgress = true;
      filterQuery = '';
      tick().then(() => {
        if (container) {
          container.scrollTop = origIdx * LINE_HEIGHT - containerHeight / 2 + LINE_HEIGHT / 2;
          scrollTop = container.scrollTop;
        }
        navigateInProgress = false;
      });
      return;
    }
    if (filteredPos >= 0) {
      container.scrollTop = filteredPos * LINE_HEIGHT - containerHeight / 2 + LINE_HEIGHT / 2;
      scrollTop = container.scrollTop;
    }
  });

  // Reset scroll on filter change
  $effect(() => {
    filterQuery;
    filterIsRegex;
    expandedLines = new Set();
    if (container && !navigateInProgress) {
      container.scrollTop = 0;
      scrollTop = 0;
    }
  });

  // Deselect line if filtered out
  let filteredIndexSet = $derived(new Set(filteredIndices));
  $effect(() => {
    if (selectedLineIdx !== null && filterQuery) {
      const inFiltered = filteredIndexSet.has(selectedLineIdx);
      if (!inFiltered) selectedLineIdx = null;
    }
  });
</script>

<svelte:window onkeydown={onSearchKeydown} />

<div class="log-viewer-root">
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
          {filterQuery ? `${baseFilteredIndices.length} of ${totalLines} lines` : ''}
        </span>
        {#if loadingPages}<span class="loading-indicator">Loading...</span>{/if}
        {#if filterError}<span class="regex-error" title={filterError}>Invalid regex</span>{/if}
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
          {#if searchError}<span class="regex-error" title={searchError}>Invalid regex</span>{/if}
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
  </div>

  {#if selectedLineIdx !== null && (lineMatchMap.has(selectedLineIdx) || accumulatedState !== null)}
    <div class="state-panel">
      <button class="close-btn" onclick={() => (selectedLineIdx = null)}>x</button>
      {#if lineMatchMap.has(selectedLineIdx)}
        <h3>Extracted State</h3>
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
      {/if}
      {#if accumulatedState !== null}
        {#if lineMatchMap.has(selectedLineIdx)}
          <div class="state-section-divider"></div>
        {/if}
        <h3 class="state-section-header" title={accumulatedState.timestamp}>
          State at {formatTimestamp(accumulatedState.timestamp)}
        </h3>
        {#if accumulatedState.state.size === 0}
          <div class="state-empty">No state mutations before this point</div>
        {:else}
          {#each [...accumulatedState.state.entries()].sort((a, b) => {
            if (a[0] === source.name) return -1;
            if (b[0] === source.name) return 1;
            return a[0].localeCompare(b[0]);
          }) as [sourceName, entries]}
            <div class="state-group">
              <div class="state-source-name" style="color:{getSourceColor(sourceName)}">
                {sourceName}
              </div>
              {#each [...entries.entries()] as [key, val]}
                <div class="state-entry">
                  <span class="state-key">{key}</span>
                  <span class="state-value">{formatStateValue(val)}</span>
                </div>
              {/each}
            </div>
          {/each}
        {/if}
      {/if}
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
  .log-viewer-root {
    position: relative;
    height: 100%;
  }

  .log-viewer-wrapper {
    display: flex;
    height: 100%;
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
    overflow-x: auto;
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
    position: sticky;
    left: 8px;
    z-index: 1;
    background: var(--bg);
  }

  .log-line:hover .line-number {
    background: var(--bg-secondary);
  }

  .log-line.selected .line-number {
    background: var(--bg-tertiary);
  }

  .line-content {
    flex: 1;
  }

  .state-panel {
    position: absolute;
    top: 0;
    right: 0;
    width: 300px;
    max-height: calc(100vh - 160px);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px;
    overflow-y: auto;
    z-index: 10;
    box-shadow: -2px 0 8px rgba(0, 0, 0, 0.15);
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

  .state-section-divider {
    border-top: 1px solid var(--border);
    margin: 12px 0;
  }

  .state-section-header {
    font-size: 13px;
    margin: 0 0 8px 0;
  }

  .state-source-name {
    font-weight: 600;
    font-size: 13px;
    margin-bottom: 8px;
    color: var(--cyan);
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

  .loading-indicator {
    font-size: 12px;
    color: var(--text-muted);
  }

  .regex-error {
    font-size: 12px;
    color: var(--red);
    white-space: nowrap;
  }
</style>
