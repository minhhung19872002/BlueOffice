import { DocumentView } from '@blueoffice/canvas-host';
import { createI18n, I18nextProvider, type Language } from '@blueoffice/i18n';
import type { OpenedFile, Platform, RecentFile } from '@blueoffice/platform';
import { Ribbon, StatusBar, useResolvedTheme, type RibbonTab, type ThemeSetting } from '@blueoffice/ui';
import {
  AlignCenter,
  AlignJustify,
  AlignLeft,
  AlignRight,
  Bold,
  ClipboardPaste,
  Copy,
  FileDown,
  FileText,
  FolderOpen,
  Image,
  Italic,
  Languages,
  Maximize2,
  MoonStar,
  Printer,
  RectangleHorizontal,
  Ruler,
  Scaling,
  Scissors,
  Share2,
  SquareSplitVertical,
  Table,
  Underline,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { CommandRegistry } from './commands';
import styles from './Editor.module.css';
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
  app?: 'write' | 'sheet' | 'slide';
  language?: Language;
  theme?: ThemeSetting;
  host?: HostConfig;
  /** URL of the sample document (served by the shell). */
  sampleUrl?: string;
  /** Called after a document is opened (for analytics / recent files). */
  onDocumentOpened?: (name: string, pageCount: number) => void;
}

const ICON = 20;
const SMALL = 16;

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

