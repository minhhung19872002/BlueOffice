import { SheetA11y } from './SheetA11y';
import {
  cellNear,
  chromePage,
  expandMerges,
  geometryFromPage,
  hitLocal,
  rangeBox,
  segSize,
  type GridGeometry,
  type LocalHit,
} from './gridView';
import { overlayCommands, parseColor } from './gridOverlay';
import {
  CELL_FONT_FAMILY,
  bottomBaseline,
  caretAt,
  cellFontFor,
  editorWidth,
  loadCellFonts,
  resizeTip,
  wheelRows,
  wheelScrollPt,
  type FontMetrics,
} from './gridFeel';
import { GlyphStore, ImageStore, PX_PER_PT, renderPage } from '@blueoffice/canvas-host';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import type {
  TimelineInfo,
  SlicerInfo,
  BorderSpec,
  CellFormatState,
  ChartElementsInfo,
  ChartElementsPatch,
  ChartInfo,
  SheetSmartArtInfo,
  SmartArtPreview,
  PasteMode,
  PivotInfo,
  Platform,
  Rejection,
  Rgba,
  SheetCommand,
  SheetHit,
  SheetHost,
  SheetRange,
  SheetThreadInfo,
  SheetView,
  StylePatch,
  WorkbookInfo,
  RangeStats,
  AutoSumFunction,
  AutoSumPlan,
  FlashFillResult,
  SortRegion,
  AutoFillMode,
  FormulaCheck,
  InvalidCellsResult,
  SheetPermissions,
  FormControlInfo,
  SheetSecurityResult,
  SparklineGroupInfo,
  TableInfo,
  TableStyleEntry,
} from '@blueoffice/platform';
import {
  ARRAY_PART,
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
  TABULAR_FORMATS,
  TABULAR_OPEN_EXTENSIONS,
  WEB_FORMAT_COMMANDS,
  webFormats,
  webOpenExtensions,
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
  StatusButtons,
  StatusZoom,
  STATUS_ITEMS,
  useStatusBarPrefs,
  useThemePref,
  TitleBar,
  QuickAccessBar,
  useCommandSearchShortcut,
  useResolvedTheme,
  TaskPaneHost,
  useTaskPaneLayout,
  type RibbonItem,
  type RibbonTab,
  type TaskPaneDef,
  type ThemeSetting,
  SHEET_MENUS,
  isContextMenuKey,
  registryMenuLookup,
  resolveMenu,
  ribbonMenuLookup,
  type SheetMenuContext,
  BackgroundRemovalMode,
  type BackgroundPreviewValue,
  Icon,
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
  Bold,
  CalendarRange,
  ChartColumn,
  ChartSpline,
  ClipboardCheck,
  History,
  SpellCheck,
  ClipboardList,
  ClipboardPaste,
  Coins,
  DecimalsArrowLeft,
  DecimalsArrowRight,
  Columns2,
  Columns3,
  Copy,
  Eraser,
  Eye,
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
  Omega,
  Shapes,
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
  ListChecks,
  ListFilter,
  FunnelX,
  FunnelPlus,
  Calculator,
  Sigma,
  SquareFunction,
  ChevronDown,
  Group,
  Ungroup,
  Snowflake,
  Table2,
  TableProperties,
  Tag,
  Tags,
  TableCellsMerge,
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
  SquareMinus,
  SquarePlus,
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
  Mic,
  SquareTerminal,
  FileCode2,
  FileSearch,
  Braces,
  Database,
  FileCode,
  FileSpreadsheet,
  FileText,
  Globe,
  AppWindow,
  LayoutGrid,
  RotateCcw,
  ArrowDownUp,
  Zap,
  BookA,
  Link2,
} from 'lucide-react';
import { SplitPane } from './SplitPane';
import {
  freezeFromSplit,
  splitAtCell,
  splitNeedsUnfreeze,
  updateSplit,
  type SheetSplit,
} from './sheetSplit';
import { recordSheetHost, ScriptPane, ScriptRecorder, sheetScriptHost } from '@blueoffice/script';
import { sheetSelection, useAddins } from '@blueoffice/addins';
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type MutableRefObject,
  type ReactNode,
} from 'react';
import {
  CommandRegistry,
  FileBackstage,
  useWindowTools,
  FileProtectionUi,
  ProtectionStatus,
  useFileProtection,
  MacroBanner,
  MacroViewer,
  useMacroViewer,
  PasswordDialog,
  PdfExportDialog,
  SaveAsTypeDialog,
  SavePasswordPanel,
  SmartArtDialog,
  smartArtPicturePicker,
  type SmartArtPreviewLoader,
  SmartArtLivePreview,
  SmartArtPaneToggle,
  SmartArtTextPane,
  registerSmartArtTools,
  smartArtDesignGroups,
  smartArtDesignLabels,
  type SmartArtToolsApi,
  latestOnly,
  SymbolPanel,
  IconPanel,
  insertSheetIcons,
  PrintPanel,
  clipboardHistory,
  fromBase64,
  planOpen,
  ReadAloudLink,
  DictationLink,
  registerDictationCommands,
  useDictation,
  registerReadAloudCommands,
  registerTranslateCommands,
  TranslateLink,
  useTranslate,
  useThesaurus,
  AiLink,
  registerAiCommands,
  useBlueAi,
  type TranslateAction,
  ThesaurusLink,
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
  useRibbonCustomization,
  type OpenContextMenu,
  pictureTabLabels,
  ChartFormatPane,
  chartElementsGroup,
  registerChartElementCommands,
  PasteOptionsButton,
} from '@blueoffice/editor';
import { cycleReference } from './formulaRef';
import { editableInput, isArrayEntry } from './arrayFormula';
import { EditLinksDialog } from './links';
import {
  argumentHint,
  callAt,
  canPoint,
  formulaRefs,
  functionMatches,
  functionPrefix,
  mirrorParts,
  refText,
} from './formulaEdit';
import { sheetReadUnits } from './readAloud';
import { sheetDictationTarget } from './dictation';
import { asTextInput, translateCells, type CellTranslateMode } from './translate';
import { cellWord } from './thesaurus';
import { SHEET_AI_COMMANDS, SheetAiTargetLink, sheetAiFeatures } from './sheetAi';
import { useSheetQueries } from './queries';
import { useDataModel } from './dataModel';
import {
  SheetClipboardUi,
  sheetPasteCommand,
  type SheetPasteBase,
  type SheetPasteChoice,
} from './pasteOptions';
import { AnalysisDialog } from './analysis';
import { QuickAnalysis, RecommendedChartsDialog } from './quickAnalysis';
import {
  AUTO_SUM_COMMANDS,
  AUTO_SUM_FUNCTIONS,
  AutoSumLink,
  InsertFunctionDialog,
  autoSumCommand,
} from './autoSum';
import { registerMapDatasets } from './mapDatasets';
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
  ErrorMarks,
  ErrorCheckDialog,
  EvaluateFormulaDialog,
  TraceArrowsOverlay,
  WatchWindow,
  useAudit,
} from './audit';
import { ProtectSheetDialog, UnprotectDialog } from './protection';
import { ScenarioDialog } from './scenarios';
import {
  EditSparklineDataDialog,
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
import {
  FrameOptionsDialog,
  InsertFramesDialog,
  PivotChartFilters,
  SlicerFrame,
  TimelineFrame,
  timelineShowOf,
} from './slicers';
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
  type CellPictureTarget,
  CellPictureAltDialog,
  clickCheckbox,
  deleteCheckboxes,
  loadRequestedImages,
  mediaState,
  registerCellMediaCommands,
  sheetMediaSource,
} from './cellMedia';
import type { CellMediaState, DocProperties, VbaProjectInfo } from '@blueoffice/platform';
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
import { FormatCellsDialog, type FormatCellsTab, type NumberPreset } from './FormatCellsDialog';
import {
  BORDER_MENU,
  COMMA_STYLE,
  formatKeyCommand,
  shownDecimals,
  stepDecimals,
  stepFontSize,
} from './homeFormat';
import {
  CellsDialog,
  ConfirmDialog,
  CreateTableDialog,
  FontNameBox,
  FontSizeBox,
  NumberFormatBox,
  SizeDialog,
  SplitButton,
} from './homeControls';
import { VERTICAL_TEXT } from './orientation';
import { PageSetupDialog, PrintPreviewDialog } from './pageSetup';
import { AdvancedFilterDialog, SortDialog, SortWarningDialog } from './sortFilter';
import { CreateNamesDialog, NameEditDialog, NameManagerDialog, absoluteRef } from './names';
import { SeriesDialog } from './fillSeries';
import {
  CustomViewsDialog,
  PageBreakOverlay,
  PageLayoutView,
  sheetViewTabGroups,
  TempViewsDialog,
  useSheetViews,
  useTempViews,
} from './sheetViews';
import { OutlineBar, outlineBarSize } from './outlineBar';
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
  MAX_COL,
  MAX_ROW,
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
  /^(?:format\.(?:bold|italic|underline|strike|align\w+|wrap|indentInc|indentDec|fontGrow|fontShrink|percent|comma|currency|date|mergeAcross|mergeCells|unmerge|clear|lockCell|decimalInc|decimalDec|num(?:General|Number|Time|Date|Currency|Scientific)|bordersLast|border(?:Outline|None)|fontColorLast|fillColorLast)|cells\.(?:insert|delete|hide|unhide|autofit)\w*|data\.sort(?:Asc|Desc)|edit\.(?:clear|fillDown|fillRight))$/;

/**
 * The orientation menu: `textRotation` of each choice (ECMA-376 Part 1, §18.8.1: 0–90
 * counter-clockwise, 91–180 clockwise, 255 vertical).
 */
const ORIENTATIONS: Record<string, number> = {
  ccw: 45,
  cw: 135,
  vertical: VERTICAL_TEXT,
  up: 90,
  down: 180,
};

/**
 * Whether the first row of `r` looks like column names (Create table): every cell of it
 * holds text (no number, no formula) and data rows follow.
 */
async function guessHeaders(
  host: Pick<SheetHost, 'cell'>,
  sheet: number,
  r: Range,
): Promise<boolean> {
  if (r.r1 <= r.r0) return false;
  const cols = Math.min(r.c1 - r.c0 + 1, 64);
  const first = await Promise.all(
    Array.from({ length: cols }, (_, i) => host.cell(sheet, r.r0, r.c0 + i)),
  );
  const numeric = (s: string) =>
    Number.isFinite(Number(s.replace(/[.\s]/g, '').replace(',', '.').replace('%', '')));
  return first.every((c) => {
    const v = c.input.trim();
    return v !== '' && !v.startsWith('=') && !numeric(v);
  });
}

/** Items of a two-row ribbon group: the `top` ids on the first row, the others below. */
function twoRows(top: string[], items: RibbonItem[]): RibbonItem[] {
  return items.map((i) => ({ ...i, row: top.includes(i.id) ? 1 : 2 }));
}
/** Font group: name, size and the size steps on top; styles, borders and colours below. */
const FONT_TOP_ROW = ['format.fontName', 'format.fontSize', 'format.fontGrow', 'format.fontShrink'];
/** Number group: the format box (and its menu) on top, the format buttons below. */
const NUMBER_TOP_ROW = ['format.numberFormatBox', 'format.numberFormat'];

/**
 * Drop-down menus of the Home tab that list commands (command id, label key): Clear, the
 * merge choices, and Insert / Delete / Format of the Cells group.
 */
const COMMAND_MENUS: Record<string, [string, string][]> = {
  clear: [
    ['edit.clearAll', 'sheetEditor.clearAll'],
    ['format.clear', 'sheetEditor.clearFormats'],
    ['edit.clear', 'sheetEditor.clear'],
  ],
  merge: [
    ['format.mergeCenter', 'sheetEditor.mergeCenter'],
    ['format.mergeAcross', 'sheetEditor.mergeAcross'],
    ['format.mergeCells', 'sheetEditor.mergeCells'],
    ['format.unmerge', 'sheetEditor.unmerge'],
  ],
  insert: [
    ['cells.insert', 'sheetEditor.insertCells'],
    ['cells.insertRow', 'sheetEditor.insertRow'],
    ['cells.insertCol', 'sheetEditor.insertCol'],
    ['sheet.add', 'sheetEditor.addSheet'],
  ],
  delete: [
    ['cells.delete', 'sheetEditor.deleteCells'],
    ['cells.deleteRow', 'sheetEditor.deleteRow'],
    ['cells.deleteCol', 'sheetEditor.deleteCol'],
    ['sheet.delete', 'sheetEditor.deleteSheet'],
  ],
  cellFormat: [
    ['cells.rowHeight', 'sheetEditor.rowHeightTitle'],
    ['cells.autofitRow', 'sheetEditor.autofitRow'],
    ['cells.colWidth', 'sheetEditor.colWidthTitle'],
    ['cells.autofitCol', 'sheetEditor.autofitCol'],
    ['cells.hideRows', 'sheetEditor.hideRows'],
    ['cells.unhideRows', 'sheetEditor.unhideRows'],
    ['cells.hideCols', 'sheetEditor.hideCols'],
    ['cells.unhideCols', 'sheetEditor.unhideCols'],
    ['sheet.rename', 'sheetEditor.renameSheet'],
    ['sheet.moveLeft', 'sheetEditor.moveSheetLeft'],
    ['sheet.moveRight', 'sheetEditor.moveSheetRight'],
    ['format.lockCell', 'sheetEditor.protection.lockCell'],
    ['format.cells', 'sheetEditor.fmt.title'],
  ],
};

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
  /** Elements of the selected chart and their options (CORE-CHART-005). */
  chartElements: ChartElementsInfo | null;
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
/** Selections above this many cells get their statistics after a short pause. */
const STATS_ASYNC = 50_000;
/** Selection statistics in the order the status bar shows them: item id, label key. */
const STAT_ITEMS: readonly (readonly [string, string])[] = [
  ['average', 'sheetEditor.average'],
  ['count', 'sheetEditor.count'],
  ['numericalCount', 'statusBar.numericalCount'],
  ['min', 'statusBar.min'],
  ['max', 'statusBar.max'],
  ['sum', 'sheetEditor.sum'],
];
const STAT_TEST_IDS: Record<string, string> = { numericalCount: 'numerical-count' };
const NO_STATS: RangeStats = {
  sum: 0,
  count: 0,
  numbers: 0,
  average: null,
  min: null,
  max: null,
};
const statsKeyOf = (sheet: number, ranges: Range[], revision: number) =>
  `${sheet}:${ranges.map((r) => `${r.r0},${r.c0},${r.r1},${r.c1}`).join(';')}:${revision}`;

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
  /** Number format the cell takes when the proposed formula is entered (auto total). */
  numFmt?: string;
  /** Sheet of the edited cell (another sheet may be shown while picking references). */
  sheet?: number;
}

type DialogState =
  | { kind: 'format'; tab?: FormatCellsTab }
  /** Column width (characters) / row height (points). */
  | { kind: 'size'; rows: boolean; initial: number }
  /** Insert / Delete cells: how the other cells shift. */
  | { kind: 'cells'; insert: boolean }
  /** Merging several filled cells keeps only the upper-left value. */
  | { kind: 'mergeWarn'; center: boolean; across: boolean }
  /** Writing over cells that hold data: asked first, `run` does the write. */
  | { kind: 'replaceWarn'; run: () => void }
  /** Create table: its range and whether the first row holds the names. */
  | { kind: 'table'; range: string; headers: boolean }
  | { kind: 'sort' }
  /** A quick sort of one column inside a wider block: expand the selection? */
  | { kind: 'sortWarning'; descending: boolean; col: number; range: SheetRange }
  /** Defined names: the Name Manager, New name, Create from Selection. */
  | { kind: 'nameManager' }
  | { kind: 'editLinks' }
  /** Home › Fill › Series. */
  | { kind: 'series'; firstIsDate: boolean }
  | { kind: 'defineName' }
  | { kind: 'createNames'; guess: { top: boolean; left: boolean } }
  | { kind: 'advancedFilter' }
  | { kind: 'cf' }
  | { kind: 'cfManage' }
  | { kind: 'dv' }
  | { kind: 'goalSeek' }
  | { kind: 'scenarios' }
  | { kind: 'sparklineInsert' }
  | { kind: 'sparklineOptions' }
  | { kind: 'sparklineData'; single?: boolean }
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
  /** Slicers / timelines to insert over the fields of a pivot table. */
  | { kind: 'frameInsert'; pivot: PivotInfo; frames: 'slicer' | 'timeline' }
  /** Options and report connections of a slicer / timeline of the sheet. */
  | { kind: 'frameOptions'; frames: 'slicer' | 'timeline'; index: number }
  | { kind: 'message'; title: string; message: string };

type MenuState = { kind: string; x: number; y: number };

/** Cursors over the column / row headers: a small black arrow pointing into the grid. */
const arrowCursor = (path: string, hx: number, hy: number, fallback: string) =>
  `url("data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><path d='${path}' fill='black' stroke='white' stroke-width='1'/></svg>`,
  )}") ${hx} ${hy}, ${fallback}`;
const HEADER_CURSOR_DOWN = arrowCursor('M6.5 1.5h3v7h3l-4.5 6-4.5-6h3z', 8, 15, 's-resize');
const HEADER_CURSOR_RIGHT = arrowCursor('M1.5 6.5v3h7v3l6-4.5-6-4.5v3z', 15, 8, 'e-resize');

/** A 2D context for measuring text (shared; created on first use). */
let measuring: CanvasRenderingContext2D | null | undefined;
function measureContext(): CanvasRenderingContext2D | null {
  if (measuring === undefined)
    measuring =
      typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
  return measuring;
}

