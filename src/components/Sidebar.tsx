import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { api } from '../lib/api';
import { buildTree, messageFor, type TreeNode } from '../lib/pure';
import type { PageSummary, Project, WorkspaceListItem } from '../lib/types';
import type { View } from '../App';

interface Props {
  workspaceName: string;
  /** Id of the open workspace. The list of workspaces is reloaded when it changes. */
  workspaceId: string;
  pages: PageSummary[];
  projects: Project[];
  activeView: View;
  onNavigate: (view: View) => void;
  onOpenPage: (id: string) => void;
  /** Opens a page in another workspace. The app switches workspace first. */
  onOpenElsewhere: (path: string, pageId: string) => void;
  onNewPage: (parentId: string | null) => void;
  onSearch: (query: string) => void;
}

export function Sidebar({
  workspaceName,
  workspaceId,
  pages,
  projects,
  activeView,
  onNavigate,
  onOpenPage,
  onOpenElsewhere,
  onNewPage,
  onSearch,
}: Props) {
  const [query, setQuery] = useState('');
  const [workspaces, setWorkspaces] = useState<WorkspaceListItem[]>([]);
  // Linked source files are listed in the Sources view, not in the page tree.
  const live = useMemo(() => pages.filter((p) => p.deletedAt === null && !p.sourceId), [pages]);
  const favorites = live.filter((p) => p.isFavorite).sort((a, b) => a.title.localeCompare(b.title));
  const activePageId = activeView.kind === 'page' ? activeView.id : null;

  // Reload the workspace list whenever the open workspace changes, so the open marker stays right.
  useEffect(() => {
    let current = true;
    api
      .listWorkspaces()
      .then((list) => current && setWorkspaces(list))
      .catch(() => current && setWorkspaces([]));
    return () => {
      current = false;
    };
  }, [workspaceId]);

  const submitSearch = (event: FormEvent) => {
    event.preventDefault();
    onSearch(query.trim());
  };

  return (
    <nav className="sidebar" aria-label="Workspace">
      <div className="workspace-name" title={workspaceName}>
        {workspaceName}
      </div>

      <form role="search" onSubmit={submitSearch} className="sidebar-search">
        <input
          type="search"
          aria-label="Search workspace"
          placeholder="Search…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </form>

      <ul className="nav-list">
        <NavItem label="Home" active={activeView.kind === 'home'} onClick={() => onNavigate({ kind: 'home' })} />
        <NavItem
          label="Tasks"
          active={activeView.kind === 'tasks' && activeView.projectId === null}
          onClick={() => onNavigate({ kind: 'tasks', projectId: null })}
        />
        <NavItem
          label="Meetings"
          active={activeView.kind === 'meetings'}
          onClick={() => onNavigate({ kind: 'meetings' })}
        />
        <NavItem
          label="Sources"
          active={activeView.kind === 'sources'}
          onClick={() => onNavigate({ kind: 'sources' })}
        />
        <NavItem
          label="Recipes"
          active={activeView.kind === 'recipes'}
          onClick={() => onNavigate({ kind: 'recipes' })}
        />
        <NavItem label="Trash" active={activeView.kind === 'trash'} onClick={() => onNavigate({ kind: 'trash' })} />
        <NavItem
          label="Settings"
          active={activeView.kind === 'settings'}
          onClick={() => onNavigate({ kind: 'settings' })}
        />
      </ul>

      <div className="sidebar-section">
        <h2>Workspaces</h2>
        <p className="muted small">Each workspace keeps its own pages. Other workspaces are read only.</p>
        <ul className="plain-list" aria-label="Pages by workspace">
          {orderedWorkspaces(workspaces).map((item) => (
            <WorkspaceGroup
              key={item.path}
              item={item}
              livePages={live}
              activePageId={activePageId}
              onOpenPage={onOpenPage}
              onOpenElsewhere={onOpenElsewhere}
              onNewPage={onNewPage}
            />
          ))}
        </ul>
      </div>

      {favorites.length > 0 && (
        <div className="sidebar-section">
          <h2>Favorites</h2>
          <ul className="plain-list">
            {favorites.map((p) => (
              <li key={p.id}>
                <button
                  type="button"
                  className={p.id === activePageId ? 'nav-button is-active' : 'nav-button'}
                  onClick={() => onOpenPage(p.id)}
                >
                  {p.title}
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      {projects.length > 0 && (
        <div className="sidebar-section">
          <h2>Projects</h2>
          <ul className="plain-list">
            {projects.map((project) => (
              <li key={project.id}>
                <button
                  type="button"
                  className={
                    activeView.kind === 'tasks' && activeView.projectId === project.id
                      ? 'nav-button is-active'
                      : 'nav-button'
                  }
                  onClick={() => onNavigate({ kind: 'tasks', projectId: project.id })}
                >
                  {project.name}
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </nav>
  );
}

/** The open workspace first, then the rest in list order. */
function orderedWorkspaces(items: WorkspaceListItem[]): WorkspaceListItem[] {
  return [...items].sort((a, b) => Number(b.active) - Number(a.active));
}

function WorkspaceGroup({
  item,
  livePages,
  activePageId,
  onOpenPage,
  onOpenElsewhere,
  onNewPage,
}: {
  item: WorkspaceListItem;
  livePages: PageSummary[];
  activePageId: string | null;
  onOpenPage: (id: string) => void;
  onOpenElsewhere: (path: string, pageId: string) => void;
  onNewPage: (parentId: string | null) => void;
}) {
  const [open, setOpen] = useState(item.active);
  const [remote, setRemote] = useState<PageSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Another workspace's pages are read only when its group is opened.
  useEffect(() => {
    if (!open || item.active || !item.available) return;
    let current = true;
    api
      .workspacePages(item.path)
      .then((list) => current && setRemote(list))
      .catch((err) => current && setError(messageFor(err)));
    return () => {
      current = false;
    };
  }, [open, item.active, item.available, item.path]);

  const source = item.active ? livePages : remote;
  const tree = useMemo(() => buildTree((source ?? []).filter((p) => p.deletedAt === null && !p.sourceId)), [source]);

  return (
    <li className="workspace-group">
      <button
        type="button"
        className={item.active ? 'nav-button workspace-head is-active' : 'nav-button workspace-head'}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        {open ? '▾' : '▸'} {item.name}
        {item.active && <span className="small"> (open)</span>}
      </button>
      {open && !item.available && <p className="muted small">Folder or database not found.</p>}
      {open && item.available && error && <p className="inline-error small">{error}</p>}
      {open && item.available && !item.active && !remote && !error && <p className="muted small">Loading…</p>}
      {open && item.available && source && (
        <>
          {tree.length === 0 ? (
            <p className="muted small">No pages yet.</p>
          ) : (
            <TreeList
              nodes={tree}
              activePageId={item.active ? activePageId : null}
              readOnly={!item.active}
              onOpenPage={item.active ? onOpenPage : (id) => onOpenElsewhere(item.path, id)}
              onNewPage={onNewPage}
            />
          )}
          {item.active && (
            <button type="button" className="nav-button small" onClick={() => onNewPage(null)}>
              + New top-level page
            </button>
          )}
        </>
      )}
    </li>
  );
}

function NavItem({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <li>
      <button
        type="button"
        className={active ? 'nav-button is-active' : 'nav-button'}
        aria-current={active ? 'page' : undefined}
        onClick={onClick}
      >
        {label}
      </button>
    </li>
  );
}

function TreeList({
  nodes,
  activePageId,
  readOnly = false,
  onOpenPage,
  onNewPage,
}: {
  nodes: TreeNode[];
  activePageId: string | null;
  readOnly?: boolean;
  onOpenPage: (id: string) => void;
  onNewPage: (parentId: string | null) => void;
}) {
  return (
    <ul className="tree">
      {nodes.map((node) => (
        <TreeItem
          key={node.page.id}
          node={node}
          activePageId={activePageId}
          readOnly={readOnly}
          onOpenPage={onOpenPage}
          onNewPage={onNewPage}
        />
      ))}
    </ul>
  );
}

function TreeItem({
  node,
  activePageId,
  readOnly,
  onOpenPage,
  onNewPage,
}: {
  node: TreeNode;
  activePageId: string | null;
  readOnly: boolean;
  onOpenPage: (id: string) => void;
  onNewPage: (parentId: string | null) => void;
}) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children.length > 0;
  const isActive = node.page.id === activePageId;
  return (
    <li>
      <div className="tree-row">
        {hasChildren ? (
          <button
            type="button"
            className="icon-button"
            aria-expanded={expanded}
            aria-label={`${expanded ? 'Collapse' : 'Expand'} ${node.page.title}`}
            onClick={() => setExpanded((v) => !v)}
          >
            {expanded ? '▾' : '▸'}
          </button>
        ) : (
          <span className="tree-spacer" aria-hidden="true" />
        )}
        <button
          type="button"
          className={isActive ? 'nav-button tree-title is-active' : 'nav-button tree-title'}
          aria-current={isActive ? 'page' : undefined}
          onClick={() => onOpenPage(node.page.id)}
        >
          {node.page.title}
        </button>
        {!readOnly && (
          <button
            type="button"
            className="icon-button"
            aria-label={`Add a sub-page under ${node.page.title}`}
            onClick={() => onNewPage(node.page.id)}
          >
            +
          </button>
        )}
      </div>
      {hasChildren && expanded && (
        <TreeList
          nodes={node.children}
          activePageId={activePageId}
          readOnly={readOnly}
          onOpenPage={onOpenPage}
          onNewPage={onNewPage}
        />
      )}
    </li>
  );
}
