import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from './lib/api';
import { messageFor } from './lib/pure';
import type { Page, PageSummary, Project, WorkspaceInfo } from './lib/types';
import { applyTheme } from './lib/theme';
import { Onboarding } from './components/Onboarding';
import { Sidebar } from './components/Sidebar';
import { PageEditor } from './components/PageEditor';
import { TasksView } from './components/TasksView';
import { SearchView } from './components/SearchView';
import { SettingsView } from './components/SettingsView';
import { TrashView } from './components/TrashView';
import { CommandPalette, type PaletteAction } from './components/CommandPalette';
import { AssistantPanel } from './components/AssistantPanel';
import { MeetingsView } from './components/MeetingsView';
import { RecipesView } from './components/RecipesView';

export type View =
  | { kind: 'home' }
  | { kind: 'page'; id: string }
  | { kind: 'tasks'; projectId: string | null }
  | { kind: 'search'; query: string }
  | { kind: 'settings' }
  | { kind: 'trash' }
  | { kind: 'meetings' }
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

  const openPage = useCallback((id: string) => go({ kind: 'page', id }), [go]);

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
      <Sidebar
        workspaceName={workspace.name}
        pages={pages}
        projects={projects}
        activeView={view}
        onNavigate={go}
        onOpenPage={openPage}
        onNewPage={(parent) => void newPage(parent)}
        onSearch={(query) => go({ kind: 'search', query })}
      />

      <main className="main" id="main">
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
            pages={pages}
            onNavigate={openPage}
            onChanged={() => void refreshLists().catch(reportError)}
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
            onError={reportError}
          />
        )}

        {view.kind === 'meetings' && <MeetingsView onOpenPage={openPage} onError={reportError} />}

        {view.kind === 'recipes' && <RecipesView onError={reportError} />}

        {view.kind === 'trash' && (
          <TrashView onRestored={() => void refreshLists().catch(reportError)} onError={reportError} />
        )}
      </main>

      {assistantOpen && (
        <AssistantPanel
          pageId={view.kind === 'page' ? view.id : null}
          pageTitle={view.kind === 'page' && page ? page.title : null}
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
  const recent = [...pages].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)).slice(0, 8);
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
