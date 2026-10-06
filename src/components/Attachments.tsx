import { useCallback, useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';
import type { Attachment } from '../lib/types';

function sizeText(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Files attached to one page. Attaching copies the file into the workspace; the original stays
 * where it is. Programs and scripts are refused.
 */
export function Attachments({ pageId, onError }: { pageId: string; onError: (e: unknown) => void }) {
  const [items, setItems] = useState<Attachment[] | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(() => {
    api.attachmentsList(pageId).then(setItems).catch(onError);
  }, [pageId, onError]);

  useEffect(() => {
    setItems(null);
    load();
  }, [load]);

  const add = async () => {
    setNotice(null);
    try {
      const file = await open({ multiple: false, title: 'Choose a file to attach' });
      if (typeof file !== 'string') return;
      const added = await api.attachmentAdd(pageId, file);
      setNotice(added.readStatus ?? null);
      load();
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const remove = async (item: Attachment) => {
    if (
      !window.confirm(
        `Remove "${item.fileName}" from this page? The stored copy is deleted. Your original file is not touched.`,
      )
    )
      return;
    try {
      await api.attachmentRemove(item.id);
      load();
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  return (
    <section className="attachments" aria-labelledby="attachments-heading">
      <div className="section-head">
        <h3 id="attachments-heading">Attachments</h3>
        <button type="button" onClick={() => void add()}>
          Attach a file…
        </button>
      </div>
      {items === null && (
        <p role="status" className="muted small">
          Loading…
        </p>
      )}
      {items?.length === 0 && <p className="muted small">No files attached to this page.</p>}
      {items && items.length > 0 && (
        <ul className="plain-list">
          {items.map((item) => (
            <li key={item.id} className="attachment-row">
              <span>{item.fileName}</span>
              <span className="muted small">{sizeText(item.size)}</span>
              <button
                type="button"
                onClick={() => void api.attachmentReveal(item.id).catch((e) => setNotice(messageFor(e)))}
              >
                Show in folder
              </button>
              <button type="button" className="danger-quiet" onClick={() => void remove(item)}>
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
      {notice && (
        <p role="alert" className="inline-error">
          {notice}
        </p>
      )}
    </section>
  );
}
