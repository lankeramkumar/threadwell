import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';
import type { ImportReport, ScanItem, ScanReport } from '../lib/types';

const STATUS_TEXT: Record<ScanItem['status'], string> = {
  new: 'New',
  imported: 'Already imported (unchanged)',
  changed: 'Changed since import: imports again as a new page',
  unsupported: 'Not supported (export it as Markdown, text, Word or PDF first)',
  too_large: 'Larger than 5 MB',
};

function importable(item: ScanItem): boolean {
  return item.status === 'new' || item.status === 'changed';
}

/**
 * Imports notes from a folder on disk. Originals are never changed. Already-imported, unchanged
 * files are skipped, so running the import twice does not duplicate pages.
 */
export function FolderImport({ onImported, onError }: { onImported: () => void; onError: (e: unknown) => void }) {
  const [folder, setFolder] = useState<string | null>(null);
  const [scan, setScan] = useState<ScanReport | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [report, setReport] = useState<ImportReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const chooseFolder = async () => {
    setNotice(null);
    setReport(null);
    try {
      const dir = await open({ directory: true, multiple: false, title: 'Choose a folder of notes' });
      if (typeof dir !== 'string') return;
      setBusy(true);
      const result = await api.localScanFolder(dir);
      setFolder(dir);
      setScan(result);
      setSelected(new Set(result.items.filter((i) => i.status === 'new').map((i) => i.relativePath)));
    } catch (err) {
      setNotice(messageFor(err));
    } finally {
      setBusy(false);
    }
  };

  const toggle = (path: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const runImport = async () => {
    if (!folder || selected.size === 0) return;
    setBusy(true);
    setNotice(null);
    try {
      const result = await api.localImportFolder(folder, [...selected], null);
      setReport(result);
      onImported();
      const rescan = await api.localScanFolder(folder);
      setScan(rescan);
      setSelected(new Set());
    } catch (err) {
      setNotice(messageFor(err));
      onError(err);
    } finally {
      setBusy(false);
    }
  };

  const importableItems = scan?.items.filter(importable) ?? [];

  return (
    <section className="panel" aria-labelledby="folder-heading">
      <h2 id="folder-heading">Import a folder of notes</h2>
      <p className="muted small">
        Reads Markdown (.md), text (.txt), Word (.docx) and text-based PDF files, one note per file. Your files are
        never changed or moved. Scanned PDFs and OneNote (.one) files are not supported.
      </p>

      <div className="row">
        <button type="button" onClick={() => void chooseFolder()} disabled={busy}>
          Choose folder…
        </button>
        {folder && <code className="path">{folder}</code>}
      </div>

      {scan && (
        <>
          {scan.truncated && (
            <p role="alert" className="inline-error">
              The folder has more than 2,000 notes. Only the first 2,000 are listed. Split the folder and import the
              rest.
            </p>
          )}
          {scan.items.length === 0 && <p className="muted small">No notes found in this folder.</p>}
          {scan.items.length > 0 && (
            <>
              <div className="row">
                <button
                  type="button"
                  onClick={() => setSelected(new Set(importableItems.map((i) => i.relativePath)))}
                  disabled={busy}
                >
                  Select all importable
                </button>
                <button type="button" onClick={() => setSelected(new Set())} disabled={busy}>
                  Clear selection
                </button>
                <button
                  type="button"
                  className="primary"
                  onClick={() => void runImport()}
                  disabled={busy || selected.size === 0}
                >
                  Import {selected.size} note{selected.size === 1 ? '' : 's'}
                </button>
              </div>
              <div className="table-wrap">
                <table className="task-table">
                  <caption className="visually-hidden">Notes found in the folder</caption>
                  <thead>
                    <tr>
                      <th scope="col">Import</th>
                      <th scope="col">File</th>
                      <th scope="col">Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {scan.items.map((item) => (
                      <tr key={item.relativePath}>
                        <td>
                          <input
                            type="checkbox"
                            aria-label={`Import ${item.relativePath}`}
                            disabled={!importable(item) || busy}
                            checked={selected.has(item.relativePath)}
                            onChange={() => toggle(item.relativePath)}
                          />
                        </td>
                        <td>{item.relativePath}</td>
                        <td className="muted small">{STATUS_TEXT[item.status]}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </>
      )}

      {report && (
        <div role="status" className="small">
          <p>
            Imported {report.imported} note{report.imported === 1 ? '' : 's'}. Skipped {report.skipped} already
            imported.
          </p>
          {report.failed.length > 0 && (
            <ul>
              {report.failed.map((f) => (
                <li key={f.relativePath}>
                  {f.relativePath}: {f.reason}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {notice && (
        <p role="alert" className="inline-error">
          {notice}
        </p>
      )}
    </section>
  );
}
