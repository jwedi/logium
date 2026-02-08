<script lang="ts">
  import { projects as projectsApi, type Project } from './lib/api';
  import ProjectManager from './lib/ProjectManager.svelte';
  import SourceManager from './lib/SourceManager.svelte';
  import TemplateManager from './lib/TemplateManager.svelte';
  import RuleList from './lib/RuleList.svelte';
  import RulesetManager from './lib/RulesetManager.svelte';
  import PatternEditor from './lib/PatternEditor.svelte';
  import AnalysisView from './lib/AnalysisView.svelte';

  type View = 'projects' | 'sources' | 'templates' | 'rules' | 'rulesets' | 'patterns' | 'analysis';

  let allProjects: Project[] = $state([]);
  let currentProjectId: number | null = $state(null);
  let currentView: View = $state('projects');
  let loading = $state(false);
  let error: string | null = $state(null);

  let currentProject = $derived(allProjects.find(p => p.id === currentProjectId) ?? null);

  const navItems: { view: View; label: string; requiresProject: boolean }[] = [
    { view: 'projects', label: 'Projects', requiresProject: false },
    { view: 'sources', label: 'Sources', requiresProject: true },
    { view: 'templates', label: 'Templates', requiresProject: true },
    { view: 'rules', label: 'Rules', requiresProject: true },
    { view: 'rulesets', label: 'Rulesets', requiresProject: true },
    { view: 'patterns', label: 'Patterns', requiresProject: true },
    { view: 'analysis', label: 'Analysis', requiresProject: true },
  ];

  async function loadProjects() {
    loading = true;
    error = null;
    try {
      allProjects = await projectsApi.list();
    } catch (e: any) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  function selectProject(id: number) {
    currentProjectId = id;
    currentView = 'sources';
  }

  function onProjectCreated(project: Project) {
    allProjects = [...allProjects, project];
    selectProject(project.id);
  }

  function onProjectDeleted(id: number) {
    allProjects = allProjects.filter(p => p.id !== id);
    if (currentProjectId === id) {
      currentProjectId = null;
      currentView = 'projects';
    }
  }

  $effect(() => {
    loadProjects();
  });
</script>

<div class="app-layout">
  <aside class="sidebar">
    <div class="sidebar-header">
      <h1>Logium</h1>
    </div>

    {#if allProjects.length > 0}
      <div class="project-selector">
        <label>Project</label>
        <select
          value={currentProjectId ?? ''}
          onchange={(e) => {
            const val = (e.target as HTMLSelectElement).value;
            if (val) selectProject(Number(val));
          }}
        >
          <option value="">Select project...</option>
          {#each allProjects as project}
            <option value={project.id}>{project.name}</option>
          {/each}
        </select>
      </div>
    {/if}

    <nav class="sidebar-nav">
      {#each navItems as item}
        {#if !item.requiresProject || currentProjectId}
          <button
            class="nav-item"
            class:active={currentView === item.view}
            onclick={() => currentView = item.view}
          >
            {item.label}
          </button>
        {/if}
      {/each}
    </nav>

    {#if error}
      <div class="sidebar-error">{error}</div>
    {/if}
  </aside>

  <main class="main-content">
    {#if loading && allProjects.length === 0}
      <div class="empty">Loading...</div>
    {:else if currentView === 'projects'}
      <ProjectManager
        projects={allProjects}
        {onProjectCreated}
        {onProjectDeleted}
        onSelect={selectProject}
      />
    {:else if currentProjectId}
      {#if currentView === 'sources'}
        <SourceManager projectId={currentProjectId} />
      {:else if currentView === 'templates'}
        <TemplateManager projectId={currentProjectId} />
      {:else if currentView === 'rules'}
        <RuleList projectId={currentProjectId} />
      {:else if currentView === 'rulesets'}
        <RulesetManager projectId={currentProjectId} />
      {:else if currentView === 'patterns'}
        <PatternEditor projectId={currentProjectId} />
      {:else if currentView === 'analysis'}
        <AnalysisView projectId={currentProjectId} />
      {/if}
    {:else}
      <div class="empty">Select or create a project to get started.</div>
    {/if}
  </main>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100%;
    width: 100%;
  }

  .sidebar {
    width: 220px;
    min-width: 220px;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  .sidebar-header {
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .sidebar-header h1 {
    font-size: 20px;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: -0.02em;
  }

  .project-selector {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .project-selector select {
    width: 100%;
  }

  .sidebar-nav {
    display: flex;
    flex-direction: column;
    padding: 8px;
    gap: 2px;
    flex: 1;
  }

  .nav-item {
    text-align: left;
    border: none;
    background: transparent;
    padding: 8px 12px;
    border-radius: var(--radius);
    color: var(--text-dim);
    font-size: 13px;
    font-weight: 500;
  }

  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .nav-item.active {
    background: var(--bg-tertiary);
    color: var(--accent);
  }

  .sidebar-error {
    padding: 12px 16px;
    color: var(--red);
    font-size: 12px;
  }

  .main-content {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }
</style>
