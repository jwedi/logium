<script lang="ts">
  import {
    analysis as analysisApi,
    sources as sourcesApi,
    rules as rulesApi,
    patterns as patternsApi,
    rulesets as rulesetsApi,
    type AnalysisResult,
    type RuleMatch,
    type PatternMatch,
    type StateChange,
    type Source,
    type LogRule,
    type Pattern,
    type Ruleset,
    type StateValue,
    type TimeRange,
  } from './api';
  import LogViewer from './LogViewer.svelte';
  import TimelineView from './TimelineView.svelte';
  import StateEvolutionView from './StateEvolutionView.svelte';
  import ErrorClusteringView from './ErrorClusteringView.svelte';
  import { getInvalidationStamp } from './analysisInvalidation.svelte';
  import { getCachedAnalysis, setCachedAnalysis } from './analysisCache.svelte';

  let { projectId }: { projectId: number } = $props();

  let result = $state<AnalysisResult | null>(null);
  let sourceList: Source[] = $state([]);
  let ruleList: LogRule[] = $state([]);
  let patternList: Pattern[] = $state([]);
  let rulesetList: Ruleset[] = $state([]);
  let running = $state(false);
  let error: string | null = $state(null);
  let selectedSourceId: number | null = $state(null);
  let viewMode: 'table' | 'timeline' | 'state' | 'clusters' = $state('table');
  let linesProcessed: number = $state(0);
  let autoTriggered = $state(false);
  let currentHandle: { close: () => void } | null = $state(null);
  let lastRunStamp = $state(0);

  let timeStart: string = $state('');
  let timeEnd: string = $state('');

  let navigateTarget: string | null = $state(null);

  let filterRuleId: number | null = $state(null);
  let filterSourceId: number | null = $state(null);

  let showExportOptions = $state(false);
  let exportRuleMatches = $state(true);
  let exportPatternMatches = $state(true);
  let exportStateChanges = $state(true);

  function doExport(format: 'json' | 'csv') {
    const timeRange: TimeRange = {
      start: timeStart || null,
      end: timeEnd || null,
    };
    if (format === 'json') {
      const include: string[] = [];
      if (exportRuleMatches) include.push('rule_matches');
      if (exportPatternMatches) include.push('pattern_matches');
      if (exportStateChanges) include.push('state_changes');
      analysisApi.exportJson(projectId, timeRange, include.length < 3 ? include : undefined);
    } else {
      // CSV: one file per section (each has different column schema)
      const sections: string[] = [];
      if (exportRuleMatches) sections.push('rule_matches');
      if (exportPatternMatches) sections.push('pattern_matches');
      if (exportStateChanges) sections.push('state_changes');
      for (const section of sections) {
        analysisApi.exportCsv(projectId, section, timeRange);
      }
    }
  }

  let selectedSource = $derived(sourceList.find((s) => s.id === selectedSourceId) ?? null);

  const emptyResult: AnalysisResult = { rule_matches: [], pattern_matches: [], state_changes: [] };

  let filteredResult: AnalysisResult = $derived.by(() => {
    if (!result) return emptyResult;
    let rm = result.rule_matches;
    let sc = result.state_changes;
    if (filterRuleId !== null) {
      rm = rm.filter((m) => m.rule_id === filterRuleId);
      sc = sc.filter((c) => c.rule_id === filterRuleId);
    }
    if (filterSourceId !== null) {
      rm = rm.filter((m) => m.source_id === filterSourceId);
      sc = sc.filter((c) => c.source_id === filterSourceId);
    }
    return { rule_matches: rm, pattern_matches: result.pattern_matches, state_changes: sc };
  });

  let ruleBreakdown = $derived.by(() => {
    if (!result) return [];
    const counts = new Map<number, number>();
    for (const rm of result.rule_matches) counts.set(rm.rule_id, (counts.get(rm.rule_id) ?? 0) + 1);
    return Array.from(counts.entries())
      .map(([id, count]) => ({ id, name: getRuleName(id), count }))
      .sort((a, b) => b.count - a.count);
  });

  let sourceBreakdown = $derived.by(() => {
    if (!result) return [];
    const counts = new Map<number, number>();
    for (const rm of result.rule_matches)
      counts.set(rm.source_id, (counts.get(rm.source_id) ?? 0) + 1);
    return Array.from(counts.entries())
      .map(([id, count]) => ({ id, name: getSourceName(id), count }))
      .sort((a, b) => b.count - a.count);
  });

  let sourceRuleMatches = $derived(
    filteredResult?.rule_matches.filter((m) => m.source_id === selectedSourceId) ?? [],
  );

  function getRuleName(id: number): string {
    return ruleList.find((r) => r.id === id)?.name ?? `Rule #${id}`;
  }

  function getPatternName(id: number): string {
    return patternList.find((p) => p.id === id)?.name ?? `Pattern #${id}`;
  }

  function getSourceName(id: number): string {
    return sourceList.find((s) => s.id === id)?.name ?? `Source #${id}`;
  }

  function handleNavigate(sourceId: number, rawLine: string) {
    viewMode = 'table';
    selectedSourceId = sourceId;
    navigateTarget = rawLine;
  }

  function formatStateValue(sv: StateValue): string {
    if ('String' in sv) return sv.String;
    if ('Integer' in sv) return String(sv.Integer);
    if ('Float' in sv) return String(sv.Float);
    if ('Bool' in sv) return String(sv.Bool);
    return '?';
  }

  // Virtual scroll for pattern matches
  const PM_PADDING = 32; // .card padding: 16px * 2
  const PM_BORDER = 2; // .card border: 1px * 2
  const PM_HEADER = 29; // .pm-header height (~21px text + 8px margin-bottom)
  const PM_SOURCE_HEADER = 22; // .pm-source-name line height
  const PM_ENTRY = 22; // .pm-entry height (12px font + 2px padding * 2 + flex spacing)
  const PM_SOURCE_MARGIN = 8; // .pm-source margin-bottom
  const PM_CARD_MARGIN = 8; // .pattern-match margin-bottom

  function estimatePmHeight(pm: PatternMatch): number {
    let h = PM_PADDING + PM_BORDER + PM_HEADER;
    for (const [, stateMap] of Object.entries(pm.state_snapshot)) {
      h += PM_SOURCE_HEADER + Object.keys(stateMap).length * PM_ENTRY + PM_SOURCE_MARGIN;
    }
    return h + PM_CARD_MARGIN;
  }

  let pmScrollTop = $state(0);
  let pmContainerHeight = $state(400);
  let pmContainer: HTMLDivElement | undefined = $state();

  const PM_OVERSCAN = 5;

  let pmOffsets = $derived.by(() => {
    const pms = filteredResult.pattern_matches;
    const offsets = new Array<number>(pms.length + 1);
    offsets[0] = 0;
    for (let i = 0; i < pms.length; i++) {
      offsets[i + 1] = offsets[i] + estimatePmHeight(pms[i]);
    }
    return offsets;
  });

  let pmTotalHeight = $derived(pmOffsets[pmOffsets.length - 1] ?? 0);

  let pmStartIdx = $derived.by(() => {
    const offsets = pmOffsets;
    let lo = 0,
      hi = offsets.length - 2;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (offsets[mid + 1] <= pmScrollTop) lo = mid + 1;
      else hi = mid;
    }
    return Math.max(0, lo - PM_OVERSCAN);
  });

  let pmEndIdx = $derived.by(() => {
    const offsets = pmOffsets;
    const bottom = pmScrollTop + pmContainerHeight;
    let lo = pmStartIdx,
      hi = offsets.length - 2;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (offsets[mid] < bottom) lo = mid + 1;
      else hi = mid;
    }
    return Math.min(offsets.length - 1, lo + PM_OVERSCAN);
  });

  let pmVisibleMatches = $derived(filteredResult.pattern_matches.slice(pmStartIdx, pmEndIdx));

  let pmOffsetY = $derived(pmOffsets[pmStartIdx] ?? 0);

  function onPmScroll(e: Event) {
    pmScrollTop = (e.target as HTMLDivElement).scrollTop;
  }

  $effect(() => {
    if (!pmContainer) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        pmContainerHeight = entry.contentRect.height;
      }
    });
    ro.observe(pmContainer);
    return () => ro.disconnect();
  });

  async function load() {
    try {
      [sourceList, ruleList, patternList, rulesetList] = await Promise.all([
        sourcesApi.list(projectId),
        rulesApi.list(projectId),
        patternsApi.list(projectId),
        rulesetsApi.list(projectId),
      ]);
    } catch (e: any) {
      error = e.message;
    }
  }

  function runAnalysis(auto = false) {
    // Cancel any in-flight analysis
    if (currentHandle) {
      currentHandle.close();
      currentHandle = null;
    }

    autoTriggered = auto;
    running = true;
    error = null;
    linesProcessed = 0;
    lastRunStamp = getInvalidationStamp();
    filterRuleId = null;
    filterSourceId = null;
    result = { rule_matches: [], pattern_matches: [], state_changes: [] };

    // Re-fetch rules/patterns/sources for auto-reruns
    load();

    // Buffers for batched UI updates
    let ruleMatchBuffer: RuleMatch[] = [];
    let patternMatchBuffer: PatternMatch[] = [];
    let stateChangeBuffer: StateChange[] = [];

    const flushInterval = setInterval(() => {
      if (
        ruleMatchBuffer.length > 0 ||
        patternMatchBuffer.length > 0 ||
        stateChangeBuffer.length > 0
      ) {
        result!.rule_matches.push(...ruleMatchBuffer);
        result!.pattern_matches.push(...patternMatchBuffer);
        result!.state_changes.push(...stateChangeBuffer);
        ruleMatchBuffer = [];
        patternMatchBuffer = [];
        stateChangeBuffer = [];
      }
    }, 100);

    const timeRange: TimeRange = {
      start: timeStart || null,
      end: timeEnd || null,
    };

    const handle = analysisApi.runStreaming(
      projectId,
      {
        onRuleMatch: (rm) => {
          ruleMatchBuffer.push(rm);
        },
        onPatternMatch: (pm) => {
          patternMatchBuffer.push(pm);
        },
        onStateChange: (sc) => {
          stateChangeBuffer.push(sc);
        },
        onProgress: (lines) => {
          linesProcessed = lines;
        },
        onComplete: () => {
          clearInterval(flushInterval);
          // Final flush
          if (
            ruleMatchBuffer.length > 0 ||
            patternMatchBuffer.length > 0 ||
            stateChangeBuffer.length > 0
          ) {
            result!.rule_matches.push(...ruleMatchBuffer);
            result!.pattern_matches.push(...patternMatchBuffer);
            result!.state_changes.push(...stateChangeBuffer);
            ruleMatchBuffer = [];
            patternMatchBuffer = [];
            stateChangeBuffer = [];
          }
          setCachedAnalysis(projectId, result!);
          running = false;
          currentHandle = null;
        },
        onError: (message) => {
          clearInterval(flushInterval);
          // Final flush
          if (
            ruleMatchBuffer.length > 0 ||
            patternMatchBuffer.length > 0 ||
            stateChangeBuffer.length > 0
          ) {
            result!.rule_matches.push(...ruleMatchBuffer);
            result!.pattern_matches.push(...patternMatchBuffer);
            result!.state_changes.push(...stateChangeBuffer);
          }
          error = message;
          running = false;
          currentHandle = null;
        },
      },
      timeRange,
    );

    currentHandle = handle;
    return handle;
  }

  $effect(() => {
    projectId;
    result = getCachedAnalysis(projectId);
    load();
  });

  // Auto-rerun analysis when rules/patterns/rulesets change
  $effect(() => {
    const stamp = getInvalidationStamp();
    if (stamp > 0 && stamp !== lastRunStamp) {
      const timer = setTimeout(() => {
        runAnalysis(true);
      }, 500);
      return () => clearTimeout(timer);
    }
  });
