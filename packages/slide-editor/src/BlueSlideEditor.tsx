import { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import { CommandRegistry } from '@blueoffice/editor';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import type {
  DocInfo,
  Platform,
  SlideCommand,
  SlideEditResult,
  SlidePreview,
  SlideShapeInfo,
} from '@blueoffice/platform';
import { Ribbon, StatusBar, useResolvedTheme, type RibbonTab } from '@blueoffice/ui';
import {
  AArrowDown,
  AArrowUp,
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
  ArrowRight,
  ArrowUp,
  Baseline,
  Bold,
  BookOpen,
  BringToFront,
  Circle,
  Copy,
  CopyPlus,
  FileDown,
  FolderOpen,
  Group,
  ImagePlus,
  Italic,
  LayoutGrid,
  List,
  MonitorPlay,
  PaintBucket,
  PenLine,
  Play,
  Presentation,
  Printer,
  Redo2,
  Rows3,
  Save,
  SendToBack,
  Slash,
  Square,
  SquarePlus,
  Table,
  Trash,
  Triangle,
  Type,
  Underline,
  Undo2,
  Ungroup,
} from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from 'react';
import { SlideController } from './controller';
import { fromEditorText, hexOf, nudgeCommand, toEditorText } from './editing';
import { fit, formatElapsed, showAction, step } from './show';
import { SlideCanvas } from './SlideCanvas';
import styles from './SlideEditor.module.css';
import { SlideOverlay, type TextEditing } from './SlideOverlay';

export interface BlueSlideEditorProps {
  platform: Platform;
  language?: Language;
}

export type SlideView = 'normal' | 'sorter' | 'reading' | 'show' | 'presenter';

/** State passed to registry commands. */
export interface SlideEditorState {
  hasDeck: boolean;
  current: number;
  hasSelection: boolean;
  multi: boolean;
  hasText: boolean;
  isGroup: boolean;
  canUndo: boolean;
  canRedo: boolean;
}

const PPTX_MIME = 'application/vnd.openxmlformats-officedocument.presentationml.presentation';

type ColorTarget = 'font' | 'fill' | 'line';

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
  const showRef = useRef<HTMLDivElement>(null);
  const slideRef = useRef<HTMLDivElement>(null);
  const colorId = useId();
  const [colorTarget, setColorTarget] = useState<ColorTarget>('fill');
  const [stageRef, stage] = useSize();
  const glyphs = useMemo(() => new GlyphStore(engine), [engine]);
  const images = useMemo(() => new ImageStore(engine), [engine]);

  const count = info?.pageCount ?? 0;
  const slideSize = info?.pages[0] ?? { width: 960, height: 540 };
  const modified = edit?.modified ?? false;

  const fail = useCallback(
    (e: unknown) => setError(e instanceof Error ? e.message : String(e)),
    [],
  );

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
  const run = useCallback((cmd: SlideCommand) => void ctrl.run(cmd), [ctrl]);

  const load = useCallback(
    async (bytes: Uint8Array, name: string) => {
      setStatus(t('slideEditor.loading'));
      setError('');
      await engine.init();
      const doc = await engine.open(bytes, name);
      images.clear();
      setEditing(null);
      setNotesDraft(null);
      setInfo(doc);
      setFileName(name);
      setCurrent(0);
      setRevs([]);
      setEpoch((e) => e + 1);
      setStatus('');
      platform.window.setTitle(`${name} — ${t('app.slide')}`);
      await ctrl.run({ cmd: 'state' });
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
      .getSlideNotes(Math.min(current, count - 1))
      .then((n) => !cancelled && setNotes(n))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [engine, info, current, count, view]);

  // Clock for the presenter view.
  useEffect(() => {
    if (view !== 'presenter') return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [view]);

  const goTo = useCallback(
    (i: number) => {
      const n = Math.max(0, Math.min(i, count - 1));
      setCurrent(n);
      setEditing(null);
      run({ cmd: 'goTo', slide: n });
    },
    [count, run],
  );

  const startShow = useCallback(
    (from: number, presenter: boolean) => {
      if (!count) return;
      setReturnView(view === 'show' || view === 'presenter' ? 'normal' : view);
      setCurrent(Math.max(0, Math.min(from, count - 1)));
      setBlack(false);
      setStartedAt(Date.now());
      setNow(Date.now());
      setEditing(null);
      setView(presenter ? 'presenter' : 'show');
    },
    [count, view],
  );

  const endShow = useCallback(() => {
    setView(returnView);
    const n = Math.min(current, Math.max(0, count - 1));
    run({ cmd: 'goTo', slide: n });
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
  }, [returnView, count, current, run]);

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

  const startEdit = useCallback(
    (shape: SlideShapeInfo) => {
      if (!shape.editableText) return;
      setEditing({
        id: shape.id,
        text: shape.text ?? '',
        rect: shape.rect,
        size: edit?.format.size ?? 18,
      });
    },
    [edit],
  );

  const commitText = useCallback(
    (id: number, text: string) => {
      setEditing(null);
      const orig = edit?.selection.find((s) => s.id === id)?.text ?? '';
      if (text !== orig) run({ cmd: 'setText', id, text });
      slideRef.current?.focus();
    },
    [edit, run],
  );

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
          const f = await platform.files.openFile(['.pptx']);
          if (f) await load(f.bytes, f.name);
        }),
    });
    reg.register({
      id: 'file.save',
      shortcut: 'Ctrl+S',
      isEnabled: deck,
      run: () =>
        guard(async () => {
          await ctrl.idle();
          const saved = await platform.files.saveFile(
            await engine.savePptx(),
            `${base()}.pptx`,
            PPTX_MIME,
          );
          if (saved) {
            await engine.markSaved();
            await ctrl.run({ cmd: 'state' });
            setStatus(t('slideEditor.saved', { name: saved }));
          }
        }),
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
      run: () => guard(async () => platform.print.printPdf(await engine.exportPdf(), base())),
    });
    for (const v of ['normal', 'sorter', 'reading'] as const) {
      reg.register({ id: `view.${v}`, isChecked: () => view === v, run: () => setView(v) });
    }
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
    reg.register({ id: 'format.fill', isEnabled: sel, run: () => pickColor('fill') });
    simple('format.noFill', { cmd: 'setFill', color: null }, sel);
    reg.register({ id: 'format.line', isEnabled: sel, run: () => pickColor('line') });

    // Insert.
    simple('insert.textBox', { cmd: 'insertTextBox', text: '' }, deck);
    for (const [id, preset] of [
      ['insert.rect', 'rect'],
      ['insert.ellipse', 'ellipse'],
      ['insert.arrow', 'rightArrow'],
      ['insert.triangle', 'triangle'],
    ] as const) {
      simple(id, { cmd: 'insertShape', preset }, deck);
    }
    simple('insert.table', { cmd: 'insertTable', rows: 3, cols: 3 }, deck);
    reg.register({
      id: 'insert.picture',
      isEnabled: deck,
      run: () =>
        guard(async () => {
          const f = await platform.files.openFile(['.png', '.jpg', '.jpeg']);
          if (f) await ctrl.insertPicture(f.bytes, f.name);
        }),
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
    return reg;
  }, [fail, fileName, t, platform, load, engine, view, startShow, run, ctrl, pickColor]);

  const state: SlideEditorState = useMemo(() => {
    const s = edit?.selection ?? [];
    return {
      hasDeck: count > 0,
      current,
      hasSelection: s.length > 0,
      multi: s.length > 1,
      hasText: s.some((x) => x.editableText),
      isGroup: s.some((x) => x.kind === 'group'),
      canUndo: edit?.canUndo ?? false,
      canRedo: edit?.canRedo ?? false,
    };
  }, [count, current, edit]);
  const exec = useCallback((id: string) => void registry.execute(id, state), [registry, state]);

  // Keyboard: shortcuts everywhere, editing keys on the slide, navigation in views and the show.
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
  }, [view, count, current, registry, exec, endShow, edit, run, startEdit, goTo]);

  const ribbonTabs: RibbonTab[] = useMemo(() => {
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
              item('file.exportPdf', t('slideEditor.exportPdf'), <FileDown size={16} />, sm),
              item('file.print', t('slideEditor.print'), <Printer size={16} />, sm),
            ],
          },
          {
            id: 'slides',
            label: t('slideEditor.groupSlides'),
            items: [
              item('slide.new', t('slideEditor.newSlide'), <SquarePlus size={18} />),
              item('slide.duplicate', t('slideEditor.duplicateSlide'), <CopyPlus size={16} />, sm),
              item('slide.delete', t('slideEditor.deleteSlide'), <Trash size={16} />, sm),
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
              item('format.color', t('slideEditor.fontColor'), <Baseline size={16} />, sm),
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
            id: 'show',
            label: t('slideEditor.groupShow'),
            items: [
              item('show.fromStart', t('slideEditor.fromStart'), <Play size={18} />),
              item('show.fromCurrent', t('slideEditor.fromCurrent'), <MonitorPlay size={16} />, sm),
              item('show.presenter', t('slideEditor.presenter'), <Presentation size={16} />, sm),
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
            items: [item('insert.textBox', t('slideEditor.textBox'), <Type size={18} />)],
          },
          {
            id: 'shapes',
            label: t('slideEditor.groupShapes'),
            items: [
              item('insert.rect', t('slideEditor.rect'), <Square size={16} />, sm),
              item('insert.ellipse', t('slideEditor.ellipse'), <Circle size={16} />, sm),
              item('insert.arrow', t('slideEditor.arrow'), <ArrowRight size={16} />, sm),
              item('insert.triangle', t('slideEditor.triangle'), <Triangle size={16} />, sm),
            ],
          },
          {
            id: 'media',
            label: t('slideEditor.groupMedia'),
            items: [
              item('insert.picture', t('slideEditor.picture'), <ImagePlus size={18} />),
              item('insert.table', t('slideEditor.table'), <Table size={18} />),
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
  }, [t, registry, state, exec, edit]);

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

  const shown = Math.min(current, Math.max(0, count - 1));
  const thumbs = (
    <nav className={styles.thumbs} aria-label={t('slideEditor.thumbnails')} data-slide-thumbs>
      {Array.from({ length: count }, (_, i) => (
        <button
          key={i}
          type="button"
          draggable
          data-testid={`thumb-${i + 1}`}
          className={`${i === shown ? styles.thumbActive : styles.thumb} ${
            dropOver === i && dragFrom !== null && dragFrom !== i ? styles.thumbDrop : ''
          }`}
          aria-current={i === shown}
          onClick={() => goTo(i)}
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
        </button>
      ))}
      {count > 0 && (
        <button type="button" className={styles.thumbAdd} onClick={() => exec('slide.new')}>
          {t('slideEditor.addSlide')}
        </button>
      )}
    </nav>
  );

  const main = fit(slideSize, { width: stage.width - 32, height: stage.height - 32 });
  const elapsed = formatElapsed(now - startedAt);
  const message = error
    ? t('slideEditor.error', { message: error })
    : refused
      ? t('slideEditor.refused', { code: refused })
      : status;

  return (
    <div className={`bo-root ${styles.root}`} data-theme={theme} data-app="slide">
      {view !== 'show' && view !== 'presenter' && <Ribbon tabs={ribbonTabs} />}
      {view === 'normal' && (
        <div className={styles.normal}>
          {thumbs}
          <div className={styles.center}>
            <div className={styles.stage} ref={stageRef}>
              {count > 0 ? (
                <div
                  className={styles.slideWrap}
                  ref={slideRef}
                  tabIndex={-1}
                  style={{ width: main.width, height: main.height }}
                >
                  {canvas(shown, main.width, styles.mainSlide)}
                  <SlideOverlay
                    slide={info?.pages[shown] ?? slideSize}
                    width={main.width}
                    edit={edit}
                    preview={preview}
                    controller={ctrl}
                    editing={editing}
                    onStartEdit={startEdit}
                    onCommitText={commitText}
                    label={t('slideEditor.editor')}
                    editorLabel={t('slideEditor.textEditor')}
                  />
                </div>
              ) : (
                <p className={styles.empty}>{t('slideEditor.empty')}</p>
              )}
            </div>
            {count > 0 && (
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
        </div>
      )}
      {view === 'sorter' && (
        <div className={styles.sorter}>
          {Array.from({ length: count }, (_, i) => (
            <button
              key={i}
              type="button"
              className={i === shown ? styles.sortItemActive : styles.sortItem}
              onClick={() => goTo(i)}
              onDoubleClick={() => {
                goTo(i);
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
          {count > 0 && canvas(shown, fit(slideSize, stage).width)}
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
          else if (target === 'fill') run({ cmd: 'setFill', color: hex });
          else run({ cmd: 'setLine', color: hex });
        }}
      />
      {view !== 'show' && (
        <StatusBar
          left={
            <span data-testid="slide-status">
              {message ||
                (count > 0 ? t('slideEditor.slideOf', { n: shown + 1, total: count }) : '')}
            </span>
          }
          right={
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
          }
        />
      )}
    </div>
  );
}
