/**
 * Print panel shared by BlueDoc, BlueSlide and BlueSheet (CORE-PRINT-001): the job's
 * settings on the left, the preview on the right. The preview draws sheet `i` of the very
 * PDF that is printed (`printPreview` of the engine's print pipeline, same options), so
 * what is shown is what comes out. Every string is a key of the `print` namespace.
 */
import { GlyphStore, ImageStore, renderPage } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import type {
  EngineHost,
  PageDisplayList,
  PrintColor,
  PrintHost,
  PrintOptions,
  PrintPageChoice,
  PrintPaper,
  PrintPerSheet,
  PrintPreview,
  PrintSubset,
  SheetPrintScope,
  SlidePrintLayout,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import styles from '../Editor.module.css';
import own from './PrintPanel.module.css';

export type PrintApp = 'doc' | 'slide' | 'sheet';

export const PER_SHEET: PrintPerSheet[] = [1, 2, 4, 6, 8, 16];
export const PAPERS: PrintPaper[] = ['a4', 'a3', 'a5', 'b5', 'letter', 'legal'];
export const HANDOUT_PER_PAGE = [1, 2, 3, 4, 6, 9] as const;

/** Next preview job number: the engine rebuilds the sheets for a new number. */
let jobCounter = 0;

/** Options the panel sends: copies clamped, the range only when it is used. */
export function jobOptions(o: PrintOptions, app: PrintApp, job: number): PrintOptions {
  const copies = Math.min(999, Math.max(1, Math.floor(Number(o.copies) || 1)));
  const out: PrintOptions = { ...o, copies, job };
  if (out.pages !== 'range') delete out.range;
  if (app !== 'doc') delete out.doc;
  if (app !== 'slide') delete out.slide;
  if (app !== 'sheet') delete out.sheet;
  return out;
}

/** Tri-state of a sheet override: as the page setup says, on, off. */
type Tri = 'setup' | 'on' | 'off';
const tri = (v: boolean | undefined): Tri => (v === undefined ? 'setup' : v ? 'on' : 'off');
const fromTri = (v: Tri): boolean | undefined => (v === 'setup' ? undefined : v === 'on');

export interface PrintPanelProps {
  app: PrintApp;
  /** The engine of the document (print pipeline + glyph outlines, pictures). */
  host: PrintHost & Pick<EngineHost, 'getGlyphOutlines'> & Partial<Pick<EngineHost, 'getMedia'>>;
  /** Starting options (current page, selection, active sheet …). */
  initial: PrintOptions;
  /** The selection choice is offered (slides selected, cells selected). */
  allowSelection?: boolean;
  /** Prints the job's PDF (the platform's print). */
  onPrint: (pdf: Uint8Array) => Promise<void>;
  onClose: () => void;
  fail: (e: unknown) => void;
}

/** Print settings and preview. */
export function PrintPanel({
  app,
  host,
  initial,
  allowSelection,
  onPrint,
  onClose,
  fail,
}: PrintPanelProps) {
  const { t } = useTranslation();
  const [o, setO] = useState<PrintOptions>(() => ({
    pages: 'all',
    copies: 1,
    collate: true,
    perSheet: 1,
    order: 'across',
    color: 'color',
    ...initial,
  }));
  const set = (patch: Partial<PrintOptions>) => setO((v) => ({ ...v, ...patch }));
  const job = useMemo(() => ++jobCounter, []);
  const [index, setIndex] = useState(0);
  const [info, setInfo] = useState<PrintPreview | null>(null);
  const [error, setError] = useState(false);
  const [busy, setBusy] = useState(false);
  const canvas = useRef<HTMLCanvasElement>(null);
  // The host object may be rebuilt on every render of the editor: read it through a ref.
  const hostRef = useRef(host);
  useEffect(() => {
    hostRef.current = host;
  });
  const glyphs = useMemo(
    () => new GlyphStore({ getGlyphOutlines: (f, g) => hostRef.current.getGlyphOutlines(f, g) }),
    [],
  );
  const hasMedia = !!host.getMedia;
  const images = useMemo(
    () =>
      hasMedia
        ? new ImageStore({
            getMedia: (m) => hostRef.current.getMedia?.(m) ?? Promise.resolve(new Uint8Array()),
          })
        : undefined,
    [hasMedia],
  );
  const opts = useMemo(() => jobOptions(o, app, job), [o, app, job]);
  const key = JSON.stringify(opts);

  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const r = await hostRef.current.printPreview(opts, index);
          if (!live) return;
          setError(false);
          setInfo(r);
          if (r.page) await draw(r.page);
        } catch {
          // A range that cannot be read: shown in the panel, nothing printed.
          if (live) {
            setError(true);
            setInfo(null);
          }
        }
      })();
    }, 150);
    const draw = async (page: PageDisplayList) => {
      const el = canvas.current;
      const ctx = el?.getContext('2d');
      if (!el || !ctx) return;
      await glyphs.ensure(page);
      await images?.ensure(page);
      if (!live) return;
      // Fit the sheet into 420 × 520 CSS pixels.
      const scale = Math.min(420 / page.width, 520 / page.height);
      const dpr = window.devicePixelRatio || 1;
      el.width = Math.round(page.width * scale * dpr);
      el.height = Math.round(page.height * scale * dpr);
      el.style.width = `${Math.round(page.width * scale)}px`;
      el.style.height = `${Math.round(page.height * scale)}px`;
      renderPage(ctx, page, glyphs, scale * dpr, images);
    };
    return () => {
      live = false;
      clearTimeout(timer);
    };
    // `key` stands for `opts`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, index, glyphs, images]);

  const sheets = info?.sheets ?? 0;
  const last = Math.max(0, sheets - 1);
  const shown = Math.min(index, last);

  const print = async () => {
    setBusy(true);
    try {
      await onPrint(await hostRef.current.printPdf(opts));
      onClose();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const choices: PrintPageChoice[] = [
    'all',
    'current',
    ...(allowSelection ? ['selection' as const] : []),
    'range',
  ];
  const sheetScopes: SheetPrintScope[] = ['active', 'workbook', 'selection'];

  const select = <T extends string | number>(
    id: string,
    label: string,
    value: T,
    values: readonly T[],
    text: (v: T) => string,
    onChange: (v: T) => void,
  ): ReactNode => (
    <label className={styles.field}>
      <span>{label}</span>
      <select
        data-testid={`print-${id}`}
        value={String(value)}
        onChange={(e) => {
          const v = values.find((x) => String(x) === e.target.value);
          if (v !== undefined) onChange(v);
        }}
      >
        {values.map((v) => (
          <option key={String(v)} value={String(v)}>
            {text(v)}
          </option>
        ))}
      </select>
    </label>
  );
  const check = (id: string, label: string, value: boolean, onChange: (v: boolean) => void) => (
    <label className={own.check}>
      <input
        type="checkbox"
        data-testid={`print-${id}`}
        checked={value}
        onChange={(e) => onChange(e.target.checked)}
      />
      {label}
    </label>
  );

  const doc = o.doc ?? {};
  const slide = o.slide ?? {};
  const sheet = o.sheet ?? {};
  const triValues: readonly Tri[] = ['setup', 'on', 'off'];

  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('print.title')}
      width={900}
      testId="print-panel"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('print.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="print-go"
            disabled={busy || error || sheets === 0}
            onClick={() => void print()}
          >
            {t('print.print')}
          </button>
        </>
      }
    >
      <div className={own.layout}>
        <div className={own.settings}>
          <label className={styles.field}>
            <span>{t('print.copies')}</span>
            <input
              type="number"
              min={1}
              max={999}
              data-testid="print-copies"
              value={o.copies ?? 1}
              onChange={(e) => set({ copies: Number(e.target.value) })}
            />
          </label>
          <div className={own.row}>
            {check('collate', t('print.collate'), o.collate !== false, (v) => set({ collate: v }))}
            {check('reverse', t('print.reverse'), !!o.reverse, (v) => set({ reverse: v }))}
          </div>

          <h3 className={own.heading}>{t('print.whatTitle')}</h3>
          {app === 'sheet' &&
            select(
              'sheet-scope',
              t('print.sheet.scope'),
              sheet.scope ?? 'active',
              sheetScopes,
              (v) => t(`print.sheet.scopes.${v}`),
              (v) => set({ sheet: { ...sheet, scope: v } }),
            )}
          {select(
            'pages',
            t('print.pages'),
            o.pages ?? 'all',
            app === 'sheet' ? (['all', 'current', 'range'] as PrintPageChoice[]) : choices,
            (v) => t(`print.pageChoice.${v}`),
            (v) => set({ pages: v }),
          )}
          {o.pages === 'range' && (
            <label className={styles.field}>
              <span>{t('print.range')}</span>
              <input
                data-testid="print-range"
                value={o.range ?? ''}
                placeholder={t(app === 'doc' ? 'print.rangeHintDoc' : 'print.rangeHint')}
                aria-invalid={error}
                onChange={(e) => set({ range: e.target.value })}
              />
            </label>
          )}
          {error && (
            <div className={own.error} role="alert" data-testid="print-range-error">
              {t('print.rangeError')}
            </div>
          )}
          {select(
            'subset',
            t('print.subset'),
            o.subset ?? 'all',
            ['all', 'odd', 'even'] as PrintSubset[],
            (v) => t(`print.subsets.${v}`),
            (v) => set({ subset: v }),
          )}

          {app === 'doc' && (
            <div className={own.row}>
              {check('markup', t('print.doc.markup'), doc.markup !== false, (v) =>
                set({ doc: { ...doc, markup: v } }),
              )}
              {check('properties', t('print.doc.properties'), !!doc.properties, (v) =>
                set({ doc: { ...doc, properties: v, labels: propertyLabels(t) } }),
              )}
              {check('background', t('print.doc.background'), !!doc.background, (v) =>
                set({ doc: { ...doc, background: v } }),
              )}
            </div>
          )}
          {app === 'slide' && (
            <>
              {select(
                'slide-layout',
                t('print.slide.layout'),
                slide.layout ?? 'slides',
                ['slides', 'notes', 'outline', 'handouts'] as SlidePrintLayout[],
                (v) => t(`print.slide.layouts.${v}`),
                (v) => set({ slide: { ...slide, layout: v } }),
              )}
              {slide.layout === 'handouts' && (
                <>
                  {select(
                    'handout-per-page',
                    t('print.slide.perPage'),
                    slide.handoutPerPage ?? 6,
                    HANDOUT_PER_PAGE,
                    (v) => String(v),
                    (v) => set({ slide: { ...slide, handoutPerPage: v } }),
                  )}
                  {select(
                    'handout-order',
                    t('print.order'),
                    slide.handoutOrder ?? 'horizontal',
                    ['horizontal', 'vertical'] as const,
                    (v) => t(`print.orders.${v === 'horizontal' ? 'across' : 'down'}`),
                    (v) => set({ slide: { ...slide, handoutOrder: v } }),
                  )}
                </>
              )}
              <div className={own.row}>
                {check('hidden', t('print.slide.hidden'), !!slide.hidden, (v) =>
                  set({ slide: { ...slide, hidden: v } }),
                )}
              </div>
            </>
          )}
          {app === 'sheet' && (
            <>
              {sheet.scope === 'selection' && !sheet.selection && (
                <div className={styles.hint}>{t('print.sheet.noSelection')}</div>
              )}
              {select(
                'gridlines',
                t('print.sheet.gridlines'),
                tri(sheet.gridlines),
                triValues,
                (v) => t(`print.sheet.tri.${v}`),
                (v) => set({ sheet: { ...sheet, gridlines: fromTri(v) } }),
              )}
              {select(
                'headings',
                t('print.sheet.headings'),
                tri(sheet.headings),
                triValues,
                (v) => t(`print.sheet.tri.${v}`),
                (v) => set({ sheet: { ...sheet, headings: fromTri(v) } }),
              )}
              <label className={styles.field}>
                <span>{t('print.sheet.titleRows')}</span>
                <input
                  data-testid="print-title-rows"
                  value={sheet.titleRows ?? ''}
                  placeholder="1:1"
                  onChange={(e) =>
                    set({
                      sheet: {
                        ...sheet,
                        titleRows: e.target.value.trim() ? e.target.value : undefined,
                      },
                    })
                  }
                />
              </label>
              <label className={styles.field}>
                <span>{t('print.sheet.titleCols')}</span>
                <input
                  data-testid="print-title-cols"
                  value={sheet.titleCols ?? ''}
                  placeholder="A:A"
                  onChange={(e) =>
                    set({
                      sheet: {
                        ...sheet,
                        titleCols: e.target.value.trim() ? e.target.value : undefined,
                      },
                    })
                  }
                />
              </label>
              <div className={own.row}>
                {check(
                  'ignore-area',
                  t('print.sheet.ignorePrintArea'),
                  !!sheet.ignorePrintArea,
                  (v) => set({ sheet: { ...sheet, ignorePrintArea: v } }),
                )}
              </div>
            </>
          )}

          <h3 className={own.heading}>{t('print.layoutTitle')}</h3>
          {select(
            'per-sheet',
            t('print.perSheet'),
            o.perSheet ?? 1,
            PER_SHEET,
            (v) => t('print.perSheetN', { n: v }),
            (v) => set({ perSheet: v }),
          )}
          {(o.perSheet ?? 1) > 1 &&
            select(
              'order',
              t('print.order'),
              o.order ?? 'across',
              ['across', 'down'] as const,
              (v) => t(`print.orders.${v}`),
              (v) => set({ order: v }),
            )}
          {select(
            'paper',
            t('print.paper'),
            o.paper ?? 'none',
            ['none', ...PAPERS] as const,
            (v) => t(`print.papers.${v}`),
            (v) => set({ paper: v === 'none' ? undefined : v }),
          )}
          {select(
            'color',
            t('print.color'),
            o.color ?? 'color',
            ['color', 'grayscale', 'blackWhite'] as PrintColor[],
            (v) => t(`print.colors.${v}`),
            (v) => set({ color: v }),
          )}
          <div className={own.row}>
            {check(
              'frame',
              t(app === 'slide' ? 'print.slide.frame' : 'print.frame'),
              !!o.frame,
              (v) => set({ frame: v }),
            )}
          </div>
        </div>

        <div className={own.preview}>
          <div className={own.nav}>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="print-prev"
              disabled={shown <= 0}
              onClick={() => setIndex(Math.max(0, shown - 1))}
            >
              {t('print.prev')}
            </button>
            <span data-testid="print-preview-count">
              {sheets === 0
                ? t('print.nothing')
                : t('print.sheetOf', { sheet: shown + 1, sheets, copies: info?.copies ?? 1 })}
            </span>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="print-next"
              disabled={shown >= last}
              onClick={() => setIndex(Math.min(last, shown + 1))}
            >
              {t('print.next')}
            </button>
          </div>
          <div className={own.paper}>
            <canvas
              ref={canvas}
              data-testid="print-preview-canvas"
              data-sheet={sheets ? shown : undefined}
              aria-label={t('print.previewLabel')}
            />
          </div>
        </div>
      </div>
    </Dialog>
  );
}

/** Keys of the document properties page (`bo_engine::print::DocPrint::labels`). */
export const PROPERTY_KEYS = [
  'title',
  'subject',
  'author',
  'keywords',
  'comments',
  'category',
  'template',
  'createdate',
  'savedate',
  'lastsavedby',
  'revnum',
  'filename',
  'pages',
  'words',
] as const;

function propertyLabels(t: (k: string) => string): Record<string, string> {
  return Object.fromEntries(PROPERTY_KEYS.map((k) => [k, t(`print.doc.props.${k}`)]));
}
