import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from '../lib/api';
import { messageFor, isConflict } from '../lib/pure';
import type {
  AiDeltaEvent,
  ConversationHit,
  AiDoneEvent,
  AiProposalEvent,
  AiReadiness,
  AiStatus,
  AiToolEvent,
  Citation,
  ConversationSummary,
  Proposal,
  StoredMessage,
  WorkspaceListItem,
} from '../lib/types';

interface Props {
  selectedText: string | null;
  pageId: string | null;
  pageTitle: string | null;
  /** Opens a page. For a linked file, the section the answer drew on is passed too. */
  onOpenPage: (id: string, section?: string) => void;
  onOpenTasks: () => void;
  onOpenSettings: () => void;
  onDataChanged: () => void;
  /** Text to place in the question box, from "Ask about this file" and similar buttons. */
  prefill?: { text: string; nonce: number } | null;
}

const READINESS_TEXT: Record<AiReadiness, string> = {
  ready: 'Connected',
  model_missing: 'Model not installed',
  unreachable: 'Model server not reachable',
  not_configured: 'Not set up',
};

interface Pending {
  runId: string;
  text: string;
  tools: AiToolEvent[];
}

/**
 * Chat with the workspace. Answers stream in as they are generated. Changes the assistant
 * proposes appear as cards that the user applies, rejects, or undoes. Nothing is written
 * without an explicit click.
 */
