<script lang="ts">
  import { projects as projectsApi, type Project } from './api';

  let {
    projects,
    onProjectCreated,
    onProjectUpdated,
    onProjectDeleted,
    onSelect,
  }: {
    projects: Project[];
    onProjectCreated: (p: Project) => void;
    onProjectUpdated: (p: Project) => void;
    onProjectDeleted: (id: number) => void;
    onSelect: (id: number) => void;
  } = $props();

  let newName = $state('');
  let creating = $state(false);
  let fileInput: HTMLInputElement | null = $state(null);
  let importTargetId: number | null = $state(null);
  let editingId: number | null = $state(null);
  let editName = $state('');
  let starterMenuId: number | null = $state(null);

  const STARTERS = [
    {
      key: 'nginx',
      label: 'Nginx Access Log',
      description: 'HTTP status extraction, 404 detection',
    },
    {
      key: 'syslog',
      label: 'Syslog Auth Log',
      description: 'Auth failures, remote host extraction',
    },
    {
      key: 'zookeeper',
      label: 'Zookeeper',
      description: 'WARN/ERROR levels, connection events',
    },
  ] as const;

  async function exportProject(id: number, name: string) {
    try {
      const blob = await projectsApi.exportConfig(id);
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${name}.logium.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e: any) {
      alert(e.message);
    }
  }

  function startImport(id: number) {
    importTargetId = id;
    fileInput?.click();
  }

  async function handleImportFile(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file || importTargetId == null) return;
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      const result = await projectsApi.importConfig(importTargetId, data);
      const total =
        result.timestamp_templates +
        result.source_templates +
        result.rules +
        result.rulesets +
        result.patterns;
      alert(
        `Imported ${total} items: ${result.timestamp_templates} timestamp templates, ${result.source_templates} source templates, ${result.rules} rules, ${result.rulesets} rulesets, ${result.patterns} patterns`,
      );
    } catch (e: any) {
      alert(`Import failed: ${e.message}`);
    } finally {
      input.value = '';
      importTargetId = null;
    }
  }

  async function createProject() {
    if (!newName.trim()) return;
    creating = true;
    try {
      const project = await projectsApi.create({ name: newName.trim() });
      newName = '';
      onProjectCreated(project);
    } catch (e: any) {
      alert(e.message);
    } finally {
      creating = false;
    }
  }

  async function deleteProject(id: number) {
    if (!confirm('Delete this project?')) return;
    try {
      await projectsApi.delete(id);
      onProjectDeleted(id);
    } catch (e: any) {
      alert(e.message);
    }
  }

  function startRename(project: Project) {
    editingId = project.id;
    editName = project.name;
  }

  function cancelRename() {
    editingId = null;
    editName = '';
  }

  async function loadStarter(projectId: number, key: string) {
    starterMenuId = null;
    try {
      const res = await fetch(`/starters/${key}.logium.json`);
      if (!res.ok) throw new Error(`Failed to fetch starter config`);
      const data = await res.json();
      const result = await projectsApi.importConfig(projectId, data);
      const total =
        result.timestamp_templates +
        result.source_templates +
        result.rules +
        result.rulesets +
        result.patterns;
      alert(
        `Loaded ${total} items (${result.timestamp_templates} timestamp templates, ${result.source_templates} source templates, ${result.rules} rules, ${result.rulesets} rulesets, ${result.patterns} patterns).\n\nDownload a sample log file at /starters/samples/${key}_sample.log to try it out.`,
      );
    } catch (e: any) {
      alert(`Failed to load starter: ${e.message}`);
    }
  }

  async function saveRename(id: number) {
    const trimmed = editName.trim();
    if (!trimmed) return;
    try {
      const updated = await projectsApi.update(id, { name: trimmed });
      onProjectUpdated(updated);
    } catch (e: any) {
      alert(e.message);
    } finally {
      editingId = null;
      editName = '';
    }
  }
</script>

<h2>Projects</h2>

<div class="create-form">
  <input
    type="text"
    bind:value={newName}
    placeholder="New project name..."
    onkeydown={(e) => e.key === 'Enter' && createProject()}
  />
  <button class="primary" onclick={createProject} disabled={creating || !newName.trim()}>
    Create
  </button>
</div>

{#if projects.length === 0}
  <div class="empty">No projects yet. Create one to get started.</div>
{:else}
  <div class="project-list">
    {#each projects as project}
      <div class="project-card card">
        <div class="project-info">
          {#if editingId === project.id}
            <input
              type="text"
              class="rename-input"
              bind:value={editName}
              onkeydown={(e) => {
                if (e.key === 'Enter') saveRename(project.id);
                if (e.key === 'Escape') cancelRename();
              }}
            />
          {:else}
            <span class="project-name">{project.name}</span>
          {/if}
          <span class="project-date">{new Date(project.created_at).toLocaleDateString()}</span>
        </div>
        <div class="project-actions">
          {#if editingId === project.id}
            <button
              class="primary"
              onclick={() => saveRename(project.id)}
              disabled={!editName.trim()}
            >
              Save
            </button>
            <button onclick={cancelRename}>Cancel</button>
          {:else}
            <button onclick={() => onSelect(project.id)}>Open</button>
            <button onclick={() => startRename(project)}>Rename</button>
            <button onclick={() => exportProject(project.id, project.name)}>Export</button>
            <button onclick={() => startImport(project.id)}>Import</button>
            <div class="starter-dropdown">
              <button
                onclick={() => (starterMenuId = starterMenuId === project.id ? null : project.id)}
              >
                Load Starter
              </button>
              {#if starterMenuId === project.id}
                <div class="starter-menu">
                  {#each STARTERS as starter}
                    <button
                      class="starter-option"
                      onclick={() => loadStarter(project.id, starter.key)}
                    >
                      <strong>{starter.label}</strong>
                      <span class="hint">{starter.description}</span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
            <button class="danger" onclick={() => deleteProject(project.id)}>Delete</button>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{/if}

<input
  type="file"
  accept=".json"
  style="display: none"
  bind:this={fileInput}
  onchange={handleImportFile}
/>

<style>
  .create-form {
    display: flex;
    gap: 8px;
    margin-bottom: 20px;
  }

  .create-form input {
    flex: 1;
    max-width: 400px;
  }

  .project-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .project-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .project-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .project-name {
    font-weight: 600;
    font-size: 15px;
  }

  .project-date {
    font-size: 12px;
    color: var(--text-muted);
  }

  .project-actions {
    display: flex;
    gap: 8px;
  }

  .rename-input {
    font-size: 15px;
    font-weight: 600;
    padding: 2px 6px;
    width: 200px;
  }

  .starter-dropdown {
    position: relative;
  }

  .starter-menu {
    position: absolute;
    top: 100%;
    right: 0;
    z-index: 10;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    min-width: 240px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .starter-option {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 12px;
    border: none;
    background: none;
    cursor: pointer;
    border-radius: 4px;
    text-align: left;
    width: 100%;
  }

  .starter-option:hover {
    background: var(--bg-hover);
  }

  .starter-option .hint {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
