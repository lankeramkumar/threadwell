import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import { formatTimestamp, messageFor } from '../lib/pure';
import type { RunSummary, ToolTrace } from '../lib/types';

/**
 * Recent assistant runs with timing and token counts. Selecting a run shows each tool call with
 * its outcome. Model reasoning is not stored or shown; only the actions and their results are.
 */
export function RunHistory({ onError }: { onError: (e: unknown) => void }) {
  const [runs, setRuns] = useState<RunSummary[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [trace, setTrace] = useState<ToolTrace[]>([]);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    api.aiListRuns(30).then(setRuns).catch(onError);
  }, [onError]);

  useEffect(() => {
    if (!selected) {
      setTrace([]);
      return;
    }
    setMessage(null);
    api
      .aiRunTrace(selected)
      .then(setTrace)
      .catch((err) => setMessage(messageFor(err)));
  }, [selected]);

  const removeRun = async () => {
    if (!selected) return;
    if (!window.confirm("Delete this run's trace? Suggestions and pages are kept.")) return;
    try {
      await api.aiDeleteRun(selected);
      setSelected(null);
      setRuns(await api.aiListRuns(30));
    } catch (err) {
      setMessage(messageFor(err));
    }
  };

  const totals = (runs ?? []).reduce(
    (acc, r) => ({
      duration: acc.duration + (r.durationMs ?? 0),
      prompt: acc.prompt + (r.promptTokens ?? 0),
      output: acc.output + (r.outputTokens ?? 0),
    }),
    { duration: 0, prompt: 0, output: 0 },
  );

  return (
    <section className="panel" aria-labelledby="runs-heading">
      <h2 id="runs-heading">Assistant history</h2>
      {runs === null && <p role="status">Loading…</p>}
      {runs?.length === 0 && <p className="muted small">No assistant runs yet.</p>}
      {runs && runs.length > 0 && (
        <>
          <p className="muted small">
            Last {runs.length} runs: {(totals.duration / 1000).toFixed(1)} s total, {totals.prompt} prompt tokens,{' '}
            {totals.output} output tokens (counts reported by the model server).
          </p>
          <table className="task-table">
            <caption className="visually-hidden">Recent assistant runs</caption>
            <thead>
              <tr>
                <th scope="col">When</th>
                <th scope="col">Kind</th>
                <th scope="col">Status</th>
                <th scope="col">Steps</th>
                <th scope="col">Time</th>
                <th scope="col">Model</th>
              </tr>
            </thead>
            <tbody>
              {runs.map((r) => (
                <tr key={r.id} className={r.id === selected ? 'is-selected' : undefined}>
                  <td>
                    <button
                      type="button"
                      className="link-button"
                      aria-pressed={r.id === selected}
                      onClick={() => setSelected(r.id)}
                    >
                      {formatTimestamp(r.startedAt)}
                    </button>
                  </td>
                  <td>{r.kind.replace('_', ' ')}</td>
                  <td>
                    {r.status}
                    {r.errorCategory ? ` (${r.errorCategory})` : ''}
                  </td>
                  <td>{r.steps}</td>
                  <td>{r.durationMs !== null ? `${(r.durationMs / 1000).toFixed(1)} s` : '—'}</td>
                  <td>{r.model}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      {selected && (
        <div aria-label="Tool trace">
          <h3>Tool trace</h3>
          <button type="button" className="danger-quiet" onClick={() => void removeRun()}>
            Delete this run's trace
          </button>
          {message && <p className="inline-error">{message}</p>}
          {trace.length === 0 && !message && <p className="muted small">No tool calls in this run.</p>}
          <ol>
            {trace.map((t) => (
              <li key={`${t.step}-${t.tool}-${t.summary}`}>
                <span className="muted small">step {t.step}</span> {t.ok ? '✓' : '✕'} <code>{t.tool}</code>: {t.summary}
                {t.errorCategory ? <span className="muted small"> ({t.errorCategory})</span> : null}
              </li>
            ))}
          </ol>
        </div>
      )}
    </section>
  );
}
