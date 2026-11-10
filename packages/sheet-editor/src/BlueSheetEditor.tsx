import { GlyphStore, ImageStore, PX_PER_PT, renderPage } from '@blueoffice/canvas-host';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import type {
  TimelineInfo,
  SlicerInfo,
  BorderPreset,
  BorderSpec,
  CellFormatState,
  ChartInfo,
  SheetSmartArtInfo,
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
  FlashFillResult,
  InvalidCellsResult,
  SheetPermissions,
  FormControlInfo,
  SheetSecurityResult,
  SparklineGroupInfo,
  TableInfo,
  TableStyleEntry,
} from '@blueoffice/platform';
import {
  PASSWORD_REQUIRED,
  SECURITY_ERRORS,
  SHEET_PROTECTED,
  THEME_MIME,
  WORKBOOK_PROTECTED,
  WRONG_PASSWORD,
  WRONG_PROTECTION_PASSWORD,
  mentionFields,
  mimeFor,
  notifyMentions,
  openExtensions,
  withExtension,
} from '@blueoffice/platform';
import {
  CommentThreads,
  type MentionSubmit,
  Ribbon,
  ribbonLabels,
  RIBBON_CONTEXTS,
  CommandSearch,
  StatusBar,
  TitleBar,
  useCommandSearchShortcut,
  useResolvedTheme,
  TaskPaneHost,
  useTaskPaneLayout,
  type RibbonTab,
  type TaskPaneDef,
  type ThemeSetting,
  SHEET_MENUS,
  isContextMenuKey,
  registryMenuLookup,
  resolveMenu,
  ribbonMenuLookup,
  type SheetMenuContext,
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
  ChartSpline,
  ClipboardCheck,
  History,
  SpellCheck,
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
  ListOrdered,
  ListTree,
  LocateFixed,
  Layers,
  Lock,
  LockOpen,
  MoveHorizontal,
  MoveVertical,
  Omega,
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
  SeparatorHorizontal,
  Settings,
  Settings2,
  ShieldCheck,
  Target,
  Sparkles,
  Grid3x3,
  ListFilter,
  FunnelX,
  FunnelPlus,
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
  Network,
  Pencil,
  Underline,
  Undo2,
  ZoomIn,
  ZoomOut,
  MessageSquareText,
  RectangleHorizontal,
  SquareCheck,
  CircleDot,
  SquareChevronDown,
  List,
  GalleryVertical,
  ChevronsUpDown,
  SquareDashed,
  Type,
  SlidersHorizontal,
  CopyX,
  Combine,
  CircleSlash,
  ChevronsDownUp,
  WandSparkles,
  Languages,
  ListX,
  ImagePlus,
  ScanText,
  Volume2,
  SquareTerminal,
  Braces,
  Database,
  FileCode,
  FileSpreadsheet,
  FileText,
  Globe,
} from 'lucide-react';
import { recordSheetHost, ScriptPane, ScriptRecorder, sheetScriptHost } from '@blueoffice/script';
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
  FileBackstage,
  FileProtectionUi,
  ProtectionStatus,
  useFileProtection,
  MacroBanner,
  PasswordDialog,
  PdfExportDialog,
  SaveAsTypeDialog,
  SavePasswordPanel,
  SmartArtDialog,
  SymbolPanel,
  PrintPanel,
  clipboardHistory,
  fromBase64,
  planOpen,
  ReadAloudLink,
  registerReadAloudCommands,
  registerTranslateCommands,
  TranslateLink,
  useTranslate,
  type TranslateAction,
  saveName,
  taskPaneLabels,
  toBase64,
  useReadAloud,
  useScriptBindings,
  type BackstagePropertiesAccess,
  type ClipItem,
  type SmartArtDraft,
  commandLabel,
  shortcutCommands,
  useContextMenu,
  useCustomShortcuts,
  type OpenContextMenu,
} from '@blueoffice/editor';
import { cycleReference } from './formulaRef';
import { sheetReadUnits } from './readAloud';
import { asTextInput, translateCells, type CellTranslateMode } from './translate';
import { useSheetQueries } from './queries';
import {
  SheetClipboardUi,
  sheetPasteCommand,
  type SheetPasteBase,
  type SheetPasteChoice,
} from './pasteOptions';
import { AnalysisDialog } from './analysis';
import {
  ConsolidateDialog,
  InvalidCircles,
  MAX_CIRCLES,
  RemoveDuplicatesDialog,
  SubtotalDialog,
  TextToColumnsDialog,
  type CircleRect,
} from './dataTools';
import {
  AuditIcons,
  CalcOptionsDialog,
  CalcStatus,
  CircularWarning,
  ErrorCheckDialog,
  EvaluateFormulaDialog,
  TraceArrowsOverlay,
  WatchWindow,
  useAudit,
} from './audit';
import { ProtectSheetDialog, UnprotectDialog } from './protection';
import { ScenarioDialog } from './scenarios';
import {
  InsertSparklinesDialog,
  SparklineOptionsDialog,
  groupAt,
  registerSparklineCommands,
  sparklineTabGroups,
  useSparklines,
} from './sparklines';
import {
  TableAutoBar,
  TableNameDialog,
  TableResizeDialog,
  TableTotalsDialog,
  registerTableCommands,
  tableAt,
  tableDesignTabGroups,
  useTables,
} from './tableDesign';
import { SheetCheckDocumentDialog, type SheetGoTo } from './inspect';
import { SheetShowChangesPane } from './showChanges';
import { SheetEditSpelling, SheetSpellDialog, useSheetSpelling } from './spelling';
import {
  SaveChartTemplateDialog,
  chartDesignGroups,
  exportChartTemplate,
  importChartTemplate,
  layoutTexts,
  templateMenuItems,
  templateOfItem,
  useChartGallery,
} from './chartStyles';
import { FormatControlDialog, FormControlLayer, INSERTABLE } from './formControls';
import {
  type AltTextTarget,
  CellPictureAltDialog,
  clickCheckbox,
  deleteCheckboxes,
  loadRequestedImages,
  mediaState,
  registerCellMediaCommands,
  sheetMediaSource,
} from './cellMedia';
import type { CellMediaState, DocProperties } from '@blueoffice/platform';
import { CfManagerDialog } from './cfRules';
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
  PasteSpecialDialog,
  ValidationDialog,
  type MenuItem,
} from './dialogs';
import { FormatCellsDialog, type NumberPreset } from './FormatCellsDialog';
import { PageSetupDialog, PrintPreviewDialog } from './pageSetup';
import { AdvancedFilterDialog, SortDialog } from './sortFilter';
import {
  CustomViewsDialog,
  PageBreakOverlay,
  PageLayoutView,
  sheetViewTabGroups,
  TempViewsDialog,
  useSheetViews,
  useTempViews,
} from './sheetViews';
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
import { PeerSelections, usePeerBoxes } from './peers';
import styles from './SheetEditor.module.css';

/**
 * Contexts whose tab is activated when they appear (CORE-UI-001);
 * the active cell moves through tables and sparklines while typing: those tabs only show.
 */
const AUTO_CONTEXTS = RIBBON_CONTEXTS.filter((c) => c !== 'table' && c !== 'sparkline');

export interface BlueSheetEditorProps {
  platform: Platform;
  language?: Language;
  /** Light, dark or follow the OS (the embedding host sets it, prompt §6A.2). */
  theme?: ThemeSetting;
}

/** Editor state passed to commands. */
/** Commands F4 repeats on the new selection (CORE-UI-004); dialogs and menus are left out. */
export const SHEET_REPEATABLE =
  /^(?:format\.(?:bold|italic|underline|strike|align\w+|wrap|indentInc|indentDec|fontGrow|fontShrink|percent|comma|currency|date|mergeAcross|unmerge|clear|lockCell)|cells\.(?:insert|delete|hide|unhide|autofit)\w*|data\.sort(?:Asc|Desc)|edit\.(?:clear|fillDown|fillRight))$/;

/** An open BlueSheet context menu (CORE-UI-003); `sheet` for a sheet tab. */
type SheetMenu = OpenContextMenu & { context: SheetMenuContext; sheet?: number };

export interface SheetEditorState {
  host: SheetHost;
  sheet: number;
  selection: Selection;
  hasWorkbook: boolean;
  /** Formatting of the active cell (ribbon toggles). */
  fmt: CellFormatState | null;
  /** Selected chart (click on a chart). */
  chart: number | null;
  /** Selected SmartArt diagram (CORE-SMART-005). */
  smartArt: number | null;
  /** Pivot table containing the active cell. */
  pivot: number | null;
  /** Every range of a multi-range (Ctrl) selection, the current one last. */
  ranges: Range[];
  /** The current sheet is protected (§18.3.1.85). */
  sheetProtected: boolean;
  /** The workbook structure is locked (§18.2.29). */
  structureLocked: boolean;
  /** Sparkline group of the active cell. */
  sparkline: SparklineGroupInfo | null;
  /** Table of the active cell (SHEET-TABLE-001). */
  table: TableInfo | null;
  /** Automatic table behaviour options. */
  tableAuto: { autoExpand: boolean; autoCalc: boolean } | null;
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
/** Fonts offered by the symbol picker besides the active cell's font. */
const SYMBOL_FONTS = [
  'Calibri',
  'Arial',
  'Times New Roman',
  'Tahoma',
  'Carlito',
  'Liberation Sans',
];
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
  | { kind: 'advancedFilter' }
  | { kind: 'cf' }
  | { kind: 'cfManage' }
  | { kind: 'dv' }
  | { kind: 'goalSeek' }
  | { kind: 'scenarios' }
  | { kind: 'sparklineInsert' }
  | { kind: 'sparklineOptions' }
  | { kind: 'tableRename' }
  | { kind: 'tableResize' }
  | { kind: 'tableTotals' }
  | { kind: 'checkDocument' }
  | { kind: 'dataTable' }
  | { kind: 'solver' }
  | { kind: 'analysis' }
  | { kind: 'textToColumns' }
  | { kind: 'dedupe' }
  | { kind: 'consolidate' }
  | { kind: 'subtotal' }
  | { kind: 'chart'; index: number | null }
  | { kind: 'find'; replace: boolean }
  | { kind: 'goto' }
  | { kind: 'pasteSpecial' }
  | { kind: 'pageSetup' }
  | { kind: 'printPreview' }
  | { kind: 'customViews' }
  | { kind: 'tempViews' }
  | { kind: 'print' }
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
      /** A SmartArt diagram instead of a chart (CORE-SMART-005). */
      smart?: boolean;
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
  // Macro recorder (CORE-SCRIPT-002): it sees every sheet host call the editor makes.
  const [recorder] = useState(() => new ScriptRecorder('sheet'));
  const host = useMemo(
    () => (platform.sheet ? recordSheetHost(platform.sheet, recorder) : platform.sheet),
    [platform.sheet, recorder],
  );
  const [, setRecorderTick] = useState(0);
  useEffect(() => recorder.subscribe(() => setRecorderTick((n) => n + 1)), [recorder]);
  const theme = useResolvedTheme(themeSetting);
  const decimal: ',' | '.' = i18n.language === 'en-US' ? '.' : ',';

