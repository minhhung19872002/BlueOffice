/**
 * BlueSheet page setup (tabs: page, margins, header / footer, sheet) and
 * print preview. Settings go through `setPageSetup`; the preview draws the
 * pages of the engine's `printPreview` command, which uses the same
 * pagination as the PDF export (ECMA-376 Part 1, §18.3.1.63 pageSetup,
 * §18.3.1.62 pageMargins, §18.3.1.70 printOptions, §18.3.1.46 headerFooter).
 */
import { GlyphStore, renderPage } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import type {
  HeaderFooterSpec,
  PageOrder,
  PageSetupInfo,
  PrintComments,
  PrintErrors,
  PrintPreviewInfo,
  SheetHost,
  SheetRange,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useMemo, useRef, useState } from 'react';
import { a1, parseA1, rangeOf } from './grid';
import {
  HF_FIELDS,
  areaText,
  colSpanText,
  insertCode,
  joinSections,
  parseColSpan,
  parseRowSpan,
  rowSpanText,
  splitSections,
  type HfSections,
} from './hfCodes';
import styles from './SheetEditor.module.css';
import ps from './pageSetup.module.css';

interface Common {
  host: SheetHost;
  sheet: number;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
  /** Picks a picture file (through the platform); absent: no header / footer pictures. */
  pickFile?: (accept: string[]) => Promise<{ name: string; bytes: Uint8Array } | null>;
}

const HF_PICTURE_TYPES = ['.png', '.jpg', '.jpeg', '.gif', '.bmp'];

