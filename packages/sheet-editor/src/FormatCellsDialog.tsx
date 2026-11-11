import { useTranslation } from '@blueoffice/i18n';
import type { BorderSpec, CellFormatState, StylePatch } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import { SHEET_FONTS } from './homeFormat';
import { VERTICAL_TEXT, degreesToRotation, rotationToDegrees } from './orientation';
import styles from './SheetEditor.module.css';

export interface NumberPreset {
  id: string;
  code: string;
}

/** Tabs of Format Cells (Ctrl+1), in the spreadsheet's order. */
export type FormatCellsTab = 'number' | 'alignment' | 'font' | 'border' | 'fill' | 'protection';
type Tab = FormatCellsTab;

const FONTS = SHEET_FONTS;
const LINE_STYLES = ['thin', 'medium', 'thick', 'dashed', 'dotted', 'double', 'hair'];
const H_ALIGN = ['', 'left', 'center', 'right', 'justify', 'fill', 'distributed'];
const V_ALIGN = ['', 'top', 'center'];

const hex = (c: string) => (c ? `#${c}` : '#000000');
const unhex = (c: string) => c.replace('#', '').toUpperCase();

/**
 * "Định dạng ô / Format Cells" dialog: Number, Alignment, Font, Border and Fill
 * tabs. Mounted when opened, so its state starts from `initial`.
 */
