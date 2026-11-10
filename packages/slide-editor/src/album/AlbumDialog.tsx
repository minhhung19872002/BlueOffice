import { Dialog } from '@blueoffice/ui';
import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ALBUM_FRAMES,
  ALBUM_LAYOUTS,
  effectiveOptions,
  perSlide,
  type AlbumFrame,
  type AlbumLayout,
  type AlbumOptions,
  type AlbumPicture,
} from './album';
import styles from './album.module.css';

type T = (key: string, opts?: Record<string, unknown>) => string;

/** Where the album goes: after the current slide, or into a new presentation. */
export type AlbumTarget = 'append' | 'new';

export interface AlbumRequest {
  pictures: AlbumPicture[];
  options: AlbumOptions;
  target: AlbumTarget;
  /** Title of a new presentation's first slide. */
  title: string;
}

interface Item extends AlbumPicture {
  id: number;
  url: string | null;
}

function objectUrl(p: AlbumPicture): string | null {
  try {
    if (typeof URL.createObjectURL !== 'function') return null;
    return URL.createObjectURL(new Blob([p.bytes as BlobPart]));
  } catch {
    return null;
  }
}

/** `list` with the item at `from` moved to `to`. */
export function moved<V>(list: V[], from: number, to: number): V[] {
  if (from === to || from < 0 || to < 0 || from >= list.length || to >= list.length) return list;
  const out = [...list];
  const [it] = out.splice(from, 1);
  out.splice(to, 0, it!);
  return out;
}

/**
 * Photo album (Insert › Photo album): pick pictures, put them in order, choose
 * the slide layout, captions and frame. Everything is reachable with the
 * keyboard: the picture rows are buttons, Alt+↑ / Alt+↓ moves the focused row.
 */