function EditorInner({
  platform,
  app = 'write',
  language = 'vi-VN',
  theme = 'light',
  host,
  sampleUrl,
  onDocumentOpened,
  i18nInstance,
}: BlueEditorProps & { i18nInstance: ReturnType<typeof createI18n> }) {
  const t = useMemo(() => i18nInstance.t.bind(i18nInstance), [i18nInstance]);
  const [store] = useState(() => createEditorStore(language, theme));
  const state = useStore(store);
  const resolvedTheme = useResolvedTheme(state.theme);
  const viewHost = useRef<HTMLDivElement>(null);
  const view = useRef<DocumentView | null>(null);
  const [tab, setTab] = useState('home');
  const [recent, setRecent] = useState<RecentFile[]>([]);

  const fail = useCallback(
    (key: string, e: unknown) =>
      store.setState({ status: { kind: 'error', key, message: e instanceof Error ? e.message : String(e) } }),
    [store],
  );

  const openFile = useCallback(
    async (file: OpenedFile) => {
      store.setState({ status: { kind: 'busy', key: 'status.loading' } });
      try {
        const info = await platform.engine.open(file.bytes, file.name);
        store.setState({ doc: info, page: 0, status: { kind: 'idle' } });
        view.current?.setDocument(info);
        platform.window.setTitle(`${info.name} — ${t('app.write')}`);
        if (file.path) await platform.shell.addRecentFile({ name: file.name, path: file.path, openedAt: Date.now() });
        onDocumentOpened?.(info.name, info.pageCount);
      } catch (e) {
        fail('errors.open', e);
      }
    },
    [platform, store, t, fail, onDocumentOpened],
  );

  // Keep the latest openFile without re-creating the view.
  const openFileRef = useRef(openFile);
  useEffect(() => {
    openFileRef.current = openFile;
  }, [openFile]);

  // Engine + view lifecycle.
  useEffect(() => {
    if (app !== 'write' || !viewHost.current) return;
    const v = new DocumentView(viewHost.current, platform.engine, {
      onPageChange: (page) => store.setState({ page }),
      onZoomChange: (zoom) => store.setState({ zoom }),
    });
    view.current = v;
    platform.engine.init().then(
      () => store.setState({ engineReady: true }),
      (e: unknown) => fail('errors.engine', e),
    );
    const unOpen = platform.shell.onOpenRequest((f) => void openFileRef.current(f));
    void platform.shell.recentFiles().then(setRecent);
    return () => {
      unOpen();
      v.dispose();
      view.current = null;
    };
  }, [app, platform, store, fail]);

  const registry: Registry = useMemo(() => {
    const r = new CommandRegistry<EditorState>();
    const hasDoc = (s: EditorState) => !!s.doc;
    const exportPdf = async () => {
      store.setState({ status: { kind: 'busy', key: 'status.exporting' } });
      try {
        const pdf = await platform.engine.exportPdf();
        const name = (store.getState().doc?.name ?? 'document').replace(/\.[^.]+$/, '') + '.pdf';
        const saved = await platform.files.saveFile(pdf, name, 'application/pdf');
        store.setState({ status: saved ? { kind: 'info', key: 'status.saved', params: { name } } : { kind: 'idle' } });
      } catch (e) {
        fail('errors.export', e);
      }
    };
    r.register({
      id: 'file.open',
      shortcut: 'Ctrl+O',
      run: async () => {
        try {
          const doc = await platform.storage.open();
          if (doc) await openFile({ name: doc.ref.name, path: doc.ref.id, bytes: doc.bytes });
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
          await openFile({ name: sampleUrl!.split('/').pop() ?? 'sample.docx', bytes: new Uint8Array(await res.arrayBuffer()) });
        } catch (e) {
          fail('errors.open', e);
        }
      },
    });
    r.register({ id: 'file.exportPdf', shortcut: 'Ctrl+Shift+E', isEnabled: hasDoc, run: exportPdf });
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
    r.register({ id: 'view.zoomIn', shortcut: 'Ctrl+=', isEnabled: hasDoc, run: (s) => view.current?.setZoom(s.zoom * 1.1) });
    r.register({ id: 'view.zoomOut', shortcut: 'Ctrl+-', isEnabled: hasDoc, run: (s) => view.current?.setZoom(s.zoom / 1.1) });
    r.register({ id: 'view.zoomReset', shortcut: 'Ctrl+0', isEnabled: hasDoc, isChecked: (s) => s.zoom === 1, run: () => view.current?.setZoom(1) });
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
    // Editing commands exist in the registry so menus/shortcuts are wired; they turn on in Phase 2.
    for (const id of [
      'edit.paste', 'edit.cut', 'edit.copy',
      'format.bold', 'format.italic', 'format.underline',
      'format.alignLeft', 'format.alignCenter', 'format.alignRight', 'format.justify',
      'insert.table', 'insert.image', 'insert.pageBreak',
      'layout.margins', 'layout.orientation', 'layout.size',
    ]) {
      r.register({ id, isEnabled: () => false, run: () => {} });
    }
    return r;
  }, [platform, store, sampleUrl, openFile, fail, i18nInstance]);

  // Keyboard shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const c = registry.match(e);
      if (!c) return;
      e.preventDefault();
      void registry.execute(c.id, store.getState());
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [registry, store]);

  const item = (id: string, labelKey: string, icon: ReactNode, size: 'large' | 'small' = 'large') => {
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
      tooltip: enabled || id.startsWith('view.') || id.startsWith('file.') ? t(labelKey) : t('ribbon.comingLater'),
      onSelect: () => void registry.execute(id, store.getState()),
    };
  };

  const tabs: RibbonTab[] = [
    {
      id: 'file',
      label: t('ribbon.tabs.file'),
      groups: [
        { id: 'document', label: t('ribbon.groups.document'), items: [item('file.open', 'commands.file.open', <FolderOpen size={ICON} />), item('file.openSample', 'commands.file.openSample', <FileText size={ICON} />)] },
        { id: 'output', label: t('ribbon.groups.output'), items: [item('file.exportPdf', 'commands.file.exportPdf', <FileDown size={ICON} />), item('file.print', 'commands.file.print', <Printer size={ICON} />)] },
      ],
    },
    {
      id: 'home',
      label: t('ribbon.tabs.home'),
      groups: [
        { id: 'clipboard', label: t('ribbon.groups.clipboard'), items: [item('edit.paste', 'commands.edit.paste', <ClipboardPaste size={ICON} />), item('edit.cut', 'commands.edit.cut', <Scissors size={SMALL} />, 'small'), item('edit.copy', 'commands.edit.copy', <Copy size={SMALL} />, 'small')] },
        { id: 'font', label: t('ribbon.groups.font'), items: [item('format.bold', 'commands.format.bold', <Bold size={SMALL} />, 'small'), item('format.italic', 'commands.format.italic', <Italic size={SMALL} />, 'small'), item('format.underline', 'commands.format.underline', <Underline size={SMALL} />, 'small')] },
        { id: 'paragraph', label: t('ribbon.groups.paragraph'), items: [item('format.alignLeft', 'commands.format.alignLeft', <AlignLeft size={SMALL} />, 'small'), item('format.alignCenter', 'commands.format.alignCenter', <AlignCenter size={SMALL} />, 'small'), item('format.alignRight', 'commands.format.alignRight', <AlignRight size={SMALL} />, 'small'), item('format.justify', 'commands.format.justify', <AlignJustify size={SMALL} />, 'small')] },
        { id: 'document', label: t('ribbon.groups.document'), items: [item('file.open', 'commands.file.open', <FolderOpen size={ICON} />), item('file.exportPdf', 'commands.file.exportPdf', <FileDown size={ICON} />)] },
      ],
    },
    {
      id: 'insert',
      label: t('ribbon.tabs.insert'),
      groups: [
        { id: 'objects', label: t('ribbon.groups.insertObjects'), items: [item('insert.table', 'commands.insert.table', <Table size={ICON} />), item('insert.image', 'commands.insert.image', <Image size={ICON} />)] },
        { id: 'pages', label: t('ribbon.groups.pages'), items: [item('insert.pageBreak', 'commands.insert.pageBreak', <SquareSplitVertical size={ICON} />)] },
      ],
    },
    {
      id: 'layout',
      label: t('ribbon.tabs.layout'),
      groups: [
        { id: 'pageSetup', label: t('ribbon.groups.pageSetup'), items: [item('layout.margins', 'commands.layout.margins', <Ruler size={ICON} />), item('layout.orientation', 'commands.layout.orientation', <RectangleHorizontal size={ICON} />), item('layout.size', 'commands.layout.size', <Scaling size={ICON} />)] },
      ],
    },
    {
      id: 'view',
      label: t('ribbon.tabs.view'),
      groups: [
        { id: 'zoom', label: t('ribbon.groups.zoom'), items: [item('view.zoomIn', 'commands.view.zoomIn', <ZoomIn size={ICON} />), item('view.zoomOut', 'commands.view.zoomOut', <ZoomOut size={ICON} />), item('view.zoomReset', 'commands.view.zoomReset', <span style={{ fontWeight: 700, fontSize: 13 }}>1:1</span>), item('view.fitWidth', 'commands.view.fitWidth', <Maximize2 size={ICON} />)] },
        { id: 'appearance', label: t('ribbon.groups.appearance'), items: [item('view.toggleTheme', 'commands.view.toggleTheme', <MoonStar size={ICON} />), item('view.toggleLanguage', 'commands.view.toggleLanguage', <Languages size={ICON} />)] },
      ],
    },
  ];

  const status = state.status;
  const appName = t(`app.${app}`);
  return (
    <div className={`bo-root ${styles.root}`} data-theme={resolvedTheme} data-app={app} lang={state.language}>
      <header className={styles.titleBar}>
        <span className={styles.logo}>
          <FileText size={18} aria-hidden /> {host?.brandName ? `${host.brandName} · ${appName}` : appName}
        </span>
        <span className={styles.docName} data-testid="doc-name">
          {state.doc?.name ?? ''}
        </span>
        {host?.showShare && (
          <button type="button" className={styles.hostButton} onClick={host.onShare}>
            <Share2 size={14} /> {t('host.share')}
          </button>
        )}
        {host?.showClose && (
          <button type="button" className={styles.hostButton} onClick={host.onClose} aria-label={t('host.close')}>
            <X size={16} />
          </button>
        )}
      </header>
      {app === 'write' ? (
        <>
          <Ribbon tabs={tabs} value={tab} onValueChange={setTab} />
          <main className={styles.workspace}>
            <div ref={viewHost} className={styles.viewHost} data-testid="document-view" />
            {!state.doc && (
              <div className={styles.start}>
                <div className={styles.card}>
                  <h1>{t('start.title')}</h1>
                  <p>{t('start.subtitle')}</p>
                  <div className={styles.actions}>
                    <button type="button" className={styles.primary} onClick={() => void registry.execute('file.open', state)}>
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
                      {recent.length === 0 ? <li>{t('start.noRecent')}</li> : recent.slice(0, 5).map((r) => <li key={r.path}>{r.name}</li>)}
                    </ul>
                  </div>
                </div>
              </div>
            )}
            {status.kind === 'error' && (
              <div className={styles.error} role="alert" onClick={() => store.setState({ status: { kind: 'idle' } })}>
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
              {state.doc ? t('status.page', { current: state.page + 1, total: state.doc.pageCount }) : t('status.ready')}
            </span>
            {state.doc && <span>{t('status.words', { count: state.doc.wordCount })}</span>}
            {status.kind === 'busy' && <span>{t(status.key)}</span>}
            {status.kind === 'info' && <span>{t(status.key, status.params)}</span>}
          </>
        }
        right={
          <>
            <span data-testid="status-engine">{t('status.engine', { backend: platform.engine.backend === 'wasm' ? 'WASM' : 'native' })}</span>
            <button type="button" className={styles.statusButton} onClick={() => void registry.execute('view.zoomOut', state)} aria-label={t('commands.view.zoomOut')}>
              −
            </button>
            <span data-testid="status-zoom">{t('status.zoom', { value: Math.round(state.zoom * 100) })}</span>
            <button type="button" className={styles.statusButton} onClick={() => void registry.execute('view.zoomIn', state)} aria-label={t('commands.view.zoomIn')}>
              +
            </button>
          </>
        }
      />
    </div>
  );
}
