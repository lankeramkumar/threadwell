// "/" block commands: a Tiptap suggestion extension with a small keyboard-driven menu.
import { Extension, type Editor, type Range } from '@tiptap/core';
import Suggestion, { type SuggestionProps } from '@tiptap/suggestion';

interface SlashItem {
  title: string;
  keywords: string;
  run: (editor: Editor) => void;
}

const ITEMS: SlashItem[] = [
  { title: 'Text', keywords: 'paragraph plain', run: (e) => e.chain().focus().setParagraph().run() },
  { title: 'Heading 1', keywords: 'h1 title', run: (e) => e.chain().focus().setHeading({ level: 1 }).run() },
  { title: 'Heading 2', keywords: 'h2 section', run: (e) => e.chain().focus().setHeading({ level: 2 }).run() },
  { title: 'Heading 3', keywords: 'h3 subsection', run: (e) => e.chain().focus().setHeading({ level: 3 }).run() },
  { title: 'Bulleted list', keywords: 'ul bullet unordered', run: (e) => e.chain().focus().toggleBulletList().run() },
  { title: 'Numbered list', keywords: 'ol ordered steps', run: (e) => e.chain().focus().toggleOrderedList().run() },
  { title: 'Checklist', keywords: 'task todo checkbox', run: (e) => e.chain().focus().toggleTaskList().run() },
  { title: 'Quote', keywords: 'blockquote callout', run: (e) => e.chain().focus().toggleBlockquote().run() },
  { title: 'Code block', keywords: 'code snippet', run: (e) => e.chain().focus().toggleCodeBlock().run() },
  {
    title: 'Table',
    keywords: 'grid columns',
    run: (e) => e.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run(),
  },
  { title: 'Divider', keywords: 'hr rule line separator', run: (e) => e.chain().focus().setHorizontalRule().run() },
];

export function filterSlashItems(query: string): SlashItem[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  return ITEMS.filter((item) => {
    const haystack = `${item.title} ${item.keywords}`.toLowerCase();
    return terms.every((term) => haystack.includes(term));
  });
}

function renderMenu() {
  let el: HTMLDivElement | null = null;
  let selected = 0;
  let current: SuggestionProps<SlashItem, SlashItem> | null = null;

  const place = (props: SuggestionProps<SlashItem, SlashItem>) => {
    const rect = props.clientRect?.();
    if (el && rect) {
      el.style.left = `${rect.left}px`;
      el.style.top = `${rect.bottom + 6}px`;
    }
  };

  const draw = () => {
    if (!el || !current) return;
    el.replaceChildren();
    if (current.items.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'slash-empty';
      empty.textContent = 'No matching block';
      el.append(empty);
      return;
    }
    current.items.forEach((item, index) => {
      const option = document.createElement('button');
      option.type = 'button';
      option.setAttribute('role', 'option');
      option.setAttribute('aria-selected', String(index === selected));
      option.className = index === selected ? 'slash-item is-selected' : 'slash-item';
      option.textContent = item.title;
      option.addEventListener('mousedown', (event) => {
        event.preventDefault();
        current?.command(item);
      });
      el!.append(option);
    });
  };

  return {
    onStart(props: SuggestionProps<SlashItem, SlashItem>) {
      current = props;
      selected = 0;
      el = document.createElement('div');
      el.className = 'slash-menu';
      el.setAttribute('role', 'listbox');
      el.setAttribute('aria-label', 'Block commands');
      document.body.append(el);
      place(props);
      draw();
    },
    onUpdate(props: SuggestionProps<SlashItem, SlashItem>) {
      current = props;
      selected = Math.min(selected, Math.max(0, props.items.length - 1));
      place(props);
      draw();
    },
    onKeyDown({ event }: { event: KeyboardEvent }) {
      if (!current) return false;
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        const step = event.key === 'ArrowDown' ? 1 : -1;
        const count = current.items.length;
        if (count > 0) selected = (selected + step + count) % count;
        draw();
        return true;
      }
      if (event.key === 'Enter') {
        const item = current.items[selected];
        if (item) current.command(item);
        return true;
      }
      return false;
    },
    onExit() {
      el?.remove();
      el = null;
      current = null;
    },
  };
}

export const SlashCommands = Extension.create({
  name: 'slashCommands',
  addProseMirrorPlugins() {
    return [
      Suggestion<SlashItem, SlashItem>({
        editor: this.editor,
        char: '/',
        allowSpaces: false,
        command: ({ editor, range, props }: { editor: Editor; range: Range; props: SlashItem }) => {
          editor.chain().focus().deleteRange(range).run();
          props.run(editor);
        },
        items: ({ query }: { query: string }) => filterSlashItems(query),
        render: renderMenu,
      }),
    ];
  },
});
