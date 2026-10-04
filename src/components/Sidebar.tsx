import { useMemo, useState, type FormEvent } from 'react';
import { buildTree, type TreeNode } from '../lib/pure';
import type { PageSummary, Project } from '../lib/types';
import type { View } from '../App';

interface Props {
  workspaceName: string;
  pages: PageSummary[];
  projects: Project[];
  activeView: View;
  onNavigate: (view: View) => void;
  onOpenPage: (id: string) => void;
  onNewPage: (parentId: string | null) => void;
  onSearch: (query: string) => void;
}

export function Sidebar({
  workspaceName,
  pages,
  projects,
  activeView,
  onNavigate,
  onOpenPage,
  onNewPage,
  onSearch,
}: Props) {
  const [query, setQuery] = useState('');
  const live = useMemo(() => pages.filter((p) => p.deletedAt === null), [pages]);
  const tree = useMemo(() => buildTree(live), [live]);
  const favorites = live.filter((p) => p.isFavorite).sort((a, b) => a.title.localeCompare(b.title));
  const activePageId = activeView.kind === 'page' ? activeView.id : null;

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
        <NavItem label="Trash" active={activeView.kind === 'trash'} onClick={() => onNavigate({ kind: 'trash' })} />
        <NavItem
          label="Settings"
          active={activeView.kind === 'settings'}
          onClick={() => onNavigate({ kind: 'settings' })}
        />
      </ul>

      <div className="sidebar-section">
        <div className="section-head">
          <h2>Pages</h2>
          <button type="button" className="icon-button" aria-label="New top-level page" onClick={() => onNewPage(null)}>
            +
          </button>
        </div>
        {tree.length === 0 ? (
          <p className="muted small">No pages yet.</p>
        ) : (
          <TreeList nodes={tree} activePageId={activePageId} onOpenPage={onOpenPage} onNewPage={onNewPage} />
        )}
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
  onOpenPage,
  onNewPage,
}: {
  nodes: TreeNode[];
  activePageId: string | null;
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
  onOpenPage,
  onNewPage,
}: {
  node: TreeNode;
  activePageId: string | null;
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
        <button
          type="button"
          className="icon-button"
          aria-label={`Add a sub-page under ${node.page.title}`}
          onClick={() => onNewPage(node.page.id)}
        >
          +
        </button>
      </div>
      {hasChildren && expanded && (
        <TreeList nodes={node.children} activePageId={activePageId} onOpenPage={onOpenPage} onNewPage={onNewPage} />
      )}
    </li>
  );
}
