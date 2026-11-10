import { GlyphStore, ImageStore, PX_PER_PT } from '@blueoffice/canvas-host';
import {
  CommandRegistry,
  FileProtectionUi,
  ProtectionStatus,
  useFileProtection,
  MacroBanner,
  PasswordDialog,
  PdfExportDialog,
  SaveAsTypeDialog,
  FileBackstage,
  useWindowTools,
  SavePasswordPanel,
  SmartArtDialog,
  SymbolPanel,
  PrintPanel,
  clipboardHistory,
  pictureTabLabels,
  planOpen,
  registerPictureCommands,
  ReadAloudLink,
  registerReadAloudCommands,
  DictationLink,
  registerDictationCommands,
  useDictation,
  registerTranslateCommands,
  TranslateLink,
  translatedName,
  useTranslate,
  type TranslateAction,
  saveName,
  taskPaneLabels,
  objectPlatform,
  objectToolsLabels,
  registerObjectCommands,
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
} from '@blueoffice/editor';
import { slideReadRequest } from './readAloud';
import { slideDictationTarget } from './dictation';
import {
  applyDeckTexts,
  editedSelection,
  readDeckTexts,
  spliceEdited,
  translateDeckTexts,
  translateSelectedShapes,
} from './translate';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import type {
  ClipboardContent,
  SlideIssue,
  DocInfo,
  Platform,
  SlideAnimEffect,
  SlideChartInfo,
  SlideChartKind,
  SlideCommand,
  SlideEditResult,
  SlidePeer,
  SlidePreview,
  SlideShapeInfo,
  SlideShowInfo,
  SlideTextEdit,
  SlideTransitionKind,
} from '@blueoffice/platform';
import {
  DEFAULT_SHOW_SETUP,
  PASSWORD_REQUIRED,
  THEME_MIME,
  MODEL3D_FILES,
  WRONG_PASSWORD,
  mentionFields,
  mimeFor,
  notifyMentions,
  openExtensions,
  type DisplayTarget,
  type MediaCaptureSession,
  type SlideRecordingInput,
  type SlideLinkAction,
  type SlideShowSetup,
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
  usePictureTools,
  useResolvedTheme,
  toBase64 as pictureBase64,
  PictureActionsHub,
  useObjectTools,
  ObjectActionsHub,
  TaskPaneHost,
  useTaskPaneLayout,
  type RibbonTab,
  type TaskPaneDef,
  type ThemeSetting,
  MiniToolbar,
  SLIDE_MENUS,
  isContextMenuKey,
  menuPointOf,
  registryMenuLookup,
  resolveMenu,
  ribbonMenuLookup,
  type RibbonItem,
  type SlideMenuContext,
} from '@blueoffice/ui';
import {
  AArrowDown,
  AArrowUp,
  FileSpreadsheet,
  RefreshCw,
  AlignCenter,
  AlignCenterHorizontal,
  AlignCenterVertical,
  AlignEndHorizontal,
  AlignEndVertical,
  AlignHorizontalDistributeCenter,
  AlignLeft,
  AlignRight,
  AlignStartHorizontal,
  AlignStartVertical,
  AlignVerticalDistributeCenter,
  ArrowDown,
  ArrowDownToLine,
  ArrowLeftToLine,
  ArrowRight,
  ArrowRightToLine,
  ArrowUp,
  ArrowUpToLine,
  ArrowRightLeft,
  Baseline,
  Blend,
  Bold,
  BookOpen,
  BringToFront,
  Cast,
  ChartArea,
  ChartBar,
  ChartColumn,
  ChartLine,
  ChartPie,
  Circle,
  CircleDot,
  Clapperboard,
  ClipboardCheck,
  History,
  Layers,
  SpellCheck,
  ClipboardPaste,
  Copy,
  Columns3,
  CopyPlus,
  CornerDownRight,
  Eraser,
  Eye,
  FileDown,
  FileImage,
  FileStack,
  Files,
  Film,
  KeyRound,
  Lock,
  FolderOpen,
  Group,
  Highlighter,
  ImagePlus,
  IndentDecrease,
  IndentIncrease,
  Italic,
  LayoutGrid,
  List,
  ListOrdered,
  ListTree,
  NotebookText,
  Languages,
  StickyNote,
  LogIn,
  LogOut,
  Merge,
  MonitorPlay,
  PaintBucket,
  PanelRight,
  PanelTop,
  Network,
  Omega,
  Pencil,
  PenLine,
  Play,
  Presentation,
  Printer,
  Redo2,
  Rows3,
  Save,
  Scissors,
  SendToBack,
  Slash,
  Sparkles,
  Spline,
  Split,
  Square,
  SquarePlus,
  Strikethrough,
  Table,
  Trash,
  Triangle,
  Type,
  Underline,
  Undo2,
  Ungroup,
  MessageSquareText,
  Shapes,
  Volume2,
  Mic,
  SquareTerminal,
  AppWindow,
  RotateCcw,
  ArrowDownUp,
  Columns2,
  Maximize2,
  MessageSquare,
} from 'lucide-react';
import { recordEngine, ScriptPane, ScriptRecorder, slideScriptHost } from '@blueoffice/script';
import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';
import { AnimationPane } from './AnimationPane';
import { SelectionPane } from './SelectionPane';
import { AudienceLink } from './audience';
import { AudienceView } from './AudienceView';
import { ChartDialog, type ChartDraft } from './ChartDialog';
import {
  SaveChartTemplateDialog,
  layoutTexts,
  slideChartDesignGroups,
  slideGalleryLoader,
} from './chartStyles';
import { SlideCheckDocumentDialog } from './inspect';
import { SlideController } from './controller';
import { SlideSpellDialog, SlideSpellLayer, useSlideSpelling } from './spelling';
import { SlideVersionsPane } from './compareVersions';
import { fromEditorText, hexOf, nudgeCommand, toEditorText } from './editing';
import {
  SlideClipboardUi,
  selectionCorner,
  slidePasteChoices,
  type SlidePasteChoice,
} from './pasteOptions';
import { ExportDialog, slidePng, type ExportPngOptions } from './ExportDialog';
import { ExportVideoDialog } from './exportVideo';
import { exportShow, type ShowExportRequest } from './video/export';
import { ExportDocsDialog, exportPlan, type ExportDocsState } from './exportDocs';
import { MediaButtons } from './MediaLayer';
import { MediaActionsHub, registerMediaCommands, useMediaTools } from './mediaPlayback';
import { DrawActionsHub, drawTab, registerDrawCommands, useDrawTools } from './draw/drawTab';
import type { ShowPos } from './player';
import { fit, formatElapsed, showAction, step, type ShowAction } from './show';
import { CustomShowsDialog } from './customShows';
import { InkLayer, type PresenterTool } from './ink';
import {
  InactivityTimer,
  NO_ZOOM,
  RehearsalClock,
  kioskAllows,
  newRun,
  playOrder,
  runGoto,
  runLink,
  runPosOf,
  runStep,
  startAt,
  toggleCover,
  type RunPos,
  type ShowRun,
  type ZoomTransform,
} from './presenting';
import { SetupShowDialog } from './setupShow';
import { slideShowTab } from './showTab';
import {
  EndShowDialog,
  RehearsalBar,
  ShowSurface,
  SlideGrid,
  type EndShowState,
} from './showTools';
import { ShowCache, ShowStage } from './ShowStage';
import { ShowRecorder, recordingInput } from './recording';
import {
  CameraPreview,
  DEFAULT_RECORD_OPTIONS,
  RecordSetupDialog,
  RecordingBar,
  type RecordOptions,
} from './recordTools';
import { SlideCanvas } from './SlideCanvas';
import { zipStore } from './zip';
import styles from './SlideEditor.module.css';
import { SlideOverlay, type TextEditing } from './SlideOverlay';
import { EditPointsOverlay } from './editPoints';
import { registerShapeTools, shapeFormatTab } from './shapeFormat';
import { Model3dLayer } from './model3d/Model3dLayer';
import {
  ZoomInsertDialog,
  insertCommand as zoomInsertCommand,
  registerZoomCommands,
  zoomInsertItems,
  zoomTab,
  type ZoomInsertKind,
  type ZoomState,
} from './zoom';
import {
  Model3dAltTextDialog,
  Model3dIcon,
  model3dTab,
  registerModel3dCommands,
  type Model3dState,
} from './model3d/model3dTab';
import { registerShapeEffects, shapeEffectsGroup, warpPreviewLoader } from './shapeEffects';
import {
  RenameDialog,
  SlideSizeDialog,
  designTab,
  masterState,
  masterTab,
  masterThumbLabel,
  registerDesignChoices,
  registerMasterCommands,
  type MasterState,
} from './masterView';
import masterStyles from './masterView.module.css';
import { NotesPageView } from './notesPage';
import { OutlineView } from './outlineView';
import { PeerBadges, peersBySlide } from './peerBadges';
import {
  HandoutSlots,
  pageMasterState,
  pageMasterTab,
  registerPageMasterCommands,
  type PageMasterState,
} from './pageMasterView';
import {
  SectionHeader,
  SectionRenameDialog,
  paneEntries,
  registerSectionCommands,
  sectionState,
  sectionsGroup,
  type SectionState,
} from './sections';

/**
 * Contexts whose tab is activated when they appear (CORE-UI-001);
 * shapes and tables are typed in with the Home tab: their tabs only show.
 */
const AUTO_CONTEXTS = RIBBON_CONTEXTS.filter((c) => c !== 'shape' && c !== 'table');

export interface BlueSlideEditorProps {
  platform: Platform;
  language?: Language;
  /** Open as the audience screen of a presenter window (channel id). */
  audience?: string;
  /** Light, dark or follow the OS (the embedding host sets it, prompt §6A.2). */
  theme?: ThemeSetting;
}

export type SlideView =
  'normal' | 'sorter' | 'reading' | 'show' | 'presenter' | 'outline' | 'notesPage';

/** State passed to registry commands. */
/** An open BlueSlide context menu (CORE-UI-003). */
type SlideMenu = OpenContextMenu & { context: SlideMenuContext };

/** Commands of the slide mini toolbar (text being edited), from the Home tab. */
const SLIDE_MINI = [
  'format.bold',
  'format.italic',
  'format.underline',
  'format.grow',
  'format.shrink',
  'format.color',
  'format.highlight',
  'format.bullets',
  'format.numbering',
  'format.alignLeft',
  'format.alignCenter',
  'format.alignRight',
];

/** What a slide selection is, for its context menu. */
export function slideMenuContext(r: {
  selection: { kind: string }[];
  table?: unknown;
  textEdit?: unknown;
}): SlideMenuContext {
  if (r.textEdit) return r.table ? 'table' : 'text';
  if (r.table) return 'table';
  const s = r.selection;
  if (s.length === 0) return 'canvas';
  if (s.every((x) => x.kind === 'picture')) return 'picture';
  if (s.length === 1 && s[0]?.kind === 'chart') return 'chart';
  if (s.length === 1 && s[0]?.kind === 'table') return 'table';
  return 'shape';
}

export interface SlideEditorState
  extends MasterState, SectionState, PageMasterState, Model3dState, ZoomState {
  hasDeck: boolean;
  current: number;
  hasSelection: boolean;
  multi: boolean;
  hasText: boolean;
  isGroup: boolean;
  canUndo: boolean;
  canRedo: boolean;
  /** What the internal clipboard holds. */
  clip?: 'shapes' | 'slides';
  /** A table is being edited (cells selected). */
  hasTable?: boolean;
  firstRow?: boolean;
  bandRow?: boolean;
  /** The selection is one chart. */
  isChart?: boolean;
  /** The selection is one SmartArt. */
  isSmartArt?: boolean;
  /** Every selected shape is a picture (picture tools, CORE-PIC-001). */
  isPicture?: boolean;
  /** Transition kind of the current slide (`none` without one). */
  transition?: string;
  /** One plain shape is selected (edit points). */
  isShape?: boolean;
  /** Two or more top-level shapes are selected (merge shapes). */
  mergeable?: boolean;
  /** The point editor is open on the selected shape. */
  editingPoints?: boolean;
}

const PPTX_MIME = 'application/vnd.openxmlformats-officedocument.presentationml.presentation';
/** File name without the extension of a deck variant (CORE-FMT-001). */
const deckBase = (name: string) => name.replace(/\.(pptx|pptm|potx|potm|ppsx|ppsm)$/i, '');

/** Files offered by "Insert video / audio" (played when the platform has the codec). */
const MEDIA_FILES = [
  '.mp4',
  '.m4v',
  '.webm',
  '.ogv',
  '.mov',
  '.mp3',
  '.m4a',
  '.wav',
  '.ogg',
  '.oga',
];

/** Ribbon transitions (the pane offers every kind). */
const RIBBON_TRANSITIONS: SlideTransitionKind[] = [
  'none',
  'fade',
  'push',
  'wipe',
  'cover',
  'split',
];
const ENTRANCES: SlideAnimEffect[] = ['appear', 'fade', 'flyIn', 'wipe'];

/** Show position; `at` = place in the playing order (SLIDE-SHOW-001). */
type Pos = ShowPos & { from: number | null; at?: number };

/** Dialog fields of an existing chart. */
function chartDraftOf(c: SlideChartInfo): ChartDraft {
  return {
    kind: c.kind === 'other' ? 'column' : c.kind,
    title: c.title ?? '',
    legend: c.legend ?? 'none',
    data: c.data,
  };
}

type ColorTarget = 'font' | 'fill' | 'line' | 'highlight' | 'cell';

/** Attribute marking BlueSlide's own copies in the system clipboard's HTML. */
const CLIP_MARK = 'data-bo-slide-clip';

function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => `&#${ch.charCodeAt(0)};`);
}

function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Line spacing multiples offered in the format bar (§21.1.2.2.10 lnSpc). */
const LINE_SPACINGS = [1, 1.15, 1.5, 2, 2.5, 3];
/** Typefaces offered in the format bar (any other typeface of the deck is kept). */
const FONTS = [
  'Arial',
  'Calibri',
  'Times New Roman',
  'Tahoma',
  'Carlito',
  'Liberation Sans',
  'Tinos',
];

export function BlueSlideEditor(props: BlueSlideEditorProps) {
  const [i18n] = useState(() => createI18n(props.language));
  return (
    <I18nextProvider i18n={i18n}>
      {props.audience ? (
        <AudienceView platform={props.platform} channel={props.audience} />
      ) : (
        <SlideEditorInner {...props} />
      )}
    </I18nextProvider>
  );
}

/**
 * Size of an element, tracked with ResizeObserver. Returns a callback ref so
 * the observer follows the element when views swap it out.
 */
function useSize(): [(el: HTMLElement | null) => void, { width: number; height: number }] {
  const [el, setEl] = useState<HTMLElement | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    if (!el || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(() =>
      setSize({ width: el.clientWidth, height: el.clientHeight }),
    );
    ro.observe(el);
    return () => ro.disconnect();
  }, [el]);
  return [setEl, size];
}

