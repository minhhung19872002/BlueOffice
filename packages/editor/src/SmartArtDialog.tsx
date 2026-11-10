import { useTranslation } from '@blueoffice/i18n';
import type {
  SlideSmartArtColors,
  SlideSmartArtItem,
  SlideSmartArtLayout,
  SlideSmartArtStyle,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './SmartArtDialog.module.css';

export const SMARTART_LAYOUTS: SlideSmartArtLayout[] = [
  'list',
  'process',
  'cycle',
  'hierarchy',
  'matrix',
  'pyramid',
  'relationship',
];

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

export interface SmartArtDraft {
  layout: SlideSmartArtLayout;
  items: SlideSmartArtItem[];
  colors: SlideSmartArtColors;
  style: SlideSmartArtStyle;
}

/**
 * Insert a SmartArt or edit the selected one: layout and the text pane (an indented outline).
 * Shared by BlueSlide, BlueDoc and BlueSheet (CORE-SMART-003/005).
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
  const items = itemsOf(text);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(
        mode === 'insert' ? 'slideEditor.smartart.insertTitle' : 'slideEditor.smartart.editTitle',
      )}
      width={560}
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
        <fieldset>
          <legend>{t('slideEditor.smartart.layout')}</legend>
          {SMARTART_LAYOUTS.map((l) => (
            <label key={l}>
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
        <label>
          {t('slideEditor.smartart.textPane')}
          <textarea
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
