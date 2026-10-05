import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { App } from '../src/App';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const WORKSPACE = {
  id: 'ws-1',
  name: 'Test workspace',
  path: 'C:\\Notes',
  createdAt: '2026-01-01T00:00:00.000Z',
  schemaVersion: 1,
};

function mockBackend(responses: Record<string, unknown>) {
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command in responses) return responses[command];
    throw { code: 'validation', message: `unexpected command ${command}` };
  });
}

describe('App shell', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it('shows onboarding when no workspace is open', async () => {
    mockBackend({ app_status: { workspace: null } });
    render(<App />);
    expect(await screen.findByRole('radio', { name: 'Create a workspace' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Threadwell' })).toBeTruthy();
  });

  it('opens the workspace home with its pages and no network-dependent UI', async () => {
    mockBackend({
      app_status: { workspace: WORKSPACE },
      get_settings: { theme: 'system' },
      list_pages: [
        {
          id: '11111111-1111-4111-8111-111111111111',
          parentId: null,
          title: 'Atlas notes',
          revision: 1,
          isFavorite: false,
          deletedAt: null,
          updatedAt: '2026-01-01T00:00:00.000Z',
        },
      ],
      list_projects: [],
    });
    render(<App />);

    await waitFor(() => expect(screen.getByRole('heading', { name: 'Test workspace' })).toBeTruthy());
    expect(screen.getAllByRole('button', { name: 'Atlas notes' }).length).toBeGreaterThan(0);
    expect(screen.queryByText(/sign in/i)).toBeNull();
    // Excluded by design: accounts and billing are not part of this product.
    expect(screen.queryByRole('button', { name: /billing|subscription|log in/i })).toBeNull();
  });
});
