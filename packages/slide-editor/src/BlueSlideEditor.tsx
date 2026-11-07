import { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import { CommandRegistry } from '@blueoffice/editor';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import type { DocInfo, Platform } from '@blueoffice/platform';
import { Ribbon, StatusBar, useResolvedTheme, type RibbonTab } from '@blueoffice/ui';
import {
  BookOpen,
  FileDown,
  FolderOpen,
  LayoutGrid,
  MonitorPlay,
  Play,
  Presentation,
  Printer,
  Rows3,
  Save,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { fit, formatElapsed, showAction, step } from './show';
import { SlideCanvas } from './SlideCanvas';
import styles from './SlideEditor.module.css';

export interface BlueSlideEditorProps {
  platform: Platform;
  language?: Language;
}

export type SlideView = 'normal' | 'sorter' | 'reading' | 'show' | 'presenter';

/** State passed to registry commands. */
export interface SlideEditorState {
  hasDeck: boolean;
  current: number;
}

const PPTX_MIME = 'application/vnd.openxmlformats-officedocument.presentationml.presentation';

export function BlueSlideEditor(props: BlueSlideEditorProps) {
  const [i18n] = useState(() => createI18n(props.language));
  return (
    <I18nextProvider i18n={i18n}>
      <SlideEditorInner {...props} />
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

function SlideEditorInner({ platform }: BlueSlideEditorProps) {
  const { t } = useTranslation();
  const engine = platform.engine;
  const theme = useResolvedTheme('auto');
  const [info, setInfo] = useState<DocInfo | null>(null);
  const [fileName, setFileName] = useState('');
  const [current, setCurrent] = useState(0);
  const [view, setView] = useState<SlideView>('normal');
  const [returnView, setReturnView] = useState<SlideView>('normal');
  const [notes, setNotes] = useState('');
  const [status, setStatus] = useState('');
  const [black, setBlack] = useState(false);
  const [startedAt, setStartedAt] = useState(0);
  const [now, setNow] = useState(() => Date.now());
  const [revision, setRevision] = useState(0);
  const showRef = useRef<HTMLDivElement>(null);
  const [stageRef, stage] = useSize();
  const glyphs = useMemo(() => new GlyphStore(engine), [engine]);
  const images = useMemo(() => new ImageStore(engine), [engine]);

  const count = info?.pageCount ?? 0;
  const slideSize = info?.pages[0] ?? { width: 960, height: 540 };

  const fail = useCallback(
    (e: unknown) =>
      setStatus(t('slideEditor.error', { message: e instanceof Error ? e.message : String(e) })),
    [t],
  );

  const load = useCallback(
    async (bytes: Uint8Array, name: string) => {
      setStatus(t('slideEditor.loading'));
      await engine.init();
      const doc = await engine.open(bytes, name);
      images.clear();
      setInfo(doc);
      setFileName(name);
      setCurrent(0);
      setRevision((r) => r + 1);
      setStatus('');
      platform.window.setTitle(`${name} — ${t('app.slide')}`);
    },
    [engine, images, platform, t],
  );

  useEffect(() => {
    platform.window.setTitle(t('appTitle.slide'));
  }, [platform, t]);

  useEffect(() => {
    const un = platform.shell.onOpenRequest((f) => void load(f.bytes, f.name).catch(fail));
    return un;
  }, [platform, load, fail]);

  // Notes of the current slide.
  useEffect(() => {
    if (!info) return;
    let cancelled = false;
    void engine
      .getSlideNotes(Math.min(current, count - 1))
      .then((n) => !cancelled && setNotes(n))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [engine, info, current, count]);

  // Clock for the presenter view.
  useEffect(() => {
    if (view !== 'presenter') return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [view]);

  const startShow = useCallback(
    (from: number, presenter: boolean) => {
      if (!count) return;
      setReturnView(view === 'show' || view === 'presenter' ? 'normal' : view);
      setCurrent(Math.max(0, Math.min(from, count - 1)));
      setBlack(false);
      setStartedAt(Date.now());
      setNow(Date.now());
      setView(presenter ? 'presenter' : 'show');
    },
    [count, view],
  );

  const endShow = useCallback(() => {
    setView(returnView);
    setCurrent((c) => Math.min(c, Math.max(0, count - 1)));
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
  }, [returnView, count]);

  // Full screen while the show runs.
  useEffect(() => {
    if (view === 'show') void showRef.current?.requestFullscreen?.().catch(() => {});
  }, [view]);

  // Leaving full screen with the browser's own Esc ends the show too.
  useEffect(() => {
    const onFs = () => {
      if (!document.fullscreenElement && view === 'show') setView(returnView);
    };
    document.addEventListener('fullscreenchange', onFs);
    return () => document.removeEventListener('fullscreenchange', onFs);
  }, [view, returnView]);

  const registry = useMemo(() => {
    const reg = new CommandRegistry<SlideEditorState>();
    const guard = async (f: () => Promise<unknown>) => {
      try {
        await f();
      } catch (e) {
        fail(e);
      }
    };
    const base = () => fileName.replace(/\.pptx$/i, '') || t('slideEditor.untitled');
    reg.register({
      id: 'file.open',
      shortcut: 'Ctrl+O',
      run: () =>
        guard(async () => {
          const f = await platform.files.openFile(['.pptx']);
          if (f) await load(f.bytes, f.name);
        }),
    });
    reg.register({
      id: 'file.save',
      shortcut: 'Ctrl+S',
      isEnabled: (s) => s.hasDeck,
      run: () =>
        guard(async () => {
          const saved = await platform.files.saveFile(
            await engine.savePptx(),
            `${base()}.pptx`,
            PPTX_MIME,
          );
          if (saved) setStatus(t('slideEditor.saved', { name: saved }));
        }),
    });
    reg.register({
      id: 'file.exportPdf',
      isEnabled: (s) => s.hasDeck,
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
      isEnabled: (s) => s.hasDeck,
      run: () => guard(async () => platform.print.printPdf(await engine.exportPdf(), base())),
    });
    for (const v of ['normal', 'sorter', 'reading'] as const) {
      reg.register({ id: `view.${v}`, isChecked: () => view === v, run: () => setView(v) });
    }
    reg.register({
      id: 'show.fromStart',
      shortcut: 'F5',
      isEnabled: (s) => s.hasDeck,
      run: () => startShow(0, false),
    });
    reg.register({
      id: 'show.fromCurrent',
      shortcut: 'Shift+F5',
      isEnabled: (s) => s.hasDeck,
      run: (s) => startShow(s.current, false),
    });
    reg.register({
      id: 'show.presenter',
      shortcut: 'Alt+F5',
      isEnabled: (s) => s.hasDeck,
      run: (s) => startShow(s.current, true),
    });
    return reg;
  }, [fail, fileName, t, platform, load, engine, view, startShow]);

  const state: SlideEditorState = useMemo(
    () => ({ hasDeck: count > 0, current }),
    [count, current],
  );
  const exec = useCallback((id: string) => void registry.execute(id, state), [registry, state]);

  // Keyboard: shortcuts everywhere, navigation in views and during the show.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) return;
      if (view === 'show' || view === 'presenter') {
        const a = showAction(e.key);
        if (!a) return;
        e.preventDefault();
        if (a === 'exit') endShow();
        else if (a === 'black') setBlack((b) => !b);
        else {
          setBlack(false);
          setCurrent((c) => step(c, count, a));
        }
        return;
      }
      const cmd = registry.match(e);
      if (cmd) {
        e.preventDefault();
        exec(cmd.id);
        return;
      }
      const a = showAction(e.key);
      if (a === 'next' || a === 'prev' || a === 'first' || a === 'last') {
        if (e.key === ' ' || e.key === 'Enter' || e.key === 'Backspace') return;
        e.preventDefault();
        setCurrent((c) => Math.min(step(c, count, a), Math.max(0, count - 1)));
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [view, count, registry, exec, endShow]);

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
      enabled: registry.isEnabled(id, state),
      checked: registry.isChecked(id, state),
      onSelect: () => exec(id),
    });
    return [
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
              item('file.exportPdf', t('slideEditor.exportPdf'), <FileDown size={16} />, 'small'),
              item('file.print', t('slideEditor.print'), <Printer size={16} />, 'small'),
            ],
          },
          {
            id: 'show',
            label: t('slideEditor.groupShow'),
            items: [
              item('show.fromStart', t('slideEditor.fromStart'), <Play size={18} />),
              item(
                'show.fromCurrent',
                t('slideEditor.fromCurrent'),
                <MonitorPlay size={16} />,
                'small',
              ),
              item(
                'show.presenter',
                t('slideEditor.presenter'),
                <Presentation size={16} />,
                'small',
              ),
            ],
          },
        ],
      },
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
            ],
          },
        ],
      },
    ];
  }, [t, registry, state, exec]);

  const canvas = (index: number, width: number, className?: string) => (
    <SlideCanvas
      engine={engine}
      glyphs={glyphs}
      images={images}
      index={index}
      slide={info?.pages[index] ?? slideSize}
      width={width}
      revision={revision}
      className={className}
      label={t('slideEditor.slideOf', { n: index + 1, total: count })}
    />
  );

  const thumbs = (
    <nav className={styles.thumbs} aria-label={t('slideEditor.thumbnails')}>
      {Array.from({ length: count }, (_, i) => (
        <button
          key={i}
          type="button"
          className={i === current ? styles.thumbActive : styles.thumb}
          aria-current={i === current}
          onClick={() => setCurrent(i)}
        >
          <span className={styles.thumbNo}>{i + 1}</span>
          {canvas(i, 150)}
        </button>
      ))}
    </nav>
  );

  const main = fit(slideSize, { width: stage.width - 32, height: stage.height - 32 });
  const elapsed = formatElapsed(now - startedAt);

  return (
    <div className={styles.root} data-theme={theme} data-app="slide">
      {view !== 'show' && view !== 'presenter' && <Ribbon tabs={ribbonTabs} />}
      {view === 'normal' && (
        <div className={styles.normal}>
          {thumbs}
          <div className={styles.center}>
            <div className={styles.stage} ref={stageRef}>
              {count > 0 ? (
                canvas(current, main.width, styles.mainSlide)
              ) : (
                <p className={styles.empty}>{t('slideEditor.empty')}</p>
              )}
            </div>
            {count > 0 && (
              <section className={styles.notes} aria-label={t('slideEditor.notes')}>
                {notes || <span className={styles.muted}>{t('slideEditor.noNotes')}</span>}
              </section>
            )}
          </div>
        </div>
      )}
      {view === 'sorter' && (
        <div className={styles.sorter}>
          {Array.from({ length: count }, (_, i) => (
            <button
              key={i}
              type="button"
              className={i === current ? styles.sortItemActive : styles.sortItem}
              onClick={() => setCurrent(i)}
              onDoubleClick={() => {
                setCurrent(i);
                setView('normal');
              }}
            >
              {canvas(i, 220)}
              <span>{i + 1}</span>
            </button>
          ))}
        </div>
      )}
      {view === 'reading' && (
        <div className={styles.reading} ref={stageRef}>
          {count > 0 && canvas(current, fit(slideSize, stage).width)}
        </div>
      )}
      {view === 'show' && (
        <div
          className={styles.show}
          ref={showRef}
          onClick={() => setCurrent((c) => step(c, count, 'next'))}
          onContextMenu={(e) => {
            e.preventDefault();
            setCurrent((c) => step(c, count, 'prev'));
          }}
        >
          {!black && current < count ? (
            <div key={current} className={styles.fade}>
              {canvas(
                current,
                fit(slideSize, { width: window.innerWidth, height: window.innerHeight }).width,
              )}
            </div>
          ) : (
            current >= count && <p className={styles.endText}>{t('slideEditor.end')}</p>
          )}
        </div>
      )}
      {view === 'presenter' && (
        <div className={styles.presenter}>
          <div className={styles.presenterMain} ref={stageRef}>
            {current < count && !black ? (
              canvas(current, main.width)
            ) : (
              <p className={styles.endText}>{t('slideEditor.end')}</p>
            )}
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
            {current + 1 < count ? canvas(current + 1, 280) : <p>{t('slideEditor.end')}</p>}
            <h3>{t('slideEditor.notes')}</h3>
            <div className={styles.presenterNotes}>{notes || t('slideEditor.noNotes')}</div>
            <div className={styles.presenterButtons}>
              <button type="button" onClick={() => setCurrent((c) => step(c, count, 'prev'))}>
                {t('slideEditor.previous')}
              </button>
              <button type="button" onClick={() => setCurrent((c) => step(c, count, 'next'))}>
                {t('slideEditor.next')}
              </button>
              <button type="button" onClick={() => setBlack((b) => !b)}>
                {t('slideEditor.blackScreen')}
              </button>
              <button type="button" onClick={endShow}>
                {t('slideEditor.exit')}
              </button>
            </div>
          </aside>
        </div>
      )}
      {view !== 'show' && (
        <StatusBar
          left={
            <span>
              {status ||
                (count > 0
                  ? t('slideEditor.slideOf', { n: Math.min(current + 1, count), total: count })
                  : '')}
            </span>
          }
          right={<span>{fileName}</span>}
        />
      )}
    </div>
  );
}
