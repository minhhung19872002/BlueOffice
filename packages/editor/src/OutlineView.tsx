import type { EditCommand, EditResult, OutlineItem } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import { useMemo, useState, type DragEvent, type KeyboardEvent } from 'react';
import { hasChildren, itemAt, levelOf, visibleItems } from './outlineModel';
import styles from './Views.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

export interface OutlineViewProps {
  /** Outline of the document (null while it loads). */
  items: OutlineItem[] | null;
  /** Paragraph of the caret. */
  caretPara: number | undefined;
  run: Run;
  /** Run an editor command (`outline.promote`, `outline.moveUp`, …). */
  onCommand: (id: string) => void;
  /** Leave the outline view (to the print layout), optionally at a paragraph. */
  onClose: (para?: number) => void;
}

const DRAG_TYPE = 'application/x-bluedoc-outline';

/** Start a drag of an outline paragraph (shared with the navigation pane). */
export function startOutlineDrag(e: DragEvent, para: number): void {
  e.dataTransfer.setData(DRAG_TYPE, String(para));
  e.dataTransfer.setData('text/plain', String(para));
  e.dataTransfer.effectAllowed = 'move';
}

/** Paragraph id carried by an outline drag, if any. */
export function outlineDragPara(e: DragEvent): number | null {
  const raw = e.dataTransfer.getData(DRAG_TYPE) || e.dataTransfer.getData('text/plain');
  const n = Number.parseInt(raw, 10);
  return Number.isFinite(n) ? n : null;
}

/**
 * Outline view: the document's paragraphs by outline level. Headings expand and
 * collapse, Tab / Shift+Tab lower / raise a level, Alt+Shift+↑/↓ (editor commands)
 * move a heading with its subordinate text, and rows can be dragged to a new place.
 */