  const [info, setInfo] = useState<WorkbookInfo | null>(null);
  /** The template the open workbook was created from (CORE-FMT-001: "Edit template"). */
  const [templateFile, setTemplateFile] = useState<PendingFile | null>(null);
  /** "Save as" file type dialog. */
  const [saveAsType, setSaveAsType] = useState(false);
  /** PDF/A, tagged PDF and bookmarks dialog (CORE-FMT-002). */
  const [pdfOptions, setPdfOptions] = useState(false);
  const [fileName, setFileName] = useState('');
  const [sheet, setSheet] = useState(0);
  /** The sheet shown now (read by BlueScript runs). */
  const sheetNow = useRef(sheet);
  useEffect(() => {
    sheetNow.current = sheet;
  }, [sheet]);
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
  // Sparkline group of the active cell (SHEET-SPARK-001): the contextual tab and its commands.
  const sparklines = useSparklines(host, sheet, revision);
  const sparkGroup = groupAt(sparklines, selection.active.row, selection.active.col);
  // Table of the active cell (SHEET-TABLE-001): the Table Design contextual tab.
  const tables = useTables(host, sheet, revision);
  const activeTable = tableAt(tables, selection.active.row, selection.active.col);
  const tableAuto = useMemo(
    () => (tables ? { autoExpand: tables.autoExpand, autoCalc: tables.autoCalc } : null),
    [tables],
  );
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
  /** Selected SmartArt diagram, its anchors and on-screen box (px) (CORE-SMART-005). */
  const [smartArt, setSmartArt] = useState<number | null>(null);
  const [smartInfo, setSmartInfo] = useState<SheetSmartArtInfo | null>(null);
  const [smartBox, setSmartBox] = useState<Rect | null>(null);
  const [smartArtDialog, setSmartArtDialog] = useState<{
    mode: 'insert' | 'edit';
    index: number | null;
    initial: SmartArtDraft;
  } | null>(null);
  /** "Save as template…" of the selected chart is open. */
  const [saveTemplate, setSaveTemplate] = useState(false);
  const [pivots, setPivots] = useState<{ sheet: number; list: PivotInfo[] }>({
    sheet: -1,
    list: [],
  });
  /** The field list shows when the active cell is inside a pivot table. */
  const [pivotPanel, setPivotPanel] = useState(true);
  /** Threaded comments side panel ([MS-XLSX] threaded comments). */
  const [commentsOpen, setCommentsOpen] = useState(false);
  // Show Changes / version history pane (CORE-VERSION-001).
  const [versionsOpen, setVersionsOpen] = useState(false);
  /** BlueScript pane (CORE-SCRIPT-001). */
  const [scriptOpen, setScriptOpen] = useState(false);
  // Task pane layout of BlueSheet, remembered per app (CORE-UI-006).
  const [paneLayout, setPaneLayout] = useTaskPaneLayout('sheet');
  const [threads, setThreads] = useState<SheetThreadInfo[]>([]);
  /** Symbol picker panel (insert into the cell being edited, else the active cell). */
  const [symbolsOpen, setSymbolsOpen] = useState(false);
  /** Clipboard history panel (Edit group launcher). */
  const [clipPanel, setClipPanel] = useState(false);
  /** The last paste of copied cells while its mode can change (paste options button). */
  const [pasteUi, setPasteUi] = useState<{
    n: number;
    mode: SheetPasteChoice;
    base: SheetPasteBase;
    /** The selection the paste set: another selection ends the paste options. */
    sel: Selection;
  } | null>(null);
  /** The last copy was a cut (pasted once: no paste options). */
  const [lastCut, setLastCut] = useState(false);
  const pivotLabels = usePivotLabels();

