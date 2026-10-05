import { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';
import type { WorkspaceInfo, WorkspaceListItem } from '../lib/types';

interface Props {
  onSwitched: (info: WorkspaceInfo) => void;
  onNewWorkspace: () => void;
  onError: (err: unknown) => void;
}

/**
 * Lists the workspaces this installation knows about. Each workspace has its own pages, tasks and
 * assistant history. Switching changes which one is open; forgetting only removes it from this list.
 */
export function WorkspacesPanel({ onSwitched, onNewWorkspace, onError }: Props) {
  const [items, setItems] = useState<WorkspaceListItem[]>([]);
  const [notice, setNotice] = useState<string | null>(null);
  const [name, setName] = useState('');

  const load = useCallback(() => {
    api.listWorkspaces().then(setItems).catch(onError);
  }, [onError]);

  useEffect(() => {
    load();
  }, [load]);

  const switchTo = async (path: string) => {
    setNotice(null);
    try {
      onSwitched(await api.switchWorkspace(path));
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const rename = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim()) return;
    setNotice(null);
    try {
      await api.renameWorkspace(name.trim());
      setName('');
      setNotice('Renamed.');
      load();
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const forget = async (item: WorkspaceListItem) => {
    if (!window.confirm(`Remove "${item.name}" from this list? Its folder and pages are kept.`)) return;
    setNotice(null);
    try {
      await api.forgetWorkspace(item.path);
      load();
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  return (
    <section className="panel" aria-labelledby="workspaces-heading">
      <h2 id="workspaces-heading">Workspaces</h2>
      <p className="muted small">
        Each workspace keeps its own pages, tasks, meetings, recipes and assistant history in its own folder. The
        assistant only reads other workspaces when you ask it to, and it never changes them.
      </p>
      <ul className="list" aria-label="Known workspaces">
        {items.map((item) => (
          <li key={item.path} className="row">
            <span>
              <strong>{item.name}</strong>
              <span className="muted small"> {item.path}</span>
              {item.active && <span className="small"> (open)</span>}
              {!item.available && <span className="small"> (folder or database not found)</span>}
            </span>
            {!item.active && item.available && (
              <button type="button" onClick={() => void switchTo(item.path)}>
                Switch
              </button>
            )}
            {!item.active && (
              <button type="button" onClick={() => void forget(item)}>
                Remove from list
              </button>
            )}
          </li>
        ))}
      </ul>
      <div className="row">
        <button type="button" onClick={onNewWorkspace}>
          Create or open another workspace
        </button>
      </div>
      <form className="row" onSubmit={rename}>
        <label className="field">
          <span>Rename the open workspace</span>
          <input value={name} maxLength={80} onChange={(e) => setName(e.target.value)} />
        </label>
        <button type="submit" disabled={!name.trim()}>
          Rename
        </button>
      </form>
      {notice && (
        <p role="status" className="muted small">
          {notice}
        </p>
      )}
    </section>
  );
}
