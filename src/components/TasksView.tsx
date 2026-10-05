import { useCallback, useEffect, useMemo, useState, type FormEvent } from 'react';
import { api } from '../lib/api';
import { adjacentStatus, groupTasksByStatus, isConflict, messageFor, STATUS_LABELS, STATUS_ORDER } from '../lib/pure';
import type { PageSummary, Project, Task, TaskPatch, TaskPriority, TaskStatus } from '../lib/types';

interface Props {
  projects: Project[];
  initialProjectId: string | null;
  pages: PageSummary[];
  onOpenPage: (id: string) => void;
  onProjectsChanged: () => void;
  onError: (error: unknown) => void;
}

const PRIORITIES: TaskPriority[] = ['high', 'medium', 'low'];

/** Table and board views over the same tasks. Every edit uses the task's revision. */
export function TasksView({ projects, initialProjectId, pages, onOpenPage, onProjectsChanged, onError }: Props) {
  const [projectId, setProjectId] = useState<string | null>(initialProjectId);
  const [mode, setMode] = useState<'table' | 'board'>('table');
  const [tasks, setTasks] = useState<Task[] | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [newProjectName, setNewProjectName] = useState('');

  const load = useCallback(async () => {
    try {
      setTasks(await api.listTasks(projectId));
    } catch (err) {
      onError(err);
    }
  }, [projectId, onError]);

  useEffect(() => {
    void load();
  }, [load]);

  const projectName = (id: string | null) => projects.find((p) => p.id === id)?.name ?? '';
  const pageTitle = (id: string | null) => pages.find((p) => p.id === id)?.title ?? null;

  const applyPatch = async (task: Task, patch: TaskPatch) => {
    setNotice(null);
    try {
      const updated = await api.updateTask(task.id, patch, task.revision);
      setTasks((list) => list?.map((t) => (t.id === updated.id ? updated : t)) ?? null);
    } catch (err) {
      if (isConflict(err)) {
        setNotice(messageFor(err));
        await load();
      } else {
        onError(err);
      }
    }
  };

  const remove = async (task: Task) => {
    try {
      await api.deleteTask(task.id);
      setTasks((list) => list?.filter((t) => t.id !== task.id) ?? null);
    } catch (err) {
      onError(err);
    }
  };

  const createProject = async (event: FormEvent) => {
    event.preventDefault();
    try {
      const project = await api.createProject(newProjectName);
      setNewProjectName('');
      onProjectsChanged();
      setProjectId(project.id);
    } catch (err) {
      onError(err);
    }
  };

  const grouped = useMemo(() => (tasks ? groupTasksByStatus(tasks) : null), [tasks]);

  return (
    <section className="tasks-view" aria-labelledby="tasks-title">
      <header className="view-header">
        <h1 id="tasks-title">{projectId ? projectName(projectId) : 'All tasks'}</h1>
        <div className="view-controls">
          <label className="inline-label">
            <span>Project</span>
            <select value={projectId ?? ''} onChange={(e) => setProjectId(e.target.value || null)}>
              <option value="">All projects</option>
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
          <div role="group" aria-label="View" className="segmented small">
            <button type="button" aria-pressed={mode === 'table'} onClick={() => setMode('table')}>
              Table
            </button>
            <button type="button" aria-pressed={mode === 'board'} onClick={() => setMode('board')}>
              Board
            </button>
          </div>
        </div>
      </header>

      <details className="disclosure">
        <summary>Add a task</summary>
        <NewTaskForm projects={projects} defaultProjectId={projectId} onCreated={() => void load()} onError={onError} />
      </details>

      <form className="inline-form" onSubmit={createProject}>
        <label className="inline-label">
          <span>New project</span>
          <input value={newProjectName} maxLength={120} onChange={(e) => setNewProjectName(e.target.value)} />
        </label>
        <button type="submit" disabled={!newProjectName.trim()}>
          Create project
        </button>
      </form>

      {notice && (
        <p role="alert" className="inline-error">
          {notice}
        </p>
      )}

      {tasks === null && <p role="status">Loading tasks…</p>}
      {tasks !== null && tasks.length === 0 && (
        <p className="muted">No tasks here yet. Add one above, or turn a note into tasks.</p>
      )}

      {tasks !== null && tasks.length > 0 && mode === 'table' && (
        <div className="table-wrap">
          <table className="task-table">
            <caption className="visually-hidden">Tasks</caption>
            <thead>
              <tr>
                <th scope="col">Title</th>
                <th scope="col">Status</th>
                <th scope="col">Priority</th>
                <th scope="col">Due date</th>
                <th scope="col">Project</th>
                <th scope="col">Source</th>
                <th scope="col">
                  <span className="visually-hidden">Actions</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {tasks.map((task) => (
                <tr key={task.id}>
                  <th scope="row">
                    <input
                      aria-label={`Title for ${task.title}`}
                      defaultValue={task.title}
                      key={`${task.id}-${task.revision}`}
                      maxLength={200}
                      onBlur={(e) => {
                        const value = e.target.value.trim();
                        if (value && value !== task.title) void applyPatch(task, { title: value });
                      }}
                    />
                  </th>
                  <td>
                    <select
                      aria-label={`Status for ${task.title}`}
                      value={task.status}
                      onChange={(e) => void applyPatch(task, { status: e.target.value as TaskStatus })}
                    >
                      {STATUS_ORDER.map((s) => (
                        <option key={s} value={s}>
                          {STATUS_LABELS[s]}
                        </option>
                      ))}
                    </select>
                  </td>
                  <td>
                    <select
                      aria-label={`Priority for ${task.title}`}
                      value={task.priority}
                      onChange={(e) => void applyPatch(task, { priority: e.target.value as TaskPriority })}
                    >
                      {PRIORITIES.map((p) => (
                        <option key={p} value={p}>
                          {p}
                        </option>
                      ))}
                    </select>
                  </td>
                  <td>
                    <div className="row tight">
                      <input
                        type="date"
                        aria-label={`Due date for ${task.title}`}
                        value={task.dueDate ?? ''}
                        onChange={(e) => void applyPatch(task, { dueDate: e.target.value })}
                      />
                      {task.dueDate && (
                        <button
                          type="button"
                          className="icon-button"
                          aria-label={`Clear due date for ${task.title}`}
                          onClick={() => void applyPatch(task, { dueDate: '' })}
                        >
                          ×
                        </button>
                      )}
                    </div>
                  </td>
                  <td>
                    <select
                      aria-label={`Project for ${task.title}`}
                      value={task.projectId ?? ''}
                      onChange={(e) => void applyPatch(task, { projectId: e.target.value })}
                    >
                      <option value="">None</option>
                      {projects.map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.name}
                        </option>
                      ))}
                    </select>
                  </td>
                  <td>
                    {task.sourcePageId && pageTitle(task.sourcePageId) ? (
                      <button type="button" className="link-button" onClick={() => onOpenPage(task.sourcePageId!)}>
                        {pageTitle(task.sourcePageId)}
                      </button>
                    ) : (
                      <span className="muted">Manual</span>
                    )}
                  </td>
                  <td>
                    <button type="button" className="danger-quiet" onClick={() => void remove(task)}>
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {tasks !== null && tasks.length > 0 && mode === 'board' && grouped && (
        <div className="board" role="list" aria-label="Task board">
          {STATUS_ORDER.map((status) => (
            <section
              key={status}
              className="board-column"
              role="listitem"
              aria-label={STATUS_LABELS[status]}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                e.preventDefault();
                const dropped = tasks?.find((t) => t.id === e.dataTransfer.getData('text/plain'));
                if (dropped && dropped.status !== status) void applyPatch(dropped, { status });
              }}
            >
              <h2>
                {STATUS_LABELS[status]} <span className="muted">({grouped[status].length})</span>
              </h2>
              {grouped[status].map((task) => (
                <article
                  key={task.id}
                  className="card"
                  draggable
                  onDragStart={(e) => e.dataTransfer.setData('text/plain', task.id)}
                >
                  <h3>{task.title}</h3>
                  <p className="muted small">
                    {task.priority} priority
                    {task.dueDate ? ` · due ${task.dueDate}` : ''}
                    {task.projectId ? ` · ${projectName(task.projectId)}` : ''}
                  </p>
                  <div className="row tight">
                    <button
                      type="button"
                      disabled={status === 'todo'}
                      aria-label={`Move ${task.title} to the previous column`}
                      onClick={() => void applyPatch(task, { status: adjacentStatus(status, -1) })}
                    >
                      ←
                    </button>
                    <button
                      type="button"
                      disabled={status === 'done'}
                      aria-label={`Move ${task.title} to the next column`}
                      onClick={() => void applyPatch(task, { status: adjacentStatus(status, 1) })}
                    >
                      →
                    </button>
                  </div>
                </article>
              ))}
            </section>
          ))}
        </div>
      )}
    </section>
  );
}

