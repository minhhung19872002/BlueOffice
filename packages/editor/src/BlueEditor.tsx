import {
  DocumentView,
  MAX_ZOOM,
  MIN_ZOOM,
  type DocumentViewOptions,
  type ImeEvent,
} from '@blueoffice/canvas-host';
import { overtypeInsert } from './overtype';
import { WordCountDialog } from './WordCountDialog';
import { useWindowTools, type WindowTools } from './windowTools';
import { loadIgnoredWords, loadUserWords, saveUserWord } from './SpellMenu';
import {
  LanguageDialog,
  PROOF_LANGUAGES,
  SpellCheckDialog,
  loadProofOptions,
} from './SpellCheckDialog';
import { AutoCorrectDialog, loadAutoCorrect, openSetupCommand } from './AutoCorrectDialog';
import { MacroViewer, useMacroViewer } from './MacroViewer';
import { DocCheckDocumentDialog } from './DocCheckDocument';
import { VersionsPane } from './versions/VersionsPane';
import { createI18n, I18nextProvider, type Language } from '@blueoffice/i18n';
import type {
  CharFormat,
  EditCommand,
  OpenedFile,
  ParaFormat,
  DocPasteMode,
  PdfExport,
  Platform,
  RecentFile,
  SigningDefaults,
  SignRequest,
  SpellInfo,
  StoredDocument,
  TableEdit,
  VbaProjectInfo,
  ViewLayoutMode,
} from '@blueoffice/platform';
import {
  BLOCK_STORE_KEY,
  PASSWORD_REQUIRED,
  SPLIT_DEFAULT,
  SPLIT_MIN,
  THEME_MIME,
  WEB_FORMAT_COMMANDS,
  clampSplit,
  dragSplit,
  WRONG_PASSWORD,
  openExtensions,
  openProtection,
  updateNotice,
  webFormats,
  withExtension,
} from '@blueoffice/platform';
import {
  Ribbon,
  ribbonLabels,
  RIBBON_CONTEXTS,
  CommandSearch,
  STATUS_ITEMS,
  StatusBar,
  StatusButtons,
  StatusZoom,
  getThemePref,
  setThemePref,
  useStatusBarPrefs,
  useThemePref,
  TitleBar,
  QuickAccessBar,
  useCommandSearchShortcut,
  nextDensity,
  nextTheme,
  setDensity,
  getDensity,
  useDensity,
  useResolvedTheme,
  usePictureTools,
  PictureActionsHub,
  useObjectTools,
  ObjectActionsHub,
  TaskPaneHost,
  taskPaneInsets,
  useTaskPaneLayout,
  type TaskPaneDef,
  type RibbonTab,
  type ThemeSetting,
  DOC_MENUS,
  MiniToolbar,
  Popover,
  isContextMenuKey,
  menuPointOf,
  registryMenuLookup,
  resolveMenu,
  ribbonMenuLookup,
  type DocMenuContext,
  type MenuLine,
  type RibbonItem,
} from '@blueoffice/ui';
import { pictureTabLabels, registerPictureCommands } from './pictureTools';
import { objectPlatform, objectToolsLabels, registerObjectCommands } from './objectTools';
import { docObjectContexts, docObjectTabs } from './objectContexts';
import {
  AArrowDown,
  AArrowUp,
  CaseSensitive,
  ArrowDownToLine,
  ArrowUpToLine,
  WandSparkles,
  ClipboardCheck,
  AlignCenter,
  AlignJustify,
  AlignLeft,
  AlignRight,
  BetweenHorizontalEnd,
  BetweenHorizontalStart,
  BetweenVerticalEnd,
  BetweenVerticalStart,
  Bold,
  ClipboardType,
  Columns2,
  Grid2x2,
  Grid3x3,
  LayoutTemplate,
  LibraryBig,
  Paintbrush,
  PanelBottom,
  PanelTop,
  Hash,
  Rows3,
  SquareDashed,
  StickyNote,
  TableCellsMerge,
  TableCellsSplit,
  TableProperties,
  Trash,
  Check,
  CheckCheck,
  ChevronLeft,
  ChevronRight,
  CircleX,
  ClipboardPaste,
  CircleCheck,
  Copy,
  Eye,
  FileDown,
  FilePenLine,
  FileSpreadsheet,
  Files,
  FilePlus,
  FileText,
  FolderOpen,
  Image,
  Italic,
  KeyRound,
  Languages,
  Sparkles,
  Link2,
  List,
  ListIndentDecrease,
  ListIndentIncrease,
  ListOrdered,
  ListTree,
  Lock,
  Mails,
  Mail,
  Database,
  Tags,
  Maximize2,
  MessageSquarePlus,
  MessagesSquare,
  MoonStar,
  Rows4,
  Omega,
  PanelLeft,
  PanelRight,
  Pilcrow,
  Printer,
  Redo2,
  RectangleHorizontal,
  RefreshCw,
  RemoveFormatting,
  Replace,
  Ruler,
  Save,
  ShieldCheck,
  Signature,
  Scaling,
  Scissors,
  Search,
  SeparatorHorizontal,
  Share2,
  SquareDashedMousePointer,
  SquareSplitVertical,
  Strikethrough,
  Subscript,
  Superscript,
  Table,
  Tag,
  Type,
  Underline,
  Undo2,
  Users,
  X,
  ZoomIn,
  ZoomOut,
  SpellCheck,
  SpellCheck2,
  FileCheck2,
  Sigma,
  SquareFunction,
  TextCursorInput,
  GitCompare,
  History,
  GitMerge,
  Braces,
  ListChecks,
  FileCode,
  LockOpen,
  Unlink,
  AlignHorizontalSpaceAround,
  AlignVerticalSpaceAround,
  ArrowDownUp,
  Expand,
  RotateCw,
  Shrink,
  Table2,
  WrapText,
  Frame,
  SquareCheck,
  SquareChevronDown,
  SlidersHorizontal,
  LockKeyhole,
  BookOpen,
  BookOpenText,
  Globe,
  ScrollText,
  Focus,
  PenLine,
  ArrowLeftRight,
  Bookmark,
  Library,
  Quote,
  Scale,
  Volume2,
  Mic,
  Network,
  ChartArea,
  ChartBar,
  ChartColumn,
  ChartLine,
  ChartPie,
  Pencil,
  Palette,
  MessageSquare,
  SquareTerminal,
  FileCode2,
  FileSearch,
  CircleDot,
  Shapes,
  AppWindow,
  LayoutGrid,
  Rows2,
  RotateCcw,
  BookA,
} from 'lucide-react';
import { docScriptHost, recordEngine, ScriptPane, ScriptRecorder } from '@blueoffice/script';
import { docSelection, useAddins } from '@blueoffice/addins';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { CommandRegistry } from './commands';
import { useScriptBindings } from './scriptBindings';
import { DocController, MATH_UNSUPPORTED, type EditView } from './controller';
import styles from './Editor.module.css';
import { FindPanel } from './Panels';
import { SymbolPanel } from './SymbolPanel';
import { IconPanel } from './IconPanel';
import { insertDocIcons } from './insertIcons';
import { DocClipboardUi } from './clipboard/DocClipboardUi';
import { CaptionPanel, CrossRefPanel, MergeFieldPanel } from './ReferencePanels';
import { CommentsPanel, ReviewPanel } from './ReviewPanel';
import { MarkupModeSelect, reviewersOf, ShowMarkupMenu } from './MarkupMenus';
import { ReviewingPane } from './ReviewingPane';
import {
  balloonLabel,
  LockTrackingDialog,
  registerTrackingCommands,
  reviewErrorKey,
} from './TrackingControls';
import { PasswordDialog, SavePasswordPanel } from './SecurityPanels';
import { SaveAsTypeDialog, planOpen, saveName } from './FileVariants';
import {
  BlockGalleryMenu,
  BlocksOrganizer,
  blockErrorKey,
  clearBlockPreviews,
  SaveBlockDialog,
} from './BuildingBlocks';
import { fromBase64, toBase64 } from './clipboard/docPaste';
import {
  identityNames,
  isProtectionRefusal,
  refusalKey,
  OpenProtectionDialog,
  ProtectDialog,
  ProtectionBar,
  ProtectionStatus,
} from './ProtectionPanels';
import { SignaturesPanel, SignDialog } from './SignaturePanels';
import {
  bytesToSign,
  imagesCommand,
  InsertSignatureLineDialog,
  isoDay,
  type LineChoice,
  lineSignRequest,
  newLineId,
  SignLineDialog,
  statusCommand,
} from './SignatureLineDialogs';
import {
  bytesToBase64,
  documentState,
  looksSigned,
  parseRemembered,
  remember,
  SIGNING_SETTING_KEY,
  signedFileName,
  type RememberedSigning,
  type SignForm,
} from './signing';
import { createEditorStore, togglePane, type DocTaskPane, type EditorState } from './store';
import { StylesGallery } from './StylesGallery';
import { StylesPane, type StylesPaneRequest } from './StylesPane';
import { RevealFormattingPane, StyleInspectorPane } from './StyleInspector';
import { taskPaneLabels } from './taskPaneLabels';
import {
  HeaderFooterBar,
  NotesPanel,
  PageSetupDialog,
  LinkDialog,
  PasteSpecialDialog,
  EquationDialog,
  FieldDialog,
  ControlDialog,
  ControlBar,
  PicturePositionDialog,
  TablePropertiesDialog,
} from './DocDialogs';
import { AdminFormatDialog, TemplateDialog } from './TemplateDialogs';
import { ConvertDialog, FormulaDialog, SortDialog } from './TableToolDialogs';
import {
  CommandMenu,
  EnvelopeDialog,
  FinishDialog,
  LabelsDialog,
  ProviderDialog,
  RecipientsDialog,
  RuleDialog,
  RULE_KINDS,
  type FinishMode,
  type RuleKind,
} from './MergeDialogs';
import {
  FormFieldBar,
  FormFieldDialog,
  ProtectFormsDialog,
  formErrorKey,
} from './FormFieldDialogs';
import {
  ColorPicker,
  FontFamilyPicker,
  FontSizeBox,
  SpacingMenu,
  nextFontSize,
} from './FontControls';
import { DefineListDialog, MultilevelMenu, SetNumberingDialog } from './ListDialogs';
import {
  PageBordersDialog,
  StyleSetGallery,
  ThemeGallery,
  WatermarkDialog,
  WatermarkMenu,
} from './PageBackgroundDialogs';
import {
  ColumnsDialog,
  DropCapDialog,
  HyphenationDialog,
  LineNumbersDialog,
  TextDirectionDialog,
} from './LayoutDialogs';
import { PdfExportDialog } from './PdfExportDialog';
import { CombineCharsDialog, FontEffectsDialog, PhoneticGuideDialog } from './FontEffectsDialog';
import { isReferenceDialog, ReferenceDialog } from './ReferenceDialogs';
import { CitationDialog, CitationStylePicker, isCitationDialog } from './CitationDialogs';
import { Rulers, type PageBox } from './Ruler';
import { NavigationPane } from './NavigationPane';
import { XmlMappingPane } from './XmlMappingPane';
import { OutlineView } from './OutlineView';
import { ReadMode } from './ReadMode';
import { FocusedReader } from './FocusedReader';
import { docReaderBlocks, readerStart } from './docFocusedReader';
import { PrintPanel } from './print/PrintPanel';
import { useReadAloud, type ReadAloud } from './ReadAloudBar';
import { registerReadAloudCommands } from './readAloudCommands';
import { docReadRequest } from './docReadAloud';
import { useDictation, type Dictation } from './DictationBar';
import { registerDictationCommands } from './dictationCommands';
import { docDictationTarget } from './docDictation';
import { useTranslate, type TranslateAction } from './TranslatePane';
import { useBlueAi } from './AiPane';
import {
  thesaurusMenuEntry,
  useThesaurus,
  useThesaurusMenu,
  type Thesaurus,
} from './ThesaurusPane';
import { docReplaceWord, docWordAtCaret } from './docThesaurus';
import { AiLink, registerAiCommands } from './aiCommands';
import { DOC_AI_COMMANDS, docAiFeatures } from './docAi';
import { TranslateLink, registerTranslateCommands } from './translateCommands';
import {
  docInsertAfter,
  docSelectionText,
  translateDocumentSegments,
  translatedName,
} from './docTranslate';
import { demoted, promoted } from './outlineModel';
import viewStyles from './Views.module.css';
import {
  SmartArtDialog,
  smartArtPicturePicker,
  type SmartArtDraft,
  type SmartArtPreviewLoader,
} from './SmartArtDialog';
import { ChartDialog, type ChartDraft } from './chart/ChartDialog';
import { SaveChartTemplateDialog } from './chart/chartDesign';
import { ChartFormatPane } from './chart/ChartFormatPane';
import {
  chartCommand,
  docChartTabs,
  docGalleryLoader,
  registerDocChartCommands,
} from './chart/docCharts';
import { FileBackstage, type BackstagePropertiesAccess } from './FileBackstage';
import { useCustomShortcuts } from './ShortcutsDialog';
import { useRibbonCustomization } from './RibbonCustomizeDialog';
import { commandLabel, shortcutCommands, useContextMenu, type OpenContextMenu } from './commandUi';

/**
 * Contexts whose tab is activated when they appear (CORE-UI-001);
 * the caret moves through table cells while typing: the Table tab only shows.
 */
// Caret contexts (a table cell, an equation the caret follows while typing) only show.
const AUTO_CONTEXTS = RIBBON_CONTEXTS.filter((c) => c !== 'table' && c !== 'equation');

/** Options controlled by the embedding host (prompt §3.5). */
export interface HostConfig {
  brandName?: string;
  showShare?: boolean;
  showClose?: boolean;
  onShare?: () => void;
  onClose?: () => void;
}

export interface BlueEditorProps {
  platform: Platform;
  app?: 'doc' | 'sheet' | 'slide';
  language?: Language;
  theme?: ThemeSetting;
  host?: HostConfig;
  /** URL of the sample document (served by the shell). */
  sampleUrl?: string;
  /** Called after a document is opened (for analytics / recent files). */
  onDocumentOpened?: (name: string, pageCount: number) => void;
  /** Autosave interval in ms (0 disables). */
  autosaveMs?: number;
}

const ICON = 20;
const SMALL = 16;
/** Room the docked reviewing pane takes from the pages (CSS px, see ReviewingPane.module.css). */
const REVIEWING_SIZE = { left: 300, bottom: 220 };
/** Offline-cache keys of the crash-recovery copy (prompt §6 Phase 2: autosave + recovery). */
export const AUTOSAVE_KEY = 'bluedoc.autosave.docx';
export const AUTOSAVE_NAME_KEY = 'bluedoc.autosave.name';

type Registry = CommandRegistry<EditorState>;

/** An open BlueDoc context menu (CORE-UI-003). */
type DocMenu = OpenContextMenu & { context: DocMenuContext; spell?: SpellInfo };

/** Right click on a page, from the document view. */
interface DocContextEvent {
  page: number;
  x: number;
  y: number;
  clientX: number;
  clientY: number;
  target: EventTarget | null;
}

/** What the selection is in, for its context menu. */
function docMenuContext(s: EditorState): DocMenuContext {
  const f = s.edit?.format;
  if (f?.pictureTools || f?.picture) return 'picture';
  if (f?.smartArt) return 'shape';
  if (f?.table) return 'table';
  const story = s.edit?.story?.kind;
  if (story === 'header' || story === 'footer') return 'header';
  return 'text';
}

let painterClickAt = 0;
/** Whether this Format Painter click follows the previous one closely (double click). */
function painterDoubleClick(): boolean {
  const now = Date.now();
  const double = now - painterClickAt < 450;
  painterClickAt = now;
  return double;
}

/** The editor. Everything platform-specific comes from `platform`. */
export function BlueEditor(props: BlueEditorProps) {
  const [i18n] = useState(() => createI18n(props.language));
  return (
    <I18nextProvider i18n={i18n}>
      <EditorInner {...props} i18nInstance={i18n} />
    </I18nextProvider>
  );
}

const enc = new TextEncoder();
const dec = new TextDecoder();

