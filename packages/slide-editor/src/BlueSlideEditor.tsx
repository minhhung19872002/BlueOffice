import { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import {
  CommandRegistry,
  MacroBanner,
  PasswordDialog,
  SavePasswordPanel,
  SymbolPanel,
} from '@blueoffice/editor';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import type {
  ClipboardContent,
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
import { PASSWORD_REQUIRED, WRONG_PASSWORD } from '@blueoffice/platform';
import {
  CommentThreads,
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
  Clapperboard,
  ClipboardPaste,
  Copy,
  Columns3,
  CopyPlus,
  CornerDownRight,
  Eraser,
  Eye,
  FileDown,
  FileImage,
  Files,
  KeyRound,
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
} from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from 'react';
import { AnimationPane } from './AnimationPane';
import { AudienceLink } from './audience';
import { AudienceView } from './AudienceView';
import { ChartDialog, type ChartDraft } from './ChartDialog';
import { SmartArtDialog, type SmartArtDraft } from './SmartArtDialog';
import { SlideController } from './controller';
import { fromEditorText, hexOf, nudgeCommand, toEditorText } from './editing';
import { ExportDialog, slidePng, type ExportPngOptions } from './ExportDialog';
import { MediaButtons } from './MediaLayer';
import { advance, type ShowPos } from './player';
import { fit, formatElapsed, showAction, step, type ShowAction } from './show';
import { ShowCache, ShowStage } from './ShowStage';
import { SlideCanvas } from './SlideCanvas';
import { zipStore } from './zip';
import styles from './SlideEditor.module.css';
import { SlideOverlay, type TextEditing } from './SlideOverlay';

export interface BlueSlideEditorProps {
  platform: Platform;
  language?: Language;
  /** Open as the audience screen of a presenter window (channel id). */
  audience?: string;
  /** Light, dark or follow the OS (the embedding host sets it, prompt §6A.2). */
  theme?: ThemeSetting;
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
  /** Transition kind of the current slide (`none` without one). */
  transition?: string;
}

const PPTX_MIME = 'application/vnd.openxmlformats-officedocument.presentationml.presentation';

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

type Pos = ShowPos & { from: number | null };

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
  const { t } = useTranslation();
  const engine = platform.engine;
  const theme = useResolvedTheme(themeSetting);
  const [info, setInfo] = useState<DocInfo | null>(null);
  /** The open deck has macros (kept, never run): show the warning bar. */
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
  const [peers, setPeers] = useState<SlidePeer[]>([]);
  /** An encrypted deck waiting for its password ([MS-OFFCRYPTO]). */
  const [passwordPrompt, setPasswordPrompt] = useState<{
    bytes: Uint8Array;
    name: string;
    wrong: boolean;
  } | null>(null);
  const [savePassword, setSavePassword] = useState(false);
  const [macroBanner, setMacroBanner] = useState(false);
  const [pos, setPos] = useState<Pos>({ slide: 0, click: 0, enter: 'jump', from: null });
  const [paneOpen, setPaneOpen] = useState(false);
  /** Comments side panel ([MS-PPTX] modern comments). */
  const [commentsOpen, setCommentsOpen] = useState(false);
  /** Symbol picker panel (inserts into the text being edited, else a new text box). */
  const [symbolsOpen, setSymbolsOpen] = useState(false);
  const [previewToken, setPreviewToken] = useState(0);
  const [ribbonTab, setRibbonTab] = useState('home');
  const [smartArtDialog, setSmartArtDialog] = useState<{
    mode: 'insert' | 'edit';
    initial: SmartArtDraft;
  } | null>(null);
  const [chartDialog, setChartDialog] = useState<{
    mode: 'insert' | 'edit';
    initial: ChartDraft;
  } | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [sortFrom, setSortFrom] = useState<number | null>(null);
  const [sortOver, setSortOver] = useState<number | null>(null);
  const showRef = useRef<HTMLDivElement>(null);
  const slideRef = useRef<HTMLDivElement>(null);
  const colorId = useId();
  const [colorTarget, setColorTarget] = useState<ColorTarget>('fill');
  const [stageRef, stage] = useSize();
  const glyphs = useMemo(() => new GlyphStore(engine), [engine]);
  const images = useMemo(() => new ImageStore(engine), [engine]);
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
    async (bytes: Uint8Array, name: string, password?: string) => {
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
      .getSlideNotes(Math.min(pos.slide, count - 1))
      .then((n) => !cancelled && setNotes(n))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [engine, info, pos.slide, count, view]);

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
      audience.update(pos.slide, black, false, pos.click, pos.enter);
    } else {
      audience.update(current, false, true);
      audience.stop();
    }
  }, [audience, view, current, black, pos]);

  const startShow = useCallback(
    (from: number, presenter: boolean) => {
      if (!count) return;
      setReturnView(view === 'show' || view === 'presenter' ? 'normal' : view);
      const first = Math.max(0, Math.min(from, count - 1));
      setCurrent(first);
      showCache.clear();
      setPos({ slide: first, click: 0, enter: 'transition', from: null });
      setBlack(false);
      setStartedAt(Date.now());
      setNow(Date.now());
      setEditing(null);
      setView(presenter ? 'presenter' : 'show');
    },
    [count, view, showCache],
  );

  const endShow = useCallback(() => {
    setView(returnView);
    const n = Math.min(pos.slide, Math.max(0, count - 1));
    setCurrent(n);
    run({ cmd: 'goTo', slide: n });
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
  }, [returnView, count, pos.slide, run]);

  /** Slide show navigation: clicks play the next effect, then the next slide. */
  const moveShow = useCallback(
    (a: ShowAction) => {
      setBlack(false);
      const clicks = (i: number) => showCache.peek(i)?.clicks ?? 0;
      setPos((p) => {
        if (a === 'first') return { slide: 0, click: 0, enter: 'jump', from: null };
        if (a === 'last') {
          const last = Math.max(0, count - 1);
          return { slide: last, click: clicks(last), enter: 'jump', from: null };
        }
        if (a !== 'next' && a !== 'prev') return p;
        const n = advance(p, a, count, clicks);
        return { ...n, from: n.enter === 'transition' ? p.slide : null };
      });
    },
    [count, showCache],
  );

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
      if (!after || s.slide !== pos.slide || pos.click < s.clicks) return;
      autoTimer.current = setTimeout(() => moveShow('next'), after);
    },
    [pos, moveShow],
  );
  /** A mouse click in the show (ignored at the end of a slide whose advClick is off). */
  const clickShow = useCallback(() => {
    const s = showCache.peek(pos.slide);
    if (s && s.transition && !s.transition.advanceOnClick && pos.click >= s.clicks) return;
    moveShow('next');
  }, [showCache, pos, moveShow]);

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
    async (sys: ClipboardContent | null) => {
      const mark = sys?.html ? new RegExp(`${CLIP_MARK}="([^"]+)"`).exec(sys.html)?.[1] : undefined;
      const external = sys && (sys.image || sys.text?.trim()) && !ctrl.isOwnClip(mark, sys.text);
      if (!external) {
        // The engine refuses (noClip) when its clipboard is empty.
        await ctrl.run({ cmd: 'paste' });
        return;
      }
      if (sys.image) await ctrl.insertPicture(sys.image, 'clipboard.png');
      else if (sys.text) await ctrl.run({ cmd: 'insertTextBox', text: fromEditorText(sys.text) });
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
      if (mode === 'insert') {
        run({ cmd: 'insertChart', kind: d.kind, title: d.title, legend: d.legend, data: d.data });
      } else {
        run({ cmd: 'setChart', kind: d.kind, title: d.title, legend: d.legend, data: d.data });
      }
    },
    [chartDialog, run],
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
          const f = await platform.files.openFile(['.pptx', '.ppt', '.odp']);
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
      id: 'file.savePassword',
      isEnabled: deck,
      run: () => setSavePassword(true),
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
    reg.register({ id: 'file.exportPng', isEnabled: deck, run: () => setExportOpen(true) });
    for (const v of ['normal', 'sorter', 'reading'] as const) {
      reg.register({
        id: `view.${v}`,
        isChecked: () => view === v,
        run: (s) => {
          if (v === 'reading') {
            // Reading view plays the slides like the show, in the window.
            showCache.clear();
            setPos({ slide: s.current, click: 0, enter: 'transition', from: null });
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
          await audience.start(s.current);
          startShow(s.current, true);
        }),
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
      id: 'review.comments',
      isEnabled: deck,
      isChecked: () => commentsOpen,
      run: async () => setCommentsOpen((o) => !o),
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
          const f = await platform.files.openFile(['.png', '.jpg', '.jpeg']);
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
    return reg;
  }, [
    fail,
    fileName,
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
    paneOpen,
    commentsOpen,
    newChart,
    openSmartArt,
    openChart,
    edit,
    pos.slide,
  ]);

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
      transition: edit?.transition?.kind ?? 'none',
    };
  }, [count, current, edit]);
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

  // Shortcuts typed inside the shape text (format.*, edit.*).
  const textShortcut = useCallback(
    (e: KeyboardEvent) => {
      const cmd = registry.match(e);
      if (!cmd || !/^(format|edit\.(undo|redo))/.test(cmd.id)) return false;
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
        const a = showAction(e.key);
        if (!a) return;
        e.preventDefault();
        if (a === 'exit') endShow();
        else if (a === 'black') setBlack((b) => !b);
        else moveShow(a);
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
  }, [view, count, current, registry, exec, endShow, edit, run, startEdit, goTo, moveShow]);

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
              item(
                'file.savePassword',
                t('commands.review.savePassword'),
                <KeyRound size={16} />,
                sm,
              ),
              item('file.exportPdf', t('slideEditor.exportPdf'), <FileDown size={16} />, sm),
              item('file.exportPng', t('slideEditor.exportPng'), <FileImage size={16} />, sm),
              item('file.print', t('slideEditor.print'), <Printer size={16} />, sm),
            ],
          },
          {
            id: 'clipboard',
            label: t('slideEditor.groupClipboard'),
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
              item('insert.smartart', t('slideEditor.smartart.button'), <Network size={18} />),
              item('insert.media', t('slideEditor.media'), <Clapperboard size={18} />),
            ],
          },
        ],
      },
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
      ...(state.isSmartArt
        ? [
            {
              id: 'smartart',
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
              ],
            },
          ]
        : []),
      ...(state.hasTable
        ? [
            {
              id: 'table',
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
              item('view.animationPane', t('slideEditor.animPane'), <PanelRight size={16} />, sm),
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
      {view !== 'show' && view !== 'presenter' && (
        <Ribbon
          tabs={ribbonTabs}
          moreLabel={t('ribbon.more')}
          value={ribbonTabs.some((x) => x.id === ribbonTab) ? ribbonTab : 'home'}
          onValueChange={(v) => {
            setRibbonTab(v);
            // The pane belongs to the Transitions / Animations tabs.
            if ((v === 'transitions' || v === 'animations') && view === 'normal') setPaneOpen(true);
          }}
        />
      )}
      {macroBanner && view === 'normal' && (
        <MacroBanner inline onDismiss={() => setMacroBanner(false)} />
      )}
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
                    onExitEdit={exitEdit}
                    onTextShortcut={textShortcut}
                    label={t('slideEditor.editor')}
                    editorLabel={t('slideEditor.textEditor')}
                    adjustLabel={t('slideEditor.adjustHandle')}
                    connectorLabel={t('slideEditor.connectorEnd')}
                    onStartCellEdit={startCellEdit}
                    peers={peers
                      .filter((p) => p.slide === shown)
                      .map((p) => {
                        const name = p.name || t('collab.anonymous');
                        return { ...p, name, label: t('collab.selected', { name }) };
                      })}
                  />
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
          {symbolsOpen && (
            <SymbolPanel
              fonts={[...new Set([edit?.format.font, ...FONTS].filter((x): x is string => !!x))]}
              font={edit?.format.font ?? undefined}
              onInsert={insertSymbol}
              onClose={() => setSymbolsOpen(false)}
            />
          )}
          {commentsOpen && count > 0 && (
            <CommentThreads
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
              }}
              newAnchor={t('slideEditor.slideOf', { n: shown + 1, total: count })}
              onAdd={(text) =>
                run({
                  cmd: 'addComment',
                  text,
                  author: platform.auth?.user?.name ?? '',
                  date: new Date().toISOString(),
                })
              }
              onReply={(parent, text) =>
                run({
                  cmd: 'addComment',
                  parent,
                  text,
                  author: platform.auth?.user?.name ?? '',
                  date: new Date().toISOString(),
                })
              }
              onResolve={(id, done) => run({ cmd: 'resolveComment', id, done })}
              onDelete={(id) => run({ cmd: 'deleteComment', id })}
              onClose={() => setCommentsOpen(false)}
            />
          )}
          {paneOpen && count > 0 && (
            <AnimationPane
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
          )}
        </div>
      )}
      {view === 'sorter' && (
        <div className={styles.sorter}>
          {Array.from({ length: count }, (_, i) => (
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
          ))}
        </div>
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
          {!black && pos.slide < count
            ? showStage(
                fit(slideSize, { width: window.innerWidth, height: window.innerHeight }).width,
              )
            : pos.slide >= count && <p className={styles.endText}>{t('slideEditor.end')}</p>}
        </div>
      )}
      {view === 'presenter' && (
        <div className={styles.presenter}>
          <div className={styles.presenterMain} ref={stageRef}>
            {pos.slide < count && !black ? (
              showStage(main.width)
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
            {pos.slide + 1 < count ? canvas(pos.slide + 1, 280) : <p>{t('slideEditor.end')}</p>}
            <h3>{t('slideEditor.notes')}</h3>
            <div className={styles.presenterNotes}>{notes || t('slideEditor.noNotes')}</div>
            <div className={styles.presenterButtons}>
              <button type="button" onClick={() => moveShow('prev')}>
                {t('slideEditor.previous')}
              </button>
              <button type="button" onClick={() => moveShow('next')}>
                {t('slideEditor.next')}
              </button>
              <button type="button" onClick={() => setBlack((b) => !b)}>
                {t('slideEditor.blackScreen')}
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
      {chartDialog && (
        <ChartDialog
          open
          mode={chartDialog.mode}
          initial={chartDialog.initial}
          onClose={() => setChartDialog(null)}
          onApply={applyChart}
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
