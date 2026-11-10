import { useTranslation } from '@blueoffice/i18n';
import type { MediaCaptureSession, SlideCameraCorner } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useRef, useState } from 'react';
import type { RecordState } from './recording';
import { formatElapsed } from './show';
import styles from './showTools.module.css';

/** What to record (SLIDE-REC-001). */
export interface RecordOptions {
  narration: boolean;
  camera: boolean;
  corner: SlideCameraCorner;
}

export const DEFAULT_RECORD_OPTIONS: RecordOptions = {
  narration: true,
  camera: false,
  corner: 'bottomRight',
};

const CORNERS: SlideCameraCorner[] = ['topLeft', 'topRight', 'bottomLeft', 'bottomRight'];

/** Before recording: microphone, camera and where the camera picture goes. */
export function RecordSetupDialog({
  devices,
  initial,
  onStart,
  onCancel,
}: {
  /** What the platform can record from (`null`: no recording of media here). */
  devices: { microphone: boolean; camera: boolean } | null;
  initial: RecordOptions;
  onStart: (o: RecordOptions) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [o, setO] = useState<RecordOptions>({
    ...initial,
    narration: initial.narration && !!devices?.microphone,
    camera: initial.camera && !!devices?.camera,
  });
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onCancel()}
      title={t('showRecord.setupTitle')}
      testId="record-setup-dialog"
      width={420}
      footer={
        <>
          <button type="button" data-testid="record-cancel" onClick={onCancel}>
            {t('showRecord.cancel')}
          </button>
          <button type="button" data-testid="record-start" onClick={() => onStart(o)}>
            {t('showRecord.start')}
          </button>
        </>
      }
    >
      <fieldset className={styles.form}>
        {!devices && <p data-testid="record-no-devices">{t('showRecord.noDevices')}</p>}
        <label className={styles.row}>
          <input
            type="checkbox"
            data-testid="record-narration"
            checked={o.narration}
            disabled={!devices?.microphone}
            onChange={(e) => setO({ ...o, narration: e.target.checked })}
          />
          {t('showRecord.microphone')}
        </label>
        <label className={styles.row}>
          <input
            type="checkbox"
            data-testid="record-camera"
            checked={o.camera}
            disabled={!devices?.camera}
            onChange={(e) => setO({ ...o, camera: e.target.checked })}
          />
          {t('showRecord.camera')}
        </label>
        <label className={styles.row}>
          {t('showRecord.cameraCorner')}
          <select
            data-testid="record-corner"
            value={o.corner}
            disabled={!o.camera}
            onChange={(e) => setO({ ...o, corner: e.target.value as SlideCameraCorner })}
          >
            {CORNERS.map((c) => (
              <option key={c} value={c}>
                {t(`showRecord.corner.${c}`)}
              </option>
            ))}
          </select>
        </label>
        <p>{t('showRecord.hint')}</p>
      </fieldset>
    </Dialog>
  );
}

/** Recording toolbar over the show: state, slide / total time, pause, record again, stop. */
export function RecordingBar({
  state,
  slideMs,
  totalMs,
  notice,
  onPause,
  onRestart,
  onStop,
}: {
  state: RecordState;
  slideMs: number;
  totalMs: number;
  /** A message (devices refused). */
  notice?: string | null;
  onPause: () => void;
  onRestart: () => void;
  onStop: () => void;
}) {
  const { t } = useTranslation();
  const paused = state === 'paused';
  return (
    <div
      className={styles.rehearsal}
      data-testid="recording-bar"
      data-state={state}
      role="timer"
      aria-label={paused ? t('showRecord.paused') : t('showRecord.recording')}
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.stopPropagation()}
    >
      <span
        className={paused ? styles.recDotPaused : styles.recDot}
        aria-hidden
        data-testid="recording-dot"
      />
      <span data-testid="recording-state">
        {paused ? t('showRecord.paused') : t('showRecord.recording')}
      </span>
      <button type="button" data-testid="recording-pause" aria-pressed={paused} onClick={onPause}>
        {paused ? t('showRecord.resume') : t('showRecord.pause')}
      </button>
      <span data-testid="recording-slide-time" title={t('showRecord.slideTime')}>
        {formatElapsed(slideMs)}
      </span>
      <button type="button" data-testid="recording-restart" onClick={onRestart}>
        {t('showRecord.restartSlide')}
      </button>
      <span data-testid="recording-total" title={t('showRecord.totalTime')}>
        {formatElapsed(totalMs)}
      </span>
      <button type="button" data-testid="recording-stop" onClick={onStop}>
        {t('showRecord.stop')}
      </button>
      {notice && (
        <span role="status" data-testid="recording-notice">
          {notice}
        </span>
      )}
    </div>
  );
}

/** The live camera picture in its corner of the show while recording. */
export function CameraPreview({
  session,
  corner,
}: {
  session: MediaCaptureSession;
  corner: SlideCameraCorner;
}) {
  const { t } = useTranslation();
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    return session.preview(el);
  }, [session]);
  const pos: Record<SlideCameraCorner, string> = {
    topLeft: styles.camTopLeft!,
    topRight: styles.camTopRight!,
    bottomLeft: styles.camBottomLeft!,
    bottomRight: styles.camBottomRight!,
  };
  return (
    <video
      ref={ref}
      className={`${styles.camera} ${pos[corner]}`}
      data-testid="record-camera-preview"
      aria-label={t('showRecord.preview')}
      muted
      playsInline
      autoPlay
    />
  );
}
