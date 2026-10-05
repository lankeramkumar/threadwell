import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from 'react';
import { EditorContent, useEditor, useEditorState } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import { Table } from '@tiptap/extension-table';
import TableRow from '@tiptap/extension-table-row';
import TableHeader from '@tiptap/extension-table-header';
import TableCell from '@tiptap/extension-table-cell';
import { api } from '../lib/api';
import { breadcrumbs, formatTimestamp, isConflict, LINK_PREFIX, messageFor, subtreeIds } from '../lib/pure';
import type { JsonNode, Page, PageSummary } from '../lib/types';
import { SlashCommands } from './slashCommands';
import { SelectionAssistant } from './SelectionAssistant';
import { Attachments } from './Attachments';

const AUTOSAVE_DELAY_MS = 800;
const RETRY_DELAY_MS = 5000;

type SaveState =
  | { kind: 'saved'; at: string }
  | { kind: 'unsaved' }
  | { kind: 'saving' }
  | { kind: 'error'; message: string }
  | { kind: 'conflict'; message: string };

interface Snapshot {
  title: string;
  body: JsonNode;
}

interface Props {
  onSelectionChange: (text: string | null) => void;
  page: Page;
  pages: PageSummary[];
  onNavigate: (id: string) => void;
  onChanged: () => void;
  onTrashed: () => void;
}

/**
 * Edits one page. The component is keyed by page id by its parent, so each page gets
 * fresh editor state. Edits are saved after a short pause and retried on failure.
 * A saved revision that no longer matches the database is reported as a conflict.
 */
