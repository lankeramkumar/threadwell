import { useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import { messageFor, parseSnippet } from '../lib/pure';
import type { SearchHit } from '../lib/types';

interface Props {
  initialQuery: string;
  onOpenPage: (id: string) => void;
  onOpenTasks: () => void;
}

const DEBOUNCE_MS = 200;

/** Lexical search across pages and tasks in the open workspace. Stale responses are ignored. */
export function SearchView({ initialQuery, onOpenPage, onOpenTasks }: Props) {
  const [query, setQuery] = useState(initialQuery);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [state, setState] = useState<'idle' | 'searching' | 'done' | 'error'>('idle');
  const [message, setMessage] = useState('');
  const requestRef = useRef(0);

  useEffect(() => {
    const trimmed = query.trim();
    const request = ++requestRef.current;
    if (!trimmed) {
      setHits([]);
      setState('idle');
      return;
    }
    setState('searching');
    const timer = window.setTimeout(() => {
      api
        .search(trimmed)
        .then((results) => {
          if (request !== requestRef.current) return;
          setHits(results);
          setState('done');
        })
        .catch((err) => {
          if (request !== requestRef.current) return;
          setMessage(messageFor(err));
          setState('error');
        });
    }, DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [query]);

  return (
    <section className="search-view" aria-labelledby="search-title">
      <h1 id="search-title">Search</h1>
      <label className="field">
        <span className="visually-hidden">Search text</span>
        <input
          type="search"
          autoFocus
          placeholder="Search pages and tasks"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>

      <p role="status" aria-live="polite" className="muted small">
        {state === 'idle' && 'Type to search titles, page text and task details.'}
        {state === 'searching' && 'Searching…'}
        {state === 'done' &&
          (hits.length === 0
            ? 'No matches in this workspace.'
            : `${hits.length} result${hits.length === 1 ? '' : 's'}`)}
        {state === 'error' && message}
      </p>

      <ul className="results">
        {hits.map((hit) => (
          <li key={`${hit.kind}-${hit.id}`} className="result">
            <button
              type="button"
              className="link-button result-title"
              onClick={() => (hit.kind === 'page' ? onOpenPage(hit.id) : onOpenTasks())}
            >
              {hit.kind === 'page' ? 'Page' : 'Task'}: {hit.title}
            </button>
            {hit.snippet && (
              <p className="snippet">
                {parseSnippet(hit.snippet).map((part, i) =>
                  part.highlight ? <mark key={i}>{part.text}</mark> : <span key={i}>{part.text}</span>,
                )}
              </p>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}
