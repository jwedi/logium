<script lang="ts">
  import { projects as projectsApi, type Project } from './api';

  let { projects, onProjectCreated, onProjectDeleted, onSelect }: {
    projects: Project[];
    onProjectCreated: (p: Project) => void;
    onProjectDeleted: (id: number) => void;
    onSelect: (id: number) => void;
  } = $props();

  let newName = $state('');
  let creating = $state(false);

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
          <span class="project-name">{project.name}</span>
          <span class="project-date">{new Date(project.created_at).toLocaleDateString()}</span>
        </div>
        <div class="project-actions">
          <button onclick={() => onSelect(project.id)}>Open</button>
          <button class="danger" onclick={() => deleteProject(project.id)}>Delete</button>
        </div>
      </div>
    {/each}
  </div>
{/if}

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
</style>
