import { describe, expect, it } from 'vitest';
import {
  adjacentStatus,
  breadcrumbs,
  buildTree,
  groupTasksByStatus,
  isConflict,
  isoDate,
  messageFor,
  parseSnippet,
  subtreeIds,
} from '../src/lib/pure';
import type { PageSummary, Task } from '../src/lib/types';

function page(id: string, title: string, parentId: string | null = null): PageSummary {
  return {
    id,
    parentId,
    title,
    revision: 1,
    isFavorite: false,
    deletedAt: null,
    updatedAt: '2026-01-01T00:00:00.000Z',
    sourceId: null,
  };
}

function task(id: string, status: Task['status']): Task {
  return {
    id,
    projectId: null,
    title: id,
    description: '',
    status,
    priority: 'medium',
    dueDate: null,
    sourcePageId: null,
    revision: 1,
    createdAt: '',
    updatedAt: '',
  };
}

describe('buildTree', () => {
  it('nests children under parents and sorts by title', () => {
    const tree = buildTree([page('b', 'Beta'), page('a', 'Alpha'), page('a1', 'Zed', 'a'), page('a2', 'Able', 'a')]);
    expect(tree.map((n) => n.page.title)).toEqual(['Alpha', 'Beta']);
    expect(tree[0].children.map((n) => n.page.title)).toEqual(['Able', 'Zed']);
  });

  it('keeps pages with a missing parent reachable at the top level', () => {
    const tree = buildTree([page('orphan', 'Orphan', 'gone')]);
    expect(tree).toHaveLength(1);
    expect(tree[0].page.id).toBe('orphan');
  });
});

describe('breadcrumbs and subtrees', () => {
  const pages = [page('root', 'Root'), page('mid', 'Mid', 'root'), page('leaf', 'Leaf', 'mid')];

  it('lists ancestors from the top down', () => {
    expect(breadcrumbs(pages, 'leaf').map((p) => p.title)).toEqual(['Root', 'Mid', 'Leaf']);
  });

  it('finds a page and all of its descendants', () => {
    expect([...subtreeIds(pages, 'root')].sort()).toEqual(['leaf', 'mid', 'root']);
    expect([...subtreeIds(pages, 'leaf')]).toEqual(['leaf']);
  });

  it('survives a corrupted parent cycle without hanging', () => {
    const cyclic = [page('x', 'X', 'y'), page('y', 'Y', 'x')];
    expect(breadcrumbs(cyclic, 'x').length).toBeLessThanOrEqual(2);
  });
});

describe('parseSnippet', () => {
  it('turns bracketed matches into highlighted parts without markup', () => {
    expect(parseSnippet('we chose [passkeys] today')).toEqual([
      { text: 'we chose ', highlight: false },
      { text: 'passkeys', highlight: true },
      { text: ' today', highlight: false },
    ]);
  });

  it('keeps angle brackets and scripts as plain text', () => {
    const parts = parseSnippet('<img src=x onerror=alert(1)> [hit]');
    expect(parts[0]).toEqual({ text: '<img src=x onerror=alert(1)> ', highlight: false });
    expect(parts[1].highlight).toBe(true);
  });
});

describe('tasks', () => {
  it('groups tasks by status and keeps empty columns', () => {
    const groups = groupTasksByStatus([task('1', 'todo'), task('2', 'done')]);
    expect(groups.todo.map((t) => t.id)).toEqual(['1']);
    expect(groups.doing).toEqual([]);
    expect(groups.done.map((t) => t.id)).toEqual(['2']);
  });

  it('moves status one column at a time and stops at the edges', () => {
    expect(adjacentStatus('todo', -1)).toBe('todo');
    expect(adjacentStatus('todo', 1)).toBe('doing');
    expect(adjacentStatus('done', 1)).toBe('done');
    expect(adjacentStatus('doing', -1)).toBe('todo');
  });
});

describe('errors', () => {
  it('uses the backend message when it has one', () => {
    const err = { code: 'conflict', message: 'Reload first' };
    expect(messageFor(err)).toBe('Reload first');
    expect(isConflict(err)).toBe(true);
  });

  it('falls back to a generic message for unknown shapes', () => {
    expect(messageFor({ weird: true })).toBe('Something went wrong. Try again.');
    expect(isConflict('boom')).toBe(false);
  });
});

describe('dates', () => {
  it('formats local calendar dates as YYYY-MM-DD', () => {
    expect(isoDate(new Date(2026, 0, 5))).toBe('2026-01-05');
  });
});
