import { useState, type FormEvent } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';
import type { WorkspaceInfo } from '../lib/types';

interface Props {
  onReady: (workspace: WorkspaceInfo) => void;
  onError: (message: string) => void;
  error: string | null;
}

/** First-run screen: create a workspace folder or open an existing one. No account needed. */
export function Onboarding({ onReady, onError, error }: Props) {
  const [mode, setMode] = useState<'create' | 'open'>('create');
  const [name, setName] = useState('My workspace');
  const [folder, setFolder] = useState<string | null>(null);
  const [withSample, setWithSample] = useState(true);
  const [busy, setBusy] = useState(false);

  const pickFolder = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: mode === 'create' ? 'Choose a folder for the new workspace' : 'Choose a Threadwell workspace folder',
    });
    if (typeof selected === 'string') setFolder(selected);
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!folder) {
      onError('Choose a folder first.');
      return;
    }
    setBusy(true);
    try {
      const info =
        mode === 'create' ? await api.createWorkspace(folder, name, withSample) : await api.openWorkspace(folder);
      onReady(info);
    } catch (err) {
      onError(messageFor(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="onboarding">
      <header>
        <h1>Threadwell</h1>
        <p className="tagline">Your notes, projects and AI assistant in one desktop workspace.</p>
      </header>

      <div role="radiogroup" aria-label="Start" className="segmented">
        <label>
          <input
            type="radio"
            name="mode"
            value="create"
            checked={mode === 'create'}
            onChange={() => setMode('create')}
          />
          Create a workspace
        </label>
        <label>
          <input type="radio" name="mode" value="open" checked={mode === 'open'} onChange={() => setMode('open')} />
          Open an existing workspace
        </label>
      </div>

      <form onSubmit={submit} className="stack">
        {mode === 'create' && (
          <label className="field">
            <span>Workspace name</span>
            <input value={name} maxLength={80} onChange={(e) => setName(e.target.value)} required />
          </label>
        )}

        <div className="field">
          <span>Folder</span>
          <div className="row">
            <code className="path" aria-live="polite">
              {folder ?? 'No folder selected'}
            </code>
            <button type="button" onClick={pickFolder}>
              Choose folder…
            </button>
          </div>
        </div>

        {mode === 'create' && (
          <label className="checkbox">
            <input type="checkbox" checked={withSample} onChange={(e) => setWithSample(e.target.checked)} />
            Include sample notes, tasks and a meeting transcript, plus a small linked sample project of code and
            documents you can ask about
          </label>
        )}

        <button type="submit" className="primary" disabled={busy || !folder}>
          {busy ? 'Working…' : mode === 'create' ? 'Create workspace' : 'Open workspace'}
        </button>
        {error && (
          <p role="alert" className="inline-error">
            {error}
          </p>
        )}
      </form>

      <p className="fine-print">
        Data stays in the folder you choose. No account or internet connection is required. AI features are not part of
        this build yet.
      </p>
    </main>
  );
}
