/**
 * Unsaved-changes tracking of a hosted file for the editor SDK's `dirty`
 * event (prompt §6A.2): a local command that changes the document makes the
 * session dirty, a saved host version makes it clean again. Remote changes of
 * other participants do not count (they save their own edits).
 */
import { overlay } from '@blueoffice/collab';
import type {
  EditResult,
  EngineHost,
  Platform,
  SheetCommand,
  SheetHost,
  SlideCommand,
  SlideEditResult,
  TimedCommand,
} from '@blueoffice/platform';
import { DOC_READ_COMMANDS, SHEET_READ_COMMANDS, SLIDE_READ_COMMANDS } from './readonly';

export class DirtyState {
  private value = false;
  private readonly listeners = new Set<(dirty: boolean) => void>();

  get dirty(): boolean {
    return this.value;
  }

  set(dirty: boolean): void {
    if (dirty === this.value) return;
    this.value = dirty;
    this.listeners.forEach((l) => l(dirty));
  }

  subscribe(fn: (dirty: boolean) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
}

/** Wraps the platform's engines: every successful document-changing call marks `state` dirty. */
export function trackDirty(p: Platform, state: DirtyState): Platform {
  const engine = p.engine;
  const mark = <T>(r: T): T => {
    state.set(true);
    return r;
  };
  const tracked = overlay(engine, {
    command: async (cmd: TimedCommand): Promise<EditResult> => {
      const r = await engine.command(cmd);
      // `setAuthor` runs when a document opens; it changes nothing the user sees.
      return DOC_READ_COMMANDS.has(cmd.cmd) || cmd.cmd === 'setAuthor' || r.error ? r : mark(r);
    },
    insertImage: async (bytes: Uint8Array, name: string): Promise<EditResult> =>
      mark(await engine.insertImage(bytes, name)),
    slideCommand: async (cmd: SlideCommand): Promise<SlideEditResult> => {
      const r = await engine.slideCommand(cmd);
      return SLIDE_READ_COMMANDS.has(cmd.cmd) || r.error ? r : mark(r);
    },
    slideInsertPicture: async (bytes: Uint8Array, name: string): Promise<SlideEditResult> =>
      mark(await engine.slideInsertPicture(bytes, name)),
  }) as EngineHost;
  const sheet = p.sheet;
  const trackedSheet: SheetHost | undefined = sheet
    ? (overlay(sheet, {
        setInputs: async (s: number, cells: [number, number, string][]) =>
          mark(await sheet.setInputs(s, cells)),
        structural: async (...args: Parameters<SheetHost['structural']>) =>
          mark(await sheet.structural(...args)),
        undo: async () => {
          const r = await sheet.undo();
          if (r) state.set(true);
          return r;
        },
        redo: async () => {
          const r = await sheet.redo();
          if (r) state.set(true);
          return r;
        },
        addSheet: async (name?: string) => mark(await sheet.addSheet(name)),
        renameSheet: async (s: number, name: string) => mark(await sheet.renameSheet(s, name)),
        deleteSheet: async (s: number) => mark(await sheet.deleteSheet(s)),
        exec: async <T = unknown>(cmd: SheetCommand): Promise<T> => {
          const r = await sheet.exec<T>(cmd);
          return SHEET_READ_COMMANDS.has(cmd.cmd) ? r : mark(r);
        },
      }) as SheetHost)
    : undefined;
  return { ...p, engine: tracked, ...(trackedSheet ? { sheet: trackedSheet } : {}) };
}
