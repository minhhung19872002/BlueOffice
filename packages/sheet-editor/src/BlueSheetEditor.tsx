import { GlyphStore, PX_PER_PT, renderPage } from '@blueoffice/canvas-host';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import type { Platform, SheetHost, SheetView, WorkbookInfo } from '@blueoffice/platform';
import { Ribbon, StatusBar, useResolvedTheme, type RibbonTab } from '@blueoffice/ui';
import {
  ClipboardPaste,
  Columns3,
  Copy,
  Eraser,
  FileDown,
  FilePlus,
  FolderOpen,
  Plus,
  Printer,
  Redo2,
  Rows3,
  Save,
  Scissors,
  Table2,
  Trash2,
  Undo2,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';
import { CommandRegistry } from '@blueoffice/editor';
import {
  a1,
  advance,
  cellsOf,
  move,
  parseA1,
  parseNumber,
  parseTsv,
  pasteCells,
  rangeA1,
  rangeOf,
  single,
  span,
  toTsv,
  type CellPos,
  type Direction,
  type Selection,
} from './grid';
import styles from './SheetEditor.module.css';

export interface BlueSheetEditorProps {
  platform: Platform;
  language?: Language;
}

/** Editor state passed to commands. */
export interface SheetEditorState {
  host: SheetHost;
  sheet: number;
  selection: Selection;
  hasWorkbook: boolean;
}

const XLSX_MIME = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';
const ZOOMS = [0.5, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 2];
/** Selections larger than this skip the status-bar statistics. */
const STATS_LIMIT = 2_000;

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
}

