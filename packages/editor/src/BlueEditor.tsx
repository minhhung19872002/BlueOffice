import { DocumentView } from '@blueoffice/canvas-host';
import { createI18n, I18nextProvider, type Language } from '@blueoffice/i18n';
import type {
  CharFormat,
  EditCommand,
  OpenedFile,
  ParaFormat,
  Platform,
  RecentFile,
  StoredDocument,
} from '@blueoffice/platform';
import {
  Ribbon,
  StatusBar,
  useResolvedTheme,
  type RibbonTab,
  type ThemeSetting,
} from '@blueoffice/ui';
import {
  AArrowDown,
  AArrowUp,
  AlignCenter,
  AlignJustify,
  AlignLeft,
  AlignRight,
  Baseline,
  Bold,
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
  FilePlus,
  FileText,
  FolderOpen,
  Heading1,
  Heading2,
  Highlighter,
  Image,
  Italic,
  Languages,
  List,
  ListIndentDecrease,
  ListIndentIncrease,
  ListOrdered,
  Maximize2,
  MessageSquarePlus,
  MessagesSquare,
  MoonStar,
  Omega,
  PanelRight,
  Pilcrow,
  Printer,
  Redo2,
  RectangleHorizontal,
  RemoveFormatting,
  Replace,
  Ruler,
  Save,
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
  Type,
  Underline,
  Undo2,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { CommandRegistry } from './commands';
import { DocController } from './controller';
import styles from './Editor.module.css';
import { FindPanel, SymbolPanel } from './Panels';
import { CommentsPanel, ReviewPanel } from './ReviewPanel';
import { createEditorStore, type EditorState } from './store';

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
  const viewHost = useRef<HTMLDivElement>(null);
  const view = useRef<DocumentView | null>(null);
  const controller = useRef<DocController | null>(null);
  const stored = useRef<StoredDocument | null>(null);
  const [tab, setTab] = useState('home');
  const [recent, setRecent] = useState<RecentFile[]>([]);

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
      store.setState({ doc: info, page: 0, status: { kind: 'idle' }, edit: null, panel: null });
      view.current?.setDocument(info);
      platform.window.setTitle(`${info.name} — ${t('app.doc')}`);
      // New revisions and comments are attributed to the signed-in user (fallback "BlueOffice").
      await controller.current?.run({ cmd: 'setAuthor', name: platform.auth.user?.name ?? '' });
      await controller.current?.run({ cmd: 'state' });
      view.current?.focus();
      onDocumentOpened?.(info.name, info.pageCount);
    },
    [platform, store, t, onDocumentOpened],
  );

  const openFile = useCallback(
    async (file: OpenedFile, doc?: StoredDocument) => {
      store.setState({ status: { kind: 'busy', key: 'status.loading' } });
      try {
        const info = await platform.engine.open(file.bytes, file.name);
        stored.current = doc ?? null;
        await loaded(info);
        if (file.path)
          await platform.shell.addRecentFile({
            name: file.name,
            path: file.path,
            openedAt: Date.now(),
          });
      } catch (e) {
        fail('errors.open', e);
      }
    },
    [platform, store, fail, loaded],
  );

  const newDocument = useCallback(async () => {
    try {
      await platform.engine.init();
      stored.current = null;
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
      onPointer: (e) => controller.current?.pointer(e),
    });
    view.current = v;
    controller.current = new DocController(
      platform.engine,
      v,
      platform.clipboard,
      (r) => {
        store.setState(
          r.review ? { edit: r, doc: r.info, review: r.review } : { edit: r, doc: r.info },
        );
        if (r.error) fail('errors.edit', r.error);
      },
      (e) => fail('errors.edit', e),
    );
    platform.engine.init().then(
      () => store.setState({ engineReady: true }),
      (e: unknown) => fail('errors.engine', e),
    );
    const unOpen = platform.shell.onOpenRequest((f) => void openFileRef.current(f));
    void platform.shell.recentFiles().then(setRecent);
    // Crash recovery: an autosave left behind by a previous session.
    void platform.offline.get(AUTOSAVE_NAME_KEY).then(
      (n) => n && store.setState({ recovery: dec.decode(n) }),
      () => {},
    );
    return () => {
      unOpen();
      v.dispose();
      view.current = null;
      controller.current = null;
    };
  }, [app, platform, store, fail]);

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
        const name = s.doc.name.replace(/\.(txt|md)$/i, '.docx');
        const doc =
          stored.current && !as
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
    r.register({ id: 'format.clear', isEnabled: hasDoc, run: () => fmt({ kind: 'clear' }) });

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
      id: 'view.toggleTheme',
      isChecked: (s) => s.theme === 'dark',
      run: (s) => store.setState({ theme: s.theme === 'dark' ? 'light' : 'dark' }),
    });
    r.register({
      id: 'view.toggleLanguage',
      run: async (s) => {
        const next: Language = s.language === 'vi-VN' ? 'en-US' : 'vi-VN';
        await i18nInstance.changeLanguage(next);
        store.setState({ language: next });
      },
    });
    // Review: track changes (ECMA-376 Part 1, §17.13.5).
    const rv = (s: EditorState) => s.review;
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
    // Page setup dialogs arrive with the ruler (see docs/plan/phase-2.md, "Hạn chế").
    for (const id of ['layout.margins', 'layout.orientation', 'layout.size']) {
      r.register({ id, isEnabled: () => false, run: () => {} });
    }
    return r;
  }, [platform, store, sampleUrl, openFile, newDocument, fail, i18nInstance]);
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
            item('file.open', 'commands.file.open', <FolderOpen size={ICON} />),
            item('file.save', 'commands.file.save', <Save size={ICON} />),
            item('file.saveAs', 'commands.file.saveAs', <Save size={ICON} />),
            item('file.openSample', 'commands.file.openSample', <FileText size={ICON} />),
          ],
        },
        {
          id: 'output',
          label: t('ribbon.groups.output'),
          items: [
            item('file.exportPdf', 'commands.file.exportPdf', <FileDown size={ICON} />),
            item('file.print', 'commands.file.print', <Printer size={ICON} />),
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
          ],
        },
        {
          id: 'font',
          label: t('ribbon.groups.font'),
          items: [
            s('format.bold', 'commands.format.bold', <Bold size={SMALL} />),
            s('format.italic', 'commands.format.italic', <Italic size={SMALL} />),
            s('format.underline', 'commands.format.underline', <Underline size={SMALL} />),
            s('format.strike', 'commands.format.strike', <Strikethrough size={SMALL} />),
            s('format.subscript', 'commands.format.subscript', <Subscript size={SMALL} />),
            s('format.superscript', 'commands.format.superscript', <Superscript size={SMALL} />),
            s('format.grow', 'commands.format.grow', <AArrowUp size={SMALL} />),
            s('format.shrink', 'commands.format.shrink', <AArrowDown size={SMALL} />),
            s('format.color', 'commands.format.color', <Baseline size={SMALL} />),
            s('format.highlight', 'commands.format.highlight', <Highlighter size={SMALL} />),
            s('format.clear', 'commands.format.clear', <RemoveFormatting size={SMALL} />),
          ],
        },
        {
          id: 'paragraph',
          label: t('ribbon.groups.paragraph'),
          items: [
            s('format.bullets', 'commands.format.bullets', <List size={SMALL} />),
            s('format.numbering', 'commands.format.numbering', <ListOrdered size={SMALL} />),
            s('format.outdent', 'commands.format.outdent', <ListIndentDecrease size={SMALL} />),
            s('format.indent', 'commands.format.indent', <ListIndentIncrease size={SMALL} />),
            s('format.alignLeft', 'commands.format.alignLeft', <AlignLeft size={SMALL} />),
            s('format.alignCenter', 'commands.format.alignCenter', <AlignCenter size={SMALL} />),
            s('format.alignRight', 'commands.format.alignRight', <AlignRight size={SMALL} />),
            s('format.justify', 'commands.format.justify', <AlignJustify size={SMALL} />),
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
          ],
        },
      ],
    },
    {
      id: 'review',
      label: t('ribbon.tabs.review'),
      groups: [
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
            item('view.toggleTheme', 'commands.view.toggleTheme', <MoonStar size={ICON} />),
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
  const runCommand = (c: EditCommand) => controller.current?.run(c) ?? Promise.resolve(null);
  const closePanel = () => {
    store.setState({ panel: null });
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
          <Ribbon tabs={tabs} value={tab} onValueChange={setTab} />
          <main className={styles.workspace}>
            <div ref={viewHost} className={styles.viewHost} data-testid="document-view" />
            {(state.panel === 'find' || state.panel === 'replace') && (
              <FindPanel
                run={runCommand}
                replace={state.panel === 'replace'}
                onClose={closePanel}
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
