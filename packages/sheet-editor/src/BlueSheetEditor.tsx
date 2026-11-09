import { GlyphStore, PX_PER_PT, renderPage } from '@blueoffice/canvas-host';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import type {
  SlicerInfo,
  BorderPreset,
  BorderSpec,
  CellFormatState,
  ChartInfo,
  PasteMode,
  PivotInfo,
  Platform,
  Rejection,
  SheetCommand,
  SheetHost,
  SheetRange,
  SheetThreadInfo,
  SheetView,
  StylePatch,
  WorkbookInfo,
  RangeStats,
  SheetPermissions,
} from '@blueoffice/platform';
import {
  PASSWORD_REQUIRED,
  SHEET_PROTECTED,
  WORKBOOK_PROTECTED,
  WRONG_PASSWORD,
  WRONG_PROTECTION_PASSWORD,
} from '@blueoffice/platform';
import {
  CommentThreads,
  Ribbon,
  StatusBar,
  useResolvedTheme,
  type RibbonTab,
  type ThemeSetting,
} from '@blueoffice/ui';
import {
  AArrowDown,
  AArrowUp,
  AlignVerticalJustifyCenter,
  ArrowDownAZ,
  ArrowDownToLine,
  ArrowDownZA,
  ArrowRightToLine,
  ArrowUpDown,
  ArrowUpToLine,
  Baseline,
  Bold,
  Calendar,
  ChartColumn,
  ClipboardList,
  ClipboardPaste,
  Coins,
  Columns2,
  Columns3,
  Copy,
  Eraser,
  Eye,
  EyeOff,
  FileCog,
  FileDown,
  FilePlus,
  FolderOpen,
  Funnel,
  Grid2x2,
  Hash,
  Highlighter,
  Italic,
  KeyRound,
  ListIndentDecrease,
  ListIndentIncrease,
  ListTree,
  LocateFixed,
  Lock,
  LockOpen,
  MoveHorizontal,
  MoveVertical,
  PaintBucket,
  Paintbrush,
  Palette,
  PanelLeft,
  PanelTop,
  PencilLine,
  Percent,
  Plus,
  Printer,
  Redo2,
  RefreshCw,
  Replace,
  Rows2,
  Rows3,
  Save,
  Scissors,
  Search,
  Settings,
  Settings2,
  ShieldCheck,
  Target,
  Grid3x3,
  ListFilter,
  FunnelX,
  Calculator,
  Sigma,
  Group,
  Ungroup,
  Snowflake,
  Strikethrough,
  Table2,
  TableProperties,
  TableCellsMerge,
  TableCellsSplit,
  TextAlignCenter,
  TextAlignEnd,
  TextAlignStart,
  TextWrap,
  Trash,
  Trash2,
  Underline,
  Undo2,
  ZoomIn,
  ZoomOut,
  MessageSquareText,
} from 'lucide-react';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';
import {
  CommandRegistry,
  MacroBanner,
  PasswordDialog,
  SavePasswordPanel,
} from '@blueoffice/editor';
import { ProtectSheetDialog, UnprotectDialog } from './protection';
import {
  CfDialog,
  ChartDialog,
  ColorMenu,
  CHART_TYPES,
  FilterMenu,
  FindDialog,
  GoalSeekDialog,
  DataTableDialog,
  SolverDialog,
  GoToDialog,
  Menu,
  MessageDialog,
  PageSetupDialog,
  PasteSpecialDialog,
  SortDialog,
  ValidationDialog,
  type MenuItem,
} from './dialogs';
import { FormatCellsDialog, type NumberPreset } from './FormatCellsDialog';
import { PivotDialog, PivotPanel, pivotAt, pivotSelectionItems, usePivotLabels } from './pivot';
import {
  PivotCalcDialog,
  PivotCalcItemDialog,
  PivotFieldDialog,
  PivotMoveDialog,
  PivotOptionsDialog,
  PivotSourceDialog,
  PivotValueDialog,
  areasOf,
} from './pivot-dialogs';
import {
  advance,
  cellsOf,
  fillTarget,
  move,
  colName,
  parseA1,
  parseNumber,
  parseTsv,
  pasteCells,
  rangeA1,
  rangeOf,
  single,
  span,
  type CellPos,
  type Direction,
  type HiddenSpans,
  type Range,
  type Selection,
} from './grid';
import styles from './SheetEditor.module.css';

export interface BlueSheetEditorProps {
  platform: Platform;
  language?: Language;
  /** Light, dark or follow the OS (the embedding host sets it, prompt §6A.2). */
  theme?: ThemeSetting;
}

/** Editor state passed to commands. */
export interface SheetEditorState {
  host: SheetHost;
  sheet: number;
  selection: Selection;
  hasWorkbook: boolean;
  /** Formatting of the active cell (ribbon toggles). */
  fmt: CellFormatState | null;
  /** Selected chart (click on a chart). */
  chart: number | null;
  /** Pivot table containing the active cell. */
  pivot: number | null;
  /** Every range of a multi-range (Ctrl) selection, the current one last. */
  ranges: Range[];
  /** The current sheet is protected (§18.3.1.85). */
  sheetProtected: boolean;
  /** The workbook structure is locked (§18.2.29). */
  structureLocked: boolean;
}

/** A file waiting for its password ([MS-OFFCRYPTO]). */
interface PendingFile {
  bytes: Uint8Array;
  name: string;
  path?: string;
}

type SecurityPanel =
  | { kind: 'savePassword' }
  | { kind: 'protect'; workbook: boolean }
  | { kind: 'unprotect'; workbook: boolean; wrong: boolean };

const XLSX_MIME = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';
const ZOOMS = [0.5, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 2];
/** Selections larger than this skip the status-bar statistics. */
const STATS_LIMIT = 2_000;

/** Preview colours of the cell style gallery (fill, text). */
const STYLE_PREVIEW: Record<string, [string, string, boolean]> = {
  Normal: ['FFFFFF', '1B1F24', false],
  Good: ['D8F3E3', '1E6B3C', false],
  Bad: ['FADBD8', '9B1C1C', false],
  Neutral: ['FDF0C4', '7A5A00', false],
  Title: ['FFFFFF', '1F3A5F', true],
  'Heading 1': ['FFFFFF', '1F3A5F', true],
  'Heading 2': ['FFFFFF', '1F3A5F', true],
  Total: ['FFFFFF', '1B1F24', true],
  Note: ['FFF8D6', '1B1F24', false],
};

/** Wall-clock time (recent files). */
const nowMs = () => Date.now();

const toRange = (r: Range): SheetRange => ({ r0: r.r0, c0: r.c0, r1: r.r1, c1: r.c1 });
const norm = (s: string) => s.replace(/\r\n?/g, '\n').replace(/\n$/, '');

/** Numbers workbook loads; only the latest started one is shown. */
class LoadTickets {
  private n = 0;
  next(): number {
    this.n += 1;
    return this.n;
  }
  /** Any load still running is not shown any more. */
  cancel(): void {
    this.n += 1;
  }
  isLatest(ticket: number): boolean {
    return ticket === this.n;
  }
}

export function BlueSheetEditor(props: BlueSheetEditorProps) {
  const [i18n] = useState(() => createI18n(props.language));
  return (
    <I18nextProvider i18n={i18n}>
      <SheetEditorInner {...props} />
    </I18nextProvider>
  );
}

interface EditState {
  pos: CellPos;
  text: string;
  /** Started with F2 / double-click: arrows move the caret instead of the cell. */
  inPlace: boolean;
}

type DialogState =
  | { kind: 'format' }
  | { kind: 'sort' }
  | { kind: 'cf' }
  | { kind: 'dv' }
  | { kind: 'goalSeek' }
  | { kind: 'dataTable' }
  | { kind: 'solver' }
  | { kind: 'chart'; index: number | null }
  | { kind: 'find'; replace: boolean }
  | { kind: 'goto' }
  | { kind: 'pasteSpecial' }
  | { kind: 'pageSetup' }
  | { kind: 'pivot' }
  | { kind: 'pivotField'; pivot: PivotInfo; field: number }
  | { kind: 'pivotValue'; pivot: PivotInfo; idx: number }
  | { kind: 'pivotOptions'; pivot: PivotInfo }
  | { kind: 'pivotCalc'; pivot: PivotInfo }
  | { kind: 'pivotCalcItem'; pivot: PivotInfo }
  | { kind: 'pivotMove'; pivot: PivotInfo }
  | { kind: 'pivotSource'; pivot: PivotInfo }
  | { kind: 'message'; title: string; message: string };

type MenuState = { kind: string; x: number; y: number };

type Drag =
  | { kind: 'select' }
  | { kind: 'fill'; source: Range }
  | { kind: 'resize'; rows: boolean; index: number; start: number; size: number }
  /** Moving (or, with 
esize, sizing) a chart: anchors in cells. */
  | {
      kind: 'chart';
      index: number;
      resize: boolean;
      start: CellPos;
      from: [number, number];
      to: [number, number];
    };

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

