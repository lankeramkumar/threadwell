import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { Sidebar } from '../src/components/Sidebar';
import type { PageSummary } from '../src/lib/types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const page = (id: string, title: string): PageSummary => ({
  id,
  parentId: null,
  title,
  revision: 1,
  isFavorite: false,
  deletedAt: null,
  updatedAt: '2026-10-05T00:00:00Z',
  sourceId: null,
});

describe('Sidebar pages by workspace', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
      if (command === 'workspaces_list')
        return [
          { name: 'Work', path: 'C:/work', active: true, available: true },
          { name: 'Home', path: 'C:/home', active: false, available: true },
        ];
      if (command === 'workspace_pages' && (args as { path: string }).path === 'C:/home')
        return [page('h1', 'Home note')];
      return undefined;
    });
  });

  it('shows the open workspace pages and loads another workspace only when opened', async () => {
    const onOpenElsewhere = vi.fn();
    render(
      <Sidebar
        workspaceName="Work"
        workspaceId="w1"
        pages={[page('p1', 'Work note')]}
        projects={[]}
        activeView={{ kind: 'home' }}
        onNavigate={() => undefined}
        onOpenPage={() => undefined}
        onOpenElsewhere={onOpenElsewhere}
        onNewPage={() => undefined}
        onSearch={() => undefined}
      />,
    );

    expect(await screen.findByRole('button', { name: /Work\s*\(open\)/ })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Work note' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Home note' })).toBeNull();
    expect(
      vi
        .mocked(invoke)
        .mock.calls.some((c) => c[0] === 'workspace_pages' && (c[1] as { path: string }).path === 'C:/home'),
    ).toBe(false);

    fireEvent.click(screen.getByRole('button', { name: /▸ Home/ }));
    const homeNote = await screen.findByRole('button', { name: 'Home note' });
    fireEvent.click(homeNote);
    expect(onOpenElsewhere).toHaveBeenCalledWith('C:/home', 'h1');
  });

  it('offers no add-page control on another workspace', async () => {
    render(
      <Sidebar
        workspaceName="Work"
        workspaceId="w1"
        pages={[]}
        projects={[]}
        activeView={{ kind: 'home' }}
        onNavigate={() => undefined}
        onOpenPage={() => undefined}
        onOpenElsewhere={() => undefined}
        onNewPage={() => undefined}
        onSearch={() => undefined}
      />,
    );
    fireEvent.click(await screen.findByRole('button', { name: /▸ Home/ }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Home note' })).toBeTruthy());
    expect(screen.queryByRole('button', { name: 'Add a sub-page under Home note' })).toBeNull();
  });
});