export function AssistantPanel({
  pageId,
  pageTitle,
  selectedText,
  onOpenPage,
  onOpenTasks,
  onOpenSettings,
  onDataChanged,
  prefill,
}: Props) {
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [messages, setMessages] = useState<StoredMessage[]>([]);
  const [pending, setPending] = useState<Pending | null>(null);
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [input, setInput] = useState('');

  // A question chosen elsewhere (for example "Ask about this file") goes into the box for the user to send.
  useEffect(() => {
    if (prefill) setInput(prefill.text);
  }, [prefill]);
  const [includePage, setIncludePage] = useState(true);
  const [includeSelection, setIncludeSelection] = useState(true);
  const [historyQuery, setHistoryQuery] = useState('');
  const [historyHits, setHistoryHits] = useState<ConversationHit[] | null>(null);
  // Which workspaces the next question covers. "current" is the open workspace only.
  const [scope, setScope] = useState('current');
  const [workspaces, setWorkspaces] = useState<WorkspaceListItem[]>([]);

  // Search past conversations as the user types. Stale responses are ignored.
  useEffect(() => {
    const query = historyQuery.trim();
    if (!query) {
      setHistoryHits(null);
      return;
    }
    let current = true;
    const timer = window.setTimeout(() => {
      api
        .aiSearchConversations(query)
        .then((hits) => current && setHistoryHits(hits))
        .catch(() => current && setHistoryHits([]));
    }, 250);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [historyQuery]);
  const [notice, setNotice] = useState<string | null>(null);
  const activeRunRef = useRef<string | null>(null);

  const refreshConversations = useCallback(() => {
    api
      .aiListConversations()
      .then(setConversations)
      .catch(() => setConversations([]));
  }, []);

  const loadConversation = useCallback((id: string | null) => {
    setNotice(null);
    setPending(null);
    setProposals([]);
    setConversationId(id);
    if (!id) {
      setMessages([]);
      return;
    }
    api
      .aiGetConversation(id)
      .then(setMessages)
      .catch((err) => setNotice(messageFor(err)));
  }, []);

  useEffect(() => {
    api
      .listWorkspaces()
      .then(setWorkspaces)
      .catch(() => setWorkspaces([]));
  }, []);

  useEffect(() => {
    api
      .aiGetStatus()
      .then(setStatus)
      .catch((err) => setNotice(messageFor(err)));
    refreshConversations();
  }, [refreshConversations]);

  // Event subscriptions. Events for runs this panel did not start (page actions) are ignored.
  useEffect(() => {
    const unlisteners: Promise<() => void>[] = [
      listen<AiDeltaEvent>('ai://delta', (e) => {
        if (e.payload.runId !== activeRunRef.current) return;
        setPending((p) => (p && p.runId === e.payload.runId ? { ...p, text: p.text + e.payload.text } : p));
      }),
      listen<AiToolEvent>('ai://tool', (e) => {
        if (e.payload.runId !== activeRunRef.current) return;
        setPending((p) => (p && p.runId === e.payload.runId ? { ...p, tools: [...p.tools, e.payload] } : p));
      }),
      listen<AiProposalEvent>('ai://proposal', (e) => {
        if (e.payload.runId !== activeRunRef.current) return;
        setProposals((list) => [e.payload.proposal, ...list.filter((p) => p.id !== e.payload.proposal.id)]);
      }),
      listen<AiDoneEvent>('ai://done', (e) => {
        const done = e.payload;
        if (done.runId !== activeRunRef.current) return;
        activeRunRef.current = null;
        setPending(null);
        if (done.status === 'failed') setNotice(done.message ?? 'The assistant could not finish.');
        if (done.status === 'cancelled') setNotice(done.message ?? 'Cancelled. Nothing was applied.');
        if (done.conversationId) {
          setConversationId(done.conversationId);
          api
            .aiGetConversation(done.conversationId)
            .then(setMessages)
            .catch(() => undefined);
        }
        api
          .aiListProposals(done.runId)
          .then(setProposals)
          .catch(() => undefined);
        refreshConversations();
      }),
    ];
    return () => {
      unlisteners.forEach((u) => u.then((off) => off()));
    };
  }, [onDataChanged, refreshConversations]);

  const running = pending !== null;
  const readiness: AiReadiness = status?.state ?? 'not_configured';
  const canSend = readiness === 'ready' && !running && input.trim().length > 0;

  const send = async () => {
    const message = input.trim();
    if (!canSend || !message) return;
    setNotice(null);
    setInput('');
    setProposals([]);
    // The run id is chosen here, before the call, so streamed events are never dropped.
    const runId = crypto.randomUUID();
    activeRunRef.current = runId;
    setPending({ runId, text: '', tools: [] });
    setMessages((list) => [
      ...list,
      { id: `local-${runId}`, role: 'user', content: message, citations: [], createdAt: new Date().toISOString() },
    ]);
    try {
      const started = await api.aiChatSend({
        runId,
        conversationId,
        message,
        pageId: includePage ? pageId : null,
        selectedText: includeSelection && selectedText ? selectedText : null,
        scope,
      });
      setConversationId(started.conversationId);
    } catch (err) {
      activeRunRef.current = null;
      setPending(null);
      setNotice(messageFor(err));
    }
  };

  const cancel = async () => {
    const runId = activeRunRef.current;
    if (runId) await api.aiCancel(runId).catch(() => undefined);
  };

  const onKey = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      void send();
    }
  };

  const decide = async (proposal: Proposal, action: 'apply' | 'reject' | 'undo') => {
    setNotice(null);
    try {
      const updated =
        action === 'apply'
          ? await api.aiApplyProposal(proposal.id)
          : action === 'reject'
            ? await api.aiRejectProposal(proposal.id)
            : await api.aiUndoProposal(proposal.id);
      setProposals((list) => list.map((p) => (p.id === updated.id ? updated : p)));
      onDataChanged();
    } catch (err) {
      setNotice(messageFor(err));
      if (isConflict(err) && proposal.runId) {
        api
          .aiListProposals(proposal.runId)
          .then(setProposals)
          .catch(() => undefined);
      }
      onDataChanged();
    }
  };

  return (
    <aside className="assistant" aria-label="Assistant">
      <header className="assistant-head">
        <h2>Assistant</h2>
        <span className={`chip chip-${readiness}`} role="status">
          {READINESS_TEXT[readiness]}
        </span>
        <button type="button" onClick={() => loadConversation(null)} disabled={running}>
          New chat
        </button>
      </header>

      <div className="assistant-search">
        <input
          type="search"
          aria-label="Search past conversations"
          placeholder="Search past conversations"
          value={historyQuery}
          onChange={(e) => setHistoryQuery(e.target.value)}
        />
        {historyHits && (
          <ul className="history-hits" aria-label="Matching conversations">
            {historyHits.length === 0 && <li className="muted small">No conversation mentions that.</li>}
            {historyHits.map((hit) => (
              <li key={hit.conversationId}>
                <button
                  type="button"
                  className="link-button"
                  onClick={() => {
                    setHistoryQuery('');
                    setHistoryHits(null);
                    loadConversation(hit.conversationId);
                  }}
                >
                  {hit.title}
                </button>
                <div className="muted small">{hit.snippet}</div>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="assistant-controls">
        <label className="inline-label grow">
          <span className="visually-hidden">Conversation</span>
          <select
            aria-label="Conversation"
            value={conversationId ?? ''}
            disabled={running}
            onChange={(e) => loadConversation(e.target.value || null)}
          >
            <option value="">Start a new conversation</option>
            {conversations.map((c) => (
              <option key={c.id} value={c.id}>
                {c.title}
              </option>
            ))}
          </select>
        </label>
      </div>

      {readiness !== 'ready' && (
        <div className="assistant-setup" role="note">
          <p>
            {readiness === 'not_configured' &&
              'Choose a local model to use the assistant. Nothing is sent anywhere until you do.'}
            {readiness === 'model_missing' &&
              `The model "${status?.config.model}" is not installed. Run "ollama pull ${status?.config.model}".`}
            {readiness === 'unreachable' &&
              'Start Ollama, or check the endpoint in settings. The rest of the app works without it.'}
          </p>
          <button type="button" onClick={onOpenSettings}>
            Open AI settings
          </button>
        </div>
      )}

      <div className="assistant-log" aria-live="polite">
        {messages.length === 0 && !pending && readiness === 'ready' && (
          <p className="muted small">
            Ask about your notes, for example “What did we decide about authentication?” Answers cite the pages they
            used.
          </p>
        )}
        {messages.map((m) => (
          <MessageView key={m.id} message={m} onOpenPage={onOpenPage} onOpenTasks={onOpenTasks} />
        ))}
        {pending && (
          <div className="bubble assistant-bubble is-pending">
            <div className="bubble-label">Assistant</div>
            {pending.tools.map((t) => (
              <p key={`${t.step}-${t.tool}-${t.summary}`} className="tool-line muted small">
                {t.ok ? '✓' : '✕'} {toolLabel(t.tool)}: {t.summary}
              </p>
            ))}
            <div className="bubble-text">{pending.text || 'Thinking…'}</div>
            <button type="button" onClick={() => void cancel()}>
              Stop
            </button>
          </div>
        )}
        {proposals.map((p) => (
          <ProposalCardView key={p.id} proposal={p} onDecide={decide} />
        ))}
        {notice && (
          <p role="alert" className="inline-error">
            {notice}
          </p>
        )}
      </div>

      <form
        className="composer"
        onSubmit={(e) => {
          e.preventDefault();
          void send();
        }}
      >
        {workspaces.length > 1 && (
          <label className="inline-label small">
            <span>Answer from</span>
            <select value={scope} onChange={(e) => setScope(e.target.value)}>
              <option value="current">This workspace only</option>
              <option value="all">All workspaces (read-only for the others)</option>
              {workspaces
                .filter((w) => w.available && !w.active)
                .map((w) => (
                  <option key={w.path} value={w.name}>
                    {w.name} (read-only)
                  </option>
                ))}
            </select>
          </label>
        )}
        {selectedText && (
          <label className="checkbox small">
            <input type="checkbox" checked={includeSelection} onChange={(e) => setIncludeSelection(e.target.checked)} />
            Include the selected text as context
          </label>
        )}
        {pageId && (
          <label className="checkbox small">
            <input type="checkbox" checked={includePage} onChange={(e) => setIncludePage(e.target.checked)} />
            Include “{pageTitle}” as context
          </label>
        )}
        <textarea
          aria-label="Message the assistant"
          placeholder={readiness === 'ready' ? 'Ask about your workspace…' : 'Set up a model to ask questions'}
          value={input}
          rows={3}
          maxLength={8000}
          disabled={readiness !== 'ready' || running}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={onKey}
        />
        <div className="row">
          <button type="submit" className="primary" disabled={!canSend}>
            Send
          </button>
          <span className="muted small">Enter sends · Shift+Enter for a new line</span>
        </div>
      </form>
    </aside>
  );
}

function toolLabel(tool: string): string {
  switch (tool) {
    case 'search_workspace':
      return 'Searched';
    case 'read_page':
      return 'Read page';
    case 'list_tasks':
      return 'Listed tasks';
    case 'propose_create_page':
      return 'Proposed a page';
    case 'propose_edit_page':
      return 'Proposed an edit';
    case 'propose_task_changes':
      return 'Proposed task changes';
    default:
      return tool;
  }
}

function MessageView({
  message,
  onOpenPage,
  onOpenTasks,
}: {
  message: StoredMessage;
  onOpenPage: (id: string, section?: string) => void;
  onOpenTasks: () => void;
}) {
  if (message.role === 'user') {
    return (
      <div className="bubble user-bubble">
        <div className="bubble-text">{message.content}</div>
      </div>
    );
  }
  return (
    <div className="bubble assistant-bubble">
      <div className="bubble-label">Assistant</div>
      <div className="bubble-text">{message.content}</div>
      {message.citations.length > 0 && (
        <ul className="citations" aria-label="Sources">
          {message.citations.map((c: Citation) => (
            <li key={`${c.kind}-${c.id}`}>
              <button
                type="button"
                className="link-button"
                onClick={() => (c.kind === 'page' ? onOpenPage(c.id, c.section ?? undefined) : onOpenTasks())}
              >
                [{c.n}] {c.title}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function ProposalCardView({
  proposal,
  onDecide,
}: {
  proposal: Proposal;
  onDecide: (p: Proposal, action: 'apply' | 'reject' | 'undo') => void;
}) {
  const statusText: Record<Proposal['status'], string> = {
    pending: 'Waiting for your review',
    applied: 'Applied',
    rejected: 'Rejected',
    stale: 'Out of date. The content changed. Ask for a new suggestion.',
    undone: 'Undone',
  };
  return (
    <section className={`proposal proposal-${proposal.status}`} aria-label={`Proposed change: ${proposal.summary}`}>
      <h3>{proposal.summary}</h3>
      <p className="muted small">{statusText[proposal.status]}</p>
      <DiffView text={proposal.diffText} />
      <div className="row">
        {proposal.status === 'pending' && (
          <>
            <button type="button" className="primary" onClick={() => onDecide(proposal, 'apply')}>
              Apply
            </button>
            <button type="button" onClick={() => onDecide(proposal, 'reject')}>
              Reject
            </button>
          </>
        )}
        {proposal.status === 'applied' && (
          <button type="button" onClick={() => onDecide(proposal, 'undo')}>
            Undo
          </button>
        )}
      </div>
    </section>
  );
}

/** A proposal card that applies, rejects or undoes on its own. Used outside the assistant panel. */
export function ProposalCard({
  proposal,
  onChanged,
  onError,
}: {
  proposal: Proposal;
  onChanged: () => void;
  onError: (error: unknown) => void;
}) {
  const [current, setCurrent] = useState(proposal);
  useEffect(() => setCurrent(proposal), [proposal]);
  const decide = async (_p: Proposal, action: 'apply' | 'reject' | 'undo') => {
    try {
      const updated =
        action === 'apply'
          ? await api.aiApplyProposal(current.id)
          : action === 'reject'
            ? await api.aiRejectProposal(current.id)
            : await api.aiUndoProposal(current.id);
      setCurrent(updated);
      onChanged();
    } catch (err) {
      onError(err);
    }
  };
  return <ProposalCardView proposal={current} onDecide={decide} />;
}

export function DiffView({ text }: { text: string }) {
  const lines = text.split('\n').filter((line, i, all) => !(i === all.length - 1 && line === ''));
  return (
    <pre className="diff" aria-label="Preview of the change">
      {lines.map((line, i) => {
        const kind = line.startsWith('+') ? 'add' : line.startsWith('-') ? 'remove' : 'same';
        const mark = kind === 'add' ? '+ ' : kind === 'remove' ? '- ' : '  ';
        return (
          <span key={i} className={`diff-line diff-${kind}`}>
            {mark}
            {line.slice(1)}
            {'\n'}
          </span>
        );
      })}
    </pre>
  );
}
