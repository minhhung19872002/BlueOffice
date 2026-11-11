import { useTranslation } from '@blueoffice/i18n';
import type {
  FileAccess,
  SlideSmartArtCategory,
  SlideSmartArtColors,
  SlideSmartArtItem,
  SlideSmartArtLayout,
  SlideSmartArtStyle,
  SmartArtPreview,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
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

/**
 * Thumbnails of the layouts by colours and look. The engine draws the same thumbnails in every
 * app, so one cache serves BlueDoc, BlueSheet and BlueSlide for the life of the page.
 */
const previewCache = new Map<string, Promise<SmartArtPreview[]>>();

/** Loads the layout thumbnails for colours `colors` and look `style` (from the engine). */
export type SmartArtPreviewLoader = (
  colors: SlideSmartArtColors,
  style: SlideSmartArtStyle,
) => Promise<SmartArtPreview[]>;

/** The cached thumbnails for `colors` / `style` (a failed load is not kept). */
export function smartArtPreviews(
  load: SmartArtPreviewLoader,
  colors: SlideSmartArtColors,
  style: SlideSmartArtStyle,
): Promise<SmartArtPreview[]> {
  // A diagram's own colours / look are in its file: the library is shown in the defaults.
  const c = colors === 'file' ? 'colorful' : colors;
  const s = style === 'file' ? 'fill' : style;
  const key = `${c}|${s}`;
  let hit = previewCache.get(key);
  if (!hit) {
    hit = load(c, s).catch((e: unknown) => {
      previewCache.delete(key);
      throw e;
    });
    previewCache.set(key, hit);
  }
  return hit;
}

/** Forget the cached thumbnails (tests). */
export function clearSmartArtPreviews(): void {
  previewCache.clear();
}

/**
 * `pickPicture` of the dialog: a PNG / JPEG file chosen through the platform's file dialog, as a
 * `data:` URL (the engine reads the format from the bytes).
 */
export function smartArtPicturePicker(files: FileAccess): () => Promise<string | null> {
  return async () => {
    const file = await files.openFile(['.png', '.jpg', '.jpeg']);
    if (!file) return null;
    let bin = '';
    const bytes = file.bytes;
    for (let i = 0; i < bytes.length; i += 0x8000) {
      bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    }
    const mime = /\.png$/i.test(file.name) ? 'image/png' : 'image/jpeg';
    return `data:${mime};base64,${btoa(bin)}`;
  };
}

/** An SVG document as an image source. */
function svgSrc(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/**
 * The pictures of the nodes after the outline was retyped: unchanged when the number of nodes is
 * the same (texts were edited), else each node keeps the picture of the first old node with its
 * text (lines were added or removed).
 */
export function realignPictures(
  before: SlideSmartArtItem[],
  pictures: (string | undefined)[],
  after: SlideSmartArtItem[],
): (string | undefined)[] {
  if (before.length === after.length) return after.map((_, i) => pictures[i]);
  const used = new Set<number>();
  return after.map((it) => {
    const k = before.findIndex((b, i) => !used.has(i) && pictures[i] && b.text === it.text);
    if (k < 0) return undefined;
    used.add(k);
    return pictures[k];
  });
}

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
    return { ...i, level };
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
    src.map((it, k) => (k >= index && k < end ? { ...it, level: it.level + by } : it));
  switch (op) {
    case 'add':
      return {
        items: [...src.slice(0, end), { text: newText, level: cur.level }, ...src.slice(end)],
        index: end,
      };
    case 'remove': {
      const kept = src
        .map((it, k) => (k > index && k < end ? { ...it, level: it.level - 1 } : it))
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
 * Insert a SmartArt or edit the selected one: the layout library by category (with thumbnails
 * drawn by the engine), colours, look and the text pane (an indented outline with node buttons:
 * add, remove, promote, demote, move, and the node's picture: choose / replace, remove).
 * Shared by BlueSlide, BlueDoc and BlueSheet (CORE-SMART-003/004/005).
 */
export function SmartArtDialog({
  mode,
  initial,
  onApply,
  onClose,
  loadPreviews,
  pickPicture,
}: {
  mode: 'insert' | 'edit';
  initial: SmartArtDraft;
  onApply: (d: SmartArtDraft) => void;
  onClose: () => void;
  /** Layout thumbnails from the engine (none: the library shows names only). */
  loadPreviews?: SmartArtPreviewLoader;
  /** Lets the user choose a PNG / JPEG picture; resolves its `data:` URL (`null`: cancelled). */
  pickPicture?: () => Promise<string | null>;
}) {
  const { t } = useTranslation();
  const [layout, setLayout] = useState<SlideSmartArtLayout>(initial.layout);
  const [text, setTextOnly] = useState(textOf(initial.items));
  /** The picture of each node (index in the outline): an id the diagram has, or a new `data:` URL. */
  const [pictures, setPictures] = useState<(string | undefined)[]>(
    initial.items.map((i) => i.picture),
  );
  const [colors, setColors] = useState<SlideSmartArtColors>(initial.colors);
  const [style, setStyle] = useState<SlideSmartArtStyle>(initial.style);
  const [previews, setPreviews] = useState<Map<SlideSmartArtLayout, string>>(new Map());
  /** The node the caret of the text pane is in. */
  const [caretNode, setCaretNode] = useState(0);
  useEffect(() => {
    if (!loadPreviews) return;
    let live = true;
    smartArtPreviews(loadPreviews, colors, style).then(
      (list) => {
        if (live) setPreviews(new Map(list.map((p) => [p.layout, p.svg])));
      },
      // No thumbnails: the library still lists every layout by name.
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [loadPreviews, colors, style]);
  /** The outline was retyped: the pictures follow their nodes. */
  const setText = (next: string) => {
    setPictures((p) => realignPictures(itemsOf(text), p, itemsOf(next)));
    setTextOnly(next);
  };
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
  const items: SlideSmartArtItem[] = itemsOf(text).map((it, i) =>
    pictures[i] ? { ...it, picture: pictures[i] } : it,
  );
  const caretAt = () => nodeAtCaret(text, pane.current?.selectionStart ?? text.length);
  const setPicture = (node: number, picture: string | undefined) =>
    setPictures((p) => {
      const next = items.map((_, i) => p[i]);
      next[node] = picture;
      return next;
    });
  const choosePicture = async () => {
    const node = caretAt();
    const url = await pickPicture?.();
    if (url) setPicture(node, url);
    pane.current?.focus();
  };
  const nodeOp = (op: SmartArtNodeOp) => {
    const at = caretAt();
    const r = editOutline(
      items,
      at,
      op,
      t('slideEditor.smartart.sampleItem', { n: items.length + 1 }),
    );
    const next = textOf(r.items);
    setCaretNode(r.index);
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
    } else {
      // The nodes moved with their pictures.
      setPictures(r.items.map((i) => i.picture));
      setTextOnly(next);
    }
  };
  const pictured = items.map((it, i) => ({ it, i })).filter((x) => !!x.it.picture);
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
              {g.layouts.map((l) => {
                const svg = previews.get(l);
                return (
                  <label key={l} className={styles.layout}>
                    <input
                      type="radio"
                      name="smartart-layout"
                      data-testid={`smartart-layout-${l}`}
                      checked={layout === l}
                      onChange={() => setLayout(l)}
                    />{' '}
                    {svg && (
                      <img
                        className={styles.thumb}
                        data-testid={`smartart-preview-${l}`}
                        src={svgSrc(svg)}
                        alt=""
                      />
                    )}
                    {t(`slideEditor.smartart.layouts.${l}`)}
                  </label>
                );
              })}
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
            {initial.colors === 'file' && (
              <option value="file">{t('slideEditor.smartart.palette.file')}</option>
            )}
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
            {initial.style === 'file' && (
              <option value="file">{t('slideEditor.smartart.styles.file')}</option>
            )}
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
          {pickPicture && (
            <>
              <button
                type="button"
                data-testid="smartart-node-picture"
                disabled={items.length === 0}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => void choosePicture()}
              >
                {t(
                  pictures[caretNode]
                    ? 'slideEditor.smartart.node.replacePicture'
                    : 'slideEditor.smartart.node.picture',
                )}
              </button>
              <button
                type="button"
                data-testid="smartart-node-removePicture"
                disabled={!pictures[caretNode]}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => setPicture(caretAt(), undefined)}
              >
                {t('slideEditor.smartart.node.removePicture')}
              </button>
            </>
          )}
        </div>
        {pictured.length > 0 && (
          <ul
            className={styles.pictures}
            data-testid="smartart-pictures"
            aria-label={t('slideEditor.smartart.pictures')}
          >
            {pictured.map(({ it, i }) => (
              <li key={i} data-node={i}>
                {it.picture?.startsWith('data:') && (
                  <img className={styles.nodePicture} src={it.picture} alt="" />
                )}
                {it.text}
              </li>
            ))}
          </ul>
        )}
        <label className={styles.pane}>
          {t('slideEditor.smartart.textPane')}
          <textarea
            ref={pane}
            data-testid="smartart-text"
            rows={7}
            value={text}
            spellCheck={false}
            onChange={(e) => {
              setText(e.target.value);
              setCaretNode(nodeAtCaret(e.target.value, e.target.selectionStart));
            }}
            onSelect={(e) =>
              setCaretNode(nodeAtCaret(text, e.currentTarget.selectionStart ?? text.length))
            }
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
        <div style={{ fontSize: 12, opacity: 0.75 }}>
          {t('slideEditor.smartart.hint')}
          {pickPicture && ` ${t('slideEditor.smartart.pictureHint')}`}
        </div>
      </div>
    </Dialog>
  );
}