function SheetEditorInner({ platform, theme: themeSetting = 'auto' }: BlueSheetEditorProps) {
  const { t, i18n } = useTranslation();
  const host = platform.sheet;
  const theme = useResolvedTheme(themeSetting);
  const decimal: ',' | '.' = i18n.language === 'en-US' ? '.' : ',';

  const [info, setInfo] = useState<WorkbookInfo | null>(null);
  const [fileName, setFileName] = useState('');
  const [sheet, setSheet] = useState(0);
  const [selection, setSelection] = useState<Selection>(single({ row: 0, col: 0 }));
  const [scroll, setScroll] = useState({ x: 0, y: 0 });
  const [size, setSize] = useState({ w: 800, h: 500 });
  const [content, setContent] = useState({ w: 2000, h: 4000 });
  const [zoom, setZoom] = useState(1);
  const [edit, setEdit] = useState<EditState | null>(null);
  const [barText, setBarText] = useState('');
  const [selRect, setSelRect] = useState<Rect | null>(null);
  const [activeRect, setActiveRect] = useState<Rect | null>(null);
  const [status, setStatus] = useState('');
  /** Cells converted from TCVN3 / VNI fonts to Unicode on open (0 = no notice). */
  const [legacyNotice, setLegacyNotice] = useState(0);
  const [stats, setStats] = useState<{
    key: string;
    sum: number;
    count: number;
    numbers: number;
  } | null>(null);
  const [renaming, setRenaming] = useState<number | null>(null);
  const [revision, setRevision] = useState(0);
  const [loadSeq, setLoadSeq] = useState(0);
  const [fmt, setFmt] = useState<CellFormatState | null>(null);
  const [presets, setPresets] = useState<{ numberFormats: NumberPreset[]; cellStyles: string[] }>({
    numberFormats: [],
    cellStyles: [],
  });
  const [dialog, setDialog] = useState<DialogState | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [filterMenu, setFilterMenu] = useState<{ col: number; x: number; y: number } | null>(null);
  const [dvList, setDvList] = useState<string[] | null>(null);
  const [dvOpen, setDvOpen] = useState(false);
  /** The active cell has a data validation rule (inputs are checked). */
  const [hasRule, setHasRule] = useState(false);
  const [painter, setPainter] = useState<Range | null>(null);
  const [fillRect, setFillRect] = useState<Rect | null>(null);
  /** Selected chart: its anchors and on-screen box (px). */
  const [chartInfo, setChartInfo] = useState<ChartInfo | null>(null);
  const [chartBox, setChartBox] = useState<Rect | null>(null);
  const chartTarget = useRef<{ from: [number, number]; to: [number, number] } | null>(null);
  const [guide, setGuide] = useState<{ x?: number; y?: number } | null>(null);
  const [cursor, setCursor] = useState('cell');
  const [chart, setChart] = useState<number | null>(null);
  const [pivots, setPivots] = useState<{ sheet: number; list: PivotInfo[] }>({
    sheet: -1,
    list: [],
  });
  /** The field list shows when the active cell is inside a pivot table. */
  const [pivotPanel, setPivotPanel] = useState(true);
  /** Threaded comments side panel ([MS-XLSX] threaded comments). */
  const [commentsOpen, setCommentsOpen] = useState(false);
  const [threads, setThreads] = useState<SheetThreadInfo[]>([]);
  const pivotLabels = usePivotLabels();

  const scrollerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const glyphs = useMemo(() => (host ? new GlyphStore(host) : null), [host]);
  const renderSeq = useRef(0);
  const drag = useRef<Drag | null>(null);
  const fillTo = useRef<Range | null>(null);
  const composing = useRef(false);
  /** TSV text of our last copy (paste uses the internal cell records when it matches). */
  const [lastCopy, setLastCopy] = useState<string | null>(null);
  /** Other ranges of a Ctrl multi-range selection (the current one is `selection`). */
  const [extra, setExtra] = useState<Range[]>([]);
  const [extraRects, setExtraRects] = useState<Rect[]>([]);
  const [multiStats, setMultiStats] = useState<(RangeStats & { key: string }) | null>(null);
  const [passwordPrompt, setPasswordPrompt] = useState<{
    file: PendingFile;
    wrong: boolean;
  } | null>(null);
  const [security, setSecurity] = useState<SecurityPanel | null>(null);
  const [tabMenu, setTabMenu] = useState<{ sheet: number; x: number; y: number } | null>(null);
  const [dragTab, setDragTab] = useState<number | null>(null);
  const [macroBanner, setMacroBanner] = useState(false);
  const hovering = useRef(false);

  const scale = PX_PER_PT * zoom;
  const view: SheetView = useMemo(
    () => ({
      sheet,
      scrollX: scroll.x,
      scrollY: scroll.y,
      width: size.w / scale,
      height: size.h / scale,
    }),
    [sheet, scroll, size, scale],
  );
  const summary = info?.sheets[sheet];
  const hidden: HiddenSpans = useMemo(
    () => ({ rows: summary?.hiddenRows ?? [], cols: summary?.hiddenCols ?? [] }),
    [summary],
  );

  const fail = useCallback(
    (e: unknown) => {
      const message = e instanceof Error ? e.message : String(e);
      // Refusals of sheet / workbook protection get their own message.
      if (message.includes(SHEET_PROTECTED)) setStatus(t('sheetEditor.protection.refused'));
      else if (message.includes(WORKBOOK_PROTECTED))
        setStatus(t('sheetEditor.protection.workbookRefused'));
      else if (message.includes(WRONG_PROTECTION_PASSWORD))
        setStatus(t('sheetEditor.protection.wrongPassword'));
      else setStatus(t('sheetEditor.error', { message }));
    },
    [t],
  );

  // ------------------------------------------------------------- loading

  const loaded = useCallback(
    (wb: WorkbookInfo, name: string) => {
      setInfo(wb);
      setLegacyNotice(wb.legacyConverted ?? 0);
      setFileName(name);
      setSheet(0);
      setSelection(single({ row: 0, col: 0 }));
      setScroll({ x: 0, y: 0 });
      setChart(null);
      setPainter(null);
      setLastCopy(null);
      setExtra([]);
      setMacroBanner(!!wb.macros);
      setLoadSeq((n) => n + 1);
      setRevision((r) => r + 1);
      setStatus('');
      platform.window.setTitle(`${name} — ${t('app.sheet')}`);
    },
    [platform, t],
  );

  /**
   * Opens a workbook (file or new). Only the most recently *started* load is
   * shown: a slower earlier one (the start-up blank workbook, a previous file)
   * must not reset the selection of the file the user opened afterwards.
   */
  const [loadTicket] = useState(() => new LoadTickets());
  // Busy from mount until the start-up workbook (or a file) is shown.
  const [loading, setLoading] = useState(true);
  const load = useCallback(
    async (start: () => Promise<WorkbookInfo>, name: string) => {
      const ticket = loadTicket.next();
      try {
        await null;
        setLoading(true);
        const wb = await start();
        if (loadTicket.isLatest(ticket)) loaded(wb, name);
      } finally {
        if (loadTicket.isLatest(ticket)) setLoading(false);
      }
    },
    [loaded, loadTicket],
  );

  /** Opens a file; an encrypted one ([MS-OFFCRYPTO]) asks for its password. */
  const openFile = useCallback(
    async (f: PendingFile, password?: string): Promise<boolean> => {
      if (!host) return false;
      try {
        await load(() => host.open(f.bytes, f.name, password), f.name);
        return true;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes(PASSWORD_REQUIRED) || msg.includes(WRONG_PASSWORD)) {
          setStatus('');
          setPasswordPrompt({ file: f, wrong: msg.includes(WRONG_PASSWORD) });
          return false;
        }
        throw e;
      }
    },
    [host, load],
  );

  useEffect(() => {
    if (!host) return;
    void load(
      () => host.newWorkbook(`${t('sheetEditor.untitled')}.xlsx`),
      `${t('sheetEditor.untitled')}.xlsx`,
    ).catch(fail);
    let cancelled = false;
    void host
      .exec<{ numberFormats: NumberPreset[]; cellStyles: string[] }>({ cmd: 'presets' })
      .then((p) => !cancelled && p && setPresets(p))
      .catch(() => {});
    const un = platform.shell.onOpenRequest((f) => {
      void openFile(f).catch(fail);
    });
    return () => {
      cancelled = true;
      // A load still running for this host is not shown any more.
      loadTicket.cancel();
      un();
    };
    // Only on mount / host change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host]);

  // ------------------------------------------------------------- rendering

  useEffect(() => {
    scrollerRef.current?.scrollTo(0, 0);
    // Ready for typing straight away (IME included).
    if (loadSeq > 0) inputRef.current?.focus({ preventScroll: true });
  }, [loadSeq]);

  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    if (!host || !info) return;
    void host
      .contentSize(sheet)
      .then(([w, h]) => setContent({ w, h }))
      .catch(() => {});
  }, [host, info, sheet, revision]);

  useEffect(() => {
    if (!host || !glyphs || !info) return;
    const seq = ++renderSeq.current;
    const frame = requestAnimationFrame(() => {
      void (async () => {
        const page = await host.render(view);
        await glyphs.ensure(page);
        if (seq !== renderSeq.current) return;
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (!canvas || !ctx) return;
        const dpr = window.devicePixelRatio || 1;
        canvas.width = Math.round(size.w * dpr);
        canvas.height = Math.round(size.h * dpr);
        canvas.style.width = `${size.w}px`;
        canvas.style.height = `${size.h}px`;
        renderPage(ctx, page, glyphs, scale * dpr);
        canvas.dataset.rendered = String(seq);
      })().catch(fail);
    });
    return () => cancelAnimationFrame(frame);
  }, [host, glyphs, info, view, size, scale, revision, fail]);

  const px = useCallback(
    (x: Rect | null) => x && { x: x.x * scale, y: x.y * scale, w: x.w * scale, h: x.h * scale },
    [scale],
  );

  // Selection overlay + formula bar contents.
  useEffect(() => {
    if (!host || !info) return;
    const r = rangeOf(selection);
    let cancelled = false;
    void (async () => {
      const [rect, act, cell] = await Promise.all([
        host.rangeRect(view, r.r0, r.c0, r.r1, r.c1),
        host.rangeRect(
          view,
          selection.active.row,
          selection.active.col,
          selection.active.row,
          selection.active.col,
        ),
        host.cell(sheet, selection.active.row, selection.active.col),
      ]);
      if (cancelled) return;
      setSelRect(px(rect));
      setActiveRect(px(act));
      if (!edit) setBarText(cell.input);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, selection, view, px, revision, edit]);

  // Overlays of the other ranges of a multi-range selection.
  useEffect(() => {
    if (!host || !info || extra.length === 0) return;
    let cancelled = false;
    void Promise.all(extra.map((r) => host.rangeRect(view, r.r0, r.c0, r.r1, r.c1)))
      .then((rects) => {
        if (!cancelled) setExtraRects(rects.map(px).filter((x): x is Rect => !!x));
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, extra, view, px, revision]);

  // Status-bar figures of a multi-range selection (computed by the engine).
  useEffect(() => {
    if (!host || !info || extra.length === 0) return;
    const ranges = [...extra, rangeOf(selection)].map(toRange);
    const key = `${sheet}:${JSON.stringify(ranges)}:${revision}`;
    let cancelled = false;
    void host
      .exec<RangeStats>({ cmd: 'rangeStats', sheet, ranges })
      .then((st) => !cancelled && st && setMultiStats({ ...st, key }))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, extra, selection, revision]);

  // Formatting + validation of the active cell (ribbon state, dropdown).
  useEffect(() => {
    if (!host || !info) return;
    let cancelled = false;
    const { row, col } = selection.active;
    void (async () => {
      const [f, dv] = await Promise.all([
        host.exec<CellFormatState>({ cmd: 'formatState', sheet, row, col }),
        host.exec<{ list: string[] | null; kind?: string | null }>({
          cmd: 'validation',
          sheet,
          row,
          col,
        }),
      ]);
      if (cancelled) return;
      setFmt(f ?? null);
      setDvList(dv?.list ?? null);
      setHasRule(!!dv?.kind);
      setDvOpen(false);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, selection.active, revision]);

  // Selected chart: anchors and selection box.
  useEffect(() => {
    if (!host || chart === null) return;
    let cancelled = false;
    void (async () => {
      const list = await host.exec<ChartInfo[]>({ cmd: 'charts', sheet });
      const c = list?.[chart];
      if (cancelled || !c) return;
      const r = await host.rangeRect(view, c.from[0], c.from[1], c.to[0] - 1, c.to[1] - 1);
      if (cancelled) return;
      setChartInfo(c);
      setChartBox(px(r));
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, chart, sheet, view, px, revision]);

  // Slicers of the sheet: item buttons over the grid.
  const [slicers, setSlicers] = useState<{ info: SlicerInfo; rect: Rect }[]>([]);
  useEffect(() => {
    if (!host) return;
    let cancelled = false;
    void (async () => {
      const list = (await host.exec<SlicerInfo[]>({ cmd: 'slicers', sheet })) ?? [];
      const placed = await Promise.all(
        list.map(async (info) => {
          const r = await host.rangeRect(
            view,
            info.from[0],
            info.from[1],
            info.to[0] - 1,
            info.to[1] - 1,
          );
          const rect = px(r);
          return rect ? { info, rect } : null;
        }),
      );
      if (!cancelled) setSlicers(placed.filter((x): x is { info: SlicerInfo; rect: Rect } => !!x));
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, sheet, view, px, revision]);

  // Pivot tables of the sheet (field list panel).
  useEffect(() => {
    if (!host || !info?.sheets[sheet]?.pivots) return;
    let cancelled = false;
    void host
      .exec<PivotInfo[]>({ cmd: 'pivots', sheet })
      .then((list) => !cancelled && setPivots({ sheet, list: list ?? [] }))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, revision]);
  const activePivot =
    summary?.pivots && pivots.sheet === sheet
      ? pivotAt(pivots.list, selection.active.row, selection.active.col)
      : null;

  // Status-bar statistics of the selection.
  useEffect(() => {
    if (!host || !info) return;
    const r = rangeOf(selection);
    if (cellsOf(r) <= 1 || cellsOf(r) > STATS_LIMIT) return;
    const key = `${sheet}:${rangeA1(r)}:${revision}`;
    let cancelled = false;
    void (async () => {
      const lastRow = info.sheets[sheet]?.lastRow ?? -1;
      const lastCol = info.sheets[sheet]?.lastCol ?? -1;
      let sum = 0;
      let count = 0;
      let numbers = 0;
      const jobs: Promise<void>[] = [];
      for (let row = r.r0; row <= Math.min(r.r1, lastRow); row++) {
        for (let col = r.c0; col <= Math.min(r.c1, lastCol); col++) {
          jobs.push(
            host.cell(sheet, row, col).then((c) => {
              if (c.display === '') return;
              count++;
              const n = c.input.startsWith('=')
                ? parseNumber(c.display, decimal)
                : parseNumber(c.input, decimal);
              if (n !== null) {
                sum += n;
                numbers++;
              }
            }),
          );
        }
      }
      await Promise.all(jobs);
      if (!cancelled) setStats({ key, sum, count, numbers });
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, selection, revision, decimal]);

  // Keep the hidden/visible cell input focused so IME composition starts in it.
  const focusInput = useCallback(() => {
    // Right away, so keys typed straight after (Name Box Enter, a click) reach the
    // grid even when animation frames are delayed (background tab); again on the
    // next frame for an input that is only rendered by this update.
    inputRef.current?.focus({ preventScroll: true });
    requestAnimationFrame(() => inputRef.current?.focus({ preventScroll: true }));
  }, []);

  // ------------------------------------------------------------- editing

  const refreshInfo = useCallback(async () => {
    if (!host) return;
    setInfo(await host.info());
    setRevision((r) => r + 1);
  }, [host]);

  // Collaborators' edits arrive through the host: redraw.
  useEffect(
    () => host?.onExternalChange?.(() => void refreshInfo().catch(() => {})),
    [host, refreshInfo],
  );

  const exec = useCallback(
    async <T,>(cmd: SheetCommand, refresh = true): Promise<T | null> => {
      if (!host) return null;
      try {
        const r = await host.exec<T>(cmd);
        if (refresh) await refreshInfo();
        return r;
      } catch (e) {
        fail(e);
        return null;
      }
    },
    [host, refreshInfo, fail],
  );

  const chooseSlicerItem = useCallback(
    (info: SlicerInfo, label: string, multi: boolean) => {
      const chosen = info.items.filter((i) => i.selected).map((i) => i.label);
      const all = chosen.length === info.items.length;
      let next: string[];
      if (multi) {
        next = all
          ? [label]
          : chosen.includes(label)
            ? chosen.filter((l) => l !== label)
            : [...chosen, label];
      } else {
        next = !all && chosen.length === 1 && chosen[0] === label ? [] : [label];
      }
      void exec({ cmd: 'slicerSelect', sheet, index: info.index, items: next });
    },
    [exec, sheet],
  );

  // ------------------------------------------------------------- comments

  const [threadsRev, setThreadsRev] = useState(0);
  useEffect(() => {
    if (!host || !commentsOpen) return;
    let cancelled = false;
    void host
      .exec<SheetThreadInfo[]>({ cmd: 'comments', sheet })
      .then((r) => !cancelled && setThreads(r ?? []))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, sheet, commentsOpen, revision, threadsRev]);
  /** Runs a comment command (one undo step) and reloads the threads. */
  const commentCmd = useCallback(
    async (cmd: SheetCommand) => {
      await exec(cmd);
      setThreadsRev((n) => n + 1);
    },
    [exec],
  );
  const commentLabels = useMemo(
    () => ({
      title: t('sheetEditor.comments.title'),
      empty: t('sheetEditor.comments.empty'),
      newComment: t('sheetEditor.comments.new'),
      newPlaceholder: t('sheetEditor.comments.newPlaceholder'),
      reply: t('sheetEditor.comments.reply'),
      replyPlaceholder: t('sheetEditor.comments.replyPlaceholder'),
      resolve: t('sheetEditor.comments.resolve'),
      reopen: t('sheetEditor.comments.reopen'),
      delete: t('sheetEditor.comments.delete'),
      close: t('sheetEditor.comments.close'),
      resolved: t('sheetEditor.comments.resolved'),
    }),
    [t],
  );

  const commit = useCallback(
    async (next?: Selection) => {
      if (!host || !edit) {
        if (next) setSelection(next);
        return;
      }
      const e = edit;
      setEdit(null);
      if (inputRef.current) inputRef.current.value = '';
      // Move first so typing that follows lands in the next cell.
      if (next) setSelection(next);
      try {
        if (hasRule) {
          // Cells with a validation rule are checked by the engine (§18.3.1.32).
          const r = await host.exec<{ rejected: Rejection | null; warning: Rejection | null }>({
            cmd: 'setInputs',
            sheet,
            cells: [[e.pos.row, e.pos.col, e.text]],
            validate: true,
          });
          const rej = r?.rejected ?? r?.warning;
          if (rej) {
            setDialog({
              kind: 'message',
              title: rej.title || t('sheetEditor.dv.invalidTitle'),
              message: rej.message || t('sheetEditor.dv.invalidMessage'),
            });
          }
          if (r?.rejected) {
            setSelection(single(e.pos));
            setEdit(e);
            setBarText(e.text);
            if (inputRef.current) inputRef.current.value = e.text;
            return;
          }
        } else {
          await host.setInputs(sheet, [[e.pos.row, e.pos.col, e.text]]);
        }
        await refreshInfo();
      } catch (err) {
        fail(err);
      }
      focusInput();
    },
    [host, edit, sheet, refreshInfo, fail, focusInput, hasRule, t],
  );

  const cancelEdit = useCallback(() => {
    setEdit(null);
    if (inputRef.current) inputRef.current.value = '';
    focusInput();
  }, [focusInput]);

  const startEdit = useCallback(
    async (inPlace: boolean, initial?: string) => {
      if (!host) return;
      const pos = selection.active;
      const text = initial ?? (await host.cell(sheet, pos.row, pos.col)).input;
      setEdit({ pos, text, inPlace });
      setBarText(text);
      if (inputRef.current) inputRef.current.value = text;
      focusInput();
    },
    [host, selection, sheet, focusInput],
  );

  /** Makes the active cell visible by adjusting the scroll position. */
  const reveal = useCallback(
    async (s: Selection) => {
      if (!host) return;
      const r = await host.rangeRect(view, s.active.row, s.active.col, s.active.row, s.active.col);
      const el = scrollerRef.current;
      if (!el) return;
      if (!r) {
        // Off-screen: jump roughly (rows ~ 15pt, cols ~ 64pt).
        el.scrollTo({ left: s.active.col * 48 * scale, top: s.active.row * 15 * scale });
        return;
      }
      const x = r.x * scale;
      const y = r.y * scale;
      const w = r.w * scale;
      const h = r.h * scale;
      if (x + w > size.w) el.scrollLeft += x + w - size.w + 4;
      else if (x < 40) el.scrollLeft = Math.max(0, el.scrollLeft - (40 - x));
      if (y + h > size.h) el.scrollTop += y + h - size.h + 4;
      else if (y < 20) el.scrollTop = Math.max(0, el.scrollTop - (20 - y));
    },
    [host, view, scale, size],
  );

  const select = useCallback(
    (s: Selection) => {
      setSelection(s);
      setExtra([]);
      setChart(null);
      void reveal(s);
    },
    [reveal],
  );

  /** Every range of the selection (multi-range with Ctrl), the current one last. */
  const allRanges = useMemo(() => [...extra, rangeOf(selection)], [extra, selection]);

  const clearSelection = useCallback(async () => {
    if (!host) return;
    const cells: [number, number, string][] = [];
    const seen = new Set<string>();
    for (const r of allRanges) {
      const lastRow = Math.min(r.r1, info?.sheets[sheet]?.lastRow ?? r.r1);
      const lastCol = Math.min(r.c1, info?.sheets[sheet]?.lastCol ?? r.c1);
      for (let row = r.r0; row <= lastRow; row++)
        for (let col = r.c0; col <= lastCol; col++) {
          const k = `${row}:${col}`;
          if (seen.has(k)) continue;
          seen.add(k);
          cells.push([row, col, '']);
        }
    }
    if (cells.length) {
      await host.setInputs(sheet, cells);
      await refreshInfo();
    }
  }, [host, allRanges, info, sheet, refreshInfo]);

  const copy = useCallback(
    async (cut: boolean) => {
      if (!host) return;
      const r = rangeOf(selection);
      if (cellsOf(r) > 1_000_000) return;
      const res = await host.exec<{ text: string }>({
        cmd: 'copy',
        sheet,
        range: toRange(r),
        cut,
      });
      const text = res?.text ?? '';
      setLastCopy(text);
      await platform.clipboard.writeText(text);
      setStatus(cut ? t('sheetEditor.cutDone') : t('sheetEditor.copied'));
    },
    [host, selection, sheet, platform, t],
  );

  const internalPaste = useCallback(
    async (mode: PasteMode, transpose: boolean, skipBlanks: boolean): Promise<boolean> => {
      if (!host) return false;
      const r = rangeOf(selection);
      const res = await host.exec<{ range: SheetRange } | null>({
        cmd: 'paste',
        sheet,
        row: r.r0,
        col: r.c0,
        mode,
        transpose,
        skipBlanks,
        range: toRange(r),
      });
      if (!res) return false;
      setSelection(
        span({ row: res.range.r0, col: res.range.c0 }, { row: res.range.r1, col: res.range.c1 }),
      );
      await refreshInfo();
      return true;
    },
    [host, selection, sheet, refreshInfo],
  );

  const paste = useCallback(async () => {
    if (!host) return;
    const text = await platform.clipboard.readText();
    // Our own copy: paste the cell records (formats, adjusted formulas).
    if (lastCopy !== null && norm(text) === norm(lastCopy)) {
      if (await internalPaste('all', false, false)) return;
    }
    const rows = parseTsv(text);
    if (!rows.length) return;
    const r = rangeOf(selection);
    await host.setInputs(sheet, pasteCells({ row: r.r0, col: r.c0 }, rows));
    const lastRow = r.r0 + rows.length - 1;
    const lastCol = r.c0 + Math.max(...rows.map((x) => x.length)) - 1;
    setSelection(span({ row: r.r0, col: r.c0 }, { row: lastRow, col: lastCol }));
    await refreshInfo();
  }, [host, platform, selection, sheet, refreshInfo, internalPaste, lastCopy]);

  /** One command per range of the selection, as one undo step when there are several. */
  const onAllRanges = useCallback(
    (make: (range: SheetRange) => SheetCommand) => {
      const cmds = allRanges.map((r) => make(toRange(r)));
      return exec(cmds.length === 1 ? cmds[0]! : { cmd: 'batch', cmds });
    },
    [exec, allRanges],
  );

  const formatSel = useCallback(
    (patch: StylePatch) => onAllRanges((range) => ({ cmd: 'format', sheet, range, patch })),
    [onAllRanges, sheet],
  );

  const borderSel = useCallback(
    (spec: BorderSpec) => onAllRanges((range) => ({ cmd: 'borders', sheet, range, spec })),
    [onAllRanges, sheet],
  );

  /** Column (or row) span of the selection when it covers whole columns (rows). */
  const axisSpan = useCallback(
    (rows: boolean): [number, number] => {
      const r = rangeOf(selection);
      return rows ? [r.r0, r.r1] : [r.c0, r.c1];
    },
    [selection],
  );

  const anchorOf = (id: string): { x: number; y: number } => {
    const el = document.querySelector(`[data-command="${id}"]`);
    const b = el?.getBoundingClientRect();
    return b ? { x: b.left, y: b.bottom + 2 } : { x: 80, y: 120 };
  };

  // ------------------------------------------------------------- commands

  const registry = useMemo(() => {
    const reg = new CommandRegistry<SheetEditorState>();
    const guard = async (
      st: SheetEditorState,
      f: (s: SheetEditorState) => Promise<unknown>,
    ): Promise<void> => {
      try {
        await f(st);
      } catch (e) {
        fail(e);
      }
    };
    const rng = (s: SheetEditorState) => toRange(rangeOf(s.selection));
    const add = (
      id: string,
      run: (s: SheetEditorState) => Promise<unknown>,
      extra: Partial<{
        shortcut: string;
        isEnabled: (s: SheetEditorState) => boolean;
        isChecked: (s: SheetEditorState) => boolean;
      }> = {},
    ) => reg.register({ id, run: (st) => guard(st, run), ...extra });
    const open = (d: DialogState) => async () => setDialog(d);
    const popup = (kind: string, id: string) => async () => setMenu({ kind, ...anchorOf(id) });

    // File
    add('file.new', async (s) => {
      const name = `${t('sheetEditor.untitled')}.xlsx`;
      await load(() => s.host.newWorkbook(name), name);
    });
    add(
      'file.open',
      async () => {
        const f = await platform.files.openFile([
          '.xlsx',
          '.xlsm',
          '.xlsb',
          '.xls',
          '.ods',
          '.csv',
        ]);
        if (!f) return;
        setStatus(t('sheetEditor.loading'));
        if (!(await openFile(f))) return;
        await platform.shell.addRecentFile({
          name: f.name,
          path: f.path ?? f.name,
          openedAt: nowMs(),
        });
      },
      { shortcut: 'Ctrl+O' },
    );
    const baseName = () => fileName.replace(/\.(xlsx|xlsm|csv)$/i, '') || t('sheetEditor.untitled');
    const has = { isEnabled: (s: SheetEditorState) => s.hasWorkbook };
    add(
      'file.save',
      async (s) => {
        const bytes = await s.host.save('xlsx', s.sheet);
        const saved = await platform.files.saveFile(bytes, `${baseName()}.xlsx`, XLSX_MIME);
        if (saved) setStatus(t('sheetEditor.saved', { name: saved }));
      },
      { shortcut: 'Ctrl+S', ...has },
    );
    add(
      'file.saveCsv',
      async (s) => {
        const bytes = await s.host.save('csv', s.sheet);
        const saved = await platform.files.saveFile(bytes, `${baseName()}.csv`, 'text/csv');
        if (saved) setStatus(t('sheetEditor.saved', { name: saved }));
      },
      has,
    );
    add(
      'file.exportPdf',
      async (s) => {
        const pdf = await s.host.exportPdf();
        await platform.files.saveFile(pdf, `${baseName()}.pdf`, 'application/pdf');
      },
      has,
    );
    add('file.print', async (s) => platform.print.printPdf(await s.host.exportPdf(), baseName()), {
      shortcut: 'Ctrl+P',
      ...has,
    });

    // Edit / clipboard
    add(
      'edit.undo',
      async (s) => {
        if (await s.host.undo()) await refreshInfo();
      },
      { shortcut: 'Ctrl+Z' },
    );
    // TCVN3 / VNI cells → Unicode (also done once on open); one undo step.
    add('edit.convertLegacyText', async (s) => {
      const r = await s.host.exec<{ converted: number }>({ cmd: 'convertLegacyText' });
      if (r && r.converted > 0) {
        setLegacyNotice(r.converted);
        await refreshInfo();
      }
    });
    add(
      'edit.redo',
      async (s) => {
        if (await s.host.redo()) await refreshInfo();
      },
      { shortcut: 'Ctrl+Y' },
    );
    add('edit.copy', () => copy(false), { shortcut: 'Ctrl+C' });
    add('edit.cut', () => copy(true), { shortcut: 'Ctrl+X' });
    add('edit.paste', paste, { shortcut: 'Ctrl+V' });
    add('edit.pasteSpecial', open({ kind: 'pasteSpecial' }), { shortcut: 'Ctrl+Alt+V' });
    add(
      'edit.clear',
      async (s) => {
        if (s.chart !== null) {
          await exec({ cmd: 'deleteChart', sheet: s.sheet, index: s.chart });
          setChart(null);
          return;
        }
        await clearSelection();
      },
      { shortcut: 'Delete' },
    );
    add(
      'edit.fillDown',
      (s) => exec({ cmd: 'fillDir', sheet: s.sheet, range: rng(s), dir: 'down' }),
      {
        shortcut: 'Ctrl+D',
      },
    );
    add(
      'edit.fillRight',
      (s) => exec({ cmd: 'fillDir', sheet: s.sheet, range: rng(s), dir: 'right' }),
      {
        shortcut: 'Ctrl+R',
      },
    );
    add('edit.find', open({ kind: 'find', replace: false }), { shortcut: 'Ctrl+F' });
    add('edit.replace', open({ kind: 'find', replace: true }), { shortcut: 'Ctrl+H' });
    add('edit.goto', open({ kind: 'goto' }), { shortcut: 'Ctrl+G' });

    // Font
    const toggle = (key: 'bold' | 'italic' | 'strike') => (s: SheetEditorState) =>
      formatSel({ [key]: !(s.fmt?.[key] ?? false) });
    add('format.bold', toggle('bold'), { shortcut: 'Ctrl+B', isChecked: (s) => !!s.fmt?.bold });
    add('format.italic', toggle('italic'), {
      shortcut: 'Ctrl+I',
      isChecked: (s) => !!s.fmt?.italic,
    });
    add('format.underline', (s) => formatSel({ underline: s.fmt?.underline ? 'none' : 'single' }), {
      shortcut: 'Ctrl+U',
      isChecked: (s) => !!s.fmt?.underline,
    });
    add('format.strike', toggle('strike'), {
      shortcut: 'Ctrl+5',
      isChecked: (s) => !!s.fmt?.strike,
    });
    add('format.fontGrow', (s) =>
      formatSel({ fontSize: Math.min(409, (s.fmt?.fontSize ?? 11) + 1) }),
    );
    add('format.fontShrink', (s) =>
      formatSel({ fontSize: Math.max(1, (s.fmt?.fontSize ?? 11) - 1) }),
    );
    add('format.fontColor', popup('fontColor', 'format.fontColor'));
    add('format.fillColor', popup('fillColor', 'format.fillColor'));
    add('format.borders', popup('borders', 'format.borders'));
    // Alignment
    const align = (h: string) => (s: SheetEditorState) =>
      formatSel({ hAlign: s.fmt?.hAlign === h ? '' : h });
    const valign = (v: string) => () => formatSel({ vAlign: v });
    add('format.alignLeft', align('left'), { isChecked: (s) => s.fmt?.hAlign === 'left' });
    add('format.alignCenter', align('center'), { isChecked: (s) => s.fmt?.hAlign === 'center' });
    add('format.alignRight', align('right'), { isChecked: (s) => s.fmt?.hAlign === 'right' });
    add('format.alignTop', valign('top'), { isChecked: (s) => s.fmt?.vAlign === 'top' });
    add('format.alignMiddle', valign('center'), { isChecked: (s) => s.fmt?.vAlign === 'center' });
    add('format.alignBottom', valign(''), { isChecked: (s) => !!s.fmt && !s.fmt.vAlign });
    add('format.wrap', (s) => formatSel({ wrap: !s.fmt?.wrap }), {
      isChecked: (s) => !!s.fmt?.wrap,
    });
    add('format.indentInc', (s) => formatSel({ indent: (s.fmt?.indent ?? 0) + 1 }));
    add('format.indentDec', (s) => formatSel({ indent: Math.max(0, (s.fmt?.indent ?? 0) - 1) }));
    add(
      'format.mergeCenter',
      (s) =>
        s.fmt?.merged
          ? exec({ cmd: 'unmerge', sheet: s.sheet, range: rng(s) })
          : exec({ cmd: 'merge', sheet: s.sheet, range: rng(s), center: true }),
      { isChecked: (s) => !!s.fmt?.merged },
    );
    add('format.mergeAcross', (s) =>
      exec({ cmd: 'merge', sheet: s.sheet, range: rng(s), across: true }),
    );
    add('format.unmerge', (s) => exec({ cmd: 'unmerge', sheet: s.sheet, range: rng(s) }));
    // Number
    const preset = (id: string, fallback: string) => () =>
      formatSel({ numFmt: presets.numberFormats.find((p) => p.id === id)?.code ?? fallback });
    add('format.numberFormat', popup('numFmt', 'format.numberFormat'));
    add('format.currency', preset('currencyVnd', '#,##0 [$₫-42A]'));
    add('format.percent', preset('percent', '0%'), { shortcut: 'Ctrl+Shift+%' });
    add('format.comma', preset('number', '#,##0.00'));
    add('format.date', preset('date', 'dd/mm/yyyy'));
    // Styles
    add('format.cellStyles', popup('styles', 'format.cellStyles'));
    add('format.conditional', open({ kind: 'cf' }));
    add('format.clearConditional', (s) => exec({ cmd: 'clearCf', sheet: s.sheet, range: rng(s) }));
    add('format.cells', open({ kind: 'format' }), { shortcut: 'Ctrl+1' });
    add('format.painter', async (s) => setPainter((p) => (p ? null : rangeOf(s.selection))), {
      isChecked: () => painter !== null,
    });
    add('format.clear', (s) => {
      const cmds: SheetCommand[] = s.ranges.map((r) => ({
        cmd: 'clearFormats',
        sheet: s.sheet,
        range: toRange(r),
      }));
      return exec(cmds.length === 1 ? cmds[0]! : { cmd: 'batch', cmds });
    });
    // Protection (§18.3.1.85, §18.2.29) and encryption ([MS-OFFCRYPTO])
    add('format.lockCell', async (s) => formatSel({ locked: !(s.fmt?.locked ?? true) }), {
      ...has,
      isChecked: (s) => s.fmt?.locked ?? true,
    });
    add(
      'review.protectSheet',
      async (s) =>
        setSecurity(
          s.sheetProtected
            ? { kind: 'unprotect', workbook: false, wrong: false }
            : { kind: 'protect', workbook: false },
        ),
      { ...has, isChecked: (s) => s.sheetProtected },
    );
    add(
      'review.protectWorkbook',
      async (s) =>
        setSecurity(
          s.structureLocked
            ? { kind: 'unprotect', workbook: true, wrong: false }
            : { kind: 'protect', workbook: true },
        ),
      { ...has, isChecked: (s) => s.structureLocked },
    );
    add('review.savePassword', async () => setSecurity({ kind: 'savePassword' }), has);

    // Cells, rows and columns
    const structural = (rows: boolean, remove: boolean) => async (s: SheetEditorState) => {
      const r = rangeOf(s.selection);
      const at = rows ? r.r0 : r.c0;
      const count = rows ? r.r1 - r.r0 + 1 : r.c1 - r.c0 + 1;
      await s.host.structural(s.sheet, rows, at, Math.min(count, 10_000), remove);
      await refreshInfo();
    };
    add('cells.insertRow', structural(true, false));
    add('cells.insertCol', structural(false, false));
    add('cells.deleteRow', structural(true, true));
    add('cells.deleteCol', structural(false, true));
    const axis = (rows: boolean) => {
      const [first, last] = axisSpan(rows);
      return { rows, first, last };
    };
    add('cells.autofitCol', (s) => exec({ cmd: 'autofit', sheet: s.sheet, ...axis(false) }));
    add('cells.autofitRow', (s) => exec({ cmd: 'autofit', sheet: s.sheet, ...axis(true) }));
    add('cells.hideRows', (s) =>
      exec({ cmd: 'hide', sheet: s.sheet, ...axis(true), hidden: true }),
    );
    add('cells.unhideRows', (s) => {
      const [a, b] = axisSpan(true);
      return exec({
        cmd: 'hide',
        sheet: s.sheet,
        rows: true,
        first: Math.max(0, a - 1),
        last: b + 1,
        hidden: false,
      });
    });
    add('cells.hideCols', (s) =>
      exec({ cmd: 'hide', sheet: s.sheet, ...axis(false), hidden: true }),
    );
    add('cells.unhideCols', (s) => {
      const [a, b] = axisSpan(false);
      return exec({
        cmd: 'hide',
        sheet: s.sheet,
        rows: false,
        first: Math.max(0, a - 1),
        last: b + 1,
        hidden: false,
      });
    });

    // Sheets
    add('sheet.add', async (s) => {
      const i = await s.host.addSheet();
      await refreshInfo();
      setSheet(i);
    });
    add('sheet.rename', async (s) => setRenaming(s.sheet));
    add(
      'sheet.delete',
      async (s) => {
        await s.host.deleteSheet(s.sheet);
        setSheet(Math.max(0, s.sheet - 1));
        await refreshInfo();
      },
      { isEnabled: () => (info?.sheets.length ?? 0) > 1 },
    );

    // Comments ([MS-XLSX] threaded comments)
    add('review.comments', async () => setCommentsOpen((o) => !o), {
      ...has,
      isChecked: () => commentsOpen,
    });

    // Insert
    add('insert.chart', popup('chart', 'insert.chart'), has);
    add('insert.pivot', open({ kind: 'pivot' }), has);
    add('insert.table', (s) => exec({ cmd: 'createTable', sheet: s.sheet, range: rng(s) }), has);
    add(
      'insert.slicer',
      (s) =>
        exec({
          cmd: 'insertSlicer',
          sheet: s.sheet,
          row: s.selection.active.row,
          col: s.selection.active.col,
        }),
      has,
    );
    add(
      'pivot.refresh',
      async (s) => {
        if (s.pivot === null) return;
        const done = await exec({
          cmd: 'pivotRefresh',
          sheet: s.sheet,
          index: s.pivot,
          labels: pivotLabels,
        });
        if (done) setStatus(t('sheetEditor.pivot.refreshed'));
      },
      { isEnabled: (s) => s.pivot !== null },
    );
    add('pivot.fieldList', async () => setPivotPanel((v) => !v), {
      isEnabled: (s) => s.pivot !== null,
      isChecked: (s) => s.pivot !== null && pivotPanel,
    });
    const inPivot = { isEnabled: (s: SheetEditorState) => s.pivot !== null && !!activePivot };
    const withPivot = (f: (p: PivotInfo) => DialogState) => async () => {
      if (activePivot) setDialog(f(activePivot));
    };
    add(
      'pivot.options',
      withPivot((pivot) => ({ kind: 'pivotOptions', pivot })),
      inPivot,
    );
    add(
      'pivot.fieldSettings',
      withPivot((pivot) => {
        // The first row / column field, else the first value field.
        const field = pivot.rows[0] ?? pivot.cols[0];
        return field !== undefined
          ? { kind: 'pivotField', pivot, field }
          : { kind: 'pivotValue', pivot, idx: 0 };
      }),
      {
        isEnabled: (s) =>
          s.pivot !== null &&
          !!activePivot &&
          activePivot.rows.length + activePivot.cols.length + activePivot.values.length > 0,
      },
    );
    add(
      'pivot.calculatedField',
      withPivot((pivot) => ({ kind: 'pivotCalc', pivot })),
      inPivot,
    );
    add(
      'pivot.move',
      withPivot((pivot) => ({ kind: 'pivotMove', pivot })),
      inPivot,
    );
    add(
      'pivot.changeSource',
      withPivot((pivot) => ({ kind: 'pivotSource', pivot })),
      inPivot,
    );
    for (const layout of ['compact', 'outline', 'tabular'] as const) {
      const id = `pivot.layout${layout.charAt(0).toUpperCase()}${layout.slice(1)}`;
      add(
        id,
        async (s) => {
          if (s.pivot === null || !activePivot) return;
          await exec({
            cmd: 'pivotUpdate',
            sheet: s.sheet,
            index: s.pivot,
            labels: pivotLabels,
            ...areasOf(activePivot),
            layout,
          });
        },
        { ...inPivot, isChecked: () => activePivot?.layout === layout },
      );
    }
    add(
      'pivot.grandTotalsRows',
      async (s) => {
        if (s.pivot === null || !activePivot) return;
        await exec({
          cmd: 'pivotUpdate',
          sheet: s.sheet,
          index: s.pivot,
          labels: pivotLabels,
          ...areasOf(activePivot),
          colGrandTotals: !activePivot.colGrandTotals,
        });
      },
      { ...inPivot, isChecked: () => !!activePivot?.colGrandTotals },
    );
    add(
      'pivot.grandTotalsColumns',
      async (s) => {
        if (s.pivot === null || !activePivot) return;
        await exec({
          cmd: 'pivotUpdate',
          sheet: s.sheet,
          index: s.pivot,
          labels: pivotLabels,
          ...areasOf(activePivot),
          rowGrandTotals: !activePivot.rowGrandTotals,
        });
      },
      { ...inPivot, isChecked: () => !!activePivot?.rowGrandTotals },
    );
    add(
      'pivot.valuesOnRows',
      async (s) => {
        if (s.pivot === null || !activePivot) return;
        await exec({
          cmd: 'pivotUpdate',
          sheet: s.sheet,
          index: s.pivot,
          labels: pivotLabels,
          ...areasOf(activePivot),
          dataOnRows: !activePivot.dataOnRows,
        });
      },
      {
        isEnabled: (s) => s.pivot !== null && (activePivot?.values.length ?? 0) > 1,
        isChecked: () => !!activePivot?.dataOnRows,
      },
    );
    add(
      'pivot.chart',
      async (s) => {
        if (s.pivot === null) return;
        const done = await exec<{ index: number }>({
          cmd: 'pivotChart',
          sheet: s.sheet,
          index: s.pivot,
        });
        if (done) {
          setChart(done.index);
          setStatus(t('sheetEditor.pivot.chartInserted'));
        }
      },
      inPivot,
    );
    add(
      'pivot.calculatedItem',
      withPivot((pivot) => ({ kind: 'pivotCalcItem', pivot })),
      {
        isEnabled: (s) =>
          s.pivot !== null &&
          !!activePivot &&
          activePivot.rows.length + activePivot.cols.length > 0,
      },
    );
    // Manual groups: the item labels in the selection.
    const groupSelection = (ungroup: boolean) => async (s: SheetEditorState) => {
      if (s.pivot === null || !activePivot) return;
      const r = rangeOf(s.selection);
      const texts: string[] = [];
      for (let row = r.r0; row <= Math.min(r.r1, r.r0 + 500); row++) {
        for (let col = r.c0; col <= Math.min(r.c1, r.c0 + 50); col++) {
          texts.push((await s.host.cell(s.sheet, row, col)).display);
        }
      }
      const sel = pivotSelectionItems(activePivot, texts);
      if (!sel) {
        setStatus(t('sheetEditor.pivot.selectItems'));
        return;
      }
      await exec({
        cmd: 'pivotGroupItems',
        sheet: s.sheet,
        index: s.pivot,
        field: sel.field,
        items: sel.items,
        ungroup,
        labels: pivotLabels,
      });
    };
    add('pivot.groupSelection', groupSelection(false), inPivot);
    add('pivot.ungroupSelection', groupSelection(true), inPivot);
    add(
      'pivot.delete',
      async (s) => {
        if (s.pivot === null) return;
        const done = await exec({ cmd: 'pivotDelete', sheet: s.sheet, index: s.pivot });
        if (done) setStatus(t('sheetEditor.pivot.deleted'));
      },
      { isEnabled: (s) => s.pivot !== null },
    );
    add(
      'pivot.refreshAll',
      async () => {
        const done = await exec<{ count: number }>({ cmd: 'pivotRefreshAll', labels: pivotLabels });
        if (done) setStatus(t('sheetEditor.pivot.refreshedAll', { n: done.count }));
      },
      { isEnabled: () => !!info?.sheets.some((x) => x.pivots > 0) },
    );
    add('chart.edit', async (s) => setDialog({ kind: 'chart', index: s.chart }), {
      isEnabled: () => (summary?.charts ?? 0) > 0,
    });
    add(
      'chart.delete',
      async (s) => {
        if (s.chart === null) return;
        await exec({ cmd: 'deleteChart', sheet: s.sheet, index: s.chart });
        setChart(null);
      },
      { isEnabled: (s) => s.chart !== null },
    );

    // Data
    const quickSort = (descending: boolean) => async (s: SheetEditorState) => {
      const r = rangeOf(s.selection);
      await exec({
        cmd: 'sort',
        sheet: s.sheet,
        range: toRange(r),
        keys: [{ col: s.selection.active.col, descending }],
        hasHeader: cellsOf(r) === 1,
      });
    };
    add('data.sortAsc', quickSort(false));
    add('data.sortDesc', quickSort(true));
    add('data.sort', open({ kind: 'sort' }));
    add('data.filter', (s) => exec({ cmd: 'toggleFilter', sheet: s.sheet, range: rng(s) }), {
      shortcut: 'Ctrl+Shift+L',
      isChecked: () => !!summary?.autoFilter,
    });
    add('data.validation', open({ kind: 'dv' }));
    add('data.goalSeek', open({ kind: 'goalSeek' }));
    add('data.dataTable', open({ kind: 'dataTable' }));
    add('data.solver', open({ kind: 'solver' }));

    // Page layout
    add('page.setup', open({ kind: 'pageSetup' }));
    add('page.setPrintArea', (s) =>
      exec({ cmd: 'setPageSetup', sheet: s.sheet, printArea: rng(s) }),
    );
    add('page.clearPrintArea', (s) =>
      exec({ cmd: 'setPageSetup', sheet: s.sheet, printArea: null }),
    );

    // View
    const freeze = (rows: number, cols: number) => (s: SheetEditorState) =>
      exec({ cmd: 'freeze', sheet: s.sheet, rows, cols });
    add('view.freezeTopRow', freeze(1, 0));
    add('view.freezeFirstCol', freeze(0, 1));
    add('view.freezePanes', (s) =>
      exec({
        cmd: 'freeze',
        sheet: s.sheet,
        rows: s.selection.active.row,
        cols: s.selection.active.col,
      }),
    );
    add('view.unfreeze', freeze(0, 0), {
      isEnabled: () => (summary?.frozenRows ?? 0) + (summary?.frozenCols ?? 0) > 0,
    });
    add('view.zoomIn', async () => setZoom((z) => ZOOMS.find((v) => v > z) ?? z));
    add('view.zoomOut', async () => setZoom((z) => [...ZOOMS].reverse().find((v) => v < z) ?? z));
    return reg;
  }, [
    t,
    openFile,
    fail,
    load,
    platform,
    fileName,
    refreshInfo,
    copy,
    paste,
    clearSelection,
    info,
    exec,
    formatSel,
    presets,
    painter,
    summary,
    axisSpan,
    pivotLabels,
    pivotPanel,
    commentsOpen,
    activePivot,
  ]);

  const state: SheetEditorState | null = useMemo(
    () =>
      host
        ? {
            host,
            sheet,
            selection,
            hasWorkbook: !!info,
            fmt,
            chart,
            pivot: activePivot?.index ?? null,
            ranges: allRanges,
            sheetProtected: !!info?.sheets[sheet]?.protected,
            structureLocked: !!info?.structureLocked,
          }
        : null,
    [host, sheet, selection, info, fmt, chart, activePivot?.index, allRanges],
  );
  const run = useCallback(
    (id: string) => {
      if (state) void registry.execute(id, state);
    },
    [registry, state],
  );

  // ------------------------------------------------------------- input

  const navKey: Record<string, Direction> = {
    ArrowUp: 'up',
    ArrowDown: 'down',
    ArrowLeft: 'left',
    ArrowRight: 'right',
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (composing.current || e.nativeEvent.isComposing) return;
    if (edit) {
      if (e.key === 'Enter' && !e.altKey) {
        e.preventDefault();
        void commit(advance(selection, e.shiftKey ? 'up' : 'down'));
      } else if (e.key === 'Tab') {
        e.preventDefault();
        void commit(advance(selection, e.shiftKey ? 'left' : 'right'));
      } else if (e.key === 'Escape') {
        e.preventDefault();
        cancelEdit();
      } else if (navKey[e.key] && !edit.inPlace) {
        e.preventDefault();
        void commit(move(selection, navKey[e.key]!, false, 1, hidden));
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      const last = info?.sheets[sheet];
      select(span({ row: 0, col: 0 }, { row: last?.lastRow ?? 0, col: last?.lastCol ?? 0 }));
      return;
    }
    if (e.key === 'F5') {
      e.preventDefault();
      run('edit.goto');
      return;
    }
    if (e.altKey && e.key === 'ArrowDown' && dvList) {
      e.preventDefault();
      setDvOpen(true);
      return;
    }
    const cmd = registry.match(e);
    if (cmd) {
      e.preventDefault();
      run(cmd.id);
      return;
    }
    if (e.key === 'Escape' && painter) {
      setPainter(null);
      return;
    }
    const dir = navKey[e.key];
    if (dir) {
      e.preventDefault();
      const step = e.ctrlKey || e.metaKey ? 50 : 1;
      select(move(selection, dir, e.shiftKey, step, hidden));
    } else if (e.key === 'PageDown' || e.key === 'PageUp') {
      e.preventDefault();
      const rows = Math.max(1, Math.floor(size.h / (20 * scale)));
      select(move(selection, e.key === 'PageDown' ? 'down' : 'up', e.shiftKey, rows, hidden));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const r = rangeOf(selection);
      const one = cellsOf(r) === 1;
      select(
        one
          ? move(selection, e.shiftKey ? 'up' : 'down', false, 1, hidden)
          : advance(selection, e.shiftKey ? 'up' : 'down'),
      );
    } else if (e.key === 'Tab') {
      e.preventDefault();
      select(advance(selection, e.shiftKey ? 'left' : 'right'));
    } else if (e.key === 'F2') {
      e.preventDefault();
      void startEdit(true);
    } else if (e.key === 'Backspace') {
      e.preventDefault();
      void startEdit(false, '');
    }
  };

  // Typing (including IME output) lands in the input and switches to edit mode.
  const onInput = (text: string) => {
    if (edit) {
      setEdit({ ...edit, text });
      setBarText(text);
    } else {
      setEdit({ pos: selection.active, text, inPlace: false });
      setBarText(text);
    }
  };

  const point = (e: { clientX: number; clientY: number }) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return null;
    return { x: (e.clientX - rect.left) / scale, y: (e.clientY - rect.top) / scale };
  };

  const hitAt = async (e: { clientX: number; clientY: number }) => {
    if (!host) return null;
    const p = point(e);
    if (!p) return null;
    return host.hitTest(view, p.x, p.y);
  };

  const onMouseDown = async (e: ReactMouseEvent) => {
    if (e.button !== 0) return;
    if (edit) await commit();
    const hit = await hitAt(e);
    if (!hit || !host) return;
    const last = info?.sheets[sheet];
    setChart(null);
    const multi = (e.ctrlKey || e.metaKey) && !e.shiftKey;
    // Ctrl+click / Ctrl+drag starts another range of a multi-range selection.
    if (multi && (hit.kind === 'cell' || hit.kind === 'columnHeader' || hit.kind === 'rowHeader'))
      setExtra((x) => [...x, rangeOf(selection)]);
    else if (!e.shiftKey) setExtra([]);
    if (hit.kind === 'cell') {
      const p = { row: hit.row, col: hit.col };
      setSelection(e.shiftKey ? { ...selection, focus: p } : single(p));
      drag.current = { kind: 'select' };
    } else if (hit.kind === 'columnEdge' || hit.kind === 'rowEdge') {
      const rows = hit.kind === 'rowEdge';
      const index = rows ? hit.row : hit.col;
      const r = rows
        ? await host.rangeRect(view, index, 0, index, 0)
        : await host.rangeRect(view, 0, index, 0, index);
      const start = rows ? e.clientY : e.clientX;
      drag.current = { kind: 'resize', rows, index, start, size: r ? (rows ? r.h : r.w) : 0 };
      const at = point(e);
      if (at) setGuide(rows ? { y: at.y * scale } : { x: at.x * scale });
    } else if (hit.kind === 'filterButton') {
      setFilterMenu({ col: hit.col, x: e.clientX, y: e.clientY + 4 });
    } else if (hit.kind === 'chart') {
      setChart(hit.index);
      const list = await host.exec<ChartInfo[]>({ cmd: 'charts', sheet });
      const c = list?.[hit.index];
      if (c) {
        drag.current = {
          kind: 'chart',
          index: hit.index,
          resize: false,
          start: { row: hit.row, col: hit.col },
          from: c.from,
          to: c.to,
        };
      }
    } else if (hit.kind === 'columnHeader') {
      const to = { row: Math.max(last?.lastRow ?? 0, 999), col: hit.col };
      setSelection(
        e.shiftKey
          ? { ...selection, focus: to, anchor: { row: 0, col: selection.anchor.col } }
          : span({ row: 0, col: hit.col }, to),
      );
    } else if (hit.kind === 'rowHeader') {
      const to = { row: hit.row, col: Math.max(last?.lastCol ?? 0, 25) };
      setSelection(
        e.shiftKey
          ? { ...selection, focus: to, anchor: { row: selection.anchor.row, col: 0 } }
          : span({ row: hit.row, col: 0 }, to),
      );
    } else if (hit.kind === 'corner') {
      setSelection(span({ row: 0, col: 0 }, { row: last?.lastRow ?? 0, col: last?.lastCol ?? 0 }));
    }
    focusInput();
  };

  const onMouseMove = async (e: ReactMouseEvent) => {
    const d = drag.current;
    if (d?.kind === 'resize') {
      const at = point(e);
      if (at) setGuide(d.rows ? { y: at.y * scale } : { x: at.x * scale });
      return;
    }
    if (d?.kind === 'select') {
      const hit = await hitAt(e);
      if (hit?.kind === 'cell') {
        setSelection((s) => ({ ...s, focus: { row: hit.row, col: hit.col } }));
      }
      return;
    }
    if (d?.kind === 'chart') {
      const hit = await hitAt(e);
      if (!hit || (hit.kind !== 'cell' && hit.kind !== 'chart') || !host) return;
      const dr = d.resize ? 0 : Math.max(hit.row - d.start.row, -d.from[0]);
      const dc = d.resize ? 0 : Math.max(hit.col - d.start.col, -d.from[1]);
      const from: [number, number] = [d.from[0] + dr, d.from[1] + dc];
      const to: [number, number] = d.resize
        ? [Math.max(hit.row + 1, from[0] + 1), Math.max(hit.col + 1, from[1] + 1)]
        : [d.to[0] + dr, d.to[1] + dc];
      const same =
        from[0] === d.from[0] && from[1] === d.from[1] && to[0] === d.to[0] && to[1] === d.to[1];
      chartTarget.current = same ? null : { from, to };
      setFillRect(
        same ? null : px(await host.rangeRect(view, from[0], from[1], to[0] - 1, to[1] - 1)),
      );
      return;
    }
    if (d?.kind === 'fill') {
      const hit = await hitAt(e);
      if (hit?.kind !== 'cell' || !host) return;
      const target = fillTarget(d.source, { row: hit.row, col: hit.col });
      fillTo.current = target;
      setFillRect(
        target ? px(await host.rangeRect(view, target.r0, target.c0, target.r1, target.c1)) : null,
      );
      return;
    }
    // Hover: cursor for resize handles, filter buttons and charts.
    if (hovering.current) return;
    hovering.current = true;
    try {
      const hit = await hitAt(e);
      setCursor(
        hit?.kind === 'columnEdge'
          ? 'col-resize'
          : hit?.kind === 'rowEdge'
            ? 'row-resize'
            : hit?.kind === 'filterButton' || hit?.kind === 'chart'
              ? 'pointer'
              : painter
                ? 'copy'
                : 'cell',
      );
    } finally {
      hovering.current = false;
    }
  };

  // Mouse-up ends drags anywhere on the page.
  const endDrag = useRef<(e: MouseEvent) => void>(() => {});
  const onDragEnd = (e: MouseEvent) => {
    const d = drag.current;
    drag.current = null;
    if (!d || !host) return;
    if (d.kind === 'resize') {
      setGuide(null);
      const delta = ((d.rows ? e.clientY : e.clientX) - d.start) / scale;
      const sizePt = Math.max(0, d.size + delta);
      const r = rangeOf(selection);
      const whole = d.rows ? r.c0 === 0 && r.c1 >= 25 : r.r0 === 0 && r.r1 >= 999;
      const inSel = d.rows
        ? d.index >= r.r0 && d.index <= r.r1
        : d.index >= r.c0 && d.index <= r.c1;
      const [first, last] =
        whole && inSel ? (d.rows ? [r.r0, r.r1] : [r.c0, r.c1]) : [d.index, d.index];
      if (Math.abs(delta) > 0.5)
        void exec({ cmd: 'resize', sheet, rows: d.rows, first, last, size: sizePt });
    } else if (d.kind === 'chart') {
      const target = chartTarget.current;
      chartTarget.current = null;
      setFillRect(null);
      if (target) {
        void exec({ cmd: 'moveChart', sheet, index: d.index, from: target.from, to: target.to });
      }
    } else if (d.kind === 'fill') {
      const target = fillTo.current;
      fillTo.current = null;
      setFillRect(null);
      if (target) {
        void exec({
          cmd: 'autofill',
          sheet,
          source: toRange(d.source),
          target: toRange(target),
        }).then(() =>
          setSelection(
            span({ row: target.r0, col: target.c0 }, { row: target.r1, col: target.c1 }),
          ),
        );
      }
    } else if (d.kind === 'select' && painter) {
      const to = rangeOf(selection);
      setPainter(null);
      void exec({ cmd: 'paintFormat', sheet, from: toRange(painter), to: toRange(to) });
    }
  };
  useEffect(() => {
    endDrag.current = onDragEnd;
  });
  useEffect(() => {
    const up = (e: MouseEvent) => endDrag.current(e);
    window.addEventListener('mouseup', up);
    return () => window.removeEventListener('mouseup', up);
  }, []);

  const onDoubleClick = async (e: ReactMouseEvent) => {
    const hit = await hitAt(e);
    if (!hit) return;
    if (hit.kind === 'columnEdge' || hit.kind === 'rowEdge') {
      const rows = hit.kind === 'rowEdge';
      const index = rows ? hit.row : hit.col;
      const [a, b] = axisSpan(rows);
      const [first, last] = index >= a && index <= b ? [a, b] : [index, index];
      void exec({ cmd: 'autofit', sheet, rows, first, last });
    } else if (hit.kind === 'chart') {
      setDialog({ kind: 'chart', index: hit.index });
    } else if (hit.kind === 'cell') {
      void startEdit(true);
    }
  };

  const onNameBox = (text: string) => {
    const s = parseA1(text);
    if (s) select(s);
    else if (host)
      void host
        .exec<{ sheet: number; range: SheetRange } | null>({ cmd: 'resolve', sheet, text })
        .then((r) => {
          if (!r) return;
          setSheet(r.sheet);
          select(span({ row: r.range.r0, col: r.range.c0 }, { row: r.range.r1, col: r.range.c1 }));
        })
        .catch(fail);
    focusInput();
  };

  // ------------------------------------------------------------- view

  const ribbonTabs: RibbonTab[] = useMemo(() => {
    const item = (
      id: string,
      label: string,
      icon: ReactNode,
      size: 'large' | 'small' = 'large',
    ) => ({
      id,
      label,
      icon,
      size,
      shortcut: registry.get(id)?.shortcut,
      enabled: state ? registry.isEnabled(id, state) : false,
      checked: state && registry.get(id)?.isChecked ? registry.isChecked(id, state) : undefined,
      onSelect: () => run(id),
    });
    const s = 16;
    return [
      {
        id: 'home',
        label: t('ribbon.tabs.home'),
        groups: [
          {
            id: 'file',
            label: t('sheetEditor.groupFile'),
            items: [
              item('file.new', t('sheetEditor.newWorkbook'), <FilePlus size={18} />),
              item('file.open', t('sheetEditor.open'), <FolderOpen size={18} />),
              item('file.save', t('sheetEditor.save'), <Save size={18} />),
              item('file.saveCsv', t('sheetEditor.saveCsv'), <Table2 size={s} />, 'small'),
              item('file.exportPdf', t('sheetEditor.exportPdf'), <FileDown size={s} />, 'small'),
              item('file.print', t('sheetEditor.print'), <Printer size={s} />, 'small'),
            ],
          },
          {
            id: 'clipboard',
            label: t('sheetEditor.groupEdit'),
            items: [
              item('edit.paste', t('sheetEditor.paste'), <ClipboardPaste size={18} />),
              item(
                'edit.pasteSpecial',
                t('sheetEditor.pasteSpecial.title'),
                <ClipboardList size={s} />,
                'small',
              ),
              item('edit.cut', t('sheetEditor.cut'), <Scissors size={s} />, 'small'),
              item('edit.copy', t('sheetEditor.copy'), <Copy size={s} />, 'small'),
              item('format.painter', t('sheetEditor.painter'), <Paintbrush size={s} />, 'small'),
              item('edit.undo', t('sheetEditor.undo'), <Undo2 size={s} />, 'small'),
              item('edit.redo', t('sheetEditor.redo'), <Redo2 size={s} />, 'small'),
            ],
          },
          {
            id: 'font',
            label: t('sheetEditor.groupFont'),
            items: [
              item('format.bold', t('sheetEditor.bold'), <Bold size={s} />, 'small'),
              item('format.italic', t('sheetEditor.italic'), <Italic size={s} />, 'small'),
              item('format.underline', t('sheetEditor.underline'), <Underline size={s} />, 'small'),
              item('format.strike', t('sheetEditor.strike'), <Strikethrough size={s} />, 'small'),
              item('format.fontGrow', t('sheetEditor.fontGrow'), <AArrowUp size={s} />, 'small'),
              item(
                'format.fontShrink',
                t('sheetEditor.fontShrink'),
                <AArrowDown size={s} />,
                'small',
              ),
              item('format.fontColor', t('sheetEditor.fontColor'), <Baseline size={s} />, 'small'),
              item(
                'format.fillColor',
                t('sheetEditor.fillColor'),
                <PaintBucket size={s} />,
                'small',
              ),
              item('format.borders', t('sheetEditor.borders'), <Grid2x2 size={s} />, 'small'),
            ],
          },
          {
            id: 'alignment',
            label: t('sheetEditor.groupAlignment'),
            items: [
              item(
                'format.alignLeft',
                t('sheetEditor.alignLeft'),
                <TextAlignStart size={s} />,
                'small',
              ),
              item(
                'format.alignCenter',
                t('sheetEditor.alignCenter'),
                <TextAlignCenter size={s} />,
                'small',
              ),
              item(
                'format.alignRight',
                t('sheetEditor.alignRight'),
                <TextAlignEnd size={s} />,
                'small',
              ),
              item(
                'format.alignTop',
                t('sheetEditor.alignTop'),
                <ArrowUpToLine size={s} />,
                'small',
              ),
              item(
                'format.alignMiddle',
                t('sheetEditor.alignMiddle'),
                <AlignVerticalJustifyCenter size={s} />,
                'small',
              ),
              item(
                'format.alignBottom',
                t('sheetEditor.alignBottom'),
                <ArrowDownToLine size={s} />,
                'small',
              ),
              item('format.wrap', t('sheetEditor.wrap'), <TextWrap size={s} />, 'small'),
              item(
                'format.mergeCenter',
                t('sheetEditor.mergeCenter'),
                <TableCellsMerge size={s} />,
                'small',
              ),
              item(
                'format.unmerge',
                t('sheetEditor.unmerge'),
                <TableCellsSplit size={s} />,
                'small',
              ),
              item(
                'format.indentInc',
                t('sheetEditor.indentInc'),
                <ListIndentIncrease size={s} />,
                'small',
              ),
              item(
                'format.indentDec',
                t('sheetEditor.indentDec'),
                <ListIndentDecrease size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'number',
            label: t('sheetEditor.groupNumber'),
            items: [
              item(
                'format.numberFormat',
                t('sheetEditor.numberFormat'),
                <Hash size={s} />,
                'small',
              ),
              item('format.currency', t('sheetEditor.currency'), <Coins size={s} />, 'small'),
              item('format.percent', t('sheetEditor.percent'), <Percent size={s} />, 'small'),
              item('format.comma', t('sheetEditor.comma'), <Hash size={s} />, 'small'),
              item('format.date', t('sheetEditor.dateFormat'), <Calendar size={s} />, 'small'),
            ],
          },
          {
            id: 'styles',
            label: t('sheetEditor.groupStyles'),
            items: [
              item('format.cellStyles', t('sheetEditor.cellStyles'), <Palette size={18} />),
              item(
                'format.conditional',
                t('sheetEditor.conditional'),
                <Highlighter size={s} />,
                'small',
              ),
              item(
                'format.clearConditional',
                t('sheetEditor.clearConditional'),
                <Eraser size={s} />,
                'small',
              ),
              item('format.cells', t('sheetEditor.fmt.title'), <PencilLine size={s} />, 'small'),
              item('format.clear', t('sheetEditor.clearFormats'), <Eraser size={s} />, 'small'),
            ],
          },
          {
            id: 'cells',
            label: t('sheetEditor.groupCells'),
            items: [
              item('cells.insertRow', t('sheetEditor.insertRow'), <Rows3 size={s} />, 'small'),
              item('cells.insertCol', t('sheetEditor.insertCol'), <Columns3 size={s} />, 'small'),
              item('cells.deleteRow', t('sheetEditor.deleteRow'), <Trash2 size={s} />, 'small'),
              item('cells.deleteCol', t('sheetEditor.deleteCol'), <Trash2 size={s} />, 'small'),
              item(
                'cells.autofitCol',
                t('sheetEditor.autofitCol'),
                <MoveHorizontal size={s} />,
                'small',
              ),
              item(
                'cells.autofitRow',
                t('sheetEditor.autofitRow'),
                <MoveVertical size={s} />,
                'small',
              ),
              item('cells.hideRows', t('sheetEditor.hideRows'), <EyeOff size={s} />, 'small'),
              item('cells.unhideRows', t('sheetEditor.unhideRows'), <Eye size={s} />, 'small'),
              item('cells.hideCols', t('sheetEditor.hideCols'), <EyeOff size={s} />, 'small'),
              item('cells.unhideCols', t('sheetEditor.unhideCols'), <Eye size={s} />, 'small'),
            ],
          },
          {
            id: 'editing',
            label: t('sheetEditor.groupEditing'),
            items: [
              item('edit.clear', t('sheetEditor.clear'), <Eraser size={s} />, 'small'),
              item(
                'edit.fillDown',
                t('sheetEditor.fillDown'),
                <ArrowDownToLine size={s} />,
                'small',
              ),
              item(
                'edit.fillRight',
                t('sheetEditor.fillRight'),
                <ArrowRightToLine size={s} />,
                'small',
              ),
              item('edit.find', t('sheetEditor.find.title'), <Search size={s} />, 'small'),
              item(
                'edit.replace',
                t('sheetEditor.find.replaceTitle'),
                <Replace size={s} />,
                'small',
              ),
              item('edit.goto', t('sheetEditor.goto.title'), <LocateFixed size={s} />, 'small'),
            ],
          },
          {
            id: 'sheets',
            label: t('sheetEditor.groupSheets'),
            items: [
              item('sheet.add', t('sheetEditor.addSheet'), <Plus size={s} />, 'small'),
              item('sheet.rename', t('sheetEditor.renameSheet'), <Table2 size={s} />, 'small'),
              item('sheet.delete', t('sheetEditor.deleteSheet'), <Trash2 size={s} />, 'small'),
            ],
          },
        ],
      },
      {
        id: 'insert',
        label: t('ribbon.tabs.insert'),
        groups: [
          {
            id: 'charts',
            label: t('sheetEditor.groupCharts'),
            items: [
              item('insert.chart', t('sheetEditor.chart.insert'), <ChartColumn size={18} />),
              item(
                'review.comments',
                t('sheetEditor.comments.title'),
                <MessageSquareText size={18} />,
              ),
              item(
                'chart.edit',
                t('sheetEditor.chart.editTitle'),
                <PencilLine size={s} />,
                'small',
              ),
              item('chart.delete', t('sheetEditor.chart.delete'), <Trash size={s} />, 'small'),
            ],
          },
          {
            id: 'tables',
            label: t('sheetEditor.groupTables'),
            items: [
              item('insert.pivot', t('sheetEditor.pivot.insert'), <TableProperties size={18} />),
              item('insert.table', t('sheetEditor.table.insert'), <Table2 size={18} />),
              item('insert.slicer', t('sheetEditor.slicer.insert'), <ListFilter size={18} />),
              item(
                'pivot.options',
                t('sheetEditor.pivot.options'),
                <Settings2 size={s} />,
                'small',
              ),
              item(
                'pivot.refresh',
                t('sheetEditor.pivot.refresh'),
                <RefreshCw size={s} />,
                'small',
              ),
              item(
                'pivot.fieldList',
                t('sheetEditor.pivot.fieldList'),
                <ListTree size={s} />,
                'small',
              ),
            ],
          },
        ],
      },
      {
        id: 'pivot',
        label: t('sheetEditor.pivot.tab'),
        groups: [
          {
            id: 'pivotData',
            label: t('sheetEditor.pivot.groupData'),
            items: [
              item('pivot.refresh', t('sheetEditor.pivot.refresh'), <RefreshCw size={18} />),
              item(
                'pivot.refreshAll',
                t('sheetEditor.pivot.refreshAll'),
                <RefreshCw size={s} />,
                'small',
              ),
              item(
                'pivot.changeSource',
                t('sheetEditor.pivot.changeSource'),
                <Table2 size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'pivotLayout',
            label: t('sheetEditor.pivot.reportLayout'),
            items: [
              item(
                'pivot.layoutCompact',
                t('sheetEditor.pivot.layoutForm.compact'),
                <ListIndentIncrease size={s} />,
                'small',
              ),
              item(
                'pivot.layoutOutline',
                t('sheetEditor.pivot.layoutForm.outline'),
                <ListTree size={s} />,
                'small',
              ),
              item(
                'pivot.layoutTabular',
                t('sheetEditor.pivot.layoutForm.tabular'),
                <Grid2x2 size={s} />,
                'small',
              ),
              item(
                'pivot.grandTotalsRows',
                t('sheetEditor.pivot.colGrandTotals'),
                <Rows3 size={s} />,
                'small',
              ),
              item(
                'pivot.grandTotalsColumns',
                t('sheetEditor.pivot.rowGrandTotals'),
                <Columns3 size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'pivotCalc',
            label: t('sheetEditor.pivot.groupCalc'),
            items: [
              item(
                'pivot.fieldSettings',
                t('sheetEditor.pivot.fieldSettingsCmd'),
                <Settings size={18} />,
              ),
              item(
                'pivot.calculatedField',
                t('sheetEditor.pivot.calcField'),
                <Sigma size={s} />,
                'small',
              ),
              item(
                'pivot.calculatedItem',
                t('sheetEditor.pivot.calcItem'),
                <Sigma size={s} />,
                'small',
              ),
              item(
                'pivot.groupSelection',
                t('sheetEditor.pivot.groupSelection'),
                <Group size={s} />,
                'small',
              ),
              item(
                'pivot.ungroupSelection',
                t('sheetEditor.pivot.ungroupSelection'),
                <Ungroup size={s} />,
                'small',
              ),
              item(
                'pivot.valuesOnRows',
                t('sheetEditor.pivot.valuesOnRows'),
                <Rows3 size={s} />,
                'small',
              ),
              item(
                'pivot.options',
                t('sheetEditor.pivot.options'),
                <Settings2 size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'pivotActions',
            label: t('sheetEditor.pivot.groupActions'),
            items: [
              item('pivot.chart', t('sheetEditor.pivot.chart'), <ChartColumn size={18} />),
              item(
                'pivot.fieldList',
                t('sheetEditor.pivot.fieldList'),
                <ListTree size={s} />,
                'small',
              ),
              item('pivot.move', t('sheetEditor.pivot.move'), <MoveHorizontal size={s} />, 'small'),
              item('pivot.delete', t('sheetEditor.pivot.delete'), <Trash2 size={s} />, 'small'),
            ],
          },
        ],
      },
      {
        id: 'data',
        label: t('sheetEditor.tabData'),
        groups: [
          {
            id: 'sortFilter',
            label: t('sheetEditor.groupSortFilter'),
            items: [
              item('data.sortAsc', t('sheetEditor.sortAsc'), <ArrowDownAZ size={s} />, 'small'),
              item('data.sortDesc', t('sheetEditor.sortDesc'), <ArrowDownZA size={s} />, 'small'),
              item('data.sort', t('sheetEditor.sort.title'), <ArrowUpDown size={18} />),
              item('data.filter', t('sheetEditor.filter.toggle'), <Funnel size={18} />),
            ],
          },
          {
            id: 'tools',
            label: t('sheetEditor.groupDataTools'),
            items: [
              item('data.validation', t('sheetEditor.dv.title'), <ShieldCheck size={18} />),
              item('data.goalSeek', t('sheetEditor.goalSeek.title'), <Target size={18} />),
              item('data.dataTable', t('sheetEditor.dataTable.title'), <Grid3x3 size={18} />),
              item('data.solver', t('sheetEditor.solver.title'), <Calculator size={18} />),
            ],
          },
        ],
      },
      {
        id: 'page',
        label: t('sheetEditor.tabPage'),
        groups: [
          {
            id: 'pageSetup',
            label: t('sheetEditor.groupPageSetup'),
            items: [
              item('page.setup', t('sheetEditor.page.title'), <FileCog size={18} />),
              item('page.setPrintArea', t('sheetEditor.page.setArea'), <Rows2 size={s} />, 'small'),
              item(
                'page.clearPrintArea',
                t('sheetEditor.page.clearArea'),
                <Eraser size={s} />,
                'small',
              ),
              item('file.exportPdf', t('sheetEditor.exportPdf'), <FileDown size={s} />, 'small'),
            ],
          },
        ],
      },
      {
        id: 'review',
        label: t('ribbon.tabs.review'),
        groups: [
          {
            id: 'protect',
            label: t('sheetEditor.protection.group'),
            items: [
              item(
                'review.protectSheet',
                state?.sheetProtected
                  ? t('sheetEditor.protection.unprotectSheet')
                  : t('sheetEditor.protection.protectSheet'),
                state?.sheetProtected ? <LockOpen size={18} /> : <Lock size={18} />,
              ),
              item(
                'review.protectWorkbook',
                state?.structureLocked
                  ? t('sheetEditor.protection.unprotectWorkbook')
                  : t('sheetEditor.protection.protectWorkbook'),
                <ShieldCheck size={18} />,
              ),
              item(
                'format.lockCell',
                t('sheetEditor.protection.lockCell'),
                <Lock size={s} />,
                'small',
              ),
              item(
                'review.savePassword',
                t('commands.review.savePassword'),
                <KeyRound size={s} />,
                'small',
              ),
            ],
          },
        ],
      },
      {
        id: 'view',
        label: t('ribbon.tabs.view'),
        groups: [
          {
            id: 'freeze',
            label: t('sheetEditor.groupFreeze'),
            items: [
              item('view.freezePanes', t('sheetEditor.freezePanes'), <Snowflake size={18} />),
              item(
                'view.freezeTopRow',
                t('sheetEditor.freezeTopRow'),
                <PanelTop size={s} />,
                'small',
              ),
              item(
                'view.freezeFirstCol',
                t('sheetEditor.freezeFirstCol'),
                <PanelLeft size={s} />,
                'small',
              ),
              item('view.unfreeze', t('sheetEditor.unfreeze'), <Columns2 size={s} />, 'small'),
            ],
          },
          {
            id: 'zoom',
            label: t('sheetEditor.groupZoom'),
            items: [
              item('view.zoomIn', t('sheetEditor.zoomIn'), <ZoomIn size={s} />, 'small'),
              item('view.zoomOut', t('sheetEditor.zoomOut'), <ZoomOut size={s} />, 'small'),
            ],
          },
        ],
      },
    ];
  }, [t, registry, state, run]);

  const r = rangeOf(selection);
  const multiKey = `${sheet}:${JSON.stringify(allRanges.map(toRange))}:${revision}`;
  const shownStats =
    extra.length > 0
      ? multiStats && multiStats.key === multiKey
        ? multiStats
        : null
      : stats && stats.key === `${sheet}:${rangeA1(r)}:${revision}`
        ? stats
        : null;
  const fmtNum = (n: number) =>
    new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 10 }).format(n);

  if (!host) return null;

  const common = {
    host,
    sheet,
    onClose: () => {
      setDialog(null);
      focusInput();
    },
    onDone: refreshInfo,
    fail,
  };

  const menuItems = (kind: string): MenuItem[] => {
    switch (kind) {
      case 'borders':
        return (
          ['all', 'outline', 'inside', 'top', 'bottom', 'left', 'right', 'none'] as BorderPreset[]
        ).map((p) => ({ id: p, label: t(`sheetEditor.border.${p}`) }));
      case 'styles':
        return presets.cellStyles.map((n) => {
          const [bg, fg, b] = STYLE_PREVIEW[n] ?? ['FFFFFF', '000000', false];
          return {
            id: n,
            label: t(`sheetEditor.cellStyle.${n.replace(/\s/g, '')}`),
            preview: (
              <span
                className={styles.stylePreview}
                style={{ background: `#${bg}`, color: `#${fg}`, fontWeight: b ? 700 : 400 }}
              >
                Aa
              </span>
            ),
          };
        });
      case 'numFmt':
        return presets.numberFormats.map((p) => ({
          id: p.code,
          label: t(`sheetEditor.numFmt.${p.id}`),
        }));
      case 'chart':
        return CHART_TYPES.map((k) => ({ id: k, label: t(`sheetEditor.chart.types.${k}`) }));
      default:
        return [];
    }
  };
  const pickMenu = (kind: string, id: string) => {
    const range = toRange(r);
    if (kind === 'borders') void borderSel({ preset: id as BorderPreset, style: 'thin' });
    else if (kind === 'styles') void exec({ cmd: 'cellStyle', sheet, range, name: id });
    else if (kind === 'numFmt') void formatSel({ numFmt: id });
    else if (kind === 'chart')
      void exec<{ index: number }>({ cmd: 'insertChart', sheet, range, kind: id }).then((res) => {
        if (res) setChart(res.index);
      });
    focusInput();
  };

  return (
    <div
      className={styles.root}
      data-theme={theme}
      data-app="sheet"
      data-load-seq={loadSeq}
      aria-busy={loading}
    >
      <Ribbon tabs={ribbonTabs} moreLabel={t('ribbon.more')} />
      {legacyNotice > 0 && (
        <div className={styles.macroBar} role="status" data-testid="legacy-notice">
          <span>{t('legacy.sheetConverted', { count: legacyNotice })}</span>
          <button
            type="button"
            onClick={() => {
              setLegacyNotice(0);
              run('edit.undo');
            }}
          >
            {t('legacy.undo')}
          </button>
          <button type="button" onClick={() => setLegacyNotice(0)}>
            {t('security.dismiss')}
          </button>
        </div>
      )}
      <div className={styles.formulaBar}>
        <input
          className={styles.nameBox}
          aria-label={t('sheetEditor.nameBox')}
          defaultValue={rangeA1(r)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') onNameBox(e.currentTarget.value);
          }}
          onFocus={(e) => e.currentTarget.select()}
          onBlur={(e) => {
            e.currentTarget.value = rangeA1(r);
          }}
          ref={(el) => {
            // Uncontrolled typing, controlled display when not focused.
            if (el && document.activeElement !== el) el.value = rangeA1(r);
          }}
        />
        <span className={styles.fx}>fx</span>
        <input
          className={styles.formulaInput}
          aria-label={t('sheetEditor.formulaBar')}
          value={barText}
          onFocus={() => {
            if (!edit) setEdit({ pos: selection.active, text: barText, inPlace: true });
          }}
          onChange={(e) => {
            setBarText(e.target.value);
            setEdit({ pos: edit?.pos ?? selection.active, text: e.target.value, inPlace: true });
            if (inputRef.current) inputRef.current.value = e.target.value;
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void commit(advance(selection, 'down'));
            } else if (e.key === 'Escape') {
              e.preventDefault();
              cancelEdit();
            }
          }}
        />
      </div>
      {macroBanner && <MacroBanner inline onDismiss={() => setMacroBanner(false)} />}
      <div className={styles.workArea}>
        <div
          className={styles.scroller}
          ref={scrollerRef}
          onScroll={(e) => {
            const el = e.currentTarget;
            setScroll({ x: el.scrollLeft / scale, y: el.scrollTop / scale });
          }}
        >
          <div className={styles.sticky}>
            <canvas
              ref={canvasRef}
              role="grid"
              aria-label={t('sheetEditor.grid')}
              data-testid="sheet-canvas"
              style={{ cursor }}
              onMouseDown={(e) => void onMouseDown(e)}
              onMouseMove={(e) => void onMouseMove(e)}
              onDoubleClick={(e) => void onDoubleClick(e)}
            />
            {chart === null &&
              extra.length > 0 &&
              extraRects.map((x, i) => (
                <div
                  key={i}
                  className={styles.selectionExtra}
                  data-testid="selection-extra"
                  style={{ left: x.x, top: x.y, width: x.w, height: x.h }}
                />
              ))}
            {selRect && chart === null && (
              <div
                className={painter ? styles.selectionPainter : styles.selection}
                style={{ left: selRect.x, top: selRect.y, width: selRect.w, height: selRect.h }}
              >
                <div
                  className={styles.fillHandle}
                  data-testid="fill-handle"
                  title={t('sheetEditor.fillHandle')}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    drag.current = { kind: 'fill', source: rangeOf(selection) };
                  }}
                />
              </div>
            )}
            {chart !== null && chartBox && (
              <div
                className={styles.chartSelection}
                data-testid="chart-selection"
                style={{ left: chartBox.x, top: chartBox.y, width: chartBox.w, height: chartBox.h }}
              >
                <div
                  className={styles.chartHandle}
                  data-testid="chart-resize"
                  title={t('sheetEditor.chart.resize')}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    if (!chartInfo) return;
                    drag.current = {
                      kind: 'chart',
                      index: chart,
                      resize: true,
                      start: { row: chartInfo.to[0], col: chartInfo.to[1] },
                      from: chartInfo.from,
                      to: chartInfo.to,
                    };
                  }}
                />
              </div>
            )}
            {slicers.map(({ info, rect }) => (
              <div
                key={info.index}
                className={styles.slicer}
                data-testid="slicer"
                role="group"
                aria-label={info.caption}
                style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
                onMouseDown={(e) => e.stopPropagation()}
              >
                <div className={styles.slicerHeader}>
                  <span>{info.caption}</span>
                  <button
                    type="button"
                    data-testid="slicer-clear"
                    title={t('sheetEditor.slicer.clear')}
                    aria-label={t('sheetEditor.slicer.clear')}
                    disabled={info.items.every((i) => i.selected)}
                    onClick={() =>
                      void exec({ cmd: 'slicerSelect', sheet, index: info.index, items: [] })
                    }
                  >
                    <FunnelX size={14} />
                  </button>
                </div>
                <div
                  className={styles.slicerItems}
                  style={{ gridTemplateColumns: `repeat(${info.columns}, 1fr)` }}
                >
                  {info.items.map((item) => (
                    <button
                      key={item.label}
                      type="button"
                      className={item.selected ? styles.slicerOn : styles.slicerOff}
                      aria-pressed={item.selected}
                      onClick={(e) => chooseSlicerItem(info, item.label, e.ctrlKey || e.metaKey)}
                    >
                      {item.label || t('sheetEditor.slicer.blank')}
                    </button>
                  ))}
                </div>
              </div>
            ))}
            {fillRect && (
              <div
                className={styles.fillPreview}
                style={{ left: fillRect.x, top: fillRect.y, width: fillRect.w, height: fillRect.h }}
              />
            )}
            {guide && (
              <div
                className={guide.x !== undefined ? styles.guideV : styles.guideH}
                style={guide.x !== undefined ? { left: guide.x } : { top: guide.y }}
              />
            )}
            {dvList && activeRect && !edit && (
              <button
                type="button"
                className={styles.dvButton}
                data-testid="dv-button"
                aria-label={t('sheetEditor.dv.choose')}
                style={{
                  left: activeRect.x + activeRect.w + 1,
                  top: activeRect.y,
                  height: activeRect.h,
                }}
                onMouseDown={(e) => {
                  e.preventDefault();
                  setDvOpen((o) => !o);
                }}
              >
                ▾
              </button>
            )}
            {dvOpen && dvList && activeRect && (
              <div
                className={styles.dvList}
                role="listbox"
                data-testid="dv-list"
                style={{
                  left: activeRect.x,
                  top: activeRect.y + activeRect.h + 1,
                  minWidth: activeRect.w,
                }}
              >
                {dvList.map((v) => (
                  <button
                    key={v}
                    type="button"
                    role="option"
                    aria-selected={false}
                    className={styles.menuItem}
                    onMouseDown={(e) => {
                      e.preventDefault();
                      setDvOpen(false);
                      void exec({
                        cmd: 'setInputs',
                        sheet,
                        cells: [[selection.active.row, selection.active.col, v]],
                      }).then(focusInput);
                    }}
                  >
                    {v}
                  </button>
                ))}
              </div>
            )}
            <input
              ref={inputRef}
              className={edit ? styles.cellEditorActive : styles.cellEditor}
              aria-label={t('sheetEditor.cellEditor')}
              style={
                activeRect
                  ? {
                      left: activeRect.x,
                      top: activeRect.y,
                      minWidth: activeRect.w,
                      height: activeRect.h,
                      fontSize: 11 * scale,
                    }
                  : { left: -9999, top: 0 }
              }
              autoComplete="off"
              spellCheck={false}
              onKeyDown={onKeyDown}
              onInput={(e) => onInput(e.currentTarget.value)}
              onCompositionStart={() => {
                composing.current = true;
                if (!edit) setEdit({ pos: selection.active, text: '', inPlace: false });
              }}
              onCompositionEnd={(e) => {
                composing.current = false;
                onInput(e.currentTarget.value);
              }}
            />
          </div>
          <div style={{ width: content.w * scale, height: content.h * scale }} />
        </div>
        {commentsOpen && (
          <CommentThreads
            threads={threads.map((th) => ({ ...th, anchor: th.cell }))}
            labels={commentLabels}
            newAnchor={
              threads.some(
                (th) => th.row === selection.active.row && th.col === selection.active.col,
              )
                ? null
                : `${colName(selection.active.col)}${selection.active.row + 1}`
            }
            onAdd={(text) =>
              void commentCmd({
                cmd: 'addComment',
                sheet,
                row: selection.active.row,
                col: selection.active.col,
                text,
                author: platform.auth?.user?.name ?? '',
                date: new Date().toISOString(),
              })
            }
            onReply={(id, text) =>
              void commentCmd({
                cmd: 'addComment',
                sheet,
                parent: id,
                text,
                author: platform.auth?.user?.name ?? '',
                date: new Date().toISOString(),
              })
            }
            onResolve={(id, done) => void commentCmd({ cmd: 'resolveComment', sheet, id, done })}
            onDelete={(id) => void commentCmd({ cmd: 'deleteComment', sheet, id })}
            onSelect={(id) => {
              const th = threads.find((x) => x.id === id);
              if (th) setSelection(single({ row: th.row, col: th.col }));
            }}
            onClose={() => {
              setCommentsOpen(false);
              focusInput();
            }}
          />
        )}
        {activePivot && pivotPanel && (
          <PivotPanel
            host={host}
            sheet={sheet}
            pivot={activePivot}
            labels={pivotLabels}
            onChanged={refreshInfo}
            onRefresh={() => run('pivot.refresh')}
            onClose={() => {
              setPivotPanel(false);
              focusInput();
            }}
            onSettings={(area, idx) => {
              if (area === 'values') setDialog({ kind: 'pivotValue', pivot: activePivot, idx });
              else {
                const field = (area === 'rows' ? activePivot.rows : activePivot.cols)[idx];
                if (field !== undefined)
                  setDialog({ kind: 'pivotField', pivot: activePivot, field });
              }
            }}
            onOptions={() => setDialog({ kind: 'pivotOptions', pivot: activePivot })}
            onCalc={() => setDialog({ kind: 'pivotCalc', pivot: activePivot })}
            fail={fail}
          />
        )}
      </div>
      <div className={styles.tabs} role="tablist" aria-label={t('sheetEditor.tabs')}>
        {info?.sheets.map((s, i) =>
          s.hidden ? null : renaming === i ? (
            <input
              key={i}
              className={styles.tabRename}
              defaultValue={s.name}
              autoFocus
              onBlur={() => setRenaming(null)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  const name = e.currentTarget.value;
                  setRenaming(null);
                  void host.renameSheet(i, name).then(refreshInfo).catch(fail);
                } else if (e.key === 'Escape') setRenaming(null);
              }}
            />
          ) : (
            <button
              key={i}
              type="button"
              role="tab"
              aria-selected={i === sheet}
              className={i === sheet ? styles.tabActive : styles.tab}
              data-testid={`sheet-tab-${i}`}
              data-tab-color={s.tabColor ?? undefined}
              style={s.tabColor ? { boxShadow: `inset 0 -4px 0 #${s.tabColor}` } : undefined}
              draggable
              onDragStart={(e) => {
                setDragTab(i);
                e.dataTransfer.effectAllowed = 'move';
                e.dataTransfer.setData('text/plain', String(i));
              }}
              onDragOver={(e) => {
                if (dragTab !== null) e.preventDefault();
              }}
              onDrop={(e) => {
                e.preventDefault();
                const from = dragTab ?? Number(e.dataTransfer.getData('text/plain'));
                setDragTab(null);
                if (!Number.isInteger(from) || from === i) return;
                void exec({ cmd: 'moveSheet', from, to: i }).then((r) => {
                  if (r !== null) setSheet(i);
                });
              }}
              onDragEnd={() => setDragTab(null)}
              onContextMenu={(e) => {
                e.preventDefault();
                setTabMenu({ sheet: i, x: e.clientX, y: e.clientY - 4 });
              }}
              onClick={() => {
                setSheet(i);
                setSelection(single({ row: 0, col: 0 }));
                setChart(null);
                scrollerRef.current?.scrollTo(0, 0);
                focusInput();
              }}
              onDoubleClick={() => setRenaming(i)}
            >
              {s.protected && (
                <Lock size={10} aria-label={t('sheetEditor.protection.protectedSheet')} />
              )}
              {s.name}
            </button>
          ),
        )}
        <button
          type="button"
          className={styles.tabAdd}
          aria-label={t('sheetEditor.addSheet')}
          onClick={() => run('sheet.add')}
        >
          <Plus size={14} />
        </button>
      </div>
      <StatusBar
        left={
          <span data-testid="sheet-status">
            {status ||
              (painter
                ? t('sheetEditor.painterActive')
                : chart !== null
                  ? t('sheetEditor.chart.selected')
                  : edit
                    ? t('sheetEditor.edit')
                    : t('sheetEditor.ready'))}
          </span>
        }
        right={
          <>
            {shownStats && shownStats.numbers > 0 && (
              <>
                <span>
                  {t('sheetEditor.average')}: {fmtNum(shownStats.sum / shownStats.numbers)}
                </span>
                <span data-testid="status-sum">
                  {t('sheetEditor.sum')}: {fmtNum(shownStats.sum)}
                </span>
              </>
            )}
            {shownStats && (
              <span>
                {t('sheetEditor.count')}: {shownStats.count}
              </span>
            )}
            {info?.encrypted && (
              <span data-testid="sheet-encrypted">{t('security.protected')}</span>
            )}
            <span>{Math.round(zoom * 100)}%</span>
          </>
        }
      />
      {tabMenu && (
        <ColorMenu
          x={tabMenu.x}
          y={tabMenu.y}
          noneLabel={t('sheetEditor.tabColorNone')}
          onClose={() => setTabMenu(null)}
          onPick={(c) => {
            const target = tabMenu.sheet;
            setTabMenu(null);
            void exec({ cmd: 'setTabColor', sheet: target, color: c || null });
          }}
        />
      )}
      {passwordPrompt && (
        <PasswordDialog
          name={passwordPrompt.file.name}
          wrong={passwordPrompt.wrong}
          onSubmit={(pw) => {
            const f = passwordPrompt.file;
            setPasswordPrompt(null);
            setStatus(t('sheetEditor.loading'));
            void openFile(f, pw).catch(fail);
          }}
          onCancel={() => setPasswordPrompt(null)}
        />
      )}
      {security?.kind === 'savePassword' && (
        <SavePasswordPanel
          onSet={(password) =>
            void exec({ cmd: 'setSavePassword', password }).then(() =>
              setStatus(password ? t('security.passwordSet') : t('security.passwordRemoved')),
            )
          }
          onClose={() => setSecurity(null)}
        />
      )}
      {security?.kind === 'protect' && (
        <ProtectSheetDialog
          workbook={security.workbook}
          onClose={() => setSecurity(null)}
          onProtect={(password: string, allow: Partial<SheetPermissions>) =>
            void exec(
              security.workbook
                ? { cmd: 'protectWorkbook', password }
                : { cmd: 'protectSheet', sheet, password, allow },
            )
          }
        />
      )}
      {security?.kind === 'unprotect' && (
        <UnprotectDialog
          workbook={security.workbook}
          wrong={security.wrong}
          onClose={() => setSecurity(null)}
          onSubmit={(password) => {
            const cmd: SheetCommand = security.workbook
              ? { cmd: 'unprotectWorkbook', password }
              : { cmd: 'unprotectSheet', sheet, password };
            void host
              .exec(cmd)
              .then(async () => {
                setSecurity(null);
                await refreshInfo();
              })
              .catch((e: unknown) => {
                const msg = e instanceof Error ? e.message : String(e);
                if (msg.includes(WRONG_PROTECTION_PASSWORD))
                  setSecurity({ kind: 'unprotect', workbook: security.workbook, wrong: true });
                else fail(e);
              });
          }}
        />
      )}
      {menu && (menu.kind === 'fontColor' || menu.kind === 'fillColor') && (
        <ColorMenu
          x={menu.x}
          y={menu.y}
          noneLabel={
            menu.kind === 'fillColor' ? t('sheetEditor.fmt.noFill') : t('sheetEditor.automatic')
          }
          onClose={() => setMenu(null)}
          onPick={(c) => {
            void formatSel(menu.kind === 'fillColor' ? { fillColor: c } : { fontColor: c });
            focusInput();
          }}
        />
      )}
      {menu && menu.kind !== 'fontColor' && menu.kind !== 'fillColor' && (
        <Menu
          x={menu.x}
          y={menu.y}
          items={menuItems(menu.kind)}
          testId={`menu-${menu.kind}`}
          onClose={() => setMenu(null)}
          onPick={(id) => pickMenu(menu.kind, id)}
        />
      )}
      {filterMenu && (
        <FilterMenu
          {...common}
          col={filterMenu.col}
          x={filterMenu.x}
          y={filterMenu.y}
          onClose={() => {
            setFilterMenu(null);
            focusInput();
          }}
        />
      )}
      {dialog?.kind === 'format' && (
        <FormatCellsDialog
          open
          initial={fmt}
          presets={presets.numberFormats}
          onClose={common.onClose}
          onApply={(patch, borders) => {
            void (async () => {
              if (Object.keys(patch).length) await formatSel(patch);
              for (const b of borders) await borderSel(b);
              common.onClose();
            })();
          }}
        />
      )}
      {dialog?.kind === 'sort' && <SortDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'cf' && <CfDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'dv' && <ValidationDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'dataTable' && <DataTableDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'solver' && (
        <SolverDialog
          {...common}
          objective={`${colName(selection.active.col)}${selection.active.row + 1}`}
        />
      )}
      {dialog?.kind === 'goalSeek' && (
        <GoalSeekDialog
          {...common}
          target={`${colName(selection.active.col)}${selection.active.row + 1}`}
        />
      )}
      {dialog?.kind === 'chart' && <ChartDialog {...common} index={dialog.index} />}
      {dialog?.kind === 'pivot' && (
        <PivotDialog
          {...common}
          range={toRange(rangeOf(selection))}
          sheets={info?.sheets.map((s) => s.name) ?? []}
          labels={pivotLabels}
          onCreated={(s, index) => {
            void host
              .exec<PivotInfo[]>({ cmd: 'pivots', sheet: s })
              .then((list) => {
                const p = list?.[index];
                setSheet(s);
                setPivotPanel(true);
                if (p) setSelection(single({ row: p.location.r0, col: p.location.c0 }));
                setStatus(t('sheetEditor.pivot.created'));
              })
              .catch(fail);
          }}
        />
      )}
      {dialog?.kind === 'pivotField' && (
        <PivotFieldDialog
          {...common}
          pivot={dialog.pivot}
          labels={pivotLabels}
          field={dialog.field}
        />
      )}
      {dialog?.kind === 'pivotValue' && (
        <PivotValueDialog {...common} pivot={dialog.pivot} labels={pivotLabels} idx={dialog.idx} />
      )}
      {dialog?.kind === 'pivotOptions' && (
        <PivotOptionsDialog {...common} pivot={dialog.pivot} labels={pivotLabels} />
      )}
      {dialog?.kind === 'pivotCalcItem' && (
        <PivotCalcItemDialog {...common} pivot={dialog.pivot} labels={pivotLabels} />
      )}
      {dialog?.kind === 'pivotCalc' && (
        <PivotCalcDialog {...common} pivot={dialog.pivot} labels={pivotLabels} />
      )}
      {dialog?.kind === 'pivotSource' && (
        <PivotSourceDialog {...common} pivot={dialog.pivot} labels={pivotLabels} />
      )}
      {dialog?.kind === 'pivotMove' && (
        <PivotMoveDialog
          {...common}
          pivot={dialog.pivot}
          labels={pivotLabels}
          sheets={info?.sheets.map((x) => x.name) ?? []}
          onMoved={(s, index) => {
            void host
              .exec<PivotInfo[]>({ cmd: 'pivots', sheet: s })
              .then((list) => {
                const p = list?.[index];
                setSheet(s);
                if (p) setSelection(single({ row: p.location.r0, col: p.location.c0 }));
              })
              .catch(fail);
          }}
        />
      )}
      {dialog?.kind === 'pageSetup' && <PageSetupDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'find' && (
        <FindDialog
          {...common}
          replace={dialog.replace}
          onSelect={(at) => {
            setSheet(at.sheet);
            select(single({ row: at.row, col: at.col }));
          }}
        />
      )}
      {dialog?.kind === 'goto' && (
        <GoToDialog
          host={host}
          sheet={sheet}
          fail={fail}
          onClose={common.onClose}
          onGo={(s, g) => {
            setSheet(s);
            select(span({ row: g.r0, col: g.c0 }, { row: g.r1, col: g.c1 }));
          }}
        />
      )}
      {dialog?.kind === 'pasteSpecial' && (
        <PasteSpecialDialog
          onClose={common.onClose}
          onPaste={(mode, transpose, skip) => {
            void internalPaste(mode, transpose, skip).then((ok) => {
              if (!ok) setStatus(t('sheetEditor.pasteSpecial.empty'));
            });
          }}
        />
      )}
      {dialog?.kind === 'message' && (
        <MessageDialog title={dialog.title} message={dialog.message} onClose={common.onClose} />
      )}
    </div>
  );
}
