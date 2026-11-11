import { useTranslation } from '@blueoffice/i18n';
import type {
  SlideSmartArtCategory,
  SlideSmartArtColors,
  SlideSmartArtItem,
  SlideSmartArtLayout,
  SlideSmartArtStyle,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useLayoutEffect, useRef, useState } from 'react';
import styles from './SmartArtDialog.module.css';

/** The BlueOffice layout library by category (`bo_pptx::diagram_layout::Layout::ALL`). */
export const SMARTART_LIBRARY: {
  category: SlideSmartArtCategory;
  layouts: SlideSmartArtLayout[];
}[] = [
  { category: 'list', layouts: ['list', 'listVertical'] },
  { category: 'process', layouts: ['process', 'processChevron', 'processVertical'] },
  { category: 'cycle', layouts: ['cycle', 'cycleRadial'] },
  { category: 'hierarchy', layouts: ['hierarchy', 'hierarchyHorizontal'] },
  { category: 'matrix', layouts: ['matrix', 'matrixCentered'] },
  { category: 'pyramid', layouts: ['pyramid', 'pyramidInverted'] },
  { category: 'relationship', layouts: ['relationship', 'relationshipTarget'] },
  { category: 'picture', layouts: ['picture', 'pictureStrip'] },
];

export const SMARTART_LAYOUTS: SlideSmartArtLayout[] = SMARTART_LIBRARY.flatMap((g) => g.layouts);

const SMARTART_COLORS: SlideSmartArtColors[] = [
  'colorful',
  'blue',
  'green',
  'orange',
  'purple',
  'gray',
];

/** Outline text → items: one node per line, each leading tab (or two spaces) one level deeper. */
export function itemsOf(text: string): SlideSmartArtItem[] {
  return text
    .split(/\r?\n/)
    .map((line) => {
      const lead = /^[\t ]*/.exec(line)?.[0] ?? '';
      const level = [...lead].reduce((n, c) => n + (c === '\t' ? 2 : 1), 0) >> 1;
      return { text: line.trim(), level };
    })
    .filter((i) => i.text.length > 0);
}

/** Items → outline text (inverse of {@link itemsOf}). */
export function textOf(items: SlideSmartArtItem[]): string {
  return items.map((i) => '\t'.repeat(i.level) + i.text).join('\n');
}

/** What the node buttons of the text pane do to the node at the caret. */
export type SmartArtNodeOp = 'add' | 'remove' | 'promote' | 'demote' | 'up' | 'down';

export const SMARTART_NODE_OPS: SmartArtNodeOp[] = [
  'add',
  'remove',
  'promote',
  'demote',
  'up',
  'down',
];

/** Levels as the engine reads them: the first node at the top, no node more than one level deeper. */
function clampLevels(items: SlideSmartArtItem[]): SlideSmartArtItem[] {
  let prev = -1;
  return items.map((i) => {
    const level = Math.min(i.level, prev + 1);
    prev = level;
    return { text: i.text, level };
  });
}

/** The index after node `i` and its sub-nodes. */
function subtreeEnd(items: SlideSmartArtItem[], i: number): number {
  const level = items[i]?.level ?? 0;
  let j = i + 1;
  while (j < items.length && (items[j]?.level ?? 0) > level) j++;
  return j;
}

/**
 * Apply a node operation to the outline at node `index`; returns the new outline and the node the
 * caret goes to. A node moves, is promoted or demoted together with its sub-nodes; removing a node
 * promotes its sub-nodes; `add` puts a node named `newText` after the node and its sub-nodes at the
 * same level. An operation that does not apply (promoting a top node, demoting a first node, moving
 * past the first or last sibling) leaves the outline as it is.
 */
