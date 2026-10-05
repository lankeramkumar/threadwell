import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AssistantPanel, DiffView } from '../src/components/AssistantPanel';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined),
}));

function status(state: string, model = 'qwen2.5:3b') {
  return {
    config: { endpoint: 'http://127.0.0.1:11434', model, allowRemote: false },
    state,
  };
}

describe('AssistantPanel', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockClear();
  });

  it('explains how to fix an unreachable model server and does not allow sending', async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'ai_get_status') return status('unreachable');
      if (command === 'ai_list_conversations') return [];
      if (command === 'workspaces_list') return [];
      return undefined;
    });
    render(
      <AssistantPanel
        pageId={null}
        pageTitle={null}
        selectedText={null}
        onOpenPage={() => undefined}
        onOpenTasks={() => undefined}
        onOpenSettings={() => undefined}
        onDataChanged={() => undefined}
      />,
    );
    expect(await screen.findByText(/Start Ollama, or check the endpoint/)).toBeTruthy();
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('not reachable'));
    const send = screen.getByRole('button', { name: 'Send' }) as HTMLButtonElement;
    expect(send.disabled).toBe(true);
    expect(screen.getByRole('textbox', { name: 'Message the assistant' }).hasAttribute('disabled')).toBe(true);
  });

  it('subscribes to streaming, proposal and completion events', async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'ai_get_status') return status('ready');
      if (command === 'ai_list_conversations') return [];
      if (command === 'workspaces_list') return [];
      return undefined;
    });
    render(
      <AssistantPanel
        pageId={null}
        pageTitle={null}
        selectedText={null}
        onOpenPage={() => undefined}
        onOpenTasks={() => undefined}
        onOpenSettings={() => undefined}
        onDataChanged={() => undefined}
      />,
    );
    await waitFor(() => {
      const events = vi.mocked(listen).mock.calls.map((c) => c[0]);
      expect(events).toEqual(expect.arrayContaining(['ai://delta', 'ai://tool', 'ai://proposal', 'ai://done']));
    });
  });
});

describe('DiffView', () => {
  it('marks added and removed lines and never renders them as markup', () => {
    const { container } = render(<DiffView text={' keep\n-old <b>x</b>\n+new\n'} />);
    expect(container.querySelectorAll('.diff-remove').length).toBe(1);
    expect(container.querySelectorAll('.diff-add').length).toBe(1);
    expect(container.querySelector('b')).toBeNull();
    expect(container.textContent).toContain('<b>x</b>');
  });
});

describe('AssistantPanel workspace scope', () => {
  it('offers other workspaces read-only and sends the chosen scope with the message', async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'ai_get_status') return status('ready');
      if (command === 'ai_list_conversations') return [];
      if (command === 'workspaces_list')
        return [
          { name: 'Work', path: 'C:/work', active: true, available: true },
          { name: 'Home', path: 'C:/home', active: false, available: true },
        ];
      if (command === 'ai_chat_send') return { runId: 'r1', conversationId: 'c1' };
      return undefined;
    });
    render(
      <AssistantPanel
        pageId={null}
        pageTitle={null}
        selectedText={null}
        onOpenPage={() => undefined}
        onOpenTasks={() => undefined}
        onOpenSettings={() => undefined}
        onDataChanged={() => undefined}
      />,
    );
    const picker = await screen.findByRole('combobox', { name: 'Answer from' });
    fireEvent.change(picker, { target: { value: 'Home' } });
    fireEvent.change(screen.getByRole('textbox', { name: 'Message the assistant' }), {
      target: { value: 'What is in Home?' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => {
      const call = vi.mocked(invoke).mock.calls.find((c) => c[0] === 'ai_chat_send');
      expect(call?.[1]).toMatchObject({ request: { scope: 'Home', message: 'What is in Home?' } });
    });
  });
});
