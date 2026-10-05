import { useEffect, useState, type FormEvent } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from '../lib/api';
import { formatTimestamp, messageFor } from '../lib/pure';
import type { Proposal, Recipe, RecipeDoneEvent, RecipeInput, RecipeRun, ScheduleKind } from '../lib/types';
import { ProposalCard } from './AssistantPanel';

const WEEKDAYS = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];

function systemTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
  } catch {
    return 'UTC';
  }
}

function describe(recipe: Recipe): string {
  switch (recipe.scheduleKind) {
    case 'manual':
      return 'Manual only';
    case 'daily':
      return `Daily at ${recipe.scheduleTime} (${recipe.timezone})`;
    case 'weekly':
      return `Weekly, ${WEEKDAYS[recipe.weekday ?? 0]} at ${recipe.scheduleTime} (${recipe.timezone})`;
  }
}

/**
 * Recurring drafts. Runs happen only while Threadwell is open. Each result is a draft proposal
 * that needs review; nothing is written to a page automatically.
 */
export function RecipesView({ onError }: { onError: (e: unknown) => void }) {
  const [recipes, setRecipes] = useState<Recipe[] | null>(null);
  const [draft, setDraft] = useState<Proposal | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [runs, setRuns] = useState<RecipeRun[]>([]);
  const [editing, setEditing] = useState<Recipe | 'new' | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = () => {
    api.recipesList().then(setRecipes).catch(onError);
  };

  useEffect(() => {
    load();
    const off = listen<RecipeDoneEvent>('recipes://done', () => {
      load();
      if (selected)
        api
          .recipeRunsList(selected)
          .then(setRuns)
          .catch(() => undefined);
    });
    return () => {
      off.then((u) => u());
    };
    // The listener is re-created when the selection changes, so it reads the current selection.
  }, [selected]);

  useEffect(() => {
    if (!selected) {
      setRuns([]);
      return;
    }
    api.recipeRunsList(selected).then(setRuns).catch(onError);
  }, [selected, onError]);

  const openDraft = async (proposalId: string) => {
    try {
      const all = await api.aiListProposals(null);
      setDraft(all.find((p) => p.id === proposalId) ?? null);
    } catch (err) {
      onError(err);
    }
  };

  const save = async (input: RecipeInput, id: string | null) => {
    try {
      if (id) await api.recipesUpdate(id, input);
      else await api.recipesCreate(input);
      setEditing(null);
      load();
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const runNow = async (recipe: Recipe) => {
    setNotice(null);
    try {
      await api.recipeRunNow(recipe.id);
      setSelected(recipe.id);
      setNotice('Started. The draft will appear in the run history and in your proposals.');
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const toggle = async (recipe: Recipe) => {
    try {
      await api.recipesUpdate(recipe.id, {
        name: recipe.name,
        prompt: recipe.prompt,
        scheduleKind: recipe.scheduleKind,
        scheduleTime: recipe.scheduleTime,
        weekday: recipe.weekday,
        timezone: recipe.timezone,
        enabled: !recipe.enabled,
      });
      load();
    } catch (err) {
      onError(err);
    }
  };

  const remove = async (recipe: Recipe) => {
    if (!window.confirm(`Delete the recipe "${recipe.name}" and its run history? Drafts already created are kept.`))
      return;
    try {
      await api.recipesDelete(recipe.id);
      if (selected === recipe.id) setSelected(null);
      load();
    } catch (err) {
      onError(err);
    }
  };

  const current = recipes?.find((r) => r.id === selected) ?? null;

  return (
    <section className="recipes-view" aria-labelledby="recipes-title">
      <header className="view-header">
        <h1 id="recipes-title">Recipes</h1>
        <button type="button" className="primary" onClick={() => setEditing('new')}>
          New recipe
        </button>
      </header>
      <p className="muted small">
        Recipes run while Threadwell is open. A missed schedule runs once when the app is next open. Results are drafts
        for you to review.
      </p>

      {notice && (
        <p role="status" className="muted">
          {notice}
        </p>
      )}

      {editing && (
        <RecipeForm
          initial={editing === 'new' ? null : editing}
          onCancel={() => setEditing(null)}
          onSave={(input) => void save(input, editing === 'new' ? null : editing.id)}
        />
      )}

      {recipes === null && <p role="status">Loading recipes…</p>}
      {recipes !== null && recipes.length === 0 && (
        <p className="muted">No recipes yet. Create one to draft a recurring update.</p>
      )}

      {recipes !== null && recipes.length > 0 && (
        <ul className="recipe-list">
          {recipes.map((recipe) => (
            <li key={recipe.id} className={recipe.id === selected ? 'recipe-row is-selected' : 'recipe-row'}>
              <div>
                <button
                  type="button"
                  className="link-button"
                  aria-pressed={recipe.id === selected}
                  onClick={() => setSelected(recipe.id)}
                >
                  {recipe.name}
                </button>
                <div className="muted small">
                  {describe(recipe)}
                  {recipe.nextRunAt && recipe.enabled ? ` · next ${formatTimestamp(recipe.nextRunAt)}` : ''}
                  {!recipe.enabled ? ' · paused' : ''}
                </div>
              </div>
              <div className="row tight">
                <button type="button" onClick={() => void runNow(recipe)}>
                  Run now
                </button>
                <button type="button" onClick={() => void toggle(recipe)}>
                  {recipe.enabled ? 'Pause' : 'Resume'}
                </button>
                <button type="button" onClick={() => setEditing(recipe)}>
                  Edit
                </button>
                <button type="button" className="danger-quiet" onClick={() => void remove(recipe)}>
                  Delete
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {draft && (
        <section aria-label="Draft to review">
          <h2>Draft to review</h2>
          <ProposalCard proposal={draft} onChanged={() => void openDraft(draft.id)} onError={onError} />
        </section>
      )}

      {current && (
        <section className="panel" aria-label={`Run history for ${current.name}`}>
          <h2>Run history</h2>
          {runs.length === 0 && <p className="muted small">No runs yet.</p>}
          <ul className="plain-list">
            {runs.map((run) => (
              <li key={run.id} className="run-row">
                <span>{formatTimestamp(run.startedAt)}</span>
                <span className="muted small">
                  {run.trigger === 'catch_up' ? 'catch-up' : run.trigger}
                  {run.durationMs !== null ? ` · ${(run.durationMs / 1000).toFixed(1)} s` : ''}
                </span>
                <span className={`status-text status-${run.status}`}>{run.status}</span>
                {run.proposalId && (
                  <button type="button" className="link-button" onClick={() => void openDraft(run.proposalId!)}>
                    Review draft
                  </button>
                )}
                {run.message && run.status === 'failed' && <span className="muted small">{run.message}</span>}
              </li>
            ))}
          </ul>
        </section>
      )}
    </section>
  );
}

function RecipeForm({
  initial,
  onSave,
  onCancel,
}: {
  initial: Recipe | null;
  onSave: (input: RecipeInput) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(initial?.name ?? '');
  const [prompt, setPrompt] = useState(
    initial?.prompt ?? 'Draft a weekly update from completed tasks. Group them by project.',
  );
  const [kind, setKind] = useState<ScheduleKind>(initial?.scheduleKind ?? 'weekly');
  const [time, setTime] = useState(initial?.scheduleTime ?? '09:00');
  const [weekday, setWeekday] = useState(initial?.weekday ?? 4);
  const [timezone, setTimezone] = useState(initial?.timezone ?? systemTimezone());
  const [enabled, setEnabled] = useState(initial?.enabled ?? true);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    onSave({
      name,
      prompt,
      scheduleKind: kind,
      scheduleTime: kind === 'manual' ? null : time,
      weekday: kind === 'weekly' ? weekday : null,
      timezone,
      enabled,
    });
  };

  return (
    <form className="panel stack" onSubmit={submit} aria-label={initial ? 'Edit recipe' : 'New recipe'}>
      <label className="field">
        <span>Name</span>
        <input value={name} maxLength={120} required onChange={(e) => setName(e.target.value)} />
      </label>
      <label className="field">
        <span>Instructions</span>
        <textarea rows={4} value={prompt} maxLength={2000} required onChange={(e) => setPrompt(e.target.value)} />
      </label>
      <div className="row wrap">
        <label className="inline-label">
          <span>Schedule</span>
          <select value={kind} onChange={(e) => setKind(e.target.value as ScheduleKind)}>
            <option value="manual">Manual only</option>
            <option value="daily">Daily</option>
            <option value="weekly">Weekly</option>
          </select>
        </label>
        {kind === 'weekly' && (
          <label className="inline-label">
            <span>Day</span>
            <select value={weekday} onChange={(e) => setWeekday(Number(e.target.value))}>
              {WEEKDAYS.map((d, i) => (
                <option key={d} value={i}>
                  {d}
                </option>
              ))}
            </select>
          </label>
        )}
        {kind !== 'manual' && (
          <label className="inline-label">
            <span>Time</span>
            <input type="time" value={time} onChange={(e) => setTime(e.target.value)} required />
          </label>
        )}
        {kind !== 'manual' && (
          <label className="inline-label">
            <span>Timezone</span>
            <input value={timezone} maxLength={64} onChange={(e) => setTimezone(e.target.value)} required />
          </label>
        )}
      </div>
      <label className="checkbox small">
        <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
        Enabled
      </label>
      <div className="row">
        <button type="submit" className="primary">
          Save recipe
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