export function OutlineView({ items, caretPara, run, onCommand, onClose }: OutlineViewProps) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set());
  const [showLevel, setShowLevel] = useState<number | null>(null);
  const [dropOn, setDropOn] = useState<number | 'end' | null>(null);
  const all = useMemo(() => items ?? [], [items]);
  const shown = useMemo(() => visibleItems(all, collapsed, showLevel), [all, collapsed, showLevel]);
  const current = itemAt(all, caretPara)?.para;

  const select = (para: number) =>
    void run({
      cmd: 'setSelection',
      selection: { anchor: { p: para, o: 0 }, focus: { p: para, o: 0 } },
    });
  const toggle = (para: number) =>
    setCollapsed((c) => {
      const n = new Set(c);
      if (n.has(para)) n.delete(para);
      else n.add(para);
      return n;
    });

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    if (e.key === 'Tab') {
      e.preventDefault();
      onCommand(e.shiftKey ? 'outline.promote' : 'outline.demote');
      return;
    }
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      const i = shown.findIndex((x) => x.para === current);
      const next =
        shown[e.key === 'ArrowUp' ? Math.max(0, i - 1) : Math.min(shown.length - 1, i + 1)];
      if (next) select(next.para);
      return;
    }
    if (e.key === 'Enter' && current !== undefined) {
      e.preventDefault();
      onClose(current);
    }
  };

  const drop = (e: DragEvent, before: number | null) => {
    e.preventDefault();
    setDropOn(null);
    const para = outlineDragPara(e);
    if (para === null || para === before) return;
    void run({ cmd: 'moveOutlineTo', para, before });
  };

  return (
    <section
      className={styles.outline}
      aria-label={t('views.outline.title')}
      data-testid="outline-view"
    >
      <div className={styles.outlineBar} role="toolbar">
        {(
          [
            ['outline.promote', 'commands.outline.promote', '⇤'],
            ['outline.demote', 'commands.outline.demote', '⇥'],
            ['outline.bodyText', 'commands.outline.bodyText', '¶'],
            ['outline.moveUp', 'commands.outline.moveUp', '↑'],
            ['outline.moveDown', 'commands.outline.moveDown', '↓'],
          ] as const
        ).map(([id, key, glyph]) => (
          <button
            key={id}
            type="button"
            title={t(key)}
            aria-label={t(key)}
            data-testid={id}
            onClick={() => onCommand(id)}
          >
            <span aria-hidden>{glyph}</span> {t(key)}
          </button>
        ))}
        <label>
          {t('views.outline.showLevel')}{' '}
          <select
            data-testid="outline-show-level"
            value={showLevel ?? ''}
            onChange={(e) => setShowLevel(e.target.value ? Number(e.target.value) : null)}
          >
            <option value="">{t('views.outline.allLevels')}</option>
            {Array.from({ length: 9 }, (_, i) => (
              <option key={i} value={i + 1}>
                {t('views.outline.upTo', { n: i + 1 })}
              </option>
            ))}
          </select>
        </label>
        <span className={styles.spacer} />
        <button type="button" data-testid="outline-close" onClick={() => onClose()}>
          {t('views.outline.close')}
        </button>
      </div>
      <div
        className={styles.outlineList}
        role="tree"
        tabIndex={0}
        aria-label={t('views.outline.title')}
        onKeyDown={onKeyDown}
        data-testid="outline-list"
      >
        {all.length === 0 && <p className={styles.empty}>{t('views.outline.empty')}</p>}
        {shown.map((it) => {
          const index = all.indexOf(it);
          const parent = hasChildren(all, index);
          // Collapsed by the user or by the "show level" filter.
          const open =
            !collapsed.has(it.para) && (showLevel === null || levelOf(it) + 1 < showLevel);
          const level = levelOf(it);
          return (
            <div
              key={it.para}
              role="treeitem"
              aria-level={level + 1}
              aria-selected={it.para === current}
              aria-expanded={parent ? open : undefined}
              data-testid="outline-item"
              data-para={it.para}
              data-level={it.level ?? 'body'}
              className={`${styles.outlineItem} ${it.level === null ? styles.body : styles.heading} ${
                it.para === current ? styles.selected : ''
              } ${dropOn === it.para ? styles.dropBefore : ''}`}
              style={{
                paddingLeft:
                  8 + (it.level === null ? prevHeadingLevel(all, index) + 1 : level) * 22,
              }}
              draggable
              onDragStart={(e) => startOutlineDrag(e, it.para)}
              onDragOver={(e) => {
                e.preventDefault();
                setDropOn(it.para);
              }}
              onDragLeave={() => setDropOn((d) => (d === it.para ? null : d))}
              onDrop={(e) => drop(e, it.para)}
              onClick={() => select(it.para)}
              onDoubleClick={() => onClose(it.para)}
            >
              <button
                type="button"
                className={styles.bullet}
                tabIndex={-1}
                aria-label={
                  parent ? t(open ? 'views.nav.collapse' : 'views.nav.expand') : undefined
                }
                aria-hidden={!parent}
                onClick={(e) => {
                  e.stopPropagation();
                  if (parent) toggle(it.para);
                }}
              >
                {parent ? (open ? '⊖' : '⊕') : it.level === null ? '▫' : '•'}
              </button>
              <span
                className={styles.outlineText}
                style={
                  it.level === null ? undefined : { fontSize: `${Math.max(13, 20 - level * 2)}px` }
                }
                title={
                  it.level === null
                    ? t('views.outline.bodyText')
                    : t('views.outline.level', { n: level + 1 })
                }
              >
                {it.text || ' '}
              </span>
            </div>
          );
        })}
        <div
          className={`${styles.dropEnd} ${dropOn === 'end' ? styles.dropBefore : ''}`}
          data-testid="outline-drop-end"
          onDragOver={(e) => {
            e.preventDefault();
            setDropOn('end');
          }}
          onDragLeave={() => setDropOn(null)}
          onDrop={(e) => drop(e, null)}
        >
          {t('views.outline.dropEnd')}
        </div>
      </div>
    </section>
  );
}

/** Level of the heading above item `index` (-1 when none): body text is indented under it. */
function prevHeadingLevel(items: OutlineItem[], index: number): number {
  for (let i = index - 1; i >= 0; i--) {
    const l = items[i]?.level;
    if (l !== null && l !== undefined) return l;
  }
  return -1;
}
