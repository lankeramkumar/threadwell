// Pure helpers with no Tauri or React dependencies, so they can be unit tested directly.
import type { AppErrorPayload, PageSummary, Task, TaskStatus } from './types';

/** Internal links are stored as threadwell://page/<id> hrefs. */
export const LINK_PREFIX = 'threadwell://page/';

// ---- errors ---------------------------------------------------------------

export function isAppError(value: unknown): value is AppErrorPayload {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    'message' in value &&
    typeof (value as AppErrorPayload).message === 'string'
  );
}

/** A user-facing message for anything thrown by a Tauri command. */
export function messageFor(error: unknown): string {
  if (isAppError(error)) return error.message;
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  return 'Something went wrong. Try again.';
}

export function isConflict(error: unknown): boolean {
  return isAppError(error) && error.code === 'conflict';
}

// ---- page tree ------------------------------------------------------------

export interface TreeNode {
  page: PageSummary;
  children: TreeNode[];
}

/**
 * Builds a page tree. Pages whose parent is missing appear at the top level,
 * so nothing becomes unreachable.
 */
export function buildTree(pages: PageSummary[]): TreeNode[] {
  const ids = new Set(pages.map((p) => p.id));
  const nodes = new Map<string, TreeNode>(pages.map((page) => [page.id, { page, children: [] }]));
  const roots: TreeNode[] = [];
  for (const page of pages) {
    const node = nodes.get(page.id)!;
    if (page.parentId !== null && ids.has(page.parentId)) {
      nodes.get(page.parentId)!.children.push(node);
    } else {
      roots.push(node);
    }
  }
  const sortRecursive = (list: TreeNode[]) => {
    list.sort((a, b) => a.page.title.localeCompare(b.page.title));
    list.forEach((n) => sortRecursive(n.children));
  };
  sortRecursive(roots);
  return roots;
}

/** Titles from the top-level ancestor down to the page itself. */
export function breadcrumbs(pages: PageSummary[], id: string): PageSummary[] {
  const byId = new Map(pages.map((p) => [p.id, p]));
  const trail: PageSummary[] = [];
  const seen = new Set<string>();
  let current = byId.get(id);
  while (current && !seen.has(current.id)) {
    seen.add(current.id);
    trail.unshift(current);
    current = current.parentId ? byId.get(current.parentId) : undefined;
  }
  return trail;
}

/** A page and all of its descendants. Used to stop a page being moved under itself. */
export function subtreeIds(pages: PageSummary[], id: string): Set<string> {
  const result = new Set<string>([id]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const p of pages) {
      if (p.parentId && result.has(p.parentId) && !result.has(p.id)) {
        result.add(p.id);
        grew = true;
      }
    }
  }
  return result;
}

// ---- search snippets ------------------------------------------------------

export interface SnippetPart {
  text: string;
  highlight: boolean;
}

/** Splits a backend snippet on [matched] markers into plain parts for safe rendering. */
export function parseSnippet(snippet: string): SnippetPart[] {
  const parts: SnippetPart[] = [];
  let rest = snippet;
  while (rest.length > 0) {
    const open = rest.indexOf('[');
    const close = open >= 0 ? rest.indexOf(']', open) : -1;
    if (open < 0 || close < 0) {
      parts.push({ text: rest, highlight: false });
      break;
    }
    if (open > 0) parts.push({ text: rest.slice(0, open), highlight: false });
    parts.push({ text: rest.slice(open + 1, close), highlight: true });
    rest = rest.slice(close + 1);
  }
  return parts;
}

// ---- tasks ----------------------------------------------------------------

export const STATUS_ORDER: TaskStatus[] = ['todo', 'doing', 'done'];

export const STATUS_LABELS: Record<TaskStatus, string> = {
  todo: 'To do',
  doing: 'In progress',
  done: 'Done',
};

export function groupTasksByStatus(tasks: Task[]): Record<TaskStatus, Task[]> {
  const groups: Record<TaskStatus, Task[]> = { todo: [], doing: [], done: [] };
  for (const task of tasks) groups[task.status].push(task);
  return groups;
}

/** Moves a status one column left or right, clamped to the board. */
export function adjacentStatus(status: TaskStatus, direction: -1 | 1): TaskStatus {
  const index = STATUS_ORDER.indexOf(status) + direction;
  return STATUS_ORDER[Math.min(STATUS_ORDER.length - 1, Math.max(0, index))];
}

// ---- dates ----------------------------------------------------------------

export function formatTimestamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
}

export function isoDate(now: Date = new Date()): string {
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, '0');
  const d = String(now.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}
