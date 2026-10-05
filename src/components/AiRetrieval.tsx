import { useEffect, useState, type FormEvent } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';
import type { AiIndexEvent, IndexStatus } from '../lib/types';

/**
 * How the assistant finds material: keyword search, or keyword plus semantic search with a local
 * embedding model. Shows how much of the workspace is indexed.
 */
export function AiRetrieval({ onError }: { onError: (e: unknown) => void }) {
  const [embedModel, setEmbedModel] = useState('nomic-embed-text');
  const [mode, setMode] = useState<'lexical' | 'hybrid'>('hybrid');
  const [weightLexical, setWeightLexical] = useState(0.4);
  const [weightVector, setWeightVector] = useState(0.6);
  const [index, setIndex] = useState<IndexStatus | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = () => {
    api
      .aiIndexStatus()
      .then(setIndex)
      .catch(() => setIndex(null));
  };

  useEffect(() => {
    api
      .getSettings()
      .then(() => api.aiGetStatus())
      .then((s) => {
        setEmbedModel(s.config.embedModel);
        setMode(s.config.retrievalMode);
        setWeightLexical(s.config.weightLexical);
        setWeightVector(s.config.weightVector);
      })
      .catch(onError);
    refresh();
    const off = listen<AiIndexEvent>('ai://index', (e) => {
      if (e.payload.status === 'error')
        setNotice(`Indexing stopped (${e.payload.category ?? 'error'}). Keyword search still works.`);
      refresh();
    });
    return () => {
      off.then((u) => u());
    };
    // Loaded once; index events refresh the counts.
  }, []);

  const save = async (event: FormEvent) => {
    event.preventDefault();
    setNotice(null);
    try {
      await api.aiSaveRetrieval(embedModel, mode, weightLexical, weightVector);
      setNotice('Saved.');
      void api.aiIndexStart().catch(() => undefined);
      refresh();
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const rebuild = async () => {
    setNotice(null);
    try {
      const started = await api.aiIndexStart();
      setNotice(started ? 'Indexing started.' : 'Indexing is already running.');
    } catch (err) {
      setNotice(messageFor(err));
    }
  };

  const pct = index && index.total > 0 ? Math.round((index.embedded / index.total) * 100) : 0;

  return (
    <section className="panel" aria-labelledby="retrieval-heading">
      <h2 id="retrieval-heading">Finding information</h2>
      <p className="muted small">
        Keyword search always works. Semantic search also finds passages that use different words, and needs a local
        embedding model such as <code>nomic-embed-text</code>. Pages marked “Exclude from AI” are never searched by the
        assistant.
      </p>

      {index && (
        <p role="status" className="small">
          Indexed: {index.embedded} of {index.total} passages ({pct}%) with {index.model}
          {index.running ? ' · indexing now' : ''}
        </p>
      )}

      <form className="stack" onSubmit={save}>
        <label className="field">
          <span>Embedding model</span>
          <input value={embedModel} onChange={(e) => setEmbedModel(e.target.value)} />
        </label>
        <label className="inline-label">
          <span>Mode</span>
          <select value={mode} onChange={(e) => setMode(e.target.value as 'lexical' | 'hybrid')}>
            <option value="hybrid">Keyword and semantic (hybrid)</option>
            <option value="lexical">Keyword only</option>
          </select>
        </label>
        <div className="row wrap">
          <label className="inline-label">
            <span>Keyword weight</span>
            <input
              type="number"
              min={0}
              max={1}
              step={0.1}
              value={weightLexical}
              onChange={(e) => setWeightLexical(Number(e.target.value))}
            />
          </label>
          <label className="inline-label">
            <span>Semantic weight</span>
            <input
              type="number"
              min={0}
              max={1}
              step={0.1}
              value={weightVector}
              onChange={(e) => setWeightVector(Number(e.target.value))}
            />
          </label>
        </div>
        <div className="row">
          <button type="submit" className="primary">
            Save
          </button>
          <button type="button" onClick={() => void rebuild()}>
            Build or update index
          </button>
        </div>
        {notice && (
          <p role="status" className="muted small">
            {notice}
          </p>
        )}
      </form>
    </section>
  );
}