function NewTaskForm({
  projects,
  defaultProjectId,
  onCreated,
  onError,
}: {
  projects: Project[];
  defaultProjectId: string | null;
  onCreated: () => void;
  onError: (error: unknown) => void;
}) {
  const [title, setTitle] = useState('');
  const [priority, setPriority] = useState<TaskPriority>('medium');
  const [dueDate, setDueDate] = useState('');
  const [projectId, setProjectId] = useState(defaultProjectId ?? '');

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    try {
      await api.createTask({
        title,
        priority,
        dueDate: dueDate || undefined,
        projectId: projectId || undefined,
      });
      setTitle('');
      setDueDate('');
      onCreated();
    } catch (err) {
      onError(err);
    }
  };

  return (
    <form className="inline-form wrap" onSubmit={submit}>
      <label className="inline-label grow">
        <span>Title</span>
        <input value={title} maxLength={200} required onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label className="inline-label">
        <span>Priority</span>
        <select value={priority} onChange={(e) => setPriority(e.target.value as TaskPriority)}>
          {PRIORITIES.map((p) => (
            <option key={p} value={p}>
              {p}
            </option>
          ))}
        </select>
      </label>
      <label className="inline-label">
        <span>Due date (optional)</span>
        <input type="date" value={dueDate} onChange={(e) => setDueDate(e.target.value)} />
      </label>
      <label className="inline-label">
        <span>Project</span>
        <select value={projectId} onChange={(e) => setProjectId(e.target.value)}>
          <option value="">None</option>
          {projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>
      <button type="submit" className="primary" disabled={!title.trim()}>
        Add task
      </button>
    </form>
  );
}