export function editOutline(
  items: SlideSmartArtItem[],
  index: number,
  op: SmartArtNodeOp,
  newText = '',
): { items: SlideSmartArtItem[]; index: number } {
  const src = clampLevels(items);
  const cur = src[index];
  if (!cur) {
    return op === 'add'
      ? { items: [...src, { text: newText, level: 0 }], index: src.length }
      : { items: src, index: Math.max(0, Math.min(index, src.length - 1)) };
  }
  const end = subtreeEnd(src, index);
  const shift = (by: number) =>
    src.map((it, k) => (k >= index && k < end ? { text: it.text, level: it.level + by } : it));
  switch (op) {
    case 'add':
      return {
        items: [...src.slice(0, end), { text: newText, level: cur.level }, ...src.slice(end)],
        index: end,
      };
    case 'remove': {
      const kept = src
        .map((it, k) => (k > index && k < end ? { text: it.text, level: it.level - 1 } : it))
        .filter((_, k) => k !== index);
      return { items: clampLevels(kept), index: Math.max(0, Math.min(index, kept.length - 1)) };
    }
    case 'promote':
      return cur.level === 0 ? { items: src, index } : { items: shift(-1), index };
    case 'demote': {
      const prev = src[index - 1];
      return !prev || cur.level > prev.level ? { items: src, index } : { items: shift(1), index };
    }
    case 'up': {
      let p = index - 1;
      while (p >= 0 && (src[p]?.level ?? 0) > cur.level) p--;
      if (p < 0 || src[p]?.level !== cur.level) return { items: src, index };
      return {
        items: [
          ...src.slice(0, p),
          ...src.slice(index, end),
          ...src.slice(p, index),
          ...src.slice(end),
        ],
        index: p,
      };
    }
    case 'down': {
      if (src[end]?.level !== cur.level) return { items: src, index };
      const nextEnd = subtreeEnd(src, end);
      return {
        items: [
          ...src.slice(0, index),
          ...src.slice(end, nextEnd),
          ...src.slice(index, end),
          ...src.slice(nextEnd),
        ],
        index: index + (nextEnd - end),
      };
    }
  }
}

/** The node the caret at `offset` of the outline text is in (blank lines belong to the node above). */
export function nodeAtCaret(text: string, offset: number): number {
  const before = text.slice(0, offset).split('\n');
  const line = before.length - 1;
  const lines = text.split('\n');
  const filled = lines.slice(0, line + 1).filter((l) => l.trim().length > 0).length;
  return Math.max(0, filled - 1);
}

export interface SmartArtDraft {
  layout: SlideSmartArtLayout;
  items: SlideSmartArtItem[];
  colors: SlideSmartArtColors;
  style: SlideSmartArtStyle;
}

/**
 * Insert a SmartArt or edit the selected one: the layout library by category, colours, look and
 * the text pane (an indented outline with node buttons: add, remove, promote, demote, move).
 * Shared by BlueSlide, BlueDoc and BlueSheet (CORE-SMART-003/004/005).
 */