function bytesToBase64(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

type Tab = 'page' | 'margins' | 'headerFooter' | 'sheet';
type HfKey =
  'oddHeader' | 'oddFooter' | 'evenHeader' | 'evenFooter' | 'firstHeader' | 'firstFooter';
type Section = keyof HfSections;

const HF_KEYS: HfKey[] = [
  'oddHeader',
  'oddFooter',
  'evenHeader',
  'evenFooter',
  'firstHeader',
  'firstFooter',
];

/** Parses `A1:B5, D1:E5` into ranges; `null` when any part is invalid. */
function parseAreas(text: string): SheetRange[] | null {
  const parts = text
    .split(/[,;]/)
    .map((p) => p.trim())
    .filter(Boolean);
  const out: SheetRange[] = [];
  for (const p of parts) {
    const s = parseA1(p.includes('!') ? p.slice(p.lastIndexOf('!') + 1) : p);
    if (!s) return null;
    out.push(rangeOf(s));
  }
  return out;
}

/** VML shape id of a header / footer picture (§18.3.1.50 legacyDrawingHF): `L`/`C`/`R`,
 * `H`/`F`, then `FIRST` / `EVEN` for those variants. */
export function pictureSection(key: HfKey, sec: Section): string {
  const letter = sec === 'left' ? 'L' : sec === 'center' ? 'C' : 'R';
  const hf = key.endsWith('Header') ? 'H' : 'F';
  const suffix = key.startsWith('first') ? 'FIRST' : key.startsWith('even') ? 'EVEN' : '';
  return `${letter}${hf}${suffix}`;
}

/** A picture chosen for a section (applied on OK); `null` removes the section's picture. */
export type PendingPicture = { image: string; ext: string; name: string } | null;

function HeaderFooterEditor({
  label,
  value,
  onChange,
  testId,
  hfKey,
  pictures,
  onPicture,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  testId: string;
  hfKey: HfKey;
  /** Sections (shape ids) that have a picture. */
  pictures: Set<string>;
  /** Choose (`true`) or remove (`false`) the focused section's picture; absent: no picker. */
  onPicture?: (section: string, add: boolean) => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const sections = splitSections(value);
  const [focus, setFocus] = useState<Section>('center');
  const refs = {
    left: useRef<HTMLTextAreaElement>(null),
    center: useRef<HTMLTextAreaElement>(null),
    right: useRef<HTMLTextAreaElement>(null),
  };
  const set = (sec: Section, text: string) => onChange(joinSections({ ...sections, [sec]: text }));
  const insert = (code: string) => {
    const el = refs[focus].current;
    const cur = sections[focus];
    const { text, caret } = insertCode(
      cur,
      code,
      el?.selectionStart ?? cur.length,
      el?.selectionEnd ?? cur.length,
    );
    set(focus, text);
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(caret, caret);
    });
  };
  return (
    <fieldset className={ps.hfBox} data-testid={testId}>
      <legend>{label}</legend>
      <div className={ps.hfCodes}>
        {HF_FIELDS.map(([key, code]) => (
          <button
            key={key}
            type="button"
            className={styles.btn}
            title={t(`sheetEditor.printSetup.code.${key}`)}
            data-testid={`${testId}-code-${key}`}
            onClick={() => insert(code)}
          >
            {t(`sheetEditor.printSetup.code.${key}`)}
          </button>
        ))}
        {onPicture && (
          <button
            type="button"
            className={styles.btn}
            data-testid={`${testId}-picture`}
            onClick={() =>
              void onPicture(pictureSection(hfKey, focus), true).then((ok) => {
                if (ok && !sections[focus].includes('&G')) insert('&G');
              })
            }
          >
            {t('sheetEditor.printSetup.picture')}
          </button>
        )}
        {onPicture && pictures.has(pictureSection(hfKey, focus)) && (
          <button
            type="button"
            className={styles.btn}
            data-testid={`${testId}-remove-picture`}
            onClick={() => {
              void onPicture(pictureSection(hfKey, focus), false);
              set(focus, sections[focus].split('&G').join(''));
            }}
          >
            {t('sheetEditor.printSetup.removePicture')}
          </button>
        )}
      </div>
      <div className={ps.hfSections}>
        {(['left', 'center', 'right'] as const).map((sec) => (
          <label key={sec}>
            {pictures.has(pictureSection(hfKey, sec))
              ? t('sheetEditor.printSetup.hasPicture', {
                  section: t(`sheetEditor.printSetup.section.${sec}`),
                })
              : t(`sheetEditor.printSetup.section.${sec}`)}
            <textarea
              ref={refs[sec]}
              rows={2}
              value={sections[sec]}
              data-testid={`${testId}-${sec}`}
              onFocus={() => setFocus(sec)}
              onChange={(e) => set(sec, e.target.value)}
            />
          </label>
        ))}
      </div>
    </fieldset>
  );
}