function SheetEditorInner({ platform }: BlueSheetEditorProps) {
  const { t, i18n } = useTranslation();
  const host = platform.sheet;
  const theme = useResolvedTheme('auto');
  const decimal: ',' | '.' = i18n.language === 'en-US' ? '.' : ',';

  const [info, setInfo] = useState<WorkbookInfo | null>(null);
  const [fileName, setFileName] = useState('');
  const [sheet, setSheet] = useState(0);
  const [selection, setSelection] = useState<Selection>(single({ row: 0, col: 0 }));
  const [scroll, setScroll] = useState({ x: 0, y: 0 });
  const [size, setSize] = useState({ w: 800, h: 500 });
  const [content, setContent] = useState({ w: 2000, h: 4000 });
  const [zoom, setZoom] = useState(1);
  const [edit, setEdit] = useState<EditState | null>(null);
  const [barText, setBarText] = useState('');
  const [selRect, setSelRect] = useState<{ x: number; y: number; w: number; h: number } | null>(
    null,
  );
  const [activeRect, setActiveRect] = useState<{
    x: number;
    y: number;
    w: number;
    h: number;
  } | null>(null);
  const [status, setStatus] = useState('');
  const [stats, setStats] = useState<{
    key: string;
    sum: number;
    count: number;
    numbers: number;
  } | null>(null);
  const [renaming, setRenaming] = useState<number | null>(null);
  const [revision, setRevision] = useState(0);
  const [loadSeq, setLoadSeq] = useState(0);

  const scrollerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const glyphs = useMemo(() => (host ? new GlyphStore(host) : null), [host]);
  const renderSeq = useRef(0);
  const dragging = useRef(false);
  const composing = useRef(false);

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

  const fail = useCallback(
    (e: unknown) =>
      setStatus(t('sheetEditor.error', { message: e instanceof Error ? e.message : String(e) })),
    [t],
  );

  // ------------------------------------------------------------- loading

  const loaded = useCallback(
    (wb: WorkbookInfo, name: string) => {
      setInfo(wb);
      setFileName(name);
      setSheet(0);
      setSelection(single({ row: 0, col: 0 }));
      setScroll({ x: 0, y: 0 });
      setLoadSeq((n) => n + 1);
      setRevision((r) => r + 1);
      setStatus('');
      platform.window.setTitle(`${name} — ${t('app.sheet')}`);
    },
    [platform, t],
  );

  useEffect(() => {
    if (!host) return;
    let cancelled = false;
    void host
      .newWorkbook(`${t('sheetEditor.untitled')}.xlsx`)
      .then((wb) => !cancelled && loaded(wb, `${t('sheetEditor.untitled')}.xlsx`))
      .catch(fail);
    const un = platform.shell.onOpenRequest((f) => {
      void host
        .open(f.bytes, f.name)
        .then((wb) => loaded(wb, f.name))
        .catch(fail);
    });
    return () => {
      cancelled = true;
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
        await glyphs.ensure(page);
        if (seq !== renderSeq.current) return;
        const canvas = canvasRef.current;
        const ctx = canvas?.getContext('2d');
        if (!canvas || !ctx) return;
        const dpr = window.devicePixelRatio || 1;
        canvas.width = Math.round(size.w * dpr);
        canvas.height = Math.round(size.h * dpr);
        canvas.style.width = `${size.w}px`;
        canvas.style.height = `${size.h}px`;
        renderPage(ctx, page, glyphs, scale * dpr);
      })().catch(fail);
    });
    return () => cancelAnimationFrame(frame);
  }, [host, glyphs, info, view, size, scale, revision, fail]);

  // Selection overlay + formula bar contents.
  useEffect(() => {
    if (!host || !info) return;
    const r = rangeOf(selection);
    let cancelled = false;
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
      const px = (x: { x: number; y: number; w: number; h: number } | null) =>
        x && { x: x.x * scale, y: x.y * scale, w: x.w * scale, h: x.h * scale };
      setSelRect(px(rect));
      setActiveRect(px(act));
      if (!edit) setBarText(cell.input);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, selection, view, scale, revision, edit]);

  // Status-bar statistics of the selection.
  useEffect(() => {
    if (!host || !info) return;
    const r = rangeOf(selection);
    if (cellsOf(r) <= 1 || cellsOf(r) > STATS_LIMIT) return;
    const key = `${sheet}:${rangeA1(r)}:${revision}`;
    let cancelled = false;
    void (async () => {
      const lastRow = info.sheets[sheet]?.lastRow ?? -1;
      const lastCol = info.sheets[sheet]?.lastCol ?? -1;
      let sum = 0;
      let count = 0;
      let numbers = 0;
      const jobs: Promise<void>[] = [];
      for (let row = r.r0; row <= Math.min(r.r1, lastRow); row++) {
        for (let col = r.c0; col <= Math.min(r.c1, lastCol); col++) {
          jobs.push(
            host.cell(sheet, row, col).then((c) => {
              if (c.display === '') return;
              count++;
              const n = c.input.startsWith('=')
                ? parseNumber(c.display, decimal)
                : parseNumber(c.input, decimal);
              if (n !== null) {
                sum += n;
                numbers++;
              }
            }),
          );
        }
      }
      await Promise.all(jobs);
      if (!cancelled) setStats({ key, sum, count, numbers });
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, info, sheet, selection, revision, decimal]);

  // Keep the hidden/visible cell input focused so IME composition starts in it.
  const focusInput = useCallback(() => {
    requestAnimationFrame(() => inputRef.current?.focus({ preventScroll: true }));
  }, []);

  // ------------------------------------------------------------- editing

  const refreshInfo = useCallback(async () => {
    if (!host) return;
    setInfo(await host.info());
    setRevision((r) => r + 1);
  }, [host]);

  const commit = useCallback(
    async (next?: Selection) => {
      if (!host || !edit) {
        if (next) setSelection(next);
        return;
      }
      const e = edit;
      setEdit(null);
      if (inputRef.current) inputRef.current.value = '';
      try {
        await host.setInputs(sheet, [[e.pos.row, e.pos.col, e.text]]);
        await refreshInfo();
      } catch (err) {
        fail(err);
      }
      if (next) setSelection(next);
      focusInput();
    },
    [host, edit, sheet, refreshInfo, fail, focusInput],
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
      const text = initial ?? (await host.cell(sheet, pos.row, pos.col)).input;
      setEdit({ pos, text, inPlace });
      setBarText(text);
      if (inputRef.current) inputRef.current.value = text;
      focusInput();
    },
    [host, selection, sheet, focusInput],
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
      void reveal(s);
    },
    [reveal],
  );

  const clearSelection = useCallback(async () => {
    if (!host) return;
    const r = rangeOf(selection);
    const lastRow = Math.min(r.r1, info?.sheets[sheet]?.lastRow ?? r.r1);
    const lastCol = Math.min(r.c1, info?.sheets[sheet]?.lastCol ?? r.c1);
    const cells: [number, number, string][] = [];
    for (let row = r.r0; row <= lastRow; row++)
      for (let col = r.c0; col <= lastCol; col++) cells.push([row, col, '']);
    if (cells.length) {
      await host.setInputs(sheet, cells);
      await refreshInfo();
    }
  }, [host, selection, info, sheet, refreshInfo]);

  const copy = useCallback(
    async (cut: boolean) => {
      if (!host) return;
      const r = rangeOf(selection);
      if (cellsOf(r) > 100_000) return;
      const rows: string[][] = [];
      for (let row = r.r0; row <= r.r1; row++) {
        const line: string[] = [];
        for (let col = r.c0; col <= r.c1; col++) {
          const c = await host.cell(sheet, row, col);
          line.push(c.input.startsWith('=') ? c.display : c.input);
        }
        rows.push(line);
      }
      await platform.clipboard.writeText(toTsv(rows));
      if (cut) await clearSelection();
    },
    [host, selection, sheet, platform, clearSelection],
  );

  const paste = useCallback(async () => {
    if (!host) return;
    const text = await platform.clipboard.readText();
    const rows = parseTsv(text);
    if (!rows.length) return;
    const r = rangeOf(selection);
    await host.setInputs(sheet, pasteCells({ row: r.r0, col: r.c0 }, rows));
    const lastRow = r.r0 + rows.length - 1;
    const lastCol = r.c0 + Math.max(...rows.map((x) => x.length)) - 1;
    setSelection(span({ row: r.r0, col: r.c0 }, { row: lastRow, col: lastCol }));
    await refreshInfo();
  }, [host, platform, selection, sheet, refreshInfo]);

  // ------------------------------------------------------------- commands

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
    reg.register({
      id: 'file.new',
      run: (st) =>
        guard(st, async (s) => {
          const name = `${t('sheetEditor.untitled')}.xlsx`;
          loaded(await s.host.newWorkbook(name), name);
        }),
    });
    reg.register({
      id: 'file.open',
      shortcut: 'Ctrl+O',
      run: (st) =>
        guard(st, async (s) => {
          const f = await platform.files.openFile([
            '.xlsx',
            '.xlsm',
            '.xlsb',
            '.xls',
            '.ods',
            '.csv',
          ]);
          if (!f) return;
          setStatus(t('sheetEditor.loading'));
          loaded(await s.host.open(f.bytes, f.name), f.name);
          await platform.shell.addRecentFile({
            name: f.name,
            path: f.path ?? f.name,
            openedAt: Date.now(),
          });
        }),
    });
    const baseName = () => fileName.replace(/\.(xlsx|xlsm|csv)$/i, '') || t('sheetEditor.untitled');
    reg.register({
      id: 'file.save',
      shortcut: 'Ctrl+S',
      isEnabled: (s) => s.hasWorkbook,
      run: (st) =>
        guard(st, async (s) => {
          const bytes = await s.host.save('xlsx', s.sheet);
          const saved = await platform.files.saveFile(bytes, `${baseName()}.xlsx`, XLSX_MIME);
          if (saved) setStatus(t('sheetEditor.saved', { name: saved }));
        }),
    });
    reg.register({
      id: 'file.saveCsv',
      isEnabled: (s) => s.hasWorkbook,
      run: (st) =>
        guard(st, async (s) => {
          const bytes = await s.host.save('csv', s.sheet);
          const saved = await platform.files.saveFile(bytes, `${baseName()}.csv`, 'text/csv');
          if (saved) setStatus(t('sheetEditor.saved', { name: saved }));
        }),
    });
    reg.register({
      id: 'file.exportPdf',
      isEnabled: (s) => s.hasWorkbook,
      run: (st) =>
        guard(st, async (s) => {
          const pdf = await s.host.exportPdf();
          await platform.files.saveFile(pdf, `${baseName()}.pdf`, 'application/pdf');
        }),
    });
    reg.register({
      id: 'file.print',
      shortcut: 'Ctrl+P',
      isEnabled: (s) => s.hasWorkbook,
      run: (st) =>
        guard(st, async (s) => platform.print.printPdf(await s.host.exportPdf(), baseName())),
    });
    reg.register({
      id: 'edit.undo',
      shortcut: 'Ctrl+Z',
      run: (st) =>
        guard(st, async (s) => {
          if (await s.host.undo()) await refreshInfo();
        }),
    });
    reg.register({
      id: 'edit.redo',
      shortcut: 'Ctrl+Y',
      run: (st) =>
        guard(st, async (s) => {
          if (await s.host.redo()) await refreshInfo();
        }),
    });
    reg.register({
      id: 'edit.copy',
      shortcut: 'Ctrl+C',
      run: (st) => guard(st, () => copy(false)),
    });
    reg.register({ id: 'edit.cut', shortcut: 'Ctrl+X', run: (st) => guard(st, () => copy(true)) });
    reg.register({ id: 'edit.paste', shortcut: 'Ctrl+V', run: (st) => guard(st, paste) });
    reg.register({ id: 'edit.clear', shortcut: 'Delete', run: (st) => guard(st, clearSelection) });
    const structural = (rows: boolean, remove: boolean) => (st: SheetEditorState) =>
      guard(st, async (s) => {
        const r = rangeOf(s.selection);
        const at = rows ? r.r0 : r.c0;
        const count = rows ? r.r1 - r.r0 + 1 : r.c1 - r.c0 + 1;
        await s.host.structural(s.sheet, rows, at, Math.min(count, 10_000), remove);
        await refreshInfo();
      });
    reg.register({ id: 'cells.insertRow', run: structural(true, false) });
    reg.register({ id: 'cells.insertCol', run: structural(false, false) });
    reg.register({ id: 'cells.deleteRow', run: structural(true, true) });
    reg.register({ id: 'cells.deleteCol', run: structural(false, true) });
    reg.register({
      id: 'sheet.add',
      run: (st) =>
        guard(st, async (s) => {
          const i = await s.host.addSheet();
          await refreshInfo();
          setSheet(i);
        }),
    });
    reg.register({
      id: 'sheet.rename',
      run: (st) => guard(st, async (s) => setRenaming(s.sheet)),
    });
    reg.register({
      id: 'sheet.delete',
      isEnabled: () => (info?.sheets.length ?? 0) > 1,
      run: (st) =>
        guard(st, async (s) => {
          await s.host.deleteSheet(s.sheet);
          setSheet(Math.max(0, s.sheet - 1));
          await refreshInfo();
        }),
    });
    reg.register({
      id: 'view.zoomIn',
      run: (st) => guard(st, async () => setZoom((z) => ZOOMS.find((v) => v > z) ?? z)),
    });
    reg.register({
      id: 'view.zoomOut',
      run: (st) =>
        guard(st, async () => setZoom((z) => [...ZOOMS].reverse().find((v) => v < z) ?? z)),
    });
    return reg;
  }, [t, fail, loaded, platform, fileName, refreshInfo, copy, paste, clearSelection, info]);

  const state: SheetEditorState | null = useMemo(
    () => (host ? { host, sheet, selection, hasWorkbook: !!info } : null),
    [host, sheet, selection, info],
  );
  const exec = useCallback(
    (id: string) => {
      if (state) void registry.execute(id, state);
    },
    [registry, state],
  );

  // ------------------------------------------------------------- input

  const navKey: Record<string, Direction> = {
    ArrowUp: 'up',
    ArrowDown: 'down',
    ArrowLeft: 'left',
    ArrowRight: 'right',
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (composing.current || e.nativeEvent.isComposing) return;
    if (edit) {
      if (e.key === 'Enter' && !e.altKey) {
        e.preventDefault();
        void commit(advance(selection, e.shiftKey ? 'up' : 'down'));
      } else if (e.key === 'Tab') {
        e.preventDefault();
        void commit(advance(selection, e.shiftKey ? 'left' : 'right'));
      } else if (e.key === 'Escape') {
        e.preventDefault();
        cancelEdit();
      } else if (navKey[e.key] && !edit.inPlace) {
        e.preventDefault();
        void commit(move(selection, navKey[e.key]!, false));
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      const last = info?.sheets[sheet];
      select(span({ row: 0, col: 0 }, { row: last?.lastRow ?? 0, col: last?.lastCol ?? 0 }));
      return;
    }
    const cmd = registry.match(e);
    if (cmd) {
      e.preventDefault();
      exec(cmd.id);
      return;
    }
    const dir = navKey[e.key];
    if (dir) {
      e.preventDefault();
      const step = e.ctrlKey || e.metaKey ? 50 : 1;
      select(move(selection, dir, e.shiftKey, step));
    } else if (e.key === 'PageDown' || e.key === 'PageUp') {
      e.preventDefault();
      const rows = Math.max(1, Math.floor(size.h / (20 * scale)));
      select(move(selection, e.key === 'PageDown' ? 'down' : 'up', e.shiftKey, rows));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      select(advance(selection, e.shiftKey ? 'up' : 'down'));
    } else if (e.key === 'Tab') {
      e.preventDefault();
      select(advance(selection, e.shiftKey ? 'left' : 'right'));
    } else if (e.key === 'F2') {
      e.preventDefault();
      void startEdit(true);
    } else if (e.key === 'Backspace') {
      e.preventDefault();
      void startEdit(false, '');
    }
  };

  // Typing (including IME output) lands in the input and switches to edit mode.
  const onInput = (text: string) => {
    if (edit) {
      setEdit({ ...edit, text });
      setBarText(text);
    } else {
      setEdit({ pos: selection.active, text, inPlace: false });
      setBarText(text);
    }
  };

  const hitAt = async (e: ReactMouseEvent) => {
    if (!host) return null;
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return null;
    return host.hitTest(view, (e.clientX - rect.left) / scale, (e.clientY - rect.top) / scale);
  };

  const onMouseDown = async (e: ReactMouseEvent) => {
    if (e.button !== 0) return;
    if (edit) await commit();
    const hit = await hitAt(e);
    if (!hit) return;
    const last = info?.sheets[sheet];
    if (hit.kind === 'cell') {
      const p = { row: hit.row, col: hit.col };
      setSelection(e.shiftKey ? { ...selection, focus: p } : single(p));
      dragging.current = true;
    } else if (hit.kind === 'columnHeader') {
      setSelection(
        span({ row: 0, col: hit.col }, { row: Math.max(last?.lastRow ?? 0, 999), col: hit.col }),
      );
    } else if (hit.kind === 'rowHeader') {
      setSelection(
        span({ row: hit.row, col: 0 }, { row: hit.row, col: Math.max(last?.lastCol ?? 0, 25) }),
      );
    } else if (hit.kind === 'corner') {
      setSelection(span({ row: 0, col: 0 }, { row: last?.lastRow ?? 0, col: last?.lastCol ?? 0 }));
    }
    focusInput();
  };

  const onMouseMove = async (e: ReactMouseEvent) => {
    if (!dragging.current) return;
    const hit = await hitAt(e);
    if (hit?.kind === 'cell') {
      setSelection((s) => ({ ...s, focus: { row: hit.row, col: hit.col } }));
    }
  };

  useEffect(() => {
    const up = () => (dragging.current = false);
    window.addEventListener('mouseup', up);
    return () => window.removeEventListener('mouseup', up);
  }, []);

  const onNameBox = (text: string) => {
    const s = parseA1(text);
    if (s) select(s);
    focusInput();
  };

  // ------------------------------------------------------------- view

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
      enabled: state ? registry.isEnabled(id, state) : false,
      onSelect: () => exec(id),
    });
    return [
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
              item('file.saveCsv', t('sheetEditor.saveCsv'), <Table2 size={16} />, 'small'),
              item('file.exportPdf', t('sheetEditor.exportPdf'), <FileDown size={16} />, 'small'),
              item('file.print', t('sheetEditor.print'), <Printer size={16} />, 'small'),
            ],
          },
          {
            id: 'edit',
            label: t('sheetEditor.groupEdit'),
            items: [
              item('edit.undo', t('sheetEditor.undo'), <Undo2 size={16} />, 'small'),
              item('edit.redo', t('sheetEditor.redo'), <Redo2 size={16} />, 'small'),
              item('edit.cut', t('sheetEditor.cut'), <Scissors size={16} />, 'small'),
              item('edit.copy', t('sheetEditor.copy'), <Copy size={16} />, 'small'),
              item('edit.paste', t('sheetEditor.paste'), <ClipboardPaste size={18} />),
              item('edit.clear', t('sheetEditor.clear'), <Eraser size={16} />, 'small'),
            ],
          },
          {
            id: 'cells',
            label: t('sheetEditor.groupCells'),
            items: [
              item('cells.insertRow', t('sheetEditor.insertRow'), <Rows3 size={16} />, 'small'),
              item('cells.insertCol', t('sheetEditor.insertCol'), <Columns3 size={16} />, 'small'),
              item('cells.deleteRow', t('sheetEditor.deleteRow'), <Trash2 size={16} />, 'small'),
              item('cells.deleteCol', t('sheetEditor.deleteCol'), <Trash2 size={16} />, 'small'),
            ],
          },
          {
            id: 'sheets',
            label: t('sheetEditor.groupSheets'),
            items: [
              item('sheet.add', t('sheetEditor.addSheet'), <Plus size={16} />, 'small'),
              item('sheet.rename', t('sheetEditor.renameSheet'), <Table2 size={16} />, 'small'),
              item('sheet.delete', t('sheetEditor.deleteSheet'), <Trash2 size={16} />, 'small'),
            ],
          },
          {
            id: 'zoom',
            label: t('sheetEditor.groupZoom'),
            items: [
              item('view.zoomIn', t('sheetEditor.zoomIn'), <ZoomIn size={16} />, 'small'),
              item('view.zoomOut', t('sheetEditor.zoomOut'), <ZoomOut size={16} />, 'small'),
            ],
          },
        ],
      },
    ];
  }, [t, registry, state, exec]);

  const r = rangeOf(selection);
  const shownStats = stats && stats.key === `${sheet}:${rangeA1(r)}:${revision}` ? stats : null;
  const fmt = (n: number) =>
    new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 10 }).format(n);

  if (!host) return null;

  return (
    <div className={styles.root} data-theme={theme} data-app="sheet">
      <Ribbon tabs={ribbonTabs} />
      <div className={styles.formulaBar}>
        <input
          className={styles.nameBox}
          aria-label={t('sheetEditor.nameBox')}
          value={rangeA1(r) === a1(selection.active) ? a1(selection.active) : rangeA1(r)}
          onChange={() => {}}
          onKeyDown={(e) => {
            if (e.key === 'Enter') onNameBox(e.currentTarget.value);
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
        <input
          className={styles.formulaInput}
          aria-label={t('sheetEditor.formulaBar')}
          value={barText}
          onFocus={() => {
            if (!edit) setEdit({ pos: selection.active, text: barText, inPlace: true });
          }}
          onChange={(e) => {
            setBarText(e.target.value);
            setEdit({ pos: edit?.pos ?? selection.active, text: e.target.value, inPlace: true });
            if (inputRef.current) inputRef.current.value = e.target.value;
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void commit(advance(selection, 'down'));
            } else if (e.key === 'Escape') {
              e.preventDefault();
              cancelEdit();
            }
          }}
        />
      </div>
      <div
        className={styles.scroller}
        ref={scrollerRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          setScroll({ x: el.scrollLeft / scale, y: el.scrollTop / scale });
        }}
      >
        <div className={styles.sticky}>
          <canvas
            ref={canvasRef}
            role="grid"
            aria-label={t('sheetEditor.grid')}
            onMouseDown={(e) => void onMouseDown(e)}
            onMouseMove={(e) => void onMouseMove(e)}
            onDoubleClick={() => void startEdit(true)}
          />
          {selRect && (
            <div
              className={styles.selection}
              style={{ left: selRect.x, top: selRect.y, width: selRect.w, height: selRect.h }}
            />
          )}
          <input
            ref={inputRef}
            className={edit ? styles.cellEditorActive : styles.cellEditor}
            aria-label={t('sheetEditor.cellEditor')}
            style={
              activeRect
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
            onInput={(e) => onInput(e.currentTarget.value)}
            onCompositionStart={() => {
              composing.current = true;
              if (!edit) setEdit({ pos: selection.active, text: '', inPlace: false });
            }}
            onCompositionEnd={(e) => {
              composing.current = false;
              onInput(e.currentTarget.value);
            }}
          />
        </div>
        <div style={{ width: content.w * scale, height: content.h * scale }} />
      </div>
      <div className={styles.tabs} role="tablist" aria-label={t('sheetEditor.tabs')}>
        {info?.sheets.map((s, i) =>
          s.hidden ? null : renaming === i ? (
            <input
              key={i}
              className={styles.tabRename}
              defaultValue={s.name}
              autoFocus
              onBlur={() => setRenaming(null)}
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
              onClick={() => {
                setSheet(i);
                setSelection(single({ row: 0, col: 0 }));
                scrollerRef.current?.scrollTo(0, 0);
                focusInput();
              }}
              onDoubleClick={() => setRenaming(i)}
            >
              {s.name}
            </button>
          ),
        )}
        <button
          type="button"
          className={styles.tabAdd}
          aria-label={t('sheetEditor.addSheet')}
          onClick={() => exec('sheet.add')}
        >
          <Plus size={14} />
        </button>
      </div>
      <StatusBar
        left={<span>{status || (edit ? t('sheetEditor.edit') : t('sheetEditor.ready'))}</span>}
        right={
          <>
            {shownStats && shownStats.numbers > 0 && (
              <>
                <span>
                  {t('sheetEditor.average')}: {fmt(shownStats.sum / shownStats.numbers)}
                </span>
                <span>
                  {t('sheetEditor.sum')}: {fmt(shownStats.sum)}
                </span>
              </>
            )}
            {shownStats && (
              <span>
                {t('sheetEditor.count')}: {shownStats.count}
              </span>
            )}
            <span>{Math.round(zoom * 100)}%</span>
          </>
        }
      />
    </div>
  );
}
