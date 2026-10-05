import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import { api } from './lib/api';
import { messageFor } from './lib/pure';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { Page, PageSummary, Project, WorkspaceInfo } from './lib/types';
import { applyTheme } from './lib/theme';
import { Onboarding } from './components/Onboarding';
import { Sidebar } from './components/Sidebar';
const PageEditor = lazy(() => import('./components/PageEditor').then((m) => ({ default: m.PageEditor })));
const TasksView = lazy(() => import('./components/TasksView').then((m) => ({ default: m.TasksView })));
const SearchView = lazy(() => import('./components/SearchView').then((m) => ({ default: m.SearchView })));
const SettingsView = lazy(() => import('./components/SettingsView').then((m) => ({ default: m.SettingsView })));
const TrashView = lazy(() => import('./components/TrashView').then((m) => ({ default: m.TrashView })));
import { CommandPalette, type PaletteAction } from './components/CommandPalette';
import { AssistantPanel } from './components/AssistantPanel';
const MeetingsView = lazy(() => import('./components/MeetingsView').then((m) => ({ default: m.MeetingsView })));
const SourcesView = lazy(() => import('./components/SourcesView').then((m) => ({ default: m.SourcesView })));
const RecipesView = lazy(() => import('./components/RecipesView').then((m) => ({ default: m.RecipesView })));

export type View =
  | { kind: 'home' }
  | { kind: 'page'; id: string; section?: string }
  | { kind: 'tasks'; projectId: string | null }
  | { kind: 'search'; query: string }
  | { kind: 'settings' }
  | { kind: 'trash' }
  | { kind: 'meetings' }
  | { kind: 'sources' }
  | { kind: 'recipes' };

type Boot = 'loading' | 'onboarding' | 'ready';

