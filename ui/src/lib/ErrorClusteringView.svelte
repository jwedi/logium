<script lang="ts">
  import { clustering as clusteringApi, type ClusterResult, type Source } from './api';
  import RuleCreator from './RuleCreator.svelte';

  let {
    projectId,
    sourceList,
  }: {
    projectId: number;
    sourceList: Source[];
  } = $props();

  let result: ClusterResult | null = $state(null);
  let loading = $state(false);
  let error: string | null = $state(null);
  let expandedIdx: number | null = $state(null);
  let showRuleCreator = $state(false);
  let ruleCreatorText = $state('');
  let ruleCreatorTemplateId: number = $state(0);
  let filterSourceId: number | null = $state(null);

  function getSourceName(id: number): string {
    return sourceList.find((s) => s.id === id)?.name ?? `Source #${id}`;
  }

  let displayedClusters = $derived.by(() => {
    if (!result) return [];
    if (filterSourceId === null) return result.clusters;
    return result.clusters.filter((c) => c.source_ids.includes(filterSourceId!));
  });

  async function runClustering() {
    loading = true;
    error = null;
    result = null;
    expandedIdx = null;
    try {
      result = await clusteringApi.run(projectId);
    } catch (e: any) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  function handleCreateRule(cluster: (typeof displayedClusters)[number]) {
    ruleCreatorText = cluster.sample_lines[0] ?? cluster.template;
    // Determine source template from the first source in the cluster
    const sourceId = cluster.source_ids[0];
    const source = sourceList.find((s) => s.id === sourceId);
    ruleCreatorTemplateId = source?.template_id ?? 0;
    showRuleCreator = true;
  }

  $effect(() => {
    projectId;
    runClustering();
  });
</script>

<div class="cluster-view">
  <div class="cluster-header">
    {#if loading}
      <span class="loading-text">Clustering log lines...</span>
    {:else if error}
      <span class="error-text">{error}</span>
    {:else if result}
      <span>{result.clusters.length} clusters from {result.total_lines} lines</span>
      <button class="refresh-btn" onclick={runClustering}>Refresh</button>
    {/if}
  </div>

  {#if result && result.clusters.length > 0}
    <div class="cluster-filters">
      <span class="facet-label">Sources</span>
      <div class="facet-chips">
        {#each sourceList as src}
          <button
            class="facet-chip"
            class:active={filterSourceId === src.id}
            onclick={() => {
              filterSourceId = filterSourceId === src.id ? null : src.id;
            }}
          >
            {src.name}
          </button>
        {/each}
      </div>
    </div>

    <div class="cluster-list">
      {#each displayedClusters as cluster, i}
        <div class="cluster-card card">
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="cluster-summary"
            onclick={() => {
              expandedIdx = expandedIdx === i ? null : i;
            }}
          >
            <code class="cluster-template">{cluster.template}</code>
            <div class="cluster-meta">
              <span class="badge count-badge">{cluster.count}</span>
              {#each cluster.source_ids as sid}
                <span class="badge source-badge">{getSourceName(sid)}</span>
              {/each}
              <button
                class="create-rule-btn"
                onclick={(e) => {
                  e.stopPropagation();
                  handleCreateRule(cluster);
                }}
              >
                Create Rule
              </button>
            </div>
          </div>
          {#if expandedIdx === i}
            <div class="cluster-samples">
              {#each cluster.sample_lines as line}
                <code class="sample-line">{line}</code>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {:else if result && result.clusters.length === 0}
    <div class="empty">No clusters found.</div>
  {/if}
</div>

{#if showRuleCreator}
  <RuleCreator
    {projectId}
    selectedText={ruleCreatorText}
    sourceTemplateId={ruleCreatorTemplateId}
    onClose={() => {
      showRuleCreator = false;
    }}
    onCreated={() => {
      showRuleCreator = false;
    }}
  />
{/if}

<style>
  .cluster-view {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .cluster-header {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 14px;
  }

  .loading-text {
    color: var(--text-muted);
    font-style: italic;
  }

  .error-text {
    color: var(--red);
  }

  .refresh-btn {
    padding: 4px 12px;
    font-size: 12px;
  }

  .cluster-filters {
    margin-bottom: 4px;
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

  .cluster-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .cluster-card {
    padding: 10px 14px;
  }

  .cluster-summary {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    cursor: pointer;
  }

  .cluster-template {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cluster-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .count-badge {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 8px;
  }

  .source-badge {
    background: var(--bg);
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 6px;
    color: var(--text-dim);
  }

  .create-rule-btn {
    padding: 3px 10px;
    font-size: 11px;
    white-space: nowrap;
  }

  .cluster-samples {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .sample-line {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-dim);
    padding: 4px 8px;
    background: var(--bg);
    border-radius: var(--radius);
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>