</script>

<div class="header-row">
  <h2>Analysis</h2>
  <div class="header-actions">
    <button class="primary" onclick={() => runAnalysis(false)} disabled={running}>
      {running
        ? linesProcessed > 0
          ? `Processing... ${linesProcessed} lines`
          : autoTriggered
            ? 'Re-analyzing...'
            : 'Running...'
        : 'Run Analysis'}
    </button>
    {#if result}
      <button onclick={() => (showExportOptions = !showExportOptions)} disabled={running}>
        Export
      </button>
    {/if}
  </div>
</div>

{#if showExportOptions}
  <div class="export-options">
    <label><input type="checkbox" bind:checked={exportRuleMatches} /> Rule matches</label>
    <label><input type="checkbox" bind:checked={exportPatternMatches} /> Pattern matches</label>
    <label><input type="checkbox" bind:checked={exportStateChanges} /> State changes</label>
    <button onclick={() => doExport('json')}>Download JSON</button>
    <button onclick={() => doExport('csv')}>Download CSV</button>
  </div>
{/if}

<div class="time-range-row">
  <label>From <input type="datetime-local" bind:value={timeStart} step="1" /></label>
  <label>To <input type="datetime-local" bind:value={timeEnd} step="1" /></label>
  {#if timeStart || timeEnd}
    <button
      onclick={() => {
        timeStart = '';
        timeEnd = '';
      }}>Clear</button
    >
  {/if}
</div>

{#if error}
  <div class="error-banner">{error}</div>
{/if}

{#if result}
  <div class="results-summary card">
    <h3>Results</h3>
    <div class="summary-stats">
      <div class="stat">
        <span class="stat-value">{result.rule_matches.length}</span>
        <span class="stat-label">Rule Matches</span>
      </div>
      <div class="stat">
        <span class="stat-value">{result.pattern_matches.length}</span>
        <span class="stat-label">Pattern Matches</span>
      </div>
      <div class="stat">
        <span class="stat-value">{result.state_changes.length}</span>
        <span class="stat-label">State Changes</span>
      </div>
    </div>
    {#if result.rule_matches.length > 0}
      <div class="filter-facets">
        <div class="facet-group">
          <span class="facet-label">Rules</span>
          <div class="facet-chips">
            {#each ruleBreakdown as rb}
              <button
                class="facet-chip"
                class:active={filterRuleId === rb.id}
                onclick={() => {
                  filterRuleId = filterRuleId === rb.id ? null : rb.id;
                }}
              >
                {rb.name} <span class="chip-count">{rb.count}</span>
              </button>
            {/each}
          </div>
        </div>
        <div class="facet-group">
          <span class="facet-label">Sources</span>
          <div class="facet-chips">
            {#each sourceBreakdown as sb}
              <button
                class="facet-chip"
                class:active={filterSourceId === sb.id}
                onclick={() => {
                  filterSourceId = filterSourceId === sb.id ? null : sb.id;
                }}
              >
                {sb.name} <span class="chip-count">{sb.count}</span>
              </button>
            {/each}
          </div>
        </div>
        {#if filterRuleId !== null || filterSourceId !== null}
          <div class="filter-status">
            <span
              >Showing {filteredResult.rule_matches.length} of {result.rule_matches.length} matches</span
            >
            <button
              onclick={() => {
                filterRuleId = null;
                filterSourceId = null;
              }}>Clear filters</button
            >
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="view-tabs">
    <button
      class:active={viewMode === 'table'}
      onclick={() => {
        viewMode = 'table';
        navigateTarget = null;
      }}>Table</button
    >
    <button
      class:active={viewMode === 'timeline'}
      onclick={() => {
        viewMode = 'timeline';
        navigateTarget = null;
      }}>Timeline</button
    >
    <button
      class:active={viewMode === 'state'}
      onclick={() => {
        viewMode = 'state';
        navigateTarget = null;
      }}>State Evolution</button
    >
    <button
      class:active={viewMode === 'clusters'}
      onclick={() => {
        viewMode = 'clusters';
        navigateTarget = null;
      }}>Clusters</button
    >
  </div>

  {#if viewMode === 'table'}
    {#if sourceList.length > 0}
      <div class="source-selector">
        <label>View source</label>
        <div class="source-buttons">
          {#each sourceList as src}
            <button
              class:active={selectedSourceId === src.id}
              onclick={() => {
                selectedSourceId = src.id;
                navigateTarget = null;
              }}
            >
              {src.name}
              {#if filteredResult.rule_matches.filter((m) => m.source_id === src.id).length > 0}
                <span class="match-count"
                  >{filteredResult.rule_matches.filter((m) => m.source_id === src.id).length}</span
                >
              {/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}

    {#if selectedSource}
      <div class="viewer-section">
        <LogViewer
          source={selectedSource}
          {projectId}
          ruleMatches={sourceRuleMatches}
          patternMatches={filteredResult.pattern_matches}
          {navigateTarget}
        />
      </div>
    {/if}

    {#if filteredResult.pattern_matches.length > 0}
      <div class="pattern-matches-section">
        <h3>Pattern Matches ({filteredResult.pattern_matches.length})</h3>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="pm-scroll-container" bind:this={pmContainer} onscroll={onPmScroll}>
          <div class="pm-scroll-spacer" style="height: {pmTotalHeight}px;">
            <div class="pm-visible-cards" style="transform: translateY({pmOffsetY}px);">
              {#each pmVisibleMatches as pm}
                <div class="pattern-match card">
                  <div class="pm-header">
                    <span class="pm-name">{getPatternName(pm.pattern_id)}</span>
                    <span class="pm-time">{pm.timestamp}</span>
                  </div>
                  <div class="pm-state">
                    {#each Object.entries(pm.state_snapshot) as [sourceName, stateMap]}
                      <div class="pm-source">
                        <span class="pm-source-name">{sourceName}</span>
                        {#each Object.entries(stateMap) as [key, val]}
                          <div class="pm-entry">
                            <span class="state-key">{key}</span>
                            <span class="state-value">{formatStateValue(val.value)}</span>
                            <span class="state-set-at">{val.set_at}</span>
                          </div>
                        {/each}
                      </div>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          </div>
        </div>
      </div>
    {/if}

    {#if filteredResult.rule_matches.length > 0 && !selectedSource}
      <div class="rule-matches-section">
        <h3>Rule Matches</h3>
        <div class="match-table">
          {#each filteredResult.rule_matches.slice(0, 100) as rm}
            <div class="match-row">
              <span class="badge">{getRuleName(rm.rule_id)}</span>
              <span class="badge">{getSourceName(rm.source_id)}</span>
              <code class="match-line">{rm.log_line.content || rm.log_line.raw}</code>
            </div>
          {/each}
          {#if filteredResult.rule_matches.length > 100}
            <div class="text-muted">
              ...and {filteredResult.rule_matches.length - 100} more matches
            </div>
          {/if}
        </div>
      </div>
    {/if}
  {:else if viewMode === 'timeline'}
    <TimelineView
      result={filteredResult}
      {sourceList}
      {ruleList}
      {patternList}
      onNavigate={handleNavigate}
    />
  {:else if viewMode === 'state'}
    <StateEvolutionView stateChanges={filteredResult.state_changes} {sourceList} {ruleList} />
  {:else if viewMode === 'clusters'}
    <ErrorClusteringView {projectId} {sourceList} />
  {/if}
{:else if !running}
  <div class="guidance">
    <strong>Setup checklist</strong>
    <ul class="checklist">
      <li class:done={sourceList.length > 0}>
        {sourceList.length > 0
          ? `${sourceList.length} source${sourceList.length > 1 ? 's' : ''} configured`
          : 'Add at least one source (Sources tab)'}
      </li>
      <li class:done={ruleList.length > 0}>
        {ruleList.length > 0
          ? `${ruleList.length} rule${ruleList.length > 1 ? 's' : ''} defined`
          : 'Create rules to detect events (Rules tab)'}
      </li>
      <li class:done={rulesetList.length > 0}>
        {rulesetList.length > 0
          ? `${rulesetList.length} ruleset${rulesetList.length > 1 ? 's' : ''} linking rules to templates`
          : 'Create a ruleset to bind rules to a template (Rulesets tab)'}
      </li>
    </ul>
    {#if sourceList.length > 0 && ruleList.length > 0 && rulesetList.length > 0}
      <p>Ready — click <strong>Run Analysis</strong> above.</p>
    {:else}
      <p>Complete the steps above, then click <strong>Run Analysis</strong>.</p>
    {/if}
  </div>
{/if}

<style>
  .header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .header-row h2 {
    margin: 0;
  }

  .header-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .export-options {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 8px 0;
  }

  .time-range-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
    font-size: 13px;
  }

  .time-range-row input {
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 4px 8px;
    margin-left: 4px;
  }

  .error-banner {
    background: rgba(247, 118, 142, 0.1);
    border: 1px solid var(--red);
    color: var(--red);
    padding: 12px;
    border-radius: var(--radius);
    margin-bottom: 16px;
  }

  .results-summary {
    margin-bottom: 16px;
  }

  .summary-stats {
    display: flex;
    gap: 24px;
    margin-top: 8px;
  }

  .stat {
    display: flex;
    flex-direction: column;
  }

  .stat-value {
    font-size: 24px;
    font-weight: 700;
    color: var(--accent);
  }

  .stat-label {
    font-size: 12px;
    color: var(--text-dim);
  }

  .view-tabs {
    display: flex;
    gap: 4px;
    margin-bottom: 16px;
  }

  .view-tabs button {
    padding: 6px 16px;
    font-size: 13px;
    border-radius: var(--radius) var(--radius) 0 0;
    border-bottom: 2px solid transparent;
  }

  .view-tabs button.active {
    background: var(--bg-secondary);
    border-bottom-color: var(--accent);
    color: var(--accent);
  }

  .source-selector {
    margin-bottom: 16px;
  }

  .source-buttons {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }

  .source-buttons button.active {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
  }

  .match-count {
    display: inline-block;
    background: var(--accent);
    color: var(--bg);
    font-size: 11px;
    padding: 0 6px;
    border-radius: 8px;
    margin-left: 4px;
    font-weight: 600;
  }

  .source-buttons button.active .match-count {
    background: var(--bg);
    color: var(--accent);
  }

  .viewer-section {
    margin-bottom: 24px;
  }

  .pattern-matches-section,
  .rule-matches-section {
    margin-top: 24px;
  }

  .pattern-match {
    margin-bottom: 8px;
  }

  .pm-header {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .pm-name {
    font-weight: 600;
    color: var(--purple);
  }

  .pm-time {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-dim);
  }

  .pm-source {
    margin-bottom: 8px;
  }

  .pm-source-name {
    font-weight: 600;
    font-size: 12px;
    color: var(--cyan);
    display: block;
    margin-bottom: 4px;
  }

  .pm-entry {
    display: flex;
    justify-content: space-between;
    padding: 2px 0;
    font-size: 12px;
  }

  .state-key {
    font-family: var(--font-mono);
    color: var(--cyan);
  }

  .state-value {
    font-family: var(--font-mono);
  }

  .state-set-at {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }

  .match-table {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .match-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px;
    background: var(--bg);
    border-radius: var(--radius);
  }

  .match-line {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .text-muted {
    color: var(--text-muted);
    font-size: 12px;
    padding: 8px;
  }

  .filter-facets {
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .facet-group {
    margin-bottom: 8px;
  }

  .facet-label {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
    display: block;
  }

  .facet-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .facet-chip {
    padding: 3px 10px;
    font-size: 12px;
    border-radius: 12px;
  }

  .facet-chip.active {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
  }

  .chip-count {
    font-size: 10px;
    padding: 0 5px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.15);
    margin-left: 4px;
  }

  .facet-chip.active .chip-count {
    background: rgba(0, 0, 0, 0.2);
  }

  .filter-status {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .pm-scroll-container {
    max-height: 600px;
    overflow-y: auto;
    position: relative;
  }

  .pm-scroll-spacer {
    position: relative;
  }

  .pm-visible-cards {
    position: absolute;
    left: 0;
    right: 0;
  }

  .checklist {
    list-style: none;
    padding: 8px 0;
  }

  .checklist li {
    padding: 4px 0;
  }

  .checklist li::before {
    content: '\2717  ';
    color: var(--text-muted);
  }

  .checklist li.done::before {
    content: '\2713  ';
    color: var(--green);
  }
</style>