export function App() {
  const [boot, setBoot] = useState<Boot>('loading');
  const [workspace, setWorkspace] = useState<WorkspaceInfo | null>(null);
  const [pages, setPages] = useState<PageSummary[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [view, setView] = useState<View>({ kind: 'home' });
  const [page, setPage] = useState<Page | null>(null);
  const [loadNonce, setLoadNonce] = useState(0);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [assistantOpen, setAssistantOpen] = useState(true);
  const [selectedText, setSelectedText] = useState<string | null>(null);
  const onSelectionChange = useCallback((text: string | null) => setSelectedText(text), []);
  const pendingOpen = useRef<string | null>(null);
  const viewRef = useRef(view);
  viewRef.current = view;

  const refreshLists = useCallback(async () => {
    const [nextPages, nextProjects] = await Promise.all([api.listPages(), api.listProjects()]);
    setPages(nextPages);
    setProjects(nextProjects);
  }, []);

  const reportError = useCallback((err: unknown) => setError(messageFor(err)), []);

  /** Reloads the open page after an approved AI change so the editor has the new revision. */
  const reloadCurrentPage = useCallback(async () => {
    const current = viewRef.current;
    if (current.kind !== 'page') return;
    try {
      const loaded = await api.getPage(current.id);
      setPage(loaded);
      setLoadNonce((n) => n + 1);
    } catch (err) {
      reportError(err);
    }
  }, [reportError]);

  const aiDataChanged = useCallback(() => {
    void refreshLists().catch(reportError);
    void reloadCurrentPage();
  }, [refreshLists, reloadCurrentPage, reportError]);

  const enterWorkspace = useCallback(
    async (info: WorkspaceInfo) => {
      setWorkspace(info);
      setBoot('ready');
      setView({ kind: 'home' });
      setPage(null);
      try {
        const settings = await api.getSettings();
        applyTheme(settings.theme);
        await refreshLists();
      } catch (err) {
        reportError(err);
      }
    },
    [refreshLists, reportError],
  );

  useEffect(() => {
    applyTheme('system');
    api
      .appStatus()
      .then(({ workspace: current }) => (current ? enterWorkspace(current) : setBoot('onboarding')))
      .catch((err) => {
        reportError(err);
        setBoot('onboarding');
      });
  }, [enterWorkspace, reportError]);

  const go = useCallback(
    (next: View) => {
      setError(null);
      setView(next);
      if (next.kind === 'page') {
        api
          .getPage(next.id)
          .then((loaded) => {
            setPage(loaded);
            setLoadNonce((n) => n + 1);
          })
          .catch((err) => {
            setPage(null);
            setView({ kind: 'home' });
            reportError(err);
          });
      } else {
        setPage(null);
      }
    },
    [reportError],
  );

  const openPage = useCallback((id: string, section?: string) => go({ kind: 'page', id, section }), [go]);

  // Text for the assistant box, set by "Ask about this file" and similar buttons. The nonce makes
  // the same question selectable twice.
  const [prefill, setPrefill] = useState<{ text: string; nonce: number } | null>(null);
  const askAbout = useCallback((text: string) => setPrefill({ text, nonce: Date.now() }), []);

  // Files and folders dropped on the window: folders are linked, Markdown and text files imported.
  const [dragging, setDragging] = useState(false);
  const [dropNotice, setDropNotice] = useState<string | null>(null);
  const handleDrop = useCallback(
    async (paths: string[]) => {
      try {
        const report = await api.sourcesDrop(paths);
        const parts: string[] = [];
        if (report.linked.length) parts.push(`Linked ${report.linked.join(', ')}.`);
        if (report.imported.length) parts.push(`Imported ${report.imported.join(', ')}.`);
        if (report.skipped.length) parts.push(report.skipped.join(' '));
        setDropNotice(parts.join(' ') || 'Nothing was dropped.');
        await refreshLists();
        if (report.linked.length) go({ kind: 'sources' });
      } catch (err) {
        setDropNotice(messageFor(err));
      }
    },
    [go, refreshLists],
  );
  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    try {
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          const payload = event.payload;
          if (payload.type === 'enter' || payload.type === 'over') setDragging(true);
          else if (payload.type === 'leave') setDragging(false);
          else if (payload.type === 'drop') {
            setDragging(false);
            void handleDrop(payload.paths);
          }
        })
        .then((unlisten) => {
          if (cancelled) unlisten();
          else off = unlisten;
        })
        .catch(() => undefined);
    } catch {
      // Not running inside the desktop shell (for example in tests). Drops are simply not handled.
    }
    return () => {
      cancelled = true;
      off?.();
    };
  }, [handleDrop]);

  // Opening a page in another workspace switches workspace, then opens the page once it is loaded.
  const openElsewhere = useCallback(
    async (path: string, pageId: string) => {
      try {
        pendingOpen.current = pageId;
        await enterWorkspace(await api.switchWorkspace(path));
      } catch (err) {
        pendingOpen.current = null;
        reportError(err);
      }
    },
    [enterWorkspace, reportError],
  );

  useEffect(() => {
    const target = pendingOpen.current;
    if (!workspace || !target) return;
    pendingOpen.current = null;
    openPage(target);
  }, [workspace, openPage]);

  const newPage = useCallback(
    async (parentId: string | null = null) => {
      try {
        const created = await api.createPage('Untitled', parentId);
        await refreshLists();
        go({ kind: 'page', id: created.id });
      } catch (err) {
        reportError(err);
      }
    },
    [go, refreshLists, reportError],
  );

  // Watched folders are checked once when a workspace opens. Nothing runs in the background.
  useEffect(() => {
    if (!workspace) return;
    void api
      .localSyncNow()
      .then((results) => {
        if (results.some((r) => r.imported > 0)) void refreshLists();
      })
      .catch(() => undefined);
  }, [workspace?.id]);

  // Indexing is incremental and cheap when nothing is pending, so it runs after every list change.
  useEffect(() => {
    if (workspace) void api.aiIndexStart().catch(() => undefined);
  }, [workspace, pages]);

  // Keyboard: Ctrl+K opens the command palette.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  if (boot === 'loading') {
    return (
      <div className="centered" role="status">
        Opening Threadwell…
      </div>
    );
  }

  if (boot === 'onboarding' || !workspace) {
    return <Onboarding onReady={enterWorkspace} onError={(message) => setError(message)} error={error} />;
  }

  const paletteActions: PaletteAction[] = [
    { id: 'new-page', label: 'New page', run: () => void newPage(null) },
    { id: 'tasks', label: 'Go to tasks', run: () => go({ kind: 'tasks', projectId: null }) },
    { id: 'search', label: 'Go to search', run: () => go({ kind: 'search', query: '' }) },
    { id: 'trash', label: 'Go to trash', run: () => go({ kind: 'trash' }) },
    { id: 'meetings', label: 'Go to meetings', run: () => go({ kind: 'meetings' }) },
    { id: 'recipes', label: 'Go to recipes', run: () => go({ kind: 'recipes' }) },
    { id: 'sources', label: 'Go to sources', run: () => go({ kind: 'sources' }) },
    { id: 'settings', label: 'Open settings', run: () => go({ kind: 'settings' }) },
    { id: 'home', label: 'Go to home', run: () => go({ kind: 'home' }) },
  ];
  const paletteItems: PaletteAction[] = [
    ...paletteActions,
    ...pages
      .filter((p) => p.deletedAt === null)
      .map((p) => ({ id: `page-${p.id}`, label: `Open page: ${p.title}`, run: () => openPage(p.id) })),
  ];

  return (
    <div className={assistantOpen ? 'app-shell with-assistant' : 'app-shell'}>
      {dragging && (
        <div className="drop-overlay" aria-hidden="true">
          Drop a folder to link it, or a Markdown or text file to import it
        </div>
      )}
      {dropNotice && (
        <div className="drop-notice" role="status">
          <span>{dropNotice}</span>
          <button type="button" onClick={() => setDropNotice(null)}>
            Dismiss
          </button>
        </div>
      )}
      <Sidebar
        workspaceName={workspace.name}
        workspaceId={workspace.id}
        pages={pages}
        projects={projects}
        activeView={view}
        onNavigate={go}
        onOpenPage={openPage}
        onOpenElsewhere={(path, pageId) => void openElsewhere(path, pageId)}
        onNewPage={(parent) => void newPage(parent)}
        onSearch={(query) => go({ kind: 'search', query })}
      />

      <main className="main" id="main">
        <Suspense
          fallback={
            <p role="status" className="muted">
              Loading…
            </p>
          }
        >
          <div className="main-toolbar">
            <button type="button" aria-expanded={assistantOpen} onClick={() => setAssistantOpen((open) => !open)}>
              {assistantOpen ? 'Hide assistant' : 'Show assistant'}
            </button>
          </div>
          {error && (
            <div role="alert" className="banner">
              <span>{error}</span>
              <button type="button" onClick={() => setError(null)} aria-label="Dismiss message">
                ×
              </button>
            </div>
          )}

          {view.kind === 'home' && (
            <Home workspace={workspace} pages={pages} onOpenPage={openPage} onNewPage={() => void newPage(null)} />
          )}

          {view.kind === 'page' && page && page.id === view.id && (
            <PageEditor
              key={`${page.id}:${loadNonce}`}
              page={page}
              section={view.kind === 'page' ? view.section : undefined}
              onAskAbout={askAbout}
              pages={pages}
              onNavigate={openPage}
              onChanged={() => void refreshLists().catch(reportError)}
              onSelectionChange={onSelectionChange}
              onTrashed={() => {
                void refreshLists().catch(reportError);
                go({ kind: 'home' });
              }}
            />
          )}

          {view.kind === 'tasks' && (
            <TasksView
              key={view.projectId ?? 'all'}
              projects={projects}
              initialProjectId={view.projectId}
              pages={pages}
              onOpenPage={openPage}
              onProjectsChanged={() => void refreshLists().catch(reportError)}
              onError={reportError}
            />
          )}

          {view.kind === 'search' && (
            <SearchView
              initialQuery={view.query}
              onOpenPage={openPage}
              onOpenTasks={() => go({ kind: 'tasks', projectId: null })}
            />
          )}

          {view.kind === 'settings' && (
            <SettingsView
              workspace={workspace}
              onOpenPage={openPage}
              onPagesChanged={() => void refreshLists().catch(reportError)}
              onThemeChange={applyTheme}
              onRestored={(info) => void enterWorkspace(info)}
              onSwitched={(info) => void enterWorkspace(info)}
              onNewWorkspace={() => setBoot('onboarding')}
              onError={reportError}
            />
          )}

          {view.kind === 'meetings' && <MeetingsView onOpenPage={openPage} onError={reportError} />}

          {view.kind === 'sources' && (
            <SourcesView
              pages={pages}
              onOpenPage={openPage}
              onAskAbout={askAbout}
              onChanged={() => void refreshLists().catch(reportError)}
              onError={reportError}
            />
          )}

          {view.kind === 'recipes' && <RecipesView onError={reportError} />}

          {view.kind === 'trash' && (
            <TrashView onRestored={() => void refreshLists().catch(reportError)} onError={reportError} />
          )}
        </Suspense>
      </main>

      {assistantOpen && (
        <AssistantPanel
          key={workspace?.id ?? 'none'}
          pageId={view.kind === 'page' ? view.id : null}
          pageTitle={view.kind === 'page' && page ? page.title : null}
          selectedText={selectedText}
          prefill={prefill}
          onOpenPage={openPage}
          onOpenTasks={() => go({ kind: 'tasks', projectId: null })}
          onOpenSettings={() => go({ kind: 'settings' })}
          onDataChanged={aiDataChanged}
        />
      )}

      <CommandPalette open={paletteOpen} actions={paletteItems} onClose={() => setPaletteOpen(false)} />
    </div>
  );
}

function Home({
  workspace,
  pages,
  onOpenPage,
  onNewPage,
}: {
  workspace: WorkspaceInfo;
  pages: PageSummary[];
  onOpenPage: (id: string) => void;
  onNewPage: () => void;
}) {
  // Linked source files are listed in the Sources view, so they are left out of the notes shown here.
  const recent = [...pages]
    .filter((p) => !p.sourceId)
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    .slice(0, 8);
  return (
    <section className="home" aria-labelledby="home-title">
      <h1 id="home-title">{workspace.name}</h1>
      <p className="muted">Your notes, projects and tasks, stored in {workspace.path}.</p>
      <div className="home-actions">
        <button type="button" className="primary" onClick={onNewPage}>
          New page
        </button>
      </div>
      {recent.length === 0 ? (
        <p>No pages yet. Create one to get started.</p>
      ) : (
        <>
          <h2>Recently edited</h2>
          <ul className="plain-list">
            {recent.map((p) => (
              <li key={p.id}>
                <button type="button" className="link-button" onClick={() => onOpenPage(p.id)}>
                  {p.title}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
