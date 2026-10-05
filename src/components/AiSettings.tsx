import { useEffect, useState, type FormEvent } from 'react';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';
import type { AiReadiness, AiStatus } from '../lib/types';

const READINESS_TEXT: Record<AiReadiness, string> = {
  ready: 'Connected. The model is installed and answering.',
  model_missing: 'The server answered, but the model is not installed.',
  unreachable: 'No model server answered at this endpoint.',
  not_configured: 'Not set up yet.',
};

/**
 * Local model settings. Only loopback endpoints work unless remote endpoints are enabled.
 * No API key is stored in this build; Ollama needs none.
 */
export function AiSettings({ onError }: { onError: (error: unknown) => void }) {
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [endpoint, setEndpoint] = useState('');
  const [model, setModel] = useState('');
  const [allowRemote, setAllowRemote] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  const load = () =>
    api
      .aiGetStatus()
      .then((s) => {
        setStatus(s);
        setEndpoint(s.config.endpoint);
        setModel(s.config.model);
        setAllowRemote(s.config.allowRemote);
      })
      .catch(onError);

  useEffect(() => {
    void load();
  }, []);

  const save = async (event: FormEvent) => {
    event.preventDefault();
    setSaved(null);
    try {
      await api.aiSaveConfig(endpoint, model, allowRemote);
      setSaved('Saved.');
      setChecking(true);
      await load();
      setChecking(false);
    } catch (err) {
      setSaved(messageFor(err));
    }
  };

  const state = status?.state ?? 'not_configured';

  return (
    <section aria-labelledby="ai-heading" className="panel">
      <h2 id="ai-heading">AI assistance (local model)</h2>
      <p className="muted small">
        Threadwell talks to a model server on this computer, such as <a href="https://ollama.com">Ollama</a>. Install
        Ollama, then run <code>ollama pull &lt;model&gt;</code> for the model you choose. Threadwell does not install or
        start Ollama and does not bundle model files.
      </p>

      <p role="status" className={`status-text status-${state}`}>
        {checking ? 'Checking…' : READINESS_TEXT[state]}
      </p>

      <form className="stack" onSubmit={save}>
        <label className="field">
          <span>Model server address</span>
          <input value={endpoint} onChange={(e) => setEndpoint(e.target.value)} placeholder="http://127.0.0.1:11434" />
        </label>
        <label className="field">
          <span>Model name (for example qwen2.5:3b)</span>
          <input value={model} onChange={(e) => setModel(e.target.value)} placeholder="qwen2.5:3b" />
        </label>
        <label className="checkbox small">
          <input type="checkbox" checked={allowRemote} onChange={(e) => setAllowRemote(e.target.checked)} />
          Allow a server on another machine (https only). Your notes would be sent to that server.
        </label>
        <div className="row">
          <button type="submit" className="primary">
            Save and check
          </button>
          {saved && (
            <span role="status" className="muted small">
              {saved}
            </span>
          )}
        </div>
      </form>
    </section>
  );
}
