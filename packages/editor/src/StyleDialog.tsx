/**
 * Modify / New Style dialog (DOC-STYLE-001, ECMA-376 Part 1, §17.7.4.17 style): name,
 * type, based on, next style, font, size, bold, italic, underline, colour, alignment,
 * spacing, indents, border, shading, numbering, "add to gallery" and "update
 * automatically", with a live preview. Values are the style's effective values; the
 * engine only stores the fields that changed.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { Alignment, DocStyleEntry, DocStyleSpec } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState, type CSSProperties } from 'react';
import styles from './Editor.module.css';

/** Empty spec for a new style. */
export function emptyStyleSpec(basedOn: string | null): DocStyleSpec {
  return {
    name: '',
    type: 'paragraph',
    basedOn,
    next: null,
    addToGallery: true,
    autoUpdate: false,
    font: null,
    size: null,
    bold: null,
    italic: null,
    underline: null,
    color: null,
    align: null,
    spaceBefore: null,
    spaceAfter: null,
    lineSpacing: null,
    indentLeft: null,
    indentRight: null,
    indentFirstLine: null,
    border: null,
    shading: null,
    numId: null,
    numLevel: 0,
  };
}

const ALIGNS: Alignment[] = ['Left', 'Center', 'Right', 'Justify'];
const BORDERS = ['single', 'double', 'dotted', 'thick'] as const;

/** Twips ↔ points (one decimal). */
const toPt = (tw: number | null) => (tw === null ? '' : String(Math.round(tw / 2) / 10));
const fromPt = (s: string) => {
  const v = Number.parseFloat(s.replace(',', '.'));
  return s.trim() === '' || Number.isNaN(v) ? null : Math.round(v * 20);
};

/** CSS of the preview paragraph. */
export function previewCss(s: DocStyleSpec): CSSProperties {
  const css: CSSProperties = {
    fontFamily: s.font ? `"${s.font}", serif` : undefined,
    fontSize: `${Math.min(28, Math.max(8, (s.size ?? 22) / 2))}pt`,
    fontWeight: s.bold ? 700 : 400,
    fontStyle: s.italic ? 'italic' : 'normal',
    textDecoration: s.underline ? 'underline' : 'none',
    color: s.color ? `#${s.color}` : undefined,
    textAlign:
      s.align === 'Center'
        ? 'center'
        : s.align === 'Right'
          ? 'right'
          : s.align === 'Justify'
            ? 'justify'
            : 'left',
    marginTop: `${(s.spaceBefore ?? 0) / 20}pt`,
    marginBottom: `${(s.spaceAfter ?? 0) / 20}pt`,
    marginLeft: `${(s.indentLeft ?? 0) / 20}pt`,
    marginRight: `${(s.indentRight ?? 0) / 20}pt`,
    textIndent: `${(s.indentFirstLine ?? 0) / 20}pt`,
    lineHeight: s.lineSpacing ? String(s.lineSpacing / 240) : undefined,
    background: s.shading ? `#${s.shading}` : undefined,
    padding: s.border || s.shading ? '2px 4px' : undefined,
  };
  if (s.border) {
    const kind =
      s.border.style === 'double' ? 'double' : s.border.style === 'dotted' ? 'dotted' : 'solid';
    css.border = `${Math.max(1, s.border.size / 8)}px ${kind} ${s.border.color ? `#${s.border.color}` : 'currentColor'}`;
  }
  return css;
}

export interface StyleDialogProps {
  /** `modify`: edit `styleId`; `new`: create (optionally from the selection). */
  mode: 'modify' | 'new';
  styleId?: string;
  initial: DocStyleSpec;
  /** Styles offered for "based on" / "next". */
  styles: DocStyleEntry[];
  /** Error key from the engine (`docStyles.errors.*`). */
  error?: string | null;
  onCancel: () => void;
  onOk: (spec: DocStyleSpec, fromSelection: boolean) => void;
}