  const scrollerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const barRef = useRef<HTMLInputElement>(null);
  const spellInputs = useMemo(() => [inputRef, barRef], []);
  /** Input that last had the caret while editing (the symbol picker inserts there). */
  const lastInput = useRef<'cell' | 'bar'>('cell');
  const glyphs = useMemo(() => (host ? new GlyphStore(host) : null), [host]);
  // Pictures in cells and IMAGE pictures of the display list (SHEET-CELL-001).
  const cellImages = useMemo(() => (host ? new ImageStore(sheetMediaSource(host)) : null), [host]);
  const renderSeq = useRef(0);
  const drag = useRef<Drag | null>(null);
  const fillTo = useRef<Range | null>(null);
  const composing = useRef(false);
  /** TSV text of our last copy (paste uses the internal cell records when it matches). */
  const [lastCopy, setLastCopy] = useState<string | null>(null);
  /** Other ranges of a Ctrl multi-range selection (the current one is `selection`). */
  const [extra, setExtra] = useState<Range[]>([]);
  const [extraRects, setExtraRects] = useState<Rect[]>([]);
  /** Sheet whose invalid data is circled (`null`: none). */
  const [circlesOn, setCirclesOn] = useState<number | null>(null);
  const [circleRects, setCircleRects] = useState<CircleRect[]>([]);
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
      // CORE-SEC-001: the workbook is open read-only.
      else if (message.includes(SECURITY_ERRORS.readOnly)) setStatus(t('protect.err.readOnly'));
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
  const [searchOpen, setSearchOpen] = useState(false);
  useCommandSearchShortcut(() => setSearchOpen(true));
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
    async (f: PendingFile, password?: string, editTemplate = false): Promise<boolean> => {
      if (!host) return false;
      try {
        // CORE-FMT-001: a template (.xltx / .xltm) starts a new, untitled workbook of the
        // matching type unless the user asked to edit the template itself.
        let fromTemplate = false;
        let shown = f.name;
        await load(async () => {
          const wb = await host.open(f.bytes, f.name, password);
          const plan = planOpen(wb.variant, f.name, editTemplate);
          setTemplateFile(plan.variant?.template ? f : null);
          if (!plan.saveAs) return wb;
          await host.exec({ cmd: 'saveFormat', ext: plan.saveAs });
          fromTemplate = true;
          shown = plan.name;
          return { ...wb, name: plan.name, variant: plan.saveAs };
        }, f.name).then(() => {
          if (fromTemplate) {
            setFileName(shown);
            setStatus(t('fileVariants.fromTemplate', { name: f.name }));
          }
        });
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
    [host, load, t],
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
        await Promise.all([glyphs.ensure(page), cellImages?.ensure(page)]);
        if (seq !== renderSeq.current) return;
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (!canvas || !ctx) return;
        const dpr = window.devicePixelRatio || 1;
        canvas.width = Math.round(size.w * dpr);
        canvas.height = Math.round(size.h * dpr);
        canvas.style.width = `${size.w}px`;
        canvas.style.height = `${size.h}px`;
        renderPage(ctx, page, glyphs, scale * dpr, cellImages ?? undefined);
        canvas.dataset.rendered = String(seq);
        // IMAGE formulas: fetch what the engine still needs, then draw again.
        if (await loadRequestedImages(host, platform.images)) setRevision((r) => r + 1);
      })().catch(fail);
    });
    return () => cancelAnimationFrame(frame);
  }, [host, glyphs, cellImages, info, view, size, scale, revision, fail, platform.images]);

  const px = useCallback(
    (x: Rect | null) => x && { x: x.x * scale, y: x.y * scale, w: x.w * scale, h: x.h * scale },
    [scale],
  );

  // Live collaboration: share the own selection, show the collaborators' ones.
  const sheetName = summary?.name;
  const peerBoxes = usePeerBoxes(host, view, sheetName, px, revision);
  useEffect(() => {
    if (sheetName !== undefined) host?.collabSelect?.(sheetName, rangeA1(rangeOf(selection)));
  }, [host, sheetName, selection]);

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

  // Circles around invalid data: the list is fetched again after every edit, so a
  // corrected cell loses its circle.
  useEffect(() => {
    if (!host || !info || circlesOn !== sheet) return;
    let cancelled = false;
    void (async () => {
      const r = await host.exec<InvalidCellsResult>({ cmd: 'invalidCells', sheet });
      const cells = r.cells.slice(0, MAX_CIRCLES);
      const rects = await Promise.all(
        cells.map(([row, col]) => host.rangeRect(view, row, col, row, col)),
      );
      if (!cancelled) setCircleRects(rects.map(px).filter((x): x is Rect => !!x));
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, circlesOn, view, px, revision]);

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

  // Checkbox / picture of the active cell, and the alt-text dialog (SHEET-CELL-001).
  const [activeMedia, setActiveMedia] = useState<CellMediaState | null>(null);
  const [altDialog, setAltDialog] = useState<AltTextTarget | null>(null);
  // Checkbox / picture of the active cell (Space toggles a checkbox).
  useEffect(() => {
    if (!host || !info) return;
    let cancelled = false;
    const { row, col } = selection.active;
    void mediaState(host, sheet, row, col)
      .then((m) => !cancelled && setActiveMedia(m))
      .catch(() => {});
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

  // Selected SmartArt: anchors and selection box.
  useEffect(() => {
    if (!host || smartArt === null) return;
    let cancelled = false;
    void (async () => {
      const a = await host
        .exec<SheetSmartArtInfo>({ cmd: 'smartArtInfo', sheet, index: smartArt })
        .catch(() => null);
      if (cancelled) return;
      if (!a) {
        setSmartArt(null);
        return;
      }
      const r = await host.rangeRect(view, a.from[0], a.from[1], a.to[0] - 1, a.to[1] - 1);
      if (cancelled) return;
      setSmartInfo(a);
      setSmartBox(px(r));
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, smartArt, sheet, view, px, revision]);

  // Timelines of the sheet: a strip of periods over the grid.
  const [timelines, setTimelines] = useState<{ info: TimelineInfo; rect: Rect }[]>([]);
  const timelineAnchor = useRef<number | null>(null);
  useEffect(() => {
    if (!host) return;
    let cancelled = false;
    void (async () => {
      const list = (await host.exec<TimelineInfo[]>({ cmd: 'timelines', sheet })) ?? [];
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
      if (!cancelled)
        setTimelines(placed.filter((x): x is { info: TimelineInfo; rect: Rect } => !!x));
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, sheet, view, px, revision]);

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

  // Form controls of the sheet: selected one (design mode), list, settings dialog.
  // The selection belongs to one sheet of one opened file.
  const [formSelAt, setFormSelAt] = useState<{ key: string; index: number } | null>(null);
  const formKey = `${loadSeq}:${sheet}`;
  const formSel = formSelAt?.key === formKey ? formSelAt.index : null;
  const setFormSel = useCallback(
    (index: number | null) => setFormSelAt(index === null ? null : { key: formKey, index }),
    [formKey],
  );
  const [formList, setFormList] = useState<FormControlInfo[]>([]);
  const [formDialog, setFormDialog] = useState<FormControlInfo | null>(null);

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

  // Spelling of typed text and of every text cell (CORE-SPELL-006).
  const spelling = useSheetSpelling(host, fileName, i18n.language);
  const { setDialog: setSpellDialog, toggle: toggleSpelling, asYouType } = spelling;

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

  // Get & Transform data (SHEET-PQ-001): ribbon actions, query editor, Queries pane.
  const queries = useSheetQueries({
    host,
    platform,
    sheet,
    cell: selection.active,
    onChanged: refreshInfo,
    goToSheet: setSheet,
    fail,
  });
  const {
    startFrom: queryFrom,
    refresh: queryRefresh,
    togglePane: toggleQueries,
    paneOpen: queriesOpen,
  } = queries;

  /** Open the SmartArt dialog on diagram `index` of sheet `sh` (CORE-SMART-005). */
  const openSmartArt = useCallback(
    async (sh: number, index: number) => {
      if (!host) return;
      const a = await host
        .exec<SheetSmartArtInfo>({ cmd: 'smartArtInfo', sheet: sh, index })
        .catch(() => null);
      if (!a) return;
      setSmartArtDialog({
        mode: 'edit',
        index,
        initial: { layout: a.layout, items: a.items, colors: a.colors, style: a.style },
      });
    },
    [host],
  );

  // Workbook views (SHEET-VIEW-001): view type, show formulas, temporary views.
  const sheetViews = useSheetViews({
    host,
    sheet,
    revision,
    loadKey: loadSeq,
    zoom,
    setZoom,
    exec,
  });
  const { mode: viewMode, setMode: setViewMode, toggleFormulas } = sheetViews;
  const showingFormulas = !!sheetViews.state?.showFormulas;
  const tempViews = useTempViews();

  // Formula auditing and calculation (SHEET-AUDIT-001).
  const audit = useAudit({ host, sheet, revision, onDone: refreshInfo, fail });
  const {
    trace: auditTrace,
    calculate: auditCalculate,
    removeArrows,
    setDialog: setAuditDialog,
    setWatchOpen,
    traces: auditTraces,
    watchOpen,
  } = audit;

  // Table style gallery (SHEET-TABLE-001): swatch colours from the engine.
  const loadTableStyles = useCallback(
    async () => (host ? ((await host.exec<TableStyleEntry[]>({ cmd: 'tableStyles' })) ?? []) : []),
    [host],
  );
  // Chart Design gallery of the selected chart (previews follow the edits).
  const loadGallery = useChartGallery(host, sheet, chart, revision);
  const goToRange = useCallback((to: SheetGoTo) => {
    setSheet(to.sheet);
    setSelection({
      anchor: { row: to.r0, col: to.c0 },
      focus: { row: to.r1, col: to.c1 },
      active: { row: to.r0, col: to.c0 },
    });
  }, []);
  const spellShow = useCallback(
    (s: number, row: number, col: number) =>
      goToRange({ sheet: s, r0: row, c0: col, r1: row, c1: col }),
    [goToRange],
  );
  // CORE-SEC-001: final mark, read-only recommendation, password to modify,
  // labels, protected ranges.
  const runSecurity = useCallback(
    async (cmd: SheetCommand) => {
      if (!host) return null;
      try {
        const r = await host.exec<SheetSecurityResult>(cmd);
        await refreshInfo();
        return r;
      } catch (e) {
        return { error: e instanceof Error ? e.message : String(e) };
      }
    },
    [host, refreshInfo],
  );
  const protection = useFileProtection(runSecurity, (platform as Partial<Platform>).auth?.user);
  const { run: runProtect, setUi: setProtectUi } = protection;
  const protectionOpened = useRef(protection.opened);
  useEffect(() => {
    protectionOpened.current = protection.opened;
  });
  // Each opened workbook: identity, then the read-only prompt when it asks for one.
  useEffect(() => {
    if (loadSeq > 0) void protectionOpened.current();
  }, [loadSeq]);

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
      const r = await exec<{ id?: string }>(cmd);
      setThreadsRev((n) => n + 1);
      return r;
    },
    [exec],
  );
  /** Posts a comment / reply with its @mentions and task, then notifies the people (CORE-COMMENT-001). */
  const postComment = useCallback(
    async (target: { row: number; col: number } | { parent: string }, extra: MentionSubmit) => {
      const r = await commentCmd({
        cmd: 'addComment',
        sheet,
        ...target,
        text: extra.text,
        author: platform.auth?.user?.name ?? '',
        date: new Date().toISOString(),
        ...mentionFields(extra.mentions, extra.assignee),
      });
      if (r?.id) {
        void notifyMentions(platform.people, {
          commentId: r.id,
          text: extra.text.trim(),
          mentions: extra.mentions,
          assignee: extra.assignee,
          app: 'sheet',
        });
      }
    },
    [commentCmd, sheet, platform],
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
      assignTo: t('comments.assignTo'),
      noMatches: t('comments.noMatches'),
      people: t('comments.people'),
      assignedTo: t('comments.assignedTo', { name: '{name}' }),
      taskDone: t('comments.taskDone', { name: '{name}' }),
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

  // Read aloud (CORE-LANG-003): the selected cells row by row; the active cell follows.
  const readAloud = useReadAloud(platform.speech, 'cell');
  const [readAloudLink] = useState(() => new ReadAloudLink<SheetEditorState>());
  const beginRead = useCallback(
    async (st: SheetEditorState) => {
      const sel = st.selection;
      const { units, cells } = await sheetReadUnits(
        st.host,
        st.sheet,
        rangeOf(sel),
        spelling.lang || i18n.language || 'vi-VN',
      );
      await readAloudLink.aloud()?.start({
        units,
        onUnit: (i) => {
          const c = cells[i];
          if (!c) return;
          const next = { ...sel, active: { row: c.row, col: c.col } };
          setSelection(next);
          void reveal(next);
        },
      });
    },
    [spelling.lang, i18n.language, reveal, readAloudLink],
  );
  useEffect(() => {
    readAloudLink.attach(readAloud, beginRead);
  });

  // Translation (CORE-LANG-001): the active cell in the pane; the selected cells in place or
  // into new columns.
  const translate = useTranslate(platform.translation, platform.clipboard);
  const [translateLink] = useState(() => new TranslateLink());
  useEffect(() => {
    translateLink.attach(translate);
  });
  const beginTranslate = useCallback(
    async (st: SheetEditorState, action?: string) => {
      const range = rangeOf(st.selection);
      const { row, col } = st.selection.active;
      const cell = await st.host.cell(st.sheet, row, col);
      const cellsAction = (id: string, mode: CellTranslateMode): TranslateAction => ({
        id,
        label: t(`translate.actions.${id}`),
        run: async ({ from, to, signal, onProgress }) => {
          const provider = platform.translation;
          if (!provider) return;
          const n = await translateCells(st.host, st.sheet, range, provider, from, to, mode, {
            signal,
            onProgress,
          });
          if (n) await refreshInfo();
          return n ? t('translate.done.cells', { count: n }) : t('translate.done.nothing');
        },
      });
      const text = cell.input.startsWith('=') ? '' : cell.display;
      translateLink.open({
        source: {
          text,
          replace: async (tr) => {
            await st.host.setInputs(st.sheet, [[row, col, asTextInput(tr)]]);
            await refreshInfo();
          },
        },
        actions: [
          cellsAction('cellsNewColumn', 'newColumn'),
          cellsAction('cellsInPlace', 'inPlace'),
        ],
        autoRun: action,
      });
    },
    [platform.translation, refreshInfo, t, translateLink],
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
      setLastCut(cut);
      clipboardHistory.push({ app: 'sheet', kind: 'cells', text });
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

  /** Paste our own copied cells, then offer the other paste modes. */
  const ownPaste = useCallback(async (): Promise<boolean> => {
    if (!host) return false;
    const r = rangeOf(selection);
    const base: SheetPasteBase = { sheet, row: r.r0, col: r.c0, range: toRange(r) };
    const res = await host.exec<{ range: SheetRange } | null>(sheetPasteCommand('all', base));
    if (!res) return false;
    const sel = span(
      { row: res.range.r0, col: res.range.c0 },
      { row: res.range.r1, col: res.range.c1 },
    );
    setSelection(sel);
    await refreshInfo();
    setPasteUi(lastCut ? null : (old) => ({ n: (old?.n ?? 0) + 1, mode: 'all', base, sel }));
    return true;
  }, [host, selection, sheet, refreshInfo, lastCut]);

  /** Redo the last paste in another mode: undo it, paste again (one undo step overall). */
  const switchPaste = useCallback(
    async (choice: SheetPasteChoice) => {
      const ui = pasteUi && pasteUi.sel === selection ? pasteUi : null;
      if (!host || !ui || ui.mode === choice) return;
      if (!(await host.undo())) return;
      let res = await host.exec<{ range: SheetRange } | null>(sheetPasteCommand(choice, ui.base));
      if (!res) {
        res = await host.exec<{ range: SheetRange } | null>(sheetPasteCommand(ui.mode, ui.base));
        choice = ui.mode;
      }
      if (res) {
        const got = res.range;
        const sel = span({ row: got.r0, col: got.c0 }, { row: got.r1, col: got.c1 });
        setSelection(sel);
        setPasteUi({ n: ui.n + 1, mode: choice, base: ui.base, sel });
      }
      await refreshInfo();
    },
    [host, pasteUi, selection, refreshInfo],
  );

  /** Paste text (tab-separated values) at the selection. */
  const pasteTsv = useCallback(
    async (text: string) => {
      if (!host) return;
      const rows = parseTsv(text);
      if (!rows.length) return;
      const r = rangeOf(selection);
      await host.setInputs(sheet, pasteCells({ row: r.r0, col: r.c0 }, rows));
      const lastRow = r.r0 + rows.length - 1;
      const lastCol = r.c0 + Math.max(...rows.map((x) => x.length)) - 1;
      setSelection(span({ row: r.r0, col: r.c0 }, { row: lastRow, col: lastCol }));
      await refreshInfo();
    },
    [host, selection, sheet, refreshInfo],
  );

  /** Paste an item of the clipboard history: our latest copy as cells, others as text. */
  const pasteItem = useCallback(
    async (item: ClipItem) => {
      if (item.app === 'sheet' && lastCopy !== null && norm(item.text) === norm(lastCopy)) {
        if (await ownPaste()) return;
      }
      await pasteTsv(item.text);
    },
    [lastCopy, ownPaste, pasteTsv],
  );

  const paste = useCallback(async () => {
    if (!host) return;
    const text = await platform.clipboard.readText();
    // Our own copy: paste the cell records (formats, adjusted formulas).
    if (lastCopy !== null && norm(text) === norm(lastCopy)) {
      if (await ownPaste()) return;
    }
    const rows = parseTsv(text);
    if (!rows.length) return;
    const r = rangeOf(selection);
    await host.setInputs(sheet, pasteCells({ row: r.r0, col: r.c0 }, rows));
    const lastRow = r.r0 + rows.length - 1;
    const lastCol = r.c0 + Math.max(...rows.map((x) => x.length)) - 1;
    setSelection(span({ row: r.r0, col: r.c0 }, { row: lastRow, col: lastCol }));
    await refreshInfo();
  }, [host, platform, selection, sheet, refreshInfo, ownPaste, lastCopy]);

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

  /** Save as the type picked in the "Save as" dialog (CORE-FMT-001). */
  const saveAsTypeChosen = useCallback(
    async (ext: string) => {
      if (!host) return;
      try {
        const r = await host.exec<{ ok: boolean }>({ cmd: 'saveFormat', ext });
        if (!r.ok) return;
        const name = withExtension(fileName || t('sheetEditor.untitled'), ext);
        setFileName(name);
        await refreshInfo();
        const saved = await platform.files.saveFile(
          await host.save('xlsx', sheet),
          name,
          mimeFor(name),
        );
        if (saved) setStatus(t('sheetEditor.saved', { name: saved }));
      } catch (e) {
        fail(e);
      }
    },
    [host, fileName, t, refreshInfo, platform, sheet, fail],
  );

  const [shortcutsShown, setShortcutsShown] = useState(false);
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
          '.xltx',
          '.xltm',
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
    const baseName = () =>
      fileName.replace(/\.(xlsx|xlsm|xltx|xltm|csv)$/i, '') || t('sheetEditor.untitled');
    const has = { isEnabled: (s: SheetEditorState) => s.hasWorkbook };
    // The workbook keeps its type: .xlsx, .xlsm, .xltx or .xltm (CORE-FMT-001).
    const saveWorkbook = async (s: SheetEditorState, variant?: string | null) => {
      const bytes = await s.host.save('xlsx', s.sheet);
      const name = saveName(baseName(), variant ?? info?.variant, 'xlsx');
      const saved = await platform.files.saveFile(bytes, name, mimeFor(name) || XLSX_MIME);
      if (saved) setStatus(t('sheetEditor.saved', { name: saved }));
    };
    add('file.save', (s) => saveWorkbook(s), { shortcut: 'Ctrl+S', ...has });
    // Save as: the file type first.
    add('file.saveAs', async () => setSaveAsType(true), { shortcut: 'Ctrl+Shift+S', ...has });
    add(
      'file.editTemplate',
      async () => {
        if (templateFile) await openFile(templateFile, undefined, true);
      },
      { isEnabled: () => !!templateFile },
    );
    // Theme files (CORE-FMT-001): apply the theme of a .thmx (or another file), save ours.
    add(
      'file.themeFile',
      async (s) => {
        const f = await platform.files.openFile([
          '.thmx',
          ...openExtensions('sheet'),
          '.docx',
          '.pptx',
        ]);
        if (!f) return;
        const r = await s.host.exec<{ ok: boolean }>({
          cmd: 'applyThemeFile',
          file: toBase64(f.bytes),
        });
        if (r.ok) await refreshInfo();
        setStatus(
          r.ok ? t('fileVariants.themeApplied', { name: f.name }) : t('fileVariants.noTheme'),
        );
      },
      has,
    );
    add(
      'file.saveThemeFile',
      async (s) => {
        const r = await s.host.exec<{ ok: boolean; file?: string }>({ cmd: 'saveThemeFile' });
        if (!r.ok || !r.file) {
          setStatus(t('fileVariants.noTheme'));
          return;
        }
        const name = `${baseName()}.thmx`;
        const saved = await platform.files.saveFile(fromBase64(r.file), name, THEME_MIME);
        if (saved) setStatus(t('fileVariants.themeSaved', { name }));
      },
      has,
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
    add('file.exportPdfOptions', async () => setPdfOptions(true), has);
    const printNow = async (s: SheetEditorState) =>
      platform.print.printPdf(await s.host.exportPdf(), baseName());
    // The print panel (sheets, selection, titles, gridlines, pages per sheet, preview).
    add(
      'file.print',
      async (s) => {
        if (s.host.printPdf && s.host.printPreview) setDialog({ kind: 'print' });
        else await printNow(s);
      },
      { shortcut: 'Ctrl+P', ...has },
    );
    add('file.printNow', printNow, has);

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
        if (s.smartArt !== null) {
          await exec({ cmd: 'deleteSmartArt', sheet: s.sheet, index: s.smartArt });
          setSmartArt(null);
          return;
        }
        // Checkbox cells: values first, then the control (SHEET-CELL-001).
        if (await deleteCheckboxes((cmd) => exec(cmd), s.sheet, rng(s))) return;
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
    add('format.conditionalManage', open({ kind: 'cfManage' }));
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
    add('review.checkDocument', open({ kind: 'checkDocument' }), has);
    add('review.spellingDialog', async () => setSpellDialog(true), { ...has, shortcut: 'F7' });
    add('review.spelling', async () => toggleSpelling(), {
      ...has,
      isChecked: () => asYouType,
    });
    registerReadAloudCommands(reg, {
      enabled: (st) => st.hasWorkbook,
      begin: (st) => readAloudLink.begin(st),
      current: () => readAloudLink.aloud(),
    });
    registerTranslateCommands(reg, {
      enabled: (st) => st.hasWorkbook,
      begin: beginTranslate,
      actions: {
        cellsNewColumn: 'review.translateCells',
        cellsInPlace: 'review.translateCellsInPlace',
      },
    });
    // CORE-SEC-001: final mark, write protection, label, protected ranges.
    add(
      'file.protect',
      async () => {
        await runProtect({ cmd: 'security' });
        setProtectUi('panel');
      },
      has,
    );

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
    add('review.versions', async () => setVersionsOpen((o) => !o), {
      ...has,
      isChecked: () => versionsOpen,
    });
    add('script.editor', async () => setScriptOpen((o) => !o), {
      ...has,
      isChecked: () => scriptOpen,
    });
    // Macro recorder (CORE-SCRIPT-002): Stop opens the recorded script in the pane.
    add(
      'script.record',
      async () => {
        if (!recorder.recording) {
          await recorder.start();
          return;
        }
        await recorder.stop();
        setScriptOpen(true);
      },
      { ...has, isChecked: () => recorder.recording },
    );

    // Insert
    add('insert.chart', popup('chart', 'insert.chart'), has);
    // SmartArt (CORE-SMART-005): the BlueSlide layout library over cells from the active one.
    add(
      'insert.smartart',
      async () =>
        setSmartArtDialog({
          mode: 'insert',
          index: null,
          initial: {
            layout: 'process',
            items: [1, 2, 3].map((n) => ({
              text: t('slideEditor.smartart.sampleItem', { n }),
              level: 0,
            })),
            colors: 'colorful',
            style: 'fill',
          },
        }),
      has,
    );
    add(
      'smartart.edit',
      async (s) => {
        if (s.smartArt !== null) await openSmartArt(s.sheet, s.smartArt);
      },
      { isEnabled: (s) => s.smartArt !== null },
    );
    add(
      'smartart.delete',
      async (s) => {
        if (s.smartArt === null) return;
        await exec({ cmd: 'deleteSmartArt', sheet: s.sheet, index: s.smartArt });
        setSmartArt(null);
      },
      { isEnabled: (s) => s.smartArt !== null },
    );
    add('insert.symbol', async () => setSymbolsOpen(true), has);
    add('insert.pivot', open({ kind: 'pivot' }), has);
    add('insert.table', (s) => exec({ cmd: 'createTable', sheet: s.sheet, range: rng(s) }), has);
    registerSparklineCommands(add, {
      range: rng,
      exec: (cmd) => exec(cmd),
      openInsert: () => setDialog({ kind: 'sparklineInsert' }),
      openOptions: () => setDialog({ kind: 'sparklineOptions' }),
    });
    registerTableCommands(add, {
      exec: (cmd) => exec(cmd),
      totalLabel: () => t('tableDesign.totalLabel'),
      openRename: () => setDialog({ kind: 'tableRename' }),
      openResize: () => setDialog({ kind: 'tableResize' }),
      openTotals: () => setDialog({ kind: 'tableTotals' }),
      openDedupe: (r) => {
        setSelection({
          anchor: { row: r.r0, col: r.c0 },
          focus: { row: r.r1, col: r.c1 },
          active: { row: r.r0, col: r.c0 },
        });
        setDialog({ kind: 'dedupe' });
      },
    });
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
    // Checkboxes and pictures in cells (SHEET-CELL-001).
    registerCellMediaCommands<SheetEditorState>(
      add,
      (s) => ({
        host: s.host,
        hasWorkbook: s.hasWorkbook,
        sheet: s.sheet,
        active: s.selection.active,
        range: rng(s),
      }),
      {
        exec: (cmd) => exec(cmd),
        pickFile: (accept) => platform.files.openFile(accept),
        openAltText: (d) => setAltDialog(d),
        fail,
      },
    );
    // Form controls ([MS-XLSX] control properties, legacy VML client data).
    for (const kind of INSERTABLE) {
      add(
        `insert.formControl.${kind}`,
        async (s) => {
          const label = t(`sheetEditor.formControl.kinds.${kind}`);
          const n = formList.filter((c) => c.kind === kind).length + 1;
          const r = await exec<{ index: number }>({
            cmd: 'insertFormControl',
            sheet: s.sheet,
            kind,
            row: s.selection.active.row,
            col: s.selection.active.col,
            text: label,
            name: `${label} ${n}`,
          });
          if (r) setFormSel(r.index);
        },
        has,
      );
    }
    const selectedControl = () => formList.find((c) => c.index === formSel) ?? null;
    const editable = () => {
      const c = selectedControl();
      return !!c && c.kind !== 'activeX';
    };
    add(
      'formControl.format',
      async () => {
        const c = selectedControl();
        if (c && c.kind !== 'activeX') setFormDialog(c);
      },
      { isEnabled: editable },
    );
    add(
      'formControl.delete',
      async (s) => {
        const c = selectedControl();
        if (!c || c.kind === 'activeX') return;
        setFormSel(null);
        await exec({ cmd: 'deleteFormControl', sheet: s.sheet, index: c.index });
      },
      { isEnabled: editable },
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
    add('data.advancedFilter', open({ kind: 'advancedFilter' }));
    add('data.showAllData', (s) => exec({ cmd: 'showAllData', sheet: s.sheet }));
    add('data.validation', open({ kind: 'dv' }));
    add(
      'data.flashFill',
      async (s) => {
        const { row, col } = s.selection.active;
        const r = await exec<FlashFillResult>({ cmd: 'flashFill', sheet: s.sheet, row, col });
        if (!r) return;
        setStatus(
          r.found && r.filled > 0
            ? t('sheetEditor.flashFill.done', { count: r.filled })
            : t('sheetEditor.flashFill.none'),
        );
      },
      { shortcut: 'Ctrl+E' },
    );
    add('data.goalSeek', open({ kind: 'goalSeek' }));
    add('data.scenarios', open({ kind: 'scenarios' }));
    add('data.dataTable', open({ kind: 'dataTable' }));
    add('data.solver', open({ kind: 'solver' }));
    add('data.analysis', open({ kind: 'analysis' }));
    add('data.textToColumns', open({ kind: 'textToColumns' }));
    add('data.removeDuplicates', open({ kind: 'dedupe' }));
    add('data.consolidate', open({ kind: 'consolidate' }));
    add('data.subtotal', open({ kind: 'subtotal' }));
    add('data.fromCsv', () => queryFrom('csv'));
    add('data.fromJson', () => queryFrom('json'));
    add('data.fromXml', () => queryFrom('xml'));
    add('data.fromWorkbook', () => queryFrom('workbook'));
    add('data.fromFolder', () => queryFrom('folder'));
    add('data.fromWeb', () => queryFrom('web'));
    add('data.queries', async () => toggleQueries(), {
      isChecked: () => queriesOpen,
    });
    add('data.refreshAll', () => queryRefresh(), { shortcut: 'Ctrl+Alt+F5' });
    const outlineGroup = (rows: boolean, ungroup: boolean) => (s: SheetEditorState) => {
      const r = rangeOf(s.selection);
      return exec({
        cmd: 'outlineGroup',
        sheet: s.sheet,
        rows,
        first: rows ? r.r0 : r.c0,
        last: rows ? r.r1 : r.c1,
        ungroup,
      });
    };
    add('data.groupRows', outlineGroup(true, false));
    add('data.ungroupRows', outlineGroup(true, true));
    add('data.groupCols', outlineGroup(false, false));
    add('data.ungroupCols', outlineGroup(false, true));
    add('data.autoOutline', async (s) => {
      const r = rangeOf(s.selection);
      try {
        await s.host.exec({
          cmd: 'autoOutline',
          sheet: s.sheet,
          range: cellsOf(r) === 1 ? null : toRange(r),
        });
        await refreshInfo();
      } catch (e) {
        if (String(e instanceof Error ? e.message : e).includes('autoOutline: none')) {
          setStatus(t('sheetEditor.dataTools.outline.none'));
        } else throw e;
      }
    });
    add('data.clearOutline', (s) => exec({ cmd: 'clearOutline', sheet: s.sheet }));
    const showLevel = (level: number) => (s: SheetEditorState) =>
      exec({
        cmd: 'batch',
        cmds: [
          { cmd: 'outlineShowLevel', sheet: s.sheet, rows: true, level },
          { cmd: 'outlineShowLevel', sheet: s.sheet, rows: false, level },
        ],
      });
    add('data.collapseOutline', showLevel(1));
    add('data.expandOutline', showLevel(8));
    add(
      'data.circleInvalid',
      async (s) => {
        const r = await exec<InvalidCellsResult>({ cmd: 'invalidCells', sheet: s.sheet }, false);
        if (!r) return;
        setCirclesOn(r.cells.length > 0 ? s.sheet : null);
        const n = r.cells.length;
        setStatus(
          n === 0
            ? t('sheetEditor.dataTools.invalid.none')
            : r.truncated || n > MAX_CIRCLES
              ? t('sheetEditor.dataTools.invalid.truncated', { count: MAX_CIRCLES })
              : t('sheetEditor.dataTools.invalid.found', { count: n }),
        );
      },
      { isChecked: (s) => circlesOn === s.sheet },
    );
    add('data.clearCircles', async () => setCirclesOn(null), {
      isEnabled: () => circlesOn !== null,
    });

    // Page layout
    add('page.setup', open({ kind: 'pageSetup' }));
    add('page.setPrintArea', (s) =>
      exec({ cmd: 'setPageSetup', sheet: s.sheet, printArea: rng(s) }),
    );
    add('page.clearPrintArea', (s) =>
      exec({ cmd: 'setPageSetup', sheet: s.sheet, printArea: null }),
    );
    add('page.preview', open({ kind: 'printPreview' }), has);
    // Manual page breaks go before the active cell's row and column (row / column 0: none).
    const pageBreak = (remove: boolean) => (s: SheetEditorState) =>
      exec({
        cmd: 'pageBreak',
        sheet: s.sheet,
        row: s.selection.active.row,
        col: s.selection.active.col,
        remove,
      });
    add('page.insertBreak', pageBreak(false));
    add('page.removeBreak', pageBreak(true));
    add('page.resetBreaks', (s) => exec({ cmd: 'resetPageBreaks', sheet: s.sheet }));

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
    // Workbook views (SHEET-VIEW-001).
    for (const m of ['normal', 'pageBreakPreview', 'pageLayout'] as const) {
      add(`view.${m}`, () => setViewMode(m), { isChecked: () => viewMode === m, ...has });
    }
    add('view.showFormulas', () => toggleFormulas(), {
      shortcut: 'Ctrl+`',
      isChecked: () => showingFormulas,
      ...has,
    });
    add('view.customViews', open({ kind: 'customViews' }), has);
    add('view.tempViews', open({ kind: 'tempViews' }), has);

    // Formulas: auditing and calculation (SHEET-AUDIT-001)
    const traceCmd = (dependents: boolean) => async (s: SheetEditorState) => {
      const { row, col } = s.selection.active;
      if ((await auditTrace(dependents, { sheet: s.sheet, row, col })) === 'none') {
        setStatus(t(dependents ? 'sheetAudit.noDependents' : 'sheetAudit.noPrecedents'));
      }
    };
    add('formulas.tracePrecedents', traceCmd(false));
    add('formulas.traceDependents', traceCmd(true));
    add('formulas.removeArrows', async () => removeArrows(), {
      isEnabled: () => auditTraces.length > 0,
    });
    add('formulas.errorCheck', async () => setAuditDialog({ kind: 'errors' }));
    add('formulas.evaluate', async (s) =>
      setAuditDialog({ kind: 'evaluate', cell: { sheet: s.sheet, ...s.selection.active } }),
    );
    add('formulas.watch', async () => setWatchOpen((o) => !o), { isChecked: () => watchOpen });
    add('formulas.calcOptions', async () => setAuditDialog({ kind: 'options' }));
    add('formulas.calcNow', () => auditCalculate('workbook'), { shortcut: 'F9' });
    add('formulas.calcSheet', () => auditCalculate('sheet'), { shortcut: 'Shift+F9' });
    add('formulas.calcFull', () => auditCalculate('full'), { shortcut: 'Ctrl+Alt+F9' });
    // F4 outside a formula: the last formatting / row-column command again (CORE-UI-004).
    reg.trackRepeat((id) => SHEET_REPEATABLE.test(id));
    add(
      'edit.repeat',
      async (s) => {
        const last = reg.lastRepeatable;
        if (last) await reg.execute(last, s);
      },
      { shortcut: 'F4', ...has },
    );
    add('options.shortcuts', async () => setShortcutsShown(true));
    return reg;
  }, [
    openSmartArt,
    readAloudLink,
    beginTranslate,
    auditTrace,
    auditCalculate,
    removeArrows,
    setAuditDialog,
    setWatchOpen,
    auditTraces,
    watchOpen,
    t,
    openFile,
    fail,
    load,
    platform,
    fileName,
    templateFile,
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
    versionsOpen,
    scriptOpen,
    activePivot,
    formList,
    formSel,
    setFormSel,
    runProtect,
    setProtectUi,
    circlesOn,
    setSpellDialog,
    toggleSpelling,
    asYouType,
    viewMode,
    setViewMode,
    toggleFormulas,
    showingFormulas,
    recorder,
    queryFrom,
    queryRefresh,
    toggleQueries,
    queriesOpen,
  ]);

  // Saved scripts on a shortcut / the Quick Access Toolbar (CORE-SCRIPT-002).
  const scriptBindings = useScriptBindings({
    app: 'sheet',
    registry,
    host: () =>
      host && info
        ? sheetScriptHost({
            host,
            activeSheet: () => sheetNow.current,
            decimal,
            onEnd: refreshInfo,
          })
        : null,
    recorder,
    label: (id) => t(`commands.${id}`),
    onResult: (sc, r) => {
      if (!r.ok)
        setStatus(t('script.boundFailed', { message: `${sc.name}: ${r.error?.message ?? ''}` }));
    },
  });

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
            smartArt,
            pivot: activePivot?.index ?? null,
            ranges: allRanges,
            sheetProtected: !!info?.sheets[sheet]?.protected,
            structureLocked: !!info?.structureLocked,
            sparkline: sparkGroup,
            table: activeTable,
            tableAuto,
          }
        : null,
    [
      host,
      sheet,
      selection,
      info,
      fmt,
      chart,
      smartArt,
      activePivot?.index,
      allRanges,
      sparkGroup,
      activeTable,
      tableAuto,
    ],
  );
  const run = useCallback(
    (id: string) => {
      if (state) void registry.execute(id, state);
    },
    [registry, state],
  );
  // The File screen (CORE-UI-002).
  const [backstage, setBackstage] = useState(false);
  const closeBackstage = useCallback(() => setBackstage(false), []);
  const backstageCommands = useMemo(
    () => ({
      has: (id: string) => registry.has(id),
      enabled: (id: string) => !!state && registry.isEnabled(id, state),
      run,
    }),
    [registry, state, run],
  );
  const docProperties = useMemo<BackstagePropertiesAccess | undefined>(
    () =>
      host && {
        load: () => host.exec<DocProperties>({ cmd: 'docProperties' }),
        save: (props) => host.exec<DocProperties>({ cmd: 'setDocProperties', props }),
      },
    [host],
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
      } else if (e.key === 'F4' && !e.ctrlKey && !e.altKey && !e.metaKey) {
        e.preventDefault();
        cycleRefIn(e.currentTarget);
      }
      return;
    }
    if (isContextMenuKey(e)) {
      e.preventDefault();
      const c = canvasRef.current?.getBoundingClientRect();
      if (c && selRect)
        sheetMenu.open({
          x: c.left + selRect.x,
          y: c.top + selRect.y + selRect.h,
          context: activeMedia?.image ? 'picture' : 'cell',
        });
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
    if (
      e.key === ' ' &&
      !e.ctrlKey &&
      !e.metaKey &&
      !e.altKey &&
      !e.shiftKey &&
      activeMedia?.checkbox
    ) {
      e.preventDefault();
      run('cell.toggleCheckbox');
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

  // Symbol picker: at the caret of the edit in progress, else appended to the active
  // cell's content in a new in-place edit (committed with Enter as usual).
  const insertSymbol = (char: string) => {
    const el = lastInput.current === 'bar' ? barRef.current : inputRef.current;
    const put = (base: string, start: number, end: number, pos: CellPos) => {
      const text = base.slice(0, start) + char + base.slice(end);
      const caret = start + char.length;
      setEdit({ pos, text, inPlace: true });
      setBarText(text);
      if (inputRef.current) inputRef.current.value = text;
      requestAnimationFrame(() => el?.setSelectionRange(caret, caret));
    };
    if (edit && el) {
      const start = el.selectionStart ?? el.value.length;
      put(
        edit.text,
        Math.min(start, edit.text.length),
        Math.min(el.selectionEnd ?? start, edit.text.length),
        edit.pos,
      );
      return;
    }
    if (!host) return;
    const pos = selection.active;
    void host
      .cell(sheet, pos.row, pos.col)
      .then((c) => {
        lastInput.current = 'cell';
        put(c.input, c.input.length, c.input.length, pos);
      })
      .catch(fail);
  };

  // F4 in a formula: the reference at the caret cycles A1 → $A$1 → A$1 → $A1 (CORE-UI-004).
  const cycleRefIn = (el: HTMLInputElement) => {
    if (!edit) return;
    const c = cycleReference(edit.text, el.selectionEnd ?? el.value.length);
    if (!c) return;
    setEdit({ ...edit, text: c.text });
    setBarText(c.text);
    if (inputRef.current) inputRef.current.value = c.text;
    requestAnimationFrame(() => el.setSelectionRange(c.start, c.end));
  };

  // Right click on the grid: select what is under the pointer unless it is in the selection,
  // then its menu (CORE-UI-003).
  const onGridContextMenu = async (e: ReactMouseEvent) => {
    e.preventDefault();
    const at = { x: e.clientX, y: e.clientY };
    if (edit) await commit();
    const hit = await hitAt(e);
    if (!hit) return;
    const rg = rangeOf(selection);
    const last = info?.sheets[sheet];
    const inRows = (row: number) => row >= rg.r0 && row <= rg.r1;
    const inCols = (col: number) => col >= rg.c0 && col <= rg.c1;
    if (hit.kind === 'cell') {
      const p = { row: hit.row, col: hit.col };
      const same = p.row === selection.active.row && p.col === selection.active.col;
      if (!(inRows(p.row) && inCols(p.col))) setSelection(single(p));
      sheetMenu.open({ ...at, context: same && activeMedia?.image ? 'picture' : 'cell' });
    } else if (hit.kind === 'rowHeader' || hit.kind === 'rowEdge') {
      if (!(inRows(hit.row) && rg.c0 === 0))
        setSelection(
          span({ row: hit.row, col: 0 }, { row: hit.row, col: Math.max(last?.lastCol ?? 0, 25) }),
        );
      sheetMenu.open({ ...at, context: 'rowHeader' });
    } else if (hit.kind === 'columnHeader' || hit.kind === 'columnEdge') {
      if (!(inCols(hit.col) && rg.r0 === 0))
        setSelection(
          span({ row: 0, col: hit.col }, { row: Math.max(last?.lastRow ?? 0, 999), col: hit.col }),
        );
      sheetMenu.open({ ...at, context: 'columnHeader' });
    } else if (hit.kind === 'chart') {
      setChart(hit.index);
      sheetMenu.open({ ...at, context: 'chart' });
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
    setSmartArt(null);
    setFormSel(null);
    const multi = (e.ctrlKey || e.metaKey) && !e.shiftKey;
    // Ctrl+click / Ctrl+drag starts another range of a multi-range selection.
    if (multi && (hit.kind === 'cell' || hit.kind === 'columnHeader' || hit.kind === 'rowHeader'))
      setExtra((x) => [...x, rangeOf(selection)]);
    else if (!e.shiftKey) setExtra([]);
    if (hit.kind === 'cell') {
      const p = { row: hit.row, col: hit.col };
      setSelection(e.shiftKey ? { ...selection, focus: p } : single(p));
      drag.current = { kind: 'select' };
      // A click on a checkbox toggles it (one undo step).
      const at = point(e);
      if (!e.shiftKey && !multi && at) {
        const box = await host.rangeRect(view, hit.row, hit.col, hit.row, hit.col);
        if (await clickCheckbox(host, (c) => exec(c), sheet, hit.row, hit.col, box, at.x, at.y))
          drag.current = null;
      }
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
    } else if (hit.kind === 'smartArt') {
      setSmartArt(hit.index);
      const a = await host
        .exec<SheetSmartArtInfo>({ cmd: 'smartArtInfo', sheet, index: hit.index })
        .catch(() => null);
      if (a) {
        drag.current = {
          kind: 'chart',
          smart: true,
          index: hit.index,
          resize: false,
          start: { row: hit.row, col: hit.col },
          from: a.from,
          to: a.to,
        };
      }
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
      if (!hit || (hit.kind !== 'cell' && hit.kind !== 'chart' && hit.kind !== 'smartArt') || !host)
        return;
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
            : hit?.kind === 'filterButton' || hit?.kind === 'chart' || hit?.kind === 'smartArt'
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
        void exec({
          cmd: d.smart ? 'moveSmartArt' : 'moveChart',
          sheet,
          index: d.index,
          from: target.from,
          to: target.to,
        });
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
    } else if (hit.kind === 'smartArt') {
      void openSmartArt(sheet, hit.index);
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

  // The user's keyboard shortcuts (CORE-UI-004).
  const ribbonTabsRef = useRef<RibbonTab[]>([]);
  const shortcuts = useCustomShortcuts({
    app: 'sheet',
    registry,
    store: platform.shell,
    files: platform.files,
    commands: () =>
      shortcutCommands(ribbonTabsRef.current, registry, (id) => commandLabel(i18n, id)),
    open: shortcutsShown,
    onOpenChange: setShortcutsShown,
  });
  const shortcutsVersion = shortcuts.version;
  const ribbonTabs: RibbonTab[] = useMemo(() => {
    // Rebuilt when the user's shortcuts change (tooltips show them).
    void shortcutsVersion;
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
    const tabs: RibbonTab[] = [
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
              item('file.saveAs', t('commands.file.saveAs'), <Save size={s} />, 'small'),
              item(
                'file.editTemplate',
                t('fileVariants.editTemplate'),
                <FolderOpen size={s} />,
                'small',
              ),
              item(
                'file.themeFile',
                t('fileVariants.applyThemeFile'),
                <FolderOpen size={s} />,
                'small',
              ),
              item(
                'file.saveThemeFile',
                t('fileVariants.saveThemeFile'),
                <Save size={s} />,
                'small',
              ),
              item('file.saveCsv', t('sheetEditor.saveCsv'), <Table2 size={s} />, 'small'),
              item('file.exportPdf', t('sheetEditor.exportPdf'), <FileDown size={s} />, 'small'),
              item(
                'file.exportPdfOptions',
                t('commands.file.exportPdfOptions'),
                <FileDown size={s} />,
                'small',
              ),
              item('file.print', t('sheetEditor.print'), <Printer size={s} />, 'small'),
            ],
          },
          {
            id: 'clipboard',
            label: t('sheetEditor.groupEdit'),
            launcher: {
              id: 'edit.clipboardPanel',
              label: t('clipHistory.open'),
              onSelect: () => setClipPanel((o) => !o),
            },
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
                'format.conditionalManage',
                t('sheetEditor.cfm.manage'),
                <ListOrdered size={s} />,
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
              item('insert.smartart', t('slideEditor.smartart.button'), <Network size={18} />),
              item('insert.sparklines', t('sparkline.insert'), <ChartSpline size={18} />),
              item(
                'insert.pictureInCell',
                t('sheetEditor.cellMedia.pictureInCell'),
                <ImagePlus size={18} />,
              ),
              item(
                'insert.checkbox',
                t('sheetEditor.cellMedia.checkbox'),
                <SquareCheck size={s} />,
                'small',
              ),
              item(
                'cell.pictureAltText',
                t('sheetEditor.cellMedia.altText'),
                <ScanText size={s} />,
                'small',
              ),
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
          {
            id: 'formControls',
            label: t('sheetEditor.formControl.group'),
            items: [
              ...INSERTABLE.map((kind) =>
                item(
                  `insert.formControl.${kind}`,
                  t(`sheetEditor.formControl.kinds.${kind}`),
                  {
                    button: <RectangleHorizontal size={s} />,
                    checkBox: <SquareCheck size={s} />,
                    optionButton: <CircleDot size={s} />,
                    comboBox: <SquareChevronDown size={s} />,
                    listBox: <List size={s} />,
                    scrollBar: <GalleryVertical size={s} />,
                    spinner: <ChevronsUpDown size={s} />,
                    groupBox: <SquareDashed size={s} />,
                    label: <Type size={s} />,
                    activeX: null,
                  }[kind],
                  'small',
                ),
              ),
              item(
                'formControl.format',
                t('sheetEditor.formControl.format'),
                <SlidersHorizontal size={s} />,
                'small',
              ),
              item(
                'formControl.delete',
                t('sheetEditor.formControl.delete'),
                <Trash size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'symbols',
            label: t('ribbon.groups.symbols'),
            items: [item('insert.symbol', t('commands.insert.symbol'), <Omega size={18} />)],
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
        id: 'formulas',
        label: t('sheetAudit.tab'),
        groups: [
          {
            id: 'formulaAudit',
            label: t('sheetAudit.groupAudit'),
            items: [
              item(
                'formulas.tracePrecedents',
                t('sheetAudit.tracePrecedents'),
                <AuditIcons.precedents size={s} />,
                'small',
              ),
              item(
                'formulas.traceDependents',
                t('sheetAudit.traceDependents'),
                <AuditIcons.dependents size={s} />,
                'small',
              ),
              item(
                'formulas.removeArrows',
                t('sheetAudit.removeArrows'),
                <AuditIcons.removeArrows size={s} />,
                'small',
              ),
              item(
                'formulas.errorCheck',
                t('sheetAudit.errorCheck'),
                <AuditIcons.errors size={18} />,
              ),
              item(
                'formulas.evaluate',
                t('sheetAudit.evaluate'),
                <AuditIcons.evaluate size={18} />,
              ),
              item('formulas.watch', t('sheetAudit.watch'), <AuditIcons.watch size={18} />),
            ],
          },
          {
            id: 'calculation',
            label: t('sheetAudit.groupCalc'),
            items: [
              item(
                'formulas.calcOptions',
                t('sheetAudit.options'),
                <AuditIcons.options size={18} />,
              ),
              item(
                'formulas.calcNow',
                t('sheetAudit.calcNow'),
                <AuditIcons.calcNow size={s} />,
                'small',
              ),
              item(
                'formulas.calcSheet',
                t('sheetAudit.calcSheet'),
                <AuditIcons.calcSheet size={s} />,
                'small',
              ),
              item(
                'formulas.calcFull',
                t('sheetAudit.calcFull'),
                <AuditIcons.calcFull size={s} />,
                'small',
              ),
            ],
          },
        ],
      },
      {
        id: 'data',
        label: t('sheetEditor.tabData'),
        groups: [
          {
            id: 'getData',
            label: t('sheetQuery.group'),
            items: [
              item('data.fromCsv', t('sheetQuery.ribbon.fromCsv'), <FileText size={18} />),
              item('data.fromJson', t('sheetQuery.ribbon.fromJson'), <Braces size={s} />, 'small'),
              item('data.fromXml', t('sheetQuery.ribbon.fromXml'), <FileCode size={s} />, 'small'),
              item(
                'data.fromWorkbook',
                t('sheetQuery.ribbon.fromWorkbook'),
                <FileSpreadsheet size={s} />,
                'small',
              ),
              item(
                'data.fromFolder',
                t('sheetQuery.ribbon.fromFolder'),
                <FolderOpen size={s} />,
                'small',
              ),
              item('data.fromWeb', t('sheetQuery.ribbon.fromWeb'), <Globe size={s} />, 'small'),
              item('data.queries', t('sheetQuery.ribbon.queries'), <Database size={18} />),
              item('data.refreshAll', t('sheetQuery.ribbon.refreshAll'), <RefreshCw size={18} />),
            ],
          },
          {
            id: 'sortFilter',
            label: t('sheetEditor.groupSortFilter'),
            items: [
              item('data.sortAsc', t('sheetEditor.sortAsc'), <ArrowDownAZ size={s} />, 'small'),
              item('data.sortDesc', t('sheetEditor.sortDesc'), <ArrowDownZA size={s} />, 'small'),
              item('data.sort', t('sheetEditor.sort.title'), <ArrowUpDown size={18} />),
              item('data.filter', t('sheetEditor.filter.toggle'), <Funnel size={18} />),
              item('data.advancedFilter', t('sheetEditor.adv.title'), <FunnelPlus size={18} />),
              item('data.showAllData', t('sheetEditor.adv.showAll'), <FunnelX size={18} />),
            ],
          },
          {
            id: 'tools',
            label: t('sheetEditor.groupDataTools'),
            items: [
              item('data.validation', t('sheetEditor.dv.title'), <ShieldCheck size={18} />),
              item('data.flashFill', t('sheetEditor.flashFill.title'), <Sparkles size={18} />),
              item('data.goalSeek', t('sheetEditor.goalSeek.title'), <Target size={18} />),
              item('data.scenarios', t('sheetEditor.scenario.title'), <Layers size={18} />),
              item('data.dataTable', t('sheetEditor.dataTable.title'), <Grid3x3 size={18} />),
              item('data.solver', t('sheetEditor.solver.title'), <Calculator size={18} />),
              item(
                'data.textToColumns',
                t('sheetEditor.dataTools.split.title'),
                <Columns3 size={s} />,
                'small',
              ),
              item(
                'data.removeDuplicates',
                t('sheetEditor.dataTools.dedupe.title'),
                <CopyX size={s} />,
                'small',
              ),
              item(
                'data.consolidate',
                t('sheetEditor.dataTools.consolidate.title'),
                <Combine size={s} />,
                'small',
              ),
              item(
                'data.circleInvalid',
                t('sheetEditor.dataTools.invalid.circle'),
                <CircleSlash size={s} />,
                'small',
              ),
              item(
                'data.clearCircles',
                t('sheetEditor.dataTools.invalid.clear'),
                <Eraser size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'outline',
            label: t('sheetEditor.groupOutline'),
            items: [
              item('data.subtotal', t('sheetEditor.dataTools.subtotal.title'), <Sigma size={18} />),
              item(
                'data.groupRows',
                t('sheetEditor.dataTools.outline.groupRows'),
                <Group size={s} />,
                'small',
              ),
              item(
                'data.ungroupRows',
                t('sheetEditor.dataTools.outline.ungroupRows'),
                <Ungroup size={s} />,
                'small',
              ),
              item(
                'data.groupCols',
                t('sheetEditor.dataTools.outline.groupCols'),
                <Group size={s} />,
                'small',
              ),
              item(
                'data.ungroupCols',
                t('sheetEditor.dataTools.outline.ungroupCols'),
                <Ungroup size={s} />,
                'small',
              ),
              item(
                'data.autoOutline',
                t('sheetEditor.dataTools.outline.auto'),
                <WandSparkles size={s} />,
                'small',
              ),
              item(
                'data.clearOutline',
                t('sheetEditor.dataTools.outline.clear'),
                <ListX size={s} />,
                'small',
              ),
              item(
                'data.collapseOutline',
                t('sheetEditor.dataTools.outline.collapse'),
                <ChevronsDownUp size={s} />,
                'small',
              ),
              item(
                'data.expandOutline',
                t('sheetEditor.dataTools.outline.expand'),
                <ChevronsUpDown size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'analysis',
            label: t('sheetEditor.groupAnalysis'),
            items: [item('data.analysis', t('sheetEditor.analysis.title'), <Sigma size={18} />)],
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
              item('page.preview', t('sheetEditor.printSetup.preview'), <Eye size={s} />, 'small'),
              item(
                'page.insertBreak',
                t('sheetEditor.printSetup.insertBreak'),
                <SeparatorHorizontal size={s} />,
                'small',
              ),
              item(
                'page.removeBreak',
                t('sheetEditor.printSetup.removeBreak'),
                <SeparatorHorizontal size={s} />,
                'small',
              ),
              item(
                'page.resetBreaks',
                t('sheetEditor.printSetup.resetBreaks'),
                <Eraser size={s} />,
                'small',
              ),
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
              item('file.protect', t('protect.cmd.protect'), <ShieldCheck size={s} />, 'small'),
            ],
          },
          {
            id: 'check',
            label: t('inspect.title'),
            items: [
              item('review.checkDocument', t('inspect.command'), <ClipboardCheck size={18} />),
            ],
          },
          {
            id: 'versions',
            label: t('versions.group'),
            items: [item('review.versions', t('versions.showChanges'), <History size={18} />)],
          },
          {
            id: 'proofing',
            label: t('ribbon.groups.proofing'),
            items: [
              item(
                'review.spellingDialog',
                t('commands.review.spellingDialog'),
                <SpellCheck size={18} />,
              ),
              item(
                'review.spelling',
                t('commands.review.spelling'),
                <SpellCheck size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'speech',
            label: t('ribbon.groups.speech'),
            items: [
              item('review.readAloud', t('commands.review.readAloud'), <Volume2 size={18} />),
            ],
          },
          {
            id: 'language',
            label: t('ribbon.groups.language'),
            items: [
              item('review.translate', t('commands.review.translate'), <Languages size={18} />),
              item(
                'review.translateCells',
                t('commands.review.translateCells'),
                <Languages size={s} />,
                'small',
              ),
              item(
                'review.translateCellsInPlace',
                t('commands.review.translateCellsInPlace'),
                <Languages size={s} />,
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
          ...sheetViewTabGroups(t, item),
          {
            id: 'automation',
            label: t('ribbon.groups.automation'),
            items: [
              item('script.editor', t('commands.view.scriptEditor'), <SquareTerminal size={18} />),
              item('script.record', t('commands.view.recordScript'), <CircleDot size={18} />),
            ],
          },
        ],
      },
    ];
    // Sparkline contextual tab (SHEET-SPARK-001) while the active cell shows a sparkline.
    if (state?.sparkline) {
      tabs.push({
        id: 'sparkline',
        context: 'sparkline',
        label: t('sparkline.tab'),
        groups: sparklineTabGroups(t, item),
      });
    }
    // Table Design contextual tab (SHEET-TABLE-001) while the active cell is in a table.
    if (state?.table) {
      tabs.push({
        id: 'tableDesign',
        context: 'table',
        label: t('tableDesign.tab'),
        groups: tableDesignTabGroups(t, item, {
          table: state.table,
          loadStyles: loadTableStyles,
          run,
        }),
      });
    }
    // SmartArt contextual tab (CORE-SMART-005) while a diagram is selected.
    if (smartArt !== null) {
      tabs.push({
        id: 'smartart',
        context: 'smartArt',
        label: t('slideEditor.smartart.tab'),
        groups: [
          {
            id: 'smartartEdit',
            label: t('slideEditor.smartart.group'),
            items: [
              item('smartart.edit', t('slideEditor.smartart.textPane'), <Pencil size={18} />),
              item('smartart.delete', t('sheetEditor.smartArt.delete'), <Trash2 size={18} />),
            ],
          },
        ],
      });
    }
    // Chart Design contextual tab (CORE-CHART-007) while a chart is selected.
    if (chart === null) return tabs;
    return [
      ...tabs,
      {
        id: 'chartDesign',
        context: 'chart',
        label: t('chartDesign.tab'),
        groups: chartDesignGroups({
          t,
          sheet,
          index: chart,
          load: loadGallery,
          exec: (cmd) => exec(cmd),
          onSaveTemplate: () => setSaveTemplate(true),
          onExportTemplate: () => {
            if (!host) return;
            void exportChartTemplate(
              loadGallery,
              host,
              platform.files,
              fileName.replace(/\.(xlsx|xlsm|csv)$/i, '') || t('sheetEditor.untitled'),
            )
              .then((saved) => {
                if (saved) setStatus(t('chartDesign.templateSaved', { name: saved }));
              })
              .catch(fail);
          },
          onImportTemplate: () => {
            if (!host) return;
            void importChartTemplate(host, platform.files)
              .then((name) => {
                if (name) setStatus(t('chartDesign.templateSaved', { name }));
              })
              .catch(fail);
          },
        }),
      },
    ];
  }, [
    t,
    registry,
    state,
    run,
    chart,
    smartArt,
    sheet,
    loadGallery,
    exec,
    host,
    platform,
    fileName,
    fail,
    loadTableStyles,
    shortcutsVersion,
  ]);

  // Context menus (CORE-UI-003): cell, row / column header, sheet tab, chart, picture.
  const menuLookup = ribbonMenuLookup(
    ribbonTabs,
    registryMenuLookup(
      registry,
      () => state,
      (id) => commandLabel(i18n, id),
      (id) => run(id),
    ),
  );
  const sheetMenu = useContextMenu<SheetMenu>(
    t('contextMenu.label'),
    (m) =>
      resolveMenu(SHEET_MENUS[m.context], (id) =>
        id === 'sheet.tabColor'
          ? {
              id,
              label: t('contextMenu.tabColor'),
              onSelect: () => setTabMenu({ sheet: m.sheet ?? sheet, x: m.x, y: m.y }),
            }
          : menuLookup(id),
      ),
    focusInput,
  );
  useEffect(() => {
    ribbonTabsRef.current = ribbonTabs;
  });

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
        return [
          ...CHART_TYPES.map((k) => ({ id: k, label: t(`sheetEditor.chart.types.${k}`) })),
          ...templateMenuItems(t),
        ];
      default:
        return [];
    }
  };
  const pickMenu = (kind: string, id: string) => {
    const range = toRange(r);
    if (kind === 'borders') void borderSel({ preset: id as BorderPreset, style: 'thin' });
    else if (kind === 'styles') void exec({ cmd: 'cellStyle', sheet, range, name: id });
    else if (kind === 'numFmt') void formatSel({ numFmt: id });
    else if (kind === 'chart') {
      // A saved chart template inserts its type with its formatting.
      const template = templateOfItem(id);
      void exec<{ index: number }>({
        cmd: 'insertChart',
        sheet,
        range,
        kind: template?.kind ?? id,
        ...(template ? { template, texts: layoutTexts(t) } : {}),
      }).then((res) => {
        if (res) setChart(res.index);
      });
    }
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
      <TitleBar
        app="sheet"
        appName={t('app.sheet')}
        docName={fileName}
        quickAccessLabel={t('titleBar.quickAccess')}
        search={{
          label: t('commandSearch.placeholder'),
          shortcut: 'Alt+Q',
          onOpen: () => setSearchOpen(true),
        }}
        quickAccess={(
          [
            ['file.save', 'commands.file.save'],
            ['edit.undo', 'commands.edit.undo'],
            ['edit.redo', 'commands.edit.redo'],
          ] as const
        )
          .map(([id, key]) => ({
            id: id as string,
            label: t(key),
            shortcut: registry.get(id)?.shortcut,
            enabled: state ? registry.isEnabled(id, state) : false,
            onSelect: () => run(id),
          }))
          .concat(
            scriptBindings.quickAccess.map((sc) => ({
              id: `script.run.${sc.id}`,
              icon: 'script.run',
              label: t('script.runBound', { name: sc.name }),
              shortcut: sc.shortcut,
              enabled: !!info,
              onSelect: () => run(`script.run.${sc.id}`),
            })),
          )}
      />
      <CommandSearch
        tabs={ribbonTabs}
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        labels={{
          title: t('commandSearch.title'),
          placeholder: t('commandSearch.placeholder'),
          noResults: t('commandSearch.noResults'),
        }}
      />
      <Ribbon
        tabs={ribbonTabs}
        labels={ribbonLabels(t)}
        autoActivate={AUTO_CONTEXTS}
        fileMenu={{
          label: t('ribbon.tabs.file'),
          open: backstage,
          onOpen: () => setBackstage(true),
        }}
      />
      <FileBackstage
        app="sheet"
        open={backstage}
        onClose={closeBackstage}
        commands={backstageCommands}
        hasDocument={!!info}
        fileName={fileName}
        variant={info?.variant}
        macros={info?.macros}
        properties={docProperties}
        shell={platform.shell}
        openPath={
          platform.files.openPath
            ? (path) =>
                void platform.files.openPath?.(path).then(
                  (f) => openFile(f),
                  (e: unknown) => fail(e),
                )
            : undefined
        }
        onSaveAs={(ext) => void saveAsTypeChosen(ext)}
      />
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
          ref={barRef}
          aria-label={t('sheetEditor.formulaBar')}
          value={barText}
          onFocus={() => {
            lastInput.current = 'bar';
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
            } else if (e.key === 'F4' && !e.ctrlKey && !e.altKey && !e.metaKey) {
              e.preventDefault();
              cycleRefIn(e.currentTarget);
            }
          }}
        />
      </div>
      {macroBanner && <MacroBanner inline onDismiss={() => setMacroBanner(false)} />}
      {pdfOptions && (
        <PdfExportDialog
          app="sheet"
          onClose={() => setPdfOptions(false)}
          onExport={(c) => {
            void (async () => {
              try {
                // The choices hold for one export; plain PDF afterwards.
                const { pdfa, tagged, bookmarks } = c;
                await host.exec({ cmd: 'setPdfExport', pdfa, tagged, bookmarks });
                let pdf: Uint8Array;
                try {
                  pdf = await host.exportPdf();
                } finally {
                  await host.exec({ cmd: 'setPdfExport' });
                }
                const base =
                  fileName.replace(/\.(xlsx|xlsm|xltx|xltm|csv)$/i, '') ||
                  t('sheetEditor.untitled');
                await platform.files.saveFile(pdf, `${base}.pdf`, 'application/pdf');
              } catch (e) {
                fail(e);
              }
            })();
          }}
        />
      )}
      <SaveAsTypeDialog
        open={saveAsType}
        app="sheet"
        current={info?.variant}
        macros={info?.macros}
        onCancel={() => setSaveAsType(false)}
        onConfirm={(ext) => {
          setSaveAsType(false);
          void saveAsTypeChosen(ext);
        }}
      />
      <FileProtectionUi
        app="sheet"
        p={protection}
        name={fileName}
        sheet={sheet}
        selectionRef={rangeA1(rangeOf(selection))}
      />
      <div className={styles.workArea}>
        {viewMode === 'pageLayout' && host && (
          <PageLayoutView
            host={host}
            sheet={sheet}
            revision={revision}
            zoom={zoom}
            onCell={(row, col) => setSelection(single({ row, col }))}
            onHeaderFooter={() => setDialog({ kind: 'pageSetup' })}
            onKeyDown={(e) => {
              const c = registry.match(e);
              if (c) {
                e.preventDefault();
                run(c.id);
              }
            }}
            fail={fail}
          />
        )}
        <div
          className={styles.scroller}
          ref={scrollerRef}
          hidden={viewMode === 'pageLayout'}
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
              onContextMenu={(e) => void onGridContextMenu(e)}
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
            <InvalidCircles rects={circlesOn === sheet ? circleRects : []} />
            {viewMode === 'pageBreakPreview' && host && (
              <PageBreakOverlay
                host={host}
                view={view}
                revision={revision}
                scale={scale}
                width={size.w}
                height={size.h}
                frozen={{ rows: summary?.frozenRows ?? 0, cols: summary?.frozenCols ?? 0 }}
                onMove={(m) =>
                  void exec<{ scale: number | null }>({ cmd: 'movePageBreak', sheet, ...m }).then(
                    (r) => r?.scale && setStatus(t('sheetViews.scaled', { pct: r.scale })),
                  )
                }
              />
            )}
            {audit.arrows.length > 0 && (
              <TraceArrowsOverlay
                host={host}
                view={view}
                arrows={audit.arrows}
                toScreen={px}
                width={size.w}
                height={size.h}
              />
            )}
            <PeerSelections boxes={peerBoxes} />
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
            {smartArt !== null && smartBox && smartInfo && (
              <div
                className={styles.chartSelection}
                data-testid="smartart-selection"
                style={{ left: smartBox.x, top: smartBox.y, width: smartBox.w, height: smartBox.h }}
              >
                <div
                  className={styles.chartHandle}
                  data-testid="smartart-resize"
                  title={t('sheetEditor.chart.resize')}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    drag.current = {
                      kind: 'chart',
                      smart: true,
                      index: smartArt,
                      resize: true,
                      start: { row: smartInfo.to[0], col: smartInfo.to[1] },
                      from: smartInfo.from,
                      to: smartInfo.to,
                    };
                  }}
                />
              </div>
            )}
            {timelines.map(({ info, rect }) => (
              <div
                key={`t${info.index}`}
                className={styles.slicer}
                data-testid="timeline"
                role="group"
                aria-label={info.caption}
                style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
                onMouseDown={(e) => e.stopPropagation()}
              >
                <div className={styles.slicerHeader}>
                  <span>
                    {info.caption}
                    <span className={styles.timelineLevel}>
                      {' · '}
                      {t(`sheetEditor.timeline.levels.${info.level}`)}
                    </span>
                  </span>
                  <button
                    type="button"
                    data-testid="timeline-clear"
                    title={t('sheetEditor.slicer.clear')}
                    aria-label={t('sheetEditor.slicer.clear')}
                    disabled={!info.filtered}
                    onClick={() => void exec({ cmd: 'timelineSelect', sheet, index: info.index })}
                  >
                    <FunnelX size={14} />
                  </button>
                </div>
                <div className={styles.timelineStrip}>
                  {info.periods.map((p, k) => (
                    <button
                      key={p.label}
                      type="button"
                      className={info.filtered && p.selected ? styles.slicerOn : styles.slicerOff}
                      aria-pressed={info.filtered && p.selected}
                      onClick={(e) => {
                        const from =
                          e.shiftKey && timelineAnchor.current !== null
                            ? timelineAnchor.current
                            : k;
                        if (!e.shiftKey) timelineAnchor.current = k;
                        const a = info.periods[Math.min(from, k)] ?? p;
                        const b = info.periods[Math.max(from, k)] ?? p;
                        void exec({
                          cmd: 'timelineSelect',
                          sheet,
                          index: info.index,
                          start: a.start,
                          end: b.end,
                        });
                      }}
                    >
                      {p.label}
                    </button>
                  ))}
                </div>
              </div>
            ))}
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
            <TableAutoBar last={tables?.lastAuto ?? null} run={run} />
            {host && (
              <FormControlLayer
                host={host}
                sheet={sheet}
                view={view}
                scale={scale}
                revision={revision}
                exec={exec}
                selected={formSel}
                onSelect={setFormSel}
                onFormat={setFormDialog}
                onNotice={setStatus}
                onList={setFormList}
              />
            )}
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
              onFocus={() => {
                lastInput.current = 'cell';
              }}
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
        <SheetClipboardUi
          paste={pasteUi && pasteUi.sel === selection ? pasteUi : null}
          anchor={() => {
            const c = canvasRef.current?.getBoundingClientRect();
            return c && selRect
              ? { x: c.left + selRect.x + selRect.w, y: c.top + selRect.y + selRect.h }
              : null;
          }}
          onChoose={(choice) => void switchPaste(choice).then(focusInput)}
          panel={clipPanel}
          onPasteItem={(item) => void pasteItem(item).then(focusInput)}
          onClosePanel={() => {
            setClipPanel(false);
            focusInput();
          }}
        />
        {symbolsOpen && (
          <SymbolPanel
            fonts={[...new Set([fmt?.fontName, ...SYMBOL_FONTS].filter((f): f is string => !!f))]}
            font={fmt?.fontName}
            onInsert={insertSymbol}
            onClose={() => {
              setSymbolsOpen(false);
              focusInput();
            }}
          />
        )}
        <TaskPaneHost
          placement="flex"
          layout={paneLayout}
          onLayout={setPaneLayout}
          panes={[
            ...(commentsOpen
              ? [
                  {
                    id: 'comments',
                    title: commentLabels.title,
                    icon: <MessageSquareText size={14} aria-hidden />,
                    content: (
                      <CommentThreads
                        embedded
                        threads={threads.map((th) => ({ ...th, anchor: th.cell }))}
                        labels={commentLabels}
                        newAnchor={
                          threads.some(
                            (th) =>
                              th.row === selection.active.row && th.col === selection.active.col,
                          )
                            ? null
                            : `${colName(selection.active.col)}${selection.active.row + 1}`
                        }
                        searchPeople={
                          platform.people ? (q) => platform.people!.search(q) : undefined
                        }
                        onAdd={(_text, extra) =>
                          void postComment(
                            { row: selection.active.row, col: selection.active.col },
                            extra,
                          )
                        }
                        onReply={(id, _text, extra) => void postComment({ parent: id }, extra)}
                        onResolve={(id, done) =>
                          void commentCmd({ cmd: 'resolveComment', sheet, id, done })
                        }
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
                    ),
                  } satisfies TaskPaneDef,
                ]
              : []),
            ...(versionsOpen
              ? [
                  {
                    id: 'versions',
                    title: t('versions.sheetTitle'),
                    icon: <History size={14} aria-hidden />,
                    content: (
                      <SheetShowChangesPane
                        embedded
                        exec={(cmd) => exec(cmd, false)}
                        versions={platform.versions}
                        openFile={() =>
                          platform.files.openFile(['.xlsx', '.xlsm', '.xls', '.xlsb', '.ods'])
                        }
                        goTo={goToRange}
                        onClose={() => {
                          setVersionsOpen(false);
                          focusInput();
                        }}
                      />
                    ),
                  } satisfies TaskPaneDef,
                ]
              : []),
            ...(scriptOpen
              ? [
                  {
                    id: 'script',
                    title: t('script.title'),
                    icon: <SquareTerminal size={14} aria-hidden />,
                    content: (
                      <ScriptPane
                        app="sheet"
                        host={() =>
                          host && info
                            ? sheetScriptHost({
                                host,
                                activeSheet: () => sheetNow.current,
                                decimal,
                                onEnd: refreshInfo,
                              })
                            : null
                        }
                        recorder={recorder}
                        checkShortcut={scriptBindings.checkShortcut}
                        {...(platform.scriptSchedules
                          ? { schedules: platform.scriptSchedules }
                          : {})}
                      />
                    ),
                  } satisfies TaskPaneDef,
                ]
              : []),
            ...(queries.pane ? [queries.pane] : []),
          ]}
          onClose={(id) =>
            id === 'queries'
              ? queries.closePane()
              : id === 'comments'
                ? setCommentsOpen(false)
                : id === 'script'
                  ? setScriptOpen(false)
                  : setVersionsOpen(false)
          }
          labels={taskPaneLabels(t)}
          focusApp={focusInput}
        />
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
                if (i !== sheet) {
                  setSheet(i);
                  setSelection(single({ row: 0, col: 0 }));
                }
                sheetMenu.open({ x: e.clientX, y: e.clientY - 4, context: 'sheetTab', sheet: i });
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
          <>
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
            {info && <ProtectionStatus info={protection.info} />}
          </>
        }
        right={
          <>
            <CalcStatus state={audit.calc} onCalculate={() => void auditCalculate('workbook')} />
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
      {sheetMenu.element}
      {shortcuts.element}
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
      {dialog?.kind === 'advancedFilter' && <AdvancedFilterDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'cf' && <CfDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'cfManage' && <CfManagerDialog {...common} />}
      {dialog?.kind === 'dv' && <ValidationDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'dataTable' && <DataTableDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'solver' && (
        <SolverDialog
          {...common}
          objective={`${colName(selection.active.col)}${selection.active.row + 1}`}
          onCreated={(s) => setSheet(s)}
        />
      )}
      {dialog?.kind === 'analysis' && (
        <AnalysisDialog
          {...common}
          sheets={info?.sheets.map((s) => s.name) ?? []}
          input={rangeA1(r)}
          onCreated={(s) => setSheet(s)}
        />
      )}
      {dialog?.kind === 'textToColumns' && (
        <TextToColumnsDialog
          {...common}
          sheets={info?.sheets.map((s) => s.name) ?? []}
          range={toRange(r)}
        />
      )}
      {dialog?.kind === 'dedupe' && (
        <RemoveDuplicatesDialog
          {...common}
          range={toRange(r)}
          onResult={(res) =>
            setDialog({
              kind: 'message',
              title: t('sheetEditor.dataTools.dedupe.resultTitle'),
              message:
                res.removed === 0
                  ? t('sheetEditor.dataTools.dedupe.none', { remaining: res.remaining })
                  : t('sheetEditor.dataTools.dedupe.result', {
                      removed: res.removed,
                      remaining: res.remaining,
                    }),
            })
          }
        />
      )}
      {dialog?.kind === 'consolidate' && (
        <ConsolidateDialog
          {...common}
          sheets={info?.sheets.map((s) => s.name) ?? []}
          selection={rangeA1(r)}
        />
      )}
      {dialog?.kind === 'subtotal' && <SubtotalDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'goalSeek' && (
        <GoalSeekDialog
          {...common}
          target={`${colName(selection.active.col)}${selection.active.row + 1}`}
        />
      )}
      {dialog?.kind === 'checkDocument' && (
        <SheetCheckDocumentDialog exec={exec} goTo={goToRange} onClose={common.onClose} />
      )}
      {readAloud.bar}
      {translate.pane}
      {queries.element}
      <SheetEditSpelling
        host={host}
        inputs={spellInputs}
        text={edit ? edit.text : null}
        lang={spelling.lang}
        enabled={spelling.asYouType}
      />
      {spelling.dialog && host && (
        <SheetSpellDialog
          host={host}
          lang={spelling.lang}
          setLang={spelling.setLang}
          onClose={() => {
            setSpellDialog(false);
            focusInput();
          }}
          onShow={spellShow}
          onChanged={refreshInfo}
        />
      )}
      {altDialog && (
        <CellPictureAltDialog
          initial={altDialog.alt}
          decorative={altDialog.decorative}
          onClose={() => {
            setAltDialog(null);
            focusInput();
          }}
          onApply={async (alt, decorative) => {
            const d = altDialog;
            setAltDialog(null);
            await exec({
              cmd: 'cellImageAlt',
              sheet: d.sheet,
              row: d.row,
              col: d.col,
              alt,
              decorative,
            });
            focusInput();
          }}
        />
      )}
      {formDialog && (
        <FormatControlDialog
          info={formDialog}
          onClose={() => {
            setFormDialog(null);
            focusInput();
          }}
          onApply={async (spec) => {
            const done = await exec({
              cmd: 'formatFormControl',
              sheet,
              index: formDialog.index,
              spec,
            });
            if (done) setFormDialog(null);
          }}
        />
      )}
      {dialog?.kind === 'tableRename' && activeTable && (
        <TableNameDialog
          sheet={sheet}
          table={activeTable}
          exec={(cmd) => exec(cmd)}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === 'tableResize' && activeTable && (
        <TableResizeDialog
          sheet={sheet}
          table={activeTable}
          exec={(cmd) => exec(cmd)}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === 'tableTotals' && activeTable && (
        <TableTotalsDialog
          sheet={sheet}
          table={activeTable}
          exec={(cmd) => exec(cmd)}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === 'sparklineInsert' && (
        <InsertSparklinesDialog
          sheet={sheet}
          selection={rangeOf(selection)}
          exec={(cmd) => exec(cmd)}
          onClose={common.onClose}
        />
      )}
      {dialog?.kind === 'sparklineOptions' && sparkGroup && (
        <SparklineOptionsDialog
          sheet={sheet}
          group={sparkGroup}
          exec={(cmd) => exec(cmd)}
          onClose={common.onClose}
        />
      )}
      {audit.dialog?.kind === 'evaluate' && (
        <EvaluateFormulaDialog
          host={host}
          cell={audit.dialog.cell}
          onClose={() => {
            setAuditDialog(null);
            focusInput();
          }}
        />
      )}
      {audit.dialog?.kind === 'errors' && (
        <ErrorCheckDialog
          host={host}
          sheet={sheet}
          rules={audit.rules}
          onRules={audit.setRules}
          onGoto={(row, col) => setSelection(single({ row, col }))}
          onEvaluate={(row, col) => setAuditDialog({ kind: 'evaluate', cell: { sheet, row, col } })}
          onDone={refreshInfo}
          onClose={() => {
            setAuditDialog(null);
            focusInput();
          }}
          fail={fail}
        />
      )}
      {audit.dialog?.kind === 'options' && (
        <CalcOptionsDialog
          host={host}
          onClose={() => {
            setAuditDialog(null);
            focusInput();
          }}
          onDone={async (s) => {
            audit.setCalc(s);
            await refreshInfo();
          }}
          fail={fail}
        />
      )}
      {audit.dialog?.kind === 'circular' && (
        <CircularWarning
          labels={audit.dialog.labels}
          onOptions={() => setAuditDialog({ kind: 'options' })}
          onClose={() => {
            setAuditDialog(null);
            focusInput();
          }}
        />
      )}
      {watchOpen && (
        <WatchWindow
          host={host}
          watches={audit.watches}
          revision={revision}
          selection={rangeOf(selection)}
          sheet={sheet}
          onAdd={audit.addWatches}
          onRemove={(i) => audit.setWatches((w) => w.filter((_, j) => j !== i))}
          onGoto={(c) => {
            setSheet(c.sheet);
            setSelection(single({ row: c.row, col: c.col }));
          }}
          onClose={() => setWatchOpen(false)}
        />
      )}
      {dialog?.kind === 'scenarios' && (
        <ScenarioDialog
          {...common}
          sheets={info?.sheets.map((s) => s.name) ?? []}
          selection={rangeA1(rangeOf(selection))}
          author={platform.auth?.user?.name ?? ''}
          onReport={(s) => {
            setSheet(s);
            setSelection(single({ row: 0, col: 0 }));
          }}
        />
      )}
      {dialog?.kind === 'chart' && <ChartDialog {...common} index={dialog.index} />}
      {smartArtDialog && (
        <SmartArtDialog
          mode={smartArtDialog.mode}
          initial={smartArtDialog.initial}
          onClose={() => {
            setSmartArtDialog(null);
            focusInput();
          }}
          onApply={(d) => {
            const { mode, index } = smartArtDialog;
            setSmartArtDialog(null);
            const at: [number, number] = [selection.active.row, selection.active.col];
            void exec<{ index: number }>(
              mode === 'insert' || index === null
                ? { cmd: 'insertSmartArt', sheet, from: at, ...d }
                : { cmd: 'setSmartArt', sheet, index, ...d },
            ).then((r) => {
              if (r) {
                setChart(null);
                setSmartArt(r.index);
              }
            });
            focusInput();
          }}
        />
      )}
      {saveTemplate && chart !== null && (
        <SaveChartTemplateDialog
          load={loadGallery}
          onClose={() => {
            setSaveTemplate(false);
            focusInput();
          }}
          onSaved={(name) => setStatus(t('chartDesign.templateSaved', { name }))}
        />
      )}
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
      {dialog?.kind === 'customViews' && host && (
        <CustomViewsDialog
          host={host}
          exec={exec}
          sheet={sheet}
          current={() => ({
            zoom,
            active: [selection.active.row, selection.active.col],
            sqref: rangeA1(rangeOf(selection)),
          })}
          onApplied={(v) => {
            setSheet(v.sheet);
            if (v.zoom) setZoom(v.zoom / 100);
            if (v.active) setSelection(single({ row: v.active[0], col: v.active[1] }));
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === 'tempViews' && (
        <TempViewsDialog
          views={tempViews.views}
          onKeep={(name) => tempViews.add({ name, sheet, zoom, scroll, active: selection.active })}
          onShow={(v) => {
            setSheet(v.sheet);
            setZoom(v.zoom);
            setSelection(single(v.active));
            const px = PX_PER_PT * v.zoom;
            requestAnimationFrame(() =>
              scrollerRef.current?.scrollTo(v.scroll.x * px, v.scroll.y * px),
            );
          }}
          onDelete={tempViews.remove}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === 'printPreview' && (
        <PrintPreviewDialog
          {...common}
          onSetup={() => setDialog({ kind: 'pageSetup' })}
          onPrint={() => {
            setDialog(null);
            run('file.printNow');
          }}
        />
      )}
      {dialog?.kind === 'print' && host.printPdf && host.printPreview && (
        <PrintPanel
          app="sheet"
          host={{
            printPdf: host.printPdf.bind(host),
            printPreview: host.printPreview.bind(host),
            getGlyphOutlines: host.getGlyphOutlines.bind(host),
          }}
          initial={{
            sheet: {
              scope: 'active',
              sheets: [sheet],
              selection: `${colName(r.c0)}${r.r0 + 1}:${colName(r.c1)}${r.r1 + 1}`,
            },
          }}
          onPrint={(pdf) =>
            platform.print.printPdf(
              pdf,
              fileName.replace(/\.(xlsx|xlsm|csv)$/i, '') || t('sheetEditor.untitled'),
            )
          }
          onClose={common.onClose}
          fail={fail}
        />
      )}
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
