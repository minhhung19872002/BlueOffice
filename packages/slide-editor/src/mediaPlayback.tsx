import type { CommandRegistry } from '@blueoffice/editor';
import { useTranslation } from '@blueoffice/i18n';
import {
  MEDIA_ACROSS_SLIDES,
  type EngineHost,
  type SlideAnimInfo,
  type SlideCommand,
  type SlideEditResult,
  type SlideMediaInfo,
  type SlideMediaPlayback,
  type SlideMediaStart,
} from '@blueoffice/platform';
import { Dialog, toBase64, type RibbonItem, type RibbonTab } from '@blueoffice/ui';
import {
  Bookmark,
  Captions,
  CaptionsOff,
  EyeOff,
  Film,
  ImagePlus,
  Layers,
  Maximize,
  Repeat,
  Rewind,
  Scissors,
  VolumeX,
} from 'lucide-react';
import { useEffect, useRef, useState, type ReactNode, type RefObject } from 'react';
import { canPlay, useClipUrl } from './MediaLayer';
import { MediaController, parseVtt } from './mediaControl';
import { DEFAULT_PLAYBACK } from './showMedia';
import styles from './SlideEditor.module.css';
import own from './mediaPlayback.module.css';

/**
 * The "Playback" and "Video format" contextual tabs of a selected clip (SLIDE-MEDIA-001):
 * trim (dialog with a timeline and preview), fades, bookmarks (and the effects they start),
 * volume, start, playback options, WebVTT captions, and the poster frame (a frame of the video
 * or a picture). Every button is a registry command (`media.*`) acting through the hub.
 */

/** Registry command ids of the media tabs. */
export const MEDIA_COMMANDS = [
  'media.trim',
  'media.bookmarks',
  'media.captionsInsert',
  'media.captionsManage',
  'media.loop',
  'media.rewind',
  'media.hide',
  'media.fullScreen',
  'media.acrossSlides',
  'media.mute',
  'media.start.none',
  'media.start.seq',
  'media.start.auto',
  'media.start.click',
  'media.volume.0',
  'media.volume.25',
  'media.volume.50',
  'media.volume.75',
  'media.volume.100',
  'media.posterFrame',
  'media.posterFile',
] as const;

export type MediaCommandId = (typeof MEDIA_COMMANDS)[number];
export type MediaActions = Record<MediaCommandId, () => void>;

/** Hands the actions of the mounted tabs to the registry commands. */
export class MediaActionsHub {
  private actions: MediaActions | null = null;
  set(a: MediaActions | null): void {
    this.actions = a;
  }
  get(): MediaActions | null {
    return this.actions;
  }
}

/** Register the `media.*` commands (enabled while a clip is selected). */
export function registerMediaCommands<S>(reg: CommandRegistry<S>, hub: MediaActionsHub): void {
  for (const id of MEDIA_COMMANDS) {
    reg.register({ id, isEnabled: () => hub.get() !== null, run: () => hub.get()?.[id]() });
  }
}

const VOLUMES = [0, 25, 50, 75, 100] as const;
const STARTS = [
  ['none', 'slideMedia.startNone'],
  ['seq', 'slideMedia.startSeq'],
  ['auto', 'slideMedia.startAuto'],
  ['click', 'slideMedia.startClick'],
] as const;

/** `m:ss.t` of a time in ms. */
export function formatClock(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return '0:00.0';
  const tenths = Math.round(ms / 100);
  const m = Math.floor(tenths / 600);
  const s = (tenths % 600) / 10;
  return `${m}:${s.toFixed(1).padStart(4, '0')}`;
}

/** Seconds typed in a field, as ms (`null` when not a time). */
export function secondsToMs(v: string): number | null {
  const n = Number(v.replace(',', '.'));
  return Number.isFinite(n) && n >= 0 ? Math.round(n * 1000) : null;
}

export interface MediaToolsHost {
  engine: EngineHost;
  /** The selected clip, if exactly one clip is selected. */
  clip: SlideMediaInfo | null;
  /** Effects of the slide (what a bookmark can start). */
  animations: SlideAnimInfo[];
  run: (cmd: SlideCommand) => Promise<SlideEditResult | null>;
  pickFile: (accept: string[]) => Promise<{ bytes: Uint8Array; name: string } | null>;
  hub: MediaActionsHub;
}

