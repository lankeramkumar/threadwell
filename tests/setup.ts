import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

// The Tauri event bridge only exists inside the desktop shell. Tests stub it so components
// that subscribe to backend events render without it.
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined),
  emit: vi.fn(async () => undefined),
}));

afterEach(() => {
  cleanup();
});
