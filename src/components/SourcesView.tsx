import { useCallback, useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';
import type { PageSummary, SourceInfo, SyncReport } from '../lib/types';

const SHOWN_FILES = 300;

interface Props {
  pages: PageSummary[];
  onOpenPage: (id: string) => void;
  onChanged: () => void;
  onError: (err: unknown) => void;
  /** Puts a question about a folder in the assistant box. */
  onAskAbout?: (text: string) => void;
}

function describe(report: SyncReport): string {
  const parts = [
    `${report.added} added`,
    `${report.updated} updated`,
    `${report.removed} removed`,
    `${report.unchanged} unchanged`,
  ];
  if (report.skipped) parts.push(`${report.skipped} skipped (too large, binary or ignored)`);
  if (report.failed) parts.push(`${report.failed} could not be read`);
  let text = parts.join(', ') + '.';
  if (report.truncated) text += ' The folder has more than 5,000 readable files, so only the first 5,000 were linked.';
  if (report.stopped) text += ' The sync stopped because you switched workspace.';
  return text;
}

/**
 * Linked source folders. Each readable file is a read-only page, so the assistant and search use it
 * like any other note. Changes in the folder are picked up while Threadwell is open.
 */
export function SourcesView({ pages, onOpenPage, onChanged, onError, onAskAbout }: Props) {
  const [sources, setSources] = useState<SourceInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);

  const load = useCallback(() => {
    return api
      .sourcesList()
      .then(setSources)
      .catch((err) => onError(err));
  }, [onError]);

  useEffect(() => {
    void load();
  }, [load]);

  const link = async () => {
    setNotice(null);
    try {
      const dir = await open({ directory: true, multiple: false, title: 'Choose a folder to link' });
      if (typeof dir !== 'string') return;
      setBusy('link');
      const added = await api.sourcesAdd(dir);
      setNotice(`Linked ${added.name}. ${added.lastSummary ?? ''}`.trim());
      await load();
      onChanged();
    } catch (err) {
      setNotice(messageFor(err));
    } finally {
      setBusy(null);
    }
  };

  const sync = async (source: SourceInfo) => {
    setNotice(null);
    setBusy(source.id);
    try {
      const report = await api.sourcesSync(source.id);
      setNotice(`${source.name}: ${describe(report)}`);
      await load();
      onChanged();
    } catch (err) {
      setNotice(messageFor(err));
      await load();
    } finally {
      setBusy(null);
    }
  };

  const remove = async (source: SourceInfo) => {
    if (
      !window.confirm(
        `Unlink "${source.name}"? Its files are moved to Trash. The folder on your computer is not changed.`,
      )
    )
      return;
    setNotice(null);
    setBusy(source.id);
    try {
      await api.sourcesRemove(source.id);
      setNotice(`Unlinked ${source.name}.`);
      await load();
      onChanged();
    } catch (err) {
      setNotice(messageFor(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="settings" aria-labelledby="sources-title">
      <h1 id="sources-title">Sources</h1>
      <p className="muted small">
        Link a folder of documents or a project folder with source code. Threadwell reads the files and keeps them up to
        date while it is open. Ask the assistant questions about them, and it cites the file each answer comes from.
        Linked files are read only: change them in their folder, and Threadwell picks up the change.
      </p>

      <div className="row">
        <button type="button" className="primary" onClick={() => void link()} disabled={busy !== null}>
          Link a folder…
        </button>
      </div>

      {notice && (
        <p role="status" className="muted small">
          {notice}
        </p>
      )}

      {sources === null && <p role="status">Loading…</p>}
      {sources?.length === 0 && (
        <p className="muted small">
          No folders are linked yet. Supported files: Markdown, text, Word, text-based PDF, CSV, and source or
          configuration files such as .rs, .py, .ts, .json and .yaml. Ignored: .git, node_modules, target, build output,
          lock files, hidden files, and anything listed in the folder's .gitignore.
        </p>
      )}

      {sources?.map((source) => {
        const files = pages
          .filter((p) => p.sourceId === source.id && p.deletedAt === null)
          .sort((a, b) => a.title.localeCompare(b.title));
        const isOpen = expanded === source.id;
        return (
          <section key={source.id} className="panel" aria-labelledby={`source-${source.id}`}>
            <h2 id={`source-${source.id}`}>{source.name}</h2>
            <p className="muted small">
              <code>{source.rootPath}</code>
            </p>
            <p className="small">
              {source.fileCount} file{source.fileCount === 1 ? '' : 's'} linked.
              {source.lastSyncedAt
                ? ` Last synced ${new Date(source.lastSyncedAt).toLocaleString()}: ${source.lastSummary ?? ''}`
                : ' Not synced yet.'}
            </p>
            <div className="button-row">
              <button type="button" onClick={() => void sync(source)} disabled={busy !== null}>
                {busy === source.id ? 'Syncing…' : 'Sync now'}
              </button>
              {onAskAbout && (
                <button
                  type="button"
                  onClick={() =>
                    onAskAbout(`Summarise the ${source.name} folder: what it contains and how its parts fit together.`)
                  }
                >
                  Ask about this folder
                </button>
              )}
              <button type="button" onClick={() => setExpanded(isOpen ? null : source.id)} aria-expanded={isOpen}>
                {isOpen ? 'Hide files' : `Show files (${files.length})`}
              </button>
              <button
                type="button"
                className="danger-quiet"
                onClick={() => void remove(source)}
                disabled={busy !== null}
              >
                Unlink
              </button>
            </div>
            {isOpen && (
              <ul className="plain-list" aria-label={`Files in ${source.name}`}>
                {files.slice(0, SHOWN_FILES).map((file) => (
                  <li key={file.id}>
                    <button type="button" className="nav-button" onClick={() => onOpenPage(file.id)}>
                      {file.title}
                    </button>
                  </li>
                ))}
                {files.length > SHOWN_FILES && (
                  <li className="muted small">
                    Showing the first {SHOWN_FILES} of {files.length} files.
                  </li>
                )}
              </ul>
            )}
          </section>
        );
      })}
    </section>
  );
}