type Open =
  | { kind: 'trim' }
  | { kind: 'bookmarks' }
  | { kind: 'poster' }
  | { kind: 'captions' }
  | { kind: 'captionAdd'; text: string; name: string; cues: number }
  | null;

/** A live preview of the clip with a position slider (dialogs). */
function Preview({
  engine,
  clip,
  onReady,
  mediaRef,
  pos,
  onPos,
}: {
  engine: EngineHost;
  clip: SlideMediaInfo;
  onReady: (durationMs: number) => void;
  mediaRef: RefObject<HTMLVideoElement & HTMLAudioElement>;
  pos: number;
  onPos: (ms: number) => void;
}) {
  const { t } = useTranslation();
  const playable = !!clip.part && canPlay(clip.mime, clip.kind);
  const url = useClipUrl(engine, clip, playable);
  const [dur, setDur] = useState(NaN);
  if (!playable) {
    return (
      <p className={styles.animEmpty} data-testid="media-no-preview">
        {t('slideMedia.noPreview')}
      </p>
    );
  }
  const props = {
    ref: mediaRef,
    src: url ?? undefined,
    preload: 'auto' as const,
    'data-testid': 'media-preview',
    onLoadedMetadata: () => {
      const d = (mediaRef.current?.duration ?? NaN) * 1000;
      setDur(d);
      onReady(d);
    },
    onTimeUpdate: () => onPos((mediaRef.current?.currentTime ?? 0) * 1000),
  };
  return (
    <div className={styles.exportForm}>
      {clip.kind === 'video' ? (
        <video
          {...props}
          playsInline
          style={{ width: '100%', maxHeight: 220, background: '#000' }}
        />
      ) : (
        <audio {...props} />
      )}
      <label className={styles.exportForm}>
        {t('slideMedia.position')} {formatClock(pos)}
        <input
          type="range"
          data-testid="media-scrubber"
          min={0}
          max={Number.isFinite(dur) ? dur : 0}
          step={10}
          value={pos}
          disabled={!Number.isFinite(dur)}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (mediaRef.current) mediaRef.current.currentTime = v / 1000;
            onPos(v);
          }}
        />
      </label>
      {Number.isFinite(dur) && (
        <span data-testid="media-length">{t('slideMedia.length', { d: formatClock(dur) })}</span>
      )}
    </div>
  );
}

