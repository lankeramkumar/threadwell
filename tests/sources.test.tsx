import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { SourcesView } from '../src/components/SourcesView';
import type { PageSummary, SourceInfo } from '../src/lib/types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const source: SourceInfo = {
  id: 's1',
  name: 'payments',
  rootPath: 'C:/code/payments',
  addedAt: '2026-10-05T00:00:00Z',
  lastSyncedAt: null,
  lastSummary: null,
  fileCount: 1,
};

const file: PageSummary = {
  id: 'f1',
  parentId: null,
  title: 'src/charge.rs',
  revision: 1,
  isFavorite: false,
  deletedAt: null,
  updatedAt: '2026-10-05T00:00:00Z',
  sourceId: 's1',
};

describe('SourcesView', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'sources_list') return [source];
      if (command === 'sources_sync')
        return {
          added: 0,
          updated: 1,
          removed: 0,
          unchanged: 0,
          skipped: 0,
          failed: 0,
          failures: [],
          truncated: false,
          stopped: false,
        };
      return undefined;
    });
  });

  it('lists linked files and opens one of them', async () => {
    const onOpenPage = vi.fn();
    render(
      <SourcesView pages={[file]} onOpenPage={onOpenPage} onChanged={() => undefined} onError={() => undefined} />,
    );
    expect(await screen.findByRole('heading', { name: 'payments' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Show files (1)' }));
    fireEvent.click(screen.getByRole('button', { name: 'src/charge.rs' }));
    expect(onOpenPage).toHaveBeenCalledWith('f1');
  });

  it('syncs a source and reports what changed', async () => {
    render(
      <SourcesView pages={[file]} onOpenPage={() => undefined} onChanged={() => undefined} onError={() => undefined} />,
    );
    fireEvent.click(await screen.findByRole('button', { name: 'Sync now' }));
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('1 updated'));
    expect(
      vi.mocked(invoke).mock.calls.some((c) => c[0] === 'sources_sync' && (c[1] as { id: string }).id === 's1'),
    ).toBe(true);
  });
});
