import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { Editor } from '@tiptap/react';
import { api } from '../lib/api';
import { messageFor } from '../lib/pure';
import type { AiDoneEvent } from '../lib/types';

type Action = 'rewrite' | 'summarize' | 'expand' | 'translate';

interface Review {
  original: string;
  suggestion: string;
  missing: string[];
  from: number;
  to: number;
}

interface Props {
  editor: Editor | null;
  pageId: string;
  onApplied: () => void;
}

const ACTION_LABEL: Record<Action, string> = {
  rewrite: 'Rewrite',
  summarize: 'Summarize',
  expand: 'Expand',
  translate: 'Translate',
};

/**
 * Runs an assistant action on the selected text and shows the suggestion beside the
 * original. Accepting replaces the selection only if it still matches what was sent.
 */
export function SelectionAssistant({ editor, pageId, onApplied }: Props) {
  const [action, setAction] = useState<Action>('rewrite');
  const [language, setLanguage] = useState('French');
  const [phase, setPhase] = useState<'idle' | 'running' | 'review'>('idle');
  const [review, setReview] = useState<Review | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const runRef = useRef<string | null>(null);
  const captureRef = useRef<{ from: number; to: number; original: string } | null>(null);

  useEffect(() => {
    const offDone = listen<AiDoneEvent>('ai://done', (e) => {
      const done = e.payload;
      if (done.runId !== runRef.current) return;
      runRef.current = null;
      const capture = captureRef.current;
      if (done.status === 'completed' && capture && done.content) {
        setReview({ ...capture, suggestion: done.content, missing: done.missingNumbers });
        setPhase('review');
      } else {
        setPhase('idle');
        setMessage(
          done.status === 'cancelled' ? 'Cancelled. Nothing changed.' : (done.message ?? 'The action did not finish.'),
        );
      }
    });
    return () => {
      offDone.then((off) => off());
    };
  }, []);

  const start = async () => {
    if (!editor) return;
    const { from, to, empty } = editor.state.selection;
    if (empty) {
      setMessage('Select some text in the page first.');
      return;
    }
    const original = editor.state.doc.textBetween(from, to, '\n');
    const runId = crypto.randomUUID();
    runRef.current = runId;
    captureRef.current = { from, to, original };
    setMessage(null);
    setReview(null);
    setPhase('running');
    try {
      await api.aiPageAction({
        runId,
        pageId,
        action,
        language: action === 'translate' ? language : undefined,
        selectedText: original,
      });
    } catch (err) {
      runRef.current = null;
      setPhase('idle');
      setMessage(messageFor(err));
    }
  };

  const cancel = () => {
    if (runRef.current) void api.aiCancel(runRef.current);
  };

  const accept = () => {
    if (!editor || !review) return;
    const current = editor.state.doc.textBetween(review.from, review.to, '\n');
    if (current !== review.original) {
      setMessage('The selected text changed while the suggestion was being prepared. Copy the suggestion instead.');
      setReview(null);
      setPhase('idle');
      return;
    }
    const paragraphs = review.suggestion
      .split(/\n{2,}/)
      .map((p) => p.trim())
      .filter(Boolean);
    editor
      .chain()
      .focus()
      .setTextSelection({ from: review.from, to: review.to })
      .insertContent(paragraphs.map((text) => ({ type: 'paragraph', content: [{ type: 'text', text }] })))
      .run();
    setReview(null);
    setPhase('idle');
    onApplied();
  };

  const reject = () => {
    setReview(null);
    setPhase('idle');
  };

  return (
    <details className="selection-assistant">
      <summary>Assistant on selected text</summary>
      <div className="selection-body">
        <div className="row wrap">
          <label className="inline-label">
            <span>Action</span>
            <select value={action} disabled={phase === 'running'} onChange={(e) => setAction(e.target.value as Action)}>
              {(Object.keys(ACTION_LABEL) as Action[]).map((a) => (
                <option key={a} value={a}>
                  {ACTION_LABEL[a]}
                </option>
              ))}
            </select>
          </label>
          {action === 'translate' && (
            <label className="inline-label">
              <span>Into</span>
              <input value={language} maxLength={40} onChange={(e) => setLanguage(e.target.value)} />
            </label>
          )}
          {phase === 'running' ? (
            <button type="button" onClick={cancel}>
              Stop
            </button>
          ) : (
            <button type="button" className="primary" onClick={() => void start()} disabled={!editor}>
              Run on selection
            </button>
          )}
        </div>

        {phase === 'running' && (
          <p role="status" className="muted small">
            Working on the selection…
          </p>
        )}

        {review && (
          <div className="review" aria-label="Suggested replacement">
            {review.missing.length > 0 && (
              <p role="alert" className="inline-error">
                The suggestion may have dropped or changed these numbers: {review.missing.join(', ')}. Check before
                accepting.
              </p>
            )}
            <div className="compare">
              <div>
                <h4>Original</h4>
                <p className="compare-text">{review.original}</p>
              </div>
              <div>
                <h4>Suggested</h4>
                <p className="compare-text">{review.suggestion}</p>
              </div>
            </div>
            <div className="row">
              <button type="button" className="primary" onClick={accept}>
                Accept
              </button>
              <button type="button" onClick={reject}>
                Reject
              </button>
            </div>
          </div>
        )}

        {message && (
          <p role="status" className="muted small">
            {message}
          </p>
        )}
      </div>
    </details>
  );
}