function SlideEditorInner({ platform, theme: themeSetting = 'auto' }: BlueSlideEditorProps) {
  const { t, i18n } = useTranslation();
  // Macro recorder (CORE-SCRIPT-002): it sees every slide command the editor sends.
  const [recorder] = useState(() => new ScriptRecorder('slide'));
  const engine = useMemo(
    () => recordEngine(platform.engine, recorder),
    [platform.engine, recorder],
  );
  const [, setRecorderTick] = useState(0);
  useEffect(() => recorder.subscribe(() => setRecorderTick((n) => n + 1)), [recorder]);
  // Read aloud (CORE-LANG-003): the commands reach the hook through a ref.
  const readAloud = useReadAloud(platform.speech, 'text');
  const [readAloudLink] = useState(() => new ReadAloudLink());
  const uiLang = i18n.language;
  useEffect(() => {
    readAloudLink.attach(readAloud);
  });
  // Translation (CORE-LANG-001): the commands reach the pane through a link.
  const translate = useTranslate(platform.translation, platform.clipboard);
  const [translateLink] = useState(() => new TranslateLink());
  useEffect(() => {
    translateLink.attach(translate);
  });
  const [readNotes, setReadNotes] = useState(() => {
    try {
      return localStorage.getItem('bo.readAloud.slideNotes') === '1';
    } catch {
      return false;
    }
  });
  // The theme picked in File › Options (remembered) wins over the editor's own setting.
  const theme = useResolvedTheme(useThemePref(themeSetting));
  const [info, setInfo] = useState<DocInfo | null>(null);
  /** The open deck has macros (kept, never run): show the warning bar. */
  const [fileName, setFileName] = useState('');
  /** The template the open deck was created from (CORE-FMT-001: "Edit template"). */
  const [templateFile, setTemplateFile] = useState<{ bytes: Uint8Array; name: string } | null>(
    null,
  );
  /** A slide show file (.ppsx / .ppsm) was opened: start the show once it is shown. */
  const [autoShow, setAutoShow] = useState(false);
  /** "Save as" file type dialog. */
  const [saveAsType, setSaveAsType] = useState(false);
  const [current, setCurrent] = useState(0);
  const [view, setView] = useState<SlideView>('normal');
  const [returnView, setReturnView] = useState<SlideView>('normal');
  const [notes, setNotes] = useState('');
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const [refused, setRefused] = useState('');
  const [black, setBlack] = useState(false);
  const [startedAt, setStartedAt] = useState(0);
  const [now, setNow] = useState(() => Date.now());
  const [epoch, setEpoch] = useState(0);
  const [revs, setRevs] = useState<number[]>([]);
  const [edit, setEdit] = useState<SlideEditResult | null>(null);
  const [preview, setPreview] = useState<SlidePreview | null>(null);
  const [editing, setEditing] = useState<TextEditing | null>(null);
  const [notesDraft, setNotesDraft] = useState<string | null>(null);
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dropOver, setDropOver] = useState<number | null>(null);
  const [peers, setPeers] = useState<SlidePeer[]>([]);
  const peersOn = useMemo(() => peersBySlide(peers), [peers]);
  /** An encrypted deck waiting for its password ([MS-OFFCRYPTO]). */
  const [passwordPrompt, setPasswordPrompt] = useState<{
    bytes: Uint8Array;
    name: string;
    wrong: boolean;
  } | null>(null);
  const [savePassword, setSavePassword] = useState(false);
  const [macroBanner, setMacroBanner] = useState(false);
  /**
   * Show position with the playing order it moves in (SLIDE-SHOW-001), changed
   * together; `base` = the order the show started with (a kiosk restarts on it).
   */
  const [play, setPlay] = useState<{ pos: Pos; run: ShowRun; base: number[] }>(() => ({
    pos: { slide: 0, click: 0, enter: 'jump', from: null },
    run: newRun([]),
    base: [],
  }));
  const pos = play.pos;
  const showRun = play.run;
  const [white, setWhite] = useState(false);
  const [tool, setTool] = useState<PresenterTool>('pointer');
  const [zoom, setZoom] = useState<ZoomTransform>(NO_ZOOM);
  const ink = useMemo(() => new InkLayer(), []);
  const [inkRev, setInkRev] = useState(0);
  const [gridOpen, setGridOpen] = useState(false);
  /** Rehearse timings: the clock while rehearsing. */
  const [clock, setClock] = useState<RehearsalClock | null>(null);
  const rehearsing = clock !== null;
  /** Recording the show (SLIDE-REC-001): its options asked first, then the running recorder. */
  const [recordAsk, setRecordAsk] = useState<{ from: number } | null>(null);
  const [recOpts, setRecOpts] = useState<RecordOptions>(DEFAULT_RECORD_OPTIONS);
  const [rec, setRec] = useState<{
    recorder: ShowRecorder;
    session: MediaCaptureSession | null;
    opts: RecordOptions;
    /** Whole recording time (pauses left out). */
    total: RehearsalClock;
    notice: string | null;
  } | null>(null);
  const [, setRecRev] = useState(0);
  const recording = rec !== null;
  const [kioskOn, setKioskOn] = useState(false);
  const [endState, setEndState] = useState<EndShowState | null>(null);
  const [endKey, setEndKey] = useState(0);
  const [customOpen, setCustomOpen] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);
  const [displayTarget, setDisplayTarget] = useState<DisplayTarget>({ kind: 'window' });
  const [paneOpen, setPaneOpen] = useState(false);
  /** Comments side panel ([MS-PPTX] modern comments). */
  const [commentsOpen, setCommentsOpen] = useState(false);
  /** Symbol picker panel (inserts into the text being edited, else a new text box). */
  const [symbolsOpen, setSymbolsOpen] = useState(false);
  const [checkOpen, setCheckOpen] = useState(false);
  /** Version history / compare versions pane (CORE-VERSION-001). */
  const [versionsOpen, setVersionsOpen] = useState(false);
  /** BlueScript pane (CORE-SCRIPT-001). */
  const [scriptOpen, setScriptOpen] = useState(false);
  /** Selection pane: the slide's objects in z-order (CORE-UI-006). */
  const [selectionOpen, setSelectionOpen] = useState(false);
  // Task pane layout of BlueSlide, remembered per app (CORE-UI-006).
  const [paneLayout, setPaneLayout] = useTaskPaneLayout('slide');
  /** Slide size dialog and master / layout rename dialog (SLIDE-MASTER-001). */
  const [sizeOpen, setSizeOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  /** Sections (SLIDE-VIEW-001): collapsed section ids, the header picked last, rename dialog. */
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [sectionPick, setSectionPick] = useState<number | null>(null);
  const [sectionRename, setSectionRename] = useState(false);
  /** Clipboard history panel (Clipboard group launcher). */
  const [clipPanel, setClipPanel] = useState(false);
  /** Text and picture of the last shapes copied here (paste as picture / text). */
  const [shapesCopy, setShapesCopy] = useState<{ text: string; png?: string } | null>(null);
  /** The last paste of copied shapes while its mode can change (shown while its result
   * is the latest one: any other command ends it). */
  const [pasteUi, setPasteUi] = useState<{
    n: number;
    mode: SlidePasteChoice;
    choices: SlidePasteChoice[];
    result: SlideEditResult;
  } | null>(null);
  const [previewToken, setPreviewToken] = useState(0);
  const [ribbonTab, setRibbonTab] = useState('home');
  const [searchOpen, setSearchOpen] = useState(false);
  useCommandSearchShortcut(() => setSearchOpen(true));
  const [smartArtDialog, setSmartArtDialog] = useState<{
    mode: 'insert' | 'edit';
    initial: SmartArtDraft;
  } | null>(null);
  // 3D models (CORE-3DMODEL-001): pan & zoom mode and the alt text dialog.
  // Pan & zoom is on for one model (off once another object is selected).
  const [panZoomFor, setPanZoomFor] = useState<number | null>(null);
  const [model3dAlt, setModel3dAlt] = useState(false);
  // Zooms (SLIDE-ZOOM-001): the Insert › Zoom picker.
  const [zoomDialog, setZoomDialog] = useState<ZoomInsertKind | null>(null);
  const [chartDialog, setChartDialog] = useState<{
    mode: 'insert' | 'edit';
    initial: ChartDraft;
  } | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [exportDocsOpen, setExportDocsOpen] = useState(false);
  /** Print panel (CORE-PRINT-001). */
  const [printOpen, setPrintOpen] = useState(false);
  /** PDF/A, tagged PDF and bookmarks dialog (CORE-FMT-002). */
  const [pdfOptionsOpen, setPdfOptionsOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [videoOpen, setVideoOpen] = useState(false);
  const [sortFrom, setSortFrom] = useState<number | null>(null);
  const [sortOver, setSortOver] = useState<number | null>(null);
  const showRef = useRef<HTMLDivElement>(null);
  const slideRef = useRef<HTMLDivElement>(null);
  const colorId = useId();
  const [colorTarget, setColorTarget] = useState<ColorTarget>('fill');
  const [stageRef, stage] = useSize();
  // Editing zoom of the slide (CORE-UI-007): `null` = fit the window; the notes pane.
  const [stageZoom, setStageZoom] = useState<number | null>(null);
  const [notesShown, setNotesShown] = useState(true);
  const thumbsEl = useRef<HTMLElement | null>(null);
  const glyphs = useMemo(() => new GlyphStore(engine), [engine]);
  const images = useMemo(() => new ImageStore(engine), [engine]);
  // Closing the editor releases the decoded pictures (CORE-PERF-001).
  useEffect(() => () => images.clear(), [images]);
  const showCache = useMemo(() => new ShowCache(engine), [engine]);

  const count = info?.pageCount ?? 0;
  const slideSize = info?.pages[0] ?? { width: 960, height: 540 };
  const modified = edit?.modified ?? false;

  const fail = useCallback(
    (e: unknown) => setError(e instanceof Error ? e.message : String(e)),
    [],
  );

  // Text editing: open until the engine reports it ended (after it started).
  const editingRef = useRef<{ id: number; confirmed: boolean } | null>(null);

  /** Apply an engine result: selection, dirty slides, slide list. */
  const apply = useCallback((r: SlideEditResult) => {
    setEdit(r);
    setPreview(r.preview ?? null);
    setRefused(r.error ?? '');
    setInfo(r.info);
    setCurrent(r.slide);
    if (r.slidesChanged) {
      setRevs((v) => Array.from({ length: r.info.pageCount }, (_, i) => (v[i] ?? 0) + 1));
    } else if (r.dirty.length) {
      setRevs((v) => {
        const n = [...v];
        for (const i of r.dirty) n[i] = (n[i] ?? 0) + 1;
        return n;
      });
    }
  }, []);

  const ctrl = useMemo(() => new SlideController(engine, apply, fail), [engine, apply, fail]);
  // Stop recording waits for queued commands (CORE-SCRIPT-002).
  useEffect(() => recorder.setSettle(() => ctrl.idle()), [recorder, ctrl]);
  // Dictation (CORE-LANG-002): into the text being edited, one setText per phrase.
  const dictationTarget = useCallback(
    () =>
      slideDictationTarget({
        run: (c) => ctrl.run(c),
        latest: () => ctrl.last,
        idle: () => ctrl.idle(),
      }),
    [ctrl],
  );
  const dictation = useDictation(platform.dictation, dictationTarget);
  const [dictationLink] = useState(() => new DictationLink());
  useEffect(() => {
    dictationLink.attach(dictation);
  });

  // The engine ended text editing (other command, slide change): close the editor.
  useEffect(() => {
    const ed = editingRef.current;
    if (!ed || !edit) return;
    if (edit.textEdit?.id === ed.id) ed.confirmed = true;
    else if (ed.confirmed) {
      editingRef.current = null;
      setEditing(null);
    }
  }, [edit]);

  // Live collaboration: collaborators' edits and selected objects.
  useEffect(() => {
    const unChange = engine.onSlideExternalChange?.(apply);
    const unPeers = engine.onSlidePeers?.(setPeers);
    return () => {
      unChange?.();
      unPeers?.();
    };
  }, [engine, apply]);
  const run = useCallback((cmd: SlideCommand) => void ctrl.run(cmd), [ctrl]);
  // Spelling of the shown slide and of the whole deck (CORE-SPELL-006).
  const spelling = useSlideSpelling(engine, edit, current, count > 0);
  const { setDialog: setSpellDialog, toggle: toggleSpelling, asYouType } = spelling;
  // Chart Design gallery of the selected chart (CORE-CHART-007), loaded when a menu opens.
  const loadGallery = useMemo(() => slideGalleryLoader((cmd) => ctrl.run(cmd)), [ctrl]);
  const [saveTemplate, setSaveTemplate] = useState(false);
  const runResult = useCallback((cmd: SlideCommand) => ctrl.run(cmd), [ctrl]);
  /** Posts a comment / reply with its @mentions and task, then notifies the people (CORE-COMMENT-001). */
  const postComment = useCallback(
    async (parent: string | null, extra: MentionSubmit) => {
      const ids = (r: SlideEditResult | null | undefined) =>
        new Set((r?.comments ?? []).flatMap((th) => th.comments.map((c) => c.id)));
      const before = ids(edit);
      const r = await ctrl.run({
        cmd: 'addComment',
        ...(parent ? { parent } : {}),
        text: extra.text,
        author: platform.auth?.user?.name ?? '',
        date: new Date().toISOString(),
        ...mentionFields(extra.mentions, extra.assignee),
      });
      const added = [...ids(r)].find((id) => !before.has(id));
      if (added) {
        void notifyMentions(platform.people, {
          commentId: added,
          text: extra.text.trim(),
          mentions: extra.mentions,
          assignee: extra.assignee,
          app: 'slide',
        });
      }
    },
    [ctrl, edit, platform],
  );
  // Edit points (CORE-SHAPE-001): the shape whose points are being edited.
  const [pointsFor, setPointsFor] = useState<number | null>(null);
  const selectedId = edit?.selection.length === 1 ? (edit.selection[0]?.id ?? null) : null;
  const selectedIdRef = useRef(selectedId);
  useEffect(() => {
    selectedIdRef.current = selectedId;
  }, [selectedId]);
  const setEditingPoints = useCallback(
    (on: boolean) => setPointsFor(on ? selectedIdRef.current : null),
    [],
  );
  // CORE-SEC-001: final mark, password to modify, label.
  const runSecurity = useCallback((cmd: SlideCommand) => ctrl.run(cmd), [ctrl]);
  const protection = useFileProtection(runSecurity, (platform as Partial<Platform>).auth?.user);
  const { run: runProtect, setUi: setProtectUi } = protection;
  const protectionOpened = useRef(protection.opened);
  useEffect(() => {
    protectionOpened.current = protection.opened;
  });
  useEffect(() => {
    if (epoch > 0) void protectionOpened.current();
  }, [epoch]);

  // Symbol picker: typed into the shape text being edited (its textarea sends `setText`
  // on input, one undo step), else placed in a new text box.
  const insertSymbol = useCallback(
    (char: string) => {
      const ta = slideRef.current?.querySelector('textarea');
      if (ta) {
        ta.setRangeText(char, ta.selectionStart, ta.selectionEnd, 'end');
        ta.dispatchEvent(new Event('input', { bubbles: true }));
        return;
      }
      run({ cmd: 'insertTextBox', text: char });
    },
    [run],
  );

  const load = useCallback(
    async (bytes: Uint8Array, name: string, password?: string, editTemplate = false) => {
      setStatus(t('slideEditor.loading'));
      setError('');
      await engine.init();
      let doc: DocInfo;
      try {
        doc = await engine.open(bytes, name, password);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes(PASSWORD_REQUIRED) || msg.includes(WRONG_PASSWORD)) {
          setStatus('');
          setPasswordPrompt({ bytes, name, wrong: msg.includes(WRONG_PASSWORD) });
          return;
        }
        throw e;
      }
      // CORE-FMT-001: a template (.potx / .potm) starts a new, untitled deck of the
      // matching type unless the user asked to edit it; a slide show starts the show.
      const plan = planOpen(doc.variant, doc.name || name, editTemplate);
      if (plan.saveAs)
        doc = (await engine.slideCommand({ cmd: 'saveFormat', ext: plan.saveAs })).info;
      setTemplateFile(plan.variant?.template ? { bytes, name } : null);
      setMacroBanner(!!doc.macros);
      images.clear();
      setEditing(null);
      setNotesDraft(null);
      setInfo(doc);
      // .ppt / .odp decks are converted: they are saved as .pptx.
      setFileName(doc.name || name);
      setCurrent(0);
      setRevs([]);
      setEpoch((e) => e + 1);
      setStatus('');
      platform.window.setTitle(`${doc.name || name} — ${t('app.slide')}`);
      await ctrl.run({ cmd: 'state' });
      if (plan.fromTemplate) setStatus(t('fileVariants.fromTemplate', { name }));
      setAutoShow(plan.show);
    },
    [engine, images, platform, t, ctrl],
  );

  useEffect(() => {
    platform.window.setTitle(t('appTitle.slide'));
  }, [platform, t]);

  useEffect(() => {
    const un = platform.shell.onOpenRequest((f) => void load(f.bytes, f.name).catch(fail));
    return un;
  }, [platform, load, fail]);

  // Notes for the presenter view.
  useEffect(() => {
    if (!info || view !== 'presenter') return;
    let cancelled = false;
    void engine
      .getSlideNotes(Math.min(pos.slide, count - 1))
      .then((n) => !cancelled && setNotes(n))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [engine, info, pos.slide, count, view]);

  // Clock for the presenter view and the rehearsal timer.
  useEffect(() => {
    if (view !== 'presenter' && !rehearsing && !recording) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [view, rehearsing, recording]);

  const goTo = useCallback(
    (i: number) => {
      const n = Math.max(0, Math.min(i, count - 1));
      setCurrent(n);
      setEditing(null);
      run({ cmd: 'goTo', slide: n });
    },
    [count, run],
  );

  // The spelling dialog shows an issue: its slide, its shape selected.
  const showSpellIssue = useCallback(
    (i: SlideIssue) => {
      if (i.slide !== current) goTo(i.slide);
      if ('id' in i.target) run({ cmd: 'select', ids: [i.target.id] });
    },
    [current, goTo, run],
  );

  // Audience screen (second window) of the presenter view.
  const audience = useMemo(
    () =>
      new AudienceLink(platform, async () => ({
        name: fileName || 'deck.pptx',
        bytes: await engine.savePptx(),
      })),
    [platform, engine, fileName],
  );
  useEffect(() => () => audience.stop(), [audience]);
  useEffect(() => {
    if (!audience.open) return;
    if (view === 'show' || view === 'presenter') {
      audience.update(
        pos.slide,
        black,
        false,
        pos.click,
        pos.enter,
        {
          white,
          zoom,
          ink: ink.strokes(pos.slide),
        },
        pos.zoom,
      );
    } else {
      audience.update(current, false, true);
      audience.stop();
    }
  }, [audience, view, current, black, pos, white, zoom, ink, inkRev]);

  const setup: SlideShowSetup = edit?.presenting?.setup ?? DEFAULT_SHOW_SETUP;
  const customShows = edit?.presenting?.customShows;

  const startShow = useCallback(
    (
      from: number,
      presenter: boolean,
      opts: { custom?: number; rehearse?: boolean; record?: boolean } = {},
    ) => {
      if (!count) return;
      const pr = edit?.presenting;
      const st = pr?.setup ?? DEFAULT_SHOW_SETUP;
      let order = playOrder(count, {
        setup: st,
        customShows: pr?.customShows,
        hidden: pr?.hidden,
        custom: opts.custom ?? null,
      });
      if (order.length === 0) order = [Math.max(0, Math.min(from, count - 1))];
      // A kiosk loops until Esc and restarts after inactivity (§19.2.1.17).
      const kiosk = st.showType === 'kiosk' && !presenter && !opts.rehearse && !opts.record;
      const at = opts.custom !== undefined ? 0 : startAt(order, from);
      const first = order[at] ?? 0;
      setReturnView(view === 'show' || view === 'presenter' ? 'normal' : view);
      setCurrent(first);
      showCache.clear();
      setPlay({
        pos: { at, slide: first, click: 0, enter: 'transition', from: null },
        run: newRun(order, { loop: st.loop || kiosk, still: !st.animation }),
        base: order,
      });
      setBlack(false);
      setWhite(false);
      setTool('pointer');
      setZoom(NO_ZOOM);
      setGridOpen(false);
      ink.clear();
      setInkRev((n) => n + 1);
      setKioskOn(kiosk);
      setClock(opts.rehearse ? new RehearsalClock(Date.now()) : null);
      setStartedAt(Date.now());
      setNow(Date.now());
      setEditing(null);
      // Browsed by one person: the slides play in the window (reading view).
      const browse = st.showType === 'browse' && !presenter && !opts.rehearse && !opts.record;
      setView(presenter ? 'presenter' : browse ? 'reading' : 'show');
    },
    [count, view, showCache, edit, ink],
  );

  // .ppsx / .ppsm open straight into the slide show (the deck itself opens normally).
  useEffect(() => {
    if (!autoShow || !count) return;
    const timer = setTimeout(() => {
      setAutoShow(false);
      startShow(0, false);
    }, 0);
    return () => clearTimeout(timer);
  }, [autoShow, count, startShow]);

  const endShow = useCallback(() => {
    setView(returnView);
    const n = Math.min(pos.slide, Math.max(0, count - 1));
    setCurrent(n);
    run({ cmd: 'goTo', slide: n });
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
    const timings = clock ? clock.finish(Date.now()) : null;
    setClock(null);
    setKioskOn(false);
    setGridOpen(false);
    setZoom(NO_ZOOM);
    setTool('pointer');
    setWhite(false);
    const kept = timings && timings.length > 0 ? timings : null;
    if (rec) {
      // A recording: its ink, clicks and clips are saved together (SLIDE-REC-001).
      setRec(null);
      ink.setClock(null);
      const o = rec.opts;
      const aspect = rec.session?.aspect ?? null;
      void rec.recorder.finish().then((takes) => {
        if (!takes.length) return;
        const slides: SlideRecordingInput[] = recordingInput(takes, (s) => ink.timed(s), {
          narration: o.narration,
          camera: o.camera,
          corner: o.corner,
          aspect,
        });
        setEndKey((k) => k + 1);
        setEndState({
          ink: false,
          timings: null,
          recording: { slides, totalMs: takes.reduce((s, x) => s + x.durationMs, 0) },
        });
      });
      return;
    }
    if (!ink.isEmpty() || kept) {
      setEndKey((k) => k + 1);
      setEndState({ ink: !ink.isEmpty(), timings: kept });
    }
  }, [returnView, count, pos.slide, run, ink, clock, rec]);

  /** Keep (or drop) the show's ink and rehearsed timings: one undo step each. */
  const finishEnd = useCallback(
    (keep: { ink: boolean; timings: boolean; recording?: boolean }) => {
      const st = endState;
      setEndState(null);
      if (!st) return;
      if (keep.recording && st.recording) {
        run({
          cmd: 'saveRecording',
          slides: st.recording.slides,
          names: {
            narration: t('showRecord.narrationName'),
            camera: t('showRecord.cameraName'),
            ink: t('showRecord.inkName'),
          },
        });
      }
      if (keep.ink && st.ink && !ink.isEmpty()) {
        run({ cmd: 'keepInk', ink: ink.toCommand(), name: t('showTools.inkName') });
      }
      if (keep.timings && st.timings) {
        run({
          cmd: 'setTimings',
          timings: st.timings.map((x) => ({ slide: x.slide, ms: x.ms })),
          useTimings: true,
        });
      }
      ink.clear();
      setInkRev((n) => n + 1);
    },
    [endState, ink, run, t],
  );

  /** A new place in the playing order; the slide left is the transition's outgoing one. */
  const placed = useCallback(
    (prev: Pos, next: RunPos): Pos => ({
      ...next,
      from: next.enter === 'transition' && prev.slide < count ? prev.slide : null,
    }),
    [count],
  );

  /** Slide show navigation: clicks play the next effect, then the next slide of the order. */
  const moveShow = useCallback(
    (a: ShowAction) => {
      setBlack(false);
      setWhite(false);
      if (a !== 'next' && a !== 'prev' && a !== 'first' && a !== 'last') return;
      const clicks = (i: number) => showCache.peek(i)?.clicks ?? 0;
      setPlay((x) => {
        const r0 = x.run.order.length ? x.run : newRun(Array.from({ length: count }, (_, i) => i));
        const r = runStep(r0, runPosOf(x.pos, r0), a, count, clicks);
        return { ...x, run: r.run, pos: placed(x.pos, r.pos) };
      });
    },
    [count, showCache, placed],
  );

  /** A shape's hyperlink in the show (custom shows, jumps, end). */
  const followLink = useCallback(
    (action: SlideLinkAction) => {
      const clicks = (i: number) => showCache.peek(i)?.clicks ?? 0;
      const r = runLink(
        play.run,
        runPosOf(play.pos, play.run),
        action,
        count,
        clicks,
        customShows ?? [],
      );
      if (r === 'end') endShow();
      else setPlay((x) => ({ ...x, run: r.run, pos: placed(x.pos, r.pos) }));
    },
    [play, count, showCache, customShows, endShow, placed],
  );

  /** Go-to-slide grid: place `at` of the order. */
  const gotoShow = useCallback(
    (at: number) => {
      const clicks = (i: number) => showCache.peek(i)?.clicks ?? 0;
      setGridOpen(false);
      setBlack(false);
      setWhite(false);
      setPlay((x) => {
        const r = runGoto(x.run, runPosOf(x.pos, x.run), at, clicks);
        return { ...x, run: r.run, pos: placed(x.pos, r.pos) };
      });
    },
    [showCache, placed],
  );

  // Rehearsal: time each slide of the order.
  useEffect(() => {
    if (!clock) return;
    // The end screen does not count for the last slide.
    if (pos.slide < count) {
      clock.resume(Date.now());
      clock.enter(pos.slide, Date.now());
    } else clock.pause(Date.now());
  }, [clock, pos.slide, count]);

  // Recording (SLIDE-REC-001): one take per slide (a slide shown again is recorded again, its
  // ink with it), the clicks that play its effects; the end screen stops the clock.
  const recPrev = useRef<{ slide: number; click: number } | null>(null);
  useEffect(() => {
    if (!rec) {
      recPrev.current = null;
      return;
    }
    const r = rec.recorder;
    const slide = pos.slide < count ? pos.slide : null;
    let fresh = false;
    if (r.state !== 'idle') fresh = r.enter(slide);
    else if (slide !== null) {
      r.start(slide);
      fresh = true;
    }
    if (fresh && slide !== null) {
      ink.clear(slide);
      // Redrawn after this effect (the ink layer is not React state).
      queueMicrotask(() => setInkRev((n) => n + 1));
    }
    const prev = recPrev.current;
    if (prev && prev.slide === pos.slide && pos.click > prev.click && pos.enter === 'click') {
      r.click();
    }
    recPrev.current = { slide: pos.slide, click: pos.click };
    if (slide === null) rec.total.pause(Date.now());
    else if (r.state === 'recording') rec.total.resume(Date.now());
  }, [rec, pos.slide, pos.click, pos.enter, count, ink]);

  /** Open the devices and start the show as a recording. */
  const startRecording = useCallback(
    async (from: number, o: RecordOptions) => {
      setRecordAsk(null);
      setRecOpts(o);
      let session: MediaCaptureSession | null = null;
      let notice: string | null = null;
      if ((o.narration || o.camera) && platform.media) {
        try {
          session = await platform.media.open({ audio: o.narration, video: o.camera });
        } catch {
          notice = t('showRecord.denied');
        }
      }
      const recorder = new ShowRecorder(session);
      startShow(from, false, { record: true });
      ink.setClock(() => recorder.inkTime());
      setRec({
        recorder,
        session,
        opts: {
          ...o,
          narration: o.narration && !!session?.audio,
          camera: o.camera && !!session?.video,
        },
        total: new RehearsalClock(Date.now()),
        notice,
      });
    },
    [platform, ink, startShow, t],
  );
  // The recorder goes with the editor.
  const recRef = useRef(rec);
  useEffect(() => {
    recRef.current = rec;
  }, [rec]);
  useEffect(() => () => recRef.current?.recorder.cancel(), []);

  // Kiosk: restart after the inactivity period (§19.2.1.17 kiosk@restart).
  const restartMs = setup.restartMs;
  useEffect(() => {
    if (!kioskOn || view !== 'show') return;
    const timer = new InactivityTimer(restartMs, () => {
      const clicks = (i: number) => showCache.peek(i)?.clicks ?? 0;
      setPlay((x) => {
        const fresh = newRun(x.base, { loop: true, still: x.run.still });
        const r = runGoto(fresh, runPosOf(x.pos, fresh), 0, clicks);
        return { ...x, run: { ...r.run, prev: null }, pos: placed(x.pos, r.pos) };
      });
    });
    timer.start();
    const poke = () => timer.poke();
    window.addEventListener('pointermove', poke);
    window.addEventListener('pointerdown', poke);
    window.addEventListener('keydown', poke);
    return () => {
      timer.stop();
      window.removeEventListener('pointermove', poke);
      window.removeEventListener('pointerdown', poke);
      window.removeEventListener('keydown', poke);
    };
  }, [kioskOn, view, showCache, restartMs, placed]);

  // Prefetch the next slide's show data (its click count decides what "next" does).
  useEffect(() => {
    if ((view === 'show' || view === 'presenter') && pos.slide + 1 < count) {
      void showCache.get(pos.slide + 1);
    }
  }, [view, pos.slide, count, showCache]);

  // Automatic advance (§19.3.1.50 advTm) once the slide's effects are done.
  const autoTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (autoTimer.current) clearTimeout(autoTimer.current);
    },
    [pos, view],
  );
  const onShowIdle = useCallback(
    (s: SlideShowInfo) => {
      const after = s.transition?.advanceAfterMs;
      if (autoTimer.current) clearTimeout(autoTimer.current);
      // Rehearsing, recording, or timings turned off in the setup (§19.2.1.30 useTimings): by
      // hand.
      if (clock || rec || !setup.useTimings) return;
      if (!after || s.slide !== pos.slide || pos.click < s.clicks) return;
      autoTimer.current = setTimeout(() => moveShow('next'), after);
    },
    [pos, moveShow, clock, rec, setup.useTimings],
  );
  /** A mouse click in the show (ignored at the end of a slide whose advClick is off). */
  const clickShow = useCallback(() => {
    const s = showCache.peek(pos.slide);
    // At a kiosk clicks do not move the show (timings and hyperlinks do).
    if (kioskOn) return;
    if (s && s.transition && !s.transition.advanceOnClick && pos.click >= s.clicks) return;
    moveShow('next');
  }, [showCache, pos, moveShow, kioskOn]);

  // Full screen while the show runs.
  useEffect(() => {
    if (view === 'show') void showRef.current?.requestFullscreen?.().catch(() => {});
  }, [view]);

  // Leaving full screen with the browser's own Esc ends the show too.
  useEffect(() => {
    const onFs = () => {
      if (!document.fullscreenElement && view === 'show') endShow();
    };
    document.addEventListener('fullscreenchange', onFs);
    return () => document.removeEventListener('fullscreenchange', onFs);
  }, [view, endShow]);

  const startEdit = useCallback(
    (shape: SlideShapeInfo, at?: { x: number; y: number }) => {
      if (shape.kind === 'chart') {
        if (edit?.chart) setChartDialog({ mode: 'edit', initial: chartDraftOf(edit.chart) });
        return;
      }
      if (shape.kind === 'smartart') {
        if (edit?.smartart) setSmartArtDialog({ mode: 'edit', initial: edit.smartart });
        return;
      }
      if (!shape.editableText) return;
      editingRef.current = { id: shape.id, confirmed: false };
      setEditing({
        id: shape.id,
        text: shape.text ?? '',
        rect: shape.rect,
        size: edit?.format.size ?? 18,
        at,
      });
    },
    [edit],
  );

  /** Edit the text of a table cell on the canvas (the engine placed the caret). */
  const startCellEdit = useCallback(
    (te: SlideTextEdit) => {
      // Confirmed by the next result showing this text edit (like shapes).
      editingRef.current = { id: te.id, confirmed: false };
      setEditing({
        id: te.id,
        text: te.text,
        rect: te.rect,
        size: edit?.format.size ?? 18,
        cell: te.cell,
        sel: [te.start, te.end],
      });
    },
    [edit],
  );

  /** Give the keyboard back to the shape text after using a control. */
  const refocusText = () =>
    (document.querySelector('[data-testid="slide-text-editor"]') as HTMLElement | null)?.focus();

  /** Give the keyboard back to the slide (task panes: Escape, F6). */
  const focusSlide = useCallback(() => slideRef.current?.focus(), []);

  const exitEdit = useCallback(() => {
    editingRef.current = null;
    setEditing(null);
    run({ cmd: 'textExit' });
    slideRef.current?.focus();
  }, [run]);

  const commitNotes = useCallback(() => {
    if (notesDraft === null) return;
    const text = fromEditorText(notesDraft);
    setNotesDraft(null);
    if (text !== (edit?.notes ?? '')) run({ cmd: 'setNotes', slide: current, text });
  }, [notesDraft, edit, current, run]);

  // The native colour input is found by id (registry commands run outside render).
  const pickColor = useCallback(
    (target: ColorTarget) => {
      setColorTarget(target);
      (document.getElementById(colorId) as HTMLInputElement | null)?.click();
    },
    [colorId],
  );

  // Clipboard: own copies carry a marker so pasting prefers the full internal
  // copy; pictures and text from other programs become a picture / text box.
  const toSystem = useCallback(
    (r: SlideEditResult | null) => {
      const c = r?.clipboard;
      if (!c) return;
      const kind = r?.clip === 'slides' ? 'slides' : 'shapes';
      const copied = kind === 'shapes' ? { text: c.text, png: c.png } : null;
      ctrl.rememberShapesCopy(copied);
      setShapesCopy(copied);
      clipboardHistory.push({ app: 'slide', kind, text: c.text, png: c.png });
      const mark = ctrl.newClipMark(c.text);
      const html = `<div ${CLIP_MARK}="${mark}">${escapeHtml(c.text)}</div>`;
      try {
        const cb = platform.clipboard;
        const png = c.png ? fromBase64(c.png) : undefined;
        const done = cb.writeItems
          ? cb.writeItems({ text: c.text, html, png })
          : cb.writeHtml(html, c.text);
        void done.catch(() => {});
      } catch {
        /* no system clipboard: the internal copy still works */
      }
    },
    [platform, ctrl],
  );
  const pasteFrom = useCallback(
    async (read: ClipboardContent | null) => {
      const sys = read;
      const mark = sys?.html ? new RegExp(`${CLIP_MARK}="([^"]+)"`).exec(sys.html)?.[1] : undefined;
      const external = sys && (sys.image || sys.text?.trim()) && !ctrl.isOwnClip(mark, sys.text);
      if (!sys || !external) {
        // The engine refuses (noClip) when its clipboard is empty.
        const r = await ctrl.run({ cmd: 'paste' });
        const copied = ctrl.shapesCopy;
        if (r && !r.error && r.clip === 'shapes' && copied) {
          setPasteUi((old) => ({
            n: (old?.n ?? 0) + 1,
            mode: 'theme',
            choices: slidePasteChoices(copied),
            result: r,
          }));
        }
        return;
      }
      if (sys.image) await ctrl.insertPicture(sys.image, 'clipboard.png');
      else if (sys.text) await ctrl.run({ cmd: 'insertTextBox', text: fromEditorText(sys.text) });
    },
    [ctrl],
  );

  /** Redo the last shape paste in another mode: undo it, paste again (one undo step overall). */
  const switchPaste = useCallback(
    async (choice: SlidePasteChoice) => {
      const ui = pasteUi && pasteUi.result === edit ? pasteUi : null;
      const copy = shapesCopy;
      if (!ui || !copy || ui.mode === choice) return;
      const pasteAs = (c: SlidePasteChoice) =>
        c === 'picture' && copy.png
          ? ctrl.insertPicture(fromBase64(copy.png), 'clipboard.png')
          : c === 'text'
            ? ctrl.run({ cmd: 'insertTextBox', text: fromEditorText(copy.text) })
            : ctrl.run({ cmd: 'paste', keepSource: c === 'keep' });
      const undone = await ctrl.run({ cmd: 'undo' });
      if (!undone || undone.error) return;
      let r = await pasteAs(choice);
      if (!r || r.error) {
        r = await pasteAs(ui.mode);
        choice = ui.mode;
      }
      if (r && !r.error) setPasteUi({ ...ui, n: ui.n + 1, mode: choice, result: r });
    },
    [ctrl, pasteUi, edit, shapesCopy],
  );

  /** Paste an item of the clipboard history: our latest shapes copy as shapes, else a
   * picture (copies without text) or a text box. */
  const pasteItem = useCallback(
    async (item: ClipItem) => {
      if (item.app === 'slide' && ctrl.isOwnClip(undefined, item.text)) {
        await ctrl.run({ cmd: 'paste' });
      } else if (item.png && !item.text.trim()) {
        await ctrl.insertPicture(fromBase64(item.png), 'clipboard.png');
      } else if (item.text.trim()) {
        await ctrl.run({ cmd: 'insertTextBox', text: fromEditorText(item.text) });
      }
    },
    [ctrl],
  );

  // Ctrl+V outside text fields arrives as a paste event (no permission needed).
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) return;
      if (view !== 'normal' || !info) return;
      e.preventDefault();
      const d = e.clipboardData;
      const file = d
        ? Array.from(d.items ?? [])
            .find((i) => i.type === 'image/png' || i.type === 'image/jpeg')
            ?.getAsFile()
        : null;
      void (async () => {
        const sys: ClipboardContent = {
          text: d?.getData('text/plain') || undefined,
          html: d?.getData('text/html') || undefined,
          image: file ? new Uint8Array(await file.arrayBuffer()) : undefined,
        };
        await pasteFrom(sys);
      })().catch(fail);
    };
    window.addEventListener('paste', onPaste);
    return () => window.removeEventListener('paste', onPaste);
  }, [view, info, pasteFrom, fail]);

  const baseName = useCallback(
    () => fileName.replace(/\.pptx$/i, '') || t('slideEditor.untitled'),
    [fileName, t],
  );

  /** Grid of a new chart (localized sample data). */
  const newChart = useCallback(
    (): ChartDraft => ({
      kind: 'column',
      title: '',
      legend: 'b',
      data: {
        categories: [1, 2, 3, 4].map((n) => t('slideEditor.chartDefaultCategory', { n })),
        series: [
          [4.3, 2.5, 3.5, 4.5],
          [2.4, 4.4, 1.8, 2.8],
          [2, 2, 3, 5],
        ].map((values, j) => ({ name: t('slideEditor.chartSeries', { n: j + 1 }), values })),
      },
    }),
    [t],
  );
  const openSmartArt = useCallback(() => {
    const s = edit?.smartart;
    if (s) setSmartArtDialog({ mode: 'edit', initial: s });
  }, [edit]);
  const applySmartArt = useCallback(
    (d: SmartArtDraft) => {
      const mode = smartArtDialog?.mode;
      setSmartArtDialog(null);
      if (mode === 'insert') run({ cmd: 'insertSmartArt', ...d });
      else run({ cmd: 'setSmartArt', ...d });
    },
    [smartArtDialog, run],
  );
  const openChart = useCallback(() => {
    const c = edit?.chart;
    if (c) setChartDialog({ mode: 'edit', initial: chartDraftOf(c) });
  }, [edit]);
  const applyChart = useCallback(
    (d: ChartDraft) => {
      const mode = chartDialog?.mode;
      setChartDialog(null);
      // A chosen chart template brings its type and formatting.
      const tpl = d.template ? { template: d.template, texts: layoutTexts(t) } : {};
      if (mode === 'insert') {
        run({
          cmd: 'insertChart',
          kind: d.kind,
          title: d.title,
          legend: d.legend,
          data: d.data,
          ...tpl,
        });
      } else {
        run({
          cmd: 'setChart',
          kind: d.kind,
          title: d.title,
          legend: d.legend,
          data: d.data,
          ...tpl,
        });
      }
    },
    [chartDialog, run, t],
  );

  const exportVideo = useCallback(
    async (
      req: ShowExportRequest,
      onProgress: (done: number, total: number) => void,
      signal: AbortSignal,
    ) => {
      await ctrl.idle();
      const r = await exportShow(engine, platform.video, req, onProgress, signal, platform.media);
      await platform.files.saveFile(r.bytes, `${baseName()}.${r.ext}`, r.mime);
      setStatus(t(r.audioDropped ? 'slideEditor.videoNoAudio' : 'slideEditor.videoExported'));
    },
    [ctrl, engine, platform, baseName, t],
  );

  const exportPng = useCallback(
    async ({ scope, scale }: ExportPngOptions) => {
      setExporting(true);
      try {
        await ctrl.idle();
        const name = baseName();
        if (scope === 'current') {
          const n = Math.min(current, Math.max(0, count - 1));
          const png = await slidePng(engine, glyphs, images, n, scale);
          await platform.files.saveFile(png, `${name}-${n + 1}.png`, 'image/png');
        } else {
          const files: { name: string; data: Uint8Array }[] = [];
          for (let i = 0; i < count; i++) {
            files.push({
              name: `${name}-${i + 1}.png`,
              data: await slidePng(engine, glyphs, images, i, scale),
            });
          }
          await platform.files.saveFile(zipStore(files), `${name}-png.zip`, 'application/zip');
        }
        setExportOpen(false);
        setStatus(t('slideEditor.exported'));
      } catch (e) {
        fail(e);
      } finally {
        setExporting(false);
      }
    },
    [ctrl, baseName, current, count, engine, glyphs, images, platform, t, fail],
  );

  const exportDocs = useCallback(
    async (s: ExportDocsState) => {
      setExporting(true);
      try {
        if (!engine.exportSlides) throw new Error(t('slideExport.unsupported'));
        await ctrl.idle();
        const plan = exportPlan(s, baseName(), current, count);
        const bytes = await engine.exportSlides(plan.options);
        await platform.files.saveFile(bytes, plan.name, plan.mime);
        setExportDocsOpen(false);
        setStatus(t('slideExport.done'));
      } catch (e) {
        fail(e);
      } finally {
        setExporting(false);
      }
    },
    [ctrl, baseName, current, count, engine, platform, t, fail],
  );

  /** Actions of the picture tools tab (CORE-PIC-001), behind the `picture.*` commands. */
  const [pictureHub] = useState(() => new PictureActionsHub());
  /** Embedded / linked workbooks (CORE-EMBED-001), behind the `object.*` commands. */
  const [objectHub] = useState(() => new ObjectActionsHub());
  /** Actions of the clip Playback / Video format tabs (SLIDE-MEDIA-001), behind `media.*`. */
  const [mediaHub] = useState(() => new MediaActionsHub());
  // The deck keeps its type: presentation, template, slide show, with or without
  // macros (CORE-FMT-001).
  const saveDeck = useCallback(
    async (variant?: string | null) => {
      await ctrl.idle();
      const name = saveName(
        deckBase(fileName) || t('slideEditor.untitled'),
        variant ?? info?.variant,
        'pptx',
      );
      const saved = await platform.files.saveFile(
        await engine.savePptx(),
        name,
        mimeFor(name) || PPTX_MIME,
      );
      if (saved) {
        await engine.markSaved();
        await ctrl.run({ cmd: 'state' });
        setStatus(t('slideEditor.saved', { name: saved }));
      }
    },
    [ctrl, fileName, info, t, platform, engine],
  );
  /** Save as the type picked in the "Save as" dialog. */
  const saveAsTypeChosen = useCallback(
    async (ext: string) => {
      try {
        const r = await ctrl.run({ cmd: 'saveFormat', ext });
        if (!r || r.error) return;
        setInfo(r.info);
        setFileName(r.info.name);
        await saveDeck(ext);
      } catch (e) {
        fail(e);
      }
    },
    [ctrl, saveDeck, fail],
  );

  /** Draw tab tools (CORE-INK-001), behind `draw.*`. */
  const [drawHub] = useState(() => new DrawActionsHub());
  const [shortcutsShown, setShortcutsShown] = useState(false);
  // View › Window (CORE-UI-007): the other windows, side by side, New Window.
  const windowTools = useWindowTools({
    platform,
    app: 'slide',
    title: fileName,
    scroller: () => thumbsEl.current,
    getFile: async () => {
      if (count === 0) return null;
      await ctrl.idle();
      return {
        bytes: await engine.savePptx(),
        name: `${(fileName || 'slides').replace(/\.[^.]+$/, '')}.pptx`,
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
  const statusBar = useStatusBarPrefs(platform.shell, 'slide');

  const registry = useMemo(() => {
    const reg = new CommandRegistry<SlideEditorState>();
    const guard = async (f: () => Promise<unknown>) => {
      try {
        await f();
      } catch (e) {
        fail(e);
      }
    };
    const base = () => deckBase(fileName) || t('slideEditor.untitled');
    const deck = (s: SlideEditorState) => s.hasDeck;
    const sel = (s: SlideEditorState) => s.hasSelection;
    const text = (s: SlideEditorState) => s.hasText;
    const simple = (
      id: string,
      cmd: SlideCommand,
      isEnabled: (s: SlideEditorState) => boolean,
      shortcut?: string,
    ) => reg.register({ id, shortcut, isEnabled, run: () => run(cmd) });

    reg.register({
      id: 'file.open',
      shortcut: 'Ctrl+O',
      run: () =>
        guard(async () => {
          const f = await platform.files.openFile([...openExtensions('slide'), '.ppt', '.odp']);
          if (f) await load(f.bytes, f.name);
        }),
    });
    reg.register({
      id: 'file.save',
      shortcut: 'Ctrl+S',
      isEnabled: deck,
      run: () => guard(() => saveDeck()),
    });
    reg.register({
      id: 'file.editTemplate',
      isEnabled: () => !!templateFile,
      run: () =>
        guard(async () => {
          if (templateFile) await load(templateFile.bytes, templateFile.name, undefined, true);
        }),
    });
    reg.register({
      id: 'file.saveAs',
      shortcut: 'Ctrl+Shift+S',
      isEnabled: deck,
      run: () => setSaveAsType(true),
    });

    // Theme files (CORE-FMT-001): the deck's theme as a .thmx (apply one: design.themeFile).
    reg.register({
      id: 'design.saveThemeFile',
      isEnabled: deck,
      run: () =>
        guard(async () => {
          const r = await ctrl.run({ cmd: 'saveThemeFile' });
          const b64 = r?.design?.themeFile;
          if (!b64) {
            setStatus(t('fileVariants.noTheme'));
            return;
          }
          const name = `${base()}.thmx`;
          const saved = await platform.files.saveFile(fromBase64(b64), name, THEME_MIME);
          if (saved) setStatus(t('fileVariants.themeSaved', { name }));
        }),
    });
    reg.register({
      id: 'file.savePassword',
      isEnabled: deck,
      run: () => setSavePassword(true),
    });
    reg.register({
      id: 'file.protect',
      isEnabled: deck,
      run: async () => {
        await runProtect({ cmd: 'security' });
        setProtectUi('panel');
      },
    });
    reg.register({
      id: 'file.exportPdf',
      isEnabled: deck,
      run: () =>
        guard(async () => {
          await platform.files.saveFile(
            await engine.exportPdf(),
            `${base()}.pdf`,
            'application/pdf',
          );
        }),
    });
    reg.register({
      id: 'file.print',
      shortcut: 'Ctrl+P',
      isEnabled: deck,
      // The print panel (slides / notes / outline / handouts, pages per sheet, preview).
      run: () =>
        engine.printPdf && engine.printPreview
          ? setPrintOpen(true)
          : guard(async () => platform.print.printPdf(await engine.exportPdf(), base())),
    });
    reg.register({
      id: 'file.printNow',
      isEnabled: deck,
      run: () => guard(async () => platform.print.printPdf(await engine.exportPdf(), base())),
    });
    reg.register({ id: 'file.exportPng', isEnabled: deck, run: () => setExportOpen(true) });
    reg.register({
      id: 'file.exportVideo',
      isEnabled: (s) => deck(s) && !!engine.videoBegin,
      run: () => setVideoOpen(true),
    });
    reg.register({ id: 'file.exportDocs', isEnabled: deck, run: () => setExportDocsOpen(true) });
    reg.register({
      id: 'file.exportPdfOptions',
      isEnabled: (s) => deck(s) && !!engine.exportSlides,
      run: () => setPdfOptionsOpen(true),
    });
    for (const v of ['normal', 'sorter', 'reading', 'outline', 'notesPage'] as const) {
      reg.register({
        id: `view.${v}`,
        isChecked: () => view === v,
        run: (s) => {
          if (v === 'reading') {
            // Reading view plays the slides like the show, in the window.
            showCache.clear();
            setPlay((x) => ({
              ...x,
              run: newRun(Array.from({ length: count }, (_, i) => i)),
              pos: { at: s.current, slide: s.current, click: 0, enter: 'transition', from: null },
            }));
          } else if (view === 'reading') {
            const n = Math.min(pos.slide, Math.max(0, count - 1));
            setCurrent(n);
            run({ cmd: 'goTo', slide: n });
          }
          setView(v);
        },
      });
    }
    reg.register({
      id: 'view.animationPane',
      isEnabled: deck,
      isChecked: () => paneOpen,
      run: () => setPaneOpen((o) => !o),
    });
    reg.register({
      id: 'view.selectionPane',
      isEnabled: deck,
      isChecked: () => selectionOpen,
      run: () => setSelectionOpen((o) => !o),
    });

    // Transitions (§19.3.1.50) and animations (§19.5).
    for (const k of RIBBON_TRANSITIONS) {
      reg.register({
        id: `transition.${k}`,
        isEnabled: deck,
        isChecked: (s) => (s.transition ?? 'none') === k,
        run: () => run({ cmd: 'setTransition', transition: { kind: k } }),
      });
    }
    reg.register({
      id: 'transition.applyAll',
      isEnabled: (s) => s.hasDeck && (s.transition ?? 'none') !== 'none',
      run: () => {
        const tr = edit?.transition;
        if (!tr) return;
        run({
          cmd: 'setTransition',
          all: true,
          transition: {
            kind: tr.known ? (tr.kind as SlideTransitionKind) : 'keep',
            dir: tr.dir ?? undefined,
            durMs: tr.durMs,
            advanceOnClick: tr.advanceOnClick,
            advanceAfterMs: tr.advanceAfterMs ?? 0,
          },
        });
      },
    });
    for (const e of ENTRANCES) simple(`anim.in.${e}`, { cmd: 'addAnimation', effect: e }, sel);
    for (const e of ['fade', 'flyIn', 'wipe'] as const) {
      simple(`anim.out.${e}`, { cmd: 'addAnimation', effect: e, class: 'exit' }, sel);
    }
    reg.register({
      id: 'anim.preview',
      isEnabled: deck,
      run: () => {
        setPaneOpen(true);
        setPreviewToken((n) => n + 1);
      },
    });
    reg.register({
      id: 'show.fromStart',
      shortcut: 'F5',
      isEnabled: deck,
      run: () => startShow(0, false),
    });
    reg.register({
      id: 'show.fromCurrent',
      shortcut: 'Shift+F5',
      isEnabled: deck,
      run: (s) => startShow(s.current, false),
    });
    reg.register({
      id: 'show.presenter',
      shortcut: 'Alt+F5',
      isEnabled: deck,
      run: (s) => startShow(s.current, true),
    });
    reg.register({
      id: 'show.audience',
      isEnabled: (s) => s.hasDeck && !!platform.channel,
      run: (s) =>
        guard(async () => {
          await ctrl.idle();
          await audience.start(s.current, displayTarget);
          startShow(s.current, true);
        }),
    });

    // Slide Show tab (SLIDE-SHOW-001).
    reg.register({ id: 'show.customShows', isEnabled: deck, run: () => setCustomOpen(true) });
    reg.register({ id: 'show.linkCustom', isEnabled: sel, run: () => setCustomOpen(true) });
    reg.register({ id: 'show.setup', isEnabled: deck, run: () => setSetupOpen(true) });
    reg.register({
      id: 'show.rehearse',
      isEnabled: deck,
      run: () => startShow(0, false, { rehearse: true }),
    });
    reg.register({
      id: 'show.useTimings',
      isEnabled: deck,
      isChecked: () => setup.useTimings,
      run: () =>
        run({
          cmd: 'setShowSetup',
          setup: { ...setup, useTimings: !setup.useTimings },
        }),
    });
    // Recording the show (SLIDE-REC-001).
    reg.register({
      id: 'show.recordFromCurrent',
      isEnabled: deck,
      run: (s) => setRecordAsk({ from: s.current }),
    });
    reg.register({
      id: 'show.recordFromStart',
      isEnabled: deck,
      run: () => setRecordAsk({ from: 0 }),
    });
    const clearRec = (scope: 'current' | 'all', what: 'recording' | 'timings') => () =>
      run({ cmd: 'clearRecording', scope, what });
    reg.register({
      id: 'show.clearTimingCurrent',
      isEnabled: deck,
      run: clearRec('current', 'timings'),
    });
    reg.register({ id: 'show.clearTimingAll', isEnabled: deck, run: clearRec('all', 'timings') });
    reg.register({
      id: 'show.clearRecordingCurrent',
      isEnabled: deck,
      run: clearRec('current', 'recording'),
    });
    reg.register({
      id: 'show.clearRecordingAll',
      isEnabled: deck,
      run: clearRec('all', 'recording'),
    });
    reg.register({
      id: 'show.playNarration',
      isEnabled: deck,
      isChecked: () => setup.narration,
      run: () => run({ cmd: 'setShowSetup', setup: { ...setup, narration: !setup.narration } }),
    });

    // History and selection.
    simple('edit.undo', { cmd: 'undo' }, (s) => s.canUndo, 'Ctrl+Z');
    simple('edit.redo', { cmd: 'redo' }, (s) => s.canRedo, 'Ctrl+Y');
    simple('edit.selectAll', { cmd: 'selectAll' }, deck, 'Ctrl+A');
    simple('edit.duplicate', { cmd: 'duplicate' }, sel, 'Ctrl+D');
    reg.register({
      id: 'edit.delete',
      shortcut: 'Delete',
      isEnabled: deck,
      run: (s) => {
        const inThumbs = !!document.activeElement?.closest('[data-slide-thumbs]');
        if (inThumbs || !s.hasSelection) {
          if (inThumbs) run({ cmd: 'deleteSlide', index: s.current });
        } else run({ cmd: 'delete' });
      },
    });

    // Clipboard: in the slide panel it copies slides, elsewhere shapes.
    const inThumbs = () => !!document.activeElement?.closest('[data-slide-thumbs]');
    const copyCmd = (cut: boolean) => (s: SlideEditorState) =>
      guard(async () => {
        if (inThumbs()) {
          toSystem(await ctrl.run({ cmd: cut ? 'cutSlides' : 'copySlides', indices: [s.current] }));
        } else if (s.hasSelection) {
          toSystem(await ctrl.run({ cmd: cut ? 'cut' : 'copy' }));
        }
      });
    reg.register({ id: 'edit.copy', shortcut: 'Ctrl+C', isEnabled: deck, run: copyCmd(false) });
    // F4: the last formatting / insertion again (CORE-UI-004).
    reg.register({
      id: 'edit.repeat',
      shortcut: 'F4',
      isEnabled: deck,
      run: () => guard(() => ctrl.repeat()),
    });
    reg.register({ id: 'options.shortcuts', run: () => setShortcutsShown(true) });
    reg.register({ id: 'edit.cut', shortcut: 'Ctrl+X', isEnabled: deck, run: copyCmd(true) });
    reg.register({
      id: 'edit.paste',
      shortcut: 'Ctrl+V',
      isEnabled: deck,
      run: () =>
        guard(async () => {
          const sys = await Promise.resolve()
            .then(() => platform.clipboard.read?.())
            .catch(() => undefined);
          await pasteFrom(sys ?? null);
        }),
    });
    reg.register({
      id: 'slide.copy',
      isEnabled: deck,
      run: (s) =>
        guard(async () => toSystem(await ctrl.run({ cmd: 'copySlides', indices: [s.current] }))),
    });
    reg.register({
      id: 'slide.cut',
      isEnabled: deck,
      run: (s) =>
        guard(async () => toSystem(await ctrl.run({ cmd: 'cutSlides', indices: [s.current] }))),
    });
    simple('slide.paste', { cmd: 'paste' }, (s) => s.hasDeck && s.clip === 'slides');

    // Slides.
    simple('slide.new', { cmd: 'addSlide' }, deck, 'Ctrl+M');
    simple('slide.duplicate', { cmd: 'duplicateSlide' }, deck);
    simple('slide.delete', { cmd: 'deleteSlide' }, deck);

    // Text formatting.
    simple('format.bold', { cmd: 'formatText', format: { kind: 'bold' } }, text, 'Ctrl+B');
    simple('format.italic', { cmd: 'formatText', format: { kind: 'italic' } }, text, 'Ctrl+I');
    simple(
      'format.underline',
      { cmd: 'formatText', format: { kind: 'underline' } },
      text,
      'Ctrl+U',
    );
    simple(
      'format.grow',
      { cmd: 'formatText', format: { kind: 'sizeBy', points: 2 } },
      text,
      'Ctrl+Shift+>',
    );
    simple(
      'format.shrink',
      { cmd: 'formatText', format: { kind: 'sizeBy', points: -2 } },
      text,
      'Ctrl+Shift+<',
    );
    reg.register({ id: 'format.color', isEnabled: text, run: () => pickColor('font') });
    for (const [id, align, key] of [
      ['format.alignLeft', 'left', 'Ctrl+L'],
      ['format.alignCenter', 'center', 'Ctrl+E'],
      ['format.alignRight', 'right', 'Ctrl+R'],
    ] as const) {
      simple(id, { cmd: 'formatText', format: { kind: 'align', align } }, text, key);
    }
    simple('format.bullets', { cmd: 'formatText', format: { kind: 'bullets' } }, text);
    simple('format.numbering', { cmd: 'formatText', format: { kind: 'numbering' } }, text);
    simple(
      'format.strike',
      { cmd: 'formatText', format: { kind: 'strike' } },
      text,
      'Ctrl+Shift+X',
    );
    reg.register({ id: 'format.highlight', isEnabled: text, run: () => pickColor('highlight') });
    simple(
      'format.noHighlight',
      { cmd: 'formatText', format: { kind: 'highlight', hex: null } },
      text,
    );
    simple('format.indent', { cmd: 'formatText', format: { kind: 'level', delta: 1 } }, text);
    simple('format.outdent', { cmd: 'formatText', format: { kind: 'level', delta: -1 } }, text);
    for (const v of LINE_SPACINGS) {
      simple(
        `format.lineSpacing.${v}`,
        { cmd: 'formatText', format: { kind: 'lineSpacing', value: v } },
        text,
      );
    }
    for (const typeface of FONTS) {
      simple(
        `format.font.${typeface}`,
        { cmd: 'formatText', format: { kind: 'font', typeface } },
        text,
      );
    }
    reg.register({ id: 'format.fill', isEnabled: sel, run: () => pickColor('fill') });
    simple('format.noFill', { cmd: 'setFill', color: null }, sel);
    reg.register({ id: 'format.line', isEnabled: sel, run: () => pickColor('line') });

    // Insert.
    simple('insert.textBox', { cmd: 'insertTextBox', text: '' }, deck);
    reg.register({ id: 'insert.symbol', isEnabled: deck, run: async () => setSymbolsOpen(true) });
    reg.register({
      id: 'review.checkDocument',
      isEnabled: deck,
      run: async () => setCheckOpen(true),
    });
    reg.register({
      id: 'review.spellingDialog',
      shortcut: 'F7',
      isEnabled: deck,
      run: async () => setSpellDialog(true),
    });
    reg.register({
      id: 'review.spelling',
      isEnabled: deck,
      isChecked: () => asYouType,
      run: async () => toggleSpelling(),
    });
    registerDictationCommands(reg, {
      enabled: deck,
      current: () => dictationLink.current(),
    });
    registerReadAloudCommands(reg, {
      enabled: deck,
      begin: async () => {
        const req = await slideReadRequest({
          run: (c) => ctrl.run(c),
          edit,
          withNotes: readNotes,
          fallbackLang: uiLang || 'vi-VN',
        });
        await readAloudLink.aloud()?.start(req);
      },
      current: () => readAloudLink.aloud(),
    });
    registerTranslateCommands(reg, {
      enabled: deck,
      actions: { deck: 'review.translateDeck', shapes: 'review.translateShapes' },
      begin: async (_s, action) => {
        const exec = (c: SlideCommand) => ctrl.run(c);
        const provider = platform.translation;
        const editing = !!edit?.textEdit;
        const shapes = edit?.selection ?? [];
        const single = !editing && shapes.length === 1 ? shapes[0] : undefined;
        const shapesAction: TranslateAction = {
          id: 'shapes',
          label: t('translate.actions.shapes'),
          run: async ({ from, to, signal, onProgress }) => {
            if (!provider) return;
            const n = await translateSelectedShapes(exec, shapes, provider, from, to, {
              signal,
              onProgress,
            });
            return n ? t('translate.done.shapes', { count: n }) : t('translate.done.nothing');
          },
        };
        const deckAction: TranslateAction = {
          id: 'deck',
          label: t('translate.actions.deck'),
          run: async ({ from, to, signal, onProgress }) => {
            if (!provider) return;
            const texts = await readDeckTexts(
              exec,
              count,
              edit?.slide ?? 0,
              shapes.map((s) => s.id),
            );
            const { deck: done, count: n } = await translateDeckTexts(texts, provider, from, to, {
              signal,
              onProgress,
            });
            if (!n) return t('translate.done.nothing');
            // The copy: the current state saved, opened as a new file, then rewritten.
            const bytes = await engine.savePptx();
            const name = translatedName(fileName || 'presentation.pptx', to, 'pptx');
            await load(bytes, name);
            await applyDeckTexts(exec, done);
            return t('translate.done.deck', { name, count: n });
          },
        };
        translateLink.open({
          source: {
            text: editing ? editedSelection(edit) : (single?.text ?? '').replace(/\v/g, '\n'),
            ...(editing
              ? {
                  replace: (tr: string) => spliceEdited(exec, edit, tr, true),
                  insert: (tr: string) => spliceEdited(exec, edit, tr, false),
                }
              : single
                ? {
                    replace: async (tr: string) => {
                      await exec({ cmd: 'setText', id: single.id, text: tr });
                    },
                  }
                : {}),
          },
          actions: shapes.length && !editing ? [shapesAction, deckAction] : [deckAction],
          autoRun: action,
        });
      },
    });
    reg.register({
      id: 'review.readAloudNotes',
      isEnabled: deck,
      isChecked: () => readNotes,
      run: async () => {
        const on = !readNotes;
        setReadNotes(on);
        try {
          localStorage.setItem('bo.readAloud.slideNotes', on ? '1' : '0');
        } catch {
          // Storage unavailable: the choice lasts for this session.
        }
      },
    });
    reg.register({
      id: 'review.comments',
      isEnabled: deck,
      isChecked: () => commentsOpen,
      run: async () => setCommentsOpen((o) => !o),
    });
    // Status bar and View › Window (CORE-UI-007).
    reg.register({
      id: 'view.notes',
      isEnabled: deck,
      isChecked: () => notesShown,
      run: async () => setNotesShown((o) => !o),
    });
    reg.register({
      id: 'view.fitWindow',
      isEnabled: deck,
      isChecked: () => stageZoom === null,
      run: async () => setStageZoom(null),
    });
    reg.register({ id: 'window.new', run: () => windowNew() });
    reg.register({ id: 'window.arrangeAll', run: () => windowArrange() });
    reg.register({
      id: 'window.sideBySide',
      isChecked: () => windowPaired,
      run: async () => windowSideBySide(),
    });
    reg.register({
      id: 'window.syncScroll',
      isEnabled: () => windowPaired,
      isChecked: () => windowPaired && windowSync,
      run: async () => windowToggleSync(),
    });
    reg.register({
      id: 'window.resetPosition',
      isEnabled: () => windowPaired,
      run: async () => windowReset(),
    });
    reg.register({
      id: 'review.versions',
      isEnabled: deck,
      isChecked: () => versionsOpen,
      run: async () => setVersionsOpen((o) => !o),
    });
    reg.register({
      id: 'script.editor',
      isEnabled: deck,
      isChecked: () => scriptOpen,
      run: async () => setScriptOpen((o) => !o),
    });
    // Macro recorder (CORE-SCRIPT-002): Stop opens the recorded script in the pane.
    reg.register({
      id: 'script.record',
      isEnabled: deck,
      isChecked: () => recorder.recording,
      run: async () => {
        if (!recorder.recording) {
          await recorder.start();
          return;
        }
        await recorder.stop();
        setScriptOpen(true);
      },
    });
    for (const [id, preset] of [
      ['insert.rect', 'rect'],
      ['insert.ellipse', 'ellipse'],
      ['insert.arrow', 'rightArrow'],
      ['insert.triangle', 'triangle'],
    ] as const) {
      simple(id, { cmd: 'insertShape', preset }, deck);
    }
    simple('insert.table', { cmd: 'insertTable', rows: 3, cols: 3 }, deck);
    simple('insert.connector', { cmd: 'insertConnector' }, (s) => s.multi);
    simple(
      'insert.elbowConnector',
      { cmd: 'insertConnector', preset: 'bentConnector3' },
      (s) => s.multi,
    );
    reg.register({
      id: 'insert.picture',
      isEnabled: deck,
      run: () =>
        guard(async () => {
          const f = await platform.files.openFile(['.png', '.jpg', '.jpeg', '.svg']);
          // An SVG keeps its vector original next to a PNG drawn from it (CORE-PIC-001).
          if (f && /\.svg$/i.test(f.name)) {
            await ctrl.run({ cmd: 'pictureInsertSvg', data: pictureBase64(f.bytes), name: f.name });
          } else if (f) await ctrl.insertPicture(f.bytes, f.name);
        }),
    });
    reg.register({
      id: 'insert.model3d',
      isEnabled: deck,
      run: () =>
        guard(async () => {
          const f = await platform.files.openFile(MODEL3D_FILES);
          if (f) await ctrl.insertPicture(f.bytes, f.name);
        }),
    });
    reg.register({
      id: 'insert.media',
      isEnabled: deck,
      run: () =>
        guard(async () => {
          const f = await platform.files.openFile(MEDIA_FILES);
          if (f) await ctrl.insertPicture(f.bytes, f.name);
        }),
    });
    reg.register({
      id: 'insert.chart',
      isEnabled: deck,
      run: () => setChartDialog({ mode: 'insert', initial: newChart() }),
    });
    const chart = (s: SlideEditorState) => !!s.isChart;
    reg.register({ id: 'chart.edit', isEnabled: chart, run: openChart });
    reg.register({
      id: 'insert.smartart',
      isEnabled: deck,
      run: () =>
        setSmartArtDialog({
          mode: 'insert',
          initial: {
            layout: 'process',
            colors: 'colorful',
            style: 'fill',
            items: [1, 2, 3].map((n) => ({
              text: t('slideEditor.smartart.sampleItem', { n }),
              level: 0,
            })),
          },
        }),
    });
    const smart = (s: SlideEditorState) => !!s.isSmartArt;
    reg.register({ id: 'smartart.edit', isEnabled: smart, run: openSmartArt });
    simple('smartart.toShapes', { cmd: 'smartArtToShapes' }, smart);
    for (const k of ['column', 'bar', 'line', 'area', 'pie'] as SlideChartKind[]) {
      simple(`chart.type.${k}`, { cmd: 'setChart', kind: k }, chart);
    }

    // Table (while cells are selected).
    const tbl = (s: SlideEditorState) => !!s.hasTable;
    simple('table.insertRowAbove', { cmd: 'tableInsertRow', below: false }, tbl);
    simple('table.insertRowBelow', { cmd: 'tableInsertRow', below: true }, tbl);
    simple('table.insertColLeft', { cmd: 'tableInsertCol', right: false }, tbl);
    simple('table.insertColRight', { cmd: 'tableInsertCol', right: true }, tbl);
    simple('table.deleteRows', { cmd: 'tableDeleteRows' }, tbl);
    simple('table.deleteCols', { cmd: 'tableDeleteCols' }, tbl);
    simple('table.merge', { cmd: 'tableMerge' }, tbl);
    simple('table.split', { cmd: 'tableSplit' }, tbl);
    reg.register({ id: 'table.fill', isEnabled: tbl, run: () => pickColor('cell') });
    reg.register({
      id: 'table.headerRow',
      isEnabled: tbl,
      isChecked: (s) => !!s.firstRow,
      run: (s) => run({ cmd: 'tableOptions', firstRow: !s.firstRow }),
    });
    reg.register({
      id: 'table.bandedRows',
      isEnabled: tbl,
      isChecked: (s) => !!s.bandRow,
      run: (s) => run({ cmd: 'tableOptions', bandRow: !s.bandRow }),
    });

    // Arrange.
    simple('arrange.front', { cmd: 'zOrder', to: 'front' }, sel);
    simple('arrange.back', { cmd: 'zOrder', to: 'back' }, sel);
    simple('arrange.forward', { cmd: 'zOrder', to: 'forward' }, sel, 'Ctrl+]');
    simple('arrange.backward', { cmd: 'zOrder', to: 'backward' }, sel, 'Ctrl+[');
    simple('arrange.group', { cmd: 'group' }, (s) => s.multi, 'Ctrl+G');
    simple('arrange.ungroup', { cmd: 'ungroup' }, (s) => s.isGroup, 'Ctrl+Shift+G');
    for (const to of ['left', 'center', 'right', 'top', 'middle', 'bottom'] as const) {
      simple(`arrange.align.${to}`, { cmd: 'align', to }, sel);
    }
    simple('arrange.distributeH', { cmd: 'distribute', axis: 'horizontal' }, sel);
    simple('arrange.distributeV', { cmd: 'distribute', axis: 'vertical' }, sel);
    registerMasterCommands(reg, {
      run,
      openSize: () => setSizeOpen(true),
      openRename: () => setRenameOpen(true),
      pickThemeFile: async () => {
        const f = await platform.files.openFile(['.thmx', '.pptx']);
        return f?.bytes ?? null;
      },
      names: () => ({
        layout: t('slideEditor.master.newLayout'),
        master: t('slideEditor.master.newMaster'),
      }),
    });
    // Picture tools (CORE-PIC-001): buttons of the "Picture tools" tab.
    registerPictureCommands(
      reg,
      (st) => !!st.isPicture,
      () => pictureHub.get(),
    );
    registerObjectCommands(reg, deck, () => objectHub.get());
    registerMediaCommands(reg, mediaHub);
    registerDrawCommands(reg, drawHub);
    // 3D models (CORE-3DMODEL-001): registered with the registry so the ribbon sees them at once.
    registerModel3dCommands(reg, run, {
      togglePanZoom: (s) =>
        setPanZoomFor((id) => (id !== null && id === s.model3dId ? null : (s.model3dId ?? null))),
      openAltText: () => setModel3dAlt(true),
    });
    // Zooms (SLIDE-ZOOM-001).
    registerZoomCommands(reg, run, {
      openInsert: (k) => setZoomDialog(k),
      pickImage: () =>
        void guard(async () => {
          const f = await platform.files.openFile(['.png', '.jpg', '.jpeg', '.gif']);
          if (f) await ctrl.run({ cmd: 'zoomImage', data: pictureBase64(f.bytes) });
        }),
    });
    // SLIDE-VIEW-001: notes / handout master views (edited in the normal view) and sections.
    registerPageMasterCommands(reg, (cmd) => {
      if (cmd.cmd === 'pageMasterView' && cmd.on) setView('normal');
      run(cmd);
    });
    registerSectionCommands(reg, {
      run: (cmd) => {
        setSectionPick(null);
        run(cmd);
      },
      openRename: () => setSectionRename(true),
      names: () => ({
        section: t('slideEditor.sections.newSection'),
        first: t('slideEditor.sections.firstSection'),
      }),
      collapse: (ids) => setCollapsed(new Set(ids)),
    });
    return reg;
  }, [
    readAloudLink,
    dictationLink,
    translateLink,
    readNotes,
    uiLang,
    fail,
    fileName,
    saveDeck,
    templateFile,
    t,
    platform,
    load,
    engine,
    view,
    startShow,
    run,
    ctrl,
    pickColor,
    toSystem,
    pasteFrom,
    audience,
    showCache,
    count,
    displayTarget,
    setup,
    paneOpen,
    selectionOpen,
    commentsOpen,
    notesShown,
    stageZoom,
    windowNew,
    windowArrange,
    windowSideBySide,
    windowToggleSync,
    windowReset,
    windowPaired,
    windowSync,
    versionsOpen,
    scriptOpen,
    newChart,
    openSmartArt,
    openChart,
    edit,
    pos.slide,
    runProtect,
    setProtectUi,
    pictureHub,
    objectHub,
    setSpellDialog,
    toggleSpelling,
    asYouType,
    mediaHub,
    drawHub,
    recorder,
  ]);

  // Saved scripts on a shortcut / the Quick Access Toolbar (CORE-SCRIPT-002).
  const scriptBindings = useScriptBindings({
    app: 'slide',
    registry,
    host: () => (count > 0 ? slideScriptHost({ run: runResult, onEnd: focusSlide }) : null),
    recorder,
    label: (id) => t(`commands.${id}`),
    onResult: (sc, r) => {
      if (!r.ok) {
        setStatus(t('script.boundFailed', { message: `${sc.name}: ${r.error?.message ?? ''}` }));
      }
    },
  });

  const state: SlideEditorState = useMemo(() => {
    const s = edit?.selection ?? [];
    return {
      hasDeck: count > 0,
      current,
      hasSelection: s.length > 0,
      multi: s.length > 1,
      // Selected shapes with text, or a table cell's text being edited.
      hasText: s.some((x) => x.editableText) || !!edit?.textEdit,
      isGroup: s.some((x) => x.kind === 'group'),
      canUndo: edit?.canUndo ?? false,
      canRedo: edit?.canRedo ?? false,
      clip: edit?.clip,
      hasTable: !!edit?.table,
      firstRow: edit?.table?.firstRow,
      bandRow: edit?.table?.bandRow,
      isChart: s.length === 1 && s[0]?.kind === 'chart' && !!edit?.chart,
      isSmartArt: s.length === 1 && s[0]?.kind === 'smartart' && !!edit?.smartart,
      isPicture: s.length > 0 && s.every((x) => x.kind === 'picture'),
      isModel3d: s.length === 1 && s[0]?.kind === 'model3d' && !!edit?.model3d,
      model3dId: edit?.model3d?.id,
      isZoom: s.length === 1 && s[0]?.kind === 'zoom' && !!edit?.zoom,
      zoom: edit?.zoom,
      panZoom3d:
        s.length === 1 && s[0]?.kind === 'model3d' && panZoomFor === (edit?.model3d?.id ?? null),
      transition: edit?.transition?.kind ?? 'none',
      isShape: s.length === 1 && s[0]?.kind === 'shape',
      mergeable: s.length > 1 && s.every((x) => x.kind === 'shape'),
      editingPoints: pointsFor !== null && pointsFor === selectedId,
      ...masterState(edit),
      ...sectionState(edit?.sections, current, sectionPick),
      ...pageMasterState(edit),
    };
  }, [count, current, edit, sectionPick, pointsFor, selectedId, panZoomFor]);
  // Built-in themes and variants (one command each).
  const design = edit?.design;
  useEffect(() => registerDesignChoices(registry, run, design), [registry, run, design]);
  useEffect(
    () => registerShapeTools(registry, run, setEditingPoints),
    [registry, run, setEditingPoints],
  );
  useEffect(() => registerShapeEffects(registry, run), [registry, run]);

  const loadWarpPreviews = useMemo(() => warpPreviewLoader(runResult), [runResult]);
  const exec = useCallback(
    (id: string) => {
      // Undo / redo typed in quick succession arrive before the previous result has refreshed
      // `canUndo` / `canRedo`: send them anyway (the engine ignores an empty history).
      if ((id === 'edit.undo' || id === 'edit.redo') && state.hasDeck) {
        run({ cmd: id === 'edit.undo' ? 'undo' : 'redo' });
        return;
      }
      void registry.execute(id, state);
    },
    [registry, state, run],
  );
  // The File screen (CORE-UI-002).
  const [backstage, setBackstage] = useState(false);
  const closeBackstage = useCallback(() => setBackstage(false), []);
  const backstageCommands = useMemo(
    () => ({
      has: (id: string) => registry.has(id),
      enabled: (id: string) => registry.isEnabled(id, state),
      run: exec,
    }),
    [registry, state, exec],
  );
  const docProperties = useMemo<BackstagePropertiesAccess>(
    () => ({
      load: async () => (await ctrl.run({ cmd: 'docProperties' }))?.docProperties ?? null,
      save: async (props) =>
        (await ctrl.run({ cmd: 'setDocProperties', props }))?.docProperties ?? null,
    }),
    [ctrl],
  );

  // Picture tools tab (CORE-PIC-001) for the selected pictures.
  const picture = usePictureTools({
    labels: pictureTabLabels(t),
    enabled: !!state.isPicture,
    selectionKey: JSON.stringify([current, (edit?.selection ?? []).map((x) => x.id)]),
    run: (c) => ctrl.run(c),
    exec,
    pickFile: () => platform.files.openFile(['.png', '.jpg', '.jpeg', '.gif', '.bmp', '.svg']),
    shapes: true,
    hub: pictureHub,
  });
  // Embedded / linked workbooks (CORE-EMBED-001): insert dialog, editing grid, update links.
  const objects = useObjectTools({
    labels: objectToolsLabels(t),
    run: (c) => ctrl.run(c),
    ...objectPlatform(platform),
    onError: (code) => fail(t(`objectTools.error.${code}`)),
    onUpdated: (count) => setStatus(t('objectTools.updated', { count })),
    hub: objectHub,
  });
  // Playback / Video format tabs (SLIDE-MEDIA-001) for one selected clip.
  const selectedClip = useMemo(() => {
    const s = edit?.selection ?? [];
    return s.length === 1 ? ((edit?.media ?? []).find((m) => m.id === s[0]?.id) ?? null) : null;
  }, [edit]);
  const mediaTools = useMediaTools({
    engine,
    clip: selectedClip,
    animations: edit?.animations ?? [],
    run: (c) => ctrl.run(c),
    pickFile: (accept) => platform.files.openFile(accept),
    hub: mediaHub,
  });
  // Draw tab (CORE-INK-001): pen, highlighter, erasers, ruler, ink replay.
  const drawShown = Math.min(current, Math.max(0, count - 1));
  const draw = useDrawTools({
    t,
    hub: drawHub,
    enabled: count > 0 && view === 'normal' && !state.masterView && !state.pageMaster,
    slide: drawShown,
    size: info?.pages[drawShown] ?? slideSize,
    run: (c) => ctrl.run(c),
    glyphs,
    images,
    revision: epoch * 1_000_000 + (revs[drawShown] ?? 0),
  });
  const extraTabs = [...(picture.tab ? [picture.tab] : []), ...mediaTools.tabs];

  /** Opens the slide context menu (set once the menu exists, below). */
  const slideMenuOpen = useRef<(m: SlideMenu) => void>(() => {});
  /** When Shift+F10 opened the menu: the browser's own keyboard context menu event follows. */
  const keyMenuAt = useRef(-Infinity);
  // Shortcuts typed inside the shape text (format.*, edit.*).
  const textShortcut = useCallback(
    (e: KeyboardEvent) => {
      const cmd = registry.match(e);
      if (!cmd || !/^(format|edit\.(undo|redo|repeat))/.test(cmd.id)) return false;
      exec(cmd.id);
      return true;
    },
    [registry, exec],
  );

  // Keyboard: shortcuts everywhere, editing keys on the slide, navigation in views and the show.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) return;
      if (view === 'show' || view === 'presenter') {
        // Presenter tools (BlueSlide's own keys): Ctrl+P pen, Ctrl+I highlighter,
        // Ctrl+L laser, Ctrl+E eraser, Ctrl+A pointer, E clear ink, G grid, Z zoom.
        if (!kioskOn) {
          const k = e.key.toLowerCase();
          const tools: Record<string, PresenterTool> = {
            p: 'pen',
            i: 'highlighter',
            l: 'laser',
            e: 'eraser',
            a: 'pointer',
          };
          const pick = tools[k];
          if ((e.ctrlKey || e.metaKey) && pick) {
            e.preventDefault();
            setTool(pick);
            return;
          }
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            if (k === 'e') {
              e.preventDefault();
              ink.clear(pos.slide);
              setInkRev((n) => n + 1);
              return;
            }
            if (k === 'g' || e.key === '-') {
              e.preventDefault();
              setGridOpen((g) => !g);
              return;
            }
            if (k === 'z') {
              e.preventDefault();
              if (zoom.scale > 1) setZoom(NO_ZOOM);
              else setTool((x) => (x === 'zoom' ? 'pointer' : 'zoom'));
              return;
            }
          }
        }
        const a = showAction(e.key);
        if (!a) return;
        e.preventDefault();
        if (kioskOn && !kioskAllows(a)) return;
        if (a === 'exit') {
          // Esc first closes the grid, the zoom or the tool, then ends the show.
          if (gridOpen) setGridOpen(false);
          else if (zoom.scale > 1) setZoom(NO_ZOOM);
          else if (tool !== 'pointer') setTool('pointer');
          else endShow();
        } else if (a === 'black' || a === 'white') {
          const c = toggleCover(black ? 'black' : white ? 'white' : 'none', a);
          setBlack(c === 'black');
          setWhite(c === 'white');
        } else if (!gridOpen) moveShow(a);
        return;
      }
      if (view === 'reading') {
        const a = showAction(e.key);
        if (a === 'next' || a === 'prev' || a === 'first' || a === 'last') {
          e.preventDefault();
          moveShow(a);
          return;
        }
        if (a === 'exit') {
          e.preventDefault();
          exec('view.normal');
          return;
        }
      }
      if (isContextMenuKey(e) && view === 'normal' && count > 0) {
        e.preventDefault();
        keyMenuAt.current = e.timeStamp;
        const thumb = document.activeElement?.closest('[data-slide-thumbs] button');
        if (thumb) {
          slideMenuOpen.current({
            ...(menuPointOf(thumb) ?? { x: 0, y: 0 }),
            context: 'thumbnail',
          });
          return;
        }
        const sel = slideRef.current?.querySelector('[data-testid="selection-box"]');
        const at = menuPointOf(sel) ?? menuPointOf(slideRef.current);
        if (at && edit) slideMenuOpen.current({ ...at, context: slideMenuContext(edit) });
        return;
      }
      const cmd = registry.match(e);
      if (cmd) {
        e.preventDefault();
        exec(cmd.id);
        return;
      }
      // Tab / Shift+Tab cycle through the shapes of the slide.
      if (e.key === 'Tab' && !e.ctrlKey && !e.altKey && view === 'normal' && count > 0) {
        const a = document.activeElement;
        if (!a || a === document.body || slideRef.current?.contains(a)) {
          e.preventDefault();
          run({ cmd: 'selectNext', back: e.shiftKey });
          return;
        }
      }
      const sel = edit?.selection ?? [];
      if (view === 'normal' && sel.length > 0) {
        const n = nudgeCommand(e.key, e.shiftKey);
        if (n) {
          e.preventDefault();
          run(n);
          return;
        }
        if (e.key === 'Backspace') {
          e.preventDefault();
          run({ cmd: 'delete' });
          return;
        }
        if (e.key === 'Escape') {
          run({ cmd: 'select', ids: [] });
          return;
        }
        if ((e.key === 'Enter' || e.key === 'F2') && sel.length === 1) {
          e.preventDefault();
          startEdit(sel[0]!);
          return;
        }
      }
      const a = showAction(e.key);
      if (a === 'next' || a === 'prev' || a === 'first' || a === 'last') {
        if (e.key === ' ' || e.key === 'Enter' || e.key === 'Backspace') return;
        e.preventDefault();
        const to = Math.min(step(current, count, a), Math.max(0, count - 1));
        if (view === 'normal') goTo(to);
        else setCurrent(to);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [
    view,
    count,
    current,
    registry,
    exec,
    endShow,
    edit,
    run,
    startEdit,
    goTo,
    moveShow,
    kioskOn,
    ink,
    zoom,
    tool,
    gridOpen,
    black,
    white,
    pos.slide,
  ]);

  // The user's keyboard shortcuts (CORE-UI-004).
  const ribbonTabsRef = useRef<RibbonTab[]>([]);
  const shortcuts = useCustomShortcuts({
    app: 'slide',
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
    app: 'slide',
    registry,
    store: platform.shell,
    files: platform.files,
    tabs: () => ribbonTabsRef.current,
    extraCommands: () => shortcutCommands([], registry, (id) => commandLabel(i18n, id)),
    shortcuts,
    enabled: (id) => registry.isEnabled(id, state),
    run: (id) => exec(id),
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
      checked?: boolean,
    ) => ({
      id,
      label,
      icon,
      size,
      shortcut: registry.get(id)?.shortcut,
      enabled: registry.isEnabled(id, state),
      checked: checked ?? registry.isChecked(id, state),
      onSelect: () => exec(id),
    });
    const f = edit?.format;
    const sm = 'small' as const;
    return [
      ...(state.masterView ? [masterTab(t, item, state, run)] : []),
      ...(state.pageMaster ? [pageMasterTab(t, item, state.pageMaster)] : []),
      {
        id: 'home',
        label: t('ribbon.tabs.home'),
        groups: [
          {
            id: 'file',
            label: t('slideEditor.groupFile'),
            items: [
              item('file.open', t('slideEditor.open'), <FolderOpen size={18} />),
              item('file.save', t('slideEditor.save'), <Save size={18} />),
              item('file.saveAs', t('commands.file.saveAs'), <Save size={16} />, sm),
              item(
                'file.editTemplate',
                t('fileVariants.editTemplate'),
                <FolderOpen size={16} />,
                sm,
              ),
              item('design.saveThemeFile', t('fileVariants.saveThemeFile'), <Save size={16} />, sm),
              item(
                'file.savePassword',
                t('commands.review.savePassword'),
                <KeyRound size={16} />,
                sm,
              ),
              item('file.protect', t('protect.cmd.protect'), <Lock size={16} />, sm),
              item('file.exportPdf', t('slideEditor.exportPdf'), <FileDown size={16} />, sm),
              item('file.exportPng', t('slideEditor.exportPng'), <FileImage size={16} />, sm),
              item('file.exportVideo', t('slideEditor.exportVideo'), <Film size={16} />, sm),
              item('file.exportDocs', t('slideExport.title'), <FileStack size={16} />, sm),
              item(
                'file.exportPdfOptions',
                t('commands.file.exportPdfOptions'),
                <FileDown size={16} />,
                sm,
              ),
              item('file.print', t('slideEditor.print'), <Printer size={16} />, sm),
            ],
          },
          {
            id: 'clipboard',
            label: t('slideEditor.groupClipboard'),
            launcher: {
              id: 'edit.clipboardPanel',
              label: t('clipHistory.open'),
              onSelect: () => setClipPanel((o) => !o),
            },
            items: [
              item('edit.paste', t('slideEditor.paste'), <ClipboardPaste size={18} />),
              item('edit.cut', t('slideEditor.cut'), <Scissors size={16} />, sm),
              item('edit.copy', t('slideEditor.copy'), <Copy size={16} />, sm),
            ],
          },
          {
            id: 'slides',
            label: t('slideEditor.groupSlides'),
            items: [
              item('slide.new', t('slideEditor.newSlide'), <SquarePlus size={18} />),
              item('slide.duplicate', t('slideEditor.duplicateSlide'), <CopyPlus size={16} />, sm),
              item('slide.delete', t('slideEditor.deleteSlide'), <Trash size={16} />, sm),
              item('slide.copy', t('slideEditor.copySlide'), <Files size={16} />, sm),
              item('slide.cut', t('slideEditor.cutSlide'), <Scissors size={16} />, sm),
              item('slide.paste', t('slideEditor.pasteSlide'), <ClipboardPaste size={16} />, sm),
            ],
          },
          {
            id: 'history',
            label: t('slideEditor.groupHistory'),
            items: [
              item('edit.undo', t('slideEditor.undo'), <Undo2 size={16} />, sm),
              item('edit.redo', t('slideEditor.redo'), <Redo2 size={16} />, sm),
            ],
          },
          {
            id: 'font',
            label: t('slideEditor.groupFont'),
            items: [
              item('format.bold', t('slideEditor.bold'), <Bold size={16} />, sm, f?.bold),
              item('format.italic', t('slideEditor.italic'), <Italic size={16} />, sm, f?.italic),
              item(
                'format.underline',
                t('slideEditor.underline'),
                <Underline size={16} />,
                sm,
                f?.underline,
              ),
              item('format.grow', t('slideEditor.growFont'), <AArrowUp size={16} />, sm),
              item('format.shrink', t('slideEditor.shrinkFont'), <AArrowDown size={16} />, sm),
              item(
                'format.strike',
                t('slideEditor.strike'),
                <Strikethrough size={16} />,
                sm,
                f?.strike,
              ),
              item('format.color', t('slideEditor.fontColor'), <Baseline size={16} />, sm),
              item('format.highlight', t('slideEditor.highlight'), <Highlighter size={16} />, sm),
              item('format.noHighlight', t('slideEditor.noHighlight'), <Eraser size={16} />, sm),
            ],
          },
          {
            id: 'paragraph',
            label: t('slideEditor.groupParagraph'),
            items: [
              item(
                'format.alignLeft',
                t('slideEditor.alignLeft'),
                <AlignLeft size={16} />,
                sm,
                f?.hasText ? f.align === 'left' : false,
              ),
              item(
                'format.alignCenter',
                t('slideEditor.alignCenter'),
                <AlignCenter size={16} />,
                sm,
                f?.align === 'center',
              ),
              item(
                'format.alignRight',
                t('slideEditor.alignRight'),
                <AlignRight size={16} />,
                sm,
                f?.align === 'right',
              ),
              item('format.bullets', t('slideEditor.bullets'), <List size={16} />, sm, f?.bullets),
              item(
                'format.numbering',
                t('slideEditor.numbering'),
                <ListOrdered size={16} />,
                sm,
                f?.numbering,
              ),
              item('format.outdent', t('slideEditor.outdent'), <IndentDecrease size={16} />, sm),
              item('format.indent', t('slideEditor.indent'), <IndentIncrease size={16} />, sm),
            ],
          },
          {
            id: 'drawing',
            label: t('slideEditor.groupDrawing'),
            items: [
              item('format.fill', t('slideEditor.fillColor'), <PaintBucket size={16} />, sm),
              item('format.noFill', t('slideEditor.noFill'), <Slash size={16} />, sm),
              item('format.line', t('slideEditor.lineColor'), <PenLine size={16} />, sm),
            ],
          },
          {
            id: 'voice',
            label: t('ribbon.groups.speech'),
            items: [item('edit.dictate', t('commands.edit.dictate'), <Mic size={18} />)],
          },
          {
            id: 'show',
            label: t('slideEditor.groupShow'),
            items: [
              item('show.fromStart', t('slideEditor.fromStart'), <Play size={18} />),
              item('show.fromCurrent', t('slideEditor.fromCurrent'), <MonitorPlay size={16} />, sm),
              item('show.presenter', t('slideEditor.presenter'), <Presentation size={16} />, sm),
              item('show.audience', t('slideEditor.audienceWindow'), <Cast size={16} />, sm),
            ],
          },
        ],
      },
      {
        id: 'insert',
        label: t('ribbon.tabs.insert'),
        groups: [
          {
            id: 'slides',
            label: t('slideEditor.groupSlides'),
            items: [item('slide.new', t('slideEditor.newSlide'), <SquarePlus size={18} />)],
          },
          {
            id: 'text',
            label: t('slideEditor.groupText'),
            items: [
              item('insert.textBox', t('slideEditor.textBox'), <Type size={18} />),
              item(
                'review.comments',
                t('slideEditor.comments.title'),
                <MessageSquareText size={18} />,
              ),
            ],
          },
          {
            id: 'symbols',
            label: t('ribbon.groups.symbols'),
            items: [item('insert.symbol', t('commands.insert.symbol'), <Omega size={18} />)],
          },
          {
            id: 'shapes',
            label: t('slideEditor.groupShapes'),
            items: [
              item('insert.rect', t('slideEditor.rect'), <Square size={16} />, sm),
              item('insert.ellipse', t('slideEditor.ellipse'), <Circle size={16} />, sm),
              item('insert.arrow', t('slideEditor.arrow'), <ArrowRight size={16} />, sm),
              item('insert.triangle', t('slideEditor.triangle'), <Triangle size={16} />, sm),
              item('insert.connector', t('slideEditor.connect'), <Spline size={16} />, sm),
              item(
                'insert.elbowConnector',
                t('slideEditor.connector'),
                <CornerDownRight size={16} />,
                sm,
              ),
            ],
          },
          {
            id: 'media',
            label: t('slideEditor.groupMedia'),
            items: [
              item('insert.picture', t('slideEditor.picture'), <ImagePlus size={18} />),
              item('insert.table', t('slideEditor.table'), <Table size={18} />),
              item('insert.chart', t('slideEditor.chart'), <ChartColumn size={18} />),
              item('object.insert', t('commands.object.insert'), <FileSpreadsheet size={18} />),
              item('object.updateLinks', t('commands.object.updateLinks'), <RefreshCw size={18} />),
              item('insert.smartart', t('slideEditor.smartart.button'), <Network size={18} />),
              item('insert.media', t('slideEditor.media'), <Clapperboard size={18} />),
              item('insert.model3d', t('slideEditor.model3d.insert'), <Model3dIcon />),
            ],
          },
          {
            id: 'zoomInsert',
            label: t('slideEditor.zoom.group'),
            items: zoomInsertItems(t, item),
          },
        ],
      },
      designTab(t, item, edit?.design, state, current, run),
      {
        id: 'transitions',
        label: t('slideEditor.transitionTab'),
        groups: [
          {
            id: 'transitionEffects',
            label: t('slideEditor.groupTransition'),
            items: RIBBON_TRANSITIONS.map((k) =>
              item(
                `transition.${k}`,
                t(`slideEditor.transition.${k}`),
                k === 'none' ? (
                  <Slash size={18} />
                ) : k === 'fade' ? (
                  <Blend size={18} />
                ) : (
                  <ArrowRightLeft size={18} />
                ),
              ),
            ),
          },
          {
            id: 'transitionTiming',
            label: t('slideEditor.groupTiming'),
            items: [
              item('transition.applyAll', t('slideEditor.applyAll'), <Files size={16} />, sm),
              item('anim.preview', t('slideEditor.preview'), <Eye size={16} />, sm),
              item('view.animationPane', t('slideEditor.animPane'), <PanelRight size={16} />, sm),
            ],
          },
        ],
      },
      {
        id: 'animations',
        label: t('slideEditor.animationsTab'),
        groups: [
          {
            id: 'entrance',
            label: t('slideEditor.groupEntrance'),
            items: ENTRANCES.map((e) =>
              item(`anim.in.${e}`, t(`slideEditor.effect.${e}`), <LogIn size={18} />),
            ),
          },
          {
            id: 'exit',
            label: t('slideEditor.groupExit'),
            items: (['fade', 'flyIn', 'wipe'] as const).map((e) =>
              item(`anim.out.${e}`, t(`slideEditor.effectOut.${e}`), <LogOut size={16} />, sm),
            ),
          },
          {
            id: 'animPreview',
            label: t('slideEditor.groupPreview'),
            items: [
              item('anim.preview', t('slideEditor.preview'), <Sparkles size={18} />),
              item('view.animationPane', t('slideEditor.animPane'), <PanelRight size={16} />, sm),
            ],
          },
        ],
      },
      drawTab(t, item, draw.state),
      slideShowTab(t, item, setup.useTimings, setup.narration),
      {
        id: 'review',
        label: t('ribbon.tabs.review'),
        groups: [
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
            items: [item('review.versions', t('versions.compareVersions'), <History size={18} />)],
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
              item('review.spelling', t('commands.review.spelling'), <SpellCheck size={16} />, sm),
            ],
          },
          {
            id: 'speech',
            label: t('ribbon.groups.speech'),
            items: [
              item('review.readAloud', t('commands.review.readAloud'), <Volume2 size={18} />),
              item(
                'review.readAloudNotes',
                t('readAloud.withNotes'),
                <NotebookText size={16} />,
                sm,
              ),
            ],
          },
          {
            id: 'language',
            label: t('ribbon.groups.language'),
            items: [
              item('review.translate', t('commands.review.translate'), <Languages size={18} />),
              item(
                'review.translateDeck',
                t('commands.review.translateDeck'),
                <Languages size={16} />,
                sm,
              ),
            ],
          },
        ],
      },
      {
        id: 'arrange',
        label: t('slideEditor.arrangeTab'),
        groups: [
          {
            id: 'order',
            label: t('slideEditor.groupOrder'),
            items: [
              item('arrange.front', t('slideEditor.bringToFront'), <BringToFront size={16} />, sm),
              item('arrange.back', t('slideEditor.sendToBack'), <SendToBack size={16} />, sm),
              item('arrange.forward', t('slideEditor.bringForward'), <ArrowUp size={16} />, sm),
              item('arrange.backward', t('slideEditor.sendBackward'), <ArrowDown size={16} />, sm),
              item('view.selectionPane', t('slideEditor.selectionPane'), <Layers size={16} />, sm),
            ],
          },
          {
            id: 'group',
            label: t('slideEditor.groupGroup'),
            items: [
              item('arrange.group', t('slideEditor.group'), <Group size={16} />, sm),
              item('arrange.ungroup', t('slideEditor.ungroup'), <Ungroup size={16} />, sm),
            ],
          },
          {
            id: 'align',
            label: t('slideEditor.groupAlign'),
            items: [
              item(
                'arrange.align.left',
                t('slideEditor.objLeft'),
                <AlignStartVertical size={16} />,
                sm,
              ),
              item(
                'arrange.align.center',
                t('slideEditor.objCenter'),
                <AlignCenterVertical size={16} />,
                sm,
              ),
              item(
                'arrange.align.right',
                t('slideEditor.objRight'),
                <AlignEndVertical size={16} />,
                sm,
              ),
              item(
                'arrange.align.top',
                t('slideEditor.objTop'),
                <AlignStartHorizontal size={16} />,
                sm,
              ),
              item(
                'arrange.align.middle',
                t('slideEditor.objMiddle'),
                <AlignCenterHorizontal size={16} />,
                sm,
              ),
              item(
                'arrange.align.bottom',
                t('slideEditor.objBottom'),
                <AlignEndHorizontal size={16} />,
                sm,
              ),
              item(
                'arrange.distributeH',
                t('slideEditor.distributeH'),
                <AlignHorizontalDistributeCenter size={16} />,
                sm,
              ),
              item(
                'arrange.distributeV',
                t('slideEditor.distributeV'),
                <AlignVerticalDistributeCenter size={16} />,
                sm,
              ),
            ],
          },
          {
            id: 'edit',
            label: t('slideEditor.groupEdit'),
            items: [
              item('edit.duplicate', t('slideEditor.duplicate'), <Copy size={16} />, sm),
              item('edit.delete', t('slideEditor.delete'), <Trash size={16} />, sm),
            ],
          },
        ],
      },
      ...(state.isShape || state.mergeable
        ? [
            shapeFormatTab({
              t,
              state,
              enabled: (id) => registry.isEnabled(id, state),
              exec,
              extraGroups: [shapeEffectsGroup({ t, state, exec, loadPreviews: loadWarpPreviews })],
            }),
          ]
        : []),
      ...(state.isModel3d ? [model3dTab(t, item)] : []),
      ...(state.isZoom ? [zoomTab(t, item, edit?.zoom, run)] : []),
      ...(state.isSmartArt
        ? [
            {
              id: 'smartart',
              context: 'smartArt' as const,
              label: t('slideEditor.smartart.tab'),
              groups: [
                {
                  id: 'smartartEdit',
                  label: t('slideEditor.smartart.group'),
                  items: [
                    item('smartart.edit', t('slideEditor.smartart.textPane'), <Pencil size={18} />),
                    item(
                      'smartart.toShapes',
                      t('slideEditor.smartart.toShapes'),
                      <Shapes size={18} />,
                    ),
                  ],
                },
              ],
            },
          ]
        : []),
      ...(state.isChart
        ? [
            {
              id: 'chart',
              context: 'chart' as const,
              label: t('slideEditor.chartTab'),
              groups: [
                {
                  id: 'chartData',
                  label: t('slideEditor.groupChartData'),
                  items: [item('chart.edit', t('slideEditor.editChart'), <Pencil size={18} />)],
                },
                {
                  id: 'chartType',
                  label: t('slideEditor.chartType'),
                  items: [
                    item(
                      'chart.type.column',
                      t('slideEditor.chartKinds.column'),
                      <ChartColumn size={16} />,
                      sm,
                    ),
                    item(
                      'chart.type.bar',
                      t('slideEditor.chartKinds.bar'),
                      <ChartBar size={16} />,
                      sm,
                    ),
                    item(
                      'chart.type.line',
                      t('slideEditor.chartKinds.line'),
                      <ChartLine size={16} />,
                      sm,
                    ),
                    item(
                      'chart.type.area',
                      t('slideEditor.chartKinds.area'),
                      <ChartArea size={16} />,
                      sm,
                    ),
                    item(
                      'chart.type.pie',
                      t('slideEditor.chartKinds.pie'),
                      <ChartPie size={16} />,
                      sm,
                    ),
                  ],
                },
                ...slideChartDesignGroups({
                  t,
                  load: loadGallery,
                  run,
                  enabled: !!state.isChart,
                  onSaveTemplate: () => setSaveTemplate(true),
                }),
              ],
            },
          ]
        : []),
      ...(state.hasTable
        ? [
            {
              id: 'table',
              context: 'table' as const,
              label: t('slideEditor.tableTab'),
              groups: [
                {
                  id: 'rowsCols',
                  label: t('slideEditor.groupRowsCols'),
                  items: [
                    item(
                      'table.insertRowAbove',
                      t('slideEditor.insertRowAbove'),
                      <ArrowUpToLine size={16} />,
                      sm,
                    ),
                    item(
                      'table.insertRowBelow',
                      t('slideEditor.insertRowBelow'),
                      <ArrowDownToLine size={16} />,
                      sm,
                    ),
                    item(
                      'table.insertColLeft',
                      t('slideEditor.insertColLeft'),
                      <ArrowLeftToLine size={16} />,
                      sm,
                    ),
                    item(
                      'table.insertColRight',
                      t('slideEditor.insertColRight'),
                      <ArrowRightToLine size={16} />,
                      sm,
                    ),
                    item('table.deleteRows', t('slideEditor.deleteRow'), <Rows3 size={16} />, sm),
                    item(
                      'table.deleteCols',
                      t('slideEditor.deleteCol'),
                      <Columns3 size={16} />,
                      sm,
                    ),
                  ],
                },
                {
                  id: 'merge',
                  label: t('slideEditor.groupMerge'),
                  items: [
                    item('table.merge', t('slideEditor.mergeCells'), <Merge size={16} />, sm),
                    item('table.split', t('slideEditor.splitCell'), <Split size={16} />, sm),
                  ],
                },
                {
                  id: 'tableStyle',
                  label: t('slideEditor.groupTableStyle'),
                  items: [
                    item('table.headerRow', t('slideEditor.headerRow'), <PanelTop size={16} />, sm),
                    item('table.bandedRows', t('slideEditor.bandedRows'), <Rows3 size={16} />, sm),
                    item('table.fill', t('slideEditor.cellFill'), <PaintBucket size={16} />, sm),
                  ],
                },
              ],
            },
          ]
        : []),
      {
        id: 'view',
        label: t('slideEditor.viewTab'),
        groups: [
          {
            id: 'views',
            label: t('slideEditor.groupViews'),
            items: [
              item('view.normal', t('slideEditor.normal'), <Rows3 size={18} />),
              item('view.sorter', t('slideEditor.sorter'), <LayoutGrid size={18} />),
              item('view.reading', t('slideEditor.reading'), <BookOpen size={18} />),
              item('view.outline', t('slideEditor.views.outline'), <ListTree size={18} />),
              item('view.notesPage', t('slideEditor.views.notesPage'), <NotebookText size={18} />),
              item('view.animationPane', t('slideEditor.animPane'), <PanelRight size={16} />, sm),
            ],
          },
          {
            id: 'masterViews',
            label: t('slideEditor.views.groupMasters'),
            items: [
              item('view.slideMaster', t('slideEditor.master.view'), <PanelTop size={18} />),
              item(
                'view.handoutMaster',
                t('slideEditor.views.handoutMaster'),
                <Printer size={16} />,
                sm,
              ),
              item(
                'view.notesMaster',
                t('slideEditor.views.notesMaster'),
                <StickyNote size={16} />,
                sm,
              ),
            ],
          },
          sectionsGroup(t, item),
          {
            id: 'window',
            label: t('ribbon.groups.window'),
            items: [
              item('window.new', t('commands.window.new'), <AppWindow size={18} />),
              item('window.arrangeAll', t('commands.window.arrangeAll'), <LayoutGrid size={18} />),
              item(
                'window.sideBySide',
                t('commands.window.sideBySide'),
                <Columns2 size={16} />,
                sm,
              ),
              item(
                'window.syncScroll',
                t('commands.window.syncScroll'),
                <ArrowDownUp size={16} />,
                sm,
              ),
              item(
                'window.resetPosition',
                t('commands.window.resetPosition'),
                <RotateCcw size={16} />,
                sm,
              ),
            ],
          },
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
  }, [
    t,
    registry,
    state,
    exec,
    edit,
    run,
    loadGallery,
    setSaveTemplate,
    current,
    setup.useTimings,
    setup.narration,
    loadWarpPreviews,
    draw.state,
    shortcutsVersion,
  ]);

  // Context menus (CORE-UI-003): shape, picture, table, chart, text, thumbnail, empty slide.
  const allRibbonTabs = [...ribbonTabs, ...extraTabs];
  const menuLookup = ribbonMenuLookup(
    allRibbonTabs,
    registryMenuLookup(
      registry,
      () => state,
      (id) => commandLabel(i18n, id),
      (id) => exec(id),
    ),
  );
  const slideMenu = useContextMenu<SlideMenu>(t('contextMenu.label'), (m) =>
    resolveMenu(SLIDE_MENUS[m.context], menuLookup),
  );
  useEffect(() => {
    ribbonTabsRef.current = allRibbonTabs;
    slideMenuOpen.current = slideMenu.open;
  });
  const latestEdit = useRef(edit);
  useEffect(() => {
    latestEdit.current = edit;
  }, [edit]);
  /** Right click on the slide: select what is under the pointer, then its menu. */
  const onSlideContextMenu = async (e: ReactMouseEvent) => {
    e.preventDefault();
    if (e.timeStamp - keyMenuAt.current < 600) return;
    const at = { x: e.clientX, y: e.clientY };
    setMini(null);
    const cur = latestEdit.current;
    if (editing && cur) {
      slideMenu.open({ ...at, context: slideMenuContext(cur) });
      return;
    }
    const box = slideRef.current?.getBoundingClientRect();
    const size = info?.pages[shown] ?? slideSize;
    if (!box || box.width === 0) return;
    const x = ((e.clientX - box.left) / box.width) * size.width;
    const y = ((e.clientY - box.top) / box.height) * size.height;
    const h = await ctrl.run({ cmd: 'hitTest', x, y });
    const hits = h?.hits ?? [];
    const selected = new Set((cur?.selection ?? []).map((s) => s.id));
    let r = h;
    if (hits.length === 0) r = await ctrl.run({ cmd: 'select', ids: [] });
    else if (!hits.some((id) => selected.has(id))) r = await ctrl.run({ cmd: 'click', x, y });
    if (r) slideMenu.open({ ...at, context: slideMenuContext(r) });
  };

  // Mini toolbar over text selected with the mouse in a shape (CORE-UI-003).
  const [mini, setMini] = useState<{ x: number; y: number } | null>(null);
  const homeItems = new Map<string, RibbonItem>(
    ribbonTabs.flatMap((tb) => tb.groups.flatMap((g) => g.items.map((i) => [i.id, i] as const))),
  );
  const miniItems = SLIDE_MINI.flatMap((id) => {
    const it = homeItems.get(id);
    return it && !it.render ? [it] : [];
  });
  const onTextSelectEnd = (at: { x: number; y: number }) => {
    void ctrl.idle().then(() => {
      const te = latestEdit.current?.textEdit;
      if (te && te.start !== te.end) setMini(at);
    });
  };

  const revision = (i: number) => epoch * 1_000_000 + (revs[i] ?? 0);
  const canvas = (index: number, width: number, className?: string) => (
    <SlideCanvas
      engine={engine}
      glyphs={glyphs}
      images={images}
      index={index}
      slide={info?.pages[index] ?? slideSize}
      width={width}
      revision={revision(index)}
      className={className}
      label={t('slideEditor.slideOf', { n: index + 1, total: count })}
    />
  );

  const mediaLabels = {
    play: t('slideEditor.playMedia'),
    stop: t('slideEditor.stopMedia'),
    unsupported: t('slideEditor.mediaUnsupported'),
  };
  const showStage = (width: number) => (
    <ShowStage
      engine={engine}
      glyphs={glyphs}
      images={images}
      cache={showCache}
      pos={pos}
      from={pos.from}
      slide={info?.pages[pos.slide] ?? slideSize}
      width={width}
      onIdle={onShowIdle}
      label={t('slideEditor.slideOf', { n: pos.slide + 1, total: count })}
      mediaLabels={mediaLabels}
      still={showRun.still}
      onLink={view === 'show' || view === 'presenter' ? followLink : undefined}
      narration={setup.narration && !recording}
    />
  );
  /** The show's slide with the presenter tools over it. */
  const surface = (width: number) => {
    const page = info?.pages[pos.slide] ?? slideSize;
    const height = page.width > 0 ? Math.round((width * page.height) / page.width) : 0;
    const cover = black ? 'black' : white ? 'white' : 'none';
    return (
      <ShowSurface
        width={width}
        height={height}
        slide={page}
        index={pos.slide}
        tool={tool}
        onTool={setTool}
        ink={ink}
        onInk={() => setInkRev((n) => n + 1)}
        zoom={zoom}
        onZoom={setZoom}
        cover={cover}
        onCover={(c) => {
          const next = toggleCover(cover, c);
          setBlack(next === 'black');
          setWhite(next === 'white');
        }}
        onGrid={() => setGridOpen(true)}
        penColor={setup.penColor}
      >
        {showStage(width)}
      </ShowSurface>
    );
  };
  const showOverlays = (
    <>
      {gridOpen && (
        <SlideGrid
          order={showRun.order}
          at={pos.at ?? 0}
          thumb={(s, w) => canvas(s, w)}
          onPick={gotoShow}
          onClose={() => setGridOpen(false)}
        />
      )}
      {clock && (
        <RehearsalBar
          slideMs={clock.current(now)}
          totalMs={clock.total(now)}
          paused={clock.paused}
          onNext={() => moveShow('next')}
          onPause={() => {
            if (clock.paused) clock.resume(Date.now());
            else clock.pause(Date.now());
            setNow(Date.now());
          }}
          onRepeat={() => {
            clock.repeat(Date.now());
            setNow(Date.now());
          }}
        />
      )}
      {rec && (
        <RecordingBar
          state={rec.recorder.state}
          slideMs={rec.recorder.elapsed()}
          totalMs={rec.total.total(now)}
          notice={rec.notice}
          onPause={() => {
            const r = rec.recorder;
            if (r.state === 'paused') {
              r.resume();
              rec.total.resume(Date.now());
            } else {
              r.pause();
              rec.total.pause(Date.now());
            }
            setRecRev((n) => n + 1);
            setNow(Date.now());
          }}
          onRestart={() => {
            rec.recorder.restartSlide();
            ink.clear(pos.slide);
            setInkRev((n) => n + 1);
            setNow(Date.now());
          }}
          onStop={endShow}
        />
      )}
      {rec?.session?.video && <CameraPreview session={rec.session} corner={rec.opts.corner} />}
    </>
  );

  const shown = Math.min(current, Math.max(0, count - 1));
  // Section headers (SLIDE-VIEW-001) before their slides; collapsed sections hide them.
  const entries = paneEntries(count, edit?.sections, collapsed);
  const sectionHeader = (k: number, sec: NonNullable<SlideEditResult['sections']>[number]) => (
    <SectionHeader
      key={`s${sec.id}`}
      section={sec}
      index={k}
      collapsed={collapsed.has(sec.id)}
      current={state.section === k}
      onToggle={() =>
        setCollapsed((c) => {
          const n = new Set(c);
          if (n.has(sec.id)) n.delete(sec.id);
          else n.add(sec.id);
          return n;
        })
      }
      onPick={() => {
        setSectionPick(sec.count > 0 ? null : k);
        if (sec.count > 0) goTo(sec.first);
      }}
      onRename={() => {
        setSectionPick(k);
        setSectionRename(true);
      }}
    />
  );
  const thumbs = (
    <nav
      className={styles.thumbs}
      aria-label={t('slideEditor.thumbnails')}
      data-slide-thumbs
      ref={thumbsEl}
    >
      {entries.map((en) => {
        if (en.kind === 'section') return sectionHeader(en.index, en.section);
        const i = en.index;
        return (
          <button
            key={i}
            type="button"
            draggable
            data-testid={`thumb-${i + 1}`}
            className={`${i === shown ? styles.thumbActive : styles.thumb} ${
              dropOver === i && dragFrom !== null && dragFrom !== i ? styles.thumbDrop : ''
            } ${edit?.masterView?.items[i]?.kind === 'layout' ? masterStyles.thumbLayout : ''}`}
            title={
              edit?.masterView?.items[i] ? masterThumbLabel(t, edit.masterView.items[i]) : undefined
            }
            aria-current={i === shown}
            onClick={() => goTo(i)}
            onContextMenu={(e) => {
              e.preventDefault();
              if (e.timeStamp - keyMenuAt.current < 600) return;
              if (i !== shown) goTo(i);
              slideMenu.open({ x: e.clientX, y: e.clientY, context: 'thumbnail' });
            }}
            onDragStart={(e) => {
              setDragFrom(i);
              e.dataTransfer?.setData('text/plain', String(i));
            }}
            onDragOver={(e) => {
              e.preventDefault();
              setDropOver(i);
            }}
            onDragEnd={() => {
              setDragFrom(null);
              setDropOver(null);
            }}
            onDrop={(e) => {
              e.preventDefault();
              const from = dragFrom ?? Number(e.dataTransfer?.getData('text/plain'));
              setDragFrom(null);
              setDropOver(null);
              if (Number.isInteger(from) && from !== i) run({ cmd: 'moveSlide', from, to: i });
            }}
          >
            <span className={styles.thumbNo}>{i + 1}</span>
            {canvas(i, 150)}
            <PeerBadges peers={peersOn.get(i)} />
            {edit?.masterView?.items[i] && (
              <span className={masterStyles.thumbName}>
                {edit.masterView.items[i].name || t('slideEditor.master.unnamed')}
              </span>
            )}
          </button>
        );
      })}
      {count > 0 && !state.masterView && !state.pageMaster && (
        <button type="button" className={styles.thumbAdd} onClick={() => exec('slide.new')}>
          {t('slideEditor.addSlide')}
        </button>
      )}
    </nav>
  );

  // The slide fits the stage unless the user zoomed (status bar slider).
  const fitted = fit(slideSize, { width: stage.width - 32, height: stage.height - 32 });
  const fitZoom = slideSize.width > 0 ? fitted.width / (slideSize.width * PX_PER_PT) : 1;
  const main =
    stageZoom === null
      ? fitted
      : {
          width: Math.round(slideSize.width * PX_PER_PT * stageZoom),
          height: Math.round(slideSize.height * PX_PER_PT * stageZoom),
        };
  const shownZoom = stageZoom ?? (fitZoom > 0 ? fitZoom : 1);
  const uiLanguage = t(`spell.lang.${i18n.language}`, { defaultValue: i18n.language });
  // Status bar texts (also the current values in its "customise" checklist).
  const statusValues: Record<string, string | undefined> = {
    slide: count > 0 ? t('slideEditor.slideOf', { n: shown + 1, total: count }) : undefined,
    language: uiLanguage,
    notes: notesShown ? t('statusBar.on') : t('statusBar.off'),
    comments: commentsOpen ? t('statusBar.on') : t('statusBar.off'),
    zoom: `${Math.round(shownZoom * 100)}%`,
  };
  const elapsed = formatElapsed(now - startedAt);
  const nextInOrder = showRun.order[(pos.at ?? 0) + 1];
  const message = error
    ? t('slideEditor.error', { message: error })
    : refused
      ? t('slideEditor.refused', { code: refused })
      : status;

  return (
    <div className={`bo-root ${styles.root}`} data-theme={theme} data-app="slide">
      {view !== 'show' && view !== 'presenter' && (
        <TitleBar
          app="slide"
          appName={t('app.slide')}
          docName={fileName}
          saveState={state.hasDeck && fileName ? (modified ? 'unsaved' : 'saved') : null}
          saveLabel={t(modified ? 'titleBar.unsaved' : 'titleBar.saved')}
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
                  [...ribbonTabs, ...extraTabs],
                  scriptBindings.quickAccess.map((sc) => ({
                    id: `script.run.${sc.id}`,
                    icon: 'script.run',
                    label: t('script.runBound', { name: sc.name }),
                    shortcut: sc.shortcut,
                    enabled: count > 0,
                    onSelect: () => exec(`script.run.${sc.id}`),
                  })),
                )
          }
        />
      )}
      {picture.dialogs}
      {objects.dialogs}
      {mediaTools.dialogs}
      {slideMenu.element}
      {mini && miniItems.length > 0 && !slideMenu.menu && editing && (
        <MiniToolbar
          x={mini.x}
          y={mini.y}
          items={miniItems}
          label={t('miniToolbar.label')}
          onClose={() => setMini(null)}
        />
      )}
      {shortcuts.element}
      {ribbonCustom.element}
      <CommandSearch
        tabs={[...ribbonTabs, ...extraTabs]}
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        labels={{
          title: t('commandSearch.title'),
          placeholder: t('commandSearch.placeholder'),
          noResults: t('commandSearch.noResults'),
        }}
      />
      {view !== 'show' && view !== 'presenter' && (
        <Ribbon
          tabs={[...ribbonTabs, ...extraTabs]}
          labels={ribbonLabels(t)}
          autoActivate={AUTO_CONTEXTS}
          fileMenu={{
            label: t('ribbon.tabs.file'),
            open: backstage,
            onOpen: () => setBackstage(true),
          }}
          {...ribbonCustom.ribbonProps()}
          // Contextual tabs come and go with the selection; the ribbon falls back by itself.
          value={ribbonTab}
          onValueChange={(v) => {
            setRibbonTab(v);
            // The pane belongs to the Transitions / Animations tabs.
            if ((v === 'transitions' || v === 'animations') && view === 'normal') setPaneOpen(true);
          }}
        />
      )}
      {ribbonCustom.below && view !== 'show' && view !== 'presenter' && (
        <QuickAccessBar
          below
          label={ribbonCustom.qatLabel}
          items={ribbonCustom.quickAccess([...ribbonTabs, ...extraTabs])}
        />
      )}
      <FileBackstage
        app="slide"
        open={backstage}
        onClose={closeBackstage}
        commands={backstageCommands}
        hasDocument={state.hasDeck}
        fileName={fileName}
        variant={info?.variant}
        macros={info?.macros}
        properties={docProperties}
        shell={platform.shell}
        openPath={
          platform.files.openPath
            ? (path) =>
                void platform.files.openPath?.(path).then(
                  (f) => load(f.bytes, f.name),
                  (e: unknown) => fail(e),
                )
            : undefined
        }
        onSaveAs={(ext) => void saveAsTypeChosen(ext)}
      />
      <SaveAsTypeDialog
        open={saveAsType}
        app="slide"
        current={info?.variant}
        macros={info?.macros}
        onCancel={() => setSaveAsType(false)}
        onConfirm={(ext) => {
          setSaveAsType(false);
          void saveAsTypeChosen(ext);
        }}
      />
      {macroBanner && view === 'normal' && (
        <MacroBanner inline onDismiss={() => setMacroBanner(false)} />
      )}
      {view === 'normal' && <FileProtectionUi app="slide" p={protection} name={fileName} />}
      {passwordPrompt && (
        <PasswordDialog
          name={passwordPrompt.name}
          wrong={passwordPrompt.wrong}
          onSubmit={(pw) => {
            const p = passwordPrompt;
            setPasswordPrompt(null);
            void load(p.bytes, p.name, pw).catch(fail);
          }}
          onCancel={() => setPasswordPrompt(null)}
        />
      )}
      {savePassword && (
        <SavePasswordPanel
          onSet={(password) => {
            run({ cmd: 'setSavePassword', password });
            setStatus(password ? t('security.passwordSet') : t('security.passwordRemoved'));
          }}
          onClose={() => setSavePassword(false)}
        />
      )}
      {view === 'normal' && (
        <div className={styles.normal}>
          {thumbs}
          <div className={styles.center}>
            {count > 0 && (
              <div
                className={styles.formatBar}
                role="toolbar"
                aria-label={t('slideEditor.groupFont')}
              >
                <select
                  data-testid="slide-font"
                  aria-label={t('slideEditor.font')}
                  disabled={!state.hasText}
                  value={edit?.format.font ?? ''}
                  onChange={(e) => {
                    exec(`format.font.${e.target.value}`);
                    refocusText();
                  }}
                >
                  {[...new Set([edit?.format.font ?? '', ...FONTS])].map((f) => (
                    <option key={f} value={f} disabled={!FONTS.includes(f)}>
                      {f}
                    </option>
                  ))}
                </select>
                <select
                  data-testid="slide-line-spacing"
                  aria-label={t('slideEditor.lineSpacing')}
                  disabled={!state.hasText}
                  value={String(edit?.format.lineSpacing ?? 1)}
                  onChange={(e) => {
                    exec(`format.lineSpacing.${e.target.value}`);
                    refocusText();
                  }}
                >
                  {[...new Set([edit?.format.lineSpacing ?? 1, ...LINE_SPACINGS])].map((v) => (
                    <option key={v} value={String(v)} disabled={!LINE_SPACINGS.includes(v)}>
                      {v.toLocaleString()}
                    </option>
                  ))}
                </select>
              </div>
            )}
            <div
              className={styles.stage}
              ref={stageRef}
              data-testid="slide-stage"
              data-zoomed={stageZoom === null ? undefined : 'true'}
            >
              {count > 0 ? (
                <div
                  className={styles.slideWrap}
                  ref={slideRef}
                  tabIndex={-1}
                  style={{ width: main.width, height: main.height }}
                  onContextMenu={(e) => void onSlideContextMenu(e)}
                  onPointerDown={() => setMini(null)}
                >
                  {canvas(shown, main.width, styles.mainSlide)}
                  {asYouType && !state.masterView && (
                    <SlideSpellLayer
                      engine={engine}
                      controller={ctrl}
                      container={slideRef}
                      slide={shown}
                      size={info?.pages[shown] ?? slideSize}
                      width={main.width}
                      marks={spelling.marks}
                      onChanged={spelling.refresh}
                    />
                  )}
                  <SlideOverlay
                    slide={info?.pages[shown] ?? slideSize}
                    width={main.width}
                    edit={edit}
                    preview={preview}
                    controller={ctrl}
                    editing={editing}
                    onStartEdit={startEdit}
                    onExitEdit={exitEdit}
                    onTextShortcut={textShortcut}
                    onTextSelectEnd={onTextSelectEnd}
                    label={t('slideEditor.editor')}
                    editorLabel={t('slideEditor.textEditor')}
                    adjustLabel={t('slideEditor.adjustHandle')}
                    connectorLabel={t('slideEditor.connectorEnd')}
                    onStartCellEdit={startCellEdit}
                    onActivateObject={() => void objectHub.activate()}
                    peers={peers
                      .filter((p) => p.slide === shown && !p.slideOnly)
                      .map((p) => {
                        const name = p.name || t('collab.anonymous');
                        return { ...p, name, label: t('collab.selected', { name }) };
                      })}
                  />
                  <HandoutSlots
                    info={edit?.pageMaster}
                    scale={main.width / (slideSize.width || 1)}
                  />
                  {state.editingPoints && edit?.freeform && edit.slide === shown && (
                    <EditPointsOverlay
                      key={edit.freeform.id}
                      slide={info?.pages[shown] ?? slideSize}
                      width={main.width}
                      freeform={edit.freeform}
                      run={runResult}
                      onExit={() => setEditingPoints(false)}
                      labels={{
                        overlay: t('slideEditor.shapeTools.overlay'),
                        point: (n) => t('slideEditor.shapeTools.point', { n }),
                        handle: t('slideEditor.shapeTools.handle'),
                        addHint: t('slideEditor.shapeTools.addHint'),
                        deletePoint: t('slideEditor.shapeTools.deletePoint'),
                        corner: t('slideEditor.shapeTools.corner'),
                        smooth: t('slideEditor.shapeTools.smooth'),
                        symmetric: t('slideEditor.shapeTools.symmetric'),
                        straight: t('slideEditor.shapeTools.straight'),
                        curved: t('slideEditor.shapeTools.curved'),
                      }}
                    />
                  )}
                  {draw.layer}
                  {draw.replay(main.width)}
                  {edit?.slide === shown && state.isModel3d && edit.model3d && (
                    <Model3dLayer
                      key={edit.model3d.id}
                      model={edit.model3d}
                      scale={main.width / (slideSize.width || 1)}
                      panZoom={!!state.panZoom3d}
                      run={runResult}
                      labels={{
                        rotate: t('slideEditor.model3d.rotate'),
                        panZoom: t('slideEditor.model3d.panZoomHint'),
                      }}
                    />
                  )}
                  {edit?.slide === shown && (edit.media?.length ?? 0) > 0 && (
                    <MediaButtons
                      engine={engine}
                      media={edit.media ?? []}
                      scale={main.width / (slideSize.width || 1)}
                      labels={mediaLabels}
                    />
                  )}
                </div>
              ) : (
                <p className={styles.empty}>{t('slideEditor.empty')}</p>
              )}
            </div>
            {count > 0 && notesShown && !state.masterView && !state.pageMaster && (
              <section className={styles.notes} aria-label={t('slideEditor.notes')}>
                <textarea
                  className={styles.notesEditor}
                  data-testid="notes-editor"
                  aria-label={t('slideEditor.notes')}
                  placeholder={t('slideEditor.notesPlaceholder')}
                  value={notesDraft ?? toEditorText(edit?.notes ?? '')}
                  onChange={(e) => setNotesDraft(e.target.value)}
                  onBlur={commitNotes}
                />
              </section>
            )}
          </div>
          {checkOpen && (
            <SlideCheckDocumentDialog run={runResult} onClose={() => setCheckOpen(false)} />
          )}
          {spelling.dialog && (
            <SlideSpellDialog
              engine={engine}
              controller={ctrl}
              onClose={() => setSpellDialog(false)}
              onShow={showSpellIssue}
              onChanged={spelling.refresh}
            />
          )}
          <SlideSizeDialog
            open={sizeOpen}
            cx={edit?.design?.cx ?? 12_192_000}
            cy={edit?.design?.cy ?? 6_858_000}
            onClose={() => setSizeOpen(false)}
            onApply={(cmd) => {
              setSizeOpen(false);
              run(cmd);
            }}
          />
          <SectionRenameDialog
            open={sectionRename}
            initial={state.section != null ? (edit?.sections?.[state.section]?.name ?? '') : ''}
            onClose={() => setSectionRename(false)}
            onRename={(name) => {
              setSectionRename(false);
              if (state.section != null) run({ cmd: 'renameSection', index: state.section, name });
            }}
          />
          <RenameDialog
            open={renameOpen}
            initial={edit?.masterView?.items[edit.slide]?.name ?? ''}
            onClose={() => setRenameOpen(false)}
            onRename={(name) => {
              setRenameOpen(false);
              run({ cmd: 'renameMasterItem', name });
            }}
          />
          <SlideClipboardUi
            paste={pasteUi && pasteUi.result === edit ? pasteUi : null}
            anchor={selectionCorner}
            onChoose={(c) => void switchPaste(c).catch(fail)}
            panel={clipPanel}
            onPasteItem={(item) => void pasteItem(item).catch(fail)}
            onClosePanel={() => setClipPanel(false)}
          />
          {symbolsOpen && (
            <SymbolPanel
              fonts={[...new Set([edit?.format.font, ...FONTS].filter((x): x is string => !!x))]}
              font={edit?.format.font ?? undefined}
              onInsert={insertSymbol}
              onClose={() => setSymbolsOpen(false)}
            />
          )}
          <TaskPaneHost
            placement="flex"
            layout={paneLayout}
            onLayout={setPaneLayout}
            panes={[
              ...(selectionOpen && count > 0
                ? [
                    {
                      id: 'selection',
                      title: t('taskPane.selection.title'),
                      icon: <Layers size={14} aria-hidden />,
                      content: (
                        <SelectionPane
                          objects={edit?.objects ?? []}
                          selected={(edit?.selection ?? []).map((o) => o.id)}
                          run={run}
                        />
                      ),
                    } satisfies TaskPaneDef,
                  ]
                : []),
              ...(paneOpen && count > 0
                ? [
                    {
                      id: 'animation',
                      title: t('slideEditor.animPane'),
                      icon: <PanelRight size={14} aria-hidden />,
                      content: (
                        <AnimationPane
                          embedded
                          engine={engine}
                          glyphs={glyphs}
                          images={images}
                          edit={edit}
                          slideSize={info?.pages[shown] ?? slideSize}
                          revision={revision(shown)}
                          run={run}
                          onClose={() => setPaneOpen(false)}
                          previewToken={previewToken}
                          pickSound={() => platform.files.openFile(['.wav'])}
                        />
                      ),
                    } satisfies TaskPaneDef,
                  ]
                : []),
              ...(commentsOpen && count > 0
                ? [
                    {
                      id: 'comments',
                      title: t('slideEditor.comments.title'),
                      icon: <MessageSquareText size={14} aria-hidden />,
                      content: (
                        <CommentThreads
                          embedded
                          threads={(edit?.comments ?? []).map((th) => ({
                            ...th,
                            anchor: t('slideEditor.slideOf', { n: shown + 1, total: count }),
                          }))}
                          labels={{
                            title: t('slideEditor.comments.title'),
                            empty: t('slideEditor.comments.empty'),
                            newComment: t('slideEditor.comments.new'),
                            newPlaceholder: t('slideEditor.comments.newPlaceholder'),
                            reply: t('slideEditor.comments.reply'),
                            replyPlaceholder: t('slideEditor.comments.replyPlaceholder'),
                            resolve: t('slideEditor.comments.resolve'),
                            reopen: t('slideEditor.comments.reopen'),
                            delete: t('slideEditor.comments.delete'),
                            close: t('slideEditor.comments.close'),
                            resolved: t('slideEditor.comments.resolved'),
                            assignTo: t('comments.assignTo'),
                            noMatches: t('comments.noMatches'),
                            people: t('comments.people'),
                            assignedTo: t('comments.assignedTo', { name: '{name}' }),
                            taskDone: t('comments.taskDone', { name: '{name}' }),
                          }}
                          newAnchor={t('slideEditor.slideOf', { n: shown + 1, total: count })}
                          searchPeople={
                            platform.people ? (q) => platform.people!.search(q) : undefined
                          }
                          onAdd={(_text, extra) => void postComment(null, extra)}
                          onReply={(parent, _text, extra) => void postComment(parent, extra)}
                          onResolve={(id, done) => run({ cmd: 'resolveComment', id, done })}
                          onDelete={(id) => run({ cmd: 'deleteComment', id })}
                          onClose={() => setCommentsOpen(false)}
                        />
                      ),
                    } satisfies TaskPaneDef,
                  ]
                : []),
              ...(versionsOpen
                ? [
                    {
                      id: 'versions',
                      title: t('versions.slideTitle'),
                      icon: <History size={14} aria-hidden />,
                      content: (
                        <SlideVersionsPane
                          embedded
                          engine={engine}
                          versions={platform.versions}
                          openFile={() =>
                            platform.files.openFile(['.pptx', '.pptm', '.ppt', '.odp'])
                          }
                          onShow={(slide, shapeId) => {
                            if (slide !== current) goTo(slide);
                            if (shapeId !== undefined) run({ cmd: 'select', ids: [shapeId] });
                          }}
                          onClose={() => setVersionsOpen(false)}
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
                          app="slide"
                          host={() =>
                            count > 0
                              ? slideScriptHost({ run: runResult, onEnd: focusSlide })
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
            ]}
            onClose={(id) => {
              if (id === 'selection') setSelectionOpen(false);
              else if (id === 'animation') setPaneOpen(false);
              else if (id === 'comments') setCommentsOpen(false);
              else if (id === 'script') setScriptOpen(false);
              else setVersionsOpen(false);
            }}
            labels={taskPaneLabels(t)}
            focusApp={focusSlide}
          />
        </div>
      )}
      {view === 'sorter' && (
        <div className={styles.sorter}>
          {entries.map((en) => {
            if (en.kind === 'section')
              return (
                <div key={`s${en.section.id}`} style={{ flexBasis: '100%' }}>
                  {sectionHeader(en.index, en.section)}
                </div>
              );
            const i = en.index;
            return (
              <button
                key={i}
                type="button"
                draggable
                data-testid={`sorter-${i + 1}`}
                className={`${i === shown ? styles.sortItemActive : styles.sortItem} ${
                  sortOver === i && sortFrom !== null && sortFrom !== i ? styles.thumbDrop : ''
                }`}
                onClick={() => goTo(i)}
                onDoubleClick={() => {
                  goTo(i);
                  setView('normal');
                }}
                onDragStart={(e) => {
                  setSortFrom(i);
                  e.dataTransfer?.setData('text/plain', String(i));
                }}
                onDragOver={(e) => {
                  e.preventDefault();
                  setSortOver(i);
                }}
                onDragEnd={() => {
                  setSortFrom(null);
                  setSortOver(null);
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  const from = sortFrom ?? Number(e.dataTransfer?.getData('text/plain'));
                  setSortFrom(null);
                  setSortOver(null);
                  if (Number.isInteger(from) && from !== i) run({ cmd: 'moveSlide', from, to: i });
                }}
              >
                {canvas(i, 220)}
                <span>{i + 1}</span>
              </button>
            );
          })}
        </div>
      )}
      {view === 'outline' && (
        <OutlineView
          runResult={runResult}
          revision={revs.reduce((a, b) => a + b, epoch * 1_000_000)}
          current={shown}
          onSelect={goTo}
          preview={count > 0 ? canvas(shown, 420) : undefined}
        />
      )}
      {view === 'notesPage' && (
        <NotesPageView
          runResult={runResult}
          run={run}
          revision={revs.reduce((a, b) => a + b, epoch * 1_000_000)}
          current={shown}
          onSelect={goTo}
          canvas={(i, w) => canvas(i, w)}
        />
      )}
      {view === 'reading' && (
        <div
          className={styles.reading}
          ref={stageRef}
          data-testid="reading-view"
          onClick={clickShow}
        >
          {count > 0 && pos.slide < count ? (
            showStage(fit(slideSize, { width: stage.width, height: stage.height - 48 }).width)
          ) : (
            <p className={styles.endText}>{t('slideEditor.end')}</p>
          )}
          <div className={styles.readingNav} onClick={(e) => e.stopPropagation()}>
            <button type="button" data-testid="reading-prev" onClick={() => moveShow('prev')}>
              {t('slideEditor.previous')}
            </button>
            <span data-testid="reading-status">
              {t('slideEditor.slideOf', { n: Math.min(pos.slide, count - 1) + 1, total: count })}
            </span>
            <button type="button" data-testid="reading-next" onClick={() => moveShow('next')}>
              {t('slideEditor.next')}
            </button>
          </div>
        </div>
      )}
      {view === 'show' && (
        <div
          className={styles.show}
          ref={showRef}
          onClick={clickShow}
          onContextMenu={(e) => {
            e.preventDefault();
            moveShow('prev');
          }}
        >
          {pos.slide < count ? (
            surface(fit(slideSize, { width: window.innerWidth, height: window.innerHeight }).width)
          ) : (
            <p className={styles.endText}>{t('slideEditor.end')}</p>
          )}
          {showOverlays}
        </div>
      )}
      {view === 'presenter' && (
        <div className={styles.presenter}>
          <div className={styles.presenterMain} ref={stageRef}>
            {pos.slide < count ? (
              surface(main.width)
            ) : (
              <p className={styles.endText}>{t('slideEditor.end')}</p>
            )}
            {showOverlays}
          </div>
          <aside className={styles.presenterSide}>
            <div className={styles.timer}>
              <span>
                {t('slideEditor.elapsed')}: <strong>{elapsed}</strong>
              </span>
              <span>
                {t('slideEditor.clock')}: {new Date(now).toLocaleTimeString()}
              </span>
            </div>
            <h3>{t('slideEditor.next')}</h3>
            {nextInOrder !== undefined ? canvas(nextInOrder, 280) : <p>{t('slideEditor.end')}</p>}
            <h3>{t('slideEditor.notes')}</h3>
            <div className={styles.presenterNotes}>{notes || t('slideEditor.noNotes')}</div>
            <div className={styles.presenterButtons}>
              <button type="button" onClick={() => moveShow('prev')}>
                {t('slideEditor.previous')}
              </button>
              <button type="button" onClick={() => moveShow('next')}>
                {t('slideEditor.next')}
              </button>
              <button
                type="button"
                onClick={() => {
                  setBlack((b) => !b);
                  setWhite(false);
                }}
              >
                {t('slideEditor.blackScreen')}
              </button>
              <button
                type="button"
                data-testid="presenter-white"
                onClick={() => {
                  setWhite((w) => !w);
                  setBlack(false);
                }}
              >
                {t('showTools.white')}
              </button>
              <button type="button" data-testid="presenter-grid" onClick={() => setGridOpen(true)}>
                {t('showTools.grid')}
              </button>
              {!audience.open && platform.channel && (
                <button type="button" onClick={() => exec('show.audience')}>
                  {t('slideEditor.audienceWindow')}
                </button>
              )}
              <button type="button" onClick={endShow}>
                {t('slideEditor.exit')}
              </button>
            </div>
          </aside>
        </div>
      )}
      <input
        id={colorId}
        type="color"
        className={styles.colorInput}
        tabIndex={-1}
        aria-hidden="true"
        onChange={(e) => {
          const hex = hexOf(e.target.value);
          if (!hex) return;
          const target = colorTarget;
          if (target === 'font') run({ cmd: 'formatText', format: { kind: 'color', hex } });
          else if (target === 'cell') run({ cmd: 'tableFill', color: hex });
          else if (target === 'highlight') {
            run({ cmd: 'formatText', format: { kind: 'highlight', hex } });
          } else if (target === 'fill') run({ cmd: 'setFill', color: hex });
          else run({ cmd: 'setLine', color: hex });
        }}
      />
      {smartArtDialog && (
        <SmartArtDialog
          mode={smartArtDialog.mode}
          initial={smartArtDialog.initial}
          onClose={() => setSmartArtDialog(null)}
          onApply={applySmartArt}
        />
      )}
      {zoomDialog && (
        <ZoomInsertDialog
          kind={zoomDialog}
          count={count}
          sections={edit?.sections ?? []}
          thumb={(i) => canvas(i, 112)}
          t={t}
          onClose={() => setZoomDialog(null)}
          onInsert={(picked) => {
            const c = zoomInsertCommand(zoomDialog, picked, {
              title: t('slideEditor.zoom.summaryTitle'),
              summary: t('slideEditor.zoom.summarySection'),
              section: t('slideEditor.zoom.sectionN'),
              first: t('slideEditor.sections.firstSection'),
            });
            if (c) run(c);
          }}
        />
      )}
      {model3dAlt && (
        <Model3dAltTextDialog
          open
          model={edit?.model3d}
          t={t}
          onClose={() => setModel3dAlt(false)}
          onSave={(text) => run({ cmd: 'model3dAltText', text })}
        />
      )}
      {chartDialog && (
        <ChartDialog
          open
          mode={chartDialog.mode}
          initial={chartDialog.initial}
          onClose={() => setChartDialog(null)}
          onApply={applyChart}
        />
      )}
      {saveTemplate && (
        <SaveChartTemplateDialog
          load={loadGallery}
          onClose={() => setSaveTemplate(false)}
          onSaved={(name) => setStatus(t('chartDesign.templateSaved', { name }))}
        />
      )}
      <CustomShowsDialog
        open={customOpen}
        shows={customShows ?? []}
        count={count}
        slideLabel={(i) => t('showTools.slideN', { n: i + 1 })}
        canLink={state.hasSelection}
        error={customOpen ? refused : undefined}
        onClose={() => setCustomOpen(false)}
        onSave={(id, name, slides) =>
          run({ cmd: 'setCustomShow', ...(id !== undefined ? { id } : {}), name, slides })
        }
        onDelete={(id) => run({ cmd: 'deleteCustomShow', id })}
        onPlay={(id) => {
          setCustomOpen(false);
          startShow(0, false, { custom: id });
        }}
        onLink={(id, ret) =>
          run({ cmd: 'setShowLink', action: { kind: 'customShow', id, return: ret } })
        }
      />
      {setupOpen && (
        <SetupShowDialog
          open
          setup={setup}
          customShows={customShows ?? []}
          count={count}
          displays={platform.displays?.capabilities()}
          findScreens={platform.displays ? () => platform.displays!.screens() : undefined}
          target={displayTarget}
          onClose={() => setSetupOpen(false)}
          onApply={(s, target) => {
            setDisplayTarget(target);
            setSetupOpen(false);
            run({ cmd: 'setShowSetup', setup: s });
          }}
        />
      )}
      <EndShowDialog key={endKey} state={endState} onDone={finishEnd} />
      {recordAsk && (
        <RecordSetupDialog
          devices={platform.media?.available() ?? null}
          initial={recOpts}
          onCancel={() => setRecordAsk(null)}
          onStart={(o) => void startRecording(recordAsk.from, o)}
        />
      )}
      {exportOpen && (
        <ExportDialog
          open
          slide={slideSize}
          count={count}
          busy={exporting}
          onClose={() => setExportOpen(false)}
          onExport={(o) => void exportPng(o)}
        />
      )}
      {videoOpen && (
        <ExportVideoDialog
          slide={slideSize}
          video={platform.video}
          onClose={() => setVideoOpen(false)}
          run={exportVideo}
        />
      )}
      {printOpen && engine.printPdf && engine.printPreview && (
        <PrintPanel
          app="slide"
          host={{
            printPdf: engine.printPdf.bind(engine),
            printPreview: engine.printPreview.bind(engine),
            getGlyphOutlines: engine.getGlyphOutlines.bind(engine),
            getMedia: engine.getMedia.bind(engine),
          }}
          initial={{ current }}
          onPrint={(pdf) =>
            platform.print.printPdf(
              pdf,
              fileName.replace(/\.pptx$/i, '') || t('slideEditor.untitled'),
            )
          }
          onClose={() => setPrintOpen(false)}
          fail={fail}
        />
      )}
      {pdfOptionsOpen && (
        <PdfExportDialog
          app="slide"
          onClose={() => setPdfOptionsOpen(false)}
          onExport={(c) => {
            void (async () => {
              setExporting(true);
              try {
                if (!engine.exportSlides) throw new Error(t('slideExport.unsupported'));
                await ctrl.idle();
                const bytes = await engine.exportSlides(c);
                await platform.files.saveFile(bytes, `${baseName()}.pdf`, 'application/pdf');
                setStatus(t('slideExport.done'));
              } catch (e) {
                fail(e);
              } finally {
                setExporting(false);
              }
            })();
          }}
        />
      )}
      {exportDocsOpen && (
        <ExportDocsDialog
          open
          count={count}
          slide={slideSize}
          today={new Date().toLocaleDateString(i18n.language)}
          busy={exporting}
          onClose={() => setExportDocsOpen(false)}
          onExport={(s) => void exportDocs(s)}
        />
      )}
      {view !== 'show' && (
        <StatusBar
          customize={{
            label: t('statusBar.customize'),
            items: STATUS_ITEMS.slide.map((d) => ({
              id: d.id,
              label: t(`statusBar.items.${d.id}`),
              checked: statusBar.shown(d.id),
              ...(statusValues[d.id] !== undefined ? { value: statusValues[d.id] } : {}),
            })),
            onToggle: statusBar.toggle,
            resetLabel: t('statusBar.reset'),
            onReset: statusBar.reset,
          }}
          left={
            <>
              {(message || statusBar.shown('slide')) && (
                <span data-testid="slide-status">{message || (statusValues.slide ?? '')}</span>
              )}
              {count > 0 && statusBar.shown('language') && (
                <button
                  type="button"
                  className={styles.statusButton}
                  data-testid="status-language"
                  title={t('commands.review.spellingDialog')}
                  onClick={() => exec('review.spellingDialog')}
                >
                  {uiLanguage}
                </button>
              )}
              {count > 0 && <ProtectionStatus info={protection.info} />}
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
              <span>
                {fileName}
                {modified && (
                  <span
                    className={styles.modified}
                    data-testid="status-modified"
                    title={t('slideEditor.unsaved')}
                  >
                    {t('slideEditor.modified')}
                  </span>
                )}
              </span>
              {count > 0 && (statusBar.shown('notes') || statusBar.shown('comments')) && (
                <StatusButtons
                  label={t('statusBar.items.notes')}
                  testId="status-panes"
                  buttons={[
                    ...(statusBar.shown('notes')
                      ? [
                          {
                            id: 'notes',
                            label: t('statusBar.notes'),
                            icon: <StickyNote size={14} />,
                            pressed: notesShown,
                            onClick: () => exec('view.notes'),
                          },
                        ]
                      : []),
                    ...(statusBar.shown('comments')
                      ? [
                          {
                            id: 'comments',
                            label: t('statusBar.comments'),
                            icon: <MessageSquare size={14} />,
                            pressed: commentsOpen,
                            onClick: () => exec('review.comments'),
                          },
                        ]
                      : []),
                  ]}
                />
              )}
              {count > 0 && statusBar.shown('viewButtons') && (
                <StatusButtons
                  label={t('statusBar.views')}
                  testId="status-views"
                  buttons={(
                    [
                      ['normal', 'slideEditor.normal', <Rows3 key="i" size={14} />],
                      ['sorter', 'slideEditor.sorter', <LayoutGrid key="i" size={14} />],
                      ['reading', 'slideEditor.reading', <BookOpen key="i" size={14} />],
                    ] as const
                  ).map(([v, label, icon]) => ({
                    id: v,
                    label: t(label),
                    icon,
                    pressed: view === v,
                    onClick: () => exec(`view.${v}`),
                  }))}
                />
              )}
              {count > 0 &&
                view === 'normal' &&
                (statusBar.shown('zoomSlider') || statusBar.shown('zoom')) && (
                  <StatusZoom
                    zoom={shownZoom}
                    min={0.1}
                    max={4}
                    slider={statusBar.shown('zoomSlider')}
                    percent={statusBar.shown('zoom')}
                    onZoom={setStageZoom}
                    onPercent={() => setStageZoom(null)}
                    labels={{
                      slider: t('statusBar.zoomSlider'),
                      zoomIn: t('commands.view.zoomIn'),
                      zoomOut: t('commands.view.zoomOut'),
                      percent: t('statusBar.zoomPercentFit', {
                        value: Math.round(shownZoom * 100),
                      }),
                    }}
                  />
                )}
              {count > 0 && view === 'normal' && statusBar.shown('fit') && (
                <StatusButtons
                  label={t('statusBar.fit')}
                  buttons={[
                    {
                      id: 'fit',
                      label: t('statusBar.fit'),
                      icon: <Maximize2 size={14} />,
                      pressed: stageZoom === null,
                      onClick: () => exec('view.fitWindow'),
                    },
                  ]}
                />
              )}
            </>
          }
        />
      )}
      {windowTools.element}
      {readAloud.bar}
      {dictation.bar}
      {translate.pane}
    </div>
  );
}