export function AlbumDialog({
  t,
  pick,
  canAppend,
  unsaved,
  onClose,
  onCreate,
}: {
  t: T;
  /** Asks the user for pictures (several at once where the host can). */
  pick: () => Promise<AlbumPicture[]>;
  /** A presentation is open: the album can follow its current slide. */
  canAppend: boolean;
  /** The open presentation has unsaved changes (a new one would replace it). */
  unsaved: boolean;
  onClose: () => void;
  onCreate: (request: AlbumRequest) => void;
}) {
  const [items, setItems] = useState<Item[]>([]);
  const nextId = useRef(1);
  const [layout, setLayout] = useState<AlbumLayout>('one');
  const [titles, setTitles] = useState(true);
  const [captions, setCaptions] = useState(false);
  const [frame, setFrame] = useState<AlbumFrame>('rect');
  const [target, setTarget] = useState<AlbumTarget>(canAppend ? 'append' : 'new');
  const [title, setTitle] = useState(t('slideEditor.album.defaultTitle'));
  const [picking, setPicking] = useState(false);
  const listRef = useRef<HTMLOListElement>(null);
  /** Row and button to focus after a move (the row's buttons are re-rendered in place). */
  const focusAfter = useRef<{ id: number; action: string } | null>(null);

  // Thumbnails are object URLs: released when the dialog closes.
  const urls = useRef<string[]>([]);
  useEffect(
    () => () => {
      for (const u of urls.current) URL.revokeObjectURL?.(u);
    },
    [],
  );

  useEffect(() => {
    const f = focusAfter.current;
    if (!f) return;
    focusAfter.current = null;
    const row = listRef.current?.querySelector(`[data-album-item="${f.id}"]`);
    const btn =
      row?.querySelector<HTMLButtonElement>(`[data-action="${f.action}"]:not(:disabled)`) ??
      row?.querySelector<HTMLButtonElement>('button:not(:disabled)');
    btn?.focus();
  }, [items]);

  const add = async () => {
    setPicking(true);
    try {
      const picked = await pick();
      if (picked.length === 0) return;
      setItems((old) => [
        ...old,
        ...picked.map((p) => {
          const url = objectUrl(p);
          if (url) urls.current.push(url);
          return { ...p, id: nextId.current++, url };
        }),
      ]);
    } finally {
      setPicking(false);
    }
  };

  const move = (i: number, by: number, action: string) => {
    const to = i + by;
    if (to < 0 || to >= items.length) return;
    focusAfter.current = { id: items[i]!.id, action };
    setItems((old) => moved(old, i, to));
  };
  const remove = (i: number) => {
    const next = items[i + 1] ?? items[i - 1];
    focusAfter.current = next ? { id: next.id, action: 'remove' } : null;
    setItems((old) => old.filter((_, k) => k !== i));
  };

  const options = useMemo(
    () => effectiveOptions({ layout, titles, captions, frame }),
    [layout, titles, captions, frame],
  );
  const slides = Math.ceil(items.length / perSlide(layout));
  const fit = layout === 'fit';

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideEditor.album.title')}
      testId="album-dialog"
      width={520}
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="album-create"
            disabled={items.length === 0}
            onClick={() => {
              onCreate({
                pictures: items.map(({ name, bytes }) => ({ name, bytes })),
                options,
                target,
                title: title.trim() || t('slideEditor.album.defaultTitle'),
              });
              onClose();
            }}
          >
            {t('slideEditor.album.create')}
          </button>
        </>
      }
    >
      <div className={styles.row}>
        <button type="button" data-testid="album-add" disabled={picking} onClick={() => void add()}>
          {t('slideEditor.album.add')}
        </button>
        <span className={styles.note} data-testid="album-summary" aria-live="polite">
          {items.length === 0
            ? t('slideEditor.album.empty')
            : t('slideEditor.album.summary', { pictures: items.length, slides })}
        </span>
      </div>
      <ol className={styles.pictures} ref={listRef} aria-label={t('slideEditor.album.pictures')}>
        {items.map((it, i) => (
          <li
            key={it.id}
            className={styles.picture}
            data-album-item={it.id}
            data-testid={`album-item-${i}`}
            onKeyDown={(e) => {
              if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
              e.preventDefault();
              const action = (e.target as HTMLElement).getAttribute('data-action') ?? 'up';
              move(i, e.key === 'ArrowUp' ? -1 : 1, action);
            }}
          >
            {it.url ? (
              <img className={styles.pictureThumb} src={it.url} alt="" />
            ) : (
              <span className={styles.pictureThumb} aria-hidden />
            )}
            <span className={styles.pictureName} title={it.name}>
              {i + 1}. {it.name}
            </span>
            <button
              type="button"
              data-action="up"
              disabled={i === 0}
              aria-label={t('slideEditor.album.moveUp', { name: it.name })}
              title={t('slideEditor.album.moveUp', { name: it.name })}
              onClick={() => move(i, -1, 'up')}
            >
              ↑
            </button>
            <button
              type="button"
              data-action="down"
              disabled={i === items.length - 1}
              aria-label={t('slideEditor.album.moveDown', { name: it.name })}
              title={t('slideEditor.album.moveDown', { name: it.name })}
              onClick={() => move(i, 1, 'down')}
            >
              ↓
            </button>
            <button
              type="button"
              data-action="remove"
              aria-label={t('slideEditor.album.remove', { name: it.name })}
              title={t('slideEditor.album.remove', { name: it.name })}
              onClick={() => remove(i)}
            >
              ×
            </button>
          </li>
        ))}
      </ol>
      <div className={styles.options}>
        <label>
          <span>{t('slideEditor.album.layout')}</span>
          <select
            data-testid="album-layout"
            value={layout}
            onChange={(e) => setLayout(e.currentTarget.value as AlbumLayout)}
          >
            {ALBUM_LAYOUTS.map((l) => (
              <option key={l} value={l}>
                {t(`slideEditor.album.layouts.${l}`)}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>{t('slideEditor.album.frame')}</span>
          <select
            data-testid="album-frame"
            value={frame}
            onChange={(e) => setFrame(e.currentTarget.value as AlbumFrame)}
          >
            {ALBUM_FRAMES.map((f) => (
              <option key={f} value={f}>
                {t(`slideEditor.album.frames.${f}`)}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className={styles.checks}>
        <label>
          <input
            type="checkbox"
            data-testid="album-titles"
            checked={options.titles}
            disabled={fit}
            onChange={(e) => setTitles(e.currentTarget.checked)}
          />
          {t('slideEditor.album.titles')}
        </label>
        <label>
          <input
            type="checkbox"
            data-testid="album-captions"
            checked={options.captions}
            disabled={fit}
            onChange={(e) => setCaptions(e.currentTarget.checked)}
          />
          {t('slideEditor.album.captions')}
        </label>
      </div>
      <fieldset className={styles.fieldset}>
        <legend>{t('slideEditor.album.target')}</legend>
        <label>
          <input
            type="radio"
            name="album-target"
            data-testid="album-target-append"
            checked={target === 'append'}
            disabled={!canAppend}
            onChange={() => setTarget('append')}
          />
          {t('slideEditor.album.targetAppend')}
        </label>
        <label>
          <input
            type="radio"
            name="album-target"
            data-testid="album-target-new"
            checked={target === 'new'}
            onChange={() => setTarget('new')}
          />
          {t('slideEditor.album.targetNew')}
        </label>
        {target === 'new' && (
          <label>
            <span>{t('slideEditor.album.name')}</span>
            <input
              type="text"
              data-testid="album-title"
              value={title}
              onChange={(e) => setTitle(e.currentTarget.value)}
            />
          </label>
        )}
        {target === 'new' && unsaved && (
          <p className={styles.warn} role="note">
            {t('slideEditor.album.unsaved')}
          </p>
        )}
      </fieldset>
    </Dialog>
  );
}