type Drag =
  | { kind: 'select' }
  /** Dragging over the column (or row) headers from `anchor`. */
  | { kind: 'headers'; rows: boolean; anchor: number }
  /** Picking a reference for the formula being typed. */
  | { kind: 'point' }
  | { kind: 'fill'; source: Range }
  /** Dragging the selection by its border (Ctrl: copy). */
  | { kind: 'move'; source: Range; start: CellPos; to: Range | null }
  | { kind: 'resize'; rows: boolean; index: number; start: number; size: number }
  /** Moving (or, with 
esize, sizing) a chart: anchors in cells. */
  | {
      kind: 'chart';
      /** A SmartArt diagram instead of a chart (CORE-SMART-005). */
      smart?: boolean;
      /** A slicer or timeline frame instead of a chart. */
      frame?: 'slicer' | 'timeline';
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
  // The theme picked in File › Options (remembered) wins over the editor's own setting.
  const theme = useResolvedTheme(useThemePref(themeSetting));
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
  /** The selection now (read by add-ins, CORE-ADDIN-001). */
  const selectionNow = useRef(selection);
  useEffect(() => {
    selectionNow.current = selection;
  }, [selection]);
  const [scroll, setScroll] = useState({ x: 0, y: 0 });
  const [size, setSize] = useState({ w: 800, h: 500 });
  const [content, setContent] = useState({ w: 2000, h: 4000 });
  const [zoom, setZoom] = useState(1);
  const [edit, setEdit] = useState<EditState | null>(null);
  const [barText, setBarText] = useState('');
  // From the engine when the drawn grid cannot be read (see localRects).
  const [engineSelRect, setSelRect] = useState<Rect | null>(null);
  const [engineActiveRect, setActiveRect] = useState<Rect | null>(null);
  /** The formula bar shows several lines (Ctrl+Shift+U). */
  const [barExpanded, setBarExpanded] = useState(false);
  const [status, setStatus] = useState('');
  // Quick Analysis panel and Insert › Recommended charts (CORE-DESIGN-001).
  const [quickOpen, setQuickOpen] = useState(false);
  const [fnDialog, setFnDialog] = useState(false);
  const [recommendOpen, setRecommendOpen] = useState(false);
  /** Cells converted from TCVN3 / VNI fonts to Unicode on open (0 = no notice). */
  const [legacyNotice, setLegacyNotice] = useState(0);
  const [stats, setStats] = useState<(RangeStats & { key: string }) | null>(null);
  /** Split panes of the grid (View › Window › Split); a view of this window only. */
  const [split, setSplit] = useState<SheetSplit | null>(null);
  const [renaming, setRenaming] = useState<number | null>(null);
  /** When the sheet name box opened: the grid's deferred focus right after is ignored. */
  const renameOpened = useRef(0);
  useEffect(() => {
    if (renaming !== null) renameOpened.current = performance.now();
  }, [renaming]);
  const [revision, setRevision] = useState(0);
  const [loadSeq, setLoadSeq] = useState(0);
  const [fmt, setFmt] = useState<CellFormatState | null>(null);
  // The in-cell editor draws with the cell's own bundled font (no jump of the text when
  // editing starts); its metrics place the baseline where the engine draws it.
  const cellFamily = cellFontFor(fmt?.fontName);
  const [cellFontMetrics, setCellFontMetrics] = useState<Record<string, FontMetrics | null>>({});
  useEffect(() => {
    let live = true;
    void loadCellFonts(platform.fonts, cellFamily).then(
      (m) => live && setCellFontMetrics((f) => (cellFamily in f ? f : { ...f, [cellFamily]: m })),
    );
    return () => {
      live = false;
    };
  }, [platform.fonts, cellFamily]);
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
  /** The open drop-down of entries under the active cell (validation list or Alt+Down). */
  const [pick, setPick] = useState<{ items: string[]; index: number } | null>(null);
  /** Cells marked by the last copy / cut (moving border) until Esc, an entry or a paste. */
  const [copyMark, setCopyMark] = useState<{ sheet: number; range: Range; cut: boolean } | null>(
    null,
  );
  const [copyRect, setCopyRect] = useState<{ mark: object; rect: Rect | null } | null>(null);
  /** Size of the row / column headers on screen (px), for their selection highlight. */
  const [headers, setHeaders] = useState<{ w: number; h: number } | null>(null);
  /** Caret of the edit (argument hint, completion list). */
  const [caret, setCaret] = useState(0);
  /** Function names of the engine (completion while typing a formula). */
  const fnNames = useRef<string[] | null>(null);
  /** Completion list of function names: where the typed name starts, the matches. */
  const [fnPick, setFnPick] = useState<{ from: number; items: string[]; index: number } | null>(
    null,
  );
  /** The reference being picked with the arrow keys / mouse (point mode). */
  const picking = useRef<{
    start: number;
    end: number;
    anchor: CellPos;
    focus: CellPos;
    /** Sheet the reference is picked on. */
    sheet?: number;
    /** Only the sheet is picked so far (`Sheet2!`): the next arrow / click picks the cell. */
    pending?: boolean;
  } | null>(null);
  /** Enter after a run of Tabs goes to the next row of the column the run started in. */
  const tabRun = useRef<{ startCol: number; at: CellPos } | null>(null);
  /** Text entries of the column around the edited cell (AutoComplete). */
  const entries = useRef<{ key: string; list: string[] } | null>(null);
  /** Boxes of the references of the formula being typed (coloured borders). */
  const [refBoxes, setRefBoxes] = useState<{
    text: string;
    sheet: string;
    boxes: (Rect & { color: string })[];
  } | null>(null);
  /** The active cell has a data validation rule (inputs are checked). */
  const [hasRule, setHasRule] = useState(false);
  const [painter, setPainter] = useState<Range | null>(null);
  /** Format painter kept on after a double click; when it was turned on. */
  const [painterKeep, setPainterKeep] = useState({ sticky: false, at: 0 });
  const [fillRect, setFillRect] = useState<Rect | null>(null);
  /** Selected chart: its anchors and on-screen box (px). */
  const [chartInfo, setChartInfo] = useState<ChartInfo | null>(null);
  const [chartBox, setChartBox] = useState<Rect | null>(null);
  const chartTarget = useRef<{ from: [number, number]; to: [number, number] } | null>(null);
  const [guide, setGuide] = useState<{ x?: number; y?: number } | null>(null);
  /** Size tip shown while a column / row boundary is dragged (CSS px in the grid). */
  const [sizeTip, setSizeTip] = useState<{ x: number; y: number; text: string } | null>(null);
  /**
   * Geometry of the grid as last drawn (points) and the zoom it was drawn at: clicks are
   * hit-tested and the selection drawn from it in the same frame (no engine round trip).
   */
  const [grid, setGrid] = useState<{ g: GridGeometry; scale: number } | null>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const [cursor, setCursor] = useState('cell');
  const [chart, setChart] = useState<number | null>(null);
  /** The Format chart pane (CORE-CHART-005). */
  const [chartFormatOpen, setChartFormatOpen] = useState(false);
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
  // "Convert macro VBA to BlueScript" (CORE-SCRIPT-003): opens the converter of the script pane.
  const [vbaRequest, setVbaRequest] = useState(0);
  const vbaRequestHandled = useCallback(() => setVbaRequest(0), []);
  // Read-only macro viewer (CORE-VBA-002): a module goes to the script pane as a draft.
  const macroViewer = useMacroViewer(useCallback(() => setScriptOpen(true), []));
  const showMacros = macroViewer.show;
  // Asked of the engine host itself: looking at macros is not a recorded action.
  const loadMacros = useCallback(
    () =>
      platform.sheet
        ? platform.sheet.exec<VbaProjectInfo>({ cmd: 'vbaProject' })
        : Promise.reject(new Error('no sheet engine')),
    [platform.sheet],
  );
  // Task pane layout of BlueSheet, remembered per app (CORE-UI-006).
  const [paneLayout, setPaneLayout] = useTaskPaneLayout('sheet');
  const [threads, setThreads] = useState<SheetThreadInfo[]>([]);
  /** Symbol picker panel (insert into the cell being edited, else the active cell). */
  const [symbolsOpen, setSymbolsOpen] = useState(false);
  const [iconsOpen, setIconsOpen] = useState(false);
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
  /** The last fill-handle fill while Auto Fill Options can redo it another way. */
  const [fillUi, setFillUi] = useState<{
    n: number;
    mode: AutoFillMode;
    source: SheetRange;
    target: SheetRange;
    sel: Selection;
  } | null>(null);
  /** The last copy was a cut (pasted once: no paste options). */
  const [lastCut, setLastCut] = useState(false);
  const pivotLabels = usePivotLabels();

  const scrollerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const barRef = useRef<HTMLTextAreaElement>(null);
  /** The pages of the page layout view, while it is shown. */
  const pageLayoutRef = useRef<HTMLDivElement | null>(null);
  const spellInputs = useMemo(() => [inputRef, barRef], []);
  // Ctrl+wheel zooms the grid by 10 % steps (not the page).
  const gridShown = !!info;
  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      const step = e.deltaY < 0 ? 0.1 : -0.1;
      const lo = ZOOMS[0] ?? 0.5;
      const hi = ZOOMS[ZOOMS.length - 1] ?? 2;
      setZoom((z) => Math.min(hi, Math.max(lo, Math.round((z + step) * 10) / 10)));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, [gridShown]);
  /** Input that last had the caret while editing (the symbol picker inserts there). */
  const lastInput = useRef<'cell' | 'bar'>('cell');
  const glyphs = useMemo(() => (host ? new GlyphStore(host) : null), [host]);
  // Pictures in cells and IMAGE pictures of the display list (SHEET-CELL-001).
  const cellImages = useMemo(() => (host ? new ImageStore(sheetMediaSource(host)) : null), [host]);
  // Closing the editor releases the decoded pictures (CORE-PERF-001).
  useEffect(() => () => cellImages?.clear(), [cellImages]);
  const renderSeq = useRef(0);
  const drag = useRef<Drag | null>(null);
  const fillTo = useRef<Range | null>(null);
  const composing = useRef(false);
  /** TSV text of our last copy (paste uses the internal cell records when it matches). */
  const [lastCopy, setLastCopy] = useState<string | null>(null);
  /** Other ranges of a Ctrl multi-range selection (the current one is `selection`). */
  const [extra, setExtra] = useState<Range[]>([]);
  const [engineExtraRects, setExtraRects] = useState<Rect[]>([]);
  /** Sheet whose invalid data is circled (`null`: none). */
  const [circlesOn, setCirclesOn] = useState<number | null>(null);
  const [circleRects, setCircleRects] = useState<CircleRect[]>([]);
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
      // Changing part of an array formula is refused with a message.
      if (message.includes(ARRAY_PART))
        setDialog({
          kind: 'message',
          title: t('sheetEditor.arrayFormula.title'),
          message: t('sheetEditor.arrayFormula.partRefused'),
        });
      // Refusals of sheet / workbook protection get their own message.
      else if (message.includes(SHEET_PROTECTED)) setStatus(t('sheetEditor.protection.refused'));
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
      // Pictures of the previous workbook (media part names repeat across files).
      cellImages?.clear();
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
    [platform, t, cellImages],
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
    [host, load, t, setPasswordPrompt],
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
        const sum = info.sheets[view.sheet];
        const geom = geometryFromPage(page, {
          rows: sum?.frozenRows ?? 0,
          cols: sum?.frozenCols ?? 0,
        });
        // Neutral grid chrome, grid lines on device pixels; the selection layer redraws with
        // the geometry of exactly this picture.
        renderPage(
          ctx,
          chromePage(page, scale * dpr, geom),
          glyphs,
          scale * dpr,
          cellImages ?? undefined,
        );
        setGrid(geom ? { g: geom, scale } : null);
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

  // Selection rectangles from the drawn grid, before the browser paints (same frame as the
  // mouse-down / key press); the engine is asked only when the grid could not be read.
  const merges = summary?.merges;
  const hasGrid = grid !== null;
  const localBox = useCallback(
    (r: Range): Rect | null => {
      if (!grid) return null;
      const b = rangeBox(grid.g, expandMerges(r, merges ?? []), hidden);
      const k = grid.scale;
      return b && { x: b.x * k, y: b.y * k, w: b.w * k, h: b.h * k };
    },
    [grid, merges, hidden],
  );
  const localRects = useMemo(() => {
    if (!grid) return null;
    const a = selection.active;
    return {
      sel: localBox(rangeOf(selection)),
      active: localBox({ r0: a.row, c0: a.col, r1: a.row, c1: a.col }),
      extra: extra.map(localBox).filter((x): x is Rect => !!x),
    };
  }, [grid, selection, extra, localBox]);
  const selRect = localRects ? localRects.sel : engineSelRect;
  const activeRect = localRects ? localRects.active : engineActiveRect;
  const extraRects = localRects ? localRects.extra : engineExtraRects;

  // Mouse wheel: three rows a notch, the top row lined up (touchpads scroll smoothly).
  const wheelGrid = useRef(grid);
  useEffect(() => {
    wheelGrid.current = grid;
  }, [grid]);
  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      const rows = wheelRows(e);
      const g = wheelGrid.current;
      if (rows === null || !g) return;
      e.preventDefault();
      el.scrollTop = Math.max(0, el.scrollTop + wheelScrollPt(g.g, rows) * g.scale);
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, []);

  // The selection layer: highlighted headers, shading, border and fill handle.
  const [accent, setAccent] = useState<Rgba>([0x0b, 0x7a, 0x6b, 255]);
  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;
    const css = getComputedStyle(el).getPropertyValue('--bo-sheet-accent');
    setAccent((a) => parseColor(css, a));
  }, [theme]);
  useLayoutEffect(() => {
    const canvas = overlayRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.round(size.w * dpr);
    const h = Math.round(size.h * dpr);
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
      canvas.style.width = `${size.w}px`;
      canvas.style.height = `${size.h}px`;
    }
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, w, h);
    if (!grid || !glyphs) return;
    const last = info?.sheets[sheet];
    const cmds = overlayCommands(
      grid.g,
      {
        range: rangeOf(selection),
        active: selection.active,
        extra,
        hidden,
        merges: merges ?? [],
        wholeRowsFrom: Math.max(last?.lastRow ?? 0, 999),
        wholeColsFrom: Math.max(last?.lastCol ?? 0, 25),
        accent,
        showRange: chart === null && smartArt === null,
        painter: painter !== null,
      },
      { s: grid.scale * dpr, dpr },
    );
    renderPage(
      ctx,
      { width: grid.g.width, height: grid.g.height, commands: cmds },
      glyphs,
      grid.scale * dpr,
      undefined,
      { transparent: true },
    );
    canvas.dataset.sel = rangeA1(rangeOf(selection));
  }, [
    grid,
    glyphs,
    selection,
    extra,
    hidden,
    merges,
    accent,
    chart,
    smartArt,
    painter,
    size,
    info,
    sheet,
  ]);

  // Selection overlay + formula bar contents.
  /**
   * An entry was refused (formula syntax, validation) and its edit given back: a read of
   * the cell the commit had moved to, still on its way, must not blank the formula bar.
   */
  const holdBar = useRef(false);
  useEffect(() => {
    if (!host || !info) return;
    // An edit given back after a refused entry is shown again: drop the hold.
    if (edit) holdBar.current = false;
    const r = rangeOf(selection);
    let cancelled = false;
    if (hasGrid) {
      // Rectangles come from the drawn grid; only the formula bar needs the engine.
      void host
        .cell(sheet, selection.active.row, selection.active.col)
        .then((cell) => {
          if (!cancelled && !edit && !holdBar.current) setBarText(cell.input);
        })
        .catch(() => {});
      return () => {
        cancelled = true;
      };
    }
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
      if (!edit && !holdBar.current) setBarText(cell.input);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, selection, view, px, revision, edit, hasGrid]);

  // Overlays of the other ranges of a multi-range selection.
  useEffect(() => {
    if (!host || !info || extra.length === 0 || hasGrid) return;
    let cancelled = false;
    void Promise.all(extra.map((r) => host.rangeRect(view, r.r0, r.c0, r.r1, r.c1)))
      .then((rects) => {
        if (!cancelled) setExtraRects(rects.map(px).filter((x): x is Rect => !!x));
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, extra, view, px, revision, hasGrid]);

  // Coloured borders around the references of the formula being typed (those of this sheet).
  const editText = edit?.text;
  // References without a sheet name are on the edited cell's sheet.
  const editHome = info?.sheets[edit?.sheet ?? sheet]?.name;
  useEffect(() => {
    const refs = editText ? formulaRefs(editText) : [];
    if (!host || !editText || !refs.length) return;
    let cancelled = false;
    const here = refs.filter(
      (r) => (r.sheet ?? editHome)?.toLowerCase() === sheetName?.toLowerCase(),
    );
    void Promise.all(
      here.map((r) => host.rangeRect(view, r.range.r0, r.range.c0, r.range.r1, r.range.c1)),
    )
      .then((rects) => {
        if (cancelled) return;
        const boxes: (Rect & { color: string })[] = [];
        rects.forEach((x, i) => {
          const b = px(x);
          if (b) boxes.push({ ...b, color: here[i]!.color });
        });
        setRefBoxes({ text: editText, sheet: sheetName ?? '', boxes });
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, editText, view, px, sheetName, editHome]);

  // The moving border of the copied cells.
  useEffect(() => {
    if (!host || !copyMark || copyMark.sheet !== sheet) return;
    let cancelled = false;
    const r = copyMark.range;
    void host
      .rangeRect(view, r.r0, r.c0, r.r1, r.c1)
      .then((x) => !cancelled && setCopyRect({ mark: copyMark, rect: px(x) }))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, copyMark, sheet, view, px]);

  // The header sizes: the top-left corner of A1 when nothing is scrolled.
  useEffect(() => {
    if (!host || !info) return;
    let cancelled = false;
    void host
      .rangeRect({ ...view, scrollX: 0, scrollY: 0 }, 0, 0, 0, 0)
      .then((r) => {
        const b = px(r);
        if (!cancelled && b)
          setHeaders((h) => (h && h.w === b.x && h.h === b.y ? h : { w: b.x, h: b.y }));
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, view, px]);

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

  // Status-bar statistics of the selection (CORE-UI-007): the engine counts every range
  // once, limited to the used cells; a large selection is asked for after a short pause so
  // dragging stays smooth (the bar shows "Calculating…" meanwhile).
  useEffect(() => {
    if (!host || !info) return;
    const all = [...extra, rangeOf(selection)];
    const cells = all.reduce((n, r) => n + cellsOf(r), 0);
    if (cells <= 1) return;
    const key = statsKeyOf(sheet, all, revision);
    const lastRow = info.sheets[sheet]?.lastRow ?? -1;
    const lastCol = info.sheets[sheet]?.lastCol ?? -1;
    const ranges = all
      .map((r) => ({
        r0: r.r0,
        c0: r.c0,
        r1: Math.min(r.r1, lastRow),
        c1: Math.min(r.c1, lastCol),
      }))
      .filter((r) => r.r1 >= r.r0 && r.c1 >= r.c0);
    let cancelled = false;
    const timer = setTimeout(
      () => {
        void (
          ranges.length
            ? host.exec<RangeStats>({ cmd: 'rangeStats', sheet, ranges })
            : Promise.resolve<RangeStats>(NO_STATS)
        )
          .then((st) => !cancelled && st && setStats({ ...st, key }))
          .catch(() => {});
      },
      cells > STATS_ASYNC ? 150 : 0,
    );
    return () => {
      cancelled = true;
      clearTimeout(timer);
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
      setPick(null);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, selection.active, revision]);

  // Checkbox / picture of the active cell, and the alt-text dialog (SHEET-CELL-001).
  const [activeMedia, setActiveMedia] = useState<CellMediaState | null>(null);
  const [altDialog, setAltDialog] = useState<AltTextTarget | null>(null);
  // Background removal mode of a cell picture (CORE-PIC-002).
  const [bgTarget, setBgTarget] = useState<CellPictureTarget | null>(null);
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
      if (cancelled) return;
      // The chart is gone (undo of its insertion, deleted by a collaborator): unselect.
      if (!c) {
        setChart(null);
        return;
      }
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

  // The pivot table a selected pivot chart plots: its fields get filter buttons on the chart.
  const [chartPivotLoaded, setChartPivotLoaded] = useState<{
    sheet: number;
    info: PivotInfo;
  } | null>(null);
  const chartPivotAt = chart !== null ? (chartInfo?.pivotAt ?? null) : null;
  const chartPivotSheet = chartPivotAt?.[0] ?? -1;
  const chartPivotIndex = chartPivotAt?.[1] ?? -1;
  useEffect(() => {
    if (!host || chartPivotSheet < 0) return;
    let cancelled = false;
    void host
      .exec<PivotInfo[]>({ cmd: 'pivots', sheet: chartPivotSheet })
      .then((list) => {
        const p = list?.[chartPivotIndex];
        if (!cancelled) setChartPivotLoaded(p ? { sheet: chartPivotSheet, info: p } : null);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, chartPivotSheet, chartPivotIndex, revision]);
  const chartPivot =
    chartPivotLoaded &&
    chartPivotLoaded.sheet === chartPivotSheet &&
    chartPivotLoaded.info.index === chartPivotIndex
      ? chartPivotLoaded
      : null;

  // The selected slicer / timeline frame (options, delete, move and size). The selection
  // belongs to one sheet of one opened file.
  const frameKey = `${loadSeq}:${sheet}`;
  const [frameSelAt, setFrameSelAt] = useState<{
    key: string;
    kind: 'slicer' | 'timeline';
    index: number;
  } | null>(null);
  const frameSel = frameSelAt?.key === frameKey ? frameSelAt : null;
  const setFrameSel = useCallback(
    (v: { kind: 'slicer' | 'timeline'; index: number } | null) =>
      setFrameSelAt(v && { key: frameKey, ...v }),
    [frameKey],
  );

  // Timelines of the sheet: a strip of periods over the grid.
  const [timelines, setTimelines] = useState<{ info: TimelineInfo; rect: Rect }[]>([]);
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

  /** The slicer / timeline whose options dialog is open. */
  const frameDialog =
    dialog?.kind === 'frameOptions'
      ? dialog.frames === 'slicer'
        ? (slicers.find((x) => x.info.index === dialog.index)?.info ?? null)
        : (timelines.find((x) => x.info.index === dialog.index)?.info ?? null)
      : null;

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

  // Keep the hidden/visible cell input focused so IME composition starts in it.
  const focusInput = useCallback(() => {
    // Page layout view: the pages take the keys (the grid is not shown), or the cell
    // editor drawn on a page while a cell is edited there.
    const pages = pageLayoutRef.current;
    if (pages?.isConnected) {
      const cell = inputRef.current;
      (cell && pages.contains(cell) ? cell : pages).focus({ preventScroll: true });
      return;
    }
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

  // One element change of the selected chart = one `chartDesign` command (one undo step).
  const sendChartElements = useCallback(
    (patch: ChartElementsPatch) => {
      if (chart !== null) void exec({ cmd: 'chartDesign', sheet, index: chart, elements: patch });
    },
    [exec, sheet, chart],
  );

  /** Runs a read-only command (no refresh). */
  const query = useCallback(
    <T,>(cmd: SheetCommand): Promise<T | null> => exec<T>(cmd, false),
    [exec],
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

  // Data model (SHEET-DM-001): tables, relationships, measures, pivot tables over the model.
  const dataModel = useDataModel({
    host,
    labels: pivotLabels,
    onChanged: refreshInfo,
    onPivot: (s, index) => {
      if (!host) return;
      void host
        .exec<PivotInfo[]>({ cmd: 'pivots', sheet: s })
        .then((list) => {
          const p = list?.[index];
          setSheet(s);
          setPivotPanel(true);
          if (p) setSelection(single({ row: p.location.r0, col: p.location.c0 }));
          setStatus(t('sheetModel.created'));
        })
        .catch(fail);
    },
    fail,
  });
  const { open: openModel, createPivot: modelPivot } = dataModel;

  // The dialog of diagrams: layout thumbnails from the engine, node pictures from a file.
  const loadSmartArtPreviews = useCallback<SmartArtPreviewLoader>(
    async (colors, style) =>
      (
        await host?.exec<{ previews: SmartArtPreview[] }>({
          cmd: 'smartArtPreviews',
          colors,
          style,
        })
      )?.previews ?? [],
    [host],
  );
  const pickSmartArtPicture = useMemo(() => smartArtPicturePicker(platform.files), [platform]);
  // SmartArt Design tools (CORE-SMART-004): text pane, current shape, live preview; each action
  // is one `setSmartArt` of the selected diagram (one undo step).
  const [smartPane, setSmartPane] = useState(false);
  const [smartNode, setSmartNode] = useState<number | null>(null);
  const [smartPreview, setSmartPreview] = useState<string | null>(null);
  // Another diagram (or none): no current shape, no preview; the pane closes with the selection.
  const smartKey = smartArt === null ? null : `${sheet}:${smartArt}`;
  const [smartFor, setSmartFor] = useState(smartKey);
  if (smartFor !== smartKey) {
    setSmartFor(smartKey);
    setSmartNode(null);
    setSmartPreview(null);
    if (smartKey === null) setSmartPane(false);
  }
  const latestPreview = useMemo(() => latestOnly(), []);
  const smartApi = useMemo<SmartArtToolsApi>(
    () => ({
      state: () => (smartArt !== null ? smartInfo : null),
      node: () => smartNode,
      setNode: setSmartNode,
      apply: async (patch) => {
        if (smartArt !== null) await exec({ cmd: 'setSmartArt', sheet, index: smartArt, ...patch });
      },
      toShapes: async () => {
        if (smartArt === null) return;
        await exec({ cmd: 'smartArtToShapes', sheet, index: smartArt });
        setSmartArt(null);
      },
      paneOpen: () => smartPane,
      togglePane: () => setSmartPane((v) => !v),
      loadPreviews: loadSmartArtPreviews,
      preview: (patch) => {
        if (!patch || !smartInfo || !smartBox || !host) {
          latestPreview(Promise.resolve(null), setSmartPreview);
          return;
        }
        const layout = patch.layout ?? smartInfo.layout;
        latestPreview(
          host
            .exec<{ previews: SmartArtPreview[] }>({
              cmd: 'smartArtPreviews',
              items: smartInfo.items,
              colors: patch.colors ?? smartInfo.colors,
              style: patch.style ?? smartInfo.style,
              rtl: smartInfo.rtl,
              width: smartBox.w,
              height: smartBox.h,
            })
            .then((r) => r.previews.find((p) => p.layout === layout)?.svg ?? null),
          setSmartPreview,
        );
      },
    }),
    [
      smartArt,
      smartInfo,
      smartBox,
      smartNode,
      smartPane,
      sheet,
      exec,
      host,
      loadSmartArtPreviews,
      latestPreview,
      setSmartArt,
      setSmartNode,
      setSmartPane,
    ],
  );
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
    [host, setSmartArtDialog],
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
  const { mode: viewMode, setMode: setViewMode, toggleFormulas, toggleOutlineSymbols } = sheetViews;
  // Page layout view: when the editor drawn on a page goes away (Enter, Esc), the keys
  // go back to the pages.
  const editingNow = !!edit;
  useEffect(() => {
    if (viewMode !== 'pageLayout' || editingNow) return;
    const a = document.activeElement;
    if (!a || a === document.body) pageLayoutRef.current?.focus({ preventScroll: true });
  }, [viewMode, editingNow]);
  const showingFormulas = !!sheetViews.state?.showFormulas;
  // The outline bar beside the grid (SHEET-DATA-002), unless hidden (Ctrl+8).
  const outlineShown = sheetViews.state?.outlineSymbols ?? true;
  const outlineSize = outlineBarSize(
    viewMode !== 'pageLayout' && outlineShown ? summary?.outline : null,
  );
  const tempViews = useTempViews();

  // Formula auditing and calculation (SHEET-AUDIT-001).
  const audit = useAudit({ host, sheet, revision, loadSeq, onDone: refreshInfo, fail });
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

  const chooseSlicerItems = useCallback(
    (info: SlicerInfo, items: string[]) => {
      void exec({ cmd: 'slicerSelect', sheet, index: info.index, items });
    },
    [exec, sheet],
  );
  const deleteFrame = useCallback(
    async (kind: 'slicer' | 'timeline', index: number) => {
      setFrameSel(null);
      await exec({ cmd: kind === 'slicer' ? 'deleteSlicer' : 'deleteTimeline', sheet, index });
    },
    [exec, sheet, setFrameSel],
  );
  /** The source records of a pivot table value cell on a new sheet (drill-down). */
  const drillDown = useCallback(
    async (row: number, col: number) => {
      const r = await exec<{ sheet: number; name: string; rows: number }>({
        cmd: 'pivotDrillDown',
        sheet,
        row,
        col,
        name: t('sheetSlicer.detailSheet'),
        labels: pivotLabels,
      });
      if (!r) return;
      setSheet(r.sheet);
      setSelection(single({ row: 0, col: 0 }));
      setStatus(t('sheetSlicer.detailShown', { count: r.rows, name: r.name }));
    },
    [exec, sheet, t, pivotLabels],
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
      // The edited cell's sheet (references may have been picked on another one).
      const target = e.sheet ?? sheet;
      setEdit(null);
      if (target !== sheet) setSheet(target);
      // An entry ends the copy mark (the marked cells can no longer be pasted with Enter).
      setCopyMark(null);
      if (inputRef.current) inputRef.current.value = '';
      // Move first so typing that follows lands in the next cell.
      if (next) setSelection(next);
      try {
        // A formula that does not parse (even read with `;` separators, decimal commas
        // and closing parentheses added) is not entered: the editor stays open on it.
        if (e.text.length > 1 && e.text.startsWith('=')) {
          const chk = await host.exec<FormulaCheck>({ cmd: 'checkFormula', formula: e.text });
          if (chk && chk.error) {
            holdBar.current = true;
            setSelection(single(e.pos));
            setEdit(e);
            setBarText(e.text);
            if (inputRef.current) inputRef.current.value = e.text;
            setDialog({
              kind: 'message',
              title: t('sheetEditor.formulaError.title'),
              message: t('sheetEditor.formulaError.message'),
            });
            return;
          }
        }
        if (hasRule) {
          // Cells with a validation rule are checked by the engine (§18.3.1.32).
          const r = await host.exec<{ rejected: Rejection | null; warning: Rejection | null }>({
            cmd: 'setInputs',
            sheet: target,
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
            holdBar.current = true;
            setSelection(single(e.pos));
            setEdit(e);
            setBarText(e.text);
            if (inputRef.current) inputRef.current.value = e.text;
            return;
          }
        } else if (e.numFmt && e.text.startsWith('=')) {
          // A proposed total takes the number format of its numbers in the same undo step.
          const at = { r0: e.pos.row, c0: e.pos.col, r1: e.pos.row, c1: e.pos.col };
          await host.exec({
            cmd: 'batch',
            cmds: [
              { cmd: 'setInputs', sheet: target, cells: [[e.pos.row, e.pos.col, e.text]] },
              { cmd: 'format', sheet: target, range: at, patch: { numFmt: e.numFmt } },
            ],
          });
        } else {
          await host.setInputs(target, [[e.pos.row, e.pos.col, e.text]]);
        }
        await refreshInfo();
      } catch (err) {
        // Part of an array formula: the entry stays in the editor (Esc gives it up).
        if (String(err instanceof Error ? err.message : err).includes(ARRAY_PART)) {
          holdBar.current = true;
          setSelection(single(e.pos));
          setEdit(e);
          setBarText(e.text);
          if (inputRef.current) inputRef.current.value = e.text;
        }
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
      const text = initial ?? editableInput(await host.cell(sheet, pos.row, pos.col));
      setEdit({ pos, text, inPlace, sheet });
      setBarText(text);
      if (inputRef.current) inputRef.current.value = text;
      focusInput();
    },
    [host, selection, sheet, focusInput],
  );

  /**
   * Opens the cell editor on `pos` with a proposed formula and `from..to` of it selected
   * (auto total: the range to adjust; insert function: the caret between the parentheses).
   * Enter commits, Esc cancels, as for any edit.
   */
  const proposeFormula = useCallback(
    (pos: CellPos, text: string, from: number, to: number, numFmt?: string) => {
      setSelection(single(pos));
      setExtra([]);
      setEdit({ pos, text, inPlace: true, ...(numFmt ? { numFmt } : {}) });
      setBarText(text);
      lastInput.current = 'cell';
      if (inputRef.current) inputRef.current.value = text;
      focusInput();
      inputRef.current?.setSelectionRange(from, to);
      requestAnimationFrame(() => inputRef.current?.setSelectionRange(from, to));
    },
    [focusInput],
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
    [reveal, setChart],
  );

  // The latest `select` (it changes with the scroll position), the cell editor and whether a
  // cell is being edited, for the auto total commands, which are built once.
  const [autoSumLink] = useState(() => new AutoSumLink<Selection>());
  useEffect(() => {
    autoSumLink.attach(select, proposeFormula, edit !== null);
  });

  /**
   * Auto total (Σ, Alt+=): the engine plans it (`autoSum`, read only) and the plan runs as one
   * batch — one undo step. One cell: the formula is proposed in the cell editor instead.
   */
  const runAutoSum = useCallback(
    async (s: SheetEditorState, fn: AutoSumFunction) => {
      if (autoSumLink.editing()) return;
      const plan = await s.host.exec<AutoSumPlan>({
        cmd: 'autoSum',
        sheet: s.sheet,
        range: toRange(rangeOf(s.selection)),
        function: fn,
      });
      if (plan.mode === 'edit') {
        autoSumLink.propose(
          { row: plan.row, col: plan.col },
          plan.text,
          plan.selStart,
          plan.selEnd,
          plan.numFmt ?? undefined,
        );
        return;
      }
      const cmd = autoSumCommand(plan, s.sheet, t('tableDesign.totalLabel'));
      if (plan.mode === 'none' || !cmd) {
        setStatus(t('autoSum.nothing'));
        return;
      }
      if ((await exec(cmd)) === null) return;
      const to = plan.select;
      autoSumLink.select(span({ row: to.r0, col: to.c0 }, { row: to.r1, col: to.c1 }));
    },
    [exec, t, autoSumLink],
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

  // Dictation (CORE-LANG-002): into the active cell editor at its caret (an in-place edit of
  // the active cell opens when none is). The target reads the editor through refs.
  const dictLive = useRef({ edit, selection, sheet, host });
  useEffect(() => {
    dictLive.current = { edit, selection, sheet, host };
  });
  const dictationTarget = useCallback(() => {
    const input = () => (lastInput.current === 'bar' ? barRef.current : inputRef.current);
    const set = (text: string, caret: number, pos?: CellPos) => {
      const live = dictLive.current;
      const next: EditState = live.edit
        ? { ...live.edit, text }
        : { pos: pos ?? live.selection.active, text, inPlace: true };
      live.edit = next;
      setEdit(next);
      setBarText(text);
      if (inputRef.current) inputRef.current.value = text;
      if (barRef.current) barRef.current.value = text;
      const el = input();
      el?.setSelectionRange(caret, caret);
      requestAnimationFrame(() => el?.setSelectionRange(caret, caret));
    };
    return sheetDictationTarget({
      editing: () => {
        const el = input();
        if (!dictLive.current.edit || !el) return null;
        const text = el.value;
        return {
          text,
          start: el.selectionStart ?? text.length,
          end: el.selectionEnd ?? text.length,
        };
      },
      begin: async () => {
        const { host: h, sheet: sh, selection: sel } = dictLive.current;
        if (!h) return null;
        const pos = sel.active;
        const c = await h.cell(sh, pos.row, pos.col);
        lastInput.current = 'cell';
        set(c.input, c.input.length, pos);
        return { text: c.input, start: c.input.length, end: c.input.length };
      },
      set: (text, caret) => set(text, caret),
    });
  }, []);
  const dictation = useDictation(platform.dictation, dictationTarget);
  const [dictationLink] = useState(() => new DictationLink());
  useEffect(() => {
    dictationLink.attach(dictation);
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

  // Map chart boundary datasets found next to the app (CORE-CHART-006), registered once per engine.
  // The engine directly, not the recorded host: this is no user action.
  useEffect(() => {
    if (platform.sheet) void registerMapDatasets(platform.sheet, platform.mapDatasets);
  }, [platform.sheet, platform.mapDatasets]);

  // Thesaurus (CORE-THES-001): the pane on the word of the active cell (a cell holding one word);
  // a replacement writes the cell (one undoable edit) in the word's capitalisation.
  const thesaurus = useThesaurus(platform.thesaurus);
  const [thesaurusLink] = useState(() => new ThesaurusLink());
  useEffect(() => {
    thesaurusLink.attach(thesaurus);
  });
  const beginThesaurus = useCallback(
    async (st: SheetEditorState) => {
      const { row, col } = st.selection.active;
      const at = { sheet: st.sheet, row, col };
      const cell = await st.host.cell(at.sheet, row, col);
      const word = cellWord(cell.input, cell.display);
      thesaurusLink.open({
        target: {
          word: word ?? '',
          ...(word
            ? {
                replace: async (text: string, expected: string) => {
                  const now = await st.host.cell(at.sheet, at.row, at.col);
                  if (cellWord(now.input, now.display) !== expected) return false;
                  await st.host.setInputs(at.sheet, [[at.row, at.col, asTextInput(text)]]);
                  await refreshInfo();
                  return true;
                },
              }
            : {}),
        },
      });
    },
    [refreshInfo, thesaurusLink],
  );

  // BlueAI (CORE-AI-001): a task pane; its features read the selection of the moment they run.
  const ai = useBlueAi(platform.ai, platform.clipboard);
  const [aiLink] = useState(() => new AiLink());
  const [aiTarget] = useState(() => new SheetAiTargetLink());
  useEffect(() => {
    aiLink.attach(ai);
  });
  const beginAi = useCallback(
    (_st: SheetEditorState, feature?: string) =>
      aiLink.open({
        features: sheetAiFeatures({
          t: (key, options) => t(key, options ?? {}),
          target: () => aiTarget.get(),
          changed: refreshInfo,
        }),
        feature,
      }),
    [aiLink, aiTarget, refreshInfo, t],
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
    setCopyMark(null);
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
      setCopyMark({ sheet, range: r, cut });
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
    // Cut cells move once: their mark ends with the paste (copied ones stay marked).
    if (lastCut) setCopyMark(null);
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

  /** Enter with copied cells marked: paste them once and end the mark. */
  const enterPaste = useCallback(async () => {
    if (!(await ownPaste())) await paste();
    setCopyMark(null);
  }, [ownPaste, paste]);

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

  /** Colours and border the Home split buttons apply: the ones picked last. */
  const [lastUsed, setLastUsed] = useState({ font: 'FF0000', fill: 'FFFF00', border: 'bottom' });
  const setLast = useCallback(
    (patch: Partial<{ font: string; fill: string; border: string }>) =>
      setLastUsed((l) => ({ ...l, ...patch })),
    [],
  );
  /** A border of the borders menu on every range of the selection (one undo step). */
  const applyBorder = useCallback(
    (id: string) => {
      const specs = BORDER_MENU[id] ?? [];
      const cmds: SheetCommand[] = allRanges.flatMap((r) =>
        specs.map((spec) => ({ cmd: 'borders' as const, sheet, range: toRange(r), spec })),
      );
      if (!cmds.length) return Promise.resolve(null);
      return exec(cmds.length === 1 ? cmds[0]! : { cmd: 'batch', cmds });
    },
    [allRanges, sheet, exec],
  );
  /** The split buttons: the last colour / border again. */
  const applyLast = useCallback(
    (which: 'font' | 'fill' | 'border') => {
      if (which === 'border') return applyBorder(lastUsed.border);
      return formatSel(
        which === 'font' ? { fontColor: lastUsed.font } : { fillColor: lastUsed.fill },
      );
    },
    [applyBorder, formatSel, lastUsed],
  );
  /**
   * Format painter button: on / off; a second click soon after the first (a double click)
   * keeps it on for several targets until Esc or another click.
   */
  const togglePainter = useCallback(
    (sel: Selection) => {
      const now = Date.now();
      if (painter && !painterKeep.sticky && now - painterKeep.at < 450) {
        setPainterKeep({ sticky: true, at: painterKeep.at });
        return;
      }
      setPainterKeep({ sticky: false, at: now });
      setPainter((p) => (p ? null : rangeOf(sel)));
    },
    [painter, painterKeep],
  );
  /**
   * A table over `r`; without names in its first row, a row of names (Column1, …) is put
   * above the data first, all in one undo step.
   */
  const createTable = useCallback(
    async (r: Range, headers: boolean) => {
      if (!host) return;
      if (headers) {
        await exec({ cmd: 'createTable', sheet, range: toRange(r) });
        return;
      }
      const since = (await host.exec<{ history: number } | null>({ cmd: 'historyLen' }))?.history;
      await exec({
        cmd: 'shiftCells',
        sheet,
        range: { r0: r.r0, c0: r.c0, r1: r.r0, c1: r.c1 },
        down: true,
        insert: true,
      });
      await host.setInputs(
        sheet,
        Array.from(
          { length: r.c1 - r.c0 + 1 },
          (_, i) =>
            [r.r0, r.c0 + i, t('sheetEditor.createTable.column', { n: i + 1 })] as [
              number,
              number,
              string,
            ],
        ),
      );
      await exec({
        cmd: 'createTable',
        sheet,
        range: { r0: r.r0, c0: r.c0, r1: r.r1 + 1, c1: r.c1 },
      });
      if (since !== undefined) await host.exec({ cmd: 'mergeHistory', since });
      await refreshInfo();
    },
    [host, exec, sheet, t, refreshInfo],
  );
  const focusBox = useCallback((testId: string) => {
    document.querySelector<HTMLElement>(`[data-testid="${testId}"]`)?.focus();
  }, []);

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
  // View › Window (CORE-UI-007): the other windows, side by side, New Window.
  const windowTools = useWindowTools({
    platform,
    app: 'sheet',
    title: fileName,
    scroller: () => scrollerRef.current,
    getFile: async () => {
      if (!host || !info) return null;
      return {
        bytes: await host.save('xlsx', sheet),
        name: `${(fileName || info.name).replace(/\.[^.]+$/, '')}.xlsx`,
      };
    },
    notify: setStatus,
    t,
  });
  const {
    newWindow: windowNew,
    arrangeAll: windowArrange,
    sideBySide: windowSideBySide,
    toggleSynchronous: windowToggleSync,
    resetPosition: windowReset,
    synchronous: windowSync,
  } = windowTools;
  const windowPaired = !!windowTools.paired;
  const statusBar = useStatusBarPrefs(platform.shell, 'sheet');

  /** Split at the active cell (again: remove the split). A frozen sheet is unfrozen first. */
  const toggleSplit = useCallback(
    async (s: SheetEditorState) => {
      if (split) {
        setSplit(null);
        return;
      }
      if (!host) return;
      const frozen = {
        rows: info?.sheets[s.sheet]?.frozenRows ?? 0,
        cols: info?.sheets[s.sheet]?.frozenCols ?? 0,
      };
      if (splitNeedsUnfreeze(frozen)) {
        await exec({ cmd: 'freeze', sheet: s.sheet, rows: 0, cols: 0 });
        setStatus(t('windows.splitUnfroze'));
      }
      const { row, col } = s.selection.active;
      const [rect, origin] = await Promise.all([
        host.rangeRect(view, row, col, row, col),
        host.rangeRect({ ...view, scrollX: 0, scrollY: 0 }, 0, 0, 0, 0),
      ]);
      setSplit(
        splitAtCell({
          cell: { row, col },
          rect,
          headers: { x: origin?.x ?? 0, y: origin?.y ?? 0 },
          view,
          zoom,
        }),
      );
    },
    [split, host, info, exec, view, zoom, t],
  );
  // Another sheet or workbook: the split belonged to the one shown before.
  const splitScope = `${loadSeq}:${sheet}`;
  const [splitFor, setSplitFor] = useState(splitScope);
  if (splitFor !== splitScope) {
    setSplitFor(splitScope);
    setSplit(null);
  }

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
          // XML Spreadsheet 2003, Flat OPC, HTML tables (CORE-FMT-003).
          ...webOpenExtensions('sheet'),
          // SYLK, DIF, formatted text, dBASE tables (CORE-FMT-004).
          ...TABULAR_OPEN_EXTENSIONS,
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
      fileName.replace(
        /\.(xlsx|xlsm|xltx|xltm|csv|xml|html?|mht|mhtml|slk|sylk|dif|prn|dbf)$/i,
        '',
      ) || t('sheetEditor.untitled');
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
    // The workbook as HTML tables, a single-file web page or Flat OPC (CORE-FMT-003).
    for (const f of webFormats('sheet')) {
      // EPUB is a format of documents only.
      if (f.id === 'epub') continue;
      const format = f.id;
      add(
        WEB_FORMAT_COMMANDS[f.id],
        async (s) => {
          const bytes = await s.host.save(format, s.sheet);
          const saved = await platform.files.saveFile(bytes, `${baseName()}.${f.ext}`, f.mime);
          if (saved) setStatus(t('webFormats.saved', { name: saved }));
        },
        has,
      );
    }
    // The current sheet as SYLK, DIF or formatted text (CORE-FMT-004).
    for (const f of TABULAR_FORMATS) {
      add(
        f.command,
        async (s) => {
          const bytes = await s.host.save(f.id, s.sheet);
          const saved = await platform.files.saveFile(bytes, `${baseName()}.${f.ext}`, f.mime);
          if (saved) setStatus(t('tabularFormats.saved', { name: saved }));
        },
        has,
      );
    }
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
      'formulas.arrayEnter',
      async (s) => {
        const cell = await s.host.cell(s.sheet, s.selection.active.row, s.selection.active.col);
        const text = editableInput(cell);
        if (!text.startsWith('=')) return;
        await s.host.exec({ cmd: 'setArrayFormula', sheet: s.sheet, range: rng(s), formula: text });
        await refreshInfo();
      },
      has,
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
    add('edit.fillSeries', async (s) => {
      const r = rangeOf(s.selection);
      // A date in the first cell (shown as a date in the formula bar) proposes date steps.
      const first = await s.host.cell(s.sheet, r.r0, r.c0);
      setDialog({
        kind: 'series',
        firstIsDate: /^\d{1,4}[/.-]\d{1,2}[/.-]\d{1,4}/.test(first.input),
      });
    });
    add('edit.find', open({ kind: 'find', replace: false }), { shortcut: 'Ctrl+F' });
    add('edit.replace', open({ kind: 'find', replace: true }), { shortcut: 'Ctrl+H' });
    add('edit.goto', open({ kind: 'goto' }), { shortcut: 'Ctrl+G' });
    add('view.formulaBar', async () => setBarExpanded((x) => !x), {
      shortcut: 'Ctrl+Shift+U',
    });

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
    // Through the size list (8 … 72), like the font size box.
    add('format.fontGrow', (s) => formatSel({ fontSize: stepFontSize(s.fmt?.fontSize ?? 11, 1) }));
    add('format.fontShrink', (s) =>
      formatSel({ fontSize: stepFontSize(s.fmt?.fontSize ?? 11, -1) }),
    );
    // The boxes themselves apply; the commands put the focus in them.
    add('format.fontName', async () => focusBox('sheet-font-name'));
    add('format.fontSize', async () => focusBox('sheet-font-size'));
    add('format.fontTab', open({ kind: 'format', tab: 'font' }));
    add('format.fontColor', popup('fontColor', 'format.fontColor'));
    add('format.fillColor', popup('fillColor', 'format.fillColor'));
    // Split buttons: the colour / border used last (red text, yellow fill, bottom border
    // until another is picked).
    add('format.fontColorLast', () => applyLast('font'));
    add('format.fillColorLast', () => applyLast('fill'));
    add('format.borders', popup('borders', 'format.borders'));
    add('format.bordersLast', () => applyLast('border'));
    add('format.borderOutline', () => borderSel({ preset: 'outline', style: 'thin' }));
    add('format.borderNone', () => borderSel({ preset: 'none' }));
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
    add('format.orientation', popup('orientation', 'format.orientation'));
    add('format.wrap', (s) => formatSel({ wrap: !s.fmt?.wrap }), {
      isChecked: (s) => !!s.fmt?.wrap,
    });
    // An indent needs a left (or right / distributed) alignment: a General or centred cell
    // becomes left-aligned (§18.8.1 alignment/@indent).
    add('format.indentInc', (s) =>
      formatSel({
        indent: Math.min(250, (s.fmt?.indent ?? 0) + 1),
        ...(['left', 'right', 'distributed'].includes(s.fmt?.hAlign ?? '')
          ? {}
          : { hAlign: 'left' }),
      }),
    );
    add('format.indentDec', (s) => formatSel({ indent: Math.max(0, (s.fmt?.indent ?? 0) - 1) }));
    // Merging several filled cells keeps the upper-left value only: asked first.
    const merge = (center: boolean, across: boolean) => async (s: SheetEditorState) => {
      const range = rng(s);
      const st = await exec<RangeStats>({ cmd: 'rangeStats', sheet: s.sheet, ranges: [range] });
      if (st && st.count > 1) {
        setDialog({ kind: 'mergeWarn', center, across });
        return;
      }
      await exec({ cmd: 'merge', sheet: s.sheet, range, center, ...(across ? { across } : {}) });
    };
    add(
      'format.mergeCenter',
      (s) =>
        s.fmt?.merged
          ? exec({ cmd: 'unmerge', sheet: s.sheet, range: rng(s) })
          : merge(true, false)(s),
      { isChecked: (s) => !!s.fmt?.merged },
    );
    add('format.mergeAcross', merge(false, true));
    add('format.mergeCells', merge(false, false));
    add('format.unmerge', (s) => exec({ cmd: 'unmerge', sheet: s.sheet, range: rng(s) }));
    // Number
    const preset = (id: string, fallback: string) => () =>
      formatSel({ numFmt: presets.numberFormats.find((p) => p.id === id)?.code ?? fallback });
    add('format.numberFormat', popup('numFmt', 'format.numberFormat'));
    // The money button applies the accounting format (symbol and dash for zero aligned),
    // the comma style the same without the symbol.
    add(
      'format.currency',
      preset('accounting', '_-* #,##0 [$₫-42A]_-;-* #,##0 [$₫-42A]_-;_-* "-" [$₫-42A]_-;_-@_-'),
    );
    add('format.percent', preset('percent', '0%'), { shortcut: 'Ctrl+Shift+%' });
    add('format.comma', () => formatSel({ numFmt: COMMA_STYLE }));
    add('format.date', preset('date', 'dd/mm/yyyy'));
    // Ctrl+Shift+~ ! @ # $ ^ (formatKeyCommand): General, number with two decimals and
    // thousands, time with AM/PM, date with day, month and year, currency, scientific.
    add('format.numGeneral', () => formatSel({ numFmt: 'General' }));
    add('format.numNumber', () => formatSel({ numFmt: '#,##0.00' }));
    add('format.numTime', () => formatSel({ numFmt: 'h:mm AM/PM' }));
    add('format.numDate', () => formatSel({ numFmt: 'd-mmm-yy' }));
    add('format.numCurrency', () =>
      formatSel({ numFmt: decimal === ',' ? '#,##0.00 [$₫-42A]' : '[$$-409]#,##0.00' }),
    );
    add('format.numScientific', () => formatSel({ numFmt: '0.00E+00' }));
    // Increase / Decrease decimal: from the active cell's format (General: the decimals
    // it shows).
    const decimals = (dir: 1 | -1) => async (s: SheetEditorState) => {
      const code = s.fmt?.numFmt ?? 'General';
      const { row, col } = s.selection.active;
      const shown = shownDecimals((await s.host.cell(s.sheet, row, col)).display, decimal);
      await formatSel({ numFmt: stepDecimals(code, shown, dir) });
    };
    add('format.decimalInc', decimals(1));
    add('format.decimalDec', decimals(-1));
    // Styles
    add('format.cellStyles', popup('styles', 'format.cellStyles'));
    add('format.conditional', open({ kind: 'cf' }));
    add('format.conditionalManage', open({ kind: 'cfManage' }));
    add('format.clearConditional', (s) => exec({ cmd: 'clearCf', sheet: s.sheet, range: rng(s) }));
    add('format.cells', open({ kind: 'format' }), { shortcut: 'Ctrl+1' });
    // A second click soon after the first (a double click) keeps the painter on for
    // several targets until Esc or another click on the button.
    add('format.painter', async (s) => togglePainter(s.selection), {
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
    // Clear › All: contents and formats, one undo step.
    add('edit.clearAll', async (s) => {
      const since = (await s.host.exec<{ history: number } | null>({ cmd: 'historyLen' }))?.history;
      await clearSelection();
      const cmds: SheetCommand[] = s.ranges.map((r) => ({
        cmd: 'clearFormats',
        sheet: s.sheet,
        range: toRange(r),
      }));
      await exec(cmds.length === 1 ? cmds[0]! : { cmd: 'batch', cmds });
      if (since !== undefined) await s.host.exec({ cmd: 'mergeHistory', since });
    });
    add('edit.clearMenu', popup('clear', 'edit.clearMenu'));
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
    registerDictationCommands(reg, {
      enabled: (st) => st.hasWorkbook,
      current: () => dictationLink.current(),
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
    reg.register({
      id: 'review.thesaurus',
      shortcut: 'Shift+F7',
      isEnabled: (st) => st.hasWorkbook,
      run: beginThesaurus,
    });
    registerAiCommands(reg, {
      enabled: (st) => st.hasWorkbook,
      features: SHEET_AI_COMMANDS,
      isOff: () => aiLink.off,
      begin: beginAi,
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
    // Ctrl++ / Ctrl+-: whole rows or columns selected are inserted / deleted at once;
    // other cells ask how to shift their neighbours.
    const cellsCmd = (insert: boolean) => async (s: SheetEditorState) => {
      const r = rangeOf(s.selection);
      const last = { rows: MAX_ROW, cols: MAX_COL };
      if (r.c0 === 0 && r.c1 >= last.cols) return structural(true, !insert)(s);
      if (r.r0 === 0 && r.r1 >= last.rows) return structural(false, !insert)(s);
      setDialog({ kind: 'cells', insert });
    };
    add('cells.insert', cellsCmd(true));
    add('cells.delete', cellsCmd(false));
    add('cells.insertMenu', popup('insert', 'cells.insertMenu'));
    add('cells.deleteMenu', popup('delete', 'cells.deleteMenu'));
    add('cells.formatMenu', popup('cellFormat', 'cells.formatMenu'));
    add('format.mergeMenu', popup('merge', 'format.mergeMenu'));
    // Format › Column width / Row height: the size of the active cell's column / row.
    const sizeDialog = (rows: boolean) => async (s: SheetEditorState) => {
      const { row, col } = s.selection.active;
      const r = await s.host.exec<{ size: number }>({
        cmd: 'sizeOf',
        sheet: s.sheet,
        rows,
        index: rows ? row : col,
      });
      setDialog({ kind: 'size', rows, initial: r.size });
    };
    add('cells.colWidth', sizeDialog(false));
    add('cells.rowHeight', sizeDialog(true));
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
    // A new sheet goes after the active one (+ button), or before it (Shift+F11), as one
    // undo step.
    const newSheet = (after: boolean) => async (s: SheetEditorState) => {
      const since = (await s.host.exec<{ history: number } | null>({ cmd: 'historyLen' }))?.history;
      const i = await s.host.addSheet();
      const to = after ? s.sheet + 1 : s.sheet;
      if (to !== i && since !== undefined) {
        await s.host.exec({ cmd: 'moveSheet', from: i, to });
        await s.host.exec({ cmd: 'mergeHistory', since });
      }
      await refreshInfo();
      setSheet(to);
      setSelection(single({ row: 0, col: 0 }));
    };
    add('sheet.add', newSheet(true));
    add('sheet.insertBefore', newSheet(false));
    // Ctrl+PgUp / Ctrl+PgDn: the previous / next visible sheet.
    const stepSheet = (by: -1 | 1) => async (s: SheetEditorState) => {
      const sheets = info?.sheets ?? [];
      for (let i = s.sheet + by; i >= 0 && i < sheets.length; i += by) {
        if (sheets[i]?.hidden) continue;
        setSheet(i);
        setSelection(single({ row: 0, col: 0 }));
        setChart(null);
        return;
      }
    };
    add('sheet.previous', stepSheet(-1));
    add('sheet.next', stepSheet(1));
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
    // WCAG 2.5.7: reorder sheets without dragging their tabs.
    const moveSheet = (by: -1 | 1) => async (s: SheetEditorState) => {
      const to = s.sheet + by;
      const r = await exec({ cmd: 'moveSheet', from: s.sheet, to });
      if (r !== null) setSheet(to);
    };
    add('sheet.moveLeft', moveSheet(-1), { isEnabled: (s) => s.sheet > 0 });
    add('sheet.moveRight', moveSheet(1), {
      isEnabled: (s) => s.sheet + 1 < (info?.sheets.length ?? 0),
    });

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
    // Macro VBA → BlueScript draft (CORE-SCRIPT-003): VBA is only read, never run.
    add(
      'script.convertVba',
      async () => {
        setScriptOpen(true);
        setVbaRequest((n) => n + 1);
      },
      has,
    );
    // Macro source of the open file, read-only (CORE-VBA-002); VBA is never run.
    add('script.viewMacros', async () => showMacros(), has);
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
    // Alt+F1: a clustered column chart of the selection's data on this sheet, selected.
    add(
      'insert.chartDefault',
      async (s) => {
        const res = await exec<{ index: number }>({
          cmd: 'insertChart',
          sheet: s.sheet,
          range: toRange(rangeOf(s.selection)),
          kind: 'column',
          texts: layoutTexts(t),
        });
        if (res) setChart(res.index);
      },
      { shortcut: 'Alt+F1', ...has },
    );
    add('insert.chartRecommended', async () => setRecommendOpen(true), has);
    // Quick Analysis of a multi-cell selection (CORE-DESIGN-001).
    add('format.quickAnalysis', async () => setQuickOpen((o) => !o), {
      isEnabled: (s) => {
        const sel = rangeOf(s.selection);
        return s.hasWorkbook && (sel.r0 !== sel.r1 || sel.c0 !== sel.c1);
      },
    });
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
    // SmartArt Design tab (CORE-SMART-004), convert to shapes included.
    registerSmartArtTools(
      {
        register: (c) =>
          add(c.id, async (st) => c.run(st), { isEnabled: c.isEnabled, isChecked: c.isChecked }),
      },
      () => smartApi,
      (s: SheetEditorState) => s.smartArt !== null,
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
    add('insert.icon', async () => setIconsOpen(true), has);
    add('insert.pivot', open({ kind: 'pivot' }), has);
    // Ctrl+T / Ctrl+L: the data around a single cell, then the create-table question
    // (range, "the first row holds the names" guessed from the cells).
    add(
      'insert.table',
      async (s) => {
        let r = rangeOf(s.selection);
        if (cellsOf(r) === 1) {
          const reg = await exec<SheetRange>({ cmd: 'region', sheet: s.sheet, range: toRange(r) });
          if (reg) r = { r0: reg.r0, c0: reg.c0, r1: reg.r1, c1: reg.c1 };
        }
        const headers = await guessHeaders(s.host, s.sheet, r);
        setDialog({ kind: 'table', range: rangeA1(r), headers });
      },
      { shortcut: 'Ctrl+T', ...has },
    );
    registerSparklineCommands(add, {
      range: rng,
      exec: (cmd) => exec(cmd),
      openInsert: () => setDialog({ kind: 'sparklineInsert' }),
      openOptions: () => setDialog({ kind: 'sparklineOptions' }),
      openEditData: (mode) =>
        setDialog(
          mode === 'single' ? { kind: 'sparklineData', single: true } : { kind: 'sparklineData' },
        ),
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
    // In a pivot table: slicers over its fields; otherwise for the table column of the cell.
    add(
      'insert.slicer',
      async (s) => {
        if (activePivot) {
          setDialog({ kind: 'frameInsert', pivot: activePivot, frames: 'slicer' });
          return;
        }
        await exec({
          cmd: 'insertSlicer',
          sheet: s.sheet,
          row: s.selection.active.row,
          col: s.selection.active.col,
        });
      },
      has,
    );
    add(
      'insert.timeline',
      async () => {
        if (activePivot) setDialog({ kind: 'frameInsert', pivot: activePivot, frames: 'timeline' });
      },
      { isEnabled: (s) => s.pivot !== null && !!activePivot && !activePivot.model },
    );
    add('pivot.showDetails', (s) => drillDown(s.selection.active.row, s.selection.active.col), {
      isEnabled: (s) => {
        const a = activePivot?.dataArea;
        const at = s.selection.active;
        return (
          s.pivot !== null &&
          !!a &&
          !activePivot?.model &&
          at.row >= a.r0 &&
          at.row <= a.r1 &&
          at.col >= a.c0 &&
          at.col <= a.c1
        );
      },
    });
    add(
      'slicer.settings',
      async () => {
        if (frameSel)
          setDialog({ kind: 'frameOptions', frames: frameSel.kind, index: frameSel.index });
      },
      { isEnabled: () => frameSel !== null },
    );
    add(
      'slicer.delete',
      async () => {
        if (frameSel) await deleteFrame(frameSel.kind, frameSel.index);
      },
      { isEnabled: () => frameSel !== null },
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
        openBackground: (d) => setBgTarget(d),
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
    // Add chart element choices and the Format chart pane (CORE-CHART-005).
    registerChartElementCommands<SheetEditorState>(
      (c) =>
        add(c.id, async (s) => c.run(s), {
          isEnabled: (s) => s.chart !== null && c.isEnabled(s),
          ...(c.isChecked ? { isChecked: c.isChecked } : {}),
        }),
      {
        t,
        info: (s) => (s.chart !== null ? s.chartElements : null),
        apply: (patch, s) => {
          if (s.chart !== null)
            void exec({ cmd: 'chartDesign', sheet: s.sheet, index: s.chart, elements: patch });
        },
        openPane: () => setChartFormatOpen((v) => !v),
      },
    );
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
    // A→Z / Z→A: a single cell sorts its block of data, a range itself; the first row is
    // kept in place when it looks like labels. One column of a wider block asks first
    // whether to expand the selection (the default) or sort that column alone.
    const quickSort = (descending: boolean) => async (s: SheetEditorState) => {
      const r = rangeOf(s.selection);
      const g = await query<SortRegion>({ cmd: 'sortRegion', sheet: s.sheet, range: toRange(r) });
      if (!g) return;
      const col = s.selection.active.col;
      if (r.c0 === r.c1 && r.r0 !== r.r1 && (g.around.c0 < r.c0 || g.around.c1 > r.c1)) {
        setDialog({ kind: 'sortWarning', descending, col, range: toRange(r) });
        return;
      }
      await exec({
        cmd: 'sort',
        sheet: s.sheet,
        range: g.range,
        keys: [{ col, descending }],
        hasHeader: g.hasHeader,
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
    add('data.editLinks', open({ kind: 'editLinks' }), has);
    add('data.modelTables', () => openModel('tables'));
    add('data.modelRelationships', () => openModel('relationships'));
    add('data.modelMeasures', () => openModel('measures'));
    add('data.modelPivot', () => modelPivot());
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
    // Alt+Shift+→ / ←: whole columns selected group the columns, anything else the rows.
    const wholeCols = (s: SheetEditorState) => {
      const r = rangeOf(s.selection);
      return r.r0 === 0 && r.r1 >= Math.max(info?.sheets[s.sheet]?.lastRow ?? 0, 999);
    };
    add('data.group', (s) => outlineGroup(!wholeCols(s), false)(s), {
      shortcut: 'Alt+Shift+ArrowRight',
    });
    add('data.ungroup', (s) => outlineGroup(!wholeCols(s), true)(s), {
      shortcut: 'Alt+Shift+ArrowLeft',
    });
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
    // Show / Hide Detail: the row group at the active cell, else its column group
    // (columns first when whole columns are selected).
    const detail = (show: boolean) => async (s: SheetEditorState) => {
      const a = s.selection.active;
      const order = wholeCols(s) ? [false, true] : [true, false];
      for (const rows of order) {
        const r = await exec<{ changed: boolean }>({
          cmd: 'outlineDetail',
          sheet: s.sheet,
          rows,
          at: rows ? a.row : a.col,
          show,
        });
        if (r?.changed) return;
      }
    };
    add('data.showDetail', detail(true));
    add('data.hideDetail', detail(false));
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
    // Freezing a split sheet removes the split (frozen panes take its place).
    const freezeTop = freeze(1, 0);
    const freezeFirst = freeze(0, 1);
    add('view.freezeTopRow', (s) => {
      setSplit(null);
      return freezeTop(s);
    });
    add('view.freezeFirstCol', (s) => {
      setSplit(null);
      return freezeFirst(s);
    });
    add('view.freezePanes', (s) => {
      // Frozen: the button unfreezes (it reads "Unfreeze panes" then).
      if (!split && (summary?.frozenRows ?? 0) + (summary?.frozenCols ?? 0) > 0)
        return exec({ cmd: 'freeze', sheet: s.sheet, rows: 0, cols: 0 });
      // On a split sheet the split becomes frozen panes at the cell it was made at.
      const at = split
        ? freezeFromSplit(split)
        : { rows: s.selection.active.row, cols: s.selection.active.col };
      if (split) {
        setSplit(null);
        setStatus(t('windows.splitToFreeze'));
      }
      return exec({ cmd: 'freeze', sheet: s.sheet, ...at });
    });
    // Window (CORE-UI-007): second window, split panes, side by side.
    add('window.new', () => windowNew());
    add('window.arrangeAll', () => windowArrange());
    add('window.split', async (s) => toggleSplit(s), {
      isChecked: () => !!split,
      isEnabled: () => !!info && viewMode !== 'pageLayout',
    });
    add('window.sideBySide', async () => windowSideBySide(), { isChecked: () => windowPaired });
    add('window.syncScroll', async () => windowToggleSync(), {
      isEnabled: () => windowPaired,
      isChecked: () => windowPaired && windowSync,
    });
    add('window.resetPosition', async () => windowReset(), { isEnabled: () => windowPaired });
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
    add('view.outlineSymbols', () => toggleOutlineSymbols(), {
      shortcut: 'Ctrl+8',
      isChecked: () => sheetViews.state?.outlineSymbols ?? true,
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
    // Auto total (Σ, Alt+=) and the other functions of its menu.
    for (const fn of AUTO_SUM_FUNCTIONS)
      add(
        AUTO_SUM_COMMANDS[fn],
        (s) => runAutoSum(s, fn),
        fn === 'sum' ? { shortcut: 'Alt+=', ...has } : has,
      );
    add('formula.autoSumMenu', popup('autoSum', 'formula.autoSumMenu'), has);
    add('formula.insertFunction', async () => setFnDialog(true), { shortcut: 'Shift+F3', ...has });
    // Defined names.
    add('formulas.nameManager', open({ kind: 'nameManager' }), { shortcut: 'Ctrl+F3', ...has });
    add('formulas.defineName', open({ kind: 'defineName' }), has);
    add(
      'formulas.createFromSelection',
      async (s) => {
        // Ticked at first: the top row / left column when its first label cell holds text.
        const r = rangeOf(s.selection);
        const text = async (row: number, col: number) => {
          const c = await s.host.cell(s.sheet, row, col);
          return c.input !== '' && !c.input.startsWith('=') && Number.isNaN(Number(c.display));
        };
        const top = r.c1 > r.c0 ? await text(r.r0, r.c0 + 1) : await text(r.r0, r.c0);
        const left = r.r1 > r.r0 ? await text(r.r0 + 1, r.c0) : false;
        setDialog({ kind: 'createNames', guess: { top, left: left && r.c1 > r.c0 } });
      },
      { shortcut: 'Ctrl+Shift+F3', ...has },
    );
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
    borderSel,
    applyLast,
    togglePainter,
    focusBox,
    decimal,
    deleteFrame,
    drillDown,
    frameSel,
    showMacros,
    openSmartArt,
    smartApi,
    readAloudLink,
    dictationLink,
    beginTranslate,
    beginAi,
    beginThesaurus,
    aiLink,
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
    query,
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
    toggleOutlineSymbols,
    sheetViews.state,
    showingFormulas,
    split,
    toggleSplit,
    windowNew,
    windowArrange,
    windowSideBySide,
    windowToggleSync,
    windowReset,
    windowPaired,
    windowSync,
    recorder,
    queryFrom,
    queryRefresh,
    toggleQueries,
    queriesOpen,
    openModel,
    modelPivot,
    runAutoSum,
  ]);

  // Add-ins of the organisation catalogue (CORE-ADDIN-001): ribbon commands, task panes.
  // Their calls go through the BlueScript API (ADR-0012) on the host the UI uses.
  const selRange = rangeOf(selection);
  const addins = useAddins({
    app: 'sheet',
    catalog: platform.addins,
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
    selection: (h) =>
      sheetSelection(h, {
        sheet: sheetNow.current,
        sheetName: sheetName ?? '',
        range: rangeOf(selectionNow.current),
        active: selectionNow.current.active,
      }),
    selectionKey: `${sheet}:${selRange.r0},${selRange.c0},${selRange.r1},${selRange.c1}:${selection.active.row},${selection.active.col}`,
    documentKey: revision,
  });
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
            chartElements: chart !== null ? (chartInfo?.elements ?? null) : null,
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
      chartInfo,
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
  useEffect(() => {
    aiTarget.set(
      state?.hasWorkbook
        ? {
            host: state.host,
            sheet: state.sheet,
            range: rangeOf(state.selection),
            active: state.selection.active,
          }
        : null,
    );
  }, [aiTarget, state]);
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

  /** Whole columns / rows of the selection, as a click on their headers selects them. */
  const wholeColumns = (s: Selection): Selection => {
    const r = rangeOf(s);
    const last = Math.max(info?.sheets[sheet]?.lastRow ?? 0, 999);
    return { anchor: { row: 0, col: r.c0 }, focus: { row: last, col: r.c1 }, active: s.active };
  };
  const wholeRows = (s: Selection): Selection => {
    const r = rangeOf(s);
    const last = Math.max(info?.sheets[sheet]?.lastCol ?? 0, 25);
    return { anchor: { row: r.r0, col: 0 }, focus: { row: r.r1, col: last }, active: s.active };
  };

  /**
   * Ready-mode movement and selection keys of the reference spreadsheet: Ctrl+arrows to the
   * edge of the data (Shift extends), Home, Ctrl+Home, Ctrl+End, Ctrl+Space / Shift+Space,
   * Ctrl+A (current region, then the sheet), Alt+PgUp / Alt+PgDn. True when handled.
   */
  const navigate = (e: ReactKeyboardEvent<HTMLElement>): boolean => {
    const ctrl = e.ctrlKey || e.metaKey;
    const key = e.key;
    const last = info?.sheets[sheet];
    // Moves the focus (Shift) or the active cell to `p`.
    const to = (p: CellPos) =>
      select(
        e.shiftKey ? { anchor: selection.anchor, focus: p, active: selection.active } : single(p),
      );
    if (ctrl && !e.altKey && key.toLowerCase() === 'a' && !e.shiftKey) {
      e.preventDefault();
      const whole = span(
        { row: 0, col: 0 },
        { row: Math.max(last?.lastRow ?? 0, 999), col: Math.max(last?.lastCol ?? 0, 25) },
      );
      const cur = rangeOf(selection);
      const a = selection.active;
      void query<SheetRange>({
        cmd: 'region',
        sheet,
        range: { r0: a.row, c0: a.col, r1: a.row, c1: a.col },
      }).then((g) => {
        const lone = !g || (g.r0 === g.r1 && g.c0 === g.c1);
        const same =
          !!g && g.r0 === cur.r0 && g.c0 === cur.c0 && g.r1 === cur.r1 && g.c1 === cur.c1;
        if (lone || same) select({ ...whole, active: a });
        else
          select({ anchor: { row: g.r0, col: g.c0 }, focus: { row: g.r1, col: g.c1 }, active: a });
      });
      return true;
    }
    if (key === ' ' && !e.altKey && ctrl !== e.shiftKey) {
      e.preventDefault();
      select(ctrl ? wholeColumns(selection) : wholeRows(selection));
      return true;
    }
    if (key === 'Home' && !e.altKey) {
      e.preventDefault();
      const from = e.shiftKey ? selection.focus : selection.active;
      // Ctrl+Home: the first cell of the scrolling pane (below / right of frozen panes).
      to(
        ctrl
          ? { row: summary?.frozenRows ?? 0, col: summary?.frozenCols ?? 0 }
          : { row: from.row, col: 0 },
      );
      return true;
    }
    if (key === 'End' && ctrl && !e.altKey) {
      e.preventDefault();
      to({ row: last?.lastRow ?? 0, col: last?.lastCol ?? 0 });
      return true;
    }
    if ((key === 'PageDown' || key === 'PageUp') && e.altKey && !ctrl) {
      e.preventDefault();
      const cols = Math.max(1, Math.floor(size.w / (48 * scale)));
      select(move(selection, key === 'PageDown' ? 'right' : 'left', e.shiftKey, cols, hidden));
      return true;
    }
    const dir = navKey[key];
    if (dir && ctrl && !e.altKey) {
      e.preventDefault();
      const from = e.shiftKey ? selection.focus : selection.active;
      void query<CellPos>({ cmd: 'dataEdge', sheet, row: from.row, col: from.col, dir }).then(
        (p) => {
          if (p) to({ row: p.row, col: p.col });
        },
      );
      return true;
    }
    return false;
  };

  const tabFrom = (s: Selection, back: boolean): Selection => {
    const next = advance(s, back ? 'left' : 'right');
    const one = cellsOf(rangeOf(s)) === 1;
    const run = tabRun.current;
    const continued = run && run.at.row === s.active.row && run.at.col === s.active.col;
    tabRun.current = one
      ? { startCol: continued ? run.startCol : s.active.col, at: next.active }
      : null;
    return next;
  };
  const enterFrom = (s: Selection, up: boolean): Selection => {
    const run = tabRun.current;
    tabRun.current = null;
    const one = cellsOf(rangeOf(s)) === 1;
    if (!one) return advance(s, up ? 'up' : 'down');
    const next = move(s, up ? 'up' : 'down', false, 1, hidden);
    if (!up && run && run.at.row === s.active.row && run.at.col === s.active.col)
      return single({ row: next.active.row, col: run.startCol });
    return next;
  };

  /** Puts `text` in the edit (both editors) with the caret / selection `from..to`. */
  const setEditText = (text: string, from: number, to = from, patch?: Partial<EditState>) => {
    if (!edit) return;
    setEdit({ ...edit, ...patch, text });
    setBarText(text);
    const el = lastInput.current === 'bar' ? barRef.current : inputRef.current;
    if (inputRef.current) inputRef.current.value = text;
    if (barRef.current && barRef.current !== el) barRef.current.value = text;
    el?.setSelectionRange(from, to);
    requestAnimationFrame(() => el?.setSelectionRange(from, to));
    setCaret(to);
  };

  /**
   * Point mode: an arrow key (or a click) while the caret can take a reference puts the
   * reference of the moved cell there; more arrows move it, Shift extends it.
   */
  const pointMove = (dir: Direction | null, extend: boolean, at?: CellPos) => {
    if (!edit) return;
    const el = lastInput.current === 'bar' ? barRef.current : inputRef.current;
    const p = picking.current;
    const caretAt = el?.selectionStart ?? edit.text.length;
    const base = p ?? { start: caretAt, end: caretAt, anchor: edit.pos, focus: edit.pos };
    let anchor = base.anchor;
    let focus = base.focus;
    const live = !!p && !p.pending;
    if (at) {
      if (extend && live) focus = at;
      else anchor = focus = at;
    } else if (dir) {
      const s = move({ anchor, focus, active: focus }, dir, extend && live, 1, hidden);
      if (extend && live) focus = s.focus;
      else anchor = focus = s.focus;
    }
    // A reference to another sheet than the edited cell's carries the sheet name.
    const home = edit.sheet ?? sheet;
    const ref = refText(
      {
        r0: Math.min(anchor.row, focus.row),
        c0: Math.min(anchor.col, focus.col),
        r1: Math.max(anchor.row, focus.row),
        c1: Math.max(anchor.col, focus.col),
      },
      sheet !== home ? info?.sheets[sheet]?.name : undefined,
    );
    const text = edit.text.slice(0, base.start) + ref + edit.text.slice(base.end);
    picking.current = { start: base.start, end: base.start + ref.length, anchor, focus, sheet };
    setEditText(text, base.start + ref.length);
  };

  /** Whether the caret of the formula being typed can take a picked reference now. */
  const pointable = (): boolean => {
    if (!edit) return false;
    const el = lastInput.current === 'bar' ? barRef.current : inputRef.current;
    return !!picking.current || canPoint(edit.text, el?.selectionStart ?? edit.text.length);
  };

  /** A sheet tab clicked while picking: `Sheet2!` goes into the formula, that sheet shows. */
  const pointSheet = (i: number) => {
    if (!edit) return;
    const el = lastInput.current === 'bar' ? barRef.current : inputRef.current;
    const p = picking.current;
    const caretAt = el?.selectionStart ?? edit.text.length;
    const start = p?.start ?? caretAt;
    const end = p?.end ?? caretAt;
    const home = edit.sheet ?? sheet;
    const name = info?.sheets[i]?.name ?? '';
    const prefix =
      i === home ? '' : refText({ r0: 0, c0: 0, r1: 0, c1: 0 }, name).replace(/A1$/, '');
    const text = edit.text.slice(0, start) + prefix + edit.text.slice(end);
    const a1 = { row: 0, col: 0 };
    picking.current = {
      start,
      end: start + prefix.length,
      anchor: a1,
      focus: a1,
      sheet: i,
      pending: true,
    };
    setEdit({ ...edit, text, sheet: home });
    setBarText(text);
    if (inputRef.current) inputRef.current.value = text;
    setSheet(i);
    setCaret(start + prefix.length);
    requestAnimationFrame(() => {
      el?.focus({ preventScroll: true });
      el?.setSelectionRange(start + prefix.length, start + prefix.length);
    });
  };

  /** Accepts the highlighted function of the completion list: `NAME(` replaces the prefix. */
  const acceptFunction = (name: string) => {
    if (!edit || !fnPick) return;
    const el = lastInput.current === 'bar' ? barRef.current : inputRef.current;
    const caretAt = el?.selectionStart ?? edit.text.length;
    const text = `${edit.text.slice(0, fnPick.from)}${name}(${edit.text.slice(caretAt)}`;
    setFnPick(null);
    setEditText(text, fnPick.from + name.length + 1);
  };

  /** Ctrl+Enter: the entry goes into every selected cell, the selection stays. */
  const fillEntry = async () => {
    if (!host || !edit) return;
    const e = edit;
    setEdit(null);
    setFnPick(null);
    setCopyMark(null);
    picking.current = null;
    if (inputRef.current) inputRef.current.value = '';
    await exec({
      cmd: 'fillInputs',
      sheet,
      row: e.pos.row,
      col: e.pos.col,
      text: e.text,
      ranges: allRanges.map(toRange),
    });
    focusInput();
  };

  /**
   * Ctrl+Shift+Enter: the entry becomes one array formula over the selection (a single
   * cell of an existing array formula: that whole array); the selection stays.
   */
  const commitArray = async () => {
    if (!host || !edit) return;
    const e = edit;
    if (!e.text.startsWith('=')) {
      void commit(selection);
      return;
    }
    setEdit(null);
    setCopyMark(null);
    if (inputRef.current) inputRef.current.value = '';
    try {
      await host.exec({
        cmd: 'setArrayFormula',
        sheet: e.sheet ?? sheet,
        range: toRange(rangeOf(selection)),
        formula: e.text,
      });
      await refreshInfo();
    } catch (err) {
      holdBar.current = true;
      setEdit(e);
      setBarText(e.text);
      if (inputRef.current) inputRef.current.value = e.text;
      fail(err);
    }
    focusInput();
  };

  /** Keys while a cell is being edited, in the cell or in the formula bar (same rules). */
  const onEditKey = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (!edit) return;
    const el = e.currentTarget;
    const plain = !e.ctrlKey && !e.altKey && !e.metaKey;
    if (fnPick && fnPick.items.length && plain) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const n = fnPick.items.length;
        setFnPick({ ...fnPick, index: (fnPick.index + (e.key === 'ArrowDown' ? 1 : n - 1)) % n });
        return;
      }
      if (e.key === 'Tab' && !e.shiftKey) {
        e.preventDefault();
        acceptFunction(fnPick.items[fnPick.index]!);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setFnPick(null);
        return;
      }
    }
    if (e.key === 'Enter' && e.altKey && !e.ctrlKey) {
      // A line break inside the cell.
      e.preventDefault();
      const a = el.selectionStart ?? edit.text.length;
      const b = el.selectionEnd ?? a;
      picking.current = null;
      setEditText(`${edit.text.slice(0, a)}\n${edit.text.slice(b)}`, a + 1);
      return;
    }
    if (isArrayEntry(e)) {
      e.preventDefault();
      picking.current = null;
      setFnPick(null);
      void commitArray();
      return;
    }
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !e.shiftKey) {
      e.preventDefault();
      void fillEntry();
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      picking.current = null;
      setFnPick(null);
      void commit(enterFrom(selection, e.shiftKey));
    } else if (e.key === 'Tab' && plain) {
      e.preventDefault();
      picking.current = null;
      setFnPick(null);
      void commit(tabFrom(selection, e.shiftKey));
    } else if (e.key === 'Escape') {
      e.preventDefault();
      picking.current = null;
      setFnPick(null);
      cancelEdit();
    } else if (e.key === 'F2' && plain) {
      // Enter mode ⇄ Edit mode.
      e.preventDefault();
      picking.current = null;
      setEdit({ ...edit, inPlace: !edit.inPlace });
    } else if (navKey[e.key] && !edit.inPlace && !e.altKey) {
      e.preventDefault();
      const caretAt = el.selectionStart ?? edit.text.length;
      if (!e.ctrlKey && !e.metaKey && (picking.current || canPoint(edit.text, caretAt))) {
        pointMove(navKey[e.key]!, e.shiftKey);
        return;
      }
      tabRun.current = null;
      void commit(move(selection, navKey[e.key]!, false, 1, hidden));
    } else if (e.key === 'F4' && plain) {
      e.preventDefault();
      cycleRefIn(el);
    }
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (composing.current || e.nativeEvent.isComposing) return;
    if (edit) {
      onEditKey(e);
      return;
    }
    if (pick) {
      // The pick list (Alt+Down): arrows choose, Enter writes the entry, Esc closes.
      const n = pick.items.length;
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        setPick({ ...pick, index: (pick.index + (e.key === 'ArrowDown' ? 1 : n - 1)) % n });
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        const v = pick.items[pick.index];
        setPick(null);
        if (v !== undefined) choosePick(v);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setPick(null);
        return;
      }
      setPick(null);
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
    if (navigate(e)) return;
    if (e.key === 'F5') {
      e.preventDefault();
      run('edit.goto');
      return;
    }
    // Alt+Down on a heading of the AutoFilter: its drop-down.
    const af = summary?.autoFilter;
    const at = selection.active;
    if (
      e.altKey &&
      e.key === 'ArrowDown' &&
      af &&
      at.row === af[0] &&
      at.col >= af[1] &&
      at.col <= af[3]
    ) {
      e.preventDefault();
      const c = canvasRef.current?.getBoundingClientRect();
      if (c && selRect)
        setFilterMenu({ col: at.col, x: c.left + selRect.x, y: c.top + selRect.y + selRect.h });
      return;
    }
    if (e.altKey && e.key === 'ArrowDown' && !e.ctrlKey && !e.shiftKey) {
      // The validation list, else the text entries of the column (pick from the list).
      e.preventDefault();
      if (dvList) setPick({ items: dvList, index: 0 });
      else {
        const a = selection.active;
        void query<string[]>({ cmd: 'columnEntries', sheet, row: a.row, col: a.col }).then(
          (list) => {
            const items = [...(list ?? [])].sort((x, y) =>
              x.localeCompare(y, i18n.language, { sensitivity: 'base' }),
            );
            if (items.length) setPick({ items, index: 0 });
          },
        );
      }
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
    // Formatting, cells and sheet shortcuts bound to physical keys (homeFormat.ts).
    const formatKey = formatKeyCommand(e);
    if (formatKey) {
      e.preventDefault();
      run(formatKey);
      return;
    }
    const cmd =
      registry.match(e) ??
      // Alt+= by its physical key: layouts where Alt changes the character of the key.
      (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && e.code === 'Equal'
        ? registry.match({
            key: '=',
            altKey: true,
            ctrlKey: false,
            metaKey: false,
            shiftKey: false,
          })
        : undefined);
    if (cmd) {
      e.preventDefault();
      run(cmd.id);
      return;
    }
    if (e.key === 'Escape' && painter) {
      setPainterKeep({ sticky: false, at: 0 });
      setPainter(null);
      return;
    }
    if (e.key === 'Escape' && copyMark) {
      setCopyMark(null);
      return;
    }
    const dir = navKey[e.key];
    if (dir) {
      e.preventDefault();
      tabRun.current = null;
      select(move(selection, dir, e.shiftKey, 1, hidden));
    } else if (e.key === 'PageDown' || e.key === 'PageUp') {
      e.preventDefault();
      tabRun.current = null;
      const rows = Math.max(1, Math.floor(size.h / (20 * scale)));
      select(move(selection, e.key === 'PageDown' ? 'down' : 'up', e.shiftKey, rows, hidden));
    } else if (e.key === 'Enter' && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      // With cells marked by a copy, Enter pastes them once and ends the mark.
      if (copyMark && !e.shiftKey) {
        tabRun.current = null;
        void enterPaste();
        return;
      }
      select(enterFrom(selection, e.shiftKey));
    } else if (e.key === 'Tab' && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      select(tabFrom(selection, e.shiftKey));
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
  const cycleRefIn = (el: HTMLTextAreaElement) => {
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

  /** An entry of the pick list (validation list / Alt+Down) written into the active cell. */
  const choosePick = (v: string) => {
    void exec({
      cmd: 'setInputs',
      sheet,
      cells: [[selection.active.row, selection.active.col, v]],
    }).then(focusInput);
  };

  // Typing (including IME output) lands in the input and switches to edit mode.
  const onInput = (text: string, el?: HTMLTextAreaElement, inputType?: string) => {
    // An Enter whose key went elsewhere (Name Box, a dialog) and whose line break landed here.
    if (!edit && (inputType === 'insertLineBreak' || text === '\n')) {
      if (el) el.value = '';
      return;
    }
    const pos = edit?.pos ?? selection.active;
    const next: EditState = edit ? { ...edit, text } : { pos, text, inPlace: false, sheet };
    // Typing ends the reference being picked with the arrows.
    picking.current = null;
    const at = el?.selectionEnd ?? text.length;
    setCaret(at);
    // Function names completing the one being typed (Tab accepts).
    const fn = functionPrefix(text, at);
    if (fn) {
      const show = (names: string[]) => {
        const items = functionMatches(names, fn.prefix);
        setFnPick(items.length ? { from: fn.from, items, index: 0 } : null);
      };
      if (fnNames.current) show(fnNames.current);
      else
        void query<{ names: string[] }>({ cmd: 'functionList' }).then((r) => {
          fnNames.current = [...(r?.names ?? [])].sort();
          // Only if the same name is still being typed (more keys may have come meanwhile).
          const now = el ? functionPrefix(el.value, el.selectionEnd ?? el.value.length) : fn;
          if (now?.from === fn.from && now.prefix === fn.prefix) show(fnNames.current);
        });
    } else setFnPick(null);
    setEdit(next);
    setBarText(text);
    // AutoComplete: typed text (at the end of the entry, not a deletion) completed from the
    // column's entries when exactly one of them starts with it; the rest stays selected.
    const typing = !inputType || inputType.startsWith('insert');
    if (
      el &&
      !next.inPlace &&
      typing &&
      text &&
      !text.startsWith('=') &&
      at === text.length &&
      parseNumber(text, decimal) === null
    ) {
      const key = `${revision}:${sheet}:${pos.row}:${pos.col}`;
      const complete = (list: string[]) => {
        if (el.value !== text) return;
        const low = text.toLocaleLowerCase();
        // The entries are distinct ignoring case: one hit is one possible completion.
        const hits = list.filter((v) => v.toLocaleLowerCase().startsWith(low));
        if (hits.length !== 1 || hits[0]!.length <= text.length) return;
        const full = text + hits[0]!.slice(text.length);
        el.value = full;
        el.setSelectionRange(text.length, full.length);
        setEdit({ ...next, text: full });
        setBarText(full);
      };
      if (entries.current?.key === key) complete(entries.current.list);
      else
        void query<string[]>({ cmd: 'columnEntries', sheet, row: pos.row, col: pos.col }).then(
          (list) => {
            entries.current = { key, list: list ?? [] };
            complete(entries.current.list);
          },
        );
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

  /** Start moving (header) or sizing (corner handle) a slicer / timeline: anchors in cells. */
  const startFrameDrag = async (
    e: ReactMouseEvent,
    frame: 'slicer' | 'timeline',
    f: { index: number; from: [number, number]; to: [number, number] },
    resize: boolean,
  ) => {
    if (resize) {
      e.preventDefault();
      e.stopPropagation();
    }
    const at = { clientX: e.clientX, clientY: e.clientY };
    const hit = await hitAt(at);
    const start =
      hit && (hit.kind === 'cell' || hit.kind === 'chart' || hit.kind === 'smartArt')
        ? { row: hit.row, col: hit.col }
        : { row: f.from[0], col: f.from[1] };
    drag.current = {
      kind: 'chart',
      frame,
      index: f.index,
      resize,
      start,
      from: f.from,
      to: f.to,
    };
  };

  // The frames call the latest handler through a ref (it reads the current view and host).
  const frameDrag = useRef<typeof startFrameDrag>(async () => {});
  useEffect(() => {
    frameDrag.current = startFrameDrag;
  });

  /** Hit test on the drawn grid (synchronous); `null` when the grid could not be read. */
  const hitNow = (e: { clientX: number; clientY: number }): LocalHit | null => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!grid || !rect) return null;
    return hitLocal(
      grid.g,
      (e.clientX - rect.left) / grid.scale,
      (e.clientY - rect.top) / grid.scale,
      { hidden, merges: merges ?? [], autoFilter: summary?.autoFilter ?? null },
    );
  };

  /** Points of a client position on the drawn grid. */
  const gridPoint = (e: { clientX: number; clientY: number }) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!grid || !rect) return null;
    return { x: (e.clientX - rect.left) / grid.scale, y: (e.clientY - rect.top) / grid.scale };
  };

  /** Whether a client point is on the border of the selection (drag to move it). */
  const onSelectionBorder = (e: { clientX: number; clientY: number }) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!selRect || !rect || extra.length > 0 || chart !== null) return false;
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const { x: l, y: t, w, h } = selRect;
    // A narrow band: 3 px outside the border, 2 px inside it.
    const inside = x > l + 2 && x < l + w - 2 && y > t + 2 && y < t + h - 2;
    const outer = x >= l - 3 && x <= l + w + 3 && y >= t - 3 && y <= t + h + 3;
    // Not on the fill handle (bottom-right corner).
    const handle = Math.abs(x - (l + w)) <= 6 && Math.abs(y - (t + h)) <= 6;
    return outer && !inside && !handle;
  };

  const onMouseDown = async (e: ReactMouseEvent) => {
    if (e.button !== 0) return;
    if (edit) {
      // A click while the caret of a formula can take a reference picks the clicked cell.
      const el = lastInput.current === 'bar' ? barRef.current : inputRef.current;
      const at = el?.selectionStart ?? edit.text.length;
      if (picking.current || canPoint(edit.text, at)) {
        e.preventDefault();
        const now = hitNow(e);
        const hit = now?.kind === 'cell' ? now : await hitAt(e);
        if (hit?.kind === 'cell') {
          pointMove(null, e.shiftKey && !!picking.current, { row: hit.row, col: hit.col });
          drag.current = { kind: 'point' };
        }
        return;
      }
    }
    // The grid as drawn answers at once; the engine only when it could not be read, or when
    // charts may cover the cells (they are not in the drawn geometry).
    const local = hitNow(e);
    const quick =
      local !== null &&
      local.kind !== 'none' &&
      (local.kind !== 'cell' || !(summary?.charts ?? 0)) &&
      !(local.kind === 'cell' && e.altKey);
    if (
      quick &&
      local.kind === 'cell' &&
      !e.shiftKey &&
      !e.ctrlKey &&
      !e.metaKey &&
      onSelectionBorder(e)
    ) {
      // Dragging the selection by its border moves it (Ctrl: copies it).
      const at = gridPoint(e);
      const start = at && cellNear(grid!.g, at.x, at.y);
      if (start) {
        e.preventDefault();
        if (edit) await commit();
        drag.current = { kind: 'move', source: rangeOf(selection), start, to: null };
        focusInput();
        return;
      }
    }
    const before = selection;
    if (edit) {
      if (quick) void commit();
      else await commit();
    }
    const hit = quick ? local : await hitAt(e);
    if (!hit || !host) return;
    await gridMouseDown(e, hit, before, quick);
  };

  const gridMouseDown = async (
    e: ReactMouseEvent,
    hit: SheetHit | LocalHit,
    before: Selection,
    quick: boolean,
  ) => {
    if (!host) return;
    const selection = before;
    const last = info?.sheets[sheet];
    setChart(null);
    setSmartArt(null);
    setFormSel(null);
    setFrameSel(null);
    const multi = (e.ctrlKey || e.metaKey) && !e.shiftKey;
    // Ctrl+click / Ctrl+drag starts another range of a multi-range selection.
    if (multi && (hit.kind === 'cell' || hit.kind === 'columnHeader' || hit.kind === 'rowHeader'))
      setExtra((x) => [...x, rangeOf(selection)]);
    else if (!e.shiftKey) setExtra([]);
    if (hit.kind === 'cell') {
      const p = { row: hit.row, col: hit.col };
      setSelection(e.shiftKey ? { ...selection, focus: p } : single(p));
      drag.current = { kind: 'select' };
      focusInput();
      if (quick) {
        // Drawn already; the engine confirms what is under the point (a SmartArt diagram is
        // not in the drawn geometry) and the selection goes back if it was one.
        const eng = await hitAt(e);
        if (eng && (eng.kind === 'smartArt' || eng.kind === 'chart')) {
          drag.current = null;
          setSelection(before);
          await gridMouseDown(e, eng, before, false);
          return;
        }
      }
      // A click on a checkbox toggles it (one undo step).
      const at = point(e);
      if (!e.shiftKey && !multi && at) {
        const local =
          grid && rangeBox(grid.g, { r0: hit.row, c0: hit.col, r1: hit.row, c1: hit.col }, hidden);
        const box = local ?? (await host.rangeRect(view, hit.row, hit.col, hit.row, hit.col));
        if (await clickCheckbox(host, (c) => exec(c), sheet, hit.row, hit.col, box, at.x, at.y))
          drag.current = null;
      }
    } else if (hit.kind === 'columnEdge' || hit.kind === 'rowEdge') {
      const rows = hit.kind === 'rowEdge';
      const index = rows ? hit.row : hit.col;
      const start = rows ? e.clientY : e.clientX;
      const known = grid && segSize(grid.g, rows, index);
      let size = known ?? 0;
      if (known === null || known === undefined) {
        const r = rows
          ? await host.rangeRect(view, index, 0, index, 0)
          : await host.rangeRect(view, 0, index, 0, index);
        size = r ? (rows ? r.h : r.w) : 0;
      }
      drag.current = { kind: 'resize', rows, index, start, size };
      const at = point(e);
      if (at) setGuide(rows ? { y: at.y * scale } : { x: at.x * scale });
      showSizeTip(e, rows, size);
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
      // Dragging over the headers selects several columns.
      drag.current = {
        kind: 'headers',
        rows: false,
        anchor: e.shiftKey ? selection.anchor.col : hit.col,
      };
    } else if (hit.kind === 'rowHeader') {
      const to = { row: hit.row, col: Math.max(last?.lastCol ?? 0, 25) };
      setSelection(
        e.shiftKey
          ? { ...selection, focus: to, anchor: { row: selection.anchor.row, col: 0 } }
          : span({ row: hit.row, col: 0 }, to),
      );
      drag.current = {
        kind: 'headers',
        rows: true,
        anchor: e.shiftKey ? selection.anchor.row : hit.row,
      };
    } else if (hit.kind === 'corner') {
      setSelection(span({ row: 0, col: 0 }, { row: last?.lastRow ?? 0, col: last?.lastCol ?? 0 }));
    }
    focusInput();
  };

  /** Column-width / row-height tip next to the pointer while a boundary is dragged. */
  const showSizeTip = (e: { clientX: number; clientY: number }, rows: boolean, sizePt: number) => {
    const rect = scrollerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const { value, px: pixels } = resizeTip(rows, sizePt);
    const num = new Intl.NumberFormat(i18n.language, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(value);
    setSizeTip({
      x: e.clientX - rect.left + 12,
      y: e.clientY - rect.top - 28,
      text: t(rows ? 'sheetEditor.gridFeel.heightTip' : 'sheetEditor.gridFeel.widthTip', {
        value: num,
        px: pixels,
      }),
    });
  };

  /** Pointer moves while a drag runs (listened on the window: drags may leave the grid). */
  const onDragMove = async (e: MouseEvent) => {
    const d = drag.current;
    if (!d) return;
    if (d.kind === 'resize') {
      const at = point(e);
      if (at) setGuide(d.rows ? { y: at.y * scale } : { x: at.x * scale });
      const delta = ((d.rows ? e.clientY : e.clientX) - d.start) / scale;
      showSizeTip(e, d.rows, Math.max(0, d.size + delta));
      return;
    }
    const at = gridPoint(e);
    const near = grid && at ? cellNear(grid.g, at.x, at.y) : null;
    if (d.kind === 'headers') {
      if (!near) return;
      const last = info?.sheets[sheet];
      setSelection(
        d.rows
          ? span(
              { row: d.anchor, col: 0 },
              { row: near.row, col: Math.max(last?.lastCol ?? 0, 25) },
            )
          : span(
              { row: 0, col: d.anchor },
              { row: Math.max(last?.lastRow ?? 0, 999), col: near.col },
            ),
      );
      return;
    }
    if (d.kind === 'move') {
      if (!near) return;
      const dr = Math.max(near.row - d.start.row, -d.source.r0);
      const dc = Math.max(near.col - d.start.col, -d.source.c0);
      const to =
        dr === 0 && dc === 0
          ? null
          : {
              r0: d.source.r0 + dr,
              c0: d.source.c0 + dc,
              r1: d.source.r1 + dr,
              c1: d.source.c1 + dc,
            };
      d.to = to;
      setFillRect(to ? localBox(to) : null);
      return;
    }
    if (d.kind === 'select') {
      if (grid) {
        // Live, from the drawn grid; past the edge the range stops at the last visible cell.
        if (near)
          setSelection((s) =>
            s.focus.row === near.row && s.focus.col === near.col ? s : { ...s, focus: near },
          );
        return;
      }
      const hit = await hitAt(e);
      if (hit?.kind === 'cell') {
        setSelection((s) => ({ ...s, focus: { row: hit.row, col: hit.col } }));
      }
      return;
    }
    if (d.kind === 'point') {
      // Dragging while picking a reference: the picked range grows to the cell under the mouse.
      let cell: CellPos | null = near;
      if (!cell) {
        const hit = await hitAt(e);
        if (hit?.kind === 'cell') cell = { row: hit.row, col: hit.col };
      }
      const p = picking.current;
      if (cell && p && (p.focus.row !== cell.row || p.focus.col !== cell.col))
        pointMove(null, true, { row: cell.row, col: cell.col });
      return;
    }
    if (d.kind === 'fill' && grid) {
      if (!near) return;
      const target = fillTarget(d.source, near);
      fillTo.current = target;
      setFillRect(target ? localBox(target) : null);
      return;
    }
  };
  const dragMove = useRef<(e: MouseEvent) => void>(() => {});
  useEffect(() => {
    dragMove.current = (e) => void onDragMove(e);
  });
  useEffect(() => {
    const move = (e: MouseEvent) => dragMove.current(e);
    window.addEventListener('mousemove', move);
    return () => window.removeEventListener('mousemove', move);
  }, []);

  /** Hover cursor from the drawn grid (synchronous). */
  const hoverCursor = (e: ReactMouseEvent): string | null => {
    const hit = hitNow(e);
    if (!hit) return null;
    if (painter) return 'copy';
    switch (hit.kind) {
      case 'columnEdge':
        return 'col-resize';
      case 'rowEdge':
        return 'row-resize';
      case 'columnHeader':
        return HEADER_CURSOR_DOWN;
      case 'rowHeader':
        return HEADER_CURSOR_RIGHT;
      case 'filterButton':
        return 'pointer';
      case 'cell':
        if (summary?.charts) return null;
        return onSelectionBorder(e) ? 'move' : 'cell';
      default:
        return 'default';
    }
  };

  const onMouseMove = async (e: ReactMouseEvent) => {
    const d = drag.current;
    if (d && d.kind !== 'chart' && (d.kind !== 'fill' || grid)) return;
    if (!d) {
      const c = hoverCursor(e);
      if (c !== null) {
        setCursor(c);
        return;
      }
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

  /** Fills `target` from `source` (one undo step); the Auto Fill Options button follows. */
  const runFill = async (source: Range, target: Range, mode: AutoFillMode) => {
    const done = await exec({
      cmd: 'autofill',
      sheet,
      source: toRange(source),
      target: toRange(target),
      mode,
    });
    if (done === null) return;
    const sel = span({ row: target.r0, col: target.c0 }, { row: target.r1, col: target.c1 });
    setSelection(sel);
    setFillUi((u) => ({
      n: (u?.n ?? 0) + 1,
      mode,
      source: toRange(source),
      target: toRange(target),
      sel,
    }));
  };

  /** Auto Fill Options: the last fill again in another way (replacing it). */
  const switchFill = async (mode: AutoFillMode) => {
    const ui = fillUi && fillUi.sel === selection ? fillUi : null;
    if (!host || !ui || ui.mode === mode) return;
    if (!(await host.undo())) return;
    await exec({ cmd: 'autofill', sheet, source: ui.source, target: ui.target, mode });
    setSelection(ui.sel);
    setFillUi({ ...ui, n: ui.n + 1, mode });
    focusInput();
  };

  // Mouse-up ends drags anywhere on the page.
  const endDrag = useRef<(e: MouseEvent) => void>(() => {});
  const onDragEnd = (e: MouseEvent) => {
    const d = drag.current;
    drag.current = null;
    if (!d || !host) return;
    if (d.kind === 'resize') {
      setGuide(null);
      setSizeTip(null);
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
          cmd:
            d.frame === 'slicer'
              ? 'moveSlicer'
              : d.frame === 'timeline'
                ? 'moveTimeline'
                : d.smart
                  ? 'moveSmartArt'
                  : 'moveChart',
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
      // Ctrl held when the handle is released: a single number counts up, a series copies.
      if (target) void runFill(d.source, target, e.ctrlKey || e.metaKey ? 'toggle' : 'auto');
    } else if (d.kind === 'move') {
      setFillRect(null);
      const to = d.to;
      if (to) {
        // One undo step: cut (or, with Ctrl, copy) and paste at the drop place; the moved
        // cells stay selected.
        const copy = e.ctrlKey || e.metaKey;
        const drop = () =>
          void exec({
            cmd: 'batch',
            cmds: [
              { cmd: 'copy', sheet, range: toRange(d.source), cut: !copy },
              { cmd: 'paste', sheet, row: to.r0, col: to.c0 },
            ],
          }).then((r) => {
            if (r !== null)
              setSelection(span({ row: to.r0, col: to.c0 }, { row: to.r1, col: to.c1 }));
          });
        // Dropping onto cells that hold data asks first (the dragged cells themselves
        // do not count).
        void query<boolean>({
          cmd: 'occupied',
          sheet,
          range: toRange(to),
          except: toRange(d.source),
        }).then((busy) => {
          if (busy) setDialog({ kind: 'replaceWarn', run: drop });
          else drop();
        });
      }
    } else if (d.kind === 'select' && painter) {
      const to = rangeOf(selection);
      if (!painterKeep.sticky) setPainter(null);
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
    const local = hitNow(e);
    const hit =
      local && local.kind !== 'none' && (local.kind !== 'cell' || !(summary?.charts ?? 0))
        ? local
        : await hitAt(e);
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
      // A value cell of a pivot table shows its source records instead of being edited.
      const p = pivots.sheet === sheet ? pivotAt(pivots.list, hit.row, hit.col) : null;
      const a = p && !p.model ? p.dataArea : null;
      if (a && hit.row >= a.r0 && hit.row <= a.r1 && hit.col >= a.c0 && hit.col <= a.c1)
        void drillDown(hit.row, hit.col);
      else {
        // Edit in place with the caret where the cell was double-clicked.
        const clickX = e.clientX;
        await startEdit(true);
        placeCaret(clickX);
      }
    }
  };

  /**
   * Box, font and alignment of the in-cell editor: over the cell's border box with the
   * cell's font and the engine's padding and baseline, so the text does not move when editing
   * starts; it grows (right, or left for right-aligned text) to fit what is typed.
   */
  const cellEditorLayout = (r: Rect, e: EditState, lines = 1) => {
    const sizePx = (fmt?.fontSize ?? 11) * scale;
    const font = `${fmt?.italic ? 'italic ' : ''}${fmt?.bold ? 'bold ' : ''}${sizePx}px ${CELL_FONT_FAMILY(cellFamily)}`;
    let ascent = sizePx * 0.8;
    let descent = sizePx * 0.25;
    let textW = sizePx * 0.5 * e.text.length;
    const measureCtx = measureContext();
    if (measureCtx) {
      measureCtx.font = font;
      // Alt+Enter lines: the widest one sets the width.
      textW = Math.max(...e.text.split('\n').map((l) => measureCtx.measureText(l).width));
      const probe = measureCtx.measureText('Hg');
      if (probe.fontBoundingBoxAscent) ascent = probe.fontBoundingBoxAscent;
      if (probe.fontBoundingBoxDescent) descent = probe.fontBoundingBoxDescent;
    }
    const metrics = cellFontMetrics[cellFamily];
    const baseline = metrics ? bottomBaseline(r.h, sizePx, metrics, zoom) : r.h - zoom - descent;
    const lineH = ascent + descent;
    const border = 2;
    // The border box runs from one pixel before the cell to one pixel after it (like the
    // selection border); text starts 2 px (scaled) inside the cell as the engine draws it.
    const padTop = Math.max(0, baseline - ascent - (border - 1));
    // Each further line (Alt+Enter) adds one line height.
    const boxH = r.h + 2 + Math.max(0, lines - 1) * lineH;
    const padBottom = Math.max(0, boxH - 2 * border - padTop - lineH * Math.max(1, lines));
    const padX = Math.max(0, 2 * zoom - (border - 1));
    const numeric = parseNumber(e.text, decimal) !== null;
    const align: 'left' | 'right' | 'center' =
      fmt?.hAlign === 'right' ||
      (e.inPlace && numeric && (!fmt?.hAlign || fmt.hAlign === 'general'))
        ? 'right'
        : fmt?.hAlign === 'center'
          ? 'center'
          : 'left';
    const maxW = align === 'right' ? r.x + r.w + 1 : size.w - r.x;
    const width = editorWidth(textW, r.w + 2, padX + border, Math.max(r.w + 2, maxW));
    const left = align === 'right' ? r.x + r.w + 1 - width : r.x - 1;
    return {
      align,
      style: {
        left,
        top: r.y - 1,
        width,
        height: boxH,
        padding: `${padTop}px ${padX}px ${padBottom}px ${padX}px`,
        lineHeight: `${lineH}px`,
        font,
        textAlign: align,
      } as const,
    };
  };

  /** Puts the in-cell editor's caret at a client x (after the editor has been laid out). */
  const placeCaret = (clientX: number) => {
    const set = () => {
      const el = inputRef.current;
      const measureCtx = measureContext();
      if (!el || !measureCtx) return;
      const cs = getComputedStyle(el);
      measureCtx.font = cs.font || `${cs.fontSize} ${cs.fontFamily}`;
      const text = el.value;
      const measure = (s: string) => measureCtx.measureText(s).width;
      const b = el.getBoundingClientRect();
      const l = b.left + parseFloat(cs.borderLeftWidth || '0') + parseFloat(cs.paddingLeft || '0');
      const rr =
        b.right - parseFloat(cs.borderRightWidth || '0') - parseFloat(cs.paddingRight || '0');
      const w = measure(text);
      const start =
        cs.textAlign === 'right' ? rr - w : cs.textAlign === 'center' ? (l + rr - w) / 2 : l;
      const i = caretAt(text, clientX - start, measure);
      el.setSelectionRange(i, i);
    };
    requestAnimationFrame(() => {
      set();
      requestAnimationFrame(set);
    });
  };

  /**
   * Double-click on the fill handle: fills the selection down as far as the block of data
   * it belongs to goes (the neighbouring column of a list), like dragging the handle there.
   */
  const fillDownToData = async () => {
    const src = rangeOf(selection);
    const region = await query<SheetRange>({
      cmd: 'region',
      sheet,
      range: { r0: src.r0, c0: src.c0, r1: src.r0, c1: src.c0 },
    });
    if (!region || region.r1 <= src.r1) return;
    const target = fillTarget(src, { row: region.r1, col: src.c0 });
    if (!target) return;
    await runFill(src, target, 'auto');
  };

  const onNameBox = (text: string) => {
    const s = parseA1(text);
    if (s) select(s);
    else if (host)
      void host
        .exec<{ sheet: number; range: SheetRange } | null>({ cmd: 'resolve', sheet, text })
        .then((r) => {
          // A new name typed in the Name Box names the selection (workbook scope).
          if (!r && /^[\p{L}_\\][\p{L}\p{N}_.\\?]*$/u.test(text.trim())) {
            const at = absoluteRef(info?.sheets[sheet]?.name ?? '', toRange(rangeOf(selection)));
            void exec({ cmd: 'defineName', name: text.trim(), refersTo: at });
          }
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
  // The user's ribbon and Quick Access Toolbar (CORE-UI-005).
  const ribbonCustom = useRibbonCustomization({
    app: 'sheet',
    registry,
    store: platform.shell,
    files: platform.files,
    tabs: () => ribbonTabsRef.current,
    extraCommands: () => shortcutCommands([], registry, (id) => commandLabel(i18n, id)),
    shortcuts,
    enabled: (id) => !!state && registry.isEnabled(id, state),
    run: (id) => run(id),
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
              item('file.exportPdf', t('sheetEditor.exportPdf'), <FileDown size={s} />, 'small'),
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
            items: twoRows(FONT_TOP_ROW, [
              {
                ...item('format.fontName', t('sheetEditor.fontName'), null, 'small'),
                render: (
                  <FontNameBox
                    value={state?.fmt?.fontName ?? ''}
                    disabled={!state?.hasWorkbook}
                    onApply={(fontName) => void formatSel({ fontName })}
                    onDone={focusInput}
                  />
                ),
              },
              {
                ...item('format.fontSize', t('sheetEditor.fontSizeBox'), null, 'small'),
                render: (
                  <FontSizeBox
                    value={state?.fmt?.fontSize ?? null}
                    disabled={!state?.hasWorkbook}
                    onApply={(fontSize) => void formatSel({ fontSize })}
                    onDone={focusInput}
                  />
                ),
              },
              item('format.bold', t('sheetEditor.bold'), <Bold size={s} />, 'small'),
              item('format.italic', t('sheetEditor.italic'), <Italic size={s} />, 'small'),
              item('format.underline', t('sheetEditor.underline'), <Underline size={s} />, 'small'),
              item('format.fontGrow', t('sheetEditor.fontGrow'), <AArrowUp size={s} />, 'small'),
              item(
                'format.fontShrink',
                t('sheetEditor.fontShrink'),
                <AArrowDown size={s} />,
                'small',
              ),
              {
                ...item(
                  'format.bordersLast',
                  t('sheetEditor.bordersLast', {
                    border: t(`sheetEditor.border.${lastUsed.border}`),
                  }),
                  null,
                  'small',
                ),
                render: (
                  <SplitButton
                    id="format.bordersLast"
                    menuId="format.borders"
                    label={t('sheetEditor.bordersLast', {
                      border: t(`sheetEditor.border.${lastUsed.border}`),
                    })}
                    menuLabel={t('sheetEditor.borders')}
                    icon={<Icon name="format.bordersLast" size={20} />}
                    disabled={!state?.hasWorkbook}
                    onApply={() => run('format.bordersLast')}
                    onMenu={() => run('format.borders')}
                  />
                ),
              },
              {
                ...item('format.fillColorLast', t('sheetEditor.fillColorLast'), null, 'small'),
                render: (
                  <SplitButton
                    id="format.fillColorLast"
                    menuId="format.fillColor"
                    label={t('sheetEditor.fillColorLast')}
                    menuLabel={t('sheetEditor.fillColor')}
                    icon={
                      <span className={styles.colorSplit}>
                        <Icon name="format.fillColorLast" size={16} />
                        <span
                          className={styles.colorSplitBar}
                          data-color={lastUsed.fill}
                          style={{ background: `#${lastUsed.fill}` }}
                        />
                      </span>
                    }
                    disabled={!state?.hasWorkbook}
                    onApply={() => run('format.fillColorLast')}
                    onMenu={() => run('format.fillColor')}
                  />
                ),
              },
              {
                ...item('format.fontColorLast', t('sheetEditor.fontColorLast'), null, 'small'),
                render: (
                  <SplitButton
                    id="format.fontColorLast"
                    menuId="format.fontColor"
                    label={t('sheetEditor.fontColorLast')}
                    menuLabel={t('sheetEditor.fontColor')}
                    icon={
                      <span className={styles.colorSplit}>
                        <Icon name="format.fontColorLast" size={16} />
                        <span
                          className={styles.colorSplitBar}
                          data-color={lastUsed.font}
                          style={{ background: `#${lastUsed.font}` }}
                        />
                      </span>
                    }
                    disabled={!state?.hasWorkbook}
                    onApply={() => run('format.fontColorLast')}
                    onMenu={() => run('format.fontColor')}
                  />
                ),
              },
            ]),
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
              item(
                'format.orientation',
                t('sheetEditor.orientation.title'),
                <RotateCcw size={s} />,
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
                'format.mergeMenu',
                t('sheetEditor.mergeMenu'),
                <ChevronDown size={12} />,
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
            items: twoRows(NUMBER_TOP_ROW, [
              {
                ...item('format.numberFormatBox', t('sheetEditor.numberFormatBox'), null, 'small'),
                render: (
                  <NumberFormatBox
                    code={state?.fmt?.numFmt ?? 'General'}
                    presets={presets.numberFormats}
                    disabled={!state?.hasWorkbook}
                    onPick={(numFmt) => {
                      void formatSel({ numFmt });
                      focusInput();
                    }}
                    onMore={() => setDialog({ kind: 'format', tab: 'number' })}
                  />
                ),
              },
              item(
                'format.numberFormat',
                t('sheetEditor.numberFormat'),
                <Hash size={s} />,
                'small',
              ),
              item('format.currency', t('sheetEditor.currency'), <Coins size={s} />, 'small'),
              item('format.percent', t('sheetEditor.percent'), <Percent size={s} />, 'small'),
              item('format.comma', t('sheetEditor.comma'), <Hash size={s} />, 'small'),
              item(
                'format.decimalInc',
                t('sheetEditor.decimalInc'),
                <DecimalsArrowRight size={s} />,
                'small',
              ),
              item(
                'format.decimalDec',
                t('sheetEditor.decimalDec'),
                <DecimalsArrowLeft size={s} />,
                'small',
              ),
            ]),
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
              item('cells.insert', t('sheetEditor.insertCells'), <Plus size={s} />, 'small'),
              item(
                'cells.insertMenu',
                t('sheetEditor.insertMenu'),
                <ChevronDown size={12} />,
                'small',
              ),
              item('cells.delete', t('sheetEditor.deleteCells'), <Trash2 size={s} />, 'small'),
              item(
                'cells.deleteMenu',
                t('sheetEditor.deleteMenu'),
                <ChevronDown size={12} />,
                'small',
              ),
              item(
                'cells.formatMenu',
                t('sheetEditor.cellFormatMenu'),
                <Rows3 size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'voice',
            label: t('ribbon.groups.speech'),
            items: [item('edit.dictate', t('commands.edit.dictate'), <Mic size={18} />)],
          },
          {
            id: 'editing',
            label: t('sheetEditor.groupEditing'),
            items: [
              item('formula.autoSum', t('autoSum.button'), <Sigma size={s} />, 'small'),
              item('formula.autoSumMenu', t('autoSum.menu'), <ChevronDown size={12} />, 'small'),
              item('edit.clear', t('sheetEditor.clear'), <Eraser size={s} />, 'small'),
              item(
                'edit.clearMenu',
                t('sheetEditor.clearMenu'),
                <ChevronDown size={12} />,
                'small',
              ),
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
              item(
                'edit.fillSeries',
                t('sheetEditor.series.title'),
                <ListOrdered size={s} />,
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
                'insert.chartRecommended',
                t('quickAnalysis.recommended.command'),
                <ChartColumn size={16} />,
                'small',
              ),
              item('format.quickAnalysis', t('quickAnalysis.button'), <Zap size={16} />, 'small'),
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
                'cell.pictureRemoveBackground',
                t('pictureTools.removeBackground'),
                <Eraser size={s} />,
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
                'insert.timeline',
                t('sheetSlicer.insertTimeline'),
                <CalendarRange size={s} />,
                'small',
              ),
              item(
                'slicer.settings',
                t('sheetSlicer.frameOptions'),
                <Settings2 size={s} />,
                'small',
              ),
              item('slicer.delete', t('sheetSlicer.frameDelete'), <Trash2 size={s} />, 'small'),
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
            items: [
              item('insert.symbol', t('commands.insert.symbol'), <Omega size={18} />),
              item('insert.icon', t('commands.insert.icon'), <Shapes size={18} />),
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
                'insert.slicer',
                t('sheetEditor.slicer.insert'),
                <ListFilter size={s} />,
                'small',
              ),
              item(
                'insert.timeline',
                t('sheetSlicer.insertTimeline'),
                <CalendarRange size={s} />,
                'small',
              ),
              item(
                'pivot.showDetails',
                t('sheetSlicer.showDetails'),
                <ListChecks size={s} />,
                'small',
              ),
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
            id: 'functionLibrary',
            label: t('autoSum.group'),
            items: [
              item(
                'formula.insertFunction',
                t('autoSum.insertFunction.title'),
                <SquareFunction size={18} />,
              ),
              item('formula.autoSum', t('autoSum.button'), <Sigma size={18} />),
              item('formula.autoSumMenu', t('autoSum.menu'), <ChevronDown size={12} />, 'small'),
            ],
          },
          {
            id: 'definedNames',
            label: t('sheetNames.group'),
            items: [
              item('formulas.nameManager', t('sheetNames.manager'), <Tags size={18} />),
              item('formulas.defineName', t('sheetNames.define'), <Tag size={s} />, 'small'),
              item(
                'formulas.createFromSelection',
                t('sheetNames.createFromSelection'),
                <TableProperties size={s} />,
                'small',
              ),
            ],
          },
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
              // Show formulas (Ctrl+`) also sits with the auditing tools.
              item(
                'view.showFormulas',
                t('sheetViews.showFormulas'),
                <SquareFunction size={s} />,
                'small',
              ),
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
              item('data.editLinks', t('sheetLinks.command'), <Link2 size={s} />, 'small'),
            ],
          },
          {
            id: 'dataModel',
            label: t('sheetModel.group'),
            items: [
              item('data.modelTables', t('sheetModel.ribbon.tables'), <Database size={18} />),
              item(
                'data.modelRelationships',
                t('sheetModel.ribbon.relationships'),
                <Network size={s} />,
                'small',
              ),
              item(
                'data.modelMeasures',
                t('sheetModel.ribbon.measures'),
                <Sigma size={s} />,
                'small',
              ),
              item(
                'data.modelPivot',
                t('sheetModel.ribbon.pivot'),
                <TableProperties size={s} />,
                'small',
              ),
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
              item(
                'data.showDetail',
                t('sheetOutline.showDetail'),
                <SquarePlus size={s} />,
                'small',
              ),
              item(
                'data.hideDetail',
                t('sheetOutline.hideDetail'),
                <SquareMinus size={s} />,
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
              item('review.thesaurus', t('commands.review.thesaurus'), <BookA size={s} />, 'small'),
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
          {
            id: 'ai',
            label: t('ribbon.groups.ai'),
            items: [item('ai.pane', t('commands.ai.pane'), <Sparkles size={18} />)],
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
              item(
                'view.freezePanes',
                (info?.sheets[sheet]?.frozenRows ?? 0) + (info?.sheets[sheet]?.frozenCols ?? 0) > 0
                  ? t('sheetEditor.unfreeze')
                  : t('sheetEditor.freezePanes'),
                <Snowflake size={18} />,
              ),
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
            id: 'window',
            label: t('ribbon.groups.window'),
            items: [
              item('window.new', t('commands.window.new'), <AppWindow size={18} />),
              item('window.arrangeAll', t('commands.window.arrangeAll'), <LayoutGrid size={18} />),
              item('window.split', t('commands.window.split'), <Rows2 size={18} />),
              item(
                'window.sideBySide',
                t('commands.window.sideBySide'),
                <Columns2 size={s} />,
                'small',
              ),
              item(
                'window.syncScroll',
                t('commands.window.syncScroll'),
                <ArrowDownUp size={s} />,
                'small',
              ),
              item(
                'window.resetPosition',
                t('commands.window.resetPosition'),
                <RotateCcw size={s} />,
                'small',
              ),
            ],
          },
          {
            id: 'automation',
            label: t('ribbon.groups.automation'),
            items: [
              item('script.editor', t('commands.view.scriptEditor'), <SquareTerminal size={18} />),
              item('script.record', t('commands.view.recordScript'), <CircleDot size={18} />),
              item('script.convertVba', t('commands.view.convertVba'), <FileCode2 size={18} />),
              item('script.viewMacros', t('commands.view.viewMacros'), <FileSearch size={18} />),
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
          ...smartArtDesignGroups({
            api: smartApi,
            labels: smartArtDesignLabels(t).design,
            mk: (id, label, icon, size) => item(id, label, icon, size),
            enabled: (id) => !!state && registry.isEnabled(id, state),
          }),
          {
            id: 'smartartEdit',
            label: t('slideEditor.smartart.group'),
            items: [
              item('smartart.edit', t('slideEditor.smartart.design.edit'), <Pencil size={18} />),
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
        groups: [
          chartElementsGroup({
            t,
            info: chartInfo?.elements,
            run,
            enabled: true,
            paneOpen: chartFormatOpen,
          }),
          ...chartDesignGroups({
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
        ],
      },
    ];
  }, [
    t,
    registry,
    state,
    run,
    chart,
    chartInfo,
    chartFormatOpen,
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
    presets,
    formatSel,
    focusInput,
    lastUsed,
    info,
    smartApi,
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
  // Ribbon with the add-ins' commands and the Add-in tab (CORE-ADDIN-001).
  const shownTabs = addins.applyTabs(ribbonTabs);
  useEffect(() => {
    ribbonTabsRef.current = shownTabs;
  });
  // Command search also finds the commands of the Home drop-down menus (insert / delete
  // sheet, row height, merge across, …) that have no button of their own.
  const onRibbon = new Set(
    shownTabs.flatMap((tab) => tab.groups.flatMap((g) => g.items.map((i) => i.id))),
  );
  const menuCommands = Object.values(COMMAND_MENUS)
    .flat()
    .filter(([id], i, all) => !onRibbon.has(id) && all.findIndex(([x]) => x === id) === i);
  const searchTabs: RibbonTab[] = [
    ...shownTabs,
    {
      id: 'homeMenus',
      label: t('ribbon.tabs.home'),
      groups: [
        {
          id: 'homeMenus',
          label: t('sheetEditor.groupCells'),
          items: menuCommands.map(([id, label]) => ({
            id,
            label: t(label),
            enabled: state ? registry.isEnabled(id, state) : false,
            onSelect: () => run(id),
          })),
        },
      ],
    },
  ];

  const r = rangeOf(selection);
  const statCells = allRanges.reduce((n, x) => n + cellsOf(x), 0);
  const shownStats =
    statCells > 1 && stats && stats.key === statsKeyOf(sheet, allRanges, revision) ? stats : null;
  const statsPending = statCells > STATS_ASYNC && !shownStats;
  const fmtNum = (n: number) =>
    new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 10 }).format(n);
  // Status bar texts (also the current values in its "customise" checklist).
  const cellMode = edit ? (edit.inPlace ? 'edit' : 'enter') : 'ready';
  // The cell editor shows on the edited cell's sheet (not while picking on another one).
  const editHere = !!edit && (edit.sheet ?? sheet) === sheet;
  // Page layout view: the cell editor is drawn over the cell on its page (the grid and
  // its editor are hidden); it is the cell input while it is shown.
  const pageEditing = viewMode === 'pageLayout' && editHere;
  const pageEditor =
    pageEditing && edit
      ? (fontScale: number) => (
          <textarea
            ref={(el) => {
              (inputRef as MutableRefObject<HTMLTextAreaElement | null>).current = el;
              if (el && !el.dataset.ready) {
                el.dataset.ready = '1';
                el.value = edit.text;
                if (edit.inPlace) {
                  lastInput.current = 'cell';
                  el.focus({ preventScroll: true });
                  el.setSelectionRange(el.value.length, el.value.length);
                }
              }
            }}
            rows={1}
            data-testid="page-layout-cell-editor"
            aria-label={t('sheetEditor.cellEditor')}
            style={{ fontSize: 11 * fontScale }}
            autoComplete="off"
            spellCheck={false}
            onFocus={() => {
              lastInput.current = 'cell';
            }}
            onKeyDown={onKeyDown}
            onSelect={(e) => setCaret(e.currentTarget.selectionEnd ?? 0)}
            onInput={(e) =>
              onInput(
                e.currentTarget.value,
                e.currentTarget,
                (e.nativeEvent as InputEvent).inputType,
              )
            }
            onCompositionStart={() => {
              composing.current = true;
            }}
            onCompositionEnd={(e) => {
              composing.current = false;
              onInput(e.currentTarget.value, e.currentTarget);
            }}
          />
        )
      : undefined;
  // Overlays computed for the current text / mark only (older answers are ignored).
  const shownRefBoxes =
    edit && refBoxes && refBoxes.text === edit.text && refBoxes.sheet === (summary?.name ?? '')
      ? refBoxes.boxes
      : [];
  const copyBox = copyMark && copyRect?.mark === copyMark ? copyRect.rect : null;
  const editorRefs = editHere && edit ? formulaRefs(edit.text) : [];
  const editLines = edit ? edit.text.split('\n').length : 1;
  const call = edit ? callAt(edit.text, Math.min(caret, edit.text.length)) : null;
  const hint = call ? argumentHint(call.name, call.arg) : null;
  const filterMode = !!summary?.autoFilter && (summary?.hiddenRows.length ?? 0) > 0;
  // Fewer than two filled cells: nothing to summarise (the bar stays empty).
  const filledStats = shownStats && shownStats.count > 1 ? shownStats : null;
  const numeric = filledStats && filledStats.numbers > 0 ? filledStats : null;
  const statValues: Record<string, string | undefined> = {
    cellMode:
      cellMode === 'edit'
        ? t('statusBar.edit')
        : cellMode === 'enter'
          ? t('statusBar.enter')
          : t('sheetEditor.ready'),
    average: numeric ? fmtNum(numeric.sum / numeric.numbers) : undefined,
    count: filledStats ? String(filledStats.count) : undefined,
    numericalCount: filledStats ? String(filledStats.numbers) : undefined,
    min: numeric && typeof numeric.min === 'number' ? fmtNum(numeric.min) : undefined,
    max: numeric && typeof numeric.max === 'number' ? fmtNum(numeric.max) : undefined,
    sum: numeric ? fmtNum(numeric.sum) : undefined,
    zoom: `${Math.round(zoom * 100)}%`,
  };

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
      case 'autoSum':
        return [
          ...AUTO_SUM_FUNCTIONS.map((f) => ({ id: f, label: t(`autoSum.fn.${f}`) })),
          { id: 'more', label: t('autoSum.more') },
        ];
      case 'borders':
        return [
          ...Object.keys(BORDER_MENU).map((p) => ({ id: p, label: t(`sheetEditor.border.${p}`) })),
          { id: 'more', label: t('sheetEditor.border.more') },
        ];
      case 'clear':
      case 'merge':
      case 'insert':
      case 'delete':
      case 'cellFormat':
        return (COMMAND_MENUS[kind] ?? []).map(([id, label]) => ({ id, label: t(label) }));
      case 'orientation':
        return [
          ...Object.keys(ORIENTATIONS).map((id) => ({
            id,
            label: t(`sheetEditor.orientation.${id}`),
          })),
          { id: 'more', label: t('sheetEditor.orientation.more') },
        ];
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
  /** Inserts a chart of `kind` over the selection and selects it (recommended charts). */
  const insertChartKind = (kind: string) => {
    setQuickOpen(false);
    void exec<{ index: number }>({
      cmd: 'insertChart',
      sheet,
      range: toRange(r),
      kind,
      texts: layoutTexts(t),
    }).then((res) => {
      if (res) setChart(res.index);
    });
  };
  const pickMenu = (kind: string, id: string) => {
    if (kind === 'autoSum') {
      if (id === 'more') {
        run('formula.insertFunction');
        return;
      }
      focusInput();
      run(AUTO_SUM_COMMANDS[id as AutoSumFunction]);
      return;
    }
    if (COMMAND_MENUS[kind]) {
      focusInput();
      run(id);
      return;
    }
    if (kind === 'orientation') {
      if (id === 'more') {
        setDialog({ kind: 'format', tab: 'alignment' });
        return;
      }
      // The same choice again goes back to horizontal text.
      const rotation = ORIENTATIONS[id] ?? 0;
      void formatSel({ rotation: fmt?.rotation === rotation ? 0 : rotation });
      focusInput();
      return;
    }
    const range = toRange(r);
    if (kind === 'borders') {
      if (id === 'more') {
        setDialog({ kind: 'format', tab: 'border' });
        return;
      }
      setLast({ border: id });
      void applyBorder(id);
    } else if (kind === 'styles') void exec({ cmd: 'cellStyle', sheet, range, name: id });
    else if (kind === 'numFmt') void formatSel({ numFmt: id });
    else if (kind === 'chart') {
      // A saved chart template inserts its type with its formatting.
      const template = templateOfItem(id);
      void exec<{ index: number }>({
        cmd: 'insertChart',
        sheet,
        range,
        kind: template?.kind ?? id,
        // The placeholder title of a new chart, the titles of a template.
        texts: layoutTexts(t),
        ...(template ? { template } : {}),
      }).then((res) => {
        if (res) setChart(res.index);
      });
    }
    focusInput();
  };

  return (
    <div
      // `bo-root`: the shared theme rules (focus ring of every control, WCAG 2.4.7) apply here too.
      className={`bo-root ${styles.root}`}
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
        quickAccess={
          ribbonCustom.below
            ? []
            : ribbonCustom.quickAccess(
                shownTabs,
                scriptBindings.quickAccess.map((sc) => ({
                  id: `script.run.${sc.id}`,
                  icon: 'script.run',
                  label: t('script.runBound', { name: sc.name }),
                  shortcut: sc.shortcut,
                  enabled: !!info,
                  onSelect: () => run(`script.run.${sc.id}`),
                })),
              )
        }
      />
      <CommandSearch
        tabs={searchTabs}
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        labels={{
          title: t('commandSearch.title'),
          placeholder: t('commandSearch.placeholder'),
          noResults: t('commandSearch.noResults'),
        }}
      />
      <Ribbon
        tabs={shownTabs}
        labels={ribbonLabels(t)}
        autoActivate={AUTO_CONTEXTS}
        fileMenu={{
          label: t('ribbon.tabs.file'),
          open: backstage,
          onOpen: () => setBackstage(true),
        }}
        {...ribbonCustom.ribbonProps()}
      />
      {ribbonCustom.below && (
        <QuickAccessBar
          below
          label={ribbonCustom.qatLabel}
          items={ribbonCustom.quickAccess(shownTabs)}
        />
      )}
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
            if (e.key === 'Enter') {
              e.preventDefault();
              onNameBox(e.currentTarget.value);
            }
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
        <textarea
          className={styles.formulaInput}
          ref={barRef}
          rows={barExpanded ? 4 : 1}
          data-expanded={barExpanded || undefined}
          aria-label={t('sheetEditor.formulaBar')}
          value={barText}
          spellCheck={false}
          onFocus={() => {
            lastInput.current = 'bar';
            if (!edit) setEdit({ pos: selection.active, text: barText, inPlace: true, sheet });
          }}
          onSelect={(e) => setCaret(e.currentTarget.selectionEnd ?? 0)}
          onChange={(e) => {
            const el = e.currentTarget;
            if (inputRef.current) inputRef.current.value = el.value;
            onInput(el.value, el, (e.nativeEvent as InputEvent).inputType);
            setEdit((cur) => cur && { ...cur, inPlace: true });
          }}
          onKeyDown={(e) => {
            if (composing.current || e.nativeEvent.isComposing) return;
            if (!edit && e.key === 'Escape') return;
            onEditKey(e);
          }}
        />
        <button
          type="button"
          className={styles.barExpand}
          data-testid="formula-bar-expand"
          aria-label={t(
            barExpanded ? 'sheetEditor.collapseFormulaBar' : 'sheetEditor.expandFormulaBar',
          )}
          title={`${t(barExpanded ? 'sheetEditor.collapseFormulaBar' : 'sheetEditor.expandFormulaBar')} (Ctrl+Shift+U)`}
          aria-expanded={barExpanded}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => setBarExpanded((x) => !x)}
        >
          {barExpanded ? '▴' : '▾'}
        </button>
      </div>
      {macroBanner && (
        <MacroBanner
          inline
          onView={showMacros}
          signatureDropped={!!info?.vbaSignatureDropped}
          onDismiss={() => setMacroBanner(false)}
        />
      )}
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
            onCellDouble={(row, col) => {
              // Double-click edits the cell in place, on the page.
              const pos = { row, col };
              setSelection(single(pos));
              void host.cell(sheet, row, col).then((c) => {
                const text = editableInput(c);
                setEdit({ pos, text, inPlace: true, sheet });
                setBarText(text);
              });
            }}
            onHeaderFooter={() => setDialog({ kind: 'pageSetup' })}
            regionRef={pageLayoutRef}
            active={selection.active}
            {...(pageEditor ? { editor: pageEditor } : {})}
            onKeyDown={(e) => {
              const c = registry.match(e);
              if (c) {
                e.preventDefault();
                run(c.id);
                return;
              }
              if (e.ctrlKey || e.metaKey || e.altKey || e.nativeEvent.isComposing) return;
              // Typing on the page edits the selected cell (shown in the formula bar),
              // F2 edits its content; arrows move the selection.
              const arrows: Record<string, Direction> = {
                ArrowUp: 'up',
                ArrowDown: 'down',
                ArrowLeft: 'left',
                ArrowRight: 'right',
              };
              const dir = arrows[e.key];
              if (dir) {
                e.preventDefault();
                setSelection((cur) => move(cur, dir, e.shiftKey, 1, hidden));
              } else if (e.key.length === 1 || e.key === 'F2') {
                // Typing on the page edits the selected cell in place (the editor is
                // drawn over the cell on its page and takes the keys).
                e.preventDefault();
                void startEdit(true, e.key === 'F2' ? undefined : e.key);
              }
            }}
            fail={fail}
          />
        )}
        {viewMode !== 'pageLayout' && outlineShown && summary?.outline && (
          <OutlineBar
            outline={summary.outline}
            geometry={grid?.g ?? null}
            scale={grid?.scale ?? scale}
            width={size.w}
            height={size.h}
            onLevel={(rows, level) => void exec({ cmd: 'outlineShowLevel', sheet, rows, level })}
            onToggle={(rows, g) =>
              void exec({
                cmd: 'outlineDetail',
                sheet,
                rows,
                at: g.summary ?? g.first,
                show: g.collapsed,
              })
            }
          />
        )}
        <div
          className={styles.splitWrap}
          data-split={split?.orientation}
          hidden={viewMode === 'pageLayout'}
          style={
            outlineSize.left + outlineSize.top > 0
              ? { marginLeft: outlineSize.left, marginTop: outlineSize.top }
              : undefined
          }
        >
          <div
            className={styles.scroller}
            ref={scrollerRef}
            style={split ? { flex: `${split.ratio} 1 0` } : undefined}
            onScroll={(e) => {
              const el = e.currentTarget;
              setScroll({ x: el.scrollLeft / scale, y: el.scrollTop / scale });
            }}
          >
            <div className={styles.sticky}>
              <canvas
                ref={canvasRef}
                aria-hidden="true"
                data-testid="sheet-canvas"
                style={{ cursor }}
                onMouseDown={(e) => void onMouseDown(e)}
                onContextMenu={(e) => void onGridContextMenu(e)}
                onMouseMove={(e) => void onMouseMove(e)}
                onDoubleClick={(e) => void onDoubleClick(e)}
              />
              {/* Selection layer (headers, shading, border, fill handle), same frame as the click. */}
              <canvas
                ref={overlayRef}
                aria-hidden="true"
                data-testid="sheet-overlay"
                className={styles.gridOverlay}
              />
              {sizeTip && (
                <div
                  className={styles.sizeTip}
                  data-testid="resize-tip"
                  role="status"
                  style={{ left: sizeTip.x, top: Math.max(0, sizeTip.y) }}
                >
                  {sizeTip.text}
                </div>
              )}
              {/* The grid for screen readers (the canvas is pixels; WCAG 1.3.1 / 4.1.2). */}
              <SheetA11y
                host={host}
                sheet={sheet}
                view={view}
                active={selection.active}
                fmt={fmt}
                revision={revision}
                label={t('sheetEditor.grid')}
                rows={(summary?.lastRow ?? -1) + 1 || undefined}
                cols={(summary?.lastCol ?? -1) + 1 || undefined}
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
                  onAreas={(areas) => void exec({ cmd: 'setPageSetup', sheet, printAreas: areas })}
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
              {host && (
                <ErrorMarks
                  host={host}
                  view={view}
                  revision={revision}
                  rules={audit.rules}
                  toScreen={px}
                  active={selection.active}
                  onDone={refreshInfo}
                  onEdit={() => void startEdit(true)}
                  onEvaluate={(row, col) =>
                    setAuditDialog({ kind: 'evaluate', cell: { sheet, row, col } })
                  }
                  onOptions={() => setAuditDialog({ kind: 'errors' })}
                  fail={fail}
                />
              )}
              <PeerSelections boxes={peerBoxes} />
              {selRect && chart === null && headers && (
                <>
                  {/* The headers of the selected columns and rows are highlighted. */}
                  <div
                    className={styles.headerMarkCols}
                    data-testid="header-mark-cols"
                    style={{
                      left: Math.max(headers.w, selRect.x),
                      top: 0,
                      width: Math.max(0, selRect.x + selRect.w - Math.max(headers.w, selRect.x)),
                      height: headers.h,
                    }}
                  />
                  <div
                    className={styles.headerMarkRows}
                    data-testid="header-mark-rows"
                    style={{
                      left: 0,
                      top: Math.max(headers.h, selRect.y),
                      width: headers.w,
                      height: Math.max(0, selRect.y + selRect.h - Math.max(headers.h, selRect.y)),
                    }}
                  />
                </>
              )}
              {selRect && chart === null && (
                <div
                  className={painter ? styles.selectionPainter : styles.selection}
                  style={{ left: selRect.x, top: selRect.y, width: selRect.w, height: selRect.h }}
                >
                  {!edit && (
                    <div
                      className={styles.fillHandle}
                      data-testid="fill-handle"
                      title={t('sheetEditor.fillHandle')}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        drag.current = { kind: 'fill', source: rangeOf(selection) };
                      }}
                      onDoubleClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        void fillDownToData();
                      }}
                    />
                  )}
                </div>
              )}
              {selRect && chart === null && !edit && (r.r0 !== r.r1 || r.c0 !== r.c1) && (
                <QuickAnalysis
                  sheet={sheet}
                  range={toRange(r)}
                  anchor={{ x: selRect.x + selRect.w, y: selRect.y + selRect.h }}
                  exec={exec}
                  query={query}
                  insertChart={insertChartKind}
                  runCommand={(id) => run(id)}
                  onNotice={setStatus}
                  open={quickOpen}
                  onOpenChange={setQuickOpen}
                />
              )}
              {chart !== null && chartBox && (
                <div
                  className={styles.chartSelection}
                  data-testid="chart-selection"
                  style={{
                    left: chartBox.x,
                    top: chartBox.y,
                    width: chartBox.w,
                    height: chartBox.h,
                  }}
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
                  style={{
                    left: smartBox.x,
                    top: smartBox.y,
                    width: smartBox.w,
                    height: smartBox.h,
                  }}
                >
                  {smartPreview && (
                    <SmartArtLivePreview
                      svg={smartPreview}
                      style={{ left: 0, top: 0, width: '100%', height: '100%' }}
                    />
                  )}
                  <SmartArtPaneToggle
                    open={smartPane}
                    label={t('slideEditor.smartart.pane.toggle')}
                    style={{ left: 0, top: '50%' }}
                    onToggle={() => setSmartPane((v) => !v)}
                  />
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
                <TimelineFrame
                  key={`t${info.index}`}
                  info={info}
                  rect={rect}
                  selected={frameSel?.kind === 'timeline' && frameSel.index === info.index}
                  editable
                  onSelect={() => setFrameSel({ kind: 'timeline', index: info.index })}
                  onDragStart={(e, resize) => void frameDrag.current(e, 'timeline', info, resize)}
                  onSettings={() =>
                    setDialog({ kind: 'frameOptions', frames: 'timeline', index: info.index })
                  }
                  onDelete={() => void deleteFrame('timeline', info.index)}
                  onRange={(start, end) =>
                    void exec({ cmd: 'timelineSelect', sheet, index: info.index, start, end })
                  }
                  onClear={() => void exec({ cmd: 'timelineSelect', sheet, index: info.index })}
                  onLevel={(level) =>
                    void exec({ cmd: 'timelineOptions', sheet, index: info.index, level })
                  }
                  onScroll={(day) =>
                    void exec({
                      cmd: 'timelineScroll',
                      sheet,
                      index: info.index,
                      ...(day === null ? {} : { day }),
                    })
                  }
                />
              ))}
              {slicers.map(({ info, rect }) => (
                <SlicerFrame
                  key={info.index}
                  info={info}
                  rect={rect}
                  selected={frameSel?.kind === 'slicer' && frameSel.index === info.index}
                  editable
                  onSelect={() => setFrameSel({ kind: 'slicer', index: info.index })}
                  onDragStart={(e, resize) => void frameDrag.current(e, 'slicer', info, resize)}
                  onSettings={() =>
                    setDialog({ kind: 'frameOptions', frames: 'slicer', index: info.index })
                  }
                  onDelete={() => void deleteFrame('slicer', info.index)}
                  onChoose={(items) => chooseSlicerItems(info, items)}
                  onClear={() =>
                    void exec({ cmd: 'slicerSelect', sheet, index: info.index, items: [] })
                  }
                />
              ))}
              {chart !== null && chartBox && chartPivot && host && (
                <PivotChartFilters
                  pivot={chartPivot.info}
                  rect={chartBox}
                  onApply={async (field, hidden) => {
                    await host.exec({
                      cmd: 'pivotFieldItems',
                      sheet: chartPivot.sheet,
                      index: chartPivot.info.index,
                      field,
                      hidden,
                      labels: pivotLabels,
                    });
                    await refreshInfo();
                  }}
                />
              )}
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
                  data-testid="fill-preview"
                  style={{
                    left: fillRect.x,
                    top: fillRect.y,
                    width: fillRect.w,
                    height: fillRect.h,
                  }}
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
                    setPick((p) => (p ? null : { items: dvList, index: 0 }));
                  }}
                >
                  ▾
                </button>
              )}
              {pick && activeRect && (
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
                  {pick.items.map((v, i) => (
                    <button
                      key={v}
                      type="button"
                      role="option"
                      aria-selected={i === pick.index}
                      className={styles.menuItem}
                      data-current={i === pick.index || undefined}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        setPick(null);
                        choosePick(v);
                      }}
                    >
                      {v}
                    </button>
                  ))}
                </div>
              )}
              {copyMark?.sheet === sheet && copyBox && (
                <div
                  className={styles.copyMark}
                  data-testid="copy-mark"
                  style={{
                    left: copyBox.x,
                    top: copyBox.y,
                    width: copyBox.w,
                    height: copyBox.h,
                  }}
                />
              )}
              {edit &&
                shownRefBoxes.map((b, i) => (
                  <div
                    key={i}
                    className={styles.refBox}
                    data-testid="ref-box"
                    data-color={b.color}
                    style={{ left: b.x, top: b.y, width: b.w, height: b.h, borderColor: b.color }}
                  />
                ))}
              {editHere && edit && activeRect && editorRefs.length > 0 && (
                <div
                  className={styles.cellEditorMirror}
                  aria-hidden="true"
                  data-testid="cell-editor-mirror"
                  style={{
                    left: activeRect.x,
                    top: activeRect.y,
                    minWidth: activeRect.w,
                    lineHeight: `${Math.max(1, activeRect.h - 4)}px`,
                    fontSize: 11 * scale,
                  }}
                >
                  {mirrorParts(edit.text, editorRefs).map((p, i) =>
                    p.color ? (
                      <span key={i} style={{ color: p.color }} data-ref-color={p.color}>
                        {p.text}
                      </span>
                    ) : (
                      <span key={i}>{p.text}</span>
                    ),
                  )}
                </div>
              )}
              {editHere && activeRect && fnPick && fnPick.items.length > 0 && (
                <div
                  className={styles.fnList}
                  role="listbox"
                  aria-label={t('sheetEditor.functionList')}
                  data-testid="function-list"
                  style={{
                    left: activeRect.x,
                    top: activeRect.y + activeRect.h * editLines + 2,
                  }}
                >
                  {fnPick.items.map((n, i) => (
                    <div
                      key={n}
                      role="option"
                      aria-selected={i === fnPick.index}
                      data-current={i === fnPick.index || undefined}
                      className={styles.fnItem}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        acceptFunction(n);
                      }}
                    >
                      {n}
                    </div>
                  ))}
                </div>
              )}
              {editHere && activeRect && hint && !(fnPick && fnPick.items.length > 0) && (
                <div
                  className={styles.argHint}
                  role="tooltip"
                  data-testid="argument-hint"
                  style={{
                    left: activeRect.x,
                    top: activeRect.y + activeRect.h * editLines + 2,
                  }}
                >
                  {hint.name}(
                  {hint.args.map((a, i) => (
                    <span key={i}>
                      {i > 0 ? ', ' : ''}
                      {a.current ? <b data-testid="argument-current">{a.text}</b> : a.text}
                    </span>
                  ))}
                  )
                </div>
              )}
              {!pageEditing && (
                <textarea
                  ref={inputRef}
                  rows={1}
                  className={editHere ? styles.cellEditorActive : styles.cellEditor}
                  data-coloured={editorRefs.length > 0 || undefined}
                  onFocus={() => {
                    lastInput.current = 'cell';
                  }}
                  aria-label={t('sheetEditor.cellEditor')}
                  style={
                    activeRect && edit
                      ? cellEditorLayout(activeRect, edit, editLines).style
                      : activeRect
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
                  onSelect={(e) => setCaret(e.currentTarget.selectionEnd ?? 0)}
                  onInput={(e) =>
                    onInput(
                      e.currentTarget.value,
                      e.currentTarget,
                      (e.nativeEvent as InputEvent).inputType,
                    )
                  }
                  onCompositionStart={() => {
                    composing.current = true;
                    if (!edit) setEdit({ pos: selection.active, text: '', inPlace: false, sheet });
                  }}
                  onCompositionEnd={(e) => {
                    composing.current = false;
                    onInput(e.currentTarget.value, e.currentTarget);
                  }}
                />
              )}
            </div>
            <div style={{ width: content.w * scale, height: content.h * scale }} />
          </div>
          {split && glyphs && (
            <SplitPane
              host={host}
              glyphs={glyphs}
              images={cellImages}
              sheet={sheet}
              split={split}
              content={content}
              revision={revision}
              selection={{ range: toRange(r), active: selection.active }}
              labels={{ pane: t('windows.splitPane'), bar: t('windows.splitBar') }}
              onChange={(c) => setSplit((cur) => cur && updateSplit(cur, c))}
              onRemove={() => setSplit(null)}
              onSelect={(cell, extend) => {
                setExtra([]);
                setChart(null);
                setSelection(extend ? { ...selection, focus: cell } : single(cell));
                focusInput();
              }}
              fail={fail}
            />
          )}
        </div>
        {fillUi && fillUi.sel === selection && !edit && (
          <PasteOptionsButton
            key={fillUi.n}
            testId="autofill-options"
            label={t('sheetEditor.autoFill.button')}
            anchor={() => {
              const c = canvasRef.current?.getBoundingClientRect();
              return c && selRect
                ? { x: c.left + selRect.x + selRect.w, y: c.top + selRect.y + selRect.h }
                : null;
            }}
            options={(['copy', 'series', 'formats', 'values'] as const).map((id) => ({
              id,
              label: t(`sheetEditor.autoFill.${id}`),
            }))}
            current={fillUi.mode}
            onChoose={(id) => void switchFill(id as AutoFillMode)}
          />
        )}
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
        {iconsOpen && (
          <IconPanel
            onInsert={(icons) =>
              insertSheetIcons(
                (cmd) => exec(cmd),
                { sheet, row: selection.active.row, col: selection.active.col },
                icons,
              ).catch(fail)
            }
            onClose={() => {
              setIconsOpen(false);
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
                        vbaRequest={vbaRequest}
                        onVbaRequestHandled={vbaRequestHandled}
                        vbaDraft={macroViewer.draft}
                        onVbaDraftHandled={macroViewer.draftHandled}
                        {...(platform.scriptSchedules
                          ? { schedules: platform.scriptSchedules }
                          : {})}
                      />
                    ),
                  } satisfies TaskPaneDef,
                ]
              : []),
            ...(queries.pane ? [queries.pane] : []),
            ...(chartFormatOpen
              ? [
                  {
                    id: 'chartFormat',
                    title: t('chartFormat.title'),
                    icon: <SlidersHorizontal size={14} aria-hidden />,
                    closeTestId: 'chart-format-close',
                    content: (
                      <ChartFormatPane
                        info={chart !== null ? chartInfo?.elements : undefined}
                        apply={sendChartElements}
                      />
                    ),
                  } satisfies TaskPaneDef,
                ]
              : []),
            ...(ai.active
              ? [
                  {
                    id: 'ai',
                    title: t('ai.title'),
                    icon: <Sparkles size={14} aria-hidden />,
                    closeTestId: 'ai-close',
                    content: ai.content,
                  } satisfies TaskPaneDef,
                ]
              : []),
            ...(thesaurus.active
              ? [
                  {
                    id: 'thesaurus',
                    title: t('thesaurus.title'),
                    icon: <BookA size={14} aria-hidden />,
                    closeTestId: 'thesaurus-close',
                    content: thesaurus.content,
                  } satisfies TaskPaneDef,
                ]
              : []),
            ...(info ? addins.panes : []),
          ]}
          onClose={(id) =>
            addins.close(id)
              ? undefined
              : id === 'queries'
                ? queries.closePane()
                : id === 'thesaurus'
                  ? thesaurus.close()
                  : id === 'ai'
                    ? ai.close()
                    : id === 'comments'
                      ? setCommentsOpen(false)
                      : id === 'script'
                        ? setScriptOpen(false)
                        : id === 'chartFormat'
                          ? setChartFormatOpen(false)
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
      <div className={styles.tabs}>
        {/* Only tabs inside the tab list (WAI-ARIA tabs pattern); "add sheet" sits beside it. */}
        <div className={styles.tabList} role="tablist" aria-label={t('sheetEditor.tabs')}>
          {info?.sheets.map((s, i) =>
            s.hidden ? null : renaming === i ? (
              <input
                key={i}
                className={styles.tabRename}
                defaultValue={s.name}
                autoFocus
                // The whole name is selected: typing replaces it.
                onFocus={(e) => e.currentTarget.select()}
                onBlur={(e) => {
                  // The grid takes the focus back on the frame after the clicks of the
                  // double click: the name box keeps it.
                  if (performance.now() - renameOpened.current < 300) {
                    const box = e.currentTarget;
                    requestAnimationFrame(() => box.focus());
                    return;
                  }
                  setRenaming(null);
                }}
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
                onMouseDown={(e) => {
                  // Picking a reference: the editor keeps the caret (no focus change).
                  if (pointable()) e.preventDefault();
                }}
                onClick={(e) => {
                  if (pointable()) {
                    pointSheet(i);
                    return;
                  }
                  setSheet(i);
                  setSelection(single({ row: 0, col: 0 }));
                  setChart(null);
                  scrollerRef.current?.scrollTo(0, 0);
                  // The second click of a double click leaves the focus to the name box
                  // that opens (its deferred focus would close the rename at once).
                  if (e.detail < 2) focusInput();
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
        </div>
        <button
          type="button"
          className={styles.tabAdd}
          aria-label={t('sheetEditor.addSheet')}
          onClick={() => {
            run('sheet.add');
            // The keyboard stays on the grid (Ctrl+PgUp / PgDn, typing).
            focusInput();
          }}
        >
          <Plus size={14} />
        </button>
      </div>
      <StatusBar
        customize={{
          label: t('statusBar.customize'),
          items: STATUS_ITEMS.sheet.map((d) => ({
            id: d.id,
            label: t(`statusBar.items.${d.id}`),
            checked: statusBar.shown(d.id),
            ...(statValues[d.id] !== undefined ? { value: statValues[d.id] } : {}),
          })),
          onToggle: statusBar.toggle,
          resetLabel: t('statusBar.reset'),
          onReset: statusBar.reset,
        }}
        left={
          <>
            {(status || statusBar.shown('cellMode')) && (
              <span data-testid="sheet-status" data-mode={cellMode}>
                {status ||
                  (painter
                    ? t('sheetEditor.painterActive')
                    : chart !== null
                      ? t('sheetEditor.chart.selected')
                      : statValues.cellMode)}
              </span>
            )}
            {info?.macros && statusBar.shown('macro') && (
              <span data-testid="status-macro">{t('statusBar.macro')}</span>
            )}
            {filterMode && statusBar.shown('filter') && (
              <span data-testid="status-filter">{t('statusBar.filterMode')}</span>
            )}
            {info && <ProtectionStatus info={protection.info} />}
            {windowTools.shared && (
              <span data-testid="status-shared-window">{t('windows.sharedWindow')}</span>
            )}
            {windowTools.paired && (
              <span data-testid="status-side-by-side">
                {t('windows.sideBySideWith', {
                  name: windowTools.paired.title || t('windows.untitled'),
                })}
              </span>
            )}
          </>
        }
        right={
          <>
            <CalcStatus state={audit.calc} onCalculate={() => void auditCalculate('workbook')} />
            {statsPending && (
              <span data-testid="status-calculating">{t('statusBar.calculating')}</span>
            )}
            {STAT_ITEMS.map(([id, label]) =>
              statusBar.shown(id) && statValues[id] !== undefined ? (
                <span key={id} data-testid={`status-${STAT_TEST_IDS[id] ?? id}`}>
                  {t(label)}: {statValues[id]}
                </span>
              ) : null,
            )}
            {info?.encrypted && (
              <span data-testid="sheet-encrypted">{t('security.protected')}</span>
            )}
            {statusBar.shown('viewButtons') && (
              <StatusButtons
                label={t('statusBar.views')}
                testId="status-views"
                exclusive
                buttons={(
                  [
                    ['normal', 'sheetViews.normal', <Grid3x3 key="i" size={14} />],
                    ['pageLayout', 'sheetViews.pageLayout', <FileText key="i" size={14} />],
                    [
                      'pageBreakPreview',
                      'sheetViews.pageBreakPreview',
                      <SeparatorHorizontal key="i" size={14} />,
                    ],
                  ] as const
                ).map(([mode, label, icon]) => ({
                  id: mode,
                  label: t(label),
                  icon,
                  pressed: viewMode === mode,
                  onClick: () => run(`view.${mode}`),
                }))}
              />
            )}
            {(statusBar.shown('zoomSlider') || statusBar.shown('zoom')) && (
              <StatusZoom
                zoom={zoom}
                min={ZOOMS[0] ?? 0.5}
                max={ZOOMS[ZOOMS.length - 1] ?? 2}
                slider={statusBar.shown('zoomSlider')}
                percent={statusBar.shown('zoom')}
                onZoom={setZoom}
                onStep={(d) => run(d > 0 ? 'view.zoomIn' : 'view.zoomOut')}
                onPercent={() => setZoom(1)}
                labels={{
                  slider: t('statusBar.zoomSlider'),
                  zoomIn: t('sheetEditor.zoomIn'),
                  zoomOut: t('sheetEditor.zoomOut'),
                  percent: t('statusBar.zoomPercent', { value: Math.round(zoom * 100) }),
                }}
              />
            )}
          </>
        }
      />
      {windowTools.element}
      {sheetMenu.element}
      {shortcuts.element}
      {ribbonCustom.element}
      {addins.element}
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
            // The split button applies this colour next time.
            if (c) setLast(menu.kind === 'fillColor' ? { fill: c } : { font: c });
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
          range={
            summary?.autoFilter
              ? {
                  r0: summary.autoFilter[0],
                  c0: summary.autoFilter[1],
                  r1: summary.autoFilter[2],
                  c1: summary.autoFilter[3],
                }
              : { r0: 0, c0: filterMenu.col, r1: 0, c1: filterMenu.col }
          }
          onNotice={setStatus}
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
          tab={dialog.tab}
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
      {dialog?.kind === 'size' && (
        <SizeDialog
          rows={dialog.rows}
          initial={dialog.initial}
          onClose={common.onClose}
          onApply={(size) => {
            const rows = dialog.rows;
            common.onClose();
            void onAllRanges((range) => ({
              cmd: 'resize',
              sheet,
              rows,
              first: rows ? range.r0 : range.c0,
              last: rows ? range.r1 : range.c1,
              size,
              chars: !rows,
            }));
          }}
        />
      )}
      {dialog?.kind === 'cells' && (
        <CellsDialog
          insert={dialog.insert}
          onClose={common.onClose}
          onApply={(shift) => {
            const insert = dialog.insert;
            common.onClose();
            if (shift === 'row' || shift === 'col') {
              const id = `cells.${insert ? 'insert' : 'delete'}${shift === 'row' ? 'Row' : 'Col'}`;
              run(id);
              return;
            }
            void exec({
              cmd: 'shiftCells',
              sheet,
              range: toRange(r),
              down: shift === 'down' || shift === 'up',
              insert,
            });
          }}
        />
      )}
      {dialog?.kind === 'table' && (
        <CreateTableDialog
          range={dialog.range}
          headers={dialog.headers}
          onClose={common.onClose}
          onApply={(text, headers) => {
            const at = parseA1(text);
            common.onClose();
            if (!at) return;
            void createTable(rangeOf(at), headers);
          }}
        />
      )}
      {dialog?.kind === 'replaceWarn' && (
        <ConfirmDialog
          title={t('sheetEditor.replaceWarn.title')}
          message={t('sheetEditor.replaceWarn.message')}
          testId="replace-warning"
          onClose={common.onClose}
          onOk={() => {
            const { run } = dialog;
            common.onClose();
            run();
          }}
        />
      )}
      {dialog?.kind === 'mergeWarn' && (
        <ConfirmDialog
          title={t('sheetEditor.mergeWarn.title')}
          message={t('sheetEditor.mergeWarn.message')}
          testId="merge-warning"
          onClose={common.onClose}
          onOk={() => {
            const { center, across } = dialog;
            common.onClose();
            void exec({
              cmd: 'merge',
              sheet,
              range: toRange(r),
              center,
              ...(across ? { across } : {}),
            });
          }}
        />
      )}
      {dialog?.kind === 'sort' && <SortDialog {...common} range={toRange(r)} />}
      {dialog?.kind === 'series' && (
        <SeriesDialog
          {...common}
          range={toRange(r)}
          firstIsDate={dialog.firstIsDate}
          decimal={decimal}
        />
      )}
      {dialog?.kind === 'editLinks' && <EditLinksDialog {...common} platform={platform} />}
      {dialog?.kind === 'nameManager' && (
        <NameManagerDialog
          {...common}
          sheets={info?.sheets.map((x) => x.name) ?? []}
          refersTo={absoluteRef(info?.sheets[sheet]?.name ?? '', toRange(r))}
        />
      )}
      {dialog?.kind === 'defineName' && (
        <NameEditDialog
          {...common}
          sheets={info?.sheets.map((x) => x.name) ?? []}
          refersTo={absoluteRef(info?.sheets[sheet]?.name ?? '', toRange(r))}
        />
      )}
      {dialog?.kind === 'createNames' && (
        <CreateNamesDialog
          {...common}
          range={toRange(r)}
          guess={dialog.guess}
          onCreated={(names) => setStatus(t('sheetNames.created', { count: names.length }))}
        />
      )}
      {dialog?.kind === 'sortWarning' && (
        <SortWarningDialog
          onClose={() => setDialog(null)}
          onSort={(expand) => {
            const d = dialog;
            setDialog(null);
            void (async () => {
              // Expand: the block around the first cell; otherwise the selected cells.
              const at = expand
                ? { r0: d.range.r0, c0: d.range.c0, r1: d.range.r0, c1: d.range.c0 }
                : d.range;
              const g = await query<SortRegion>({ cmd: 'sortRegion', sheet, range: at });
              if (!g) return;
              await exec({
                cmd: 'sort',
                sheet,
                range: g.range,
                keys: [{ col: d.col, descending: d.descending }],
                hasHeader: g.hasHeader,
              });
              focusInput();
            })();
          }}
        />
      )}
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
      {macroViewer.open && (
        <MacroViewer
          load={loadMacros}
          onConvert={macroViewer.convert}
          onClose={macroViewer.close}
        />
      )}
      {readAloud.bar}
      {dictation.bar}
      {translate.pane}
      {queries.element}
      {dataModel.element}
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
      {bgTarget && (
        <BackgroundRemovalMode
          open
          labels={pictureTabLabels(t)}
          preview={async (spec) =>
            (await exec<BackgroundPreviewValue>({
              cmd: 'cellImageBackgroundPreview',
              ...bgTarget,
              ...spec,
            })) ?? null
          }
          onKeep={(spec) => {
            const target = bgTarget;
            setBgTarget(null);
            void exec({ cmd: 'cellImageRemoveBackground', ...target, ...spec, source: false });
          }}
          onDiscard={() => setBgTarget(null)}
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
      {dialog?.kind === 'sparklineData' && sparkGroup && (
        <EditSparklineDataDialog
          sheet={sheet}
          group={sparkGroup}
          {...(dialog.single ? { single: selection.active } : {})}
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
      {fnDialog && (
        <InsertFunctionDialog
          query={query}
          onInsert={(name) => {
            // `=NAME()` with the caret between the parentheses.
            const text = `=${name}()`;
            proposeFormula(selection.active, text, text.length - 1, text.length - 1);
          }}
          onClose={() => {
            setFnDialog(false);
            focusInput();
          }}
        />
      )}
      {recommendOpen && (
        <RecommendedChartsDialog
          sheet={sheet}
          range={toRange(r)}
          query={query}
          onInsert={insertChartKind}
          onClose={() => {
            setRecommendOpen(false);
            focusInput();
          }}
        />
      )}
      {smartPane && smartArt !== null && smartInfo && (
        <SmartArtTextPane
          key={`${sheet}:${smartArt}`}
          items={smartInfo.items}
          node={smartNode}
          onNode={setSmartNode}
          onCommit={(items) => void exec({ cmd: 'setSmartArt', sheet, index: smartArt, items })}
          onClose={() => {
            setSmartPane(false);
            focusInput();
          }}
          anchor={() =>
            document.querySelector('[data-testid="smartart-selection"]')?.getBoundingClientRect() ??
            null
          }
          labels={smartArtDesignLabels(t).pane}
        />
      )}
      {smartArtDialog && (
        <SmartArtDialog
          mode={smartArtDialog.mode}
          initial={smartArtDialog.initial}
          loadPreviews={loadSmartArtPreviews}
          pickPicture={pickSmartArtPicture}
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
      {dialog?.kind === 'frameInsert' && host && (
        <InsertFramesDialog
          pivot={dialog.pivot}
          kind={dialog.frames}
          onClose={() => setDialog(null)}
          onInsert={async (fields) => {
            try {
              for (const field of fields)
                await host.exec({
                  cmd: dialog.frames === 'slicer' ? 'insertPivotSlicer' : 'insertTimeline',
                  sheet,
                  pivot: dialog.pivot.index,
                  field,
                  labels: pivotLabels,
                });
            } finally {
              await refreshInfo();
            }
          }}
        />
      )}
      {dialog?.kind === 'frameOptions' && host && frameDialog && (
        <FrameOptionsDialog
          kind={dialog.frames}
          caption={frameDialog.caption}
          columns={'columns' in frameDialog ? frameDialog.columns : undefined}
          style={frameDialog.style ?? null}
          connections={frameDialog.pivots ?? []}
          customStyles={frameDialog.customStyles}
          show={'periods' in frameDialog ? timelineShowOf(frameDialog) : undefined}
          onClose={() => setDialog(null)}
          onApply={async (o) => {
            const index = dialog.index;
            const was = (frameDialog.pivots ?? []).filter((p) => p.connected).map((p) => p.name);
            const rewired =
              !!o.pivots &&
              (o.pivots.length !== was.length || o.pivots.some((n) => !was.includes(n)));
            const cmds: SheetCommand[] =
              dialog.frames === 'slicer'
                ? [
                    {
                      cmd: 'slicerOptions',
                      sheet,
                      index,
                      caption: o.caption,
                      columns: o.columns,
                      style: o.style,
                    },
                  ]
                : [
                    {
                      cmd: 'timelineOptions',
                      sheet,
                      index,
                      caption: o.caption,
                      style: o.style,
                      ...(o.show
                        ? {
                            showHeader: o.show.header,
                            showSelectionLabel: o.show.selectionLabel,
                            showTimeLevel: o.show.timeLevel,
                            showScrollbar: o.show.scrollbar,
                          }
                        : {}),
                    },
                  ];
            if (rewired && o.pivots)
              cmds.push({
                cmd: dialog.frames === 'slicer' ? 'slicerConnect' : 'timelineConnect',
                sheet,
                index,
                pivots: o.pivots,
                labels: pivotLabels,
              });
            try {
              // One undo step for the options and the connections.
              await host.exec({ cmd: 'batch', cmds });
            } finally {
              await refreshInfo();
            }
          }}
        />
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