export function PageEditor({ page, pages, onNavigate, onChanged, onTrashed, onSelectionChange }: Props) {
  const [title, setTitle] = useState(page.title);
  const [favorite, setFavorite] = useState(page.isFavorite);
  const [aiExcluded, setAiExcluded] = useState(page.aiExcluded);
  const [save, setSave] = useState<SaveState>({ kind: 'saved', at: page.updatedAt });
  const [backlinks, setBacklinks] = useState<PageSummary[]>([]);
  const [linkTarget, setLinkTarget] = useState('');
  const [actionError, setActionError] = useState<string | null>(null);

  const revisionRef = useRef(page.revision);
  const titleRef = useRef(page.title);
  const pendingRef = useRef<Snapshot | null>(null);
  const inFlightRef = useRef(false);
  const timerRef = useRef<number | undefined>(undefined);
  const mountedRef = useRef(true);
  const persistRef = useRef<() => void>(() => {});

  const schedule = useCallback((delay: number) => {
    window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => persistRef.current(), delay);
  }, []);

  const persist = useCallback(async () => {
    if (inFlightRef.current || !pendingRef.current) return;
    const snapshot = pendingRef.current;
    pendingRef.current = null;
    inFlightRef.current = true;
    if (mountedRef.current) setSave({ kind: 'saving' });
    try {
      const saved = await api.savePage(page.id, snapshot.title, snapshot.body, revisionRef.current);
      revisionRef.current = saved.revision;
      inFlightRef.current = false;
      if (!mountedRef.current) return;
      onChanged();
      if (pendingRef.current) {
        schedule(AUTOSAVE_DELAY_MS);
      } else {
        setSave({ kind: 'saved', at: saved.updatedAt });
      }
    } catch (error) {
      inFlightRef.current = false;
      // Keep the newest content if the user kept typing while the save was in flight.
      pendingRef.current = pendingRef.current ?? snapshot;
      if (!mountedRef.current) return;
      if (isConflict(error)) {
        pendingRef.current = null;
        setSave({ kind: 'conflict', message: messageFor(error) });
        return;
      }
      setSave({ kind: 'error', message: messageFor(error) });
      schedule(RETRY_DELAY_MS);
    }
  }, [page.id, onChanged, schedule]);

  useEffect(() => {
    persistRef.current = () => void persist();
  }, [persist]);

  const markDirty = useCallback(
    (snapshot: Snapshot) => {
      pendingRef.current = snapshot;
      if (mountedRef.current) setSave({ kind: 'unsaved' });
      schedule(AUTOSAVE_DELAY_MS);
    },
    [schedule],
  );

  // Pages that mirror a linked source file are read only. Their text changes when the file does.
  const readOnly = page.sourceId !== null;

  const editor = useEditor({
    immediatelyRender: false,
    editable: !readOnly,
    extensions: [
      StarterKit.configure({ link: false }),
      Link.configure({
        openOnClick: false,
        autolink: false,
        isAllowedUri: (url) => /^(https?:|mailto:|threadwell:)/i.test(url),
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Table.configure({ resizable: false }),
      TableRow,
      TableHeader,
      TableCell,
      Placeholder.configure({ placeholder: 'Start writing, or type / for block commands' }),
      SlashCommands,
    ],
    content: page.body,
    editorProps: {
      attributes: {
        class: 'editor-surface',
        role: 'textbox',
        'aria-multiline': 'true',
        'aria-label': `Content of ${page.title}`,
      },
    },
    onUpdate: ({ editor: instance }) => {
      markDirty({ title: titleRef.current, body: instance.getJSON() as JsonNode });
    },
  });

  // Tell the assistant panel what is selected, so it can offer it as context.
  useEffect(() => {
    if (!editor) return;
    const report = () => {
      const { from, to, empty } = editor.state.selection;
      onSelectionChange(empty ? null : editor.state.doc.textBetween(from, to, '\n'));
    };
    editor.on('selectionUpdate', report);
    return () => {
      editor.off('selectionUpdate', report);
      onSelectionChange(null);
    };
  }, [editor, onSelectionChange]);

  const active = useEditorState({
    editor,
    selector: ({ editor: e }) => ({
      bold: e?.isActive('bold') ?? false,
      italic: e?.isActive('italic') ?? false,
      code: e?.isActive('code') ?? false,
      heading1: e?.isActive('heading', { level: 1 }) ?? false,
      heading2: e?.isActive('heading', { level: 2 }) ?? false,
      bullet: e?.isActive('bulletList') ?? false,
      ordered: e?.isActive('orderedList') ?? false,
      checklist: e?.isActive('taskList') ?? false,
      quote: e?.isActive('blockquote') ?? false,
      codeBlock: e?.isActive('codeBlock') ?? false,
    }),
  });

  // Flush unsaved edits when the page is left or the window is closed.
  useEffect(() => {
    mountedRef.current = true;
    const flush = () => {
      window.clearTimeout(timerRef.current);
      if (pendingRef.current && !inFlightRef.current) {
        const snapshot = pendingRef.current;
        pendingRef.current = null;
        void api.savePage(page.id, snapshot.title, snapshot.body, revisionRef.current).catch(() => {});
      }
    };
    window.addEventListener('beforeunload', flush);
    return () => {
      mountedRef.current = false;
      window.removeEventListener('beforeunload', flush);
      flush();
    };
  }, [page.id]);

  useEffect(() => {
    let cancelled = false;
    api
      .backlinks(page.id)
      .then((list) => !cancelled && setBacklinks(list))
      .catch(() => !cancelled && setBacklinks([]));
    return () => {
      cancelled = true;
    };
  }, [page.id, pages]);

  const trail = useMemo(() => breadcrumbs(pages, page.id), [pages, page.id]);
  const excluded = useMemo(() => subtreeIds(pages, page.id), [pages, page.id]);
  const parentOptions = pages.filter((p) => !excluded.has(p.id));
  const linkOptions = pages.filter((p) => p.id !== page.id);

  const onTitleChange = (value: string) => {
    setTitle(value);
    titleRef.current = value;
    if (editor) markDirty({ title: value, body: editor.getJSON() as JsonNode });
  };

  const run = async (action: () => Promise<unknown>) => {
    setActionError(null);
    try {
      await action();
      onChanged();
    } catch (error) {
      setActionError(messageFor(error));
    }
  };

  const onEditorClick = (event: MouseEvent<HTMLDivElement>) => {
    const anchor = (event.target as HTMLElement).closest('a');
    if (!anchor) return;
    // Links never navigate the webview. Internal links open the page in the app.
    event.preventDefault();
    const href = anchor.getAttribute('href') ?? '';
    if (href.startsWith(LINK_PREFIX)) onNavigate(href.slice(LINK_PREFIX.length));
  };

  const insertPageLink = (targetId: string) => {
    const target = pages.find((p) => p.id === targetId);
    if (!editor || !target) return;
    const href = `${LINK_PREFIX}${target.id}`;
    if (editor.state.selection.empty) {
      editor
        .chain()
        .focus()
        .insertContent({ type: 'text', text: target.title, marks: [{ type: 'link', attrs: { href } }] })
        .run();
    } else {
      editor.chain().focus().setLink({ href }).run();
    }
  };

  const statusText = (() => {
    switch (save.kind) {
      case 'saved':
        return `Saved${save.at ? ` · ${formatTimestamp(save.at)}` : ''}`;
      case 'unsaved':
        return 'Unsaved changes';
      case 'saving':
        return 'Saving…';
      case 'error':
        return `Couldn't save. Retrying. ${save.message}`;
      case 'conflict':
        return save.message;
    }
  })();

  return (
    <article className="page-editor">
      <nav aria-label="Breadcrumb" className="breadcrumbs">
        {trail.map((crumb, index) => (
          <span key={crumb.id}>
            {index > 0 && <span aria-hidden="true"> / </span>}
            {index < trail.length - 1 ? (
              <button type="button" className="link-button" onClick={() => onNavigate(crumb.id)}>
                {crumb.title}
              </button>
            ) : (
              <span aria-current="page">{crumb.title}</span>
            )}
          </span>
        ))}
      </nav>

      {readOnly && (
        <p role="note" className="notice small">
          Read only. This file comes from a linked folder: <code>{page.sourcePath}</code>. Change it in that folder and
          Threadwell will pick up the change.
        </p>
      )}

      <div className="page-header">
        <input
          className="page-title"
          aria-label="Page title"
          readOnly={readOnly}
          value={title}
          maxLength={200}
          onChange={(e) => onTitleChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              editor?.commands.focus('start');
            }
          }}
        />
        <div className="page-actions">
          <label className="inline-label">
            <span>Parent</span>
            <select
              value={page.parentId ?? ''}
              onChange={(e) => run(() => api.movePage(page.id, e.target.value || null))}
            >
              <option value="">None (top level)</option>
              {parentOptions.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.title}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            aria-pressed={favorite}
            onClick={() =>
              run(async () => {
                await api.setPageFavorite(page.id, !favorite);
                setFavorite(!favorite);
              })
            }
          >
            {favorite ? 'Unfavorite' : 'Favorite'}
          </button>
          <label className="checkbox small">
            <input
              type="checkbox"
              checked={aiExcluded}
              onChange={(e) =>
                run(async () => {
                  await api.aiSetPageExcluded(page.id, e.target.checked);
                  setAiExcluded(e.target.checked);
                })
              }
            />
            Exclude from AI
          </label>
          <button
            type="button"
            className="danger-quiet"
            onClick={() =>
              run(async () => {
                await api.trashPage(page.id);
                onTrashed();
              })
            }
          >
            Move to trash
          </button>
        </div>
      </div>

      <div className="toolbar" role="toolbar" aria-label="Formatting">
        <ToolbarButton label="Bold" pressed={active?.bold} onClick={() => editor?.chain().focus().toggleBold().run()} />
        <ToolbarButton
          label="Italic"
          pressed={active?.italic}
          onClick={() => editor?.chain().focus().toggleItalic().run()}
        />
        <ToolbarButton
          label="Inline code"
          pressed={active?.code}
          onClick={() => editor?.chain().focus().toggleCode().run()}
        />
        <ToolbarButton
          label="Heading 1"
          pressed={active?.heading1}
          onClick={() => editor?.chain().focus().toggleHeading({ level: 1 }).run()}
        />
        <ToolbarButton
          label="Heading 2"
          pressed={active?.heading2}
          onClick={() => editor?.chain().focus().toggleHeading({ level: 2 }).run()}
        />
        <ToolbarButton
          label="Bulleted list"
          pressed={active?.bullet}
          onClick={() => editor?.chain().focus().toggleBulletList().run()}
        />
        <ToolbarButton
          label="Numbered list"
          pressed={active?.ordered}
          onClick={() => editor?.chain().focus().toggleOrderedList().run()}
        />
        <ToolbarButton
          label="Checklist"
          pressed={active?.checklist}
          onClick={() => editor?.chain().focus().toggleTaskList().run()}
        />
        <ToolbarButton
          label="Quote"
          pressed={active?.quote}
          onClick={() => editor?.chain().focus().toggleBlockquote().run()}
        />
        <ToolbarButton
          label="Code block"
          pressed={active?.codeBlock}
          onClick={() => editor?.chain().focus().toggleCodeBlock().run()}
        />
        <ToolbarButton label="Undo" onClick={() => editor?.chain().focus().undo().run()} />
        <ToolbarButton label="Redo" onClick={() => editor?.chain().focus().redo().run()} />
        <label className="inline-label">
          <span className="visually-hidden">Link to a page</span>
          <select
            aria-label="Link to a page"
            value={linkTarget}
            onChange={(e) => {
              insertPageLink(e.target.value);
              setLinkTarget('');
            }}
          >
            <option value="">Link to page…</option>
            {linkOptions.map((p) => (
              <option key={p.id} value={p.id}>
                {p.title}
              </option>
            ))}
          </select>
        </label>
      </div>

      <SelectionAssistant editor={editor} pageId={page.id} onApplied={() => onChanged()} />

      <div className="status-line" role="status" aria-live="polite" data-state={save.kind}>
        {statusText}
        {save.kind === 'conflict' && (
          <button type="button" className="link-button" onClick={() => onNavigate(page.id)}>
            Reload page
          </button>
        )}
      </div>
      {actionError && (
        <p role="alert" className="inline-error">
          {actionError}
        </p>
      )}

      {/* Clicks are delegated so internal page links navigate inside the app. */}
      <div className="editor-wrap" onClick={onEditorClick}>
        <EditorContent editor={editor} />
      </div>

      <Attachments pageId={page.id} onError={(e) => setActionError(messageFor(e))} />

      <section className="backlinks" aria-label="Pages that link here">
        <h2>Linked from</h2>
        {backlinks.length === 0 ? (
          <p className="muted">No other page links here yet.</p>
        ) : (
          <ul>
            {backlinks.map((b) => (
              <li key={b.id}>
                <button type="button" className="link-button" onClick={() => onNavigate(b.id)}>
                  {b.title}
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </article>
  );
}

function ToolbarButton({ label, pressed, onClick }: { label: string; pressed?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      className="toolbar-button"
      aria-label={label}
      title={label}
      aria-pressed={pressed === undefined ? undefined : pressed}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onClick}
    >
      {shortLabel(label)}
    </button>
  );
}

function shortLabel(label: string): string {
  const map: Record<string, string> = {
    Bold: 'B',
    'Heading 1': 'H1',
    'Heading 2': 'H2',
    Italic: 'I',
    'Inline code': '<>',
    'Bulleted list': '•',
    'Numbered list': '1.',
    Checklist: '☐',
    Quote: '❝',
    'Code block': '{ }',
    Undo: '↶',
    Redo: '↷',
  };
  return map[label] ?? label;
}
