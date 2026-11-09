import { DocumentView } from '@blueoffice/canvas-host';
import { SpellMenu, loadUserWords, saveUserWord } from './SpellMenu';
import { AutoCorrectDialog, autoCorrectCommand, loadAutoCorrect } from './AutoCorrectDialog';
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
} from '@blueoffice/platform';
import { PASSWORD_REQUIRED, WRONG_PASSWORD, updateNotice } from '@blueoffice/platform';
import {
  Ribbon,
  StatusBar,
  nextDensity,
  nextTheme,
  setDensity,
  getDensity,
  useDensity,
  useResolvedTheme,
  type RibbonTab,
  type ThemeSetting,
} from '@blueoffice/ui';
import {
  AArrowDown,
  AArrowUp,
  WandSparkles,
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
  Maximize2,
  MessageSquarePlus,
  MessagesSquare,
  MoonStar,
  Rows4,
  Omega,
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
  Sigma,
  SquareFunction,
  TextCursorInput,
  GitCompare,
  GitMerge,
  Braces,
  ListChecks,
  LockOpen,
  Unlink,
  Frame,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { CommandRegistry } from './commands';
import { DocController } from './controller';
import styles from './Editor.module.css';
import { FindPanel, SymbolPanel } from './Panels';
import { CaptionPanel, CrossRefPanel, MergeFieldPanel } from './ReferencePanels';
import { CommentsPanel, ReviewPanel } from './ReviewPanel';
import { PasswordDialog, SavePasswordPanel } from './SecurityPanels';
import { SignaturesPanel, SignDialog } from './SignaturePanels';
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
  TemplateDialog,
} from './DocDialogs';
import { ColorPicker, FontFamilyPicker, FontSizeBox, SpacingMenu } from './FontControls';
import { DefineListDialog, MultilevelMenu, SetNumberingDialog } from './ListDialogs';
import {
  PageBordersDialog,
  StyleSetGallery,
  ThemeGallery,
  WatermarkDialog,
  WatermarkMenu,
} from './PageBackgroundDialogs';
import { Rulers, type PageBox } from './Ruler';

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
  const [signSetup, setSignSetup] = useState<{
    defaults: SigningDefaults;
    remembered: RememberedSigning;
  } | null>(null);
  const [tab, setTab] = useState('home');
  const [recent, setRecent] = useState<RecentFile[]>([]);
  const workspace = useRef<HTMLElement>(null);
  const [pageBox, setPageBox] = useState<PageBox | null>(null);
  /** Clipboard contents read for Paste Special. */
  const pasteClip = useRef<{ text: string; html: string; internal: string } | null>(null);
  const [pasteAvail, setPasteAvail] = useState({ html: false, text: false });
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
      });
      view.current?.setDocument(info);
      platform.window.setTitle(`${info.name} — ${t('app.doc')}`);
      // New revisions and comments are attributed to the signed-in user (fallback "BlueOffice").
      await controller.current?.run({ cmd: 'setAuthor', name: platform.auth.user?.name ?? '' });
      // AutoCorrect as you type: the user's kept options, else the engine defaults.
      await controller.current?.run(autoCorrectCommand(loadAutoCorrect()));
      await controller.current?.run({ cmd: 'state' });
      if (store.getState().fonts.length === 0) {
        const f = await controller.current?.run({ cmd: 'fontList' });
        if (f?.fonts) store.setState({ fonts: f.fonts });
      }
      view.current?.focus();
      onDocumentOpened?.(info.name, info.pageCount);
    },
    [platform, store, t, onDocumentOpened],
  );

  const openFile = useCallback(
    async (file: OpenedFile, doc?: StoredDocument, password?: string) => {
      store.setState({ status: { kind: 'busy', key: 'status.loading' } });
      try {
        const info = await platform.engine.open(file.bytes, file.name, password);
        stored.current = doc ?? null;
        openedBytes.current = file.bytes;
        await loaded(info);
        // A signed file: show the verification status (ADR-0007).
        if (platform.signing && looksSigned(file.bytes)) {
          void platform.signing
            .verify(file.bytes)
            .then((report) => store.setState({ signatures: report }))
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
        store.setState({
          edit: r,
          doc: r.info,
          ...(r.review ? { review: r.review } : {}),
          ...(r.merge ? { merge: r.merge } : {}),
        });
        if (r.error) fail('errors.edit', r.error);
      },
      (e) => fail('errors.edit', e),
    );
    platform.engine.init().then(
      () => {
        store.setState({ engineReady: true });
        // User dictionary kept by this browser / app (a convenience: words re-added each start).
        for (const word of loadUserWords())
          void controller.current?.run({ cmd: 'spellAddWord', word });
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
  }, [app, platform, store, fail, measure]);

  // Rulers follow the caret's page and the panels' layout.
  useEffect(() => measure(), [measure, state.showRuler, state.doc, state.panel]);

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
        const pdf = await platform.engine.exportPdf();
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
        const name = s.doc.name.replace(/\.(txt|md|doc|odt)$/i, '.docx');
        // Files opened from another format (.doc, .odt, .txt) are saved as a new .docx.
        const sameFormat = !!stored.current && /\.(docx|docm)$/i.test(stored.current.ref.name);
        const doc =
          stored.current && !as && sameFormat
            ? await platform.storage.save(stored.current, bytes)
            : await platform.storage.saveAs(name.endsWith('.docx') ? name : `${name}.docx`, bytes);
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
    r.register({
      id: 'file.saveAs',
      shortcut: 'Ctrl+Shift+S',
      isEnabled: hasDoc,
      run: () => save(true),
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
    r.register({ id: 'file.sign', isEnabled: canSign, run: () => openSign('sign') });
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
          await platform.print.printPdf(await platform.engine.exportPdf(), s.doc?.name ?? '');
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
    // Esc: stop the Format Painter, else leave the header/footer.
    r.register({
      id: 'edit.escape',
      shortcut: 'Escape',
      isEnabled: (s) => s.painter !== 'off' || !!s.edit?.story,
      run: async (s) => {
        if (s.painter !== 'off') {
          store.setState({ painter: 'off' });
          await run({ cmd: 'cancelFormat' });
        } else await run({ cmd: 'closeHeaderFooter' });
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
          const file = await platform.files.openFile(['.csv', '.xlsx']);
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
    r.register({
      id: 'review.grammar',
      isEnabled: hasDoc,
      isChecked: (s) => s.grammarOn,
      run: async (s) => {
        store.setState({ grammarOn: !s.grammarOn });
        await controller.current?.run({ cmd: 'setGrammar', on: !s.grammarOn });
      },
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
    for (const id of ['layout.margins', 'layout.size', 'layout.pageSetup', 'layout.columns']) {
      r.register({
        id,
        isEnabled: (s) => hasDoc(s) && !!f(s)?.section,
        run: () => store.setState({ dialog: 'pageSetup' }),
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
    // Floating picture position (§20.4.2.3 anchor); a tracked move with track changes on.
    r.register({
      id: 'picture.position',
      isEnabled: (s) => hasDoc(s) && !!f(s)?.picture,
      run: () => store.setState({ dialog: 'picturePosition' }),
    });
    return r;
  }, [platform, store, sampleUrl, openFile, newDocument, fail, i18nInstance, loaded]);
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
  const fs = state.edit?.format;
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
          id: 'signature',
          label: t('ribbon.groups.signature'),
          items: [
            item('file.sign', 'commands.file.sign', <Signature size={ICON} />),
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
            item('insert.image', 'commands.insert.image', <Image size={ICON} />),
            item('insert.symbol', 'commands.insert.symbol', <Omega size={ICON} />),
          ],
        },
        {
          id: 'equation',
          label: t('ribbon.groups.equation'),
          items: [
            item('insert.equation', 'commands.insert.equation', <Sigma size={ICON} />),
            item('insert.field', 'commands.insert.field', <Braces size={ICON} />),
            item('insert.control', 'commands.insert.control', <ListChecks size={ICON} />),
            item(
              'equation.professional',
              'commands.equation.professional',
              <SquareFunction size={ICON} />,
            ),
            item('equation.linear', 'commands.equation.linear', <TextCursorInput size={ICON} />),
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
            item('review.autoCorrect', 'commands.review.autoCorrect', <WandSparkles size={ICON} />),
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
            s('review.viewMarkup', 'commands.review.viewMarkup', <Eye size={SMALL} />),
            s('review.viewFinal', 'commands.review.viewFinal', <Eye size={SMALL} />),
            s('review.viewOriginal', 'commands.review.viewOriginal', <Eye size={SMALL} />),
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
          ],
        },
      ],
    },
    {
      id: 'view',
      label: t('ribbon.tabs.view'),
      groups: [
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
      const bytes = pdf ? await platform.engine.exportPdf() : await platform.engine.saveDocx();
      const signed = await signing.sign(bytes, request);
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
  return (
    <div
      className={`bo-root ${styles.root}`}
      data-theme={resolvedTheme}
      data-app={app}
      lang={state.language}
    >
      <header className={styles.titleBar}>
        <span className={styles.logo}>
          <FileText size={18} aria-hidden />{' '}
          {host?.brandName ? `${host.brandName} · ${appName}` : appName}
        </span>
        <span className={styles.docName} data-testid="doc-name">
          {state.doc?.name ?? ''}
          {state.edit?.modified ? ' •' : ''}
        </span>
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
      </header>
      {app === 'doc' ? (
        <>
          <Ribbon
            tabs={[...tabs, ...tableTab]}
            value={tab === 'table' && !fs?.table ? 'home' : tab}
            onValueChange={setTab}
            moreLabel={t('ribbon.more')}
          />
          <main ref={workspace} className={styles.workspace}>
            {state.showRuler && state.doc && (
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
              className={`${styles.viewHost} ${state.showRuler && state.doc ? styles.withRulers : ''}`}
              data-testid="document-view"
            />
            {(state.edit?.story?.kind === 'header' || state.edit?.story?.kind === 'footer') && (
              <HeaderFooterBar story={state.edit.story} run={runCommand} />
            )}
            {state.edit?.control && <ControlBar control={state.edit.control} run={runCommand} />}
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
            {state.dialog === 'pageBorders' && (
              <PageBordersDialog run={runCommand} onClose={closeDialog} />
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
            {state.panel === 'symbols' && (
              <SymbolPanel
                onPick={(sym) => {
                  void controller.current?.run({ cmd: 'insertText', text: sym });
                  view.current?.focus();
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
            {state.edit?.modified && (
              <span data-testid="status-modified">{t('status.modified')}</span>
            )}
            {state.doc?.protected && (
              <span data-testid="status-protected">{t('security.protected')}</span>
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
    </div>
  );
}
