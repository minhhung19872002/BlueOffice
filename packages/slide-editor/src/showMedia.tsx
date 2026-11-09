import type { EngineHost, SlideMediaInfo, SlideMediaPlayback } from '@blueoffice/platform';
import { useEffect, useRef, useState } from 'react';
import { canPlay } from './MediaLayer';
import { MediaController, showMediaCarry } from './mediaControl';
import styles from './SlideEditor.module.css';

/** Playback of a clip without options (older decks, previews). */
export const DEFAULT_PLAYBACK: SlideMediaPlayback = {
  trimStartMs: 0,
  trimEndMs: 0,
  fadeInMs: 0,
  fadeOutMs: 0,
  bookmarks: [],
  start: null,
  playClick: null,
  volume: 100,
  mute: false,
  slides: 1,
  loop: false,
  rewind: false,
  hideWhenStopped: false,
  fullScreen: false,
  captions: [],
};

/** Object URLs handed on to the show's carried player (revoked by it). */
const handedOn = new Set<string>();

/** Object URL of `part` (type `mime`), revoked with the component unless handed on. */
function usePartUrl(engine: EngineHost, part: string | null, mime: string): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!part || typeof URL.createObjectURL !== 'function') return;
    let cancelled = false;
    let made: string | null = null;
    void engine
      .getMedia(part)
      .then((bytes) => {
        if (cancelled) return;
        const copy = new Uint8Array(bytes.byteLength);
        copy.set(bytes);
        made = URL.createObjectURL(new Blob([copy.buffer], { type: mime }));
        setUrl(made);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      if (made && !handedOn.has(made)) URL.revokeObjectURL(made);
    };
  }, [engine, part, mime]);
  return url;
}

function CaptionTrack({
  engine,
  part,
  label,
  lang,
  first,
}: {
  engine: EngineHost;
  part: string | null;
  label: string;
  lang: string | null;
  first: boolean;
}) {
  const url = usePartUrl(engine, part, 'text/vtt');
  if (!url) return null;
  return (
    <track
      kind="captions"
      src={url}
      label={label}
      srcLang={lang ?? undefined}
      default={first}
      data-testid="show-media-track"
    />
  );
}

export interface ShowMediaClipProps {
  engine: EngineHost;
  clip: SlideMediaInfo;
  /** CSS px per point. */
  scale: number;
  /** Slide shown and click steps done. */
  slide: number;
  click: number;
  /** Stage size in CSS px (full-screen playback). */
  stage: { width: number; height: number };
  labels?: { play: string; unsupported: string };
  /** The clip reached a bookmark: its effects start. */
  onBookmark?: (shapeId: number, name: string) => void;
  /** "Hide while not playing": the clip's layer is hidden (`true`) or shown. */
  onHidden?: (shapeId: number, hidden: boolean) => void;
}

/**
 * A clip in the slide show, played with its options (SLIDE-MEDIA-001): starts with the slide,
 * on its click step or when clicked; trim, fades, bookmarks, loop and rewind through
 * [`MediaController`]; captions as `<track>`; full screen while playing; a clip playing across
 * slides is handed to the show's carried player when its slide is left.
 */
