import { useTranslation } from '@blueoffice/i18n';
import type { VideoCodecChoice, VideoEncoding } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useRef, useState } from 'react';
import styles from './SlideEditor.module.css';
import {
  ExportCancelled,
  GIF_FPS,
  VIDEO_FPS,
  VIDEO_UNSUPPORTED,
  VIDEO_WIDTHS,
  type ShowExportFormat,
  type ShowExportRequest,
} from './video/export';

export interface ExportVideoDialogProps {
  /** Slide size in points (for the pixel sizes shown). */
  slide: { width: number; height: number };
  /** Video encoding of the platform (absent = GIF / PNG only). */
  video?: VideoEncoding;
  onClose: () => void;
  /** Runs the export (and saves the file); rejects with {@link ExportCancelled} when cancelled. */
  run: (
    req: ShowExportRequest,
    onProgress: (done: number, total: number) => void,
    signal: AbortSignal,
  ) => Promise<void>;
}

/** Even frame height for a width (as the engine computes it). */
export function frameHeight(width: number, slide: { width: number; height: number }): number {
  if (slide.width <= 0) return 0;
  return Math.max(2, Math.round((width * slide.height) / slide.width / 2) * 2);
}

/**
 * File › Export › video / animated picture: format (video when the browser can encode it,
 * animated GIF, PNG frames), size, frame rate, time per slide, slide timings, progress, cancel.
 */
export function ExportVideoDialog({ slide, video, onClose, run }: ExportVideoDialogProps) {
  const { t } = useTranslation();
  // `undefined` = still checking, `null` = no video encoding here.
  const [codec, setCodec] = useState<VideoCodecChoice | null | undefined>(video ? undefined : null);
  const [format, setFormat] = useState<ShowExportFormat>('gif');
  const [width, setWidth] = useState<number>(720);
  const [fps, setFps] = useState<number>(10);
  const [seconds, setSeconds] = useState(5);
  const [useTimings, setUseTimings] = useState(true);
  const [loop, setLoop] = useState(true);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abort = useRef<AbortController | null>(null);

  useEffect(() => {
    if (!video) return;
    let live = true;
    void video
      .probe(1280, frameHeight(1280, slide) || 720, 30)
      .catch(() => null)
      .then((c) => {
        if (!live) return;
        setCodec(c);
        if (c) {
          setFormat('video');
          setFps(30);
        }
      });
    return () => {
      live = false;
    };
  }, [video, slide]);

  useEffect(() => () => abort.current?.abort(), []);

  const busy = progress !== null;
  const gif = format === 'gif';
  const fpsChoices: readonly number[] = gif ? GIF_FPS : VIDEO_FPS;

  const choose = (f: ShowExportFormat) => {
    setFormat(f);
    const list: readonly number[] = f === 'gif' ? GIF_FPS : VIDEO_FPS;
    if (!list.includes(fps)) setFps(f === 'gif' ? 10 : 30);
  };

  const start = async () => {
    const ctl = new AbortController();
    abort.current = ctl;
    setError(null);
    setProgress({ done: 0, total: 0 });
    try {
      await run(
        { format, width, fps, secondsPerSlide: seconds, useTimings, loop },
        (done, total) => setProgress({ done, total }),
        ctl.signal,
      );
      onClose();
    } catch (e) {
      if (e instanceof ExportCancelled || ctl.signal.aborted) {
        setError(t('slideEditor.videoCancelled'));
      } else {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg === VIDEO_UNSUPPORTED ? t('slideEditor.videoUnsupported') : msg);
      }
    } finally {
      abort.current = null;
      setProgress(null);
    }
  };

  const container = codec?.container === 'webm' ? 'WebM' : 'MP4';
  const pct = progress && progress.total > 0 ? progress.done / progress.total : 0;

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && !busy && onClose()}
      title={t('slideEditor.exportVideo')}
      testId="export-video-dialog"
      width={460}
      footer={
        <>
          <button
            type="button"
            data-testid="export-video-cancel"
            onClick={() => (busy ? abort.current?.abort() : onClose())}
          >
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="export-video-ok"
            disabled={busy || (format === 'video' && !codec)}
            onClick={() => void start()}
          >
            {t('slideEditor.export')}
          </button>
        </>
      }
    >
      <fieldset className={styles.exportForm} disabled={busy}>
        <legend>{t('slideEditor.videoFormat')}</legend>
        <label className={styles.check}>
          <input
            type="radio"
            name="video-format"
            data-testid="export-video-format-video"
            checked={format === 'video'}
            disabled={!codec}
            onChange={() => choose('video')}
          />
          {t('slideEditor.videoFormatVideo', { container })}
        </label>
        {codec === undefined && (
          <p className={styles.animEmpty} data-testid="export-video-checking">
            {t('slideEditor.videoChecking')}
          </p>
        )}
        {codec === null && (
          <p className={styles.animEmpty} data-testid="export-video-unsupported">
            {t('slideEditor.videoUnsupported')}
          </p>
        )}
        <label className={styles.check}>
          <input
            type="radio"
            name="video-format"
            data-testid="export-video-format-gif"
            checked={gif}
            onChange={() => choose('gif')}
          />
          {t('slideEditor.videoFormatGif')}
        </label>
        <label className={styles.check}>
          <input
            type="radio"
            name="video-format"
            data-testid="export-video-format-png"
            checked={format === 'png'}
            onChange={() => choose('png')}
          />
          {t('slideEditor.videoFormatPng')}
        </label>
      </fieldset>
      <fieldset className={styles.exportForm} disabled={busy}>
        <label className={styles.exportForm}>
          {t('slideEditor.videoSize')}
          <select
            data-testid="export-video-width"
            value={width}
            onChange={(e) => setWidth(Number(e.target.value))}
          >
            {VIDEO_WIDTHS.map((w) => (
              <option key={w} value={w}>
                {t('slideEditor.videoSizeOption', { w, h: frameHeight(w, slide) })}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.exportForm}>
          {t('slideEditor.videoFps')}
          <select
            data-testid="export-video-fps"
            value={fps}
            onChange={(e) => setFps(Number(e.target.value))}
          >
            {fpsChoices.map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.exportForm}>
          {t('slideEditor.videoSeconds')}
          <input
            type="number"
            data-testid="export-video-seconds"
            min={0.5}
            max={600}
            step={0.5}
            value={seconds}
            onChange={(e) => {
              const v = Number(e.target.value);
              if (Number.isFinite(v) && v > 0) setSeconds(Math.min(600, v));
            }}
          />
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="export-video-timings"
            checked={useTimings}
            onChange={(e) => setUseTimings(e.target.checked)}
          />
          {t('slideEditor.videoTimings')}
        </label>
        <label className={styles.check}>
          <input type="checkbox" data-testid="export-video-narration" checked={false} disabled />
          {t('slideEditor.videoNarration')}
        </label>
        {gif && (
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="export-video-loop"
              checked={loop}
              onChange={(e) => setLoop(e.target.checked)}
            />
            {t('slideEditor.videoLoop')}
          </label>
        )}
      </fieldset>
      {busy && (
        <div className={styles.exportForm}>
          <progress data-testid="export-video-progress" max={1} value={pct} />
          <span>{t('slideEditor.videoProgress', progress)}</span>
        </div>
      )}
      {error && (
        <p className={styles.animEmpty} role="alert" data-testid="export-video-error">
          {error}
        </p>
      )}
    </Dialog>
  );
}
