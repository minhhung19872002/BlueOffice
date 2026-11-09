import type { EngineHost, ShowVideoOptions, VideoEncoding } from '@blueoffice/platform';
import { zipStore } from '../zip';
import { muxMp4 } from './mp4';
import { muxWebm } from './webm';

/**
 * Export of the slide show (frames from the engine's frame generator,
 * `bo_engine::slide_edit::video`) as a video (WebCodecs + BlueSlide's own MP4 / WebM muxer),
 * an animated GIF (engine) or a ZIP of PNG frames (engine). No DOM; unit-tested.
 */

export type ShowExportFormat = 'video' | 'gif' | 'png';

export interface ShowExportRequest {
  format: ShowExportFormat;
  /** Frame width in pixels. */
  width: number;
  fps: number;
  secondsPerSlide: number;
  useTimings: boolean;
  /** GIF: loop forever. */
  loop: boolean;
}

export interface ShowExportResult {
  bytes: Uint8Array;
  ext: 'mp4' | 'webm' | 'gif' | 'zip';
  mime: string;
}

/** Widths offered (pixels). */
export const VIDEO_WIDTHS = [480, 720, 1080, 1920] as const;
/** Frame rates offered for a video and for a GIF (a GIF stays ≤ 15 frames per second). */
export const VIDEO_FPS = [24, 30] as const;
export const GIF_FPS = [5, 10, 15] as const;
export const MAX_GIF_FPS = 15;

/** Raised when the export is cancelled. */
export class ExportCancelled extends Error {
  constructor() {
    super('export.cancelled');
  }
}

/** Raised when the browser cannot encode video. */
export const VIDEO_UNSUPPORTED = 'video.unsupported';

/** Presentation time (µs) of frame `i` and its duration, on the exact `i / fps` grid. */
export function frameTime(i: number, fps: number): { ts: number; dur: number } {
  const ts = Math.round((i * 1e6) / fps);
  return { ts, dur: Math.round(((i + 1) * 1e6) / fps) - ts };
}

/** A key frame every two seconds (and the first frame). */
export function isKeyFrame(i: number, fps: number): boolean {
  return i % Math.max(1, Math.round(fps * 2)) === 0;
}

export async function exportShow(
  engine: EngineHost,
  video: VideoEncoding | undefined,
  req: ShowExportRequest,
  onProgress: (done: number, total: number) => void,
  signal?: AbortSignal,
): Promise<ShowExportResult> {
  const { videoBegin, videoFrame, videoGifAdd, videoGifFinish, videoEnd } = engine;
  if (!videoBegin || !videoFrame || !videoGifAdd || !videoGifFinish || !videoEnd) {
    throw new Error('engine.unsupported: video');
  }
  const fps = req.format === 'gif' ? Math.min(req.fps, MAX_GIF_FPS) : req.fps;
  const opts: ShowVideoOptions = {
    width: req.width,
    fps,
    secondsPerSlide: req.secondsPerSlide,
    useTimings: req.useTimings,
    loop: req.loop,
    dither: true,
  };
  const check = () => {
    if (signal?.aborted) throw new ExportCancelled();
  };
  const info = await videoBegin.call(engine, opts);
  const total = info.frameCount;
  try {
    if (req.format === 'gif') {
      for (let i = 0; i < total; i++) {
        check();
        await videoGifAdd.call(engine, i);
        onProgress(i + 1, total);
      }
      return { bytes: await videoGifFinish.call(engine), ext: 'gif', mime: 'image/gif' };
    }
    if (req.format === 'png') {
      const files: { name: string; data: Uint8Array }[] = [];
      const digits = Math.max(4, String(total).length);
      for (let i = 0; i < total; i++) {
        check();
        files.push({
          name: `frame-${String(i + 1).padStart(digits, '0')}.png`,
          data: await videoFrame.call(engine, i, true),
        });
        onProgress(i + 1, total);
      }
      return { bytes: zipStore(files), ext: 'zip', mime: 'application/zip' };
    }
    const choice = video ? await video.probe(info.width, info.height, info.fps) : null;
    if (!video || !choice) throw new Error(VIDEO_UNSUPPORTED);
    const session = await video.open(choice, info.width, info.height, info.fps);
    try {
      for (let i = 0; i < total; i++) {
        check();
        const { ts, dur } = frameTime(i, info.fps);
        await session.encode(
          await videoFrame.call(engine, i, false),
          ts,
          dur,
          isKeyFrame(i, info.fps),
        );
        onProgress(i + 1, total);
      }
      const { chunks, description } = await session.flush();
      check();
      if (choice.container === 'mp4') {
        if (!description) throw new Error(VIDEO_UNSUPPORTED);
        return {
          bytes: muxMp4({ width: info.width, height: info.height, avcC: description, chunks }),
          ext: 'mp4',
          mime: 'video/mp4',
        };
      }
      return {
        bytes: muxWebm({
          width: info.width,
          height: info.height,
          codecId: choice.codec.startsWith('vp09') ? 'V_VP9' : 'V_VP8',
          chunks,
        }),
        ext: 'webm',
        mime: 'video/webm',
      };
    } finally {
      session.close();
    }
  } finally {
    await videoEnd.call(engine);
  }
}
