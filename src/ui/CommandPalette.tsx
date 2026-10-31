import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { nextPaletteIndex, searchPaletteItems, type PaletteItem } from '../core/command-palette';
import { Kbd } from './DesignSystem';
import { Modal } from './Modal';

export function CommandPalette({
  open,
  items,
  recentIds,
  onClose,
  onExecute,
}: {
  open: boolean;
  items: readonly PaletteItem[];
  recentIds: readonly string[];
  onClose: () => void;
  onExecute: (item: PaletteItem) => void;
}) {
  const inputId = useId();
  const listId = useId();
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const results = useMemo(
    () => searchPaletteItems(items, query, recentIds),
    [items, query, recentIds],
  );

  useEffect(() => {
    if (!open) {
      setQuery('');
      setActiveIndex(0);
    }
  }, [open]);
  useEffect(() => setActiveIndex(0), [query]);

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const next = nextPaletteIndex(event.key, activeIndex, results.length);
      if (next !== null) setActiveIndex(next);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const selected = results[activeIndex];
      if (selected) onExecute(selected);
    }
  }

  return (
    <Modal open={open} title="Paleta de comandos" onClose={onClose}>
      <div className="command-palette">
        <label className="sr-only" htmlFor={inputId}>
          Buscar comandos
        </label>
        <input
          ref={inputRef}
          id={inputId}
          data-autofocus
          className="command-palette-input"
          type="text"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls={listId}
          aria-activedescendant={
            results[activeIndex] ? `${listId}-option-${activeIndex}` : undefined
          }
          placeholder="Buscar páginas, conversas e ações…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={handleKeyDown}
        />
        <ul
          className="command-palette-list"
          id={listId}
          role="listbox"
          aria-label="Resultados de comandos"
        >
          {results.map((item, index) => (
            <li
              id={`${listId}-option-${index}`}
              key={item.id}
              role="option"
              aria-selected={index === activeIndex}
              className={index === activeIndex ? 'active' : ''}
              onMouseEnter={() => setActiveIndex(index)}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => onExecute(item)}
            >
              <span>{item.title}</span>
              {item.shortcut && <Kbd>{item.shortcut}</Kbd>}
            </li>
          ))}
          {results.length === 0 && (
            <li className="command-palette-empty" role="presentation">
              Nenhum comando encontrado.
            </li>
          )}
        </ul>
      </div>
    </Modal>
  );
}