export function StyleDialog({
  mode,
  styleId,
  initial,
  styles: list,
  error,
  onCancel,
  onOk,
}: StyleDialogProps) {
  const { t } = useTranslation();
  const [s, setS] = useState<DocStyleSpec>(initial);
  const [fromSel, setFromSel] = useState(mode === 'new');
  const set = (patch: Partial<DocStyleSpec>) => setS((o) => ({ ...o, ...patch }));
  const para = s.type === 'paragraph';
  const others = list.filter(
    (e) => e.id !== styleId && e.kind === (para ? 'paragraph' : 'character'),
  );
  const tri = (v: boolean | null, f: (b: boolean) => void, key: string, id: string) => (
    <label className={styles.field}>
      <input
        type="checkbox"
        data-testid={`style-dlg-${id}`}
        checked={!!v}
        onChange={(e) => f(e.target.checked)}
      />
      {t(key)}
    </label>
  );
  const ptField = (key: keyof DocStyleSpec, label: string, id: string) => (
    <label className={styles.field}>
      <span>{t(label)}</span>
      <input
        type="number"
        step="0.5"
        data-testid={`style-dlg-${id}`}
        value={toPt(s[key] as number | null)}
        onChange={(e) => set({ [key]: fromPt(e.target.value) } as Partial<DocStyleSpec>)}
      />
    </label>
  );
  return (
    <Dialog
      open
      width={560}
      onOpenChange={(o) => !o && onCancel()}
      title={t(mode === 'modify' ? 'docStyles.dialog.modifyTitle' : 'docStyles.dialog.newTitle')}
      testId="style-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onCancel}>
            {t('docStyles.dialog.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="style-dlg-ok"
            disabled={!s.name.trim()}
            onClick={() => onOk(s, mode === 'new' && fromSel)}
          >
            {t('docStyles.dialog.ok')}
          </button>
        </>
      }
    >
      <div className={styles.formGrid} onKeyDown={(e) => e.stopPropagation()}>
        <label className={styles.field}>
          <span>{t('docStyles.dialog.name')}</span>
          <input
            data-testid="style-dlg-name"
            value={s.name}
            onChange={(e) => set({ name: e.target.value })}
          />
        </label>
        <label className={styles.field}>
          <span>{t('docStyles.dialog.type')}</span>
          <select
            data-testid="style-dlg-type"
            value={s.type}
            disabled={mode === 'modify'}
            onChange={(e) => set({ type: e.target.value as DocStyleSpec['type'] })}
          >
            <option value="paragraph">{t('docStyles.dialog.typeParagraph')}</option>
            <option value="character">{t('docStyles.dialog.typeCharacter')}</option>
          </select>
        </label>
        <label className={styles.field}>
          <span>{t('docStyles.dialog.basedOn')}</span>
          <select
            data-testid="style-dlg-based-on"
            value={s.basedOn ?? ''}
            onChange={(e) => set({ basedOn: e.target.value || null })}
          >
            <option value="">{t('docStyles.dialog.none')}</option>
            {others.map((o) => (
              <option key={o.id} value={o.id}>
                {o.name}
              </option>
            ))}
          </select>
        </label>
        {para && (
          <label className={styles.field}>
            <span>{t('docStyles.dialog.next')}</span>
            <select
              data-testid="style-dlg-next"
              value={s.next ?? ''}
              onChange={(e) => set({ next: e.target.value || null })}
            >
              <option value="">{t('docStyles.dialog.none')}</option>
              {list
                .filter((o) => o.kind === 'paragraph')
                .map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.name}
                  </option>
                ))}
            </select>
          </label>
        )}
        <label className={styles.field}>
          <span>{t('docStyles.dialog.font')}</span>
          <input
            data-testid="style-dlg-font"
            value={s.font ?? ''}
            onChange={(e) => set({ font: e.target.value.trim() || null })}
          />
        </label>
        <label className={styles.field}>
          <span>{t('docStyles.dialog.size')}</span>
          <input
            type="number"
            step="0.5"
            min="1"
            data-testid="style-dlg-size"
            value={s.size === null ? '' : s.size / 2}
            onChange={(e) => {
              const v = Number.parseFloat(e.target.value);
              set({ size: Number.isNaN(v) ? null : Math.round(v * 2) });
            }}
          />
        </label>
        {tri(s.bold, (bold) => set({ bold }), 'docStyles.dialog.bold', 'bold')}
        {tri(s.italic, (italic) => set({ italic }), 'docStyles.dialog.italic', 'italic')}
        {tri(
          s.underline,
          (underline) => set({ underline }),
          'docStyles.dialog.underline',
          'underline',
        )}
        <label className={styles.field}>
          <span>{t('docStyles.dialog.color')}</span>
          <input
            type="color"
            data-testid="style-dlg-color"
            value={`#${s.color ?? '000000'}`}
            onChange={(e) => set({ color: e.target.value.slice(1).toUpperCase() })}
          />
        </label>
        {para && (
          <>
            <label className={styles.field}>
              <span>{t('docStyles.dialog.align')}</span>
              <select
                data-testid="style-dlg-align"
                value={s.align ?? ''}
                onChange={(e) => set({ align: (e.target.value || null) as Alignment | null })}
              >
                <option value="">{t('docStyles.dialog.none')}</option>
                {ALIGNS.map((a) => (
                  <option key={a} value={a}>
                    {t(`docStyles.dialog.align${a}`)}
                  </option>
                ))}
              </select>
            </label>
            <label className={styles.field}>
              <span>{t('docStyles.dialog.lineSpacing')}</span>
              <input
                type="number"
                step="0.05"
                min="0.5"
                data-testid="style-dlg-line"
                value={s.lineSpacing === null ? '' : s.lineSpacing / 240}
                onChange={(e) => {
                  const v = Number.parseFloat(e.target.value);
                  set({ lineSpacing: Number.isNaN(v) ? null : Math.round(v * 240) });
                }}
              />
            </label>
            {ptField('spaceBefore', 'docStyles.dialog.spaceBefore', 'space-before')}
            {ptField('spaceAfter', 'docStyles.dialog.spaceAfter', 'space-after')}
            {ptField('indentLeft', 'docStyles.dialog.indentLeft', 'indent-left')}
            {ptField('indentRight', 'docStyles.dialog.indentRight', 'indent-right')}
            {ptField('indentFirstLine', 'docStyles.dialog.indentFirstLine', 'indent-first')}
            <label className={styles.field}>
              <span>{t('docStyles.dialog.border')}</span>
              <select
                data-testid="style-dlg-border"
                value={s.border?.style ?? ''}
                onChange={(e) =>
                  set({
                    border: e.target.value
                      ? {
                          style: e.target.value,
                          size: e.target.value === 'thick' ? 12 : 4,
                          color: s.border?.color ?? null,
                        }
                      : null,
                  })
                }
              >
                <option value="">{t('docStyles.dialog.borderNone')}</option>
                {BORDERS.map((b) => (
                  <option key={b} value={b}>
                    {t(`docStyles.dialog.border${b[0]!.toUpperCase()}${b.slice(1)}`)}
                  </option>
                ))}
              </select>
            </label>
            <label className={styles.field}>
              <span>{t('docStyles.dialog.shading')}</span>
              <input
                type="checkbox"
                aria-label={t('docStyles.dialog.shading')}
                data-testid="style-dlg-shading-on"
                checked={s.shading !== null}
                onChange={(e) => set({ shading: e.target.checked ? 'F2F2F2' : null })}
              />
              <input
                type="color"
                aria-label={t('docStyles.dialog.shading')}
                disabled={s.shading === null}
                value={`#${s.shading ?? 'FFFFFF'}`}
                onChange={(e) => set({ shading: e.target.value.slice(1).toUpperCase() })}
              />
            </label>
            <label className={styles.field}>
              <span>{t('docStyles.dialog.numbering')}</span>
              <select
                data-testid="style-dlg-numbering"
                value={s.numId === null ? 'keep' : s.numId === 0 ? 'none' : String(s.numId)}
                onChange={(e) => {
                  const v = e.target.value;
                  set({ numId: v === 'keep' ? null : v === 'none' ? 0 : Number(v) });
                }}
              >
                <option value="keep">{t('docStyles.dialog.numberingKeep')}</option>
                <option value="none">{t('docStyles.dialog.numberingNone')}</option>
                {initial.numId !== null && initial.numId > 0 && (
                  <option value={String(initial.numId)}>#{initial.numId}</option>
                )}
              </select>
            </label>
            <label className={styles.field}>
              <span>{t('docStyles.dialog.numberingLevel')}</span>
              <input
                type="number"
                min="1"
                max="9"
                disabled={!s.numId}
                value={s.numLevel + 1}
                onChange={(e) =>
                  set({ numLevel: Math.min(8, Math.max(0, (Number(e.target.value) || 1) - 1)) })
                }
              />
            </label>
          </>
        )}
        {tri(
          s.addToGallery,
          (addToGallery) => set({ addToGallery }),
          'docStyles.dialog.addToGallery',
          'gallery',
        )}
        {para &&
          tri(
            s.autoUpdate,
            (autoUpdate) => set({ autoUpdate }),
            'docStyles.dialog.autoUpdate',
            'auto-update',
          )}
        {mode === 'new' &&
          tri(fromSel, setFromSel, 'docStyles.dialog.fromSelection', 'from-selection')}
      </div>
      <fieldset style={{ marginTop: 10 }}>
        <legend>{t('docStyles.dialog.preview')}</legend>
        <p style={previewCss(s)} data-testid="style-dlg-preview">
          {t('docStyles.dialog.previewText')}
        </p>
      </fieldset>
      {error && (
        <p role="alert" className={styles.hint} data-testid="style-dlg-error">
          {t(`docStyles.errors.${error}`, { defaultValue: t('docStyles.errors.failed') })}
        </p>
      )}
    </Dialog>
  );
}