export function FormatCellsDialog({
  open,
  initial,
  tab: firstTab = 'number',
  presets,
  onClose,
  onApply,
}: {
  open: boolean;
  initial: CellFormatState | null;
  /** Tab shown first (Font for Ctrl+Shift+F, Border for "More borders"). */
  tab?: Tab;
  presets: NumberPreset[];
  onClose: () => void;
  onApply: (patch: StylePatch, borders: BorderSpec[]) => void;
}) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>(firstTab);
  const [st, setSt] = useState<CellFormatState | null>(initial);
  const [borders, setBorders] = useState<BorderSpec[]>([]);
  const [lineStyle, setLineStyle] = useState('thin');
  const [lineColor, setLineColor] = useState('000000');

  if (!st || !initial) return null;
  const set = (p: Partial<CellFormatState>) => setSt({ ...st, ...p });

  const apply = () => {
    const patch: StylePatch = {};
    if (st.numFmt !== initial.numFmt) patch.numFmt = st.numFmt;
    if (st.fontName !== initial.fontName) patch.fontName = st.fontName;
    if (st.fontSize !== initial.fontSize) patch.fontSize = st.fontSize;
    if (st.bold !== initial.bold) patch.bold = st.bold;
    if (st.italic !== initial.italic) patch.italic = st.italic;
    if (st.underline !== initial.underline)
      patch.underline = (st.underline || 'none') as StylePatch['underline'];
    if (st.strike !== initial.strike) patch.strike = st.strike;
    if (st.fontColor !== initial.fontColor) patch.fontColor = st.fontColor;
    if (st.fillColor !== initial.fillColor) patch.fillColor = st.fillColor;
    if (st.hAlign !== initial.hAlign) patch.hAlign = st.hAlign;
    if (st.vAlign !== initial.vAlign) patch.vAlign = st.vAlign;
    if (st.wrap !== initial.wrap) patch.wrap = st.wrap;
    if (st.indent !== initial.indent) patch.indent = st.indent;
    if (st.rotation !== initial.rotation) patch.rotation = st.rotation;
    if (Boolean(st.shrink) !== Boolean(initial.shrink)) patch.shrink = Boolean(st.shrink);
    if (st.locked !== initial.locked) patch.locked = st.locked;
    if (st.formulaHidden !== initial.formulaHidden) patch.formulaHidden = st.formulaHidden;
    onApply(patch, borders);
  };

  const tabs: Tab[] = ['number', 'alignment', 'font', 'border', 'fill', 'protection'];
  const presetId = presets.find((p) => p.code === st.numFmt)?.id ?? 'custom';

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.fmt.title')}
      width={520}
      testId="format-cells"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="format-cells-ok"
            onClick={apply}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.dlgTabs} role="tablist">
        {tabs.map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            className={tab === k ? styles.dlgTabActive : styles.dlgTab}
            onClick={() => setTab(k)}
          >
            {t(`sheetEditor.fmt.tab.${k}`)}
          </button>
        ))}
      </div>
      {tab === 'number' && (
        <div className={styles.form}>
          <label>
            {t('sheetEditor.fmt.category')}
            <select
              value={presetId}
              onChange={(e) => {
                const p = presets.find((x) => x.id === e.target.value);
                if (p) set({ numFmt: p.code });
              }}
            >
              {presets.map((p) => (
                <option key={p.id} value={p.id}>
                  {t(`sheetEditor.numFmt.${p.id}`)}
                </option>
              ))}
              <option value="custom">{t('sheetEditor.numFmt.custom')}</option>
            </select>
          </label>
          <label>
            {t('sheetEditor.fmt.code')}
            <input
              data-testid="numfmt-code"
              value={st.numFmt}
              onChange={(e) => set({ numFmt: e.target.value })}
            />
          </label>
        </div>
      )}
      {tab === 'alignment' && (
        <div className={styles.form}>
          <label>
            {t('sheetEditor.fmt.hAlign')}
            <select value={st.hAlign} onChange={(e) => set({ hAlign: e.target.value })}>
              {H_ALIGN.map((a) => (
                <option key={a} value={a}>
                  {t(`sheetEditor.align.${a || 'general'}`)}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sheetEditor.fmt.vAlign')}
            <select value={st.vAlign} onChange={(e) => set({ vAlign: e.target.value })}>
              {V_ALIGN.map((a) => (
                <option key={a} value={a}>
                  {t(`sheetEditor.align.${a || 'bottom'}`)}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sheetEditor.fmt.indent')}
            <input
              type="number"
              min={0}
              max={15}
              value={st.indent}
              onChange={(e) => set({ indent: Number(e.target.value) || 0 })}
            />
          </label>
          <label>
            {t('sheetEditor.fmt.rotation')}
            <input
              type="number"
              min={-90}
              max={90}
              data-testid="format-cells-rotation"
              disabled={st.rotation === VERTICAL_TEXT}
              value={rotationToDegrees(st.rotation)}
              onChange={(e) =>
                set({ rotation: degreesToRotation(Number(e.target.value) || 0, false) })
              }
            />
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="format-cells-vertical"
              checked={st.rotation === VERTICAL_TEXT}
              onChange={(e) => set({ rotation: degreesToRotation(0, e.target.checked) })}
            />
            {t('sheetEditor.fmt.verticalText')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="format-cells-shrink"
              checked={Boolean(st.shrink)}
              disabled={st.wrap}
              onChange={(e) => set({ shrink: e.target.checked })}
            />
            {t('sheetEditor.fmt.shrink')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={st.wrap}
              onChange={(e) => set({ wrap: e.target.checked })}
            />
            {t('sheetEditor.fmt.wrap')}
          </label>
        </div>
      )}
      {tab === 'font' && (
        <div className={styles.form}>
          <label>
            {t('sheetEditor.fmt.fontName')}
            <input
              list="bo-sheet-fonts"
              value={st.fontName}
              onChange={(e) => set({ fontName: e.target.value })}
            />
            <datalist id="bo-sheet-fonts">
              {FONTS.map((f) => (
                <option key={f} value={f} />
              ))}
            </datalist>
          </label>
          <label>
            {t('sheetEditor.fmt.fontSize')}
            <input
              type="number"
              min={1}
              max={409}
              value={st.fontSize}
              onChange={(e) => set({ fontSize: Number(e.target.value) || 11 })}
            />
          </label>
          <label>
            {t('sheetEditor.fmt.underline')}
            <select value={st.underline} onChange={(e) => set({ underline: e.target.value })}>
              <option value="">{t('sheetEditor.fmt.none')}</option>
              <option value="single">{t('sheetEditor.fmt.single')}</option>
              <option value="double">{t('sheetEditor.fmt.double')}</option>
            </select>
          </label>
          <label>
            {t('sheetEditor.fmt.color')}
            <input
              type="color"
              value={hex(st.fontColor)}
              onChange={(e) => set({ fontColor: unhex(e.target.value) })}
            />
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={st.bold}
              onChange={(e) => set({ bold: e.target.checked })}
            />
            {t('sheetEditor.bold')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={st.italic}
              onChange={(e) => set({ italic: e.target.checked })}
            />
            {t('sheetEditor.italic')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={st.strike}
              onChange={(e) => set({ strike: e.target.checked })}
            />
            {t('sheetEditor.strike')}
          </label>
        </div>
      )}
      {tab === 'border' && (
        <div className={styles.form}>
          <label>
            {t('sheetEditor.fmt.lineStyle')}
            <select value={lineStyle} onChange={(e) => setLineStyle(e.target.value)}>
              {LINE_STYLES.map((s) => (
                <option key={s} value={s}>
                  {t(`sheetEditor.line.${s}`)}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sheetEditor.fmt.color')}
            <input
              type="color"
              value={hex(lineColor)}
              onChange={(e) => setLineColor(unhex(e.target.value))}
            />
          </label>
          <div className={styles.row}>
            {(['none', 'outline', 'inside', 'all'] as const).map((preset) => (
              <button
                key={preset}
                type="button"
                className={styles.btn}
                onClick={() =>
                  setBorders([...borders, { preset, style: lineStyle, color: lineColor }])
                }
              >
                {t(`sheetEditor.border.${preset}`)}
              </button>
            ))}
          </div>
          <p className={styles.hint}>
            {borders.length
              ? borders.map((b) => t(`sheetEditor.border.${b.preset}`)).join(' → ')
              : st.borders.some(Boolean)
                ? st.borders.map((b) => b || '–').join(' / ')
                : t('sheetEditor.fmt.none')}
          </p>
        </div>
      )}
      {tab === 'fill' && (
        <div className={styles.form}>
          <label>
            {t('sheetEditor.fmt.fillColor')}
            <input
              type="color"
              data-testid="fill-color-input"
              value={st.fillColor ? hex(st.fillColor) : '#ffffff'}
              onChange={(e) => set({ fillColor: unhex(e.target.value) })}
            />
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={!st.fillColor}
              onChange={(e) => set({ fillColor: e.target.checked ? '' : 'FFFF00' })}
            />
            {t('sheetEditor.fmt.noFill')}
          </label>
        </div>
      )}
      {tab === 'protection' && (
        // §18.8.33 protection: takes effect once the sheet is protected.
        <div className={styles.form}>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="format-cells-locked"
              checked={st.locked}
              onChange={(e) => set({ locked: e.target.checked })}
            />
            {t('sheetEditor.fmt.locked')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="format-cells-hidden"
              checked={st.formulaHidden}
              onChange={(e) => set({ formulaHidden: e.target.checked })}
            />
            {t('sheetEditor.fmt.formulaHidden')}
          </label>
          <p className={styles.hint}>{t('sheetEditor.fmt.protectionHint')}</p>
        </div>
      )}
    </Dialog>
  );
}
