import { useEffect, useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { api } from '../lib/api';
import { AiSettings } from './AiSettings';
import { FolderImport } from './FolderImport';
import { AudioTranscription } from './AudioTranscription';
import { AiRetrieval } from './AiRetrieval';
import { RunHistory } from './RunHistory';
import type { ThemeChoice } from '../lib/theme';
import type { WorkspaceInfo } from '../lib/types';

interface Props {
  workspace: WorkspaceInfo;
  onOpenPage: (id: string) => void;
  onPagesChanged: () => void;
  onThemeChange: (theme: ThemeChoice) => void;
  onRestored: (workspace: WorkspaceInfo) => void;
  onError: (error: unknown) => void;
}

/** Workspace settings, import/export, backup and restore. */
export function SettingsView({ workspace, onOpenPage, onThemeChange, onRestored, onPagesChanged, onError }: Props) {
  const [theme, setTheme] = useState<ThemeChoice>('system');
  const [result, setResult] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    api
      .getSettings()
      .then((s) => setTheme(s.theme))
      .catch(onError);
  }, [onError]);

  const run = async (label: string, action: () => Promise<string | void>) => {
    setBusy(label);
    setResult(null);
    try {
      const message = await action();
      if (message) setResult(message);
    } catch (err) {
      onError(err);
    } finally {
      setBusy(null);
    }
  };

  const changeTheme = (value: ThemeChoice) => {
    setTheme(value);
    onThemeChange(value);
    void run('theme', () => api.setSetting('theme', value).then(() => undefined));
  };

  const exportAll = () =>
    run('export-md', async () => {
      const dir = await open({ directory: true, multiple: false, title: 'Choose a folder for the Markdown files' });
      if (typeof dir !== 'string') return;
      const count = await api.exportPagesMarkdown(dir);
      return `Exported ${count} page${count === 1 ? '' : 's'} as Markdown.`;
    });

  const exportCsv = () =>
    run('export-csv', async () => {
      const file = await save({
        title: 'Export tasks',
        defaultPath: 'threadwell-tasks.csv',
        filters: [{ name: 'CSV', extensions: ['csv'] }],
      });
      if (!file) return;
      const count = await api.exportTasksCsv(file);
      return `Exported ${count} task${count === 1 ? '' : 's'} to CSV.`;
    });

  const importMd = () =>
    run('import', async () => {
      const file = await open({
        multiple: false,
        title: 'Choose a Markdown file to import',
        filters: [{ name: 'Markdown or text', extensions: ['md', 'markdown', 'txt'] }],
      });
      if (typeof file !== 'string') return;
      const page = await api.importMarkdown(file, null);
      onOpenPage(page.id);
      return `Imported "${page.title}". The original file was not changed.`;
    });

  const backup = () =>
    run('backup', async () => {
      const dir = await open({ directory: true, multiple: false, title: 'Choose a folder for the backup' });
      if (typeof dir !== 'string') return;
      const info = await api.createBackup(dir);
      return `Backup saved to ${info.path} (${info.pages} pages, ${info.tasks} tasks).`;
    });

  const restore = () =>
    run('restore', async () => {
      const backupDir = await open({ directory: true, multiple: false, title: 'Choose a Threadwell backup folder' });
      if (typeof backupDir !== 'string') return;
      const destDir = await open({
        directory: true,
        multiple: false,
        title: 'Choose an empty folder for the restored workspace',
      });
      if (typeof destDir !== 'string') return;
      const confirmed = window.confirm(
        'The restored workspace will open in place of the current one. The current workspace will not be changed. Continue?',
      );
      if (!confirmed) return;
      onRestored(await api.restoreBackup(backupDir, destDir));
    });

  const rebuildIndex = () =>
    run('index', async () => {
      const count = await api.rebuildSearchIndex();
      return `Search index rebuilt for ${count} page${count === 1 ? '' : 's'}.`;
    });

  return (
    <section className="settings" aria-labelledby="settings-title">
      <h1 id="settings-title">Settings</h1>

      <section aria-labelledby="ws-heading" className="panel">
        <h2 id="ws-heading">Workspace</h2>
        <dl className="facts">
          <dt>Name</dt>
          <dd>{workspace.name}</dd>
          <dt>Location</dt>
          <dd>
            <code>{workspace.path}</code>
          </dd>
          <dt>Storage</dt>
          <dd>Local SQLite database. Works offline. No account.</dd>
        </dl>
      </section>

      <section aria-labelledby="appearance-heading" className="panel">
        <h2 id="appearance-heading">Appearance</h2>
        <label className="inline-label">
          <span>Theme</span>
          <select value={theme} onChange={(e) => changeTheme(e.target.value as ThemeChoice)}>
            <option value="system">Match system</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
      </section>

      <section aria-labelledby="transfer-heading" className="panel">
        <h2 id="transfer-heading">Import and export</h2>
        <p className="muted small">
          Exports never overwrite existing files. Imports copy content and leave the original file alone.
        </p>
        <div className="button-row">
          <button type="button" onClick={exportAll} disabled={busy !== null}>
            Export all pages to Markdown
          </button>
          <button type="button" onClick={exportCsv} disabled={busy !== null}>
            Export tasks to CSV
          </button>
          <button type="button" onClick={importMd} disabled={busy !== null}>
            Import a Markdown file
          </button>
        </div>
      </section>

      <FolderImport onImported={onPagesChanged} onError={onError} />
      <AudioTranscription onError={onError} />

      <section aria-labelledby="backup-heading" className="panel">
        <h2 id="backup-heading">Backup and restore</h2>
        <p className="muted small">
          A backup is a folder with a manifest, a consistent copy of the database, and attachments. It contains your
          data in plain form, so store it somewhere you trust.
        </p>
        <div className="button-row">
          <button type="button" onClick={backup} disabled={busy !== null}>
            Create backup
          </button>
          <button type="button" onClick={restore} disabled={busy !== null}>
            Restore from backup…
          </button>
        </div>
      </section>

      <section aria-labelledby="index-heading" className="panel">
        <h2 id="index-heading">Search index</h2>
        <p className="muted small">The search index is derived from your pages. Rebuild it if results look wrong.</p>
        <button type="button" onClick={rebuildIndex} disabled={busy !== null}>
          Rebuild search index
        </button>
      </section>

      <AiSettings onError={onError} />
      <AiRetrieval onError={onError} />
      <RunHistory onError={onError} />

      <section aria-labelledby="privacy-heading" className="panel">
        <h2 id="privacy-heading">Privacy</h2>
        <p>Threadwell collects no telemetry. In this build, no data leaves this computer.</p>
      </section>

      {busy && (
        <p role="status" className="muted">
          Working…
        </p>
      )}
      {result && <p role="status">{result}</p>}
    </section>
  );
}