export function SmartArtDialog({
  mode,
  initial,
  onApply,
  onClose,
}: {
  mode: 'insert' | 'edit';
  initial: SmartArtDraft;
  onApply: (d: SmartArtDraft) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [layout, setLayout] = useState<SlideSmartArtLayout>(initial.layout);
  const [text, setText] = useState(textOf(initial.items));
  const [colors, setColors] = useState<SlideSmartArtColors>(initial.colors);
  const [style, setStyle] = useState<SlideSmartArtStyle>(initial.style);
  const pane = useRef<HTMLTextAreaElement>(null);
  /** Caret to restore after a node button rewrote the outline. */
  const caret = useRef<number | null>(null);
  useLayoutEffect(() => {
    const el = pane.current;
    if (el && caret.current !== null) {
      el.focus();
      el.setSelectionRange(caret.current, caret.current);
      caret.current = null;
    }
  }, [text]);
  const items = itemsOf(text);
  const nodeOp = (op: SmartArtNodeOp) => {
    const at = nodeAtCaret(text, pane.current?.selectionStart ?? text.length);
    const r = editOutline(
      items,
      at,
      op,
      t('slideEditor.smartart.sampleItem', { n: items.length + 1 }),
    );
    const next = textOf(r.items);
    // The caret goes to the end of the node's line.
    caret.current = next
      .split('\n')
      .slice(0, r.index + 1)
      .join('\n').length;
    if (next === text) {
      const el = pane.current;
      el?.focus();
      el?.setSelectionRange(caret.current, caret.current);
      caret.current = null;
    } else setText(next);
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(
        mode === 'insert' ? 'slideEditor.smartart.insertTitle' : 'slideEditor.smartart.editTitle',
      )}
      width={640}
      testId="smartart-dialog"
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="smartart-ok"
            disabled={items.length === 0}
            onClick={() => onApply({ layout, items, colors, style })}
          >
            {t(mode === 'insert' ? 'slideEditor.smartart.insert' : 'slideEditor.smartart.apply')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <fieldset className={styles.library}>
          <legend>{t('slideEditor.smartart.layout')}</legend>
          {SMARTART_LIBRARY.map((g) => (
            <div
              key={g.category}
              role="radiogroup"
              aria-label={t(`slideEditor.smartart.categories.${g.category}`)}
              className={styles.category}
              data-testid={`smartart-category-${g.category}`}
            >
              <span className={styles.categoryName}>
                {t(`slideEditor.smartart.categories.${g.category}`)}
              </span>
              {g.layouts.map((l) => (
                <label key={l} className={styles.layout}>
                  <input
                    type="radio"
                    name="smartart-layout"
                    data-testid={`smartart-layout-${l}`}
                    checked={layout === l}
                    onChange={() => setLayout(l)}
                  />{' '}
                  {t(`slideEditor.smartart.layouts.${l}`)}
                </label>
              ))}
            </div>
          ))}
        </fieldset>
        <label>
          {t('slideEditor.smartart.colors')}{' '}
          <select
            data-testid="smartart-colors"
            value={colors}
            onChange={(e) => setColors(e.target.value as SlideSmartArtColors)}
          >
            {SMARTART_COLORS.map((c) => (
              <option key={c} value={c}>
                {t(`slideEditor.smartart.palette.${c}`)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('slideEditor.smartart.style')}{' '}
          <select
            data-testid="smartart-style"
            value={style}
            onChange={(e) => setStyle(e.target.value as SlideSmartArtStyle)}
          >
            <option value="fill">{t('slideEditor.smartart.styles.fill')}</option>
            <option value="outline">{t('slideEditor.smartart.styles.outline')}</option>
          </select>
        </label>
        <div
          className={styles.nodeBar}
          role="toolbar"
          aria-label={t('slideEditor.smartart.nodeTools')}
        >
          {SMARTART_NODE_OPS.map((op) => (
            <button
              key={op}
              type="button"
              data-testid={`smartart-node-${op}`}
              // Keep the caret in the text pane: it names the node the button acts on.
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => nodeOp(op)}
            >
              {t(`slideEditor.smartart.node.${op}`)}
            </button>
          ))}
        </div>
        <label className={styles.pane}>
          {t('slideEditor.smartart.textPane')}
          <textarea
            ref={pane}
            data-testid="smartart-text"
            rows={7}
            value={text}
            spellCheck={false}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              // Tab / Shift+Tab demote / promote the current line.
              if (e.key !== 'Tab') return;
              e.preventDefault();
              const el = e.currentTarget;
              const start = text.lastIndexOf('\n', el.selectionStart - 1) + 1;
              const next = e.shiftKey
                ? text.slice(0, start) + text.slice(start).replace(/^\t/, '')
                : `${text.slice(0, start)}\t${text.slice(start)}`;
              setText(next);
            }}
            style={{ width: '100%', font: '14px system-ui, sans-serif', tabSize: 2 }}
          />
        </label>
        <div style={{ fontSize: 12, opacity: 0.75 }}>{t('slideEditor.smartart.hint')}</div>
      </div>
    </Dialog>
  );
}
