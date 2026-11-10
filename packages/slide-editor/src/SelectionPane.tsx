import { useTranslation } from '@blueoffice/i18n';
import type { SlideCommand } from '@blueoffice/platform';
import { ArrowDown, ArrowUp } from 'lucide-react';
import styles from './SlideEditor.module.css';

export interface SelectionPaneProps {
  /** Top-level objects of the slide in z-order (back first, ECMA-376 Part 1, §19.3.1.45 spTree). */
  objects: { id: number; name: string }[];
  /** Ids of the selected objects. */
  selected: number[];
  run: (cmd: SlideCommand) => void;
}

/** Ids after a click on `id` (Ctrl / ⌘ adds or removes it, else it alone). */
export function selectionAfterClick(selected: number[], id: number, extend: boolean): number[] {
  if (!extend) return [id];
  return selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id];
}

/**
 * Selection task pane of BlueSlide (CORE-UI-006): the slide's objects, front
 * first; click selects (Ctrl+click adds), arrows move the selection one layer.
 */
export function SelectionPane({ objects, selected, run }: SelectionPaneProps) {
  const { t } = useTranslation();
  const list = [...objects].reverse();
  const any = selected.length > 0;
  return (
    <div className={styles.selPane} data-testid="selection-pane">
      <div className={styles.selTools}>
        <button
          type="button"
          disabled={!any}
          title={t('taskPane.selection.forward')}
          aria-label={t('taskPane.selection.forward')}
          data-testid="selection-forward"
          onClick={() => run({ cmd: 'zOrder', to: 'forward' })}
        >
          <ArrowUp size={14} aria-hidden />
        </button>
        <button
          type="button"
          disabled={!any}
          title={t('taskPane.selection.backward')}
          aria-label={t('taskPane.selection.backward')}
          data-testid="selection-backward"
          onClick={() => run({ cmd: 'zOrder', to: 'backward' })}
        >
          <ArrowDown size={14} aria-hidden />
        </button>
      </div>
      {list.length === 0 ? (
        <p className={styles.selNote}>{t('taskPane.selection.empty')}</p>
      ) : (
        <>
          <p className={styles.selNote}>{t('taskPane.selection.hint')}</p>
          <ul
            className={styles.selList}
            role="listbox"
            aria-multiselectable
            aria-label={t('taskPane.selection.title')}
          >
            {list.map((o) => (
              <li
                key={o.id}
                role="option"
                aria-selected={selected.includes(o.id)}
                tabIndex={0}
                className={styles.selItem}
                data-testid="selection-item"
                data-shape-id={o.id}
                onClick={(e) =>
                  run({
                    cmd: 'select',
                    ids: selectionAfterClick(selected, o.id, e.ctrlKey || e.metaKey),
                  })
                }
                onKeyDown={(e) => {
                  if (e.key !== 'Enter' && e.key !== ' ') return;
                  e.preventDefault();
                  e.stopPropagation();
                  run({
                    cmd: 'select',
                    ids: selectionAfterClick(selected, o.id, e.ctrlKey || e.metaKey),
                  });
                }}
              >
                {o.name || `#${o.id}`}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