export function PageSetupDialog({
  host,
  sheet,
  range,
  onClose,
  onDone,
  fail,
  pickFile,
}: Common & { range: SheetRange }) {
  const { t } = useTranslation();
  const [pending, setPending] = useState<Record<string, PendingPicture>>({});
  const [tab, setTab] = useState<Tab>('page');
  const [info, setInfo] = useState<PageSetupInfo | null>(null);
  const [fit, setFit] = useState(false);
  const [first, setFirst] = useState('');
  const [area, setArea] = useState('');
  const [rows, setRows] = useState('');
  const [cols, setCols] = useState('');
  const [hf, setHf] = useState<Required<HeaderFooterSpec> | null>(null);
  const [error, setError] = useState('');
  useEffect(() => {
    void host
      .exec<PageSetupInfo>({ cmd: 'pageSetup', sheet })
      .then((p) => {
        setInfo(p);
        setFit(!!p.fit);
        setFirst(p.extra.firstPageNumber === null ? '' : String(p.extra.firstPageNumber));
        setArea(areaText(p.printArea));
        setRows(rowSpanText(p.titleRows));
        setCols(colSpanText(p.extra.titleCols));
        setHf(p.extra.headerFooter);
      })
      .catch(fail);
  }, [host, sheet, fail]);
  if (!info || !hf) return null;
  const pictureSet = new Set(
    (info.extra.hfPictures ?? []).map((p) => p.section).filter((sec) => pending[sec] !== null),
  );
  for (const [sec, p] of Object.entries(pending)) if (p) pictureSet.add(sec);
  const choosePicture = async (section: string, add: boolean): Promise<boolean> => {
    if (!add) {
      setPending((m) => ({ ...m, [section]: null }));
      return true;
    }
    const f = await pickFile?.(HF_PICTURE_TYPES);
    if (!f) return false;
    const ext = f.name.includes('.') ? f.name.slice(f.name.lastIndexOf('.') + 1) : 'png';
    setPending((m) => ({
      ...m,
      [section]: { image: bytesToBase64(f.bytes), ext, name: f.name },
    }));
    return true;
  };
  const x = info.extra;
  const setX = (patch: Partial<PageSetupInfo['extra']>) =>
    setInfo({ ...info, extra: { ...x, ...patch } });
  const ok = async () => {
    const areas = parseAreas(area);
    if (areas === null) {
      setError(t('sheetEditor.printSetup.badArea'));
      setTab('sheet');
      return;
    }
    const titleRows = parseRowSpan(rows);
    const titleCols = parseColSpan(cols);
    if ((rows.trim() && !titleRows) || (cols.trim() && !titleCols)) {
      setError(t('sheetEditor.printSetup.badTitles'));
      setTab('sheet');
      return;
    }
    const firstNo = first.trim() ? Math.max(1, Math.floor(Number(first)) || 1) : null;
    try {
      await host.exec({
        cmd: 'setPageSetup',
        sheet,
        orientation: info.orientation,
        paperSize: info.paperSize,
        scale: info.scale,
        fit: fit ? (info.fit ?? [1, 1]) : null,
        margins: info.margins,
        gridLines: info.gridLines,
        titleRows,
        titleCols,
        printAreas: areas,
        headings: x.headings,
        centerH: x.centerH,
        centerV: x.centerV,
        pageOrder: x.pageOrder,
        firstPageNumber: firstNo,
        blackAndWhite: x.blackAndWhite,
        draft: x.draft,
        errors: x.errors,
        comments: x.comments,
        hfMargins: x.hfMargins,
        headerFooter: hf,
      });
      // Pictures chosen in the dialog (§18.3.1.50 legacyDrawingHF), one step each.
      for (const [section, p] of Object.entries(pending)) {
        await host.exec(
          p
            ? {
                cmd: 'headerFooterPicture',
                sheet,
                section,
                image: p.image,
                ext: p.ext,
                name: p.name,
              }
            : { cmd: 'headerFooterPicture', sheet, section },
        );
      }
      await onDone();
      onClose();
    } catch (e) {
      fail(e);
    }
  };
  const num = (
    label: string,
    value: number,
    onChange: (v: number) => void,
    testId: string,
    opts: { min?: number; max?: number; step?: number; disabled?: boolean } = {},
  ) => (
    <label>
      {label}
      <input
        type="number"
        data-testid={testId}
        min={opts.min ?? 0}
        max={opts.max}
        step={opts.step ?? 1}
        disabled={opts.disabled}
        value={value}
        onChange={(e) => onChange(Number(e.target.value) || 0)}
      />
    </label>
  );
  const check = (label: string, value: boolean, onChange: (v: boolean) => void, testId: string) => (
    <label className={styles.check}>
      <input
        type="checkbox"
        data-testid={testId}
        checked={value}
        onChange={(e) => onChange(e.target.checked)}
      />
      {label}
    </label>
  );
  const margin = (i: number, key: string) =>
    num(
      t(`sheetEditor.page.${key}`),
      info.margins[i]!,
      (v) => {
        const m = [...info.margins] as PageSetupInfo['margins'];
        m[i] = v;
        setInfo({ ...info, margins: m });
      },
      `page-margin-${key}`,
      { step: 0.05 },
    );
  const fitWH = info.fit ?? [1, 1];
  const shownHf = HF_KEYS.filter(
    (k) =>
      k.startsWith('odd') ||
      (k.startsWith('even') && hf.differentOddEven) ||
      (k.startsWith('first') && hf.differentFirst),
  );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.page.title')}
      width={620}
      testId="page-setup"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="page-ok"
            onClick={() => void ok()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.dlgTabs} role="tablist">
        {(['page', 'margins', 'headerFooter', 'sheet'] as const).map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            data-testid={`page-tab-${k}`}
            className={tab === k ? styles.dlgTabActive : styles.dlgTab}
            onClick={() => setTab(k)}
          >
            {t(`sheetEditor.printSetup.tab.${k}`)}
          </button>
        ))}
      </div>
      {error && <p className={styles.error}>{error}</p>}
      {tab === 'page' && (
        <div className={styles.form}>
          <label>
            {t('sheetEditor.page.orientation')}
            <select
              data-testid="page-orientation"
              value={info.orientation}
              onChange={(e) =>
                setInfo({ ...info, orientation: e.target.value as PageSetupInfo['orientation'] })
              }
            >
              <option value="portrait">{t('sheetEditor.page.portrait')}</option>
              <option value="landscape">{t('sheetEditor.page.landscape')}</option>
            </select>
          </label>
          <label>
            {t('sheetEditor.page.paper')}
            <select
              value={info.paperSize}
              onChange={(e) => setInfo({ ...info, paperSize: Number(e.target.value) })}
            >
              <option value={9}>A4</option>
              <option value={8}>A3</option>
              <option value={11}>A5</option>
              <option value={1}>Letter</option>
              <option value={5}>Legal</option>
            </select>
          </label>
          <label className={styles.check}>
            <input
              type="radio"
              name="page-scaling"
              data-testid="page-scale-mode"
              checked={!fit}
              onChange={() => setFit(false)}
            />
            {t('sheetEditor.printSetup.scaleTo')}
          </label>
          {num(
            t('sheetEditor.page.scale'),
            info.scale,
            (v) => setInfo({ ...info, scale: v }),
            'page-scale',
            { min: 10, max: 400, disabled: fit },
          )}
          <label className={styles.check}>
            <input
              type="radio"
              name="page-scaling"
              data-testid="page-fit-mode"
              checked={fit}
              onChange={() => setFit(true)}
            />
            {t('sheetEditor.printSetup.fitTo')}
          </label>
          {num(
            t('sheetEditor.printSetup.pagesWide'),
            fitWH[0],
            (v) => setInfo({ ...info, fit: [v, fitWH[1]] }),
            'page-fit-wide',
            { disabled: !fit },
          )}
          {num(
            t('sheetEditor.printSetup.pagesTall'),
            fitWH[1],
            (v) => setInfo({ ...info, fit: [fitWH[0], v] }),
            'page-fit-tall',
            { disabled: !fit },
          )}
          <p className={styles.hint}>{t('sheetEditor.printSetup.fitHint')}</p>
          <label>
            {t('sheetEditor.printSetup.firstPage')}
            <input
              data-testid="page-first-number"
              placeholder={t('sheetEditor.printSetup.auto')}
              value={first}
              onChange={(e) => setFirst(e.target.value)}
            />
          </label>
        </div>
      )}
      {tab === 'margins' && (
        <div className={styles.form}>
          {margin(0, 'left')}
          {margin(1, 'right')}
          {margin(2, 'top')}
          {margin(3, 'bottom')}
          {num(
            t('sheetEditor.printSetup.headerMargin'),
            x.hfMargins[0],
            (v) => setX({ hfMargins: [v, x.hfMargins[1]] }),
            'page-margin-header',
            { step: 0.05 },
          )}
          {num(
            t('sheetEditor.printSetup.footerMargin'),
            x.hfMargins[1],
            (v) => setX({ hfMargins: [x.hfMargins[0], v] }),
            'page-margin-footer',
            { step: 0.05 },
          )}
          {check(
            t('sheetEditor.printSetup.centerH'),
            x.centerH,
            (v) => setX({ centerH: v }),
            'page-center-h',
          )}
          {check(
            t('sheetEditor.printSetup.centerV'),
            x.centerV,
            (v) => setX({ centerV: v }),
            'page-center-v',
          )}
        </div>
      )}
      {tab === 'headerFooter' && (
        <div className={styles.form}>
          {check(
            t('sheetEditor.printSetup.oddEven'),
            hf.differentOddEven,
            (v) => setHf({ ...hf, differentOddEven: v }),
            'page-hf-odd-even',
          )}
          {check(
            t('sheetEditor.printSetup.firstDifferent'),
            hf.differentFirst,
            (v) => setHf({ ...hf, differentFirst: v }),
            'page-hf-first',
          )}
          {check(
            t('sheetEditor.printSetup.scaleWithDoc'),
            hf.scaleWithDoc,
            (v) => setHf({ ...hf, scaleWithDoc: v }),
            'page-hf-scale',
          )}
          {check(
            t('sheetEditor.printSetup.alignMargins'),
            hf.alignWithMargins,
            (v) => setHf({ ...hf, alignWithMargins: v }),
            'page-hf-align',
          )}
          {shownHf.map((k) => (
            <HeaderFooterEditor
              key={k}
              label={t(`sheetEditor.printSetup.hf.${k}`)}
              value={hf[k]}
              testId={`page-hf-${k}`}
              onChange={(v) => setHf({ ...hf, [k]: v })}
              hfKey={k}
              pictures={pictureSet}
              onPicture={pickFile ? choosePicture : undefined}
            />
          ))}
        </div>
      )}
      {tab === 'sheet' && (
        <div className={styles.form}>
          <label>
            {t('sheetEditor.printSetup.area')}
            <input
              data-testid="page-area"
              placeholder="A1:F40"
              value={area}
              onChange={(e) => setArea(e.target.value)}
            />
          </label>
          <button
            type="button"
            className={styles.btn}
            data-testid="page-area-selection"
            onClick={() =>
              setArea(
                `${a1({ row: range.r0, col: range.c0 })}:${a1({ row: range.r1, col: range.c1 })}`,
              )
            }
          >
            {t('sheetEditor.printSetup.useSelection')}
          </button>
          <label>
            {t('sheetEditor.page.titleRows')}
            <input
              data-testid="page-title-rows"
              placeholder="1:1"
              value={rows}
              onChange={(e) => setRows(e.target.value)}
            />
          </label>
          <label>
            {t('sheetEditor.printSetup.titleCols')}
            <input
              data-testid="page-title-cols"
              placeholder="A:A"
              value={cols}
              onChange={(e) => setCols(e.target.value)}
            />
          </label>
          {check(
            t('sheetEditor.page.gridLines'),
            info.gridLines,
            (v) => setInfo({ ...info, gridLines: v }),
            'page-gridlines',
          )}
          {check(
            t('sheetEditor.printSetup.headings'),
            x.headings,
            (v) => setX({ headings: v }),
            'page-headings',
          )}
          {check(
            t('sheetEditor.printSetup.blackWhite'),
            x.blackAndWhite,
            (v) => setX({ blackAndWhite: v }),
            'page-bw',
          )}
          {check(
            t('sheetEditor.printSetup.draft'),
            x.draft,
            (v) => setX({ draft: v }),
            'page-draft',
          )}
          <label>
            {t('sheetEditor.printSetup.comments')}
            <select
              data-testid="page-comments"
              value={x.comments}
              onChange={(e) => setX({ comments: e.target.value as PrintComments })}
            >
              {(['none', 'atEnd', 'asDisplayed'] as const).map((v) => (
                <option key={v} value={v}>
                  {t(`sheetEditor.printSetup.commentsMode.${v}`)}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sheetEditor.printSetup.errors')}
            <select
              data-testid="page-errors"
              value={x.errors}
              onChange={(e) => setX({ errors: e.target.value as PrintErrors })}
            >
              {(['displayed', 'blank', 'dash', 'NA'] as const).map((v) => (
                <option key={v} value={v}>
                  {t(`sheetEditor.printSetup.errorsMode.${v}`)}
                </option>
              ))}
            </select>
          </label>
          <fieldset className={ps.hfBox}>
            <legend>{t('sheetEditor.printSetup.order')}</legend>
            {(['downThenOver', 'overThenDown'] as const).map((v: PageOrder) => (
              <label key={v} className={styles.check}>
                <input
                  type="radio"
                  name="page-order"
                  data-testid={`page-order-${v}`}
                  checked={x.pageOrder === v}
                  onChange={() => setX({ pageOrder: v })}
                />
                {t(`sheetEditor.printSetup.orderMode.${v}`)}
              </label>
            ))}
          </fieldset>
        </div>
      )}
    </Dialog>
  );
}

/** Print preview: the printed pages of the sheet, one at a time. */
export function PrintPreviewDialog({
  host,
  sheet,
  onClose,
  onSetup,
  onPrint,
  fail,
}: Omit<Common, 'onDone'> & { onSetup: () => void; onPrint: () => void }) {
  const { t } = useTranslation();
  const [page, setPage] = useState(0);
  const [pages, setPages] = useState(0);
  const canvas = useRef<HTMLCanvasElement>(null);
  const glyphs = useMemo(() => new GlyphStore(host), [host]);
  useEffect(() => {
    let live = true;
    void (async () => {
      const r = await host.exec<PrintPreviewInfo>({ cmd: 'printPreview', sheet, page });
      if (!live) return;
      setPages(r.pages);
      const el = canvas.current;
      const ctx = el?.getContext('2d');
      if (!r.page || !el || !ctx) return;
      await glyphs.ensure(r.page);
      if (!live) return;
      // Fit the page into 520 × 560 CSS pixels.
      const scale = Math.min(520 / r.page.width, 560 / r.page.height);
      const dpr = window.devicePixelRatio || 1;
      el.width = Math.round(r.page.width * scale * dpr);
      el.height = Math.round(r.page.height * scale * dpr);
      el.style.width = `${Math.round(r.page.width * scale)}px`;
      el.style.height = `${Math.round(r.page.height * scale)}px`;
      renderPage(ctx, r.page, glyphs, scale * dpr);
      el.dataset.page = String(page);
    })().catch(fail);
    return () => {
      live = false;
    };
  }, [host, sheet, page, glyphs, fail]);
  const last = Math.max(0, pages - 1);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.printSetup.preview')}
      width={600}
      testId="print-preview"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onSetup}>
            {t('sheetEditor.page.title')}
          </button>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="print-preview-print"
            onClick={onPrint}
          >
            {t('sheetEditor.print')}
          </button>
        </>
      }
    >
      <div className={ps.nav}>
        <button
          type="button"
          className={styles.btn}
          data-testid="print-preview-prev"
          disabled={page <= 0}
          onClick={() => setPage((p) => Math.max(0, p - 1))}
        >
          {t('sheetEditor.printSetup.prev')}
        </button>
        <span data-testid="print-preview-count">
          {pages === 0
            ? t('sheetEditor.printSetup.nothing')
            : t('sheetEditor.printSetup.pageOf', { page: page + 1, pages })}
        </span>
        <button
          type="button"
          className={styles.btn}
          data-testid="print-preview-next"
          disabled={page >= last}
          onClick={() => setPage((p) => Math.min(last, p + 1))}
        >
          {t('sheetEditor.printSetup.next')}
        </button>
      </div>
      <div className={ps.paper}>
        <canvas ref={canvas} data-testid="print-preview-canvas" />
      </div>
    </Dialog>
  );
}