function TrimDialog({
  engine,
  clip,
  pb,
  run,
  onClose,
}: {
  engine: EngineHost;
  clip: SlideMediaInfo;
  pb: SlideMediaPlayback;
  run: MediaToolsHost['run'];
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const media = useRef<HTMLVideoElement & HTMLAudioElement>(null);
  const [dur, setDur] = useState(NaN);
  const [pos, setPos] = useState(pb.trimStartMs);
  const [start, setStart] = useState(String(pb.trimStartMs / 1000));
  // End position (s) once the length is known; until then the cut at the end is kept.
  const [end, setEnd] = useState('');
  const ctrl = useRef<MediaController | null>(null);
  useEffect(() => () => ctrl.current?.dispose(), []);
  const known = Number.isFinite(dur);
  const startMs = secondsToMs(start);
  const endMs = known ? secondsToMs(end) : null;
  const valid =
    startMs !== null && (!known || (endMs !== null && endMs > startMs && endMs <= dur + 1));
  const preview = () => {
    const m = media.current;
    if (!m || startMs === null) return;
    ctrl.current?.dispose();
    ctrl.current = new MediaController(m, {
      ...DEFAULT_PLAYBACK,
      trimStartMs: startMs,
      trimEndMs: known && endMs !== null ? Math.max(0, dur - endMs) : pb.trimEndMs,
    });
    ctrl.current.start();
  };
  const ok = () => {
    if (!valid || startMs === null) return;
    void run({
      cmd: 'mediaPlayback',
      id: clip.id,
      trimStartMs: startMs,
      trimEndMs: known && endMs !== null ? Math.max(0, Math.round(dur - endMs)) : pb.trimEndMs,
    });
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideMedia.trimTitle')}
      testId="media-trim-dialog"
      width={480}
      footer={
        <>
          <button type="button" data-testid="media-trim-preview" onClick={preview}>
            {t('slideMedia.preview')}
          </button>
          <button type="button" data-testid="media-trim-cancel" onClick={onClose}>
            {t('slideMedia.cancel')}
          </button>
          <button type="button" data-testid="media-trim-ok" disabled={!valid} onClick={ok}>
            {t('slideMedia.ok')}
          </button>
        </>
      }
    >
      <Preview
        engine={engine}
        clip={clip}
        mediaRef={media}
        pos={pos}
        onPos={setPos}
        onReady={(d) => {
          setDur(d);
          setEnd(String(Math.max(0, d - pb.trimEndMs) / 1000));
          if (media.current) media.current.currentTime = pb.trimStartMs / 1000;
        }}
      />
      <div className={styles.exportForm}>
        <label className={styles.exportForm}>
          {t('slideMedia.trimStart')}
          <input
            type="number"
            data-testid="media-trim-start"
            min={0}
            step={0.1}
            value={start}
            onChange={(e) => setStart(e.target.value)}
          />
          {known && (
            <input
              type="range"
              data-testid="media-trim-start-range"
              min={0}
              max={dur}
              step={10}
              value={startMs ?? 0}
              onChange={(e) => {
                setStart(String(Number(e.target.value) / 1000));
                if (media.current) media.current.currentTime = Number(e.target.value) / 1000;
              }}
            />
          )}
        </label>
        <label className={styles.exportForm}>
          {t('slideMedia.trimEnd')}
          <input
            type="number"
            data-testid="media-trim-end"
            min={0}
            step={0.1}
            value={end}
            disabled={!known}
            onChange={(e) => setEnd(e.target.value)}
          />
          {known && (
            <input
              type="range"
              data-testid="media-trim-end-range"
              min={0}
              max={dur}
              step={10}
              value={endMs ?? dur}
              onChange={(e) => {
                setEnd(String(Number(e.target.value) / 1000));
                if (media.current) media.current.currentTime = Number(e.target.value) / 1000;
              }}
            />
          )}
        </label>
      </div>
    </Dialog>
  );
}

function BookmarksDialog({
  engine,
  clip,
  pb,
  animations,
  run,
  onClose,
}: {
  engine: EngineHost;
  clip: SlideMediaInfo;
  pb: SlideMediaPlayback;
  animations: SlideAnimInfo[];
  run: MediaToolsHost['run'];
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const media = useRef<HTMLVideoElement & HTMLAudioElement>(null);
  const [pos, setPos] = useState(0);
  const [name, setName] = useState('');
  const effects = animations.filter((a) => a.shapeId !== clip.id);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideMedia.bookmarksTitle')}
      testId="media-bookmarks-dialog"
      width={520}
      footer={
        <button type="button" data-testid="media-bookmarks-close" onClick={onClose}>
          {t('slideMedia.close')}
        </button>
      }
    >
      <Preview
        engine={engine}
        clip={clip}
        mediaRef={media}
        pos={pos}
        onPos={setPos}
        onReady={() => {}}
      />
      <div className={styles.exportForm}>
        <label className={styles.exportForm}>
          {t('slideMedia.bookmarkName')}
          <input
            data-testid="media-bookmark-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <label className={styles.exportForm}>
          {t('slideMedia.position')}
          <input
            type="number"
            data-testid="media-bookmark-time"
            min={0}
            step={0.1}
            value={String(Math.round(pos) / 1000)}
            onChange={(e) => {
              const v = secondsToMs(e.target.value);
              if (v === null) return;
              setPos(v);
              if (media.current) media.current.currentTime = v / 1000;
            }}
          />
        </label>
        <button
          type="button"
          data-testid="media-bookmark-add"
          onClick={() => {
            void run({
              cmd: 'mediaBookmarkAdd',
              id: clip.id,
              timeMs: Math.round(pos),
              ...(name.trim() ? { name: name.trim() } : {}),
            });
            setName('');
          }}
        >
          {t('slideMedia.bookmarkAdd')}
        </button>
      </div>
      {pb.bookmarks.length === 0 ? (
        <p className={styles.animEmpty} data-testid="media-bookmarks-empty">
          {t('slideMedia.bookmarksEmpty')}
        </p>
      ) : (
        <ul data-testid="media-bookmarks" className={styles.exportForm}>
          {pb.bookmarks.map((b) => {
            const started = effects.find(
              (a) => a.triggerShape === clip.id && a.triggerBookmark === b.name,
            );
            return (
              <li key={b.name} data-testid="media-bookmark">
                <button
                  type="button"
                  onClick={() => {
                    setPos(b.timeMs);
                    if (media.current) media.current.currentTime = b.timeMs / 1000;
                  }}
                >
                  {formatClock(b.timeMs)}
                </button>{' '}
                <span>{b.name}</span>{' '}
                <label>
                  {t('slideMedia.bookmarkTrigger')}{' '}
                  <select
                    data-testid="media-bookmark-trigger"
                    value={started ? String(started.index) : ''}
                    onChange={(e) => {
                      const v = e.target.value;
                      if (v === '') {
                        if (started) {
                          void run({
                            cmd: 'mediaBookmarkTrigger',
                            index: started.index,
                            media: clip.id,
                            bookmark: null,
                          });
                        }
                        return;
                      }
                      void run({
                        cmd: 'mediaBookmarkTrigger',
                        index: Number(v),
                        media: clip.id,
                        bookmark: b.name,
                      });
                    }}
                  >
                    <option value="">{t('slideMedia.bookmarkTriggerNone')}</option>
                    {effects.map((a) => (
                      <option key={a.index} value={String(a.index)}>
                        {t('slideMedia.effect', { n: a.index + 1, name: a.shapeName })}
                      </option>
                    ))}
                  </select>
                </label>{' '}
                <button
                  type="button"
                  data-testid="media-bookmark-remove"
                  onClick={() =>
                    void run({ cmd: 'mediaBookmarkRemove', id: clip.id, name: b.name })
                  }
                >
                  {t('slideMedia.bookmarkRemove')}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </Dialog>
  );
}

function PosterDialog({
  engine,
  clip,
  run,
  onClose,
}: {
  engine: EngineHost;
  clip: SlideMediaInfo;
  run: MediaToolsHost['run'];
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const media = useRef<HTMLVideoElement & HTMLAudioElement>(null);
  const [pos, setPos] = useState(0);
  const [ready, setReady] = useState(false);
  const use = async () => {
    const v = media.current;
    if (!v || typeof document === 'undefined') return;
    const c = document.createElement('canvas');
    c.width = v.videoWidth || 320;
    c.height = v.videoHeight || 180;
    const ctx = c.getContext('2d');
    if (!ctx) return;
    ctx.drawImage(v, 0, 0, c.width, c.height);
    const blob = await new Promise<Blob | null>((res) => c.toBlob(res, 'image/png'));
    if (!blob) return;
    const bytes = new Uint8Array(await blob.arrayBuffer());
    await run({ cmd: 'pictureChange', data: toBase64(bytes) });
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideMedia.posterFrameTitle')}
      testId="media-poster-dialog"
      width={480}
      footer={
        <>
          <button type="button" data-testid="media-poster-cancel" onClick={onClose}>
            {t('slideMedia.cancel')}
          </button>
          <button
            type="button"
            data-testid="media-poster-use"
            disabled={!ready}
            onClick={() => void use()}
          >
            {t('slideMedia.posterUse')}
          </button>
        </>
      }
    >
      <Preview
        engine={engine}
        clip={clip}
        mediaRef={media}
        pos={pos}
        onPos={setPos}
        onReady={() => setReady(true)}
      />
    </Dialog>
  );
}

function CaptionsDialog({
  clip,
  pb,
  run,
  onClose,
}: {
  clip: SlideMediaInfo;
  pb: SlideMediaPlayback;
  run: MediaToolsHost['run'];
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideMedia.captionsTitle')}
      testId="media-captions-dialog"
      footer={
        <button type="button" data-testid="media-captions-close" onClick={onClose}>
          {t('slideMedia.close')}
        </button>
      }
    >
      {pb.captions.length === 0 ? (
        <p className={styles.animEmpty} data-testid="media-captions-empty">
          {t('slideMedia.captionsEmpty')}
        </p>
      ) : (
        <ul data-testid="media-captions" className={styles.exportForm}>
          {pb.captions.map((c) => (
            <li key={c.id} data-testid="media-caption">
              <span>
                {c.label}
                {c.lang ? ` (${c.lang})` : ''}
              </span>{' '}
              <button
                type="button"
                data-testid="media-caption-remove"
                onClick={() => void run({ cmd: 'mediaCaptionRemove', id: clip.id, track: c.id })}
              >
                {t('slideMedia.captionRemove')}
              </button>
            </li>
          ))}
        </ul>
      )}
    </Dialog>
  );
}

function CaptionAddDialog({
  clip,
  file,
  run,
  onClose,
}: {
  clip: SlideMediaInfo;
  file: { text: string; name: string; cues: number };
  run: MediaToolsHost['run'];
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [label, setLabel] = useState(file.name.replace(/\.vtt$/i, ''));
  const [lang, setLang] = useState('');
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideMedia.captionsAddTitle')}
      testId="media-caption-add-dialog"
      footer={
        <>
          <button type="button" data-testid="media-caption-cancel" onClick={onClose}>
            {t('slideMedia.cancel')}
          </button>
          <button
            type="button"
            data-testid="media-caption-ok"
            onClick={() => {
              void run({
                cmd: 'mediaCaptionAdd',
                id: clip.id,
                text: file.text,
                label: label.trim(),
                ...(lang.trim() ? { lang: lang.trim() } : {}),
              });
              onClose();
            }}
          >
            {t('slideMedia.ok')}
          </button>
        </>
      }
    >
      <p data-testid="media-caption-cues">
        {t('slideMedia.captionCues', { n: file.cues, name: file.name })}
      </p>
      <label className={styles.exportForm}>
        {t('slideMedia.captionLabel')}
        <input
          data-testid="media-caption-label"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
        />
      </label>
      <label className={styles.exportForm}>
        {t('slideMedia.captionLang')}
        <input
          data-testid="media-caption-lang"
          value={lang}
          onChange={(e) => setLang(e.target.value)}
        />
      </label>
    </Dialog>
  );
}

/** A seconds field committed on blur / Enter (fade durations). */
function SecondsField({
  testId,
  label,
  ms,
  onCommit,
}: {
  testId: string;
  label: string;
  ms: number;
  onCommit: (ms: number) => void;
}) {
  // Keyed on `ms` by the caller: a new value starts a fresh field.
  const [text, setText] = useState(String(ms / 1000));
  const commit = () => {
    const v = secondsToMs(text);
    if (v === null) setText(String(ms / 1000));
    else if (v !== ms) onCommit(v);
  };
  return (
    <label className={own.field}>
      <span>{label}</span>
      <input
        type="number"
        data-testid={testId}
        min={0}
        step={0.25}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') commit();
        }}
      />
    </label>
  );
}

/** The media tabs, their dialogs and the actions behind the `media.*` commands. */
export function useMediaTools(host: MediaToolsHost): { tabs: RibbonTab[]; dialogs: ReactNode } {
  const { t } = useTranslation();
  const { clip, engine, hub, animations } = host;
  const [open, setOpen] = useState<Open>(null);
  const [error, setError] = useState('');
  const hostRef = useRef(host);
  useEffect(() => {
    hostRef.current = host;
  });
  const pb = clip?.playback ?? DEFAULT_PLAYBACK;
  const set = (patch: Omit<Extract<SlideCommand, { cmd: 'mediaPlayback' }>, 'cmd'>) => {
    const c = hostRef.current.clip;
    if (c) void hostRef.current.run({ cmd: 'mediaPlayback', id: c.id, ...patch });
  };
  const start = (s: SlideMediaStart | 'none') => set({ start: s });
  const actions: MediaActions = {
    'media.trim': () => setOpen({ kind: 'trim' }),
    'media.bookmarks': () => setOpen({ kind: 'bookmarks' }),
    'media.captionsManage': () => setOpen({ kind: 'captions' }),
    'media.captionsInsert': () => {
      void (async () => {
        const f = await hostRef.current.pickFile(['.vtt']);
        if (!f) return;
        const text = new TextDecoder().decode(f.bytes);
        const cues = parseVtt(text);
        if (!cues) {
          setError(t('slideMedia.captionBad'));
          return;
        }
        setError('');
        setOpen({ kind: 'captionAdd', text, name: f.name, cues: cues.length });
      })();
    },
    'media.loop': () => set({ loop: !pb.loop }),
    'media.rewind': () => set({ rewind: !pb.rewind }),
    'media.hide': () => set({ hideWhenStopped: !pb.hideWhenStopped }),
    'media.fullScreen': () => set({ fullScreen: !pb.fullScreen }),
    'media.acrossSlides': () => set({ slides: pb.slides > 1 ? 1 : MEDIA_ACROSS_SLIDES }),
    'media.mute': () => set({ mute: !pb.mute }),
    'media.start.none': () => start('none'),
    'media.start.seq': () => start('seq'),
    'media.start.auto': () => start('auto'),
    'media.start.click': () => start('click'),
    'media.volume.0': () => set({ volume: 0 }),
    'media.volume.25': () => set({ volume: 25 }),
    'media.volume.50': () => set({ volume: 50 }),
    'media.volume.75': () => set({ volume: 75 }),
    'media.volume.100': () => set({ volume: 100 }),
    'media.posterFrame': () => setOpen({ kind: 'poster' }),
    'media.posterFile': () => {
      void (async () => {
        const f = await hostRef.current.pickFile(['.png', '.jpg', '.jpeg', '.gif', '.bmp']);
        if (f) await hostRef.current.run({ cmd: 'pictureChange', data: toBase64(f.bytes) });
      })();
    },
  };
  // The registry commands run the actions of the current render.
  useEffect(() => {
    hub.set(clip ? actions : null);
    return () => hub.set(null);
  });

  if (!clip) return { tabs: [], dialogs: null };
  const exec = (id: MediaCommandId) => () => actions[id]();
  const button = (
    id: MediaCommandId,
    label: string,
    icon: ReactNode,
    checked?: boolean,
  ): RibbonItem => ({ id, label, icon, checked, size: 'small', onSelect: exec(id) });
  const startValue = pb.start ?? 'none';
  const video = clip.kind === 'video';
  const playback: RibbonTab = {
    id: 'mediaPlayback',
    context: 'media',
    label: t('slideMedia.tab'),
    groups: [
      {
        id: 'mediaBookmarks',
        label: t('slideMedia.groupBookmarks'),
        items: [
          {
            ...button('media.bookmarks', t('slideMedia.bookmarks'), <Bookmark size={18} />),
            size: 'large',
          },
        ],
      },
      {
        id: 'mediaEditing',
        label: t('slideMedia.groupEditing'),
        items: [
          {
            ...button('media.trim', t('slideMedia.trim'), <Scissors size={18} />),
            size: 'large',
          },
          {
            id: 'media.fadeIn',
            label: t('slideMedia.fadeIn'),
            onSelect: () => {},
            render: (
              <SecondsField
                key={pb.fadeInMs}
                testId="media-fade-in"
                label={t('slideMedia.fadeIn')}
                ms={pb.fadeInMs}
                onCommit={(ms) => set({ fadeInMs: ms })}
              />
            ),
          },
          {
            id: 'media.fadeOut',
            label: t('slideMedia.fadeOut'),
            onSelect: () => {},
            render: (
              <SecondsField
                key={pb.fadeOutMs}
                testId="media-fade-out"
                label={t('slideMedia.fadeOut')}
                ms={pb.fadeOutMs}
                onCommit={(ms) => set({ fadeOutMs: ms })}
              />
            ),
          },
        ],
      },
      {
        id: 'mediaOptions',
        label: t('slideMedia.groupOptions'),
        items: [
          {
            id: 'media.volume',
            label: t('slideMedia.volume'),
            onSelect: () => {},
            render: (
              <label className={own.field}>
                <span>{t('slideMedia.volume')}</span>
                <select
                  data-testid="media-volume"
                  value={String(pb.volume)}
                  onChange={(e) => set({ volume: Number(e.target.value) })}
                >
                  {!(VOLUMES as readonly number[]).includes(pb.volume) && (
                    <option value={String(pb.volume)}>
                      {t('slideMedia.volumeLevel', { n: pb.volume })}
                    </option>
                  )}
                  {VOLUMES.map((v) => (
                    <option key={v} value={String(v)}>
                      {t('slideMedia.volumeLevel', { n: v })}
                    </option>
                  ))}
                </select>
              </label>
            ),
          },
          {
            id: 'media.start',
            label: t('slideMedia.start'),
            onSelect: () => {},
            render: (
              <label className={own.field}>
                <span>{t('slideMedia.start')}</span>
                <select
                  data-testid="media-start"
                  value={startValue}
                  onChange={(e) => actions[`media.start.${e.target.value}` as MediaCommandId]()}
                >
                  {STARTS.map(([s, key]) => (
                    <option key={s} value={s}>
                      {t(key)}
                    </option>
                  ))}
                </select>
              </label>
            ),
          },
          button('media.mute', t('slideMedia.mute'), <VolumeX size={16} />, pb.mute),
          button(
            'media.acrossSlides',
            t('slideMedia.acrossSlides'),
            <Layers size={16} />,
            pb.slides > 1,
          ),
          button('media.loop', t('slideMedia.loop'), <Repeat size={16} />, pb.loop),
          button('media.rewind', t('slideMedia.rewind'), <Rewind size={16} />, pb.rewind),
          button('media.hide', t('slideMedia.hide'), <EyeOff size={16} />, pb.hideWhenStopped),
          ...(video
            ? [
                button(
                  'media.fullScreen',
                  t('slideMedia.fullScreen'),
                  <Maximize size={16} />,
                  pb.fullScreen,
                ),
              ]
            : []),
        ],
      },
      {
        id: 'mediaCaptions',
        label: t('slideMedia.groupCaptions'),
        items: [
          button('media.captionsInsert', t('slideMedia.captionsInsert'), <Captions size={16} />),
          button('media.captionsManage', t('slideMedia.captionsManage'), <CaptionsOff size={16} />),
        ],
      },
    ],
  };
  const format: RibbonTab = {
    id: 'mediaFormat',
    context: 'media',
    label: t('slideMedia.formatTab'),
    groups: [
      {
        id: 'mediaPoster',
        label: t('slideMedia.groupPoster'),
        items: [
          {
            ...button('media.posterFrame', t('slideMedia.posterFrame'), <Film size={18} />),
            size: 'large',
          },
          {
            ...button('media.posterFile', t('slideMedia.posterFile'), <ImagePlus size={18} />),
            size: 'large',
          },
        ],
      },
    ],
  };
  const close = () => setOpen(null);
  const dialogs = (
    <>
      {error && (
        <div role="alert" className={styles.animEmpty} data-testid="media-error">
          {error}
        </div>
      )}
      {open?.kind === 'trim' && (
        <TrimDialog engine={engine} clip={clip} pb={pb} run={host.run} onClose={close} />
      )}
      {open?.kind === 'bookmarks' && (
        <BookmarksDialog
          engine={engine}
          clip={clip}
          pb={pb}
          animations={animations}
          run={host.run}
          onClose={close}
        />
      )}
      {open?.kind === 'poster' && (
        <PosterDialog engine={engine} clip={clip} run={host.run} onClose={close} />
      )}
      {open?.kind === 'captions' && (
        <CaptionsDialog clip={clip} pb={pb} run={host.run} onClose={close} />
      )}
      {open?.kind === 'captionAdd' && (
        <CaptionAddDialog clip={clip} file={open} run={host.run} onClose={close} />
      )}
    </>
  );
  return { tabs: video ? [playback, format] : [playback], dialogs };
}
