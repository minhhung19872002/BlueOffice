import type { EngineHost, SlideMediaInfo } from '@blueoffice/platform';
import { Pause, Play } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import styles from './SlideEditor.module.css';

/** Can this browser / webview decode the clip's type? (`maybe` counts.) */
export function canPlay(mime: string | null, kind: 'video' | 'audio'): boolean {
  if (!mime || typeof document === 'undefined') return false;
  const el = document.createElement(kind);
  return typeof el.canPlayType === 'function' && el.canPlayType(mime) !== '';
}

/** Object URL of an embedded clip, revoked when the component goes away. */
export function useClipUrl(
  engine: EngineHost,
  clip: SlideMediaInfo,
  enabled: boolean,
): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!enabled || !clip.part || typeof URL.createObjectURL !== 'function') return;
    let cancelled = false;
    let made: string | null = null;
    void engine
      .getMedia(clip.part)
      .then((bytes) => {
        if (cancelled) return;
        const copy = new Uint8Array(bytes.byteLength);
        copy.set(bytes);
        made = URL.createObjectURL(new Blob([copy.buffer], { type: clip.mime ?? '' }));
        setUrl(made);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      if (made) URL.revokeObjectURL(made);
    };
  }, [engine, clip.part, clip.mime, enabled]);
  return url;
}

export interface MediaClipProps {
  engine: EngineHost;
  clip: SlideMediaInfo;
  /** CSS px per point. */
  scale: number;
  autoPlay?: boolean;
  labels?: { play: string; unsupported: string };
}

/**
 * A playable clip over its poster frame: a `<video>` in the picture frame or
 * an `<audio>` bar under the icon. The poster drawn by the engine stays visible
 * until the clip has data, and when the platform cannot decode it.
 */
export function MediaClip({ engine, clip, scale, autoPlay = false, labels }: MediaClipProps) {
  const playable = !!clip.part && canPlay(clip.mime, clip.kind);
  const url = useClipUrl(engine, clip, playable);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const [x, y, w, h] = clip.rect;
  const stop = (e: { stopPropagation: () => void }) => e.stopPropagation();
  const common = {
    'data-testid': `media-${clip.kind}`,
    'data-ready': ready,
    'data-playable': playable && !failed,
    title: playable ? clip.name : labels?.unsupported,
    onClick: stop,
    onMouseDown: stop,
    onPointerDown: stop,
    onContextMenu: stop,
  };
  if (!playable || failed) {
    // Poster only: a marker says the clip cannot be played here.
    return (
      <div
        className={styles.mediaUnsupported}
        style={{ left: x * scale, top: (y + h) * scale - 22 }}
        {...common}
      >
        {labels?.unsupported}
      </div>
    );
  }
  if (clip.kind === 'video') {
    return (
      <video
        className={styles.mediaVideo}
        style={{
          left: x * scale,
          top: y * scale,
          width: w * scale,
          height: h * scale,
          opacity: ready ? 1 : 0,
        }}
        src={url ?? undefined}
        controls
        autoPlay={autoPlay}
        playsInline
        preload="auto"
        onLoadedData={() => setReady(true)}
        onError={() => setFailed(true)}
        {...common}
      />
    );
  }
  return (
    <audio
      className={styles.mediaAudio}
      style={{ left: Math.max(0, x * scale - 100), top: (y + h) * scale + 4 }}
      src={url ?? undefined}
      controls
      autoPlay={autoPlay}
      preload="metadata"
      onLoadedMetadata={() => setReady(true)}
      onError={() => setFailed(true)}
      {...common}
    />
  );
}

export interface MediaButtonsProps {
  engine: EngineHost;
  media: SlideMediaInfo[];
  scale: number;
  labels: { play: string; stop: string; unsupported: string };
}

/**
 * Normal view: a play button on each clip of the slide; playing shows the clip
 * over its frame (the shape stays selectable once it is stopped).
 */
export function MediaButtons({ engine, media, scale, labels }: MediaButtonsProps) {
  const [playing, setPlaying] = useState<number | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  // A clip that left the slide stops.
  const active = media.some((m) => m.id === playing) ? playing : null;
  return (
    <div ref={ref} className={styles.mediaButtons}>
      {media.map((m) => {
        const [x, y, , h] = m.rect;
        const on = active === m.id;
        return (
          <div key={m.id}>
            {on && <MediaClip engine={engine} clip={m} scale={scale} autoPlay labels={labels} />}
            <button
              type="button"
              className={styles.mediaButton}
              style={{ left: x * scale + 4, top: (y + h) * scale - 30 }}
              data-testid={`media-play-${m.id}`}
              aria-pressed={on}
              aria-label={on ? labels.stop : labels.play}
              title={on ? labels.stop : labels.play}
              onMouseDown={(e) => e.stopPropagation()}
              onPointerDown={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                setPlaying(on ? null : m.id);
              }}
            >
              {on ? <Pause size={14} /> : <Play size={14} />}
            </button>
          </div>
        );
      })}
    </div>
  );
}
