import { useEffect, useMemo, useRef, useState } from 'react';

export interface PaletteAction {
  id: string;
  label: string;
  run: () => void;
}

interface Props {
  open: boolean;
  actions: PaletteAction[];
  onClose: () => void;
}

/** Ctrl+K palette: open pages, switch views, and run quick actions by typing a few letters. */
export function CommandPalette({ open, actions, onClose }: Props) {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const matches = useMemo(() => {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    return actions.filter((a) => terms.every((t) => a.label.toLowerCase().includes(t))).slice(0, 30);
  }, [actions, query]);

  useEffect(() => {
    if (open) {
      setQuery('');
      setSelected(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    setSelected((index) => Math.min(index, Math.max(0, matches.length - 1)));
  }, [matches.length]);

  if (!open) return null;

  const choose = (index: number) => {
    const action = matches[index];
    if (!action) return;
    onClose();
    action.run();
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      setSelected((i) => (matches.length ? (i + 1) % matches.length : 0));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setSelected((i) => (matches.length ? (i - 1 + matches.length) % matches.length : 0));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      choose(selected);
    }
  };

  return (
    <div className="palette-backdrop" onMouseDown={onClose}>
      <div
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-results"
          aria-activedescendant={matches[selected] ? `palette-${matches[selected].id}` : undefined}
          aria-label="Type a page or command"
          placeholder="Type a page name or command…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />
        <ul id="palette-results" role="listbox" className="palette-results">
          {matches.length === 0 && <li className="muted palette-empty">Nothing matches.</li>}
          {matches.map((action, index) => (
            <li
              key={action.id}
              id={`palette-${action.id}`}
              role="option"
              aria-selected={index === selected}
              className={index === selected ? 'palette-item is-selected' : 'palette-item'}
              onMouseEnter={() => setSelected(index)}
              onMouseDown={(e) => {
                e.preventDefault();
                choose(index);
              }}
            >
              {action.label}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
