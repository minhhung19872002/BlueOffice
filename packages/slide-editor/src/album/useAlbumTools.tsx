import { toBase64 } from '@blueoffice/editor';
import type { EngineHost, Platform, SlideReuseSource } from '@blueoffice/platform';
import { useCallback, useState, type ReactNode } from 'react';
import type { SlideController } from '../controller';
import { ALBUM_FILES, buildAlbum, type AlbumPicture } from './album';
import { AlbumDialog, type AlbumRequest } from './AlbumDialog';
import { OutlineDialog } from './OutlineDialog';
import { importOutline, OUTLINE_FILES, outlineOfFile, type OutlineEntry } from './outlineImport';
import { REUSE_FILES, ReusePane } from './ReusePane';

type T = (key: string, opts?: Record<string, unknown>) => string;

function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** A file name for a presentation titled `title`. */
export function deckFileName(title: string): string {
  const base = title
    .replace(/[\\/:*?"<>|\u0000-\u001f]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
  return `${base || 'album'}.pptx`;
}

/** Thumbnail width (pixels) of the reuse pane. */
const REUSE_THUMB = 192;

interface ReuseState {
  source: SlideReuseSource | null;
  busy: boolean;
  error: boolean;
}

/**
 * Photo album, slides from an outline and the reuse slides pane of BlueSlide
 * (SLIDE-ALBUM-001): their state, the actions behind the commands and their UI.
 */
export function useAlbumTools({
  t,
  platform,
  engine,
  ctrl,
  load,
  hasDeck,
  modified,
  slideSize,
  setStatus,
  fail,
}: {
  t: T;
  platform: Platform;
  /** The engine host (queries that are not edits go to it directly). */
  engine: EngineHost;
  ctrl: SlideController;
  /** Opens a presentation in the editor. */
  load: (bytes: Uint8Array, name: string) => Promise<void>;
  hasDeck: boolean;
  modified: boolean;
  slideSize: { width: number; height: number };
  setStatus: (message: string) => void;
  fail: (e: unknown) => void;
}) {
  const [albumOpen, setAlbumOpen] = useState(false);
  const [outline, setOutline] = useState<{
    name: string;
    entries: OutlineEntry[] | null;
  } | null>(null);
  const [reuseOpen, setReuseOpen] = useState(false);
  const [reuse, setReuse] = useState<ReuseState>({ source: null, busy: false, error: false });

  const pickPictures = useCallback(async (): Promise<AlbumPicture[]> => {
    if (platform.files.openFiles) return platform.files.openFiles(ALBUM_FILES);
    const f = await platform.files.openFile(ALBUM_FILES);
    return f ? [f] : [];
  }, [platform]);

  const createAlbum = useCallback(
    async (req: AlbumRequest) => {
      let size = ctrl.last?.info.pages[0] ?? slideSize;
      if (req.target === 'new') {
        await ctrl.idle();
        await engine.init();
        const made = await engine.slideCommand({ cmd: 'newDeck', title: req.title });
        if (!made.file) throw new Error(made.error ?? 'newDeck');
        await load(fromBase64(made.file), deckFileName(req.title));
        // The album's name on the title slide.
        const all = await ctrl.run({ cmd: 'selectAll' });
        const title = all?.selection.find(
          (s) => s.placeholder === 'ctrTitle' || s.placeholder === 'title',
        );
        if (title) await ctrl.run({ cmd: 'setText', id: title.id, text: req.title });
        await ctrl.run({ cmd: 'select', ids: [] });
        size = all?.info.pages[0] ?? size;
      }
      const done = await buildAlbum(
        { run: (c) => ctrl.run(c), insertPicture: (b, n) => ctrl.insertPicture(b, n) },
        req.pictures,
        req.options,
        size,
      );
      // Files that were left out are named (the status line otherwise keeps the slide number).
      if (done.skipped.length)
        setStatus(
          t('slideEditor.album.doneSkipped', {
            pictures: done.pictures,
            slides: done.slides,
            skipped: done.skipped.join(', '),
          }),
        );
    },
    [ctrl, engine, load, slideSize, setStatus, t],
  );

  const pickOutline = useCallback(async () => {
    const f = await platform.files.openFile(OUTLINE_FILES);
    if (!f) return;
    const entries = await outlineOfFile(f.name, f.bytes).catch(() => null);
    setOutline({ name: f.name, entries });
  }, [platform]);

  const insertOutline = useCallback(
    async (entries: OutlineEntry[]) => {
      const after = ctrl.last?.slide ?? 0;
      await importOutline((c) => ctrl.run(c), entries, after);
    },
    [ctrl],
  );

  const openReuseFile = useCallback(async () => {
    const f = await platform.files.openFile(REUSE_FILES);
    if (!f) return;
    setReuse((r) => ({ ...r, busy: true, error: false }));
    try {
      await ctrl.idle();
      await engine.init();
      const r = await engine.slideCommand({
        cmd: 'reuseOpen',
        name: f.name,
        base64: toBase64(f.bytes),
        width: REUSE_THUMB,
      });
      setReuse({ source: r.reuse ?? null, busy: false, error: !r.reuse });
    } catch {
      setReuse({ source: null, busy: false, error: true });
    }
  }, [platform, ctrl, engine]);

  const closeReuse = useCallback(() => {
    setReuseOpen(false);
    setReuse({ source: null, busy: false, error: false });
    // The engine forgets the other presentation.
    void ctrl
      .idle()
      .then(() => engine.slideCommand({ cmd: 'reuseClose' }))
      .catch(() => undefined);
  }, [ctrl, engine]);

  const toggleReuse = useCallback(() => {
    if (reuseOpen) closeReuse();
    else setReuseOpen(true);
  }, [reuseOpen, closeReuse]);

  const insertReused = useCallback(
    (indices: number[], keepSource: boolean) => {
      void ctrl.run({ cmd: 'reuseSlides', indices, keepSource });
    },
    [ctrl],
  );

  const dialogs: ReactNode = (
    <>
      {albumOpen && (
        <AlbumDialog
          t={t}
          pick={pickPictures}
          canAppend={hasDeck}
          unsaved={hasDeck && modified}
          onClose={() => setAlbumOpen(false)}
          onCreate={(req) => void createAlbum(req).catch(fail)}
        />
      )}
      {outline && (
        <OutlineDialog
          t={t}
          name={outline.name}
          entries={outline.entries}
          onClose={() => setOutline(null)}
          onInsert={(entries) => void insertOutline(entries).catch(fail)}
        />
      )}
    </>
  );

  const reusePane: ReactNode = (
    <ReusePane
      embedded
      t={t}
      source={reuse.source}
      busy={reuse.busy}
      error={reuse.error}
      canInsert={hasDeck}
      onOpen={() => void openReuseFile().catch(fail)}
      onInsert={insertReused}
      onClose={closeReuse}
    />
  );

  return {
    /** Insert › Photo album. */
    openAlbum: useCallback(() => setAlbumOpen(true), []),
    /** New slide › Slides from outline: asks for the file, then shows what it gives. */
    pickOutline,
    /** New slide › Reuse slides: shows / hides the pane. */
    toggleReuse,
    closeReuse,
    reuseOpen,
    dialogs,
    reusePane,
  };
}
