import { useEffect, useState, type FormEvent } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';

/**
 * Audio transcription uses a local engine you install, such as whisper.cpp's whisper-cli, plus a
 * model file. Threadwell does not download or bundle either one.
 */
export function AudioTranscription({ onError }: { onError: (e: unknown) => void }) {
  const [engine, setEngine] = useState('');
  const [model, setModel] = useState('');
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    api
      .audioSettingsGet()
      .then(([e, m]) => {
        setEngine(e ?? '');
        setModel(m ?? '');
      })
      .catch(onError);
  }, [onError]);

  const pick = async (
    set: (value: string) => void,
    title: string,
    filters: { name: string; extensions: string[] }[],
  ) => {
    const file = await open({ multiple: false, title, filters });
    if (typeof file === 'string') set(file);
  };

  const save = async (event: FormEvent) => {
    event.preventDefault();
    setNotice(null);
    try {
      await api.audioSettingsSave(engine, model);
      setNotice('Saved.');
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  return (
    <section className="panel" aria-labelledby="audio-heading">
      <h2 id="audio-heading">Audio transcription (local engine)</h2>
      <p className="muted small">
        To turn recordings into meeting transcripts, install a local engine such as <strong>whisper.cpp</strong> (its
        <code>whisper-cli</code> program) and download a model file for it yourself. Choose both below. Threadwell runs
        the engine on your computer with a fixed set of options. It does not download anything or send audio anywhere.
      </p>
      <form className="stack" onSubmit={save}>
        <div className="row wrap">
          <label className="field grow">
            <span>Engine program</span>
            <input
              value={engine}
              placeholder="C:\Tools\whisper\whisper-cli.exe"
              onChange={(e) => setEngine(e.target.value)}
            />
          </label>
          <button
            type="button"
            onClick={() =>
              void pick(setEngine, 'Choose the engine program', [{ name: 'Program', extensions: ['exe'] }])
            }
          >
            Choose…
          </button>
        </div>
        <div className="row wrap">
          <label className="field grow">
            <span>Model file</span>
            <input
              value={model}
              placeholder="C:\Tools\whisper\models\ggml-base.bin"
              onChange={(e) => setModel(e.target.value)}
            />
          </label>
          <button
            type="button"
            onClick={() => void pick(setModel, 'Choose the model file', [{ name: 'Model', extensions: ['bin'] }])}
          >
            Choose…
          </button>
        </div>
        <div className="row">
          <button type="submit" className="primary">
            Save
          </button>
          {notice && (
            <span role="status" className="muted small">
              {notice}
            </span>
          )}
        </div>
      </form>
      <p className="muted small">
        A transcription can take a long time on a laptop. It stops automatically after 30 minutes.
      </p>
    </section>
  );
}