export function ShowMediaClip({
  engine,
  clip,
  scale,
  slide,
  click,
  stage,
  labels,
  onBookmark,
  onHidden,
}: ShowMediaClipProps) {
  const pb = clip.playback ?? DEFAULT_PLAYBACK;
  const playable = !!clip.part && canPlay(clip.mime, clip.kind);
  const url = usePartUrl(engine, playable ? clip.part : null, clip.mime ?? '');
  const el = useRef<HTMLVideoElement & HTMLAudioElement>(null);
  const ctrl = useRef<MediaController | null>(null);
  const started = useRef(false);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const [playing, setPlaying] = useState(false);
  const cb = useRef({ onBookmark, onHidden });
  useEffect(() => {
    cb.current = { onBookmark, onHidden };
  });
  const pbRef = useRef(pb);
  useEffect(() => {
    pbRef.current = pb;
    ctrl.current?.update(pb);
  });
  const key = `${slide}:${clip.id}`;

  // The same clip carried on from an earlier visit stops when its slide is back.
  useEffect(() => {
    if (showMediaCarry.key === key) showMediaCarry.stop();
  }, [key]);

  // Hidden until it plays.
  const hidden = pb.hideWhenStopped && !playing;
  useEffect(() => {
    cb.current.onHidden?.(clip.id, hidden);
  }, [clip.id, hidden]);

  // One controller per element.
  useEffect(() => {
    const m = el.current;
    if (!m || !url) return;
    const c = new MediaController(m, pbRef.current, {
      onBookmark: (n) => cb.current.onBookmark?.(clip.id, n),
      onPlaying: setPlaying,
    });
    ctrl.current = c;
    return () => {
      ctrl.current = null;
      const p = pbRef.current;
      if (c.isPlaying && p.slides > 1 && typeof Audio !== 'undefined') {
        // Play on across slides: a detached audio element continues from here.
        const a = new Audio(url);
        const at = m.currentTime;
        a.addEventListener('loadedmetadata', () => {
          a.currentTime = at;
        });
        handedOn.add(url);
        showMediaCarry.adopt(key, a, p, slide, p.slides, () => {
          handedOn.delete(url);
          URL.revokeObjectURL(url);
        });
      }
      c.pause();
      c.dispose();
    };
  }, [url, clip.id, key, slide]);

  // Start on the clip's click step (0 = with the slide); stepping back before it stops it.
  useEffect(() => {
    const c = ctrl.current;
    if (!c || !ready || pb.playClick === null) return;
    if (click >= pb.playClick && !started.current) {
      started.current = true;
      c.start();
    } else if (click < pb.playClick && started.current) {
      started.current = false;
      c.stop();
    }
  }, [click, ready, pb.playClick]);

  const stop = (e: { stopPropagation: () => void }) => e.stopPropagation();
  const toggle = (e: { stopPropagation: () => void }) => {
    e.stopPropagation();
    // "When clicked": the clip plays and pauses on its own clicks.
    if (pb.start === 'click') ctrl.current?.toggle();
  };
  const [x, y, w, h] = clip.rect;
  if (!playable || failed) {
    return (
      <div
        className={styles.mediaUnsupported}
        style={{ left: x * scale, top: (y + h) * scale - 22 }}
        data-testid={`media-${clip.kind}`}
        data-playable={false}
      >
        {labels?.unsupported}
      </div>
    );
  }
  const full = clip.kind === 'video' && pb.fullScreen && playing;
  const box = full
    ? { left: 0, top: 0, width: stage.width, height: stage.height, zIndex: 6 }
    : { left: x * scale, top: y * scale, width: w * scale, height: h * scale };
  const common = {
    ref: el,
    src: url ?? undefined,
    'data-testid': `media-${clip.kind}`,
    'data-ready': ready,
    'data-playable': true,
    'data-playing': playing,
    'data-full': full,
    title: clip.name,
    controls: pb.start === null,
    preload: 'auto' as const,
    onLoadedMetadata: () => {
      const m = el.current;
      if (m && !started.current && pb.trimStartMs > 0) m.currentTime = pb.trimStartMs / 1000;
      setReady(true);
    },
    onError: () => setFailed(true),
    onClick: toggle,
    onMouseDown: stop,
    onPointerDown: stop,
    onContextMenu: stop,
  };
  if (clip.kind === 'video') {
    return (
      <video
        className={styles.mediaVideo}
        style={{ ...box, opacity: ready && !hidden ? 1 : 0 }}
        playsInline
        {...common}
      >
        {pb.captions.map((c, i) => (
          <CaptionTrack
            key={c.id}
            engine={engine}
            part={c.part}
            label={c.label}
            lang={c.lang}
            first={i === 0}
          />
        ))}
      </video>
    );
  }
  return (
    <>
      <audio
        className={styles.mediaAudio}
        style={{
          left: Math.max(0, x * scale - 100),
          top: (y + h) * scale + 4,
          visibility: hidden || pb.start !== null ? 'hidden' : 'visible',
        }}
        {...common}
      />
      {pb.start === 'click' && (
        // The sound's icon takes the click (an audio element without controls is not drawn).
        <button
          type="button"
          className={styles.showTrigger}
          data-testid={`show-media-toggle-${clip.id}`}
          aria-label={labels?.play}
          style={{ left: x * scale, top: y * scale, width: w * scale, height: h * scale }}
          onClick={toggle}
          onMouseDown={stop}
          onPointerDown={stop}
        />
      )}
    </>
  );
}