function EditorInner({
  platform: basePlatform,
  app = 'doc',
  language = 'vi-VN',
  // Follows the system (dark mode, forced colours / more contrast) like BlueSheet and BlueSlide.
  theme = 'auto',
  host,
  sampleUrl,
  onDocumentOpened,
  autosaveMs = 15_000,
  i18nInstance,
}: BlueEditorProps & { i18nInstance: ReturnType<typeof createI18n> }) {
  const t = useMemo(() => i18nInstance.t.bind(i18nInstance), [i18nInstance]);
  // Macro recorder (CORE-SCRIPT-002): it sees every engine command the editor sends.
  const [recorder] = useState(() => new ScriptRecorder('doc'));
  const platform = useMemo(
    () => ({ ...basePlatform, engine: recordEngine(basePlatform.engine, recorder) }),
    [basePlatform, recorder],
  );
  const [, setRecorderTick] = useState(0);
  useEffect(() => recorder.subscribe(() => setRecorderTick((n) => n + 1)), [recorder]);
  const [store] = useState(() => createEditorStore(language, theme));
  const state = useStore(store);
  // The theme picked in File › Options (remembered) wins over the editor's own setting.
  const themeSetting = useThemePref(state.theme);
  const resolvedTheme = useResolvedTheme(themeSetting);
  const resolvedThemeRef = useRef(resolvedTheme);
  useEffect(() => {
    resolvedThemeRef.current = resolvedTheme;
  });
  const windowsRef = useRef<WindowTools | null>(null);
  const density = useDensity();
  const viewHost = useRef<HTMLDivElement>(null);
  // Command UI (CORE-UI-003 / 004): context menu, mini toolbar, shortcut dialog.
  const contextMenuAt = useRef<(e: DocContextEvent) => Promise<void>>(async () => {});
  const menuAtCaret = useRef<() => void>(() => {});
  /** When Shift+F10 opened the menu: the browser's own keyboard context menu event follows. */
  const keyMenuAt = useRef(0);
  const ribbonTabsRef = useRef<RibbonTab[]>([]);
  const [mini, setMini] = useState<{ x: number; y: number } | null>(null);
  const view = useRef<DocumentView | null>(null);
  // Split view (CORE-UI-007): the second pane's page view and which pane has the keyboard.
  const splitHost = useRef<HTMLDivElement>(null);
  const splitView = useRef<DocumentView | null>(null);
  const activePane = useRef<0 | 1>(0);
  const paneOptions = useRef<DocumentViewOptions>({});
  // Read aloud (CORE-LANG-003): the commands reach the hook through a ref.
  const readAloud = useReadAloud(platform.speech, 'paragraph');
  const readAloudRef = useRef<ReadAloud | null>(null);
  useEffect(() => {
    readAloudRef.current = readAloud;
  });
  // Translation (CORE-LANG-001): the commands reach the pane through a link.
  const translate = useTranslate(platform.translation, platform.clipboard);
  const [translateLink] = useState(() => new TranslateLink());
  useEffect(() => {
    translateLink.attach(translate);
  });
  // BlueAI (CORE-AI-001): a task pane; the commands reach it through a link.
  const ai = useBlueAi(platform.ai, platform.clipboard);
  const [aiLink] = useState(() => new AiLink());
  useEffect(() => {
    aiLink.attach(ai);
  });
  // Thesaurus (CORE-THES-001): a task pane and a context-menu submenu; the command reaches the
  // pane through a ref.
  const thesaurus = useThesaurus(platform.thesaurus);
  const thesaurusRef = useRef<Thesaurus | null>(null);
  useEffect(() => {
    thesaurusRef.current = thesaurus;
  });
  const thesMenu = useThesaurusMenu(platform.thesaurus);
  const controller = useRef<DocController | null>(null);
  // Runs the `insert.equation` command (set once the registry exists): a double click or Enter
  // on a selected equation edits it in the equation dialog.
  const equationDialog = useRef<() => void>(() => {});
  // Stop recording waits for keys typed just before it (queued in the controller).
  useEffect(
    () =>
      recorder.setSettle(() => controller.current?.query({ cmd: 'state' }) ?? Promise.resolve()),
    [recorder],
  );
  const stored = useRef<StoredDocument | null>(null);
  /** Bytes of the opened file (its digital signatures are verified on them). */
  const openedBytes = useRef<Uint8Array | null>(null);
  /** The template the open document was created from (CORE-FMT-001: "Edit template"). */
  const templateFile = useRef<OpenedFile | null>(null);
  /** "Save as" file type dialog, and the type it picked (for `file.saveAsType`). */
  const [saveAsType, setSaveAsType] = useState(false);
  const saveAsExt = useRef<string | null>(null);
  /** PDF export with the choices of the PDF export dialog (CORE-FMT-002). */
  const exportPdfWith = useRef<((choices: PdfExport) => void) | null>(null);
  /** Signature line chosen in "sign a signature line", signed by the next "Ký số". */
  const pendingLine = useRef<LineChoice | null>(null);
  const signLineRef = useRef<((choice: LineChoice) => void) | null>(null);
  const [signSetup, setSignSetup] = useState<{
    defaults: SigningDefaults;
    remembered: RememberedSigning;
  } | null>(null);
  const [tab, setTab] = useState('home');
  // The File screen (CORE-UI-002).
  const [backstage, setBackstage] = useState(false);
  /** Mail merge: rule kind of the rule dialog, finish mode of the finish dialog. */
  const [ruleKind, setRuleKind] = useState<RuleKind>('if');
  const [finishMode, setFinishMode] = useState<FinishMode>('document');
  /** View to return to when read mode ends. */
  const beforeRead = useRef<EditorState['viewMode']>('print');
  /** View layout last sent to the engine (`mode:width`). */
  const sentLayout = useRef('print:0');
  const [searchOpen, setSearchOpen] = useState(false);
  // Task pane layout of BlueDoc, remembered per app (CORE-UI-006).
  const [paneLayout, setPaneLayout] = useTaskPaneLayout('doc');
  const loadStyles = useCallback(
    async () => (await controller.current?.query({ cmd: 'styleList' }))?.styles ?? null,
    [],
  );
  const queryCommand = useCallback(
    (c: EditCommand) => controller.current?.query(c) ?? Promise.resolve(null),
    [],
  );
  /** New Style / Organizer dialog asked of the Styles pane (DOC-STYLE-001). */
  const [stylesRequest, setStylesRequest] = useState<StylesPaneRequest | null>(null);
  // "Convert macro VBA to BlueScript" (CORE-SCRIPT-003): opens the converter of the script pane.
  const [vbaRequest, setVbaRequest] = useState(0);
  const vbaRequestHandled = useCallback(() => setVbaRequest(0), []);
  // Read-only macro viewer (CORE-VBA-002): a module goes to the script pane as a draft.
  const macroViewer = useMacroViewer(
    useCallback(() => {
      const s = store.getState();
      if (!s.taskPanes.includes('script')) {
        store.setState({ taskPanes: togglePane(s.taskPanes, 'script') });
      }
    }, [store]),
  );
  const showMacros = macroViewer.show;
  const loadMacros = useCallback(
    () => platform.engine.command({ cmd: 'vbaProject' }) as unknown as Promise<VbaProjectInfo>,
    [platform],
  );
  /** Print panel (CORE-PRINT-001). */
  const [printOpen, setPrintOpen] = useState(false);
  // Alt+Q only: Ctrl+Q removes paragraph formatting, as in word processors.
  useCommandSearchShortcut(() => setSearchOpen(true), { ctrl: false });
  const [recent, setRecent] = useState<RecentFile[]>([]);
  const workspace = useRef<HTMLElement>(null);
  const [pageBox, setPageBox] = useState<PageBox | null>(null);
  /** Clipboard contents read for Paste Special. */
  const pasteClip = useRef<{ text: string; html: string; internal: string } | null>(null);
  const [pasteAvail, setPasteAvail] = useState({ html: false, text: false });
  /** Clipboard history panel (Home › Clipboard launcher). */
  const [clipPanel, setClipPanel] = useState(false);
  const [equationTemplates, setEquationTemplates] = useState<{ id: string; linear: string }[]>([]);
  /** SmartArt dialog (CORE-SMART-005): insert from the layout library or edit the selected one. */
  const [smartArtDialog, setSmartArtDialog] = useState<{
    mode: 'insert' | 'edit';
    initial: SmartArtDraft;
  } | null>(null);
  /** Chart dialog (CORE-CHART-007): insert a chart or edit the selected one's data. */
  const [chartDialog, setChartDialog] = useState<{
    mode: 'insert' | 'edit';
    initial: ChartDraft;
  } | null>(null);
  const [chartTemplateOpen, setChartTemplateOpen] = useState(false);
  /** The Format chart pane (CORE-CHART-005). */
  const [chartFormatOpen, setChartFormatOpen] = useState(false);
  // A chart just selected: read its type and wrapping for the Chart Design tab's checked state.
  const chartSelected =
    !!state.edit?.format.chart &&
    state.edit.chart === undefined &&
    state.edit.selection.anchor.o !== state.edit.selection.focus.o;
  useEffect(() => {
    if (chartSelected) void controller.current?.run({ cmd: 'chartInfo' });
  }, [chartSelected]);
  const dialogOpen = state.dialog;
  useEffect(() => {
    if (dialogOpen !== 'equation' || equationTemplates.length > 0) return;
    void controller.current
      ?.run({ cmd: 'equationTemplates' })
      .then((r) => r?.templates && setEquationTemplates(r.templates));
  }, [dialogOpen, equationTemplates.length]);
  const boxFrame = useRef(0);
  // Page box of the caret's page in workspace coordinates (rulers follow it).
  const measure = useCallback(() => {
    cancelAnimationFrame(boxFrame.current);
    boxFrame.current = requestAnimationFrame(() => {
      const s = store.getState();
      const page = s.edit?.caret?.page ?? s.edit?.rects[0]?.page ?? s.page;
      const r = view.current?.pageRect(page);
      const w = workspace.current?.getBoundingClientRect();
      if (!r || !w) return setPageBox(null);
      setPageBox((old) => {
        const n = { x: r.left - w.left, y: r.top - w.top, w: r.width, h: r.height };
        return old && old.x === n.x && old.y === n.y && old.w === n.w && old.h === n.h ? old : n;
      });
    });
  }, [store]);

  const fail = useCallback(
    (key: string, e: unknown) =>
      store.setState({
        status: { kind: 'error', key, message: e instanceof Error ? e.message : String(e) },
      }),
    [store],
  );

  /** Show a freshly loaded document and fetch its editing state. */
  const loaded = useCallback(
    async (info: Awaited<ReturnType<Platform['engine']['open']>>) => {
      store.setState({
        doc: info,
        page: 0,
        status: { kind: 'idle' },
        edit: null,
        panel: null,
        signatures: null,
        macroWarning: !!info.macros,
        legacyNotice: info.legacyConverted ?? 0,
        security: null,
        protectUi: null,
      });
      view.current?.setDocument(info);
      splitView.current?.setDocument(info);
      store.setState({ proofErrors: null });
      // Focus at once: typing may start while the set-up commands below are still running.
      view.current?.focus();
      platform.window.setTitle(`${info.name} — ${t('app.doc')}`);
      // One engine call (CORE-MISC-003): new revisions and comments are attributed to the
      // signed-in user (fallback "BlueOffice"); editable regions (CORE-SEC-001) match the
      // user's name / e-mail; AutoCorrect as you type uses the user's kept options, else
      // the engine defaults; then the editing state and, the first time, the font list.
      const r = await controller.current?.run(
        openSetupCommand(
          platform.auth.user?.name ?? '',
          identityNames(platform.auth.user),
          loadAutoCorrect(),
          store.getState().fonts.length === 0,
        ),
      );
      // A final or read-only file asks how to open it.
      if (r?.security && openProtection(r.security) !== 'none' && !r.security.markedFinal)
        store.setState({ protectUi: 'prompt' });
      if (r?.fonts) store.setState({ fonts: r.fonts });
      onDocumentOpened?.(info.name, info.pageCount);
    },
    [platform, store, t, onDocumentOpened],
  );

  const openFile = useCallback(
    async (file: OpenedFile, doc?: StoredDocument, password?: string, editTemplate = false) => {
      store.setState({ status: { kind: 'busy', key: 'status.loading' } });
      try {
        let info = await platform.engine.open(file.bytes, file.name, password);
        // CORE-FMT-001: a template (.dotx / .dotm) starts a new, untitled document of the
        // matching type, unless the user asked to edit the template itself.
        const plan = planOpen(info.variant, info.name, editTemplate);
        if (plan.saveAs) {
          info = (await platform.engine.command({ cmd: 'saveFormat', ext: plan.saveAs })).info;
        }
        templateFile.current = plan.variant?.template ? file : null;
        stored.current = plan.fromTemplate ? null : (doc ?? null);
        openedBytes.current = file.bytes;
        // Another document: its blocks (keys `doc:N`) are not the previous one's.
        clearBlockPreviews();
        await loaded(info);
        if (plan.fromTemplate) {
          store.setState({
            status: { kind: 'info', key: 'fileVariants.fromTemplate', params: { name: file.name } },
          });
        }
        // A signed file: show the verification status (ADR-0007).
        if (platform.signing && looksSigned(file.bytes)) {
          void platform.signing
            .verify(file.bytes)
            .then((report) => {
              store.setState({ signatures: report });
              // Signature lines show whether their signature verifies (CORE-SIGLINE-001).
              const status = statusCommand(report);
              if (status) void controller.current?.run(status);
            })
            .catch(() => {});
        }
        if (file.path)
          await platform.shell.addRecentFile({
            name: file.name,
            path: file.path,
            openedAt: Date.now(),
          });
      } catch (e) {
        // Encrypted file ([MS-OFFCRYPTO]): ask for the password and retry.
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes(PASSWORD_REQUIRED) || msg.includes(WRONG_PASSWORD)) {
          store.setState({
            status: { kind: 'idle' },
            passwordPrompt: { file, doc: doc ?? null, wrong: msg.includes(WRONG_PASSWORD) },
          });
          return;
        }
        fail('errors.open', e);
      }
    },
    [platform, store, fail, loaded],
  );

  const newDocument = useCallback(async () => {
    try {
      await platform.engine.init();
      stored.current = null;
      openedBytes.current = null;
      templateFile.current = null;
      await loaded(await platform.engine.newDocument(t('app.newDocName')));
    } catch (e) {
      fail('errors.open', e);
    }
  }, [platform, t, fail, loaded]);

  // Keep the latest openFile without re-creating the view.
  const openFileRef = useRef(openFile);
  useEffect(() => {
    openFileRef.current = openFile;
  }, [openFile]);

  /** Embedded / linked workbooks (CORE-EMBED-001), behind the `object.*` commands. */
  const [objectHub] = useState(() => new ObjectActionsHub());
  // Typing: the Insert key switches overtype mode, in which text replaces what follows.
  const handleInput = useCallback(
    (e: ImeEvent) => {
      const c = controller.current;
      if (!c) return;
      if (e.type === 'key' && e.key === 'Insert' && !e.ctrl && !e.shift && !e.alt) {
        store.setState({ overtype: !store.getState().overtype });
        return;
      }
      if (e.type === 'insert' && store.getState().overtype) {
        void overtypeInsert((cmd) => c.run(cmd), e.text, store.getState().edit?.selection);
        return;
      }
      c.input(e);
    },
    [store],
  );
  /** The pane that has the keyboard: page number and zoom in the status bar are its. */
  const activatePane = useCallback(
    (pane: 0 | 1) => {
      if (activePane.current === pane) return;
      activePane.current = pane;
      const v = pane === 1 ? splitView.current : view.current;
      if (v) store.setState({ zoom: v.zoom });
    },
    [store],
  );
  const activeView = useCallback(
    () => (activePane.current === 1 && splitView.current) || view.current,
    [],
  );
  // Engine + view lifecycle.
  useEffect(() => {
    if (app !== 'doc' || !viewHost.current) return;
    const viewOptions: DocumentViewOptions = {
      onPageChange: (page) => {
        if (activePane.current === 0) store.setState({ page });
      },
      onZoomChange: (zoom) => {
        if (activePane.current === 0) store.setState({ zoom });
      },
      onInput: handleInput,
      onPointer: (e) => {
        controller.current?.pointer(e);
        if (e.type === 'down') setMini(null);
        // Mouse-up after selecting text: the mini toolbar near the pointer.
        if (e.type === 'up' && e.clientX !== undefined && store.getState().painter === 'off') {
          const at = { x: e.clientX, y: e.clientY ?? 0 };
          void controller.current?.query({ cmd: 'state' }).then(() => {
            const sel = store.getState().edit?.selection;
            if (sel && (sel.anchor.p !== sel.focus.p || sel.anchor.o !== sel.focus.o)) setMini(at);
          });
        }
        // Double-click an embedded workbook: its editing grid (CORE-EMBED-001).
        if (e.type === 'down' && e.count === 2) void objectHub.activate();
        // Format Painter: the formatting goes to what was clicked or selected.
        const painter = store.getState().painter;
        if (e.type === 'up' && painter !== 'off') {
          void controller.current?.run({ cmd: 'applyFormat' }).then(() => {
            if (painter === 'once') {
              store.setState({ painter: 'off' });
              void controller.current?.run({ cmd: 'cancelFormat' });
            }
          });
        }
      },
      onViewport: () => measure(),
      // A balloon in the markup area: go to its comment or change.
      onMarkupSelect: (m) => {
        if (m.kind === 'comment') {
          void controller.current?.run({ cmd: 'selectComment', id: m.id });
          return;
        }
        const r = store.getState().review?.revisions.find((x) => x.id === m.id && !x.story);
        if (r) {
          void controller.current?.run({
            cmd: 'setSelection',
            selection: { anchor: r.start, focus: r.end },
          });
        }
      },
      markupLabel: (kind, id) =>
        balloonLabel(
          (k, o) => i18nInstance.t(k, o),
          kind,
          kind === 'format'
            ? store.getState().review?.revisions.find((x) => x.id === id && !x.story)
            : undefined,
        ),
      // Right click: the context menu of what is under the pointer (spelling first).
      onContextMenu: (e) => void contextMenuAt.current(e),
      inputLabel: i18nInstance.t('a11y.documentInput'),
      pageLabel: (n) => i18nInstance.t('a11y.page', { n }),
      mirrorLabels: {
        region: (from, to) =>
          from === to
            ? i18nInstance.t('a11y.mirrorPage', { n: from })
            : i18nInstance.t('a11y.mirrorPages', { from, to }),
        picture: (alt) =>
          alt ? i18nInstance.t('a11y.picture', { alt }) : i18nInstance.t('a11y.pictureNoAlt'),
        equation: () => i18nInstance.t('a11y.equation'),
        object: () => i18nInstance.t('a11y.object'),
        deleted: (text) => i18nInstance.t('a11y.deleted', { text }),
        blank: () => i18nInstance.t('a11y.blank'),
      },
    };
    paneOptions.current = viewOptions;
    const v = new DocumentView(viewHost.current, platform.engine, {
      ...viewOptions,
      onPointer: (e) => {
        if (e.type === 'down') activatePane(0);
        viewOptions.onPointer?.(e);
      },
    });
    view.current = v;
    // Both panes of a split show every change; the caret scrolls into view in the active one.
    const panes: EditView = {
      refresh: (info, dirty) => {
        v.refresh(info, dirty);
        splitView.current?.refresh(info, dirty);
      },
      setSelection: (caret, rects, scroll = true) => {
        const second = splitView.current;
        const lower = !!second && activePane.current === 1;
        v.setSelection(caret, rects, lower ? false : scroll);
        second?.setSelection(caret, rects, lower ? scroll : false);
      },
      setMathMarks: (m) => {
        v.setMathMarks(m ?? null);
        splitView.current?.setMathMarks(m ?? null);
      },
      setAccessibleSelection: (focus, edited) => v.setAccessibleCaret(focus, edited),
    };
    // Spelling marks follow edits after a short pause (the engine caches unchanged paragraphs).
    let spellTimer: ReturnType<typeof setTimeout> | undefined;
    const refreshSpelling = () => {
      clearTimeout(spellTimer);
      spellTimer = setTimeout(() => {
        const s = store.getState();
        if (s.spellingOn || s.grammarOn)
          void controller.current?.query({ cmd: 'spelling' }).then((r) => {
            if (!r?.spelling) return;
            v.setSpelling(r.spelling);
            store.setState({ proofErrors: r.spelling.length });
          });
      }, 400);
    };
    controller.current = new DocController(
      platform.engine,
      panes,
      platform.clipboard,
      (r) => {
        measure();
        if (r.spelling) {
          v.setSpelling(r.spelling);
          store.setState({ proofErrors: r.spelling.length });
        } else if (r.dirty.length) refreshSpelling();
        // Find "highlight all": new boxes with a find, stale ones dropped after an edit.
        if (r.highlights) v.setFindHighlights(r.highlights);
        else if (r.dirty.length) v.setFindHighlights([]);
        // Protection state; highlighted editable regions follow the text.
        if (r.security) v.setRegionHighlights(r.security.regionRects ?? []);
        store.setState({
          edit: r,
          doc: r.info,
          ...(r.outline ? { outline: r.outline } : {}),
          ...(r.dirty.length ? { layoutVersion: store.getState().layoutVersion + 1 } : {}),
          ...(r.review ? { review: r.review } : {}),
          ...(r.merge ? { merge: r.merge } : {}),
          ...(r.security ? { security: r.security } : {}),
        });
        // The user's building blocks file changed: keep it through the platform.
        if (r.blocks?.store)
          void platform.offline.put(BLOCK_STORE_KEY, fromBase64(r.blocks.store)).catch(() => {});
        if (r.error === MATH_UNSUPPORTED)
          store.setState({ status: { kind: 'info', key: 'equation.inPlaceUnsupported' } });
        else if (isProtectionRefusal(r.error))
          store.setState({ status: { kind: 'info', key: refusalKey(r.error) } });
        else if (blockErrorKey(r.error))
          store.setState({ status: { kind: 'info', key: blockErrorKey(r.error) ?? '' } });
        else if (r.error)
          fail(formErrorKey(r.error) ?? reviewErrorKey(r.error) ?? 'errors.edit', r.error);
      },
      (e) => fail('errors.edit', e),
    );
    controller.current.onEquationDialog = () => equationDialog.current();
    // Ctrl+click on a hyperlink opens it (links inside the document are left to the caret).
    controller.current.onFollowLink = (target) => {
      if (!target.startsWith('#')) void platform.shell.openExternal?.(target).catch(() => {});
    };
    platform.engine.init().then(
      () => {
        store.setState({ engineReady: true });
        // User dictionary kept by this browser / app (a convenience: words re-added each start).
        for (const word of loadUserWords())
          void controller.current?.run({ cmd: 'spellAddWord', word });
        // Words ignored everywhere and the proofing options, kept the same way.
        for (const word of loadIgnoredWords())
          void controller.current?.run({ cmd: 'spellIgnore', word });
        const proof = loadProofOptions();
        if (proof) void controller.current?.run({ cmd: 'proofOptions', ...proof });
        // The user's building blocks (a .dotx kept by the platform) for the galleries and F3.
        void platform.offline
          .get(BLOCK_STORE_KEY)
          .then((bytes) =>
            bytes?.length
              ? platform.engine.command({ cmd: 'loadBlockStore', file: toBase64(bytes) })
              : null,
          )
          // No document is open yet: the engine keeps the store and answers with an error.
          .catch(() => {});
      },
      (e: unknown) => fail('errors.engine', e),
    );
    window.addEventListener('resize', measure);
    const unOpen = platform.shell.onOpenRequest((f) => void openFileRef.current(f));
    // Live collaboration: collaborators' edits and carets.
    const unExternal = platform.engine.onExternalChange?.((r) => controller.current?.external(r));
    const unPeers = platform.engine.onPeers?.((peers) => store.setState({ peers }));
    void platform.shell.recentFiles().then(setRecent);
    // Crash recovery: an autosave left behind by a previous session.
    void platform.offline.get(AUTOSAVE_NAME_KEY).then(
      (n) => n && store.setState({ recovery: dec.decode(n) }),
      () => {},
    );
    return () => {
      clearTimeout(spellTimer);
      window.removeEventListener('resize', measure);
      unOpen();
      unExternal?.();
      unPeers?.();
      v.dispose();
      view.current = null;
      controller.current = null;
    };
  }, [app, platform, store, fail, measure, i18nInstance, objectHub, handleInput, activatePane]);

  // Split view (CORE-UI-007): a second page view of the same document, below the first,
  // with its own scroll position and zoom. Edits made in either pane show in both.
  const splitOn = state.split !== null && !!state.doc;
  useEffect(() => {
    const host = splitHost.current;
    const main = view.current;
    const info = store.getState().doc;
    if (!splitOn || !host || !main || !info) return;
    const base = { ...paneOptions.current };
    delete base.onViewport;
    const second = new DocumentView(host, platform.engine, {
      ...base,
      onPageChange: (page) => {
        if (activePane.current === 1) store.setState({ page });
      },
      onZoomChange: (zoom) => {
        if (activePane.current === 1) store.setState({ zoom });
      },
      onPointer: (e) => {
        if (e.type === 'down') activatePane(1);
        base.onPointer?.(e);
      },
    });
    second.scroller.dataset.testid = 'split-scroller';
    second.setDocument(info);
    second.setPageMode(main.pageMode);
    second.setZoom(main.zoom);
    const edit = store.getState().edit;
    if (edit) second.setSelection(edit.caret, edit.rects, false);
    splitView.current = second;
    // The new pane starts at the place the first one shows.
    const frame = requestAnimationFrame(() => {
      second.scroller.scrollTop = main.scroller.scrollTop;
    });
    return () => {
      cancelAnimationFrame(frame);
      splitView.current = null;
      second.dispose();
      activatePane(0);
      store.setState({ zoom: main.zoom });
    };
  }, [splitOn, platform, store, activatePane]);
  // The panes changed size without a window resize: lay the pages out again.
  useEffect(() => {
    window.dispatchEvent(new Event('resize'));
  }, [state.split]);
  // Dictation (CORE-LANG-002): at the caret through the controller; interim text drawn by the view.
  const dictationTarget = useCallback(
    () =>
      controller.current
        ? docDictationTarget(
            platform.engine,
            (c) => controller.current?.run(c) ?? Promise.resolve(null),
            () => view.current,
          )
        : null,
    [platform.engine],
  );
  const dictation = useDictation(platform.dictation, dictationTarget);
  const dictationRef = useRef<Dictation | null>(null);
  useEffect(() => {
    dictationRef.current = dictation;
  });

  // Rulers follow the caret's page and the panels' layout.
  useEffect(() => measure(), [measure, state.showRuler, state.doc, state.panel]);

  // View layout (print / web / draft) in the engine and the page view (DOC-VIEW-001).
  const hasDocument = !!state.doc;
  useEffect(() => {
    const v = view.current;
    if (!v || !hasDocument || !state.engineReady) return;
    const mode: ViewLayoutMode =
      state.viewMode === 'web' ? 'web' : state.viewMode === 'draft' ? 'draft' : 'print';
    v.setPageMode(mode);
    splitView.current?.setPageMode(mode);
    const send = () => {
      const width = mode === 'web' ? Math.floor(v.contentWidthPt()) : 0;
      const key = `${mode}:${width}`;
      if (key === sentLayout.current) return;
      sentLayout.current = key;
      void controller.current?.run(
        mode === 'web' ? { cmd: 'setViewLayout', mode, width } : { cmd: 'setViewLayout', mode },
      );
    };
    send();
    if (mode !== 'web') return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const onResize = () => {
      clearTimeout(timer);
      timer = setTimeout(send, 150);
    };
    window.addEventListener('resize', onResize);
    return () => {
      clearTimeout(timer);
      window.removeEventListener('resize', onResize);
    };
  }, [
    state.viewMode,
    state.zoom,
    state.navPane,
    state.reviewingPane,
    state.focusMode,
    hasDocument,
    state.engineReady,
  ]);

  // The outline follows the document while the outline view or the heading tree shows it.
  const needOutline = state.viewMode === 'outline' || state.navPane === 'headings';
  useEffect(() => {
    if (!needOutline || !hasDocument) return;
    const timer = setTimeout(() => {
      void controller.current
        ?.query({ cmd: 'outline' })
        .then((r) => r?.outline && store.setState({ outline: r.outline }));
    }, 120);
    return () => clearTimeout(timer);
  }, [needOutline, hasDocument, state.layoutVersion, store]);

  /** Draw a page into a canvas (page thumbnails, read mode). */
  const renderThumb = useCallback(
    (i: number, c: HTMLCanvasElement, w: number) =>
      view.current?.renderThumbnail(i, c, w) ?? Promise.resolve(),
    [],
  );

  /** PDF of the printed pages (the web and draft views lay out other pages for the screen). */
  const printLayoutPdf = useCallback(
    async (choices?: PdfExport): Promise<Uint8Array> => {
      // PDF/A, tagged PDF and bookmarks (CORE-FMT-002) go through the export bridge.
      const pdf = () => {
        const e = platform.engine;
        if (!choices) return e.exportPdf();
        if (!e.exportSlides) throw new Error('exportSlides');
        return e.exportSlides(choices);
      };
      const mode = store.getState().viewMode;
      if (mode !== 'web' && mode !== 'draft') return pdf();
      await controller.current?.run({ cmd: 'setViewLayout', mode: 'print' });
      try {
        return await pdf();
      } finally {
        const width = Math.floor(view.current?.contentWidthPt() ?? 0);
        sentLayout.current = `${mode}:${mode === 'web' ? width : 0}`;
        await controller.current?.run(
          mode === 'web' ? { cmd: 'setViewLayout', mode, width } : { cmd: 'setViewLayout', mode },
        );
      }
    },
    [platform, store],
  );

  // Collaborators' carets (live collaboration), drawn by the view.
  useEffect(() => {
    view.current?.setPeers(
      state.showPeers
        ? state.peers.map((p) => {
            const name = p.name || t('collab.anonymous');
            return {
              ...p,
              name,
              label: t('collab.editing', { name }),
              paraLabel: t('collab.inParagraph', { name }),
            };
          })
        : [],
    );
  }, [state.peers, state.showPeers, t]);

  // Autosave of modified documents into the offline cache.
  useEffect(() => {
    if (!autosaveMs) return;
    const timer = setInterval(() => {
      const s = store.getState();
      if (!s.edit?.modified || !s.doc) return;
      void platform.engine
        .saveDocx()
        .then(async (bytes) => {
          await platform.offline.put(AUTOSAVE_KEY, bytes);
          await platform.offline.put(AUTOSAVE_NAME_KEY, enc.encode(s.doc?.name ?? ''));
          store.setState({
            status: {
              kind: 'info',
              key: 'status.autosaved',
              params: { time: new Date().toLocaleTimeString(s.language) },
            },
          });
        })
        .catch(() => {});
    }, autosaveMs);
    return () => clearInterval(timer);
  }, [autosaveMs, platform, store]);

  const restore = useCallback(async () => {
    const name = store.getState().recovery ?? 'document.docx';
    store.setState({ recovery: null });
    const bytes = await platform.offline.get(AUTOSAVE_KEY);
    if (bytes) await openFile({ name, bytes });
  }, [platform, store, openFile]);

  const discardRecovery = useCallback(async () => {
    store.setState({ recovery: null });
    await platform.offline.delete(AUTOSAVE_KEY);
    await platform.offline.delete(AUTOSAVE_NAME_KEY);
  }, [platform, store]);

  /** Actions of the picture tools tab (CORE-PIC-001), behind the `picture.*` commands. */
  const [pictureHub] = useState(() => new PictureActionsHub());
  const [shortcutsShown, setShortcutsShown] = useState(false);
  // Command callbacks only dereference refs when executed (event time), never during render.
  /* eslint-disable react-hooks/refs */
  const registry: Registry = useMemo(() => {
    const r = new CommandRegistry<EditorState>();
    const hasDoc = (s: EditorState) => !!s.doc;
    const run = async (c: EditCommand) => {
      await controller.current?.run(c);
      view.current?.focus();
    };
    const fmt = (format: CharFormat) => run({ cmd: 'formatText', format });
    const para = (format: ParaFormat) => run({ cmd: 'formatParagraph', format });
    const exportPdf = async (choices?: PdfExport) => {
      store.setState({ status: { kind: 'busy', key: 'status.exporting' } });
      try {
        const pdf = await printLayoutPdf(choices);
        const name = (store.getState().doc?.name ?? 'document').replace(/\.[^.]+$/, '') + '.pdf';
        const saved = await platform.files.saveFile(pdf, name, 'application/pdf');
        store.setState({
          status: saved
            ? { kind: 'info', key: 'status.saved', params: { name } }
            : { kind: 'idle' },
        });
      } catch (e) {
        fail('errors.export', e);
      }
    };
    const save = async (as: boolean) => {
      const s = store.getState();
      if (!s.doc) return;
      store.setState({ status: { kind: 'busy', key: 'status.saving' } });
      try {
        await controller.current?.idle();
        const bytes = await platform.engine.saveDocx();
        // Files opened from another format (.doc, .odt, .txt) are saved as a new .docx;
        // documents, templates and macro-enabled files keep their type (CORE-FMT-001).
        const name = saveName(s.doc.name, s.doc.variant, 'docx');
        const sameFormat =
          !!stored.current && stored.current.ref.name.toLowerCase() === name.toLowerCase();
        const doc =
          stored.current && !as && sameFormat
            ? await platform.storage.save(stored.current, bytes)
            : await platform.storage.saveAs(name, bytes);
        if (!doc) {
          store.setState({ status: { kind: 'idle' } });
          return;
        }
        stored.current = doc;
        await platform.engine.markSaved();
        await platform.offline.delete(AUTOSAVE_KEY);
        await platform.offline.delete(AUTOSAVE_NAME_KEY);
        await controller.current?.run({ cmd: 'state' });
        store.setState({
          status: { kind: 'info', key: 'status.saved', params: { name: doc.ref.name } },
        });
      } catch (e) {
        fail('errors.save', e);
      }
    };
    const f = (s: EditorState) => s.edit?.format;

    r.register({ id: 'file.new', shortcut: 'Ctrl+N', run: newDocument });
    r.register({
      id: 'file.open',
      shortcut: 'Ctrl+O',
      run: async () => {
        try {
          const doc = await platform.storage.open();
          if (doc) await openFile({ name: doc.ref.name, path: doc.ref.id, bytes: doc.bytes }, doc);
        } catch (e) {
          fail('errors.open', e);
        }
      },
    });
    r.register({
      id: 'file.openSample',
      isEnabled: () => !!sampleUrl,
      run: async () => {
        try {
          const res = await fetch(sampleUrl!);
          if (!res.ok) throw new Error(`HTTP ${res.status}`);
          await openFile({
            name: sampleUrl!.split('/').pop() ?? 'sample.docx',
            bytes: new Uint8Array(await res.arrayBuffer()),
          });
        } catch (e) {
          fail('errors.open', e);
        }
      },
    });
    r.register({ id: 'file.save', shortcut: 'Ctrl+S', isEnabled: hasDoc, run: () => save(false) });
    r.register({
      id: 'review.savePassword',
      isEnabled: hasDoc,
      isChecked: (s) => !!s.doc?.protected,
      run: () => store.setState({ panel: 'savePassword' }),
    });
    // CORE-SEC-001: final mark, write protection, label, restricted editing.
    r.register({
      id: 'file.protect',
      isEnabled: hasDoc,
      isChecked: (s) => !!s.security && (s.security.readOnly || !!s.security.protection?.enforced),
      run: async () => {
        await controller.current?.run({ cmd: 'security' });
        store.setState({ protectUi: 'panel' });
      },
    });
    r.register({
      id: 'review.nextEditableRegion',
      isEnabled: (s) => !!s.security?.regions?.some((g) => g.mine),
      run: () => void controller.current?.run({ cmd: 'nextEditableRegion', forward: true }),
    });
    r.register({
      id: 'review.showEditableRegions',
      isEnabled: (s) => !!s.security?.regions?.length,
      isChecked: (s) => !!s.security?.showRegions,
      run: (s) =>
        void controller.current?.run({ cmd: 'showEditableRegions', on: !s.security?.showRegions }),
    });
    // Save as: the file type first (documents, templates, macro-enabled; CORE-FMT-001).
    r.register({
      id: 'file.saveAs',
      // F12, as in word processors (Ctrl+Shift+S there opens the styles to apply).
      shortcut: 'F12',
      isEnabled: hasDoc,
      run: () => setSaveAsType(true),
    });
    r.register({
      id: 'file.saveAsType',
      isEnabled: hasDoc,
      run: async () => {
        const ext = saveAsExt.current;
        saveAsExt.current = null;
        if (!ext) return;
        await controller.current?.run({ cmd: 'saveFormat', ext });
        await save(true);
      },
    });
    // Reopen the template the document was created from, to edit the template itself.
    r.register({
      id: 'file.editTemplate',
      isEnabled: () => !!templateFile.current,
      run: async () => {
        const f = templateFile.current;
        if (f) await openFile(f, undefined, undefined, true);
      },
    });
    r.register({
      id: 'file.exportPdf',
      isEnabled: hasDoc,
      run: () => exportPdf(),
    });
    // Web page, single-file web page and Flat OPC (CORE-FMT-003): a copy in another
    // format; the open document keeps its name and type.
    for (const f of webFormats('doc')) {
      r.register({
        id: WEB_FORMAT_COMMANDS[f.id],
        isEnabled: hasDoc,
        run: async () => {
          try {
            const res = await controller.current?.run({ cmd: 'exportFile', format: f.id });
            if (!res?.file) {
              store.setState({ status: { kind: 'info', key: 'webFormats.failed' } });
              return;
            }
            const name = withExtension(store.getState().doc?.name ?? 'document', f.ext);
            const saved = await platform.files.saveFile(fromBase64(res.file), name, f.mime);
            if (saved)
              store.setState({
                status: { kind: 'info', key: 'webFormats.saved', params: { name: saved } },
              });
          } catch (e) {
            fail('errors.save', e);
          }
        },
      });
    }
    // PDF/A, tagged PDF and bookmarks from headings (CORE-FMT-002).
    exportPdfWith.current = (c: PdfExport) => void exportPdf(c);
    r.register({
      id: 'file.exportPdfOptions',
      isEnabled: (s) => hasDoc(s) && !!platform.engine.exportSlides,
      run: () => store.setState({ dialog: 'pdfExport' }),
    });
    // Digital signatures (ADR-0007): sign a copy of the document or a PDF export.
    const canSign = (s: EditorState) => !!s.doc && !!platform.signing;
    const openSign = async (panel: 'sign' | 'signPdf') => {
      const signing = platform.signing;
      if (!signing) return;
      try {
        const [defaults, remembered] = await Promise.all([
          signing.defaults(),
          platform.shell.getSetting?.(SIGNING_SETTING_KEY) ?? Promise.resolve(null),
        ]);
        setSignSetup({ defaults, remembered: parseRemembered(remembered) });
        store.setState({ panel });
      } catch (e) {
        fail('errors.sign', e);
      }
    };
    r.register({
      id: 'file.sign',
      isEnabled: canSign,
      run: () => {
        pendingLine.current = null;
        return openSign('sign');
      },
    });
    // Signature lines (CORE-SIGLINE-001): choose the line, then the signer.
    r.register({
      id: 'file.signLine',
      isEnabled: canSign,
      run: () => store.setState({ dialog: 'signLine' }),
    });
    const signLine = (choice: LineChoice) => {
      pendingLine.current = choice;
      store.setState({ dialog: null });
      void openSign('sign');
    };
    signLineRef.current = signLine;
    r.register({
      id: 'file.exportSignedPdf',
      isEnabled: canSign,
      run: () => openSign('signPdf'),
    });
    r.register({
      id: 'file.verifySignatures',
      isEnabled: (s) => canSign(s) && !!openedBytes.current,
      isChecked: (s) => s.panel === 'signatures',
      run: async () => {
        const bytes = openedBytes.current;
        if (!platform.signing || !bytes) return;
        try {
          const report = await platform.signing.verify(bytes);
          store.setState({ signatures: report, panel: 'signatures' });
          const status = statusCommand(report);
          if (status) await controller.current?.run(status);
        } catch (e) {
          fail('errors.verify', e);
        }
      },
    });
    const printNow = async (s: EditorState) => {
      try {
        await platform.print.printPdf(await printLayoutPdf(), s.doc?.name ?? '');
      } catch (e) {
        fail('errors.export', e);
      }
    };
    // The print panel (pages, pages per sheet, copies, markup, preview) when the engine
    // has the print pipeline; otherwise the whole document goes to the platform's print.
    r.register({
      id: 'file.print',
      shortcut: 'Ctrl+P',
      isEnabled: hasDoc,
      run: async (s) => {
        if (platform.engine.printPdf && platform.engine.printPreview) setPrintOpen(true);
        else await printNow(s);
      },
    });
    r.register({ id: 'file.printNow', isEnabled: hasDoc, run: printNow });
    const updates = platform.updates;
    if (updates) {
      // Desktop auto-update: the result shows in the status line.
      r.register({
        id: 'file.checkUpdates',
        run: async () => {
          store.setState({ status: { kind: 'info', key: 'updates.checking' } });
          const n = updateNotice(await updates.check());
          store.setState({ status: { kind: 'info', key: n.key, params: n.params } });
        },
      });
    }

    // Editing.
    r.register({
      id: 'edit.undo',
      shortcut: 'Ctrl+Z',
      isEnabled: (s) => !!f(s)?.canUndo,
      run: () => run({ cmd: 'undo' }),
    });
    r.register({
      id: 'edit.redo',
      shortcut: 'Ctrl+Y',
      isEnabled: (s) => !!f(s)?.canRedo,
      run: () => run({ cmd: 'redo' }),
    });
    r.register({
      id: 'edit.selectAll',
      shortcut: 'Ctrl+A',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'selectAll' }),
    });
    // F4: the last formatting / insertion again at the selection (CORE-UI-004).
    r.register({
      id: 'edit.repeat',
      shortcut: 'F4',
      isEnabled: hasDoc,
      run: async () => {
        await controller.current?.repeat();
        view.current?.focus();
      },
    });
    r.register({
      id: 'options.shortcuts',
      run: () => setShortcutsShown(true),
    });
    r.register({
      id: 'edit.copy',
      shortcut: 'Ctrl+C',
      isEnabled: hasDoc,
      run: async () => {
        await controller.current?.copy(false);
        view.current?.focus();
      },
    });
    r.register({
      id: 'edit.cut',
      shortcut: 'Ctrl+X',
      isEnabled: hasDoc,
      run: async () => {
        await controller.current?.copy(true);
        view.current?.focus();
      },
    });
    // Ctrl+V is left to the browser so the `paste` event delivers HTML (ImeInput).
    r.register({
      id: 'edit.paste',
      isEnabled: hasDoc,
      run: async () => {
        await controller.current?.pasteFromClipboard();
        view.current?.focus();
      },
    });
    r.register({
      id: 'edit.find',
      shortcut: 'Ctrl+F',
      isEnabled: hasDoc,
      run: () => store.setState({ panel: 'find' }),
    });
    r.register({
      id: 'edit.replace',
      shortcut: 'Ctrl+H',
      isEnabled: hasDoc,
      run: () => store.setState({ panel: 'replace' }),
    });

    // Character formatting.
    const toggles: [string, CharFormat['kind'], string, (s: EditorState) => boolean][] = [
      ['format.bold', 'bold', 'Ctrl+B', (s) => !!f(s)?.bold],
      ['format.italic', 'italic', 'Ctrl+I', (s) => !!f(s)?.italic],
      ['format.underline', 'underline', 'Ctrl+U', (s) => !!f(s)?.underline],
      ['format.strike', 'strike', '', (s) => !!f(s)?.strike],
      ['format.subscript', 'subscript', 'Ctrl+=', (s) => !!f(s)?.subscript],
      ['format.superscript', 'superscript', 'Ctrl+Shift+=', (s) => !!f(s)?.superscript],
    ];
    for (const [id, kind, shortcut, checked] of toggles) {
      r.register({
        id,
        shortcut: shortcut || undefined,
        isEnabled: hasDoc,
        isChecked: checked,
        run: () => fmt({ kind } as CharFormat),
      });
    }
    const size = (s: EditorState) => f(s)?.sizeHalfPoints ?? 26;
    r.register({
      id: 'format.grow',
      shortcut: 'Ctrl+]',
      isEnabled: hasDoc,
      run: (s) => fmt({ kind: 'size', halfPoints: size(s) + 2 }),
    });
    r.register({
      id: 'format.shrink',
      shortcut: 'Ctrl+[',
      isEnabled: hasDoc,
      run: (s) => fmt({ kind: 'size', halfPoints: Math.max(2, size(s) - 2) }),
    });
    r.register({
      id: 'format.color',
      isEnabled: hasDoc,
      isChecked: (s) => !!f(s)?.color && f(s)?.color !== '000000',
      run: (s) => fmt({ kind: 'color', hex: f(s)?.color === 'C00000' ? null : 'C00000' }),
    });
    r.register({
      id: 'format.highlight',
      isEnabled: hasDoc,
      isChecked: (s) => !!f(s)?.highlight,
      run: (s) => fmt({ kind: 'highlight', hex: f(s)?.highlight ? null : 'FFFF00' }),
    });
    r.register({
      id: 'format.clear',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'clearFormatting' }),
    });
    // Character effects / spacing, phonetic guide (§17.3.3.25 ruby) and two lines in
    // one (§17.3.2.10 combine) — DOC-FMT-001.
    for (const [id, dialog, shortcut] of [
      // Ctrl+D: the font dialog, as in word processors.
      ['format.fontDialog', 'fontEffects', 'Ctrl+D'],
      ['format.phoneticGuide', 'phoneticGuide', undefined],
      ['format.combineChars', 'combineChars', undefined],
    ] as const) {
      r.register({ id, shortcut, isEnabled: hasDoc, run: () => store.setState({ dialog }) });
    }
    // Format Painter: click = once, double click = until Esc (or a third click).
    r.register({
      id: 'format.painter',
      isEnabled: hasDoc,
      isChecked: (s) => s.painter !== 'off',
      run: async (s) => {
        if (s.painter === 'once' && painterDoubleClick()) {
          store.setState({ painter: 'sticky' });
          return;
        }
        if (s.painter !== 'off') {
          store.setState({ painter: 'off' });
          await run({ cmd: 'cancelFormat' });
          return;
        }
        painterDoubleClick();
        await run({ cmd: 'pickFormat' });
        store.setState({ painter: 'once' });
      },
    });
    r.register({
      id: 'format.copyFormat',
      shortcut: 'Ctrl+Shift+C',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'pickFormat' }),
    });
    r.register({
      id: 'format.pasteFormat',
      shortcut: 'Ctrl+Shift+V',
      isEnabled: (s) => !!s.edit?.painter,
      run: () => run({ cmd: 'applyFormat' }),
    });
    // Esc: stop the Format Painter, else leave the header/footer, else leave focus mode.
    r.register({
      id: 'edit.escape',
      shortcut: 'Escape',
      isEnabled: (s) => s.painter !== 'off' || !!s.edit?.math || !!s.edit?.story || s.focusMode,
      run: async (s) => {
        // In an equation edited in place: leave it, the caret after it.
        if (s.edit?.math) await run({ cmd: 'mathMove', motion: 'exit' });
        else if (s.painter !== 'off') {
          store.setState({ painter: 'off' });
          await run({ cmd: 'cancelFormat' });
        } else if (s.edit?.story) await run({ cmd: 'closeHeaderFooter' });
        else store.setState({ focusMode: false });
      },
    });
    r.register({
      id: 'edit.pasteSpecial',
      shortcut: 'Ctrl+Alt+V',
      isEnabled: hasDoc,
      run: async () => {
        const clip = platform.clipboard;
        const text = await clip.readText().catch(() => '');
        const html = (await clip.readHtml?.().catch(() => '')) ?? '';
        const last = controller.current?.lastClip;
        const own = last && (!text || last.text === text) ? last : null;
        pasteClip.current = {
          text: text || own?.text || '',
          html: html || own?.html || '',
          internal: own?.internal ?? '',
        };
        setPasteAvail({
          html: !!pasteClip.current.html || !!pasteClip.current.internal,
          text: !!pasteClip.current.text,
        });
        store.setState({ dialog: 'pasteSpecial' });
      },
    });

    // Paragraph formatting.
    const aligns: [string, 'Left' | 'Center' | 'Right' | 'Justify', string][] = [
      ['format.alignLeft', 'Left', 'Ctrl+L'],
      ['format.alignCenter', 'Center', 'Ctrl+E'],
      ['format.alignRight', 'Right', 'Ctrl+R'],
      ['format.justify', 'Justify', 'Ctrl+J'],
    ];
    for (const [id, align, shortcut] of aligns) {
      r.register({
        id,
        shortcut,
        isEnabled: hasDoc,
        isChecked: (s) => f(s)?.align === align,
        run: () => para({ kind: 'align', align }),
      });
    }
    r.register({
      id: 'format.bullets',
      isEnabled: hasDoc,
      isChecked: (s) => !!f(s)?.bullets,
      run: () => para({ kind: 'bullets' }),
    });
    r.register({
      id: 'format.numbering',
      isEnabled: hasDoc,
      isChecked: (s) => !!f(s)?.numbering,
      run: () => para({ kind: 'numbering' }),
    });
    // Multilevel lists (ECMA-376 Part 1, §17.9): define, link to headings, set value.
    r.register({
      id: 'format.defineList',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'defineList' }),
    });
    r.register({
      id: 'format.setNumberingValue',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'setNumbering' }),
    });
    r.register({
      id: 'format.restartNumbering',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'setListNumbering', numbering: { kind: 'restart', value: 1 } }),
    });
    r.register({
      id: 'format.continueNumbering',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'setListNumbering', numbering: { kind: 'continue' } }),
    });
    r.register({
      id: 'format.indent',
      isEnabled: hasDoc,
      run: () => para({ kind: 'increaseIndent' }),
    });
    r.register({
      id: 'format.outdent',
      isEnabled: hasDoc,
      run: () => para({ kind: 'decreaseIndent' }),
    });
    for (const [id, style, shortcut] of [
      ['style.normal', 'Normal', 'Ctrl+Shift+N'],
      ['style.title', 'Title', ''],
      ['style.heading1', 'Heading1', 'Ctrl+Alt+1'],
      ['style.heading2', 'Heading2', 'Ctrl+Alt+2'],
      ['style.heading3', 'Heading3', 'Ctrl+Alt+3'],
    ] as const) {
      r.register({
        id,
        shortcut: shortcut || undefined,
        isEnabled: hasDoc,
        isChecked: (s) => (f(s)?.style ?? 'Normal') === style,
        run: () => run({ cmd: 'applyStyle', id: style }),
      });
    }
    // Everyday shortcuts of word processors (docs/parity/office-behaviour-doc.md).
    // Ctrl+Shift+> / < : the next / previous size of the font size list.
    r.register({
      id: 'format.growStep',
      shortcut: 'Ctrl+Shift+>',
      isEnabled: hasDoc,
      run: (s) => fmt({ kind: 'size', halfPoints: nextFontSize(size(s) / 2, 1) * 2 }),
    });
    r.register({
      id: 'format.shrinkStep',
      shortcut: 'Ctrl+Shift+<',
      isEnabled: hasDoc,
      run: (s) => fmt({ kind: 'size', halfPoints: nextFontSize(size(s) / 2, -1) * 2 }),
    });
    // Line spacing (§17.3.1.33 spacing @w:line, 240ths of a line): single, double, 1.5.
    for (const [id, value, shortcut] of [
      ['format.lineSpacing1', 240, 'Ctrl+1'],
      ['format.lineSpacing2', 480, 'Ctrl+2'],
      ['format.lineSpacing15', 360, 'Ctrl+5'],
    ] as const) {
      r.register({
        id,
        shortcut,
        isEnabled: hasDoc,
        isChecked: (s) => {
          const ls = f(s)?.lineSpacing;
          return ls?.rule === 'auto' && ls.value === value;
        },
        run: () => para({ kind: 'lineSpacing', value }),
      });
    }
    // Ctrl+0: 12 pt before the paragraph, or none when it already has it.
    r.register({
      id: 'format.spaceBefore12',
      shortcut: 'Ctrl+0',
      isEnabled: hasDoc,
      run: (s) => para({ kind: 'spaceBefore', twips: (f(s)?.spaceBefore ?? 0) === 240 ? 0 : 240 }),
    });
    r.register({
      id: 'format.indentKey',
      shortcut: 'Ctrl+M',
      isEnabled: hasDoc,
      run: () => para({ kind: 'increaseIndent' }),
    });
    r.register({
      id: 'format.outdentKey',
      shortcut: 'Ctrl+Shift+M',
      isEnabled: hasDoc,
      run: () => para({ kind: 'decreaseIndent' }),
    });
    // Ctrl+T / Ctrl+Shift+T: hanging indent by half an inch (the first line stays).
    for (const [id, shortcut, step] of [
      ['format.hanging', 'Ctrl+T', 720],
      ['format.unhang', 'Ctrl+Shift+T', -720],
    ] as const) {
      r.register({
        id,
        shortcut,
        isEnabled: hasDoc,
        run: (s) => {
          const cur = f(s);
          const left = Math.max(0, (cur?.indentLeft ?? 0) + step);
          const first = (cur?.indentLeft ?? 0) + (cur?.firstLine ?? 0) - left;
          return para({ kind: 'indents', left, right: cur?.indentRight ?? 0, firstLine: first });
        },
      });
    }
    r.register({
      id: 'format.bulletsKey',
      shortcut: 'Ctrl+Shift+L',
      isEnabled: hasDoc,
      run: () => para({ kind: 'bullets' }),
    });
    r.register({
      id: 'format.clearChar',
      shortcut: 'Ctrl+Space',
      isEnabled: hasDoc,
      run: () => fmt({ kind: 'clear' }),
    });
    r.register({
      id: 'format.resetPara',
      shortcut: 'Ctrl+Q',
      isEnabled: hasDoc,
      run: () => para({ kind: 'reset' }),
    });
    r.register({
      id: 'format.changeCase',
      shortcut: 'Shift+F3',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'changeCase', mode: 'cycle' }),
    });
    for (const mode of ['sentence', 'lower', 'upper', 'title', 'toggle'] as const) {
      r.register({
        id: `format.case.${mode}`,
        isEnabled: hasDoc,
        run: () => run({ cmd: 'changeCase', mode }),
      });
    }
    r.register({
      id: 'insert.nbsp',
      shortcut: 'Ctrl+Shift+Space',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'insertText', text: ' ' }),
    });
    // Alt+Shift+D / T / P: date, time and page number fields (§17.16.5).
    for (const [id, instr, shortcut] of [
      ['insert.dateField', 'DATE', 'Alt+Shift+D'],
      ['insert.timeField', 'TIME', 'Alt+Shift+T'],
      ['insert.pageField', 'PAGE', 'Alt+Shift+P'],
    ] as const) {
      r.register({
        id,
        shortcut,
        isEnabled: hasDoc,
        run: () => run({ cmd: 'insertField', instr }),
      });
    }
    // Ctrl+K: insert or edit a hyperlink (§17.16.22 hyperlink).
    r.register({
      id: 'insert.link',
      shortcut: 'Ctrl+K',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'link' }),
    });
    r.register({
      id: 'insert.openLink',
      isEnabled: (s) => !!f(s)?.link,
      run: (s) => {
        const link = f(s)?.link;
        if (link && !link.startsWith('#')) void platform.shell.openExternal?.(link).catch(() => {});
      },
    });
    r.register({
      id: 'insert.removeLink',
      isEnabled: (s) => !!f(s)?.link,
      run: () => run({ cmd: 'setLink', target: '' }),
    });

    // Insert.
    r.register({
      id: 'insert.table',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'insertTable', rows: 3, cols: 3 }),
    });
    r.register({
      id: 'insert.image',
      isEnabled: hasDoc,
      run: async () => {
        try {
          const file = await platform.files.openFile(['.png', '.jpg', '.jpeg']);
          if (file) await controller.current?.insertImage(file.bytes, file.name);
          view.current?.focus();
        } catch (e) {
          fail('errors.image', e);
        }
      },
    });
    // Design: page borders and watermarks (dialogs), watermark removal.
    r.register({
      id: 'design.pageBorders',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'pageBorders' }),
    });
    r.register({
      id: 'design.customWatermark',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'watermark' }),
    });
    r.register({
      id: 'design.removeWatermark',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'setWatermark' }),
    });
    // Theme files (CORE-FMT-001): apply the theme of a .thmx (or another file), save ours.
    r.register({
      id: 'design.themeFile',
      isEnabled: hasDoc,
      run: async () => {
        const f = await platform.files.openFile([
          '.thmx',
          ...openExtensions('doc'),
          '.pptx',
          '.xlsx',
        ]);
        if (!f) return;
        const res = await controller.current?.run({
          cmd: 'applyThemeFile',
          file: bytesToBase64(f.bytes),
        });
        store.setState({
          status: res?.error
            ? { kind: 'info', key: 'fileVariants.noTheme' }
            : { kind: 'info', key: 'fileVariants.themeApplied', params: { name: f.name } },
        });
      },
    });
    r.register({
      id: 'design.saveThemeFile',
      isEnabled: hasDoc,
      run: async () => {
        const res = await controller.current?.run({ cmd: 'saveThemeFile' });
        const b64 = res?.design?.themeFile;
        if (!b64) {
          store.setState({ status: { kind: 'info', key: 'fileVariants.noTheme' } });
          return;
        }
        const name = saveName(store.getState().doc?.name ?? 'theme', 'thmx', 'thmx');
        const saved = await platform.files.saveFile(fromBase64(b64), name, THEME_MIME);
        if (saved)
          store.setState({
            status: { kind: 'info', key: 'fileVariants.themeSaved', params: { name } },
          });
      },
    });
    r.register({
      id: 'insert.pageBreak',
      shortcut: 'Ctrl+Enter',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'insertPageBreak' }),
    });
    r.register({
      id: 'insert.sectionBreak',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'insertSectionBreak', kind: 'nextPage' }),
    });
    r.register({
      id: 'insert.symbol',
      isEnabled: hasDoc,
      run: () => store.setState({ panel: 'symbols' }),
    });
    r.register({
      id: 'insert.icon',
      isEnabled: hasDoc,
      run: () => store.setState({ panel: 'icons' }),
    });
    r.register({
      id: 'insert.field',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'field' }),
    });
    r.register({
      id: 'insert.control',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'control' }),
    });
    // Custom XML parts and content control data binding (§15.2.4, §17.5.2.6).
    r.register({
      id: 'insert.xmlMapping',
      isEnabled: hasDoc,
      isChecked: (s) => s.panel === 'xmlMapping',
      run: (s) => store.setState({ panel: s.panel === 'xmlMapping' ? null : 'xmlMapping' }),
    });
    r.register({
      id: 'insert.signatureLine',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'signatureLine' }),
    });
    // Legacy form fields (§17.16.17) and the "filling in forms" restriction (§17.15.1.29).
    const formsLocked = (s: EditorState) =>
      !!s.edit?.protection?.enforced && s.edit.protection.edit === 'forms';
    const canDesign = (s: EditorState) => hasDoc(s) && !formsLocked(s);
    for (const [id, kind] of [
      ['insert.formText', 'text'],
      ['insert.formCheckBox', 'checkBox'],
      ['insert.formDropDown', 'dropDown'],
    ] as const) {
      r.register({ id, isEnabled: canDesign, run: () => run({ cmd: 'insertFormField', kind }) });
    }
    r.register({
      id: 'insert.formProperties',
      isEnabled: (s) => canDesign(s) && !!s.edit?.formField,
      run: () => store.setState({ dialog: 'formField' }),
    });
    r.register({
      id: 'review.protectForms',
      isEnabled: hasDoc,
      isChecked: formsLocked,
      run: () => store.setState({ dialog: 'protectForms' }),
    });
    // SmartArt (CORE-SMART-005): the BlueSlide layout library, inline at the selection.
    r.register({
      id: 'insert.smartart',
      isEnabled: hasDoc,
      run: () =>
        setSmartArtDialog({
          mode: 'insert',
          initial: {
            layout: 'process',
            items: [1, 2, 3].map((n) => ({
              text: i18nInstance.t('slideEditor.smartart.sampleItem', { n }),
              level: 0,
            })),
            colors: 'colorful',
            style: 'fill',
          },
        }),
    });
    r.register({
      id: 'smartart.edit',
      isEnabled: (s) => hasDoc(s) && !!s.edit?.format.smartArt,
      run: async () => {
        const r = await controller.current?.run({ cmd: 'smartArtInfo' });
        const a = r?.smartArt;
        if (a) {
          setSmartArtDialog({
            mode: 'edit',
            initial: { layout: a.layout, items: a.items, colors: a.colors, style: a.style },
          });
        }
      },
    });
    // Convert to shapes (CORE-SMART-004): the diagram becomes a group of ordinary shapes.
    r.register({
      id: 'smartart.toShapes',
      isEnabled: (s) => hasDoc(s) && !!s.edit?.format.smartArt,
      run: async () => {
        const res = await controller.current?.run({ cmd: 'smartArtToShapes' });
        if (res?.error) fail('errors.smartArt', res.error);
        view.current?.focus();
      },
    });
    // Charts (CORE-CHART-007): BlueSlide's chart types, dialog and design, in the document.
    registerDocChartCommands(r, {
      t: i18nInstance.t.bind(i18nInstance),
      run: async (c) => (await controller.current?.run(c)) ?? null,
      open: (mode, initial) => setChartDialog({ mode, initial }),
      pane: { toggle: () => setChartFormatOpen((v) => !v) },
    });
    r.register({
      id: 'insert.equation',
      shortcut: 'Alt+=',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'equation' }),
    });
    r.register({
      id: 'equation.linear',
      isEnabled: (s) => hasDoc(s) && !!s.edit?.equation && !s.edit.equation.isLinear,
      run: () => run({ cmd: 'equationFormat', linear: true }),
    });
    r.register({
      id: 'equation.professional',
      isEnabled: (s) => hasDoc(s) && !!s.edit?.equation?.isLinear,
      run: () => run({ cmd: 'equationFormat', linear: false }),
    });
    // Put the caret into the equation to edit it in place (CORE-MATH-010).
    r.register({
      id: 'equation.editInPlace',
      isEnabled: (s) => hasDoc(s) && !!s.edit?.equation && !s.edit.math,
      run: () => run({ cmd: 'mathEnter', end: true }),
    });

    // View.
    r.register({
      id: 'view.zoomIn',
      // Ctrl+Shift+= is superscript, Ctrl+0 space before, as in word processors.
      shortcut: 'Ctrl+Alt+=',
      isEnabled: hasDoc,
      run: (s) => activeView()?.setZoom(s.zoom * 1.1),
    });
    r.register({
      id: 'view.zoomOut',
      shortcut: 'Ctrl+Alt+-',
      isEnabled: hasDoc,
      run: (s) => activeView()?.setZoom(s.zoom / 1.1),
    });
    r.register({
      id: 'view.zoomReset',
      shortcut: 'Ctrl+Alt+0',
      isEnabled: hasDoc,
      isChecked: (s) => s.zoom === 1,
      run: () => activeView()?.setZoom(1),
    });
    r.register({ id: 'view.fitWidth', isEnabled: hasDoc, run: () => activeView()?.fitWidth() });
    // Window (CORE-UI-007): second window, split panes, side by side.
    const paged = (s: EditorState) =>
      hasDoc(s) && s.viewMode !== 'outline' && s.viewMode !== 'read';
    r.register({ id: 'window.new', run: () => windowsRef.current?.newWindow() });
    r.register({ id: 'window.arrangeAll', run: () => windowsRef.current?.arrangeAll() });
    r.register({
      id: 'window.split',
      shortcut: 'Ctrl+Alt+S',
      isEnabled: paged,
      isChecked: (s) => s.split !== null,
      run: (s) => {
        store.setState({ split: s.split === null ? SPLIT_DEFAULT : null });
        view.current?.focus();
      },
    });
    r.register({
      id: 'window.sideBySide',
      isChecked: (s) => s.sideBySide,
      run: () => windowsRef.current?.sideBySide(),
    });
    r.register({
      id: 'window.syncScroll',
      isEnabled: (s) => s.sideBySide,
      isChecked: (s) => s.sideBySide && s.syncScroll,
      run: () => windowsRef.current?.toggleSynchronous(),
    });
    r.register({
      id: 'window.resetPosition',
      isEnabled: (s) => s.sideBySide,
      run: () => windowsRef.current?.resetPosition(),
    });
    r.register({
      id: 'review.wordCount',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'wordCount' }),
    });
    r.register({
      id: 'view.collaborators',
      isChecked: (s) => s.showPeers,
      run: (s) => store.setState({ showPeers: !s.showPeers }),
    });
    r.register({
      id: 'view.toggleTheme',
      isChecked: (s) => ['dark', 'hc'].includes(getThemePref() ?? s.theme),
      // Light → dark → high contrast → light (UI-TOKENS-002).
      // From the theme shown now (`auto` is whatever the system gave).
      run: () => setThemePref(nextTheme(resolvedThemeRef.current)),
    });
    r.register({
      id: 'view.density',
      // Normal → Compact → Touch (UI-TOKENS-003), remembered per user.
      run: () => setDensity(nextDensity(getDensity())),
    });
    r.register({
      id: 'view.toggleLanguage',
      run: async (s) => {
        const next: Language = s.language === 'vi-VN' ? 'en-US' : 'vi-VN';
        await i18nInstance.changeLanguage(next);
        store.setState({ language: next });
      },
    });
    // Mailings: mail merge from CSV / XLSX (ECMA-376 Part 1, §17.16.5.35 MERGEFIELD).
    const mg = (s: EditorState) => s.merge;
    const hasData = (s: EditorState) => hasDoc(s) && (mg(s)?.count ?? 0) > 0;
    const toBase64 = (bytes: Uint8Array) => {
      let bin = '';
      for (let i = 0; i < bytes.length; i += 0x8000) {
        bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      }
      return btoa(bin);
    };
    r.register({
      id: 'mailings.selectRecipients',
      isEnabled: hasDoc,
      run: async () => {
        try {
          const file = await platform.files.openFile(['.csv', '.xlsx', '.vcf', '.json']);
          if (!file) return;
          const res = await controller.current?.run({
            cmd: 'mergeData',
            name: file.name,
            base64: toBase64(file.bytes),
          });
          if (res?.merge)
            store.setState({
              panel: 'merge',
              status: {
                kind: 'info',
                key: 'merge.source',
                params: { name: res.merge.name, count: String(res.merge.count) },
              },
            });
        } catch (e) {
          fail('errors.open', e);
        }
      },
    });
    r.register({
      id: 'mailings.insertField',
      isEnabled: hasDoc,
      isChecked: (s) => s.panel === 'merge',
      run: (s) => store.setState({ panel: s.panel === 'merge' ? null : 'merge' }),
    });
    r.register({
      id: 'mailings.preview',
      isEnabled: hasData,
      isChecked: (s) => mg(s)?.preview != null,
      run: (s) => run({ cmd: 'mergePreview', record: mg(s)?.preview != null ? null : 0 }),
    });
    r.register({
      id: 'mailings.nextRecord',
      isEnabled: hasData,
      run: (s) => run({ cmd: 'mergePreview', record: (mg(s)?.preview ?? -1) + 1 }),
    });
    r.register({
      id: 'mailings.previousRecord',
      isEnabled: hasData,
      run: (s) => run({ cmd: 'mergePreview', record: Math.max(0, (mg(s)?.preview ?? 1) - 1) }),
    });
    const mergeToDocument = async (): Promise<boolean> => {
      const res = await controller.current?.run({ cmd: 'mergeToDocument' });
      if (!res || res.error) return false;
      stored.current = null;
      await loaded(res.info);
      store.setState({
        status: {
          kind: 'info',
          key: 'merge.done',
          params: { count: String(store.getState().merge?.count ?? 0), name: res.info.name },
        },
      });
      return true;
    };
    r.register({
      id: 'mailings.mergeDocument',
      isEnabled: hasData,
      run: async () => {
        await mergeToDocument();
      },
    });
    r.register({
      id: 'mailings.mergePdf',
      isEnabled: hasData,
      run: async () => {
        if (await mergeToDocument()) await exportPdf();
      },
    });
    // Extended mail merge (DOC-MERGE-001): dialogs in MergeDialogs.tsx.
    r.register({
      id: 'mailings.fromServer',
      isEnabled: (s) => hasDoc(s) && !!platform.mergeData,
      run: () => store.setState({ dialog: 'mergeProvider' }),
    });
    r.register({
      id: 'mailings.editRecipients',
      isEnabled: hasData,
      run: () => store.setState({ dialog: 'mergeRecipients' }),
    });
    for (const kind of RULE_KINDS) {
      r.register({
        id: `mailings.rule.${kind}`,
        isEnabled: hasDoc,
        run: () => {
          setRuleKind(kind);
          store.setState({ dialog: 'mergeRule' });
        },
      });
    }
    r.register({
      id: 'mailings.envelopes',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'mergeEnvelope' }),
    });
    r.register({
      id: 'mailings.labels',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'mergeLabels' }),
    });
    const finish = (mode: FinishMode) => () => {
      setFinishMode(mode);
      store.setState({ dialog: 'mergeFinish' });
    };
    r.register({ id: 'mailings.finishDocument', isEnabled: hasData, run: finish('document') });
    r.register({ id: 'mailings.finishPdfs', isEnabled: hasData, run: finish('pdfs') });
    r.register({
      id: 'mailings.finishEmail',
      isEnabled: (s) => hasData(s) && !!platform.mail,
      run: finish('email'),
    });
    // References: TOC, captions, cross-references (ECMA-376 Part 1, §17.16).
    const updateFields = async () => {
      const res = await controller.current?.run({ cmd: 'updateFields' });
      if (res && !res.error)
        store.setState({
          status: {
            kind: 'info',
            key: 'references.fieldsUpdated',
            params: { count: String(res.count ?? 0) },
          },
        });
      view.current?.focus();
    };
    r.register({
      id: 'references.insertToc',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'insertToc' }),
    });
    r.register({ id: 'references.updateToc', isEnabled: hasDoc, run: updateFields });
    r.register({
      id: 'references.updateFields',
      shortcut: 'F9',
      isEnabled: hasDoc,
      run: updateFields,
    });
    r.register({
      id: 'fields.toggleCodes',
      shortcut: 'Alt+F9',
      isEnabled: hasDoc,
      isChecked: (s) => s.fieldCodes,
      run: async () => {
        const on = !store.getState().fieldCodes;
        const res = await controller.current?.run({ cmd: 'showFieldCodes', on });
        if (res && !res.error) store.setState({ fieldCodes: on });
      },
    });
    r.register({
      id: 'fields.lock',
      shortcut: 'Ctrl+F11',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'lockFields', lock: true }),
    });
    r.register({
      id: 'fields.unlock',
      shortcut: 'Ctrl+Shift+F11',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'lockFields', lock: false }),
    });
    r.register({
      id: 'fields.unlink',
      shortcut: 'Ctrl+Shift+F9',
      isEnabled: hasDoc,
      run: () => run({ cmd: 'unlinkFields' }),
    });
    r.register({
      id: 'references.insertCaption',
      isEnabled: hasDoc,
      run: () => store.setState({ panel: 'caption' }),
    });
    r.register({
      id: 'references.crossReference',
      isEnabled: hasDoc,
      run: () => store.setState({ panel: 'crossref' }),
    });
    // Tables of figures, indexes, tables of authorities, note conversion (ReferenceDialogs).
    for (const [id, dialog] of [
      ['references.tableOfFigures', 'tableOfFigures'],
      ['references.markEntry', 'markEntry'],
      ['references.insertIndex', 'insertIndex'],
      ['references.markCitation', 'markCitation'],
      ['references.insertToa', 'insertToa'],
      ['references.convertNotes', 'convertNotes'],
      // Citations and bibliography (CitationDialogs, CORE-CITE-001).
      ['references.insertCitation', 'insertCitation'],
      ['references.manageSources', 'manageSources'],
      ['references.insertBibliography', 'insertBibliography'],
    ] as const) {
      r.register({
        id,
        isEnabled: (s) => hasDoc(s) && !s.edit?.story,
        run: () => store.setState({ dialog }),
      });
    }
    // Review: track changes (ECMA-376 Part 1, §17.13.5).
    const rv = (s: EditorState) => s.review;
    r.register({
      id: 'review.convertLegacyText',
      isEnabled: hasDoc,
      run: async () => {
        const r = await controller.current?.run({ cmd: 'convertLegacyText' });
        store.setState({ legacyNotice: r?.info?.legacyConverted ?? 0 });
        view.current?.focus();
      },
    });
    r.register({
      id: 'review.spelling',
      isEnabled: hasDoc,
      isChecked: (s) => s.spellingOn,
      run: async (s) => {
        store.setState({ spellingOn: !s.spellingOn });
        await controller.current?.run({ cmd: 'setSpelling', on: !s.spellingOn });
      },
    });
    r.register({
      id: 'review.autoCorrect',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'autoCorrect' }),
    });
    // Whole-document spelling and grammar check; proofing language of the selection.
    r.register({
      id: 'review.spellingDialog',
      shortcut: 'F7',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'spellCheck' }),
    });
    r.register({
      id: 'review.language',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'language' }),
    });
    r.register({
      id: 'review.checkDocument',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'checkDocument' }),
    });
    r.register({
      id: 'review.versions',
      isEnabled: hasDoc,
      isChecked: (s) => s.taskPanes.includes('versions'),
      run: (s) => store.setState({ taskPanes: togglePane(s.taskPanes, 'versions') }),
    });
    r.register({
      id: 'review.checkAdminFormat',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'adminFormat' }),
    });
    // Nghị định 30/2020: correct every fixable deviation (one undoable edit).
    r.register({
      id: 'review.fixAdminFormat',
      isEnabled: hasDoc,
      run: async () => {
        await controller.current?.run({ cmd: 'fixAdminFormat' });
      },
    });
    r.register({
      id: 'review.grammar',
      isEnabled: hasDoc,
      isChecked: (s) => s.grammarOn,
      run: async (s) => {
        store.setState({ grammarOn: !s.grammarOn });
        await controller.current?.run({ cmd: 'setGrammar', on: !s.grammarOn });
      },
    });
    registerDictationCommands(r, {
      enabled: hasDoc,
      current: () => dictationRef.current,
    });
    registerReadAloudCommands(r, {
      enabled: hasDoc,
      begin: async () => {
        const req = await docReadRequest(
          platform.engine,
          () => view.current,
          i18nInstance.language || 'vi-VN',
        );
        await readAloudRef.current?.start(req);
      },
      current: () => readAloudRef.current,
    });
    // Translation (CORE-LANG-001): the selection in a pane; the whole document into a copy.
    const translateRun = async (c: EditCommand) => (await controller.current?.run(c)) ?? null;
    const documentAction: TranslateAction = {
      id: 'document',
      label: i18nInstance.t('translate.actions.document'),
      run: async ({ from, to, signal, onProgress }) => {
        const provider = platform.translation;
        if (!provider) return;
        const segments = await translateDocumentSegments(translateRun, provider, from, to, {
          signal,
          onProgress,
        });
        if (!segments.length) return i18nInstance.t('translate.done.nothing');
        // The copy: the current state saved, opened as a new untitled document, then rewritten.
        const bytes = await platform.engine.saveDocx();
        const name = translatedName(store.getState().doc?.name ?? 'document.docx', to);
        await openFile({ bytes, name });
        await controller.current?.run({ cmd: 'translationApply', segments });
        return i18nInstance.t('translate.done.document', { name, count: segments.length });
      },
    };
    registerTranslateCommands(r, {
      enabled: hasDoc,
      actions: { document: 'review.translateDocument' },
      begin: async (_s, action) => {
        const text = await docSelectionText(translateRun);
        translateLink.open({
          source: {
            text,
            replace: async (tr) => {
              await run({ cmd: 'insertText', text: tr });
            },
            insert: async (tr) => {
              await docInsertAfter(translateRun, tr);
              view.current?.focus();
            },
          },
          actions: [{ ...documentAction, label: i18nInstance.t('translate.actions.document') }],
          autoRun: action,
        });
      },
    });
    // Thesaurus (CORE-THES-001): the pane on the word at the caret; a replacement is one
    // undoable `insertText` over the word, in its capitalisation (chosen by the pane).
    r.register({
      id: 'review.thesaurus',
      shortcut: 'Shift+F7',
      isEnabled: hasDoc,
      run: async () => {
        const w = await docWordAtCaret(translateRun);
        thesaurusRef.current?.open({
          target: {
            word: w?.word ?? '',
            replace: async (text, word) => {
              const ok = await docReplaceWord(translateRun, word, text);
              view.current?.focus();
              return ok;
            },
            insert: async (text) => {
              await run({ cmd: 'insertText', text });
            },
          },
        });
      },
    });
    // BlueAI (CORE-AI-001): the pane on a feature; results are applied from its preview only.
    registerAiCommands(r, {
      enabled: hasDoc,
      features: DOC_AI_COMMANDS,
      isOff: () => aiLink.off,
      begin: (_s, feature) =>
        aiLink.open({
          features: docAiFeatures({
            t: (key, options) => i18nInstance.t(key, options ?? {}),
            run: translateRun,
            focus: () => view.current?.focus(),
          }),
          feature,
        }),
    });
    registerTrackingCommands(r, {
      hasDoc,
      run,
      openLockDialog: () => store.setState({ dialog: 'lockTracking' }),
    });
    r.register({
      id: 'review.trackChanges',
      shortcut: 'Ctrl+Shift+E',
      isEnabled: hasDoc,
      isChecked: (s) => !!rv(s)?.trackChanges,
      run: (s) => run({ cmd: 'setTrackChanges', on: !rv(s)?.trackChanges }),
    });
    const hasRevisions = (s: EditorState) => hasDoc(s) && (rv(s)?.revisions.length ?? 0) > 0;
    r.register({
      id: 'review.accept',
      isEnabled: hasRevisions,
      run: () => run({ cmd: 'acceptRevision' }),
    });
    r.register({
      id: 'review.reject',
      isEnabled: hasRevisions,
      run: () => run({ cmd: 'rejectRevision' }),
    });
    r.register({
      id: 'review.acceptAll',
      isEnabled: hasRevisions,
      run: () => run({ cmd: 'acceptAllRevisions' }),
    });
    r.register({
      id: 'review.rejectAll',
      isEnabled: hasRevisions,
      run: () => run({ cmd: 'rejectAllRevisions' }),
    });
    r.register({
      id: 'review.compare',
      isEnabled: hasDoc,
      run: async () => {
        try {
          const file = await platform.files.openFile(['.docx', '.odt', '.doc']);
          if (!file) return;
          let bin = '';
          for (let i = 0; i < file.bytes.length; i += 0x8000) {
            bin += String.fromCharCode(...file.bytes.subarray(i, i + 0x8000));
          }
          await run({ cmd: 'compareWith', name: file.name, base64: btoa(bin) });
        } catch (e) {
          console.error(e);
        }
      },
    });
    r.register({
      id: 'review.combine',
      isEnabled: hasDoc,
      run: async () => {
        try {
          const file = await platform.files.openFile(['.docx', '.odt', '.doc']);
          if (!file) return;
          let bin = '';
          for (let i = 0; i < file.bytes.length; i += 0x8000) {
            bin += String.fromCharCode(...file.bytes.subarray(i, i + 0x8000));
          }
          await run({ cmd: 'combineWith', name: file.name, base64: btoa(bin) });
        } catch (e) {
          console.error(e);
        }
      },
    });
    r.register({
      id: 'review.next',
      isEnabled: hasRevisions,
      run: () => run({ cmd: 'nextRevision', forward: true }),
    });
    r.register({
      id: 'review.previous',
      isEnabled: hasRevisions,
      run: () => run({ cmd: 'nextRevision', forward: false }),
    });
    for (const [id, view] of [
      ['review.viewMarkup', 'markup'],
      ['review.viewFinal', 'final'],
      ['review.viewOriginal', 'original'],
    ] as const) {
      r.register({
        id,
        isEnabled: hasDoc,
        isChecked: (s) => (rv(s)?.view ?? 'markup') === view,
        run: () => run({ cmd: 'setRevisionView', view }),
      });
    }
    // Review display modes and markup options (view settings, never the document).
    const mk = (s: EditorState) => rv(s)?.markup;
    for (const [id, mode] of [
      ['review.markup.simple', 'simple'],
      ['review.markup.all', 'all'],
      ['review.markup.none', 'none'],
      ['review.markup.original', 'original'],
    ] as const) {
      r.register({
        id,
        isEnabled: hasDoc,
        isChecked: (s) => (mk(s)?.mode ?? 'all') === mode,
        run: () => run({ cmd: 'setMarkup', mode }),
      });
    }
    for (const [id, key] of [
      ['review.markup.insDel', 'insertionsDeletions'],
      ['review.markup.formatting', 'formatting'],
      ['review.markup.comments', 'comments'],
      ['review.markup.balloons', 'deletionBalloons'],
    ] as const) {
      const on = (s: EditorState) => mk(s)?.[key] ?? key !== 'deletionBalloons';
      r.register({
        id,
        isEnabled: hasDoc,
        isChecked: on,
        run: (s) => run({ cmd: 'setMarkup', [key]: !on(s) }),
      });
    }
    r.register({
      id: 'review.markup.allReviewers',
      isEnabled: hasDoc,
      isChecked: (s) => (mk(s)?.hiddenReviewers.length ?? 0) === 0,
      run: () => run({ cmd: 'setMarkup', hiddenReviewers: [] }),
    });
    r.register({
      id: 'review.reviewingPane',
      isEnabled: hasDoc,
      isChecked: (s) => s.reviewingPane !== null,
      run: (s) => store.setState({ reviewingPane: s.reviewingPane ? null : 'left' }),
    });
    r.register({
      id: 'review.reviewingPaneBottom',
      isEnabled: hasDoc,
      isChecked: (s) => s.reviewingPane === 'bottom',
      run: (s) => store.setState({ reviewingPane: s.reviewingPane === 'bottom' ? null : 'bottom' }),
    });
    r.register({
      id: 'review.pane',
      isEnabled: hasDoc,
      isChecked: (s) => s.panel === 'review',
      run: (s) => store.setState({ panel: s.panel === 'review' ? null : 'review' }),
    });
    // Comments (ECMA-376 Part 1, §17.13.4) with replies.
    const hasComments = (s: EditorState) => hasDoc(s) && (rv(s)?.comments.length ?? 0) > 0;
    // The comment whose range holds the caret (else the first one).
    const current = (s: EditorState) => {
      const list = rv(s)?.comments ?? [];
      const at = s.edit?.selection.focus;
      const le = (a: { p: number; o: number }, b: { p: number; o: number }) =>
        a.p < b.p || (a.p === b.p && a.o <= b.o);
      return (
        list.find(
          (c) => c.parent === null && at && c.start && c.end && le(c.start, at) && le(at, c.end),
        ) ?? list.find((c) => c.parent === null)
      );
    };
    r.register({
      id: 'review.newComment',
      shortcut: 'Ctrl+Alt+M',
      isEnabled: hasDoc,
      run: (s) => store.setState({ taskPanes: togglePane(s.taskPanes, 'comments', true) }),
    });
    r.register({
      id: 'review.comments',
      isEnabled: hasDoc,
      isChecked: (s) => s.taskPanes.includes('comments'),
      run: (s) => store.setState({ taskPanes: togglePane(s.taskPanes, 'comments') }),
    });
    r.register({
      id: 'review.deleteComment',
      isEnabled: hasComments,
      run: async (s) => {
        const c = current(s);
        if (c) await run({ cmd: 'deleteComment', id: c.id });
      },
    });
    r.register({
      id: 'review.resolveComment',
      isEnabled: hasComments,
      run: async (s) => {
        const c = current(s);
        if (c) await run({ cmd: 'resolveComment', id: c.id, done: !c.done });
      },
    });
    r.register({
      id: 'review.nextComment',
      isEnabled: hasComments,
      run: () => run({ cmd: 'nextComment', forward: true }),
    });
    r.register({
      id: 'review.previousComment',
      isEnabled: hasComments,
      run: () => run({ cmd: 'nextComment', forward: false }),
    });
    // Page setup (ECMA-376 Part 1, §17.6.13 pgSz, §17.6.11 pgMar, §17.6.4 cols).
    for (const id of ['layout.margins', 'layout.size', 'layout.pageSetup']) {
      r.register({
        id,
        isEnabled: (s) => hasDoc(s) && !!f(s)?.section,
        run: () => store.setState({ dialog: 'pageSetup' }),
      });
    }
    // Columns (§17.6.4), line numbers (§17.6.8), hyphenation (§17.15.1.10), page
    // text direction (§17.6.20) and large initial letters (§17.3.1.11 framePr).
    for (const [id, dialog] of [
      ['layout.columns', 'columns'],
      ['layout.lineNumbers', 'lineNumbers'],
      ['layout.hyphenation', 'hyphenation'],
      ['layout.textDirection', 'textDirection'],
      ['insert.dropCap', 'dropCap'],
    ] as const) {
      r.register({
        id,
        isEnabled: (s) => hasDoc(s) && !!f(s)?.section,
        run: () => store.setState({ dialog }),
      });
    }
    r.register({
      id: 'layout.orientation',
      isEnabled: (s) => hasDoc(s) && !!f(s)?.section,
      isChecked: (s) => {
        const sec = f(s)?.section;
        return !!sec && sec.width > sec.height;
      },
      run: async (s) => {
        const sec = f(s)?.section;
        if (!sec) return;
        // Swap the page size; margins turn with the page.
        await run({
          cmd: 'setPageSetup',
          scope: 'section',
          setup: {
            width: sec.height,
            height: sec.width,
            marginTop: sec.marginLeft,
            marginBottom: sec.marginRight,
            marginLeft: sec.marginBottom,
            marginRight: sec.marginTop,
            header: sec.header,
            footer: sec.footer,
            columns: sec.columns,
            columnSpace: sec.columnSpace,
          },
        });
      },
    });
    r.register({
      id: 'view.ruler',
      isChecked: (s) => s.showRuler,
      run: (s) => store.setState({ showRuler: !s.showRuler }),
    });
    // Views (DOC-VIEW-001): layouts, outline, read mode, focus, navigation pane, marks.
    const viewIds = {
      print: 'view.printLayout',
      web: 'view.webLayout',
      draft: 'view.draft',
      outline: 'view.outline',
      read: 'view.readMode',
    } as const;
    for (const mode of ['print', 'web', 'draft', 'outline', 'read'] as const) {
      r.register({
        id: viewIds[mode],
        isEnabled: hasDoc,
        isChecked: (s) => s.viewMode === mode,
        run: (s) => {
          if (mode === 'read' && s.viewMode !== 'read') beforeRead.current = s.viewMode;
          store.setState({ viewMode: mode, ...(mode === 'read' ? { focusMode: false } : {}) });
          if (mode !== 'outline' && mode !== 'read') view.current?.focus();
        },
      });
    }
    // Focused reader (CORE-LANG-004): the document's text, reflowed for reading.
    r.register({
      id: 'view.focusedReader',
      isEnabled: hasDoc,
      isChecked: (s) => s.focusedReader !== null,
      run: async (s) => {
        if (s.focusedReader) {
          store.setState({ focusedReader: null });
          view.current?.focus();
          return;
        }
        readAloudRef.current?.stop();
        const blocks = await docReaderBlocks(platform.engine, i18nInstance.language || 'vi-VN');
        store.setState({
          focusedReader: { blocks, start: readerStart(blocks, s.edit?.selection.focus.p) },
        });
      },
    });
    r.register({
      id: 'view.focus',
      isEnabled: hasDoc,
      isChecked: (s) => s.focusMode,
      run: (s) => {
        store.setState({ focusMode: !s.focusMode });
        view.current?.focus();
      },
    });
    r.register({
      id: 'view.navigationPane',
      isEnabled: hasDoc,
      isChecked: (s) => s.navPane !== null,
      run: (s) => store.setState({ navPane: s.navPane ? null : 'headings' }),
    });
    r.register({
      id: 'view.stylesPane',
      shortcut: 'Ctrl+Alt+Shift+S',
      isEnabled: hasDoc,
      isChecked: (s) => s.taskPanes.includes('styles'),
      run: (s) => store.setState({ taskPanes: togglePane(s.taskPanes, 'styles') }),
    });
    // Style management (DOC-STYLE-001).
    r.register({
      id: 'view.styleInspector',
      isEnabled: hasDoc,
      isChecked: (s) => s.taskPanes.includes('styleInspector'),
      run: (s) => store.setState({ taskPanes: togglePane(s.taskPanes, 'styleInspector') }),
    });
    r.register({
      id: 'view.revealFormatting',
      shortcut: 'Shift+F1',
      isEnabled: hasDoc,
      isChecked: (s) => s.taskPanes.includes('revealFormatting'),
      run: (s) => store.setState({ taskPanes: togglePane(s.taskPanes, 'revealFormatting') }),
    });
    for (const [id, kind] of [
      ['view.newStyle', 'newStyle'],
      ['view.styleOrganizer', 'organizer'],
    ] as const) {
      r.register({
        id,
        isEnabled: hasDoc,
        run: (s) => {
          store.setState({ taskPanes: togglePane(s.taskPanes, 'styles', true) });
          setStylesRequest((q) => ({ kind, n: (q?.n ?? 0) + 1 }));
        },
      });
    }
    // BlueScript pane (CORE-SCRIPT-001).
    r.register({
      id: 'script.editor',
      isEnabled: hasDoc,
      isChecked: (s) => s.taskPanes.includes('script'),
      run: (s) => store.setState({ taskPanes: togglePane(s.taskPanes, 'script') }),
    });
    // Macro VBA → BlueScript draft (CORE-SCRIPT-003): VBA is only read, never run.
    r.register({
      id: 'script.convertVba',
      isEnabled: hasDoc,
      run: (s) => {
        if (!s.taskPanes.includes('script')) {
          store.setState({ taskPanes: togglePane(s.taskPanes, 'script') });
        }
        setVbaRequest((n) => n + 1);
      },
    });
    // Macro source of the open file, read-only (CORE-VBA-002); VBA is never run.
    r.register({ id: 'script.viewMacros', isEnabled: hasDoc, run: () => showMacros() });
    // Macro recorder (CORE-SCRIPT-002): Stop opens the recorded script in the pane.
    r.register({
      id: 'script.record',
      isEnabled: hasDoc,
      isChecked: () => recorder.recording,
      run: async (s) => {
        if (!recorder.recording) {
          await recorder.start();
          view.current?.focus();
          return;
        }
        await recorder.stop();
        if (!s.taskPanes.includes('script')) {
          store.setState({ taskPanes: togglePane(s.taskPanes, 'script') });
        }
      },
    });
    r.register({
      id: 'view.formattingMarks',
      shortcut: 'Ctrl+Shift+8',
      isEnabled: hasDoc,
      isChecked: (s) => s.showMarks,
      run: async (s) => {
        store.setState({ showMarks: !s.showMarks });
        await run({ cmd: 'showFormattingMarks', on: !s.showMarks });
      },
    });
    // Outline: promote / demote / move a heading with its subordinate text (caret paragraph).
    const outlineNow = async () => {
      const s = store.getState();
      const live = s.viewMode === 'outline' || s.navPane === 'headings';
      return (
        (live ? s.outline : null) ??
        (await controller.current?.query({ cmd: 'outline' }))?.outline ??
        []
      );
    };
    const runOutline = async (c: EditCommand) => {
      await controller.current?.run(c);
      if (store.getState().viewMode !== 'outline') view.current?.focus();
    };
    const levelCommand = (id: string, shortcut: string | undefined, pick: typeof promoted) =>
      r.register({
        id,
        shortcut,
        isEnabled: (s) => hasDoc(s) && !s.edit?.story,
        run: async (s) => {
          const para = s.edit?.selection.focus.p;
          if (para === undefined) return;
          const level = pick(await outlineNow(), para);
          if (level !== undefined) await runOutline({ cmd: 'setOutlineLevel', para, level });
        },
      });
    levelCommand('outline.promote', 'Alt+Shift+ArrowLeft', promoted);
    levelCommand('outline.demote', 'Alt+Shift+ArrowRight', demoted);
    levelCommand('outline.bodyText', undefined, () => null);
    for (const [id, shortcut, up] of [
      ['outline.moveUp', 'Alt+Shift+ArrowUp', true],
      ['outline.moveDown', 'Alt+Shift+ArrowDown', false],
    ] as const) {
      r.register({
        id,
        shortcut,
        isEnabled: (s) => hasDoc(s) && !s.edit?.story,
        run: () => runOutline({ cmd: 'moveOutline', up }),
      });
    }
    r.register({
      id: 'file.newFromTemplate',
      run: () => store.setState({ dialog: 'templates' }),
    });
    // Headers and footers (§17.10), page numbers (§17.16.5.44 PAGE).
    const caretPage = (s: EditorState) => s.edit?.caret?.page ?? s.page;
    for (const [id, header] of [
      ['insert.header', true],
      ['insert.footer', false],
    ] as const) {
      r.register({
        id,
        isEnabled: hasDoc,
        isChecked: (s) => s.edit?.story?.kind === (header ? 'header' : 'footer'),
        run: (s) => run({ cmd: 'editHeaderFooter', page: caretPage(s), header }),
      });
    }
    // "Link to previous" (§17.10.5) of the header/footer being edited.
    r.register({
      id: 'hf.linkToPrevious',
      isEnabled: (s) => !!s.edit?.story?.canLink,
      isChecked: (s) => !!s.edit?.story?.linked,
      run: (s) => run({ cmd: 'setHeaderLink', linked: !s.edit?.story?.linked }),
    });
    // Leave the header/footer being edited (contextual tab, CORE-UI-001).
    r.register({
      id: 'hf.close',
      isEnabled: (s) => s.edit?.story?.kind === 'header' || s.edit?.story?.kind === 'footer',
      run: () => run({ cmd: 'closeHeaderFooter' }),
    });
    r.register({
      id: 'insert.pageNumber',
      isEnabled: hasDoc,
      run: async (s) => {
        if (!s.edit?.story)
          await run({ cmd: 'editHeaderFooter', page: caretPage(s), header: false });
        await run({ cmd: 'formatParagraph', format: { kind: 'align', align: 'Center' } });
        await run({ cmd: 'insertField', instr: 'PAGE' });
      },
    });
    // Building blocks (§17.12 glossary document): save the selection, Organizer, F3.
    r.register({
      id: 'insert.saveBlock',
      shortcut: 'Alt+F3',
      isEnabled: (s) => {
        const sel = s.edit?.selection;
        return (
          hasDoc(s) &&
          !s.edit?.story &&
          !!sel &&
          (sel.anchor.p !== sel.focus.p || sel.anchor.o !== sel.focus.o)
        );
      },
      run: () => store.setState({ dialog: 'saveBlock' }),
    });
    r.register({
      id: 'insert.blocksOrganizer',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'blocksOrganizer' }),
    });
    r.register({
      id: 'insert.autoText',
      shortcut: 'F3',
      isEnabled: (s) => hasDoc(s) && !s.edit?.story,
      run: () => run({ cmd: 'expandAutoText' }),
    });
    // Footnotes and endnotes (§17.11).
    r.register({
      id: 'insert.footnote',
      shortcut: 'Ctrl+Alt+F',
      isEnabled: (s) => hasDoc(s) && !s.edit?.story,
      // The caret goes into the new footnote at the page bottom (edited in place).
      run: () => run({ cmd: 'insertNote', endnote: false, text: '' }),
    });
    r.register({
      id: 'insert.endnote',
      shortcut: 'Ctrl+Alt+D',
      isEnabled: (s) => hasDoc(s) && !s.edit?.story,
      run: () => run({ cmd: 'insertNote', endnote: true, text: '' }),
    });
    r.register({
      id: 'references.notes',
      isEnabled: hasDoc,
      isChecked: (s) => s.panel === 'notes',
      run: (s) => store.setState({ panel: s.panel === 'notes' ? null : 'notes' }),
    });
    // Tables (§17.4).
    const inTable = (s: EditorState) => hasDoc(s) && !!f(s)?.table;
    const tbl = (edit: TableEdit) => run({ cmd: 'tableEdit', edit });
    const tableCmds: [string, TableEdit, ((s: EditorState) => boolean)?][] = [
      ['table.insertRowAbove', { kind: 'insertRow', below: false }],
      ['table.insertRowBelow', { kind: 'insertRow', below: true }],
      ['table.insertColumnLeft', { kind: 'insertColumn', right: false }],
      ['table.insertColumnRight', { kind: 'insertColumn', right: true }],
      ['table.deleteRow', { kind: 'deleteRow' }],
      ['table.deleteColumn', { kind: 'deleteColumn' }],
      // Cells shifted right / left (tracked as §17.13.5.4 cellIns / §17.13.5.3 cellDel).
      ['table.insertCell', { kind: 'insertCell', right: false }],
      ['table.deleteCell', { kind: 'deleteCell' }],
      ['table.deleteTable', { kind: 'deleteTable' }],
      ['table.mergeCells', { kind: 'mergeCells' }, (s) => inTable(s) && !!f(s)?.table?.multiCell],
      ['table.splitCell', { kind: 'splitCell' }],
      ['table.bordersAll', { kind: 'borders', preset: 'all' }],
      ['table.bordersNone', { kind: 'borders', preset: 'none' }],
      ['table.bordersOutside', { kind: 'borders', preset: 'outside' }],
      ['table.bordersInside', { kind: 'borders', preset: 'inside' }],
    ];
    for (const [id, edit, enabled] of tableCmds) {
      r.register({ id, isEnabled: enabled ?? inTable, run: () => tbl(edit) });
    }
    r.register({
      id: 'table.headerRow',
      isEnabled: inTable,
      isChecked: (s) => !!f(s)?.table?.headerRow,
      run: (s) => tbl({ kind: 'headerRow', on: !f(s)?.table?.headerRow }),
    });
    r.register({
      id: 'table.properties',
      isEnabled: inTable,
      run: () => store.setState({ dialog: 'tableProperties' }),
    });
    // Table tools: sizes (§17.4.53 tblLayout, §17.4.81 trHeight), sort, formula
    // (§17.16.3), text direction (§17.4.72), wrapping (§17.4.58), text ↔ table.
    const toolCmds: [string, TableEdit][] = [
      ['table.distributeRows', { kind: 'distributeRows' }],
      ['table.distributeColumns', { kind: 'distributeColumns' }],
      ['table.autoFitContents', { kind: 'autoFit', mode: 'contents' }],
      ['table.autoFitWindow', { kind: 'autoFit', mode: 'window' }],
      ['table.autoFitFixed', { kind: 'autoFit', mode: 'fixed' }],
    ];
    for (const [id, edit] of toolCmds) {
      r.register({ id, isEnabled: inTable, run: () => tbl(edit) });
    }
    const dialogCmds: [string, EditorState['dialog'], (s: EditorState) => boolean][] = [
      ['table.sort', 'tableSort', inTable],
      ['table.formula', 'tableFormula', inTable],
      ['table.toText', 'tableToText', inTable],
      ['table.fromText', 'textToTable', (s) => hasDoc(s) && !s.edit?.story],
    ];
    for (const [id, dialog, enabled] of dialogCmds) {
      r.register({ id, isEnabled: enabled, run: () => store.setState({ dialog }) });
    }
    r.register({
      id: 'table.textDirection',
      isEnabled: inTable,
      isChecked: (s) => !!f(s)?.table?.direction,
      // Horizontal → top to bottom → bottom to top → horizontal.
      run: (s) => {
        const d = f(s)?.table?.direction ?? null;
        const next = d === null || d === 'lrTb' ? 'tbRl' : d === 'tbRl' ? 'btLr' : null;
        return tbl({ kind: 'textDirection', dir: next });
      },
    });
    r.register({
      id: 'table.wrap',
      isEnabled: inTable,
      isChecked: (s) => !!f(s)?.table?.wrap,
      run: (s) => tbl({ kind: 'wrap', around: !f(s)?.table?.wrap }),
    });
    // Floating picture position (§20.4.2.3 anchor); a tracked move with track changes on.
    r.register({
      id: 'picture.position',
      isEnabled: (s) => hasDoc(s) && !!f(s)?.picture,
      run: () => store.setState({ dialog: 'picturePosition' }),
    });
    // Picture tools (CORE-PIC-001): buttons of the "Picture tools" tab.
    registerPictureCommands(
      r,
      (s) => hasDoc(s) && !!f(s)?.pictureTools,
      () => pictureHub.get(),
    );
    // Embedded / linked workbooks (CORE-EMBED-001).
    registerObjectCommands(r, hasDoc, () => objectHub.get());
    return r;
  }, [
    showMacros,
    platform,
    store,
    activeView,
    sampleUrl,
    openFile,
    newDocument,
    fail,
    i18nInstance,
    loaded,
    printLayoutPdf,
    pictureHub,
    objectHub,
    recorder,
    translateLink,
    aiLink,
  ]);
  /* eslint-enable react-hooks/refs */
  useEffect(() => {
    equationDialog.current = () => void registry.execute('insert.equation', store.getState());
  }, [registry, store]);

  // Keyboard shortcuts (registry); plain typing and caret keys go through the IME textarea.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.isComposing) return;
      const target = e.target as HTMLElement | null;
      // Inputs of dialogs keep their own keys (except global file shortcuts).
      // (Text boxes of panes too, e.g. the script editor's Ctrl+Space; not the document's input.)
      const inForm =
        !!target &&
        (target.tagName === 'INPUT' ||
          target.isContentEditable ||
          (target.tagName === 'TEXTAREA' && target.dataset.testid !== 'ime-input'));
      if (isContextMenuKey(e) && viewHost.current?.contains(target)) {
        e.preventDefault();
        keyMenuAt.current = Date.now();
        menuAtCaret.current();
        return;
      }
      const c = registry.match(e);
      if (!c || (inForm && !c.id.startsWith('file.') && c.id !== 'edit.find')) return;
      e.preventDefault();
      void registry.execute(c.id, store.getState());
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [registry, store]);

  // The user's keyboard shortcuts (CORE-UI-004), applied over the registry's defaults.
  const shortcuts = useCustomShortcuts({
    app: 'doc',
    registry,
    store: platform.shell,
    files: platform.files,
    commands: () =>
      shortcutCommands(ribbonTabsRef.current, registry, (id) => commandLabel(i18nInstance, id)),
    open: shortcutsShown,
    onOpenChange: setShortcutsShown,
  });
  // The user's ribbon and Quick Access Toolbar (CORE-UI-005).
  const ribbonCustom = useRibbonCustomization({
    app: 'doc',
    registry,
    store: platform.shell,
    files: platform.files,
    tabs: () => ribbonTabsRef.current,
    extraCommands: () => shortcutCommands([], registry, (id) => commandLabel(i18nInstance, id)),
    shortcuts,
    enabled: (id) => registry.isEnabled(id, store.getState()),
    run: (id) => void registry.execute(id, store.getState()),
  });

  const runCommand = useCallback(
    (c: EditCommand) => controller.current?.run(c) ?? Promise.resolve(null),
    [],
  );
  // The dialog of diagrams: layout thumbnails from the engine, node pictures from a file.
  const loadSmartArtPreviews = useCallback<SmartArtPreviewLoader>(
    // A query: its answer is not an edit result to apply to the view.
    async (colors, style) =>
      (await controller.current?.query({ cmd: 'smartArtPreviews', colors, style }))?.previews ?? [],
    [],
  );
  const pickSmartArtPicture = useMemo(() => smartArtPicturePicker(platform.files), [platform]);
  const scriptHost = useCallback(
    () =>
      store.getState().doc
        ? docScriptHost({ run: runCommand, onEnd: () => view.current?.focus() })
        : null,
    [store, runCommand],
  );
  // Add-ins of the organisation catalogue (CORE-ADDIN-001): ribbon commands, task panes.
  // Their calls go through the BlueScript API (ADR-0012), without moving the focus.
  const addins = useAddins({
    app: 'doc',
    catalog: platform.addins,
    registry,
    host: () => (store.getState().doc ? docScriptHost({ run: runCommand }) : null),
    selection: docSelection,
    selectionKey: JSON.stringify(state.edit?.selection ?? null),
    documentKey: state.layoutVersion,
  });
  // Saved scripts on a shortcut / the Quick Access Toolbar (CORE-SCRIPT-002).
  const scriptBindings = useScriptBindings({
    app: 'doc',
    registry,
    host: scriptHost,
    recorder,
    label: (id) => t(`commands.${id}`),
    onResult: (sc, r) => {
      if (!r.ok) fail('script.boundFailed', new Error(`${sc.name}: ${r.error?.message ?? ''}`));
    },
  });
  const docProperties = useMemo<BackstagePropertiesAccess>(
    () => ({
      load: async () => (await runCommand({ cmd: 'docProperties' }))?.docProperties ?? null,
      save: async (props) =>
        (await runCommand({ cmd: 'setDocProperties', props }))?.docProperties ?? null,
    }),
    [runCommand],
  );
  const backstageCommands = useMemo(
    () => ({
      has: (id: string) => registry.has(id),
      enabled: (id: string) => registry.isEnabled(id, store.getState()),
      run: (id: string) => void registry.execute(id, store.getState()),
    }),
    [registry, store],
  );
  const closeBackstage = useCallback(() => setBackstage(false), [setBackstage]);
  const item = (
    id: string,
    labelKey: string,
    icon: ReactNode,
    size: 'large' | 'small' = 'large',
  ) => {
    const cmd = registry.get(id);
    const enabled = registry.isEnabled(id, state);
    return {
      id,
      label: t(labelKey),
      icon,
      size,
      enabled,
      checked: cmd?.isChecked ? registry.isChecked(id, state) : undefined,
      shortcut: cmd?.shortcut,
      tooltip: enabled || !id.startsWith('layout.') ? t(labelKey) : t('ribbon.comingLater'),
      onSelect: () => void registry.execute(id, store.getState()),
    };
  };
  const s = (id: string, key: string, icon: ReactNode) => item(id, key, icon, 'small');
  /** A custom ribbon control (combobox, colour picker). */
  const custom = (id: string, render: ReactNode) => ({ id, label: id, onSelect: () => {}, render });
  /** A registry command shown in a dropdown menu (mail merge Rules ▾ / Finish ▾). */
  const menuEntry = (id: string, key: string) => ({
    id,
    label: t(key),
    enabled: registry.isEnabled(id, state),
    onSelect: () => void registry.execute(id, store.getState()),
  });
  /** A checkable registry command shown in a review markup menu. */
  const markupEntry = (id: string, key: string) => ({
    id,
    label: t(key),
    checked: registry.isChecked(id, state),
    enabled: registry.isEnabled(id, state),
    onSelect: () => void registry.execute(id, store.getState()),
  });
  const fs = state.edit?.format;
  // Picture tools tab (CORE-PIC-001) for the picture at the selection.
  const picture = usePictureTools({
    labels: pictureTabLabels(t),
    enabled: !!state.doc && !!fs?.pictureTools,
    selectionKey: JSON.stringify(state.edit?.selection ?? null),
    run: (c) => controller.current?.run(c) ?? Promise.resolve(null),
    exec: (id) => void registry.execute(id, store.getState()),
    pickFile: () => platform.files.openFile(['.png', '.jpg', '.jpeg', '.gif', '.bmp']),
    onError: (code) => fail(`pictureTools.error.${code}`, code),
    hub: pictureHub,
  });
  const pictureTab: RibbonTab[] = picture.tab ? [picture.tab] : [];
  // Embedded / linked workbooks (CORE-EMBED-001): insert dialog, editing grid, update links.
  const objects = useObjectTools({
    labels: objectToolsLabels(t),
    run: (c) => controller.current?.run(c) ?? Promise.resolve(null),
    ...objectPlatform(platform),
    onError: (code) =>
      store.setState({
        status: { kind: 'error', key: `objectTools.error.${code}`, message: code },
      }),
    onUpdated: (count) =>
      store.setState({
        status: { kind: 'info', key: 'objectTools.updated', params: { count: String(count) } },
      }),
    hub: objectHub,
  });
  // SmartArt tab (CORE-SMART-005) for the diagram at the selection.
  const smartArtTab: RibbonTab[] = fs?.smartArt
    ? [
        {
          id: 'smartart',
          context: 'smartArt',
          label: t('slideEditor.smartart.tab'),
          groups: [
            {
              id: 'smartartEdit',
              label: t('slideEditor.smartart.group'),
              items: [
                item('smartart.edit', 'slideEditor.smartart.textPane', <Pencil size={ICON} />),
                item('smartart.toShapes', 'slideEditor.smartart.toShapes', <Shapes size={ICON} />),
              ],
            },
          ],
        },
      ]
    : [];
  // Status bar: proofing language of the selection (§17.3.2.20 lang).
  const languageLabel = fs?.lang
    ? (PROOF_LANGUAGES as readonly string[]).includes(fs.lang)
      ? t(`spell.lang.${fs.lang}`)
      : fs.lang
    : t('spell.lang.none');
  /** Engine queries of dialogs (they change neither the document nor the selection). */
  const queryDoc = useCallback(
    (c: EditCommand) => controller.current?.query(c) ?? Promise.resolve(null),
    [],
  );
  // View › Window (CORE-UI-007): the other windows, side by side, New Window.
  const windowTools = useWindowTools({
    platform,
    app: 'doc',
    title: state.doc?.name ?? '',
    scroller: () => view.current?.scroller ?? null,
    getFile: async () => {
      const d = store.getState().doc;
      if (!d) return null;
      return {
        bytes: await platform.engine.saveDocx(),
        name: `${d.name.replace(/\.[^.]+$/, '')}.docx`,
      };
    },
    notify: (text) =>
      store.setState({ status: { kind: 'info', key: 'windows.message', params: { text } } }),
    t,
  });
  useEffect(() => {
    windowsRef.current = windowTools;
  });
  const sideBySide = !!windowTools.paired;
  useEffect(() => {
    store.setState({ sideBySide, syncScroll: windowTools.synchronous });
  }, [store, sideBySide, windowTools.synchronous]);

  // Status bar (CORE-UI-007): the user's items; words of the selection; Caps Lock.
  const statusBar = useStatusBarPrefs(platform.shell, 'doc');
  const [selectionWords, setSelectionWords] = useState<number | null>(null);
  const selection = state.edit?.selection;
  const hasSelection =
    !!selection &&
    (selection.anchor.p !== selection.focus.p || selection.anchor.o !== selection.focus.o);
  const wordsShown = statusBar.shown('words');
  useEffect(() => {
    if (!hasSelection || !wordsShown) return;
    let live = true;
    const timer = setTimeout(() => {
      void controller.current?.query({ cmd: 'wordCount' }).then((r) => {
        if (live) setSelectionWords(r?.wordCount?.selection?.words ?? null);
      });
    }, 200);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [hasSelection, wordsShown, selection, state.layoutVersion]);
  const [capsLock, setCapsLock] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => setCapsLock(e.getModifierState?.('CapsLock') ?? false);
    window.addEventListener('keydown', onKey);
    window.addEventListener('keyup', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('keyup', onKey);
    };
  }, []);
  const navOpen = !!state.navPane && !!state.doc && !state.focusMode;
  // Status bar texts (also the current values in its "customise" checklist).
  const wordTotal = state.doc?.wordCount ?? 0;
  const statusValues: Record<string, string | undefined> = state.doc
    ? {
        page: t('status.page', { current: state.page + 1, total: state.doc.pageCount }),
        words:
          hasSelection && selectionWords !== null
            ? t('statusBar.wordsSelected', { selected: selectionWords, count: wordTotal })
            : t('status.words', { count: wordTotal }),
        proofing:
          state.proofErrors === null
            ? undefined
            : state.proofErrors > 0
              ? t('statusBar.proofingErrors')
              : t('statusBar.proofingClean'),
        language: languageLabel,
        trackChanges: t(
          state.review?.trackChanges ? 'statusBar.trackChangesOn' : 'statusBar.trackChangesOff',
        ),
        capsLock: capsLock ? t('statusBar.on') : t('statusBar.off'),
        overtype: t(state.overtype ? 'statusBar.overtype' : 'statusBar.insert'),
        zoom: t('status.zoom', { value: Math.round(state.zoom * 100) }),
      }
    : {};
  const noDoc = !state.doc;
  const focusDoc = () => view.current?.focus();
  /** Chart Design gallery of the selected chart (CORE-CHART-007), loaded when a menu opens. */
  // The loader only dereferences the controller ref when a menu opens, never during render.
  // eslint-disable-next-line react-hooks/refs
  const chartGallery = docGalleryLoader(runCommand);
  const runFocus = (c: EditCommand) =>
    (controller.current?.run(c) ?? Promise.resolve(null)).then((r) => {
      focusDoc();
      return r;
    });
  /** Page whose header / footer a gallery block replaces: the caret's. */
  const blockPage = state.edit?.caret?.page ?? state.page;
  /** A registry command at the bottom of a building block gallery. */
  const blockAction = (id: string, key: string) => ({
    id,
    label: t(key),
    disabled: !registry.isEnabled(id, state),
    onSelect: () => void registry.execute(id, store.getState()),
  });
  const tableTab: RibbonTab[] = fs?.table
    ? [
        {
          id: 'table',
          context: 'table',
          label: t('ribbon.tabs.table'),
          groups: [
            {
              id: 'rowsCols',
              label: t('ribbon.groups.rowsColumns'),
              items: [
                s(
                  'table.insertRowAbove',
                  'commands.table.insertRowAbove',
                  <BetweenHorizontalStart size={SMALL} />,
                ),
                s(
                  'table.insertRowBelow',
                  'commands.table.insertRowBelow',
                  <BetweenHorizontalEnd size={SMALL} />,
                ),
                s(
                  'table.insertColumnLeft',
                  'commands.table.insertColumnLeft',
                  <BetweenVerticalStart size={SMALL} />,
                ),
                s(
                  'table.insertColumnRight',
                  'commands.table.insertColumnRight',
                  <BetweenVerticalEnd size={SMALL} />,
                ),
                s('table.deleteRow', 'commands.table.deleteRow', <Rows3 size={SMALL} />),
                s('table.deleteColumn', 'commands.table.deleteColumn', <Columns2 size={SMALL} />),
                s('table.insertCell', 'commands.table.insertCell', <Grid2x2 size={SMALL} />),
                s('table.deleteCell', 'commands.table.deleteCell', <SquareDashed size={SMALL} />),
              ],
            },
            {
              id: 'merge',
              label: t('ribbon.groups.merge'),
              items: [
                item(
                  'table.mergeCells',
                  'commands.table.mergeCells',
                  <TableCellsMerge size={ICON} />,
                ),
                item(
                  'table.splitCell',
                  'commands.table.splitCell',
                  <TableCellsSplit size={ICON} />,
                ),
                item('table.deleteTable', 'commands.table.deleteTable', <Trash size={ICON} />),
              ],
            },
            {
              id: 'tableStyle',
              label: t('ribbon.groups.tableStyle'),
              items: [
                s('table.bordersAll', 'commands.table.bordersAll', <Grid3x3 size={SMALL} />),
                s(
                  'table.bordersOutside',
                  'commands.table.bordersOutside',
                  <Grid2x2 size={SMALL} />,
                ),
                s('table.bordersInside', 'commands.table.bordersInside', <Grid2x2 size={SMALL} />),
                s('table.bordersNone', 'commands.table.bordersNone', <SquareDashed size={SMALL} />),
                custom(
                  'table.shading',
                  <ColorPicker
                    kind="shading"
                    label={t('commands.table.shading')}
                    testId="cell-shading"
                    value={fs.table.shading}
                    disabled={noDoc}
                    onPick={(hex) =>
                      void runFocus({ cmd: 'tableEdit', edit: { kind: 'shading', hex } })
                    }
                  />,
                ),
                s('table.headerRow', 'commands.table.headerRow', <Rows3 size={SMALL} />),
                s(
                  'table.properties',
                  'commands.table.properties',
                  <TableProperties size={SMALL} />,
                ),
              ],
            },
            {
              id: 'cellSize',
              label: t('ribbon.groups.cellSize'),
              items: [
                s(
                  'table.distributeRows',
                  'commands.table.distributeRows',
                  <AlignVerticalSpaceAround size={SMALL} />,
                ),
                s(
                  'table.distributeColumns',
                  'commands.table.distributeColumns',
                  <AlignHorizontalSpaceAround size={SMALL} />,
                ),
                s(
                  'table.autoFitContents',
                  'commands.table.autoFitContents',
                  <Shrink size={SMALL} />,
                ),
                s('table.autoFitWindow', 'commands.table.autoFitWindow', <Expand size={SMALL} />),
                s('table.autoFitFixed', 'commands.table.autoFitFixed', <Lock size={SMALL} />),
              ],
            },
            {
              id: 'tableText',
              label: t('ribbon.groups.tableText'),
              items: [
                item(
                  'table.textDirection',
                  'commands.table.textDirection',
                  <RotateCw size={ICON} />,
                ),
                s('table.wrap', 'commands.table.wrap', <WrapText size={SMALL} />),
              ],
            },
            {
              id: 'tableData',
              label: t('ribbon.groups.tableData'),
              items: [
                item('table.sort', 'commands.table.sort', <ArrowDownUp size={ICON} />),
                item('table.formula', 'commands.table.formula', <SquareFunction size={ICON} />),
                s('table.toText', 'commands.table.toText', <FileText size={SMALL} />),
              ],
            },
          ],
        },
      ]
    : [];

  // Header & footer contextual tab (CORE-UI-001) while a header or footer is edited.
  const storyKind = state.edit?.story?.kind;
  const headerFooterTab: RibbonTab[] =
    storyKind === 'header' || storyKind === 'footer'
      ? [
          {
            id: 'headerFooter',
            context: 'headerFooter',
            label: t('ribbon.tabs.headerFooter'),
            groups: [
              {
                id: 'hfInsert',
                label: t('ribbon.groups.headerFooter'),
                items: [
                  item('insert.header', 'commands.insert.header', <PanelTop size={ICON} />),
                  item('insert.footer', 'commands.insert.footer', <PanelBottom size={ICON} />),
                  item('insert.pageNumber', 'commands.insert.pageNumber', <Hash size={ICON} />),
                ],
              },
              {
                id: 'hfOptions',
                label: t('hf.section', { n: (state.edit?.story?.section ?? 0) + 1 }),
                items: [item('hf.linkToPrevious', 'hf.linkToPrevious', <Link2 size={ICON} />)],
              },
              {
                id: 'hfClose',
                label: t('hf.close'),
                items: [item('hf.close', 'hf.close', <X size={ICON} />)],
              },
            ],
          },
        ]
      : [];
  // Chart Design tab (CORE-CHART-007) for the chart at the selection; its callbacks only run
  // engine commands when a control is used, never during render.
  const chartTabs = docChartTabs({
    t,
    item,
    icons: {
      edit: <Pencil size={ICON} />,
      types: {
        column: <ChartColumn size={SMALL} />,
        bar: <ChartBar size={SMALL} />,
        line: <ChartLine size={SMALL} />,
        area: <ChartArea size={SMALL} />,
        pie: <ChartPie size={SMALL} />,
      },
      wrap: <WrapText size={SMALL} />,
    },
    shown: !!fs?.chart,
    load: chartGallery,
    apply: (spec) => void runCommand({ cmd: 'setChart', ...spec }),
    onSaveTemplate: () => setChartTemplateOpen(true),
    elements: {
      info: state.edit?.chart?.elements,
      run: (id) => void registry.execute(id, store.getState()),
      paneOpen: chartFormatOpen,
    },
  });
  // Equation and embedded object tabs (CORE-UI-008); the object tab comes before the picture
  // tab its preview also shows, so selecting an object activates it.
  const objectTabs = docObjectTabs(t, item, docObjectContexts(state.edit));
  const contextTabs = [
    ...tableTab,
    ...objectTabs,
    ...pictureTab,
    ...smartArtTab,
    ...chartTabs,
    ...headerFooterTab,
  ];

  const tabs: RibbonTab[] = [
    {
      id: 'file',
      label: t('ribbon.tabs.file'),
      groups: [
        {
          id: 'document',
          label: t('ribbon.groups.document'),
          items: [
            item('file.new', 'commands.file.new', <FilePlus size={ICON} />),
            item(
              'file.newFromTemplate',
              'commands.file.newFromTemplate',
              <LayoutTemplate size={ICON} />,
            ),
            item('file.open', 'commands.file.open', <FolderOpen size={ICON} />),
            item('file.save', 'commands.file.save', <Save size={ICON} />),
            item('file.saveAs', 'commands.file.saveAs', <Save size={ICON} />),
            item('file.editTemplate', 'fileVariants.editTemplate', <LayoutTemplate size={ICON} />),
            item('review.savePassword', 'commands.review.savePassword', <Lock size={ICON} />),
            item('file.openSample', 'commands.file.openSample', <FileText size={ICON} />),
          ],
        },
        {
          id: 'output',
          label: t('ribbon.groups.output'),
          items: [
            item('file.exportPdf', 'commands.file.exportPdf', <FileDown size={ICON} />),
            item(
              'file.exportPdfOptions',
              'commands.file.exportPdfOptions',
              <FileDown size={ICON} />,
            ),
            item('file.print', 'commands.file.print', <Printer size={ICON} />),
            ...(registry.has('file.checkUpdates')
              ? [
                  item(
                    'file.checkUpdates',
                    'commands.file.checkUpdates',
                    <RefreshCw size={ICON} />,
                    'small',
                  ),
                ]
              : []),
          ],
        },
        {
          id: 'protect',
          label: t('protect.group'),
          items: [
            item('file.protect', 'protect.cmd.protect', <Lock size={ICON} />),
            item(
              'review.nextEditableRegion',
              'protect.cmd.nextRegion',
              <ShieldCheck size={ICON} />,
              'small',
            ),
            item(
              'review.showEditableRegions',
              'protect.cmd.showRegions',
              <ShieldCheck size={ICON} />,
              'small',
            ),
          ],
        },
        {
          id: 'signature',
          label: t('ribbon.groups.signature'),
          items: [
            item('file.sign', 'commands.file.sign', <Signature size={ICON} />),
            item('file.signLine', 'commands.file.signLine', <PenLine size={ICON} />),
            item('file.exportSignedPdf', 'commands.file.exportSignedPdf', <FileDown size={ICON} />),
            item(
              'file.verifySignatures',
              'commands.file.verifySignatures',
              <ShieldCheck size={ICON} />,
            ),
          ],
        },
      ],
    },
    {
      id: 'home',
      label: t('ribbon.tabs.home'),
      groups: [
        {
          id: 'history',
          label: t('ribbon.groups.history'),
          items: [
            s('edit.undo', 'commands.edit.undo', <Undo2 size={SMALL} />),
            s('edit.redo', 'commands.edit.redo', <Redo2 size={SMALL} />),
            s('file.save', 'commands.file.save', <Save size={SMALL} />),
          ],
        },
        {
          id: 'clipboard',
          label: t('ribbon.groups.clipboard'),
          launcher: {
            id: 'edit.clipboardPanel',
            label: t('clipHistory.open'),
            onSelect: () => setClipPanel((o) => !o),
          },
          items: [
            item('edit.paste', 'commands.edit.paste', <ClipboardPaste size={ICON} />),
            s('edit.cut', 'commands.edit.cut', <Scissors size={SMALL} />),
            s('edit.copy', 'commands.edit.copy', <Copy size={SMALL} />),
            s('edit.pasteSpecial', 'commands.edit.pasteSpecial', <ClipboardType size={SMALL} />),
            s('format.painter', 'commands.format.painter', <Paintbrush size={SMALL} />),
          ],
        },
        {
          id: 'font',
          label: t('ribbon.groups.font'),
          items: [
            custom(
              'format.fontFamily',
              <FontFamilyPicker
                format={fs}
                fonts={state.fonts}
                run={runCommand}
                disabled={noDoc}
                onDone={focusDoc}
              />,
            ),
            custom(
              'format.fontSize',
              <FontSizeBox format={fs} run={runCommand} disabled={noDoc} onDone={focusDoc} />,
            ),
            s('format.bold', 'commands.format.bold', <Bold size={SMALL} />),
            s('format.italic', 'commands.format.italic', <Italic size={SMALL} />),
            s('format.underline', 'commands.format.underline', <Underline size={SMALL} />),
            s('format.strike', 'commands.format.strike', <Strikethrough size={SMALL} />),
            s('format.subscript', 'commands.format.subscript', <Subscript size={SMALL} />),
            s('format.superscript', 'commands.format.superscript', <Superscript size={SMALL} />),
            s('format.growStep', 'commands.format.growStep', <AArrowUp size={SMALL} />),
            s('format.shrinkStep', 'commands.format.shrinkStep', <AArrowDown size={SMALL} />),
            s('format.changeCase', 'commands.format.changeCase', <CaseSensitive size={SMALL} />),
            custom(
              'format.color',
              <ColorPicker
                kind="color"
                label={t('commands.format.color')}
                testId="text-color"
                value={fs?.color}
                disabled={noDoc}
                onPick={(hex) =>
                  void runFocus({ cmd: 'formatText', format: { kind: 'color', hex } })
                }
              />,
            ),
            custom(
              'format.highlight',
              <ColorPicker
                kind="highlight"
                label={t('commands.format.highlight')}
                testId="highlight-color"
                value={fs?.highlight}
                disabled={noDoc}
                onPick={(hex) =>
                  void runFocus({ cmd: 'formatText', format: { kind: 'highlight', hex } })
                }
              />,
            ),
            s('format.clear', 'commands.format.clear', <RemoveFormatting size={SMALL} />),
            s('format.fontDialog', 'commands.format.fontDialog', <WandSparkles size={SMALL} />),
            s('format.phoneticGuide', 'commands.format.phoneticGuide', <Languages size={SMALL} />),
            s('format.combineChars', 'commands.format.combineChars', <Rows3 size={SMALL} />),
          ],
        },
        {
          id: 'paragraph',
          label: t('ribbon.groups.paragraph'),
          items: [
            s('format.bullets', 'commands.format.bullets', <List size={SMALL} />),
            s('format.numbering', 'commands.format.numbering', <ListOrdered size={SMALL} />),
            custom(
              'format.multilevel',
              <MultilevelMenu
                disabled={noDoc}
                entries={(
                  [
                    ['format.defineList', 'commands.format.defineList'],
                    ['format.setNumberingValue', 'commands.format.setNumberingValue'],
                    ['format.restartNumbering', 'commands.format.restartNumbering'],
                    ['format.continueNumbering', 'commands.format.continueNumbering'],
                  ] as const
                ).map(([id, key]) => {
                  const it = item(id, key, null);
                  return { id, label: it.label, enabled: it.enabled, onSelect: it.onSelect };
                })}
              />,
            ),
            s('format.outdent', 'commands.format.outdent', <ListIndentDecrease size={SMALL} />),
            s('format.indent', 'commands.format.indent', <ListIndentIncrease size={SMALL} />),
            s('format.alignLeft', 'commands.format.alignLeft', <AlignLeft size={SMALL} />),
            s('format.alignCenter', 'commands.format.alignCenter', <AlignCenter size={SMALL} />),
            s('format.alignRight', 'commands.format.alignRight', <AlignRight size={SMALL} />),
            s('format.justify', 'commands.format.justify', <AlignJustify size={SMALL} />),
            custom(
              'format.spacing',
              <SpacingMenu format={fs} run={runCommand} disabled={noDoc} onDone={focusDoc} />,
            ),
          ],
        },
        {
          id: 'styles',
          label: t('ribbon.groups.styles'),
          launcher: {
            id: 'view.stylesPane',
            label: t('commands.view.stylesPane'),
            onSelect: () => void registry.execute('view.stylesPane', store.getState()),
          },
          // The quick styles as a gallery of tiles drawn in their own formatting.
          items: [
            custom(
              'styles.gallery',
              <StylesGallery
                load={loadStyles}
                current={fs?.style ?? null}
                refreshKey={`${state.doc?.name ?? ''}:${fs?.style ?? ''}`}
                disabled={noDoc}
                apply={(id) => void runFocus({ cmd: 'applyStyle', id })}
                onMore={() => void registry.execute('view.stylesPane', store.getState())}
              />,
            ),
          ],
        },
        // Editing, then Voice: the order of word processors' Home tab.
        {
          id: 'editing',
          label: t('ribbon.groups.editing'),
          items: [
            s('edit.find', 'commands.edit.find', <Search size={SMALL} />),
            s('edit.replace', 'commands.edit.replace', <Replace size={SMALL} />),
            s(
              'edit.selectAll',
              'commands.edit.selectAll',
              <SquareDashedMousePointer size={SMALL} />,
            ),
          ],
        },
        {
          id: 'voice',
          label: t('ribbon.groups.speech'),
          items: [item('edit.dictate', 'commands.edit.dictate', <Mic size={ICON} />)],
        },
      ],
    },
    {
      id: 'insert',
      label: t('ribbon.tabs.insert'),
      groups: [
        {
          id: 'objects',
          label: t('ribbon.groups.insertObjects'),
          items: [
            item('insert.table', 'commands.insert.table', <Table size={ICON} />),
            s('table.fromText', 'commands.table.fromText', <Table2 size={SMALL} />),
            item('insert.image', 'commands.insert.image', <Image size={ICON} />),
            item('object.insert', 'commands.object.insert', <FileSpreadsheet size={ICON} />),
            s('object.updateLinks', 'commands.object.updateLinks', <RefreshCw size={SMALL} />),
            item('insert.smartart', 'commands.insert.smartart', <Network size={ICON} />),
            item('insert.chart', 'commands.insert.chart', <ChartColumn size={ICON} />),
          ],
        },
        {
          id: 'symbols',
          label: t('ribbon.groups.symbols'),
          items: [
            item('insert.symbol', 'commands.insert.symbol', <Omega size={ICON} />),
            item('insert.icon', 'commands.insert.icon', <Shapes size={ICON} />),
            item('insert.dropCap', 'commands.insert.dropCap', <Type size={ICON} />),
          ],
        },
        {
          id: 'equation',
          label: t('ribbon.groups.equation'),
          items: [
            item('insert.equation', 'commands.insert.equation', <Sigma size={ICON} />),
            item('insert.field', 'commands.insert.field', <Braces size={ICON} />),
            item('insert.control', 'commands.insert.control', <ListChecks size={ICON} />),
            item('insert.xmlMapping', 'commands.insert.xmlMapping', <FileCode size={ICON} />),
            item('insert.signatureLine', 'commands.insert.signatureLine', <PenLine size={ICON} />),
            item(
              'equation.professional',
              'commands.equation.professional',
              <SquareFunction size={ICON} />,
            ),
            item('equation.linear', 'commands.equation.linear', <TextCursorInput size={ICON} />),
            item('equation.editInPlace', 'commands.equation.editInPlace', <Pencil size={ICON} />),
          ],
        },
        {
          id: 'formFields',
          label: t('ribbon.groups.formFields'),
          items: [
            item('insert.formText', 'commands.insert.formText', <TextCursorInput size={ICON} />),
            s('insert.formCheckBox', 'commands.insert.formCheckBox', <SquareCheck size={SMALL} />),
            s(
              'insert.formDropDown',
              'commands.insert.formDropDown',
              <SquareChevronDown size={SMALL} />,
            ),
            s(
              'insert.formProperties',
              'commands.insert.formProperties',
              <SlidersHorizontal size={SMALL} />,
            ),
            item(
              'review.protectForms',
              'commands.review.protectForms',
              <LockKeyhole size={ICON} />,
            ),
          ],
        },
        {
          id: 'pages',
          label: t('ribbon.groups.pages'),
          items: [
            item(
              'insert.pageBreak',
              'commands.insert.pageBreak',
              <SquareSplitVertical size={ICON} />,
            ),
            item(
              'insert.sectionBreak',
              'commands.insert.sectionBreak',
              <SeparatorHorizontal size={ICON} />,
            ),
          ],
        },
        {
          id: 'links',
          label: t('ribbon.groups.links'),
          items: [item('insert.link', 'commands.insert.link', <Link2 size={ICON} />)],
        },
        {
          id: 'headerFooter',
          label: t('ribbon.groups.headerFooter'),
          items: [
            custom(
              'insert.headerGallery',
              <BlockGalleryMenu
                groups={['header']}
                label={t('commands.insert.header')}
                icon={<PanelTop size={16} />}
                testId="header-gallery"
                disabled={noDoc}
                run={runFocus}
                query={queryCommand}
                page={blockPage}
                actions={[blockAction('insert.header', 'blocks.menu.editHeader')]}
              />,
            ),
            custom(
              'insert.footerGallery',
              <BlockGalleryMenu
                groups={['footer']}
                label={t('commands.insert.footer')}
                icon={<PanelBottom size={16} />}
                testId="footer-gallery"
                disabled={noDoc}
                run={runFocus}
                query={queryCommand}
                page={blockPage}
                actions={[blockAction('insert.footer', 'blocks.menu.editFooter')]}
              />,
            ),
            custom(
              'insert.pageNumberGallery',
              <BlockGalleryMenu
                groups={['pageNumber']}
                label={t('commands.insert.pageNumber')}
                icon={<Hash size={16} />}
                testId="page-number-gallery"
                disabled={noDoc}
                run={runFocus}
                query={queryCommand}
                page={blockPage}
                actions={[blockAction('insert.pageNumber', 'blocks.menu.pageNumberHere')]}
              />,
            ),
          ],
        },
        {
          id: 'buildingBlocks',
          label: t('ribbon.groups.buildingBlocks'),
          items: [
            custom(
              'insert.coverPageGallery',
              <BlockGalleryMenu
                groups={['coverPage']}
                label={t('commands.insert.coverPage')}
                icon={<LayoutTemplate size={16} />}
                testId="cover-page-gallery"
                disabled={noDoc}
                run={runFocus}
                query={queryCommand}
                page={blockPage}
                actions={[blockAction('insert.blocksOrganizer', 'commands.insert.blocksOrganizer')]}
              />,
            ),
            custom(
              'insert.buildingBlocks',
              <BlockGalleryMenu
                groups={['quickParts', 'textBox', 'table']}
                label={t('commands.insert.buildingBlocks')}
                icon={<LibraryBig size={16} />}
                testId="building-blocks"
                disabled={noDoc}
                run={runFocus}
                query={queryCommand}
                page={blockPage}
                actions={[
                  blockAction('insert.autoText', 'commands.insert.autoText'),
                  blockAction('insert.saveBlock', 'commands.insert.saveBlock'),
                  blockAction('insert.blocksOrganizer', 'commands.insert.blocksOrganizer'),
                ]}
              />,
            ),
          ],
        },
      ],
    },
    {
      id: 'design',
      label: t('ribbon.tabs.design'),
      groups: [
        {
          id: 'documentFormatting',
          label: t('ribbon.groups.documentFormatting'),
          items: [
            custom('design.themes', <ThemeGallery run={runCommand} disabled={noDoc} />),
            custom('design.styleSets', <StyleSetGallery run={runCommand} disabled={noDoc} />),
            item(
              'design.themeFile',
              'fileVariants.applyThemeFile',
              <FolderOpen size={ICON} />,
              'small',
            ),
            item(
              'design.saveThemeFile',
              'fileVariants.saveThemeFile',
              <Save size={ICON} />,
              'small',
            ),
          ],
        },
        {
          id: 'pageBackground',
          label: t('ribbon.groups.pageBackground'),
          items: [
            custom(
              'design.watermark',
              <WatermarkMenu
                run={runCommand}
                disabled={noDoc}
                onCustom={() => void registry.execute('design.customWatermark', store.getState())}
              />,
            ),
            custom(
              'design.pageColor',
              <ColorPicker
                kind="shading"
                label={t('commands.design.pageColor')}
                testId="page-color"
                value={null}
                disabled={noDoc}
                onPick={(hex) => void runFocus({ cmd: 'setPageColor', color: hex })}
              />,
            ),
            item('design.pageBorders', 'commands.design.pageBorders', <Frame size={ICON} />),
          ],
        },
      ],
    },
    {
      id: 'layout',
      label: t('ribbon.tabs.layout'),
      groups: [
        {
          id: 'pageSetup',
          label: t('ribbon.groups.pageSetup'),
          items: [
            item('layout.margins', 'commands.layout.margins', <Ruler size={ICON} />),
            item(
              'layout.orientation',
              'commands.layout.orientation',
              <RectangleHorizontal size={ICON} />,
            ),
            item('layout.size', 'commands.layout.size', <Scaling size={ICON} />),
            item('layout.columns', 'commands.layout.columns', <Columns2 size={ICON} />),
            item('layout.pageSetup', 'commands.layout.pageSetup', <Ruler size={ICON} />),
            s('layout.lineNumbers', 'commands.layout.lineNumbers', <Rows4 size={SMALL} />),
            s('layout.hyphenation', 'commands.layout.hyphenation', <WrapText size={SMALL} />),
            s('layout.textDirection', 'commands.layout.textDirection', <RotateCw size={SMALL} />),
          ],
        },
        {
          id: 'arrange',
          label: t('ribbon.groups.arrange'),
          items: [item('picture.position', 'commands.picture.position', <Image size={ICON} />)],
        },
      ],
    },
    {
      id: 'references',
      label: t('ribbon.tabs.references'),
      groups: [
        {
          id: 'toc',
          label: t('ribbon.groups.toc'),
          items: [
            item('references.insertToc', 'commands.references.insertToc', <ListTree size={ICON} />),
            s('references.updateToc', 'commands.references.updateToc', <RefreshCw size={SMALL} />),
          ],
        },
        {
          id: 'footnotes',
          label: t('ribbon.groups.footnotes'),
          items: [
            item('insert.footnote', 'commands.insert.footnote', <StickyNote size={ICON} />),
            s('insert.endnote', 'commands.insert.endnote', <StickyNote size={SMALL} />),
            s('references.notes', 'commands.references.notes', <PanelRight size={SMALL} />),
            s(
              'references.convertNotes',
              'commands.references.convertNotes',
              <ArrowLeftRight size={SMALL} />,
            ),
          ],
        },
        {
          id: 'captions',
          label: t('ribbon.groups.captions'),
          items: [
            item(
              'references.insertCaption',
              'commands.references.insertCaption',
              <Tag size={ICON} />,
            ),
            item(
              'references.crossReference',
              'commands.references.crossReference',
              <Link2 size={ICON} />,
            ),
            s(
              'references.tableOfFigures',
              'commands.references.tableOfFigures',
              <ListTree size={SMALL} />,
            ),
          ],
        },
        {
          id: 'refIndex',
          label: t('ribbon.groups.refIndex'),
          items: [
            item('references.markEntry', 'commands.references.markEntry', <Bookmark size={ICON} />),
            s(
              'references.insertIndex',
              'commands.references.insertIndex',
              <ListTree size={SMALL} />,
            ),
          ],
        },
        {
          id: 'citations',
          label: t('ribbon.groups.citations'),
          items: [
            item(
              'references.insertCitation',
              'commands.references.insertCitation',
              <Quote size={ICON} />,
            ),
            s(
              'references.manageSources',
              'commands.references.manageSources',
              <Library size={SMALL} />,
            ),
            custom(
              'references.citationStyle',
              <CitationStylePicker
                run={runCommand}
                disabled={noDoc || !!state.edit?.story}
                refreshKey={`${state.doc?.name ?? ''}|${state.dialog ?? ''}`}
              />,
            ),
            s(
              'references.insertBibliography',
              'commands.references.insertBibliography',
              <BookOpen size={SMALL} />,
            ),
          ],
        },
        {
          id: 'authorities',
          label: t('ribbon.groups.authorities'),
          items: [
            item(
              'references.markCitation',
              'commands.references.markCitation',
              <Scale size={ICON} />,
            ),
            s('references.insertToa', 'commands.references.insertToa', <ListTree size={SMALL} />),
          ],
        },
        {
          id: 'fields',
          label: t('ribbon.groups.fields'),
          items: [
            item(
              'references.updateFields',
              'commands.references.updateFields',
              <RefreshCw size={ICON} />,
            ),
            s('fields.toggleCodes', 'commands.fields.toggleCodes', <Braces size={SMALL} />),
            s('fields.lock', 'commands.fields.lock', <Lock size={SMALL} />),
            s('fields.unlock', 'commands.fields.unlock', <LockOpen size={SMALL} />),
            s('fields.unlink', 'commands.fields.unlink', <Unlink size={SMALL} />),
          ],
        },
      ],
    },
    {
      id: 'mailings',
      label: t('ribbon.tabs.mailings'),
      groups: [
        {
          id: 'mergeData',
          label: t('ribbon.groups.mergeData'),
          items: [
            item(
              'mailings.selectRecipients',
              'commands.mailings.selectRecipients',
              <FileSpreadsheet size={ICON} />,
            ),
            item('mailings.insertField', 'commands.mailings.insertField', <Mails size={ICON} />),
            s(
              'mailings.editRecipients',
              'commands.mailings.editRecipients',
              <ListChecks size={SMALL} />,
            ),
            s('mailings.fromServer', 'commands.mailings.fromServer', <Database size={SMALL} />),
          ],
        },
        {
          id: 'mergeRules',
          label: t('ribbon.groups.mergeRules'),
          items: [
            custom(
              'mailings.rules',
              <CommandMenu
                label={t('commands.mailings.rules')}
                icon={<ListChecks size={SMALL} />}
                testId="merge-rules-menu"
                disabled={noDoc}
                entries={RULE_KINDS.map((k) =>
                  menuEntry(`mailings.rule.${k}`, `mailMerge.rule.${k}`),
                )}
              />,
            ),
          ],
        },
        {
          id: 'mergeCreate',
          label: t('ribbon.groups.mergeCreate'),
          items: [
            s('mailings.envelopes', 'commands.mailings.envelopes', <Mail size={SMALL} />),
            s('mailings.labels', 'commands.mailings.labels', <Tags size={SMALL} />),
          ],
        },
        {
          id: 'mergePreview',
          label: t('ribbon.groups.mergePreview'),
          items: [
            item('mailings.preview', 'commands.mailings.preview', <Eye size={ICON} />),
            s(
              'mailings.previousRecord',
              'commands.mailings.previousRecord',
              <ChevronLeft size={SMALL} />,
            ),
            s('mailings.nextRecord', 'commands.mailings.nextRecord', <ChevronRight size={SMALL} />),
          ],
        },
        {
          id: 'mergeFinish',
          label: t('ribbon.groups.mergeFinish'),
          items: [
            item(
              'mailings.mergeDocument',
              'commands.mailings.mergeDocument',
              <Files size={ICON} />,
            ),
            item('mailings.mergePdf', 'commands.mailings.mergePdf', <FileDown size={ICON} />),
            custom(
              'mailings.finish',
              <CommandMenu
                label={t('commands.mailings.finish')}
                icon={<Files size={SMALL} />}
                testId="merge-finish-menu"
                disabled={noDoc}
                entries={(
                  [
                    ['mailings.finishDocument', 'commands.mailings.finishDocument'],
                    ['mailings.finishPdfs', 'commands.mailings.finishPdfs'],
                    ['mailings.finishEmail', 'commands.mailings.finishEmail'],
                  ] as const
                ).map(([id, key]) => menuEntry(id, key))}
              />,
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
          id: 'proofing',
          label: t('ribbon.groups.proofing'),
          items: [
            item('review.spelling', 'commands.review.spelling', <SpellCheck size={ICON} />),
            item('review.grammar', 'commands.review.grammar', <SpellCheck2 size={ICON} />),
            item(
              'review.spellingDialog',
              'commands.review.spellingDialog',
              <SpellCheck size={ICON} />,
            ),
            item('review.thesaurus', 'commands.review.thesaurus', <BookA size={ICON} />),
            item('review.language', 'commands.review.language', <Languages size={ICON} />),
            item('review.wordCount', 'commands.review.wordCount', <Hash size={ICON} />),
            item('review.autoCorrect', 'commands.review.autoCorrect', <WandSparkles size={ICON} />),
            item('review.checkDocument', 'inspect.command', <ClipboardCheck size={ICON} />),
            item(
              'review.checkAdminFormat',
              'commands.review.checkAdminFormat',
              <FileCheck2 size={ICON} />,
            ),
          ],
        },
        {
          id: 'speech',
          label: t('ribbon.groups.speech'),
          items: [item('review.readAloud', 'commands.review.readAloud', <Volume2 size={ICON} />)],
        },
        {
          id: 'language',
          label: t('ribbon.groups.language'),
          items: [
            item('review.translate', 'commands.review.translate', <Languages size={ICON} />),
            item(
              'review.translateDocument',
              'commands.review.translateDocument',
              <FileText size={ICON} />,
            ),
          ],
        },
        {
          id: 'ai',
          label: t('ribbon.groups.ai'),
          items: [item('ai.pane', 'commands.ai.pane', <Sparkles size={ICON} />)],
        },
        {
          id: 'tracking',
          label: t('ribbon.groups.tracking'),
          items: [
            item(
              'review.trackChanges',
              'commands.review.trackChanges',
              <FilePenLine size={ICON} />,
            ),
            s('review.lockTracking', 'commands.review.lockTracking', <Lock size={SMALL} />),
            s(
              'review.moveParagraphUp',
              'commands.review.moveParagraphUp',
              <ArrowUpToLine size={SMALL} />,
            ),
            s(
              'review.moveParagraphDown',
              'commands.review.moveParagraphDown',
              <ArrowDownToLine size={SMALL} />,
            ),
            custom(
              'review.markupMode',
              <MarkupModeSelect
                disabled={noDoc}
                entries={(
                  [
                    ['review.markup.simple', 'commands.review.markupSimple'],
                    ['review.markup.all', 'commands.review.markupAll'],
                    ['review.markup.none', 'commands.review.markupNone'],
                    ['review.markup.original', 'commands.review.markupOriginal'],
                  ] as const
                ).map(([id, key]) => markupEntry(id, key))}
              />,
            ),
            custom(
              'review.showMarkup',
              <ShowMarkupMenu
                disabled={noDoc}
                entries={(
                  [
                    ['review.markup.insDel', 'commands.review.markupInsDel'],
                    ['review.markup.formatting', 'commands.review.markupFormatting'],
                    ['review.markup.comments', 'commands.review.markupComments'],
                    ['review.markup.balloons', 'commands.review.markupBalloons'],
                  ] as const
                ).map(([id, key]) => markupEntry(id, key))}
                reviewers={reviewersOf(state.review)}
                hidden={state.review?.markup?.hiddenReviewers ?? []}
                onReviewers={(hiddenReviewers) =>
                  void runCommand({ cmd: 'setMarkup', hiddenReviewers })
                }
              />,
            ),
            s('review.reviewingPane', 'commands.review.reviewingPane', <PanelLeft size={SMALL} />),
            s(
              'review.reviewingPaneBottom',
              'commands.review.reviewingPaneBottom',
              <PanelBottom size={SMALL} />,
            ),
            item('review.pane', 'commands.review.pane', <PanelRight size={ICON} />),
          ],
        },
        {
          id: 'changes',
          label: t('ribbon.groups.changes'),
          items: [
            item('review.accept', 'commands.review.accept', <Check size={ICON} />),
            item('review.reject', 'commands.review.reject', <CircleX size={ICON} />),
            s('review.acceptAll', 'commands.review.acceptAll', <CheckCheck size={SMALL} />),
            s('review.rejectAll', 'commands.review.rejectAll', <X size={SMALL} />),
            s('review.previous', 'commands.review.previous', <ChevronLeft size={SMALL} />),
            s('review.next', 'commands.review.next', <ChevronRight size={SMALL} />),
          ],
        },
        {
          id: 'compare',
          label: t('ribbon.groups.compare'),
          items: [
            item('review.compare', 'commands.review.compare', <GitCompare size={ICON} />),
            item('review.combine', 'commands.review.combine', <GitMerge size={ICON} />),
            item('review.versions', 'commands.review.versions', <History size={ICON} />),
          ],
        },
        {
          id: 'comments',
          label: t('ribbon.groups.comments'),
          items: [
            item(
              'review.newComment',
              'commands.review.newComment',
              <MessageSquarePlus size={ICON} />,
            ),
            item('review.comments', 'commands.review.comments', <MessagesSquare size={ICON} />),
            s(
              'review.resolveComment',
              'commands.review.resolveComment',
              <CircleCheck size={SMALL} />,
            ),
            s('review.deleteComment', 'commands.review.deleteComment', <X size={SMALL} />),
            s(
              'review.previousComment',
              'commands.review.previousComment',
              <ChevronLeft size={SMALL} />,
            ),
            s('review.nextComment', 'commands.review.nextComment', <ChevronRight size={SMALL} />),
          ],
        },
        {
          id: 'protect',
          label: t('ribbon.groups.protect'),
          items: [
            item('review.savePassword', 'commands.review.savePassword', <KeyRound size={ICON} />),
            item(
              'review.protectForms',
              'commands.review.protectForms',
              <LockKeyhole size={ICON} />,
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
          id: 'views',
          label: t('ribbon.groups.views'),
          items: [
            item('view.readMode', 'commands.view.readMode', <BookOpen size={ICON} />),
            item('view.printLayout', 'commands.view.printLayout', <FileText size={ICON} />),
            item('view.webLayout', 'commands.view.webLayout', <Globe size={ICON} />),
            item('view.outline', 'commands.view.outline', <ListTree size={ICON} />),
            item('view.draft', 'commands.view.draft', <ScrollText size={ICON} />),
            item('view.focus', 'commands.view.focus', <Focus size={ICON} />),
            item('view.focusedReader', 'commands.view.focusedReader', <BookOpenText size={ICON} />),
          ],
        },
        {
          id: 'show',
          label: t('ribbon.groups.show'),
          items: [
            item('view.navigationPane', 'commands.view.navigationPane', <PanelLeft size={ICON} />),
            item('view.stylesPane', 'commands.view.stylesPane', <Palette size={ICON} />),
            item('view.formattingMarks', 'commands.view.formattingMarks', <Pilcrow size={ICON} />),
          ],
        },
        {
          id: 'zoom',
          label: t('ribbon.groups.zoom'),
          items: [
            item('view.zoomIn', 'commands.view.zoomIn', <ZoomIn size={ICON} />),
            item('view.zoomOut', 'commands.view.zoomOut', <ZoomOut size={ICON} />),
            item(
              'view.zoomReset',
              'commands.view.zoomReset',
              <span style={{ fontWeight: 700, fontSize: 13 }}>1:1</span>,
            ),
            item('view.fitWidth', 'commands.view.fitWidth', <Maximize2 size={ICON} />),
          ],
        },
        {
          id: 'appearance',
          label: t('ribbon.groups.appearance'),
          items: [
            item('view.ruler', 'commands.view.ruler', <Ruler size={ICON} />),
            item('view.collaborators', 'commands.view.collaborators', <Users size={ICON} />),
            item('view.toggleTheme', 'commands.view.toggleTheme', <MoonStar size={ICON} />),
            item('view.density', `commands.view.densityIs.${density}`, <Rows4 size={ICON} />),
            item('view.toggleLanguage', 'commands.view.toggleLanguage', <Languages size={ICON} />),
          ],
        },
        {
          id: 'window',
          label: t('ribbon.groups.window'),
          items: [
            item('window.new', 'commands.window.new', <AppWindow size={ICON} />),
            item('window.arrangeAll', 'commands.window.arrangeAll', <LayoutGrid size={ICON} />),
            item('window.split', 'commands.window.split', <Rows2 size={ICON} />),
            item('window.sideBySide', 'commands.window.sideBySide', <Columns2 size={ICON} />),
            item('window.syncScroll', 'commands.window.syncScroll', <ArrowDownUp size={ICON} />),
            item(
              'window.resetPosition',
              'commands.window.resetPosition',
              <RotateCcw size={ICON} />,
            ),
          ],
        },
        {
          id: 'automation',
          label: t('ribbon.groups.automation'),
          items: [
            item('script.editor', 'commands.view.scriptEditor', <SquareTerminal size={ICON} />),
            item('script.record', 'commands.view.recordScript', <CircleDot size={ICON} />),
            item('script.convertVba', 'commands.view.convertVba', <FileCode2 size={ICON} />),
            item('script.viewMacros', 'commands.view.viewMacros', <FileSearch size={ICON} />),
          ],
        },
      ],
    },
  ];

  const status = state.status;
  const appName = t(`app.${app}`);
  const appTitle = t(`appTitle.${app}`);
  // Window title before a document is opened: the product's display name (ADR-0006).
  useEffect(() => {
    if (!state.doc) platform.window.setTitle(appTitle);
  }, [platform, appTitle, state.doc]);

  /** Signs a copy (.docx) or a PDF export and saves it next to the document. */
  const signNow = async (request: SignRequest, form: SignForm) => {
    const signing = platform.signing;
    if (!signing) return;
    store.setState({ panel: null, status: { kind: 'busy', key: 'status.signing' } });
    try {
      await controller.current?.idle();
      const pdf = form.output === 'pdf';
      const line = pdf ? null : pendingLine.current;
      pendingLine.current = null;
      let bytes: Uint8Array;
      let req = request;
      if (line) {
        // A signature line: its pictures as signed / invalid come from the engine
        // ([MS-OFFCRYPTO] idValidSigLnImg / idInvalidSigLnImg); an unchanged file is
        // signed as opened so its earlier signatures stay valid.
        const r = await controller.current?.run(imagesCommand(line, isoDay(new Date())));
        const images = r?.signatureLineImages;
        if (!images) throw new Error(r?.error ?? 'signature line');
        req = { ...request, signatureLine: lineSignRequest(line, images) };
        bytes = await bytesToSign(!!store.getState().edit?.modified, openedBytes.current, () =>
          platform.engine.saveDocx(),
        );
      } else {
        bytes = pdf ? await printLayoutPdf() : await platform.engine.saveDocx();
      }
      const signed = await signing.sign(bytes, req);
      await platform.shell.setSetting?.(SIGNING_SETTING_KEY, JSON.stringify(remember(form)));
      const name = request.documentName;
      const saved = await platform.files.saveFile(
        signed,
        name,
        pdf
          ? 'application/pdf'
          : 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      );
      store.setState({
        status: saved ? { kind: 'info', key: 'status.signed', params: { name } } : { kind: 'idle' },
      });
    } catch (e) {
      fail('errors.sign', e);
    }
  };

  const closePanel = () => {
    store.setState({ panel: null });
    view.current?.focus();
  };
  const closeDialog = () => {
    store.setState({ dialog: null });
    // After the dialog has unmounted: focusing while its focus trap is still mounted lost the
    // focus to the page body (WCAG 2.4.3 — focus returns to the document).
    requestAnimationFrame(() => view.current?.focus());
  };

  const rulersOn =
    state.showRuler && !!state.doc && !state.focusMode && state.viewMode !== 'outline';
  const closeTaskPane = (id: string) => {
    if (addins.close(id)) return;
    if (id === 'chartFormat') setChartFormatOpen(false);
    else if (id === 'navigation') store.setState({ navPane: null });
    else if (id === 'ai') ai.close();
    else if (id === 'thesaurus') thesaurus.close();
    else
      store.setState({
        taskPanes: togglePane(store.getState().taskPanes, id as DocTaskPane, false),
      });
  };
  // Task panes (CORE-UI-006): navigation, styles, comments, version history.
  const paneDefs: TaskPaneDef[] = [];
  if (navOpen && state.navPane && state.doc) {
    paneDefs.push({
      id: 'navigation',
      title: t('views.nav.title'),
      icon: <PanelLeft size={14} aria-hidden />,
      defaultDock: 'left',
      closeTestId: 'navigation-close',
      content: (
        <NavigationPane
          embedded
          tab={state.navPane}
          onTab={(navPane) => store.setState({ navPane })}
          outline={state.outline}
          caretPara={state.edit?.selection.focus.p}
          page={state.page}
          pageCount={state.doc.pageCount}
          layoutVersion={state.layoutVersion}
          renderThumbnail={renderThumb}
          goToPage={(i) => view.current?.scrollToPage(i)}
          run={runCommand}
          onClearHighlights={() => view.current?.setFindHighlights([])}
          onClose={() => {
            store.setState({ navPane: null });
            view.current?.focus();
          }}
        />
      ),
    });
  }
  for (const id of state.doc ? state.taskPanes : []) {
    if (id === 'styles') {
      paneDefs.push({
        id,
        title: t('taskPane.styles.title'),
        icon: <Palette size={14} aria-hidden />,
        content: (
          <StylesPane
            load={loadStyles}
            current={fs?.style ?? null}
            refreshKey={`${state.doc?.name ?? ''}:${fs?.style ?? ''}`}
            apply={(styleId) => void runFocus({ cmd: 'applyStyle', id: styleId })}
            request={stylesRequest}
            onRequestDone={() => setStylesRequest(null)}
            manage={{
              run: runCommand,
              query: queryCommand,
              openFile: (accept) => platform.files.openFile(accept),
              saveFile: (bytes, name, mime) => platform.files.saveFile(bytes, name, mime),
              openPane: (pane) =>
                store.setState({
                  taskPanes: togglePane(store.getState().taskPanes, pane, true),
                }),
            }}
          />
        ),
      });
    } else if (id === 'styleInspector') {
      paneDefs.push({
        id,
        title: t('docStyles.inspector.title'),
        icon: <Palette size={14} aria-hidden />,
        content: (
          <StyleInspectorPane query={queryCommand} run={runCommand} refreshKey={state.edit} />
        ),
      });
    } else if (id === 'revealFormatting') {
      paneDefs.push({
        id,
        title: t('docStyles.reveal.title'),
        icon: <Eye size={14} aria-hidden />,
        content: <RevealFormattingPane query={queryCommand} refreshKey={state.edit} />,
      });
    } else if (id === 'comments') {
      paneDefs.push({
        id,
        title: t('comments.title'),
        icon: <MessageSquare size={14} aria-hidden />,
        content: (
          <CommentsPanel
            embedded
            people={platform.people}
            review={state.review}
            run={runCommand}
            language={state.language}
            onClose={() => {
              closeTaskPane(id);
              focusDoc();
            }}
          />
        ),
      });
    } else if (id === 'script') {
      paneDefs.push({
        id,
        title: t('script.title'),
        icon: <SquareTerminal size={14} aria-hidden />,
        content: (
          <ScriptPane
            app="doc"
            host={() =>
              store.getState().doc
                ? docScriptHost({ run: runCommand, onEnd: () => focusDoc() })
                : null
            }
            recorder={recorder}
            checkShortcut={scriptBindings.checkShortcut}
            vbaRequest={vbaRequest}
            onVbaRequestHandled={vbaRequestHandled}
            vbaDraft={macroViewer.draft}
            onVbaDraftHandled={macroViewer.draftHandled}
            {...(platform.scriptSchedules ? { schedules: platform.scriptSchedules } : {})}
          />
        ),
      });
    } else {
      paneDefs.push({
        id,
        title: t('versions.title'),
        icon: <History size={14} aria-hidden />,
        content: (
          <VersionsPane
            embedded
            versions={platform.versions}
            openFile={() => platform.files.openFile(['.docx', '.odt', '.doc'])}
            // BlueDoc shows the differences as tracked changes (DOC-COMPARE-001).
            compare={async (bytes, name) => {
              await runCommand({ cmd: 'compareWith', name, base64: toBase64(bytes) });
              return null;
            }}
            onClose={() => {
              closeTaskPane(id);
              focusDoc();
            }}
          />
        ),
      });
    }
  }
  if (chartFormatOpen && state.doc) {
    paneDefs.push({
      id: 'chartFormat',
      title: t('chartFormat.title'),
      icon: <SlidersHorizontal size={14} aria-hidden />,
      closeTestId: 'chart-format-close',
      content: (
        <ChartFormatPane
          info={state.edit?.format.chart ? state.edit.chart?.elements : undefined}
          apply={(patch) => void runCommand({ cmd: 'setChart', elements: patch })}
        />
      ),
    });
  }
  if (ai.active && state.doc) {
    paneDefs.push({
      id: 'ai',
      title: t('ai.title'),
      icon: <Sparkles size={14} aria-hidden />,
      closeTestId: 'ai-close',
      content: ai.content,
    });
  }
  if (thesaurus.active && state.doc) {
    paneDefs.push({
      id: 'thesaurus',
      title: t('thesaurus.title'),
      icon: <BookA size={14} aria-hidden />,
      closeTestId: 'thesaurus-close',
      content: thesaurus.content,
    });
  }
  if (state.doc) paneDefs.push(...addins.panes);
  const insets = taskPaneInsets(paneLayout, paneDefs);
  const reviewingDock = state.doc ? state.reviewingPane : null;
  const sideLeft = insets.left + (reviewingDock === 'left' ? REVIEWING_SIZE.left : 0);
  const hostStyle =
    sideLeft || insets.right || reviewingDock === 'bottom'
      ? {
          ...(sideLeft ? { left: sideLeft } : {}),
          ...(insets.right ? { right: insets.right } : {}),
          ...(reviewingDock === 'bottom' ? { bottom: REVIEWING_SIZE.bottom } : {}),
        }
      : undefined;
  // Split view: the panes share the page area between the rulers and a bottom dock.
  const splitInsets = {
    top: rulersOn ? 22 : 0,
    bottom: reviewingDock === 'bottom' ? REVIEWING_SIZE.bottom : 0,
  };
  /** Edge of the split: 0 = top of the bar, 1 = bottom inset of the first pane, 2 = top of the second. */
  const splitEdge = (edge: 0 | 1 | 2): string => {
    const r = state.split ?? SPLIT_DEFAULT;
    const room = `(100% - ${splitInsets.top + splitInsets.bottom}px)`;
    return edge === 1
      ? `calc(${splitInsets.bottom}px + ${room} * ${1 - r} + 3px)`
      : `calc(${splitInsets.top}px + ${room} * ${r} ${edge === 0 ? '-' : '+'} 3px)`;
  };
  // Context menus (CORE-UI-003): commands of what is under the pointer, resolved from the
  // ribbon (so the contextual tab of the object the right click selected is there).
  const allRibbonTabs = [...tabs, ...contextTabs];
  const menuLookup = ribbonMenuLookup(
    allRibbonTabs,
    registryMenuLookup(
      registry,
      () => store.getState(),
      (id) => commandLabel(i18nInstance, id),
      (id) => void registry.execute(id, store.getState()),
    ),
  );
  const docMenuLines = (m: DocMenu): MenuLine[] => {
    const lines: MenuLine[] = [];
    const sp = m.spell;
    if (m.context === 'spelling' && sp) {
      if (sp.rule) lines.push({ heading: t(`grammar.${sp.rule}`), testId: 'grammar-rule' });
      if (sp.suggestions.length === 0) lines.push({ heading: t('spell.noSuggestions') });
      for (const word of sp.suggestions)
        lines.push({
          id: `spell.pick.${word}`,
          label: word,
          emphasis: true,
          onSelect: () => {
            void controller.current
              ?.run({
                cmd: 'setSelection',
                selection: {
                  anchor: { p: sp.para, o: sp.start },
                  focus: { p: sp.para, o: sp.end },
                },
              })
              .then(() => controller.current?.run({ cmd: 'insertText', text: word }));
          },
        });
      lines.push({ separator: true });
      lines.push({
        id: 'spell.ignoreAll',
        label: t('spell.ignoreAll'),
        onSelect: () =>
          void controller.current?.run(
            sp.rule
              ? { cmd: 'grammarIgnore', rule: sp.rule, text: sp.word }
              : { cmd: 'spellIgnore', word: sp.word },
          ),
      });
      if (!sp.rule)
        lines.push({
          id: 'spell.addToDictionary',
          label: t('spell.addToDictionary'),
          onSelect: () => {
            saveUserWord(sp.word);
            void controller.current?.run({ cmd: 'spellAddWord', word: sp.word });
          },
        });
      lines.push({ separator: true });
    }
    // Open / remove a link only on a link; on one, "Link" edits it.
    const onLink = !!store.getState().edit?.format.link;
    const resolved = resolveMenu(
      DOC_MENUS[m.context].filter(
        (id) => onLink || (id !== 'insert.openLink' && id !== 'insert.removeLink'),
      ),
      (id) => {
        const e = menuLookup(id);
        return e && id === 'insert.link' && onLink ? { ...e, label: t('docLink.edit') } : e;
      },
    );
    // Synonyms of the word at the caret (CORE-THES-001), for text and table text.
    const synonyms =
      m.context === 'text' || m.context === 'table'
        ? thesaurusMenuEntry(
            thesMenu.state,
            (key, o) => t(key, o ?? {}),
            (text, word) => {
              const c = controller.current;
              if (!c) return;
              void docReplaceWord((x) => c.run(x), word, text).then(() => view.current?.focus());
            },
            () => void registry.execute('review.thesaurus', store.getState()),
          )
        : null;
    if (!synonyms) return [...lines, ...resolved];
    // Synonyms come before translate / the link commands, as in word processors.
    const at = resolved.findIndex(
      (l) => 'id' in l && (l.id === 'review.translate' || l.id === 'insert.link'),
    );
    if (at < 0) return [...lines, ...resolved, { separator: true }, synonyms];
    return [...lines, ...resolved.slice(0, at), synonyms, ...resolved.slice(at)];
  };
  const docMenu = useContextMenu<DocMenu>(t('contextMenu.label'), docMenuLines, focusDoc);
  useEffect(() => {
    ribbonTabsRef.current = addins.applyTabs(allRibbonTabs);
  });
  const openDocMenu = docMenu.open;
  const prepareSynonyms = thesMenu.prepare;
  // The right click (DocumentView callback) and Shift+F10 reach the menu through refs.
  useEffect(() => {
    contextMenuAt.current = async (e) => {
      const c = controller.current;
      if (!c || !store.getState().doc || Date.now() - keyMenuAt.current < 600) return;
      setMini(null);
      const at = { x: e.clientX, y: e.clientY };
      const balloon =
        e.target instanceof Element
          ? e.target.closest<HTMLElement>('[data-testid="markup-balloon"][data-kind="comment"]')
          : null;
      if (balloon) {
        const id = Number(balloon.dataset.id);
        if (Number.isFinite(id)) await c.run({ cmd: 'selectComment', id });
        openDocMenu({ ...at, context: 'comment' });
        return;
      }
      const r = await c.query({ cmd: 'spellAt', page: e.page, x: e.x, y: e.y });
      if (r?.spell) {
        openDocMenu({ ...at, context: 'spelling', spell: r.spell, testId: 'spell-menu' });
        return;
      }
      // A right click outside the selection moves the caret there first.
      const inside = (store.getState().edit?.rects ?? []).some(
        (b) =>
          b.page === e.page && e.x >= b.x && e.x <= b.x + b.w && e.y >= b.y && e.y <= b.y + b.h,
      );
      if (!inside)
        await c.run({ cmd: 'click', page: e.page, x: e.x, y: e.y, extend: false, count: 1 });
      const w = await docWordAtCaret((x) => c.run(x));
      prepareSynonyms(w?.word);
      openDocMenu({ ...at, context: docMenuContext(store.getState()) });
    };
    menuAtCaret.current = () => {
      if (!store.getState().doc) return;
      const host = viewHost.current;
      const sels = host?.querySelectorAll('.bo-sel');
      const at =
        menuPointOf(host?.querySelector('.bo-caret')) ??
        menuPointOf(sels?.[sels.length - 1]) ??
        menuPointOf(host);
      if (!at) return;
      const c = controller.current;
      void (c ? docWordAtCaret((x) => c.run(x)) : Promise.resolve(null)).then((w) => {
        prepareSynonyms(w?.word);
        openDocMenu({ ...at, context: docMenuContext(store.getState()) });
      });
    };
  });

  // Mini toolbar (CORE-UI-003): the font group's commands near a selection made with the mouse.
  const miniIds = [
    'format.bold',
    'format.italic',
    'format.underline',
    'format.growStep',
    'format.shrinkStep',
    'format.bullets',
    'format.numbering',
    'format.painter',
  ];
  const homeItems = new Map<string, RibbonItem>(
    tabs.flatMap((tb) => tb.groups.flatMap((g) => g.items.map((i) => [i.id, i] as const))),
  );
  const miniItems: RibbonItem[] = mini
    ? [
        custom(
          'format.fontFamily',
          <FontFamilyPicker
            format={fs}
            fonts={state.fonts}
            run={runCommand}
            disabled={noDoc}
            onDone={focusDoc}
            testId="mini-font-family"
          />,
        ),
        custom(
          'format.fontSize',
          <FontSizeBox
            format={fs}
            run={runCommand}
            disabled={noDoc}
            onDone={focusDoc}
            testId="mini-font-size"
          />,
        ),
        ...miniIds.flatMap((id) => homeItems.get(id) ?? []),
        custom(
          'format.color',
          <ColorPicker
            kind="color"
            label={t('commands.format.color')}
            testId="mini-text-color"
            value={fs?.color}
            disabled={noDoc}
            onPick={(hex) => void runFocus({ cmd: 'formatText', format: { kind: 'color', hex } })}
          />,
        ),
        custom(
          'format.highlight',
          <ColorPicker
            kind="highlight"
            label={t('commands.format.highlight')}
            testId="mini-highlight-color"
            value={fs?.highlight}
            disabled={noDoc}
            onPick={(hex) =>
              void runFocus({ cmd: 'formatText', format: { kind: 'highlight', hex } })
            }
          />,
        ),
        custom(
          'mini.styles',
          <Popover
            label={t('miniToolbar.styles')}
            testId="mini-styles"
            disabled={noDoc}
            trigger={<span>{t('miniToolbar.styles')} ▾</span>}
          >
            {(close) => (
              <div role="menu" aria-label={t('miniToolbar.styles')}>
                {['style.normal', 'style.title', 'style.heading1', 'style.heading2'].flatMap(
                  (id) => {
                    const it = homeItems.get(id);
                    return it
                      ? [
                          <button
                            key={id}
                            type="button"
                            role="menuitem"
                            data-mini-command={id}
                            style={{ display: 'block', width: '100%', textAlign: 'left' }}
                            onClick={() => {
                              close();
                              setMini(null);
                              it.onSelect();
                            }}
                          >
                            {it.label}
                          </button>,
                        ]
                      : [];
                  },
                )}
              </div>
            )}
          </Popover>,
        ),
      ]
    : [];
  const goToPara = (para: number) =>
    void runFocus({
      cmd: 'setSelection',
      selection: { anchor: { p: para, o: 0 }, focus: { p: para, o: 0 } },
    });
  return (
    <div
      className={`bo-root ${styles.root}`}
      data-theme={resolvedTheme}
      data-app={app}
      lang={state.language}
    >
      <TitleBar
        app={app}
        appName={host?.brandName ? `${host.brandName} · ${appName}` : appName}
        docName={state.doc?.name ?? ''}
        saveState={state.doc ? (state.edit?.modified ? 'unsaved' : 'saved') : null}
        saveLabel={t(state.edit?.modified ? 'titleBar.unsaved' : 'titleBar.saved')}
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
                [...tabs, ...contextTabs],
                scriptBindings.quickAccess.map((sc) => ({
                  id: `script.run.${sc.id}`,
                  icon: 'script.run',
                  label: t('script.runBound', { name: sc.name }),
                  shortcut: sc.shortcut,
                  enabled: !!state.doc,
                  onSelect: () => void registry.execute(`script.run.${sc.id}`, store.getState()),
                })),
              )
        }
        actions={
          <>
            {host?.showShare && (
              <button type="button" className={styles.hostButton} onClick={host.onShare}>
                <Share2 size={14} /> {t('host.share')}
              </button>
            )}
            {host?.showClose && (
              <button
                type="button"
                className={styles.hostButton}
                onClick={host.onClose}
                aria-label={t('host.close')}
              >
                <X size={16} />
              </button>
            )}
          </>
        }
      />
      {app === 'doc' ? (
        <>
          <CommandSearch
            tabs={[...tabs, ...contextTabs]}
            open={searchOpen}
            onClose={() => {
              setSearchOpen(false);
              view.current?.focus();
            }}
            labels={{
              title: t('commandSearch.title'),
              placeholder: t('commandSearch.placeholder'),
              noResults: t('commandSearch.noResults'),
            }}
          />
          {!(state.focusMode && state.doc) && (
            <Ribbon
              // File commands live on the File screen (CORE-UI-002); search still finds them.
              tabs={addins.applyTabs([...tabs.filter((x) => x.id !== 'file'), ...contextTabs])}
              // A contextual tab whose object is no longer selected falls back in the ribbon.
              value={tab}
              onValueChange={setTab}
              labels={ribbonLabels(t)}
              autoActivate={AUTO_CONTEXTS}
              fileMenu={{
                label: t('ribbon.tabs.file'),
                open: backstage,
                onOpen: () => setBackstage(true),
              }}
              {...ribbonCustom.ribbonProps()}
            />
          )}
          {ribbonCustom.below && !(state.focusMode && state.doc) && (
            <QuickAccessBar
              below
              label={ribbonCustom.qatLabel}
              items={ribbonCustom.quickAccess([...tabs, ...contextTabs])}
            />
          )}
          <main
            ref={workspace}
            className={styles.workspace}
            data-view={state.viewMode}
            data-focus={state.focusMode ? 'true' : undefined}
          >
            {rulersOn && (
              <Rulers
                box={pageBox}
                format={fs}
                run={runFocus}
                language={state.language}
                disabled={noDoc}
              />
            )}
            <div
              ref={viewHost}
              className={`${styles.viewHost} ${rulersOn ? styles.withRulers : ''}`}
              style={splitOn ? { ...hostStyle, bottom: splitEdge(1) } : hostStyle}
              data-testid="document-view"
              data-marks={state.showMarks ? 'true' : undefined}
            />
            {splitOn && (
              <>
                <div
                  role="separator"
                  aria-orientation="horizontal"
                  aria-label={t('windows.splitBar')}
                  aria-valuemin={Math.round(SPLIT_MIN * 100)}
                  aria-valuemax={Math.round((1 - SPLIT_MIN) * 100)}
                  aria-valuenow={Math.round((state.split ?? SPLIT_DEFAULT) * 100)}
                  tabIndex={0}
                  data-testid="split-bar"
                  className={viewStyles.splitBar}
                  style={{
                    left: hostStyle?.left ?? 0,
                    right: hostStyle?.right ?? 0,
                    top: splitEdge(0),
                  }}
                  onDoubleClick={() => store.setState({ split: null })}
                  onKeyDown={(e) => {
                    const step = e.key === 'ArrowUp' ? -0.05 : e.key === 'ArrowDown' ? 0.05 : 0;
                    if (!step) return;
                    e.preventDefault();
                    e.stopPropagation();
                    store.setState({ split: clampSplit((state.split ?? SPLIT_DEFAULT) + step) });
                  }}
                  onPointerDown={(e) => {
                    e.preventDefault();
                    e.currentTarget.setPointerCapture(e.pointerId);
                  }}
                  onPointerMove={(e) => {
                    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
                    const box = workspace.current?.getBoundingClientRect();
                    if (!box) return;
                    const top = box.top + splitInsets.top;
                    const total = box.height - splitInsets.top - splitInsets.bottom;
                    store.setState({ split: dragSplit(total, e.clientY - top) });
                  }}
                />
                <div
                  ref={splitHost}
                  className={`${styles.viewHost} ${rulersOn ? styles.withRulers : ''}`}
                  style={{ ...hostStyle, top: splitEdge(2) }}
                  data-testid="document-view-split"
                />
              </>
            )}
            <TaskPaneHost
              layout={paneLayout}
              onLayout={setPaneLayout}
              panes={paneDefs}
              onClose={closeTaskPane}
              labels={taskPaneLabels(t)}
              focusApp={focusDoc}
            />
            {state.viewMode === 'outline' && state.doc && (
              <div
                className={viewStyles.outlineHost}
                style={{ left: insets.left, right: insets.right }}
              >
                <OutlineView
                  items={state.outline}
                  caretPara={state.edit?.selection.focus.p}
                  run={runCommand}
                  onCommand={(id) => void registry.execute(id, store.getState())}
                  onClose={(para) => {
                    store.setState({ viewMode: 'print' });
                    if (para !== undefined) goToPara(para);
                    else view.current?.focus();
                  }}
                />
              </div>
            )}
            {state.viewMode === 'read' && state.doc && (
              <ReadMode
                name={state.doc.name}
                pages={state.doc.pages}
                start={state.page}
                renderPage={renderThumb}
                onExit={(page) => {
                  const back = beforeRead.current;
                  store.setState({ viewMode: back === 'read' ? 'print' : back });
                  requestAnimationFrame(() => view.current?.scrollToPage(page));
                  view.current?.focus();
                }}
              />
            )}
            {state.focusedReader && state.doc && (
              <FocusedReader
                name={state.doc.name}
                blocks={state.focusedReader.blocks}
                start={state.focusedReader.start}
                speech={platform.speech}
                settings={platform.shell}
                highContrast={resolvedTheme === 'hc'}
                onExit={() => {
                  store.setState({ focusedReader: null });
                  view.current?.focus();
                }}
              />
            )}
            {state.focusMode && state.doc && (
              <button
                type="button"
                className={viewStyles.focusExit}
                data-testid="focus-exit"
                onClick={() => {
                  store.setState({ focusMode: false });
                  view.current?.focus();
                }}
              >
                {t('views.focus.exit')}
              </button>
            )}
            {state.reviewingPane && state.doc && (
              <ReviewingPane
                review={state.review}
                run={runCommand}
                language={state.language}
                dock={state.reviewingPane}
                style={
                  state.reviewingPane === 'left'
                    ? { left: insets.left }
                    : { left: insets.left, right: insets.right }
                }
                onDock={(reviewingPane) => store.setState({ reviewingPane })}
                onClose={() => store.setState({ reviewingPane: null })}
              />
            )}
            {(state.edit?.story?.kind === 'header' || state.edit?.story?.kind === 'footer') && (
              <HeaderFooterBar story={state.edit.story} run={runCommand} />
            )}
            {state.edit?.control && <ControlBar control={state.edit.control} run={runCommand} />}
            {state.edit?.formField && (
              <FormFieldBar
                info={state.edit.formField}
                protection={state.edit.protection ?? null}
                run={runCommand}
              />
            )}
            {state.panel === 'xmlMapping' && state.doc && (
              <XmlMappingPane
                run={runCommand}
                control={state.edit?.control ?? null}
                pickFile={async () => (await platform.files.openFile(['.xml']))?.bytes ?? null}
                onClose={closePanel}
              />
            )}
            {state.panel === 'notes' && (
              <NotesPanel
                run={runCommand}
                onClose={closePanel}
                section={state.edit?.format.section}
              />
            )}
            {state.dialog === 'pageSetup' && (
              <PageSetupDialog
                format={fs}
                run={runCommand}
                language={state.language}
                onClose={closeDialog}
              />
            )}
            {picture.dialogs}
            {objects.dialogs}
            {state.dialog === 'picturePosition' && (
              <PicturePositionDialog
                format={fs}
                run={runCommand}
                language={state.language}
                onClose={closeDialog}
              />
            )}
            {state.dialog === 'tableProperties' && (
              <TablePropertiesDialog
                format={fs}
                run={runCommand}
                language={state.language}
                onClose={closeDialog}
              />
            )}
            {state.dialog === 'tableSort' && (
              <SortDialog table={fs?.table} run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'tableFormula' && (
              <FormulaDialog table={fs?.table} run={runCommand} onClose={closeDialog} />
            )}
            {(state.dialog === 'textToTable' || state.dialog === 'tableToText') && (
              <ConvertDialog
                toTable={state.dialog === 'textToTable'}
                run={runCommand}
                onClose={closeDialog}
              />
            )}
            {chartDialog && (
              <ChartDialog
                open
                mode={chartDialog.mode}
                initial={chartDialog.initial}
                onClose={() => {
                  setChartDialog(null);
                  focusDoc();
                }}
                onApply={(d) => {
                  const mode = chartDialog.mode;
                  setChartDialog(null);
                  void runCommand(chartCommand(mode, d, t)).then((r) => {
                    if (r?.error) fail('errors.chart', r.error);
                    focusDoc();
                  });
                }}
              />
            )}
            {chartTemplateOpen && (
              <SaveChartTemplateDialog
                load={chartGallery}
                onClose={() => setChartTemplateOpen(false)}
                onSaved={(name) =>
                  store.setState({
                    status: { kind: 'info', key: 'chartDesign.templateSaved', params: { name } },
                  })
                }
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
                  focusDoc();
                }}
                onApply={(d) => {
                  const mode = smartArtDialog.mode;
                  setSmartArtDialog(null);
                  void runCommand(
                    mode === 'insert'
                      ? { cmd: 'insertSmartArt', ...d }
                      : { cmd: 'setSmartArt', ...d },
                  ).then((r) => {
                    if (r?.error) fail('errors.smartArt', r.error);
                    focusDoc();
                  });
                }}
              />
            )}
            {state.dialog === 'equation' && (
              <EquationDialog
                initial={state.edit?.equation ?? null}
                templates={equationTemplates}
                onClose={closeDialog}
                onInsert={(text, latex, display, numbered) => {
                  const replace = !!store.getState().edit?.equation;
                  closeDialog();
                  void runFocus({
                    cmd: 'insertEquation',
                    text,
                    latex,
                    display,
                    replace,
                    numbered: numbered && !replace ? t('equation.numberLabel') : undefined,
                  });
                }}
              />
            )}
            {state.dialog === 'autoCorrect' && (
              <AutoCorrectDialog run={runCommand} onClose={closeDialog} />
            )}
            {macroViewer.open && (
              <MacroViewer
                load={loadMacros}
                onConvert={macroViewer.convert}
                onClose={macroViewer.close}
              />
            )}
            {printOpen && platform.engine.printPdf && platform.engine.printPreview && (
              <PrintPanel
                app="doc"
                host={{
                  printPdf: platform.engine.printPdf.bind(platform.engine),
                  printPreview: platform.engine.printPreview.bind(platform.engine),
                  getGlyphOutlines: platform.engine.getGlyphOutlines.bind(platform.engine),
                  getMedia: platform.engine.getMedia.bind(platform.engine),
                }}
                initial={{ current: state.edit?.caret?.page ?? state.page }}
                onPrint={(pdf) => platform.print.printPdf(pdf, state.doc?.name ?? '')}
                onClose={() => setPrintOpen(false)}
                fail={(e) => fail('errors.export', e)}
              />
            )}
            {state.dialog === 'checkDocument' && (
              <DocCheckDocumentDialog run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'formField' && state.edit?.formField && (
              <FormFieldDialog
                info={state.edit.formField}
                onClose={closeDialog}
                onApply={(cmd) => {
                  closeDialog();
                  void runFocus(cmd);
                }}
              />
            )}
            {state.dialog === 'lockTracking' && (
              <LockTrackingDialog
                review={state.review}
                onClose={closeDialog}
                onApply={(cmd) => {
                  void runCommand(cmd).then((r) => {
                    if (r && !r.error) {
                      closeDialog();
                      focusDoc();
                    }
                  });
                }}
              />
            )}
            {state.dialog === 'protectForms' && (
              <ProtectFormsDialog
                protection={state.edit?.protection ?? null}
                sections={state.edit?.format.section?.count ?? 1}
                onClose={closeDialog}
                onApply={(cmd) => {
                  void runCommand(cmd).then((r) => {
                    if (r && !r.error) {
                      closeDialog();
                      focusDoc();
                    }
                  });
                }}
              />
            )}
            {state.dialog === 'spellCheck' && (
              <SpellCheckDialog
                run={runCommand}
                onClose={closeDialog}
                onShow={(i) => {
                  // Select a word of the main text so it shows behind the dialog.
                  if (i.story.kind === 'body')
                    void controller.current?.run({
                      cmd: 'setSelection',
                      selection: {
                        anchor: { p: i.para, o: i.start },
                        focus: { p: i.para, o: i.end },
                      },
                    });
                }}
              />
            )}
            {state.dialog === 'wordCount' && (
              <WordCountDialog query={queryDoc} onClose={closeDialog} />
            )}
            {state.dialog === 'language' && (
              <LanguageDialog
                run={runCommand}
                lang={fs?.lang ?? null}
                noProof={fs?.noProof ?? false}
                onClose={closeDialog}
              />
            )}
            {state.dialog === 'adminFormat' && (
              <AdminFormatDialog run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'control' && (
              <ControlDialog
                onClose={closeDialog}
                onInsert={(cmd) => {
                  closeDialog();
                  void runFocus(cmd);
                }}
              />
            )}
            {state.dialog === 'field' && (
              <FieldDialog
                onClose={closeDialog}
                onInsert={(instr) => {
                  closeDialog();
                  void runFocus({ cmd: 'insertField', instr });
                }}
              />
            )}
            {state.dialog === 'link' && (
              <LinkDialog
                current={state.edit?.format.link}
                onClose={closeDialog}
                onApply={(target, display) => {
                  closeDialog();
                  void runFocus({ cmd: 'setLink', target, display });
                }}
                onRemove={() => {
                  closeDialog();
                  void runFocus({ cmd: 'setLink', target: '' });
                }}
              />
            )}
            {state.dialog === 'pasteSpecial' && (
              <PasteSpecialDialog
                available={pasteAvail}
                onClose={closeDialog}
                onPaste={(mode: DocPasteMode) => {
                  const c = pasteClip.current;
                  closeDialog();
                  if (c) void runFocus({ cmd: 'paste', ...c, mode });
                }}
              />
            )}
            {state.dialog === 'defineList' && (
              <DefineListDialog
                onClose={closeDialog}
                onApply={(levels, apply) => {
                  closeDialog();
                  void runFocus({ cmd: 'defineList', levels, apply });
                }}
              />
            )}
            {state.dialog === 'setNumbering' && (
              <SetNumberingDialog run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'columns' && (
              <ColumnsDialog run={runCommand} language={state.language} onClose={closeDialog} />
            )}
            {state.dialog === 'lineNumbers' && (
              <LineNumbersDialog run={runCommand} language={state.language} onClose={closeDialog} />
            )}
            {state.dialog === 'pdfExport' && (
              <PdfExportDialog
                app="doc"
                onClose={closeDialog}
                onExport={(c) => exportPdfWith.current?.(c)}
              />
            )}
            {state.dialog === 'hyphenation' && (
              <HyphenationDialog run={runCommand} language={state.language} onClose={closeDialog} />
            )}
            {state.dialog === 'textDirection' && (
              <TextDirectionDialog run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'dropCap' && (
              <DropCapDialog run={runCommand} language={state.language} onClose={closeDialog} />
            )}
            {state.dialog === 'fontEffects' && (
              <FontEffectsDialog run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'phoneticGuide' && (
              <PhoneticGuideDialog run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'combineChars' && (
              <CombineCharsDialog run={runCommand} onClose={closeDialog} />
            )}
            {isReferenceDialog(state.dialog) && (
              <ReferenceDialog
                which={state.dialog}
                run={runCommand}
                language={state.language}
                onClose={closeDialog}
              />
            )}
            {isCitationDialog(state.dialog) && (
              <CitationDialog
                which={state.dialog}
                run={runCommand}
                settings={platform.shell}
                files={platform.files}
                language={state.language}
                onClose={closeDialog}
              />
            )}
            {state.dialog === 'pageBorders' && (
              <PageBordersDialog run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'signatureLine' && (
              <InsertSignatureLineDialog
                onClose={closeDialog}
                onInsert={(spec) => {
                  closeDialog();
                  void runFocus({ cmd: 'insertSignatureLine', spec, id: newLineId() });
                }}
              />
            )}
            {state.dialog === 'signLine' && (
              <SignLineDialog
                run={runCommand}
                pickPicture={async () => {
                  const f = await platform.files.openFile(['.png', '.jpg', '.jpeg']);
                  return f ? { name: f.name, bytes: f.bytes } : null;
                }}
                onClose={closeDialog}
                onContinue={(choice) => signLineRef.current?.(choice)}
              />
            )}
            {state.dialog === 'saveBlock' && (
              <SaveBlockDialog run={runCommand} query={queryCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'blocksOrganizer' && (
              <BlocksOrganizer
                run={runCommand}
                query={queryCommand}
                page={blockPage}
                onClose={closeDialog}
              />
            )}
            {state.dialog === 'watermark' && (
              <WatermarkDialog
                run={runCommand}
                onClose={closeDialog}
                pickPicture={async () => {
                  const f = await platform.files.openFile(['.png', '.jpg', '.jpeg']);
                  return f ? { base64: bytesToBase64(f.bytes), name: f.name } : null;
                }}
              />
            )}
            {state.dialog === 'templates' && (
              <TemplateDialog
                onClose={closeDialog}
                onCreate={(id, name) => {
                  closeDialog();
                  void (async () => {
                    try {
                      await platform.engine.init();
                      if (!store.getState().doc) await platform.engine.newDocument(name);
                      const r = await controller.current?.run({ cmd: 'newFromTemplate', id, name });
                      if (r && !r.error) {
                        stored.current = null;
                        await loaded(r.info);
                      }
                    } catch (e) {
                      fail('errors.open', e);
                    }
                  })();
                }}
              />
            )}
            {state.dialog === 'mergeRecipients' && (
              <RecipientsDialog merge={state.merge} run={runCommand} onClose={closeDialog} />
            )}
            {state.dialog === 'mergeRule' && (
              <RuleDialog
                kind={ruleKind}
                fields={state.merge?.fields ?? []}
                run={runCommand}
                onClose={closeDialog}
              />
            )}
            {state.dialog === 'mergeProvider' && platform.mergeData && (
              <ProviderDialog
                provider={platform.mergeData}
                run={runCommand}
                onLoaded={(res) => {
                  if (res.merge) store.setState({ merge: res.merge, panel: 'merge' });
                }}
                onClose={closeDialog}
              />
            )}
            {(state.dialog === 'mergeEnvelope' || state.dialog === 'mergeLabels') &&
              (() => {
                const Comp = state.dialog === 'mergeEnvelope' ? EnvelopeDialog : LabelsDialog;
                return (
                  <Comp
                    fields={state.merge?.fields ?? []}
                    run={runCommand}
                    onDone={async (res) => {
                      stored.current = null;
                      await loaded(res.info);
                    }}
                    onClose={closeDialog}
                  />
                );
              })()}
            {state.dialog === 'mergeFinish' && (
              <FinishDialog
                mode={finishMode}
                merge={state.merge}
                run={runCommand}
                host={{
                  onMerged: async (res) => {
                    stored.current = null;
                    await loaded(res.info);
                  },
                  saveZip: (bytes, name) => platform.files.saveFile(bytes, name, 'application/zip'),
                  mail: platform.mail,
                }}
                onClose={closeDialog}
              />
            )}
            {(state.panel === 'find' || state.panel === 'replace') && (
              <FindPanel
                run={runCommand}
                replace={state.panel === 'replace'}
                onClose={closePanel}
                onClearHighlights={() => view.current?.setFindHighlights([])}
              />
            )}
            {state.panel === 'review' && (
              <ReviewPanel
                review={state.review}
                run={runCommand}
                language={state.language}
                onClose={closePanel}
              />
            )}
            {state.panel === 'caption' && <CaptionPanel run={runCommand} onClose={closePanel} />}
            {state.panel === 'savePassword' && (
              <SavePasswordPanel
                onSet={(password) => void runCommand({ cmd: 'setSavePassword', password })}
                onClose={closePanel}
              />
            )}
            {(state.panel === 'sign' || state.panel === 'signPdf') &&
              platform.signing &&
              signSetup && (
                <SignDialog
                  kinds={platform.signing.kinds}
                  defaults={signSetup.defaults}
                  remembered={signSetup.remembered}
                  output={state.panel === 'signPdf' ? 'pdf' : 'docx'}
                  documentName={signedFileName(
                    state.doc?.name ?? 'document',
                    state.panel === 'signPdf' ? 'pdf' : 'docx',
                  )}
                  pickFile={async () => {
                    const f = await platform.files.openFile(['.p12', '.pfx', '.pem']);
                    return f ? { name: f.name, bytes: f.bytes } : null;
                  }}
                  listTokenCertificates={platform.signing.listTokenCertificates?.bind(
                    platform.signing,
                  )}
                  onSign={(request, form) => void signNow(request, form)}
                  onClose={closePanel}
                />
              )}
            {state.panel === 'signatures' && (
              <SignaturesPanel report={state.signatures} onClose={closePanel} />
            )}
            {state.signatures && state.panel !== 'signatures' && (
              <div
                className={styles.macroBar}
                role="status"
                data-testid="signature-bar"
                data-state={documentState(state.signatures)}
              >
                <span>{t(`sign.bar.${documentState(state.signatures)}`)}</span>
                <button type="button" onClick={() => store.setState({ panel: 'signatures' })}>
                  {t('sign.details')}
                </button>
              </div>
            )}
            {state.passwordPrompt && (
              <PasswordDialog
                name={state.passwordPrompt.file.name}
                wrong={state.passwordPrompt.wrong}
                onSubmit={(pw) => {
                  const p = store.getState().passwordPrompt;
                  store.setState({ passwordPrompt: null });
                  if (p) void openFile(p.file, p.doc ?? undefined, pw);
                }}
                onCancel={() => store.setState({ passwordPrompt: null })}
              />
            )}
            {docMenu.element}
            {mini && miniItems.length > 0 && !docMenu.menu && (
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
            {addins.element}
            {state.legacyNotice > 0 && (
              <div
                className={styles.macroBar}
                style={{ top: state.macroWarning ? 34 : 0 }}
                role="status"
                data-testid="legacy-notice"
              >
                <span>{t('legacy.converted', { count: state.legacyNotice })}</span>
                <button
                  type="button"
                  onClick={() => {
                    store.setState({ legacyNotice: 0 });
                    void registry.execute('edit.undo', store.getState());
                  }}
                >
                  {t('legacy.undo')}
                </button>
                <button type="button" onClick={() => store.setState({ legacyNotice: 0 })}>
                  {t('security.dismiss')}
                </button>
              </div>
            )}
            {state.security?.readOnly && (
              <ProtectionBar
                info={state.security}
                onEdit={() => {
                  const s = state.security;
                  if (s?.markedFinal) void runCommand({ cmd: 'markFinal', on: false });
                  else if (s?.writePassword) store.setState({ protectUi: 'prompt' });
                  else void runCommand({ cmd: 'setReadOnly', on: false });
                }}
              />
            )}
            {state.protectUi === 'prompt' && state.security && (
              <OpenProtectionDialog
                info={state.security}
                name={state.doc?.name ?? ''}
                run={runCommand}
                onDone={() => store.setState({ protectUi: null })}
              />
            )}
            {state.protectUi === 'panel' && state.security && (
              <ProtectDialog
                app="doc"
                info={state.security}
                run={runCommand}
                onClose={() => {
                  store.setState({ protectUi: null });
                  view.current?.focus();
                }}
              />
            )}
            <SaveAsTypeDialog
              open={saveAsType}
              app="doc"
              current={state.doc?.variant}
              macros={state.doc?.macros}
              onCancel={() => setSaveAsType(false)}
              onConfirm={(ext) => {
                setSaveAsType(false);
                saveAsExt.current = ext;
                void registry.execute('file.saveAsType', store.getState());
              }}
            />
            {state.macroWarning && (
              <div className={styles.macroBar} role="status" data-testid="macro-warning">
                <span>{t('security.macroWarning')}</span>
                {state.doc?.vbaSignatureDropped && (
                  <span data-testid="macro-signature-dropped">
                    {t('security.macroSignatureDropped')}
                  </span>
                )}
                <button type="button" onClick={showMacros} data-testid="macro-warning-view">
                  {t('security.viewMacros')}
                </button>
                <button
                  type="button"
                  onClick={() => store.setState({ macroWarning: false })}
                  data-testid="macro-warning-dismiss"
                >
                  {t('security.dismiss')}
                </button>
              </div>
            )}
            {state.panel === 'merge' && (
              <MergeFieldPanel merge={state.merge} run={runCommand} onClose={closePanel} />
            )}
            {state.panel === 'crossref' && <CrossRefPanel run={runCommand} onClose={closePanel} />}
            <DocClipboardUi
              controller={controller}
              ready={state.engineReady}
              view={view}
              store={store}
              panel={clipPanel}
              onOpenPanel={() => setClipPanel(true)}
              onClosePanel={() => setClipPanel(false)}
            />
            {state.panel === 'symbols' && (
              <SymbolPanel
                fonts={state.fonts.map((f) => f.name)}
                font={state.edit?.format?.font ?? undefined}
                onInsert={(sym) => {
                  void controller.current?.run({ cmd: 'insertText', text: sym });
                }}
                onClose={closePanel}
              />
            )}
            {state.panel === 'icons' && (
              <IconPanel
                onInsert={async (icons) => {
                  const c = controller.current;
                  if (!c) return;
                  try {
                    // One operation for a save: the placeholder is swapped before saving.
                    await c.hold(insertDocIcons(c, icons));
                  } catch (e) {
                    fail('errors.image', e);
                  }
                }}
                onClose={closePanel}
              />
            )}
            {state.recovery && (
              <div className={styles.recovery} role="alert" data-testid="recovery">
                <span>{t('recovery.title', { name: state.recovery })}</span>
                <button type="button" onClick={() => void restore()}>
                  {t('recovery.restore')}
                </button>
                <button type="button" onClick={() => void discardRecovery()}>
                  {t('recovery.discard')}
                </button>
              </div>
            )}
            {!state.doc && (
              <div className={styles.start}>
                <div className={styles.card}>
                  <h1>{t('start.title')}</h1>
                  <p>{t('start.subtitle')}</p>
                  <div className={styles.actions}>
                    <button
                      type="button"
                      className={styles.primary}
                      data-testid="new-document"
                      onClick={() => void registry.execute('file.new', state)}
                    >
                      <FilePlus size={18} /> {t('start.blank')}
                    </button>
                    <button
                      type="button"
                      className={styles.secondary}
                      data-testid="new-from-template"
                      onClick={() => void registry.execute('file.newFromTemplate', state)}
                    >
                      <LayoutTemplate size={18} /> {t('start.template')}
                    </button>
                    <button
                      type="button"
                      className={styles.secondary}
                      onClick={() => void registry.execute('file.open', state)}
                    >
                      <FolderOpen size={18} /> {t('start.open')}
                    </button>
                    {sampleUrl && (
                      <button
                        type="button"
                        className={styles.secondary}
                        data-testid="open-sample"
                        onClick={() => void registry.execute('file.openSample', state)}
                      >
                        <FileText size={18} /> {t('start.sample')}
                      </button>
                    )}
                  </div>
                  <div className={styles.recent}>
                    {t('start.recent')}
                    <ul>
                      {recent.length === 0 ? (
                        <li>{t('start.noRecent')}</li>
                      ) : (
                        recent.slice(0, 5).map((r) => <li key={r.path}>{r.name}</li>)
                      )}
                    </ul>
                  </div>
                </div>
              </div>
            )}
            {status.kind === 'error' && (
              <div
                className={styles.error}
                role="alert"
                onClick={() => store.setState({ status: { kind: 'idle' } })}
              >
                {t(status.key, { message: status.message })}
              </div>
            )}
          </main>
        </>
      ) : (
        <main className={styles.workspace}>
          <div className={styles.start}>
            <div className={styles.card}>
              <h1>{appName}</h1>
              <p>{t(`placeholder.${app}`)}</p>
            </div>
          </div>
        </main>
      )}
      {!(state.focusMode && state.doc) && (
        <StatusBar
          customize={
            state.doc
              ? {
                  label: t('statusBar.customize'),
                  items: STATUS_ITEMS.doc.map((d) => ({
                    id: d.id,
                    label: t(`statusBar.items.${d.id}`),
                    checked: statusBar.shown(d.id),
                    ...(statusValues[d.id] !== undefined ? { value: statusValues[d.id] } : {}),
                  })),
                  onToggle: statusBar.toggle,
                  resetLabel: t('statusBar.reset'),
                  onReset: statusBar.reset,
                }
              : undefined
          }
          left={
            <>
              {(!state.doc || statusBar.shown('page')) && (
                <span data-testid="status-page">
                  {state.doc
                    ? t('status.page', { current: state.page + 1, total: state.doc.pageCount })
                    : t('status.ready')}
                </span>
              )}
              {state.doc && statusBar.shown('words') && (
                <button
                  type="button"
                  className={styles.statusButton}
                  data-testid="status-words"
                  title={t('wordCount.title')}
                  onClick={() => void registry.execute('review.wordCount', state)}
                >
                  {statusValues.words}
                </button>
              )}
              {state.doc && statusBar.shown('proofing') && state.proofErrors !== null && (
                <button
                  type="button"
                  className={styles.statusButton}
                  data-testid="status-proofing"
                  data-errors={state.proofErrors}
                  title={statusValues.proofing}
                  onClick={() => void registry.execute('review.spellingDialog', state)}
                >
                  {statusValues.proofing}
                </button>
              )}
              {state.doc && statusBar.shown('language') && (
                <button
                  type="button"
                  className={styles.statusButton}
                  data-testid="status-language"
                  aria-label={t('spell.statusLanguage', { lang: languageLabel })}
                  title={t('spell.statusLanguage', { lang: languageLabel })}
                  onClick={() => void registry.execute('review.language', state)}
                >
                  {languageLabel}
                </button>
              )}
              {state.doc && statusBar.shown('trackChanges') && (
                <button
                  type="button"
                  className={styles.statusButton}
                  data-testid="status-track-changes"
                  aria-pressed={!!state.review?.trackChanges}
                  onClick={() => void registry.execute('review.trackChanges', state)}
                >
                  {statusValues.trackChanges}
                </button>
              )}
              {state.doc && statusBar.shown('capsLock') && capsLock && (
                <span data-testid="status-caps-lock">{t('statusBar.capsLock')}</span>
              )}
              {state.doc && statusBar.shown('overtype') && (
                <button
                  type="button"
                  className={styles.statusButton}
                  data-testid="status-overtype"
                  aria-pressed={state.overtype}
                  onClick={() => store.setState({ overtype: !state.overtype })}
                >
                  {statusValues.overtype}
                </button>
              )}
              {state.edit?.modified && (
                <span data-testid="status-modified">{t('status.modified')}</span>
              )}
              {state.doc?.protected && (
                <span data-testid="status-protected">{t('security.protected')}</span>
              )}
              {state.doc && <ProtectionStatus info={state.security} />}
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
              {status.kind === 'busy' && <span>{t(status.key)}</span>}
              {status.kind === 'info' && (
                <span data-testid="status-info">{t(status.key, status.params)}</span>
              )}
            </>
          }
          right={
            <>
              <span data-testid="status-engine">
                {t('status.engine', {
                  backend: platform.engine.backend === 'wasm' ? 'WASM' : 'native',
                })}
              </span>
              {state.doc && statusBar.shown('viewButtons') && (
                <StatusButtons
                  label={t('statusBar.views')}
                  testId="status-views"
                  exclusive
                  buttons={(
                    [
                      ['read', 'view.readMode', <BookOpen key="i" size={14} />],
                      ['print', 'view.printLayout', <FileText key="i" size={14} />],
                      ['web', 'view.webLayout', <Globe key="i" size={14} />],
                    ] as const
                  ).map(([mode, id, icon]) => ({
                    id: mode,
                    label: t(`commands.${id}`),
                    icon,
                    pressed: state.viewMode === mode,
                    onClick: () => void registry.execute(id, state),
                  }))}
                />
              )}
              {(statusBar.shown('zoomSlider') || statusBar.shown('zoom')) && (
                <StatusZoom
                  zoom={state.zoom}
                  min={MIN_ZOOM}
                  max={MAX_ZOOM}
                  slider={statusBar.shown('zoomSlider')}
                  percent={statusBar.shown('zoom')}
                  onZoom={(z: number) => activeView()?.setZoom(z)}
                  onStep={(d: 1 | -1) =>
                    void registry.execute(d > 0 ? 'view.zoomIn' : 'view.zoomOut', state)
                  }
                  onPercent={() => void registry.execute('view.zoomReset', state)}
                  labels={{
                    slider: t('statusBar.zoomSlider'),
                    zoomIn: t('commands.view.zoomIn'),
                    zoomOut: t('commands.view.zoomOut'),
                    percent: t('statusBar.zoomPercent', { value: Math.round(state.zoom * 100) }),
                  }}
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
      <FileBackstage
        app="doc"
        open={backstage}
        onClose={closeBackstage}
        commands={backstageCommands}
        hasDocument={!!state.doc}
        fileName={state.doc?.name}
        variant={state.doc?.variant}
        macros={state.doc?.macros}
        properties={docProperties}
        shell={platform.shell}
        openPath={
          platform.files.openPath
            ? (path) =>
                void platform.files.openPath?.(path).then(
                  (f) => openFileRef.current(f),
                  (e: unknown) => fail('errors.open', e),
                )
            : undefined
        }
        onSaveAs={(ext) => {
          saveAsExt.current = ext;
          void registry.execute('file.saveAsType', store.getState());
        }}
        onShare={host?.showShare ? host.onShare : undefined}
        onCloseDocument={host?.showClose ? host.onClose : undefined}
      />
    </div>
  );
}
