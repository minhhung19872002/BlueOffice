import { DocumentView } from '@blueoffice/canvas-host';
import { SpellMenu, loadIgnoredWords, loadUserWords, saveUserWord } from './SpellMenu';
import {
  LanguageDialog,
  PROOF_LANGUAGES,
  SpellCheckDialog,
  loadProofOptions,
} from './SpellCheckDialog';
import { AutoCorrectDialog, autoCorrectCommand, loadAutoCorrect } from './AutoCorrectDialog';
import { DocCheckDocumentDialog } from './DocCheckDocument';
import { createI18n, I18nextProvider, type Language } from '@blueoffice/i18n';
import type {
  CharFormat,
  EditCommand,
  OpenedFile,
  ParaFormat,
  DocPasteMode,
  Platform,
  RecentFile,
  SigningDefaults,
  SignRequest,
  StoredDocument,
  TableEdit,
  ViewLayoutMode,
} from '@blueoffice/platform';
import {
  PASSWORD_REQUIRED,
  THEME_MIME,
  WRONG_PASSWORD,
  openExtensions,
  openProtection,
  updateNotice,
} from '@blueoffice/platform';
import {
  Ribbon,
  CommandSearch,
  StatusBar,
  TitleBar,
  useCommandSearchShortcut,
  nextDensity,
  nextTheme,
  setDensity,
  getDensity,
  useDensity,
  useResolvedTheme,
  usePictureTools,
  PictureActionsHub,
  type RibbonTab,
  type ThemeSetting,
} from '@blueoffice/ui';
import { pictureTabLabels, registerPictureCommands } from './pictureTools';
import {
  AArrowDown,
  AArrowUp,
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
  Heading1,
  Heading2,
  Image,
  Italic,
  KeyRound,
  Languages,
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
  Globe,
  ScrollText,
  Focus,
  PenLine,
  ArrowLeftRight,
  Bookmark,
  Scale,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { CommandRegistry } from './commands';
import { DocController } from './controller';
import styles from './Editor.module.css';
import { FindPanel } from './Panels';
import { SymbolPanel } from './SymbolPanel';
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
import { fromBase64 } from './clipboard/docPaste';
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
import { createEditorStore, type EditorState } from './store';
import {
  HeaderFooterBar,
  NotesPanel,
  PageSetupDialog,
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
import { ColorPicker, FontFamilyPicker, FontSizeBox, SpacingMenu } from './FontControls';
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
import { CombineCharsDialog, FontEffectsDialog, PhoneticGuideDialog } from './FontEffectsDialog';
import { isReferenceDialog, ReferenceDialog } from './ReferenceDialogs';
import { Rulers, type PageBox } from './Ruler';
import { NavigationPane } from './NavigationPane';
import { XmlMappingPane } from './XmlMappingPane';
import { OutlineView } from './OutlineView';
import { ReadMode } from './ReadMode';
import { demoted, promoted } from './outlineModel';
import viewStyles from './Views.module.css';

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
/** Width of the docked navigation pane (CSS px, see Views.module.css). */
const NAV_SIZE = 260;
/** Offline-cache keys of the crash-recovery copy (prompt §6 Phase 2: autosave + recovery). */
export const AUTOSAVE_KEY = 'bluedoc.autosave.docx';
export const AUTOSAVE_NAME_KEY = 'bluedoc.autosave.name';

type Registry = CommandRegistry<EditorState>;

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
  platform,
  app = 'doc',
  language = 'vi-VN',
  theme = 'light',
  host,
  sampleUrl,
  onDocumentOpened,
  autosaveMs = 15_000,
  i18nInstance,
}: BlueEditorProps & { i18nInstance: ReturnType<typeof createI18n> }) {
  const t = useMemo(() => i18nInstance.t.bind(i18nInstance), [i18nInstance]);
  const [store] = useState(() => createEditorStore(language, theme));
  const state = useStore(store);
  const resolvedTheme = useResolvedTheme(state.theme);
  const density = useDensity();
  const viewHost = useRef<HTMLDivElement>(null);
  const view = useRef<DocumentView | null>(null);
  const controller = useRef<DocController | null>(null);
  const stored = useRef<StoredDocument | null>(null);
  /** Bytes of the opened file (its digital signatures are verified on them). */
  const openedBytes = useRef<Uint8Array | null>(null);
  /** The template the open document was created from (CORE-FMT-001: "Edit template"). */
  const templateFile = useRef<OpenedFile | null>(null);
  /** "Save as" file type dialog, and the type it picked (for `file.saveAsType`). */
  const [saveAsType, setSaveAsType] = useState(false);
  const saveAsExt = useRef<string | null>(null);
  /** Signature line chosen in "sign a signature line", signed by the next "Ký số". */
  const pendingLine = useRef<LineChoice | null>(null);
  const signLineRef = useRef<((choice: LineChoice) => void) | null>(null);
  const [signSetup, setSignSetup] = useState<{
    defaults: SigningDefaults;
    remembered: RememberedSigning;
  } | null>(null);
  const [tab, setTab] = useState('home');
  /** Mail merge: rule kind of the rule dialog, finish mode of the finish dialog. */
  const [ruleKind, setRuleKind] = useState<RuleKind>('if');
  const [finishMode, setFinishMode] = useState<FinishMode>('document');
  /** View to return to when read mode ends. */
  const beforeRead = useRef<EditorState['viewMode']>('print');
  /** View layout last sent to the engine (`mode:width`). */
  const sentLayout = useRef('print:0');
  const [searchOpen, setSearchOpen] = useState(false);
  useCommandSearchShortcut(() => setSearchOpen(true));
  const [recent, setRecent] = useState<RecentFile[]>([]);
  const workspace = useRef<HTMLElement>(null);
  const [pageBox, setPageBox] = useState<PageBox | null>(null);
  /** Clipboard contents read for Paste Special. */
  const pasteClip = useRef<{ text: string; html: string; internal: string } | null>(null);
  const [pasteAvail, setPasteAvail] = useState({ html: false, text: false });
  /** Clipboard history panel (Home › Clipboard launcher). */
  const [clipPanel, setClipPanel] = useState(false);
  const [equationTemplates, setEquationTemplates] = useState<{ id: string; linear: string }[]>([]);
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
      // Focus at once: typing may start while the set-up commands below are still running.
      view.current?.focus();
      platform.window.setTitle(`${info.name} — ${t('app.doc')}`);
      // New revisions and comments are attributed to the signed-in user (fallback "BlueOffice").
      await controller.current?.run({ cmd: 'setAuthor', name: platform.auth.user?.name ?? '' });
      // Editable regions (CORE-SEC-001) match the signed-in user's name / e-mail;
      // a final or read-only file asks how to open it.
      const sec = await controller.current?.run({
        cmd: 'setIdentity',
        names: identityNames(platform.auth.user),
      });
      if (sec?.security && openProtection(sec.security) !== 'none' && !sec.security.markedFinal)
        store.setState({ protectUi: 'prompt' });
      // AutoCorrect as you type: the user's kept options, else the engine defaults.
      await controller.current?.run(autoCorrectCommand(loadAutoCorrect()));
      await controller.current?.run({ cmd: 'state' });
      if (store.getState().fonts.length === 0) {
        const f = await controller.current?.run({ cmd: 'fontList' });
        if (f?.fonts) store.setState({ fonts: f.fonts });
      }
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

  // Engine + view lifecycle.
  useEffect(() => {
    if (app !== 'doc' || !viewHost.current) return;
    const v = new DocumentView(viewHost.current, platform.engine, {
      onPageChange: (page) => store.setState({ page }),
      onZoomChange: (zoom) => store.setState({ zoom }),
      onInput: (e) => controller.current?.input(e),
      onPointer: (e) => {
        controller.current?.pointer(e);
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
      // Right click on a misspelt word: corrections and dictionary actions.
      onContextMenu: (e) => {
        void controller.current?.run({ cmd: 'spellAt', page: e.page, x: e.x, y: e.y }).then((r) => {
          store.setState({
            spellMenu: r?.spell ? { x: e.clientX, y: e.clientY, info: r.spell } : null,
          });
        });
      },
    });
    view.current = v;
    // Spelling marks follow edits after a short pause (the engine caches unchanged paragraphs).
    let spellTimer: ReturnType<typeof setTimeout> | undefined;
    const refreshSpelling = () => {
      clearTimeout(spellTimer);
      spellTimer = setTimeout(() => {
        const s = store.getState();
        if (s.spellingOn || s.grammarOn)
          void controller.current?.query({ cmd: 'spelling' }).then((r) => {
            if (r?.spelling) v.setSpelling(r.spelling);
          });
      }, 400);
    };
    controller.current = new DocController(
      platform.engine,
      v,
      platform.clipboard,
      (r) => {
        measure();
        if (r.spelling) v.setSpelling(r.spelling);
        else if (r.dirty.length) refreshSpelling();
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
        if (isProtectionRefusal(r.error))
          store.setState({ status: { kind: 'info', key: refusalKey(r.error) } });
        else if (r.error)
          fail(formErrorKey(r.error) ?? reviewErrorKey(r.error) ?? 'errors.edit', r.error);
      },
      (e) => fail('errors.edit', e),
    );
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
  }, [app, platform, store, fail, measure, i18nInstance]);

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
  const printLayoutPdf = useCallback(async (): Promise<Uint8Array> => {
    const mode = store.getState().viewMode;
    if (mode !== 'web' && mode !== 'draft') return platform.engine.exportPdf();
    await controller.current?.run({ cmd: 'setViewLayout', mode: 'print' });
    try {
      return await platform.engine.exportPdf();
    } finally {
      const width = Math.floor(view.current?.contentWidthPt() ?? 0);
      sentLayout.current = `${mode}:${mode === 'web' ? width : 0}`;
      await controller.current?.run(
        mode === 'web' ? { cmd: 'setViewLayout', mode, width } : { cmd: 'setViewLayout', mode },
      );
    }
  }, [platform, store]);

  // Collaborators' carets (live collaboration), drawn by the view.
  useEffect(() => {
    view.current?.setPeers(
      state.showPeers
        ? state.peers.map((p) => {
            const name = p.name || t('collab.anonymous');
            return { ...p, name, label: t('collab.editing', { name }) };
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
    const exportPdf = async () => {
      store.setState({ status: { kind: 'busy', key: 'status.exporting' } });
      try {
        const pdf = await printLayoutPdf();
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
      shortcut: 'Ctrl+Shift+S',
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
      shortcut: 'Ctrl+Shift+E',
      isEnabled: hasDoc,
      run: exportPdf,
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
    r.register({
      id: 'file.print',
      shortcut: 'Ctrl+P',
      isEnabled: hasDoc,
      run: async (s) => {
        try {
          await platform.print.printPdf(await printLayoutPdf(), s.doc?.name ?? '');
        } catch (e) {
          fail('errors.export', e);
        }
      },
    });
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
      ['format.superscript', 'superscript', '', (s) => !!f(s)?.superscript],
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
    for (const [id, dialog] of [
      ['format.fontDialog', 'fontEffects'],
      ['format.phoneticGuide', 'phoneticGuide'],
      ['format.combineChars', 'combineChars'],
    ] as const) {
      r.register({ id, isEnabled: hasDoc, run: () => store.setState({ dialog }) });
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
      isEnabled: (s) => s.painter !== 'off' || !!s.edit?.story || s.focusMode,
      run: async (s) => {
        if (s.painter !== 'off') {
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
    ] as const) {
      r.register({
        id,
        shortcut: shortcut || undefined,
        isEnabled: hasDoc,
        isChecked: (s) => (f(s)?.style ?? 'Normal') === style,
        run: () => run({ cmd: 'applyStyle', id: style }),
      });
    }

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

    // View.
    r.register({
      id: 'view.zoomIn',
      shortcut: 'Ctrl+Shift+=',
      isEnabled: hasDoc,
      run: (s) => view.current?.setZoom(s.zoom * 1.1),
    });
    r.register({
      id: 'view.zoomOut',
      shortcut: 'Ctrl+-',
      isEnabled: hasDoc,
      run: (s) => view.current?.setZoom(s.zoom / 1.1),
    });
    r.register({
      id: 'view.zoomReset',
      shortcut: 'Ctrl+0',
      isEnabled: hasDoc,
      isChecked: (s) => s.zoom === 1,
      run: () => view.current?.setZoom(1),
    });
    r.register({ id: 'view.fitWidth', isEnabled: hasDoc, run: () => view.current?.fitWidth() });
    r.register({
      id: 'view.collaborators',
      isChecked: (s) => s.showPeers,
      run: (s) => store.setState({ showPeers: !s.showPeers }),
    });
    r.register({
      id: 'view.toggleTheme',
      isChecked: (s) => s.theme === 'dark' || s.theme === 'hc',
      // Light → dark → high contrast → light (UI-TOKENS-002).
      run: (s) => store.setState({ theme: nextTheme(s.theme) }),
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
      id: 'review.checkAdminFormat',
      isEnabled: hasDoc,
      run: () => store.setState({ dialog: 'adminFormat' }),
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
    registerTrackingCommands(r, {
      hasDoc,
      run,
      openLockDialog: () => store.setState({ dialog: 'lockTracking' }),
    });
    r.register({
      id: 'review.trackChanges',
      shortcut: 'Ctrl+Shift+R',
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
      run: () => store.setState({ panel: 'comments' }),
    });
    r.register({
      id: 'review.comments',
      isEnabled: hasDoc,
      isChecked: (s) => s.panel === 'comments',
      run: (s) => store.setState({ panel: s.panel === 'comments' ? null : 'comments' }),
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
    // Headers and footers (§17.10), page numbers (§17.16.5.43 PAGE).
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
    // (§17.16.5.1), text direction (§17.4.72), wrapping (§17.4.58), text ↔ table.
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
    return r;
  }, [
    platform,
    store,
    sampleUrl,
    openFile,
    newDocument,
    fail,
    i18nInstance,
    loaded,
    printLayoutPdf,
    pictureHub,
  ]);
  /* eslint-enable react-hooks/refs */

  // Keyboard shortcuts (registry); plain typing and caret keys go through the IME textarea.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.isComposing) return;
      const target = e.target as HTMLElement | null;
      // Inputs of dialogs keep their own keys (except global file shortcuts).
      const inForm = !!target && target.tagName === 'INPUT';
      const c = registry.match(e);
      if (!c || (inForm && !c.id.startsWith('file.') && c.id !== 'edit.find')) return;
      e.preventDefault();
      void registry.execute(c.id, store.getState());
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [registry, store]);

  const runCommand = useCallback(
    (c: EditCommand) => controller.current?.run(c) ?? Promise.resolve(null),
    [],
  );
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
  // Status bar: proofing language of the selection (§17.3.2.20 lang).
  const languageLabel = fs?.lang
    ? (PROOF_LANGUAGES as readonly string[]).includes(fs.lang)
      ? t(`spell.lang.${fs.lang}`)
      : fs.lang
    : t('spell.lang.none');
  const noDoc = !state.doc;
  const focusDoc = () => view.current?.focus();
  const runFocus = (c: EditCommand) =>
    (controller.current?.run(c) ?? Promise.resolve(null)).then(() => focusDoc());
  const tableTab: RibbonTab[] = fs?.table
    ? [
        {
          id: 'table',
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
            s('format.grow', 'commands.format.grow', <AArrowUp size={SMALL} />),
            s('format.shrink', 'commands.format.shrink', <AArrowDown size={SMALL} />),
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
          items: [
            s('style.normal', 'commands.format.styleNormal', <Pilcrow size={SMALL} />),
            s('style.title', 'commands.format.styleTitle', <Type size={SMALL} />),
            s('style.heading1', 'commands.format.styleHeading1', <Heading1 size={SMALL} />),
            s('style.heading2', 'commands.format.styleHeading2', <Heading2 size={SMALL} />),
          ],
        },
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
          ],
        },
        {
          id: 'symbols',
          label: t('ribbon.groups.symbols'),
          items: [
            item('insert.symbol', 'commands.insert.symbol', <Omega size={ICON} />),
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
          id: 'headerFooter',
          label: t('ribbon.groups.headerFooter'),
          items: [
            item('insert.header', 'commands.insert.header', <PanelTop size={ICON} />),
            item('insert.footer', 'commands.insert.footer', <PanelBottom size={ICON} />),
            item('insert.pageNumber', 'commands.insert.pageNumber', <Hash size={ICON} />),
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
            item('review.language', 'commands.review.language', <Languages size={ICON} />),
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
          ],
        },
        {
          id: 'show',
          label: t('ribbon.groups.show'),
          items: [
            item('view.navigationPane', 'commands.view.navigationPane', <PanelLeft size={ICON} />),
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
    view.current?.focus();
  };
  const navOpen = !!state.navPane && !!state.doc && !state.focusMode;
  const rulersOn =
    state.showRuler && !!state.doc && !state.focusMode && state.viewMode !== 'outline';
  const sideLeft =
    (navOpen ? NAV_SIZE : 0) +
    (state.reviewingPane === 'left' && state.doc ? REVIEWING_SIZE.left : 0);
  const hostStyle =
    sideLeft || (state.reviewingPane === 'bottom' && state.doc)
      ? {
          ...(sideLeft ? { left: sideLeft } : {}),
          ...(state.reviewingPane === 'bottom' && state.doc
            ? { bottom: REVIEWING_SIZE.bottom }
            : {}),
        }
      : undefined;
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
        quickAccess={(
          [
            ['file.save', 'commands.file.save'],
            ['edit.undo', 'commands.edit.undo'],
            ['edit.redo', 'commands.edit.redo'],
          ] as const
        ).map(([id, key]) => ({
          id,
          label: t(key),
          shortcut: registry.get(id)?.shortcut,
          enabled: registry.isEnabled(id, state),
          onSelect: () => void registry.execute(id, store.getState()),
        }))}
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
            tabs={[...tabs, ...tableTab, ...pictureTab]}
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
              tabs={[...tabs, ...tableTab, ...pictureTab]}
              value={
                (tab === 'table' && !fs?.table) || (tab === 'pictureFormat' && !picture.tab)
                  ? 'home'
                  : tab
              }
              onValueChange={setTab}
              moreLabel={t('ribbon.more')}
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
              style={hostStyle}
              data-testid="document-view"
              data-marks={state.showMarks ? 'true' : undefined}
            />
            {navOpen && state.navPane && state.doc && (
              <NavigationPane
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
            )}
            {state.viewMode === 'outline' && state.doc && (
              <div className={viewStyles.outlineHost} style={{ left: navOpen ? NAV_SIZE : 0 }}>
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
            {state.spellMenu && (
              <SpellMenu
                menu={state.spellMenu}
                onPick={(text) => {
                  const m = store.getState().spellMenu;
                  store.setState({ spellMenu: null });
                  if (!m) return;
                  const { para, start, end } = m.info;
                  void controller.current
                    ?.run({
                      cmd: 'setSelection',
                      selection: { anchor: { p: para, o: start }, focus: { p: para, o: end } },
                    })
                    .then(() => controller.current?.run({ cmd: 'insertText', text }));
                }}
                onIgnore={(word) => {
                  const rule = store.getState().spellMenu?.info.rule;
                  store.setState({ spellMenu: null });
                  if (rule) {
                    void controller.current?.run({ cmd: 'grammarIgnore', rule, text: word });
                    return;
                  }
                  void controller.current?.run({ cmd: 'spellIgnore', word });
                }}
                onAdd={(word) => {
                  store.setState({ spellMenu: null });
                  saveUserWord(word);
                  void controller.current?.run({ cmd: 'spellAddWord', word });
                }}
                onClose={() => store.setState({ spellMenu: null })}
              />
            )}
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
                <button type="button" onClick={() => store.setState({ macroWarning: false })}>
                  {t('security.dismiss')}
                </button>
              </div>
            )}
            {state.panel === 'merge' && (
              <MergeFieldPanel merge={state.merge} run={runCommand} onClose={closePanel} />
            )}
            {state.panel === 'crossref' && <CrossRefPanel run={runCommand} onClose={closePanel} />}
            {state.panel === 'comments' && (
              <CommentsPanel
                review={state.review}
                run={runCommand}
                language={state.language}
                onClose={closePanel}
              />
            )}
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
          left={
            <>
              <span data-testid="status-page">
                {state.doc
                  ? t('status.page', { current: state.page + 1, total: state.doc.pageCount })
                  : t('status.ready')}
              </span>
              {state.doc && (
                <span data-testid="status-words">
                  {t('status.words', { count: state.doc.wordCount })}
                </span>
              )}
              {state.doc && (
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
              {state.edit?.modified && (
                <span data-testid="status-modified">{t('status.modified')}</span>
              )}
              {state.doc?.protected && (
                <span data-testid="status-protected">{t('security.protected')}</span>
              )}
              {state.doc && <ProtectionStatus info={state.security} />}
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
              <button
                type="button"
                className={styles.statusButton}
                onClick={() => void registry.execute('view.zoomOut', state)}
                aria-label={t('commands.view.zoomOut')}
              >
                −
              </button>
              <span data-testid="status-zoom">
                {t('status.zoom', { value: Math.round(state.zoom * 100) })}
              </span>
              <button
                type="button"
                className={styles.statusButton}
                onClick={() => void registry.execute('view.zoomIn', state)}
                aria-label={t('commands.view.zoomIn')}
              >
                +
              </button>
            </>
          }
        />
      )}
    </div>
  );
}
