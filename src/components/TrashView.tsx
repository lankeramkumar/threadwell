import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import { formatTimestamp } from '../lib/pure';
import type { PageSummary } from '../lib/types';

interface Props {
  onRestored: () => void;
  onError: (error: unknown) => void;
}

/** Trashed pages can be restored. Permanent deletion is not offered in this build. */
export function TrashView({ onRestored, onError }: Props) {
  const [items, setItems] = useState<PageSummary[] | null>(null);

  const load = () => {
    api.listTrash().then(setItems).catch(onError);
  };

  useEffect(load, []);

  const restore = async (id: string) => {
    try {
      await api.restorePage(id);
      onRestored();
      load();
    } catch (err) {
      onError(err);
    }
  };

  return (
    <section className="trash-view" aria-labelledby="trash-title">
      <h1 id="trash-title">Trash</h1>
      <p className="muted small">Pages moved to trash are hidden from the tree and search until you restore them.</p>
      {items === null && <p role="status">Loading…</p>}
      {items !== null && items.length === 0 && <p>Trash is empty.</p>}
      {items !== null && items.length > 0 && (
        <ul className="plain-list">
          {items.map((item) => (
            <li key={item.id} className="trash-row">
              <span className="trash-title">{item.title}</span>
              <span className="muted small">trashed {item.deletedAt ? formatTimestamp(item.deletedAt) : ''}</span>
              <button type="button" onClick={() => void restore(item.id)}>
                Restore
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
