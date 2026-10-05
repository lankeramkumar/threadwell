import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';

/**
 * Optional local performance traces. Off by default. Traces stay on this computer and never
 * contain prompts, answers or page text.
 */
export function TelemetrySettings({ onError }: { onError: (e: unknown) => void }) {
  const [enabled, setEnabled] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    api.telemetryGetSettings().then((s) => setEnabled(s.localTraces)).catch(onError);
  }, [onError]);

  const change = async (value: boolean) => {
    setNotice(null);
    try {
      await api.telemetrySetSettings(value);
      setEnabled(value);
      setNotice('Saved. Restart Threadwell for the change to take effect.');
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const remove = async () => {
    if (!window.confirm('Delete all local performance traces?')) return;
    try {
      await api.telemetryDeleteTraces();
      setNotice('Traces deleted.');
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  return (
    <section className="panel" aria-labelledby="telemetry-heading">
      <h2 id="telemetry-heading">Performance traces (local only)</h2>
      <p className="muted small">
        When on, Threadwell records timing for assistant runs, steps, model calls and tool calls in OpenTelemetry format,
        in a file on this computer. Each trace records names, step numbers, tool names, token counts, durations and
        outcomes. It never records prompts, answers, page text, search snippets or tool arguments. Nothing is sent
        anywhere. The file is limited to about 10 MB.
      </p>
      <label className="checkbox small">
        <input type="checkbox" checked={enabled} onChange={(e) => void change(e.target.checked)} />
        Keep local performance traces
      </label>
      <div className="row">
        <button type="button" onClick={() => void remove()}>
          Delete traces
        </button>
      </div>
      {notice && (
        <p role="status" className="muted small">
          {notice}
        </p>
      )}
    </section>
  );
}
