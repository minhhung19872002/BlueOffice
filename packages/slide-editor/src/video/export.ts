import type {
  EncodedAudioTrack,
  EngineHost,
  MediaCaptureService,
  ShowVideoInfo,
  ShowVideoOptions,
  SlideMediaInfo,
  VideoEncoding,
  VideoFrameReader,
} from '@blueoffice/platform';
import { zipStore } from '../zip';
import { blit, narrationTrack, type NarrationClip } from './audioMix';
import { muxMp4 } from './mp4';
import { muxWebm } from './webm';

/**
 * Export of the slide show (frames from the engine's frame generator,
 * `bo_engine::slide_edit::video`) as a video (WebCodecs + BlueSlide's own MP4 / WebM muxer),
 * an animated GIF (engine) or a ZIP of PNG frames (engine). No DOM; unit-tested.
 *
 * Recorded shows (SLIDE-REC-001): with `narration`, each slide's recorded narration becomes
 * the video's sound track (decoded by `Platform.media`, encoded by `VideoEncoding.encodeAudio`:
 * AAC in MP4, Opus in WebM) and its camera clip is drawn over the frames in its box. Without an
 * audio encoder the video is made without sound (`audioDropped`); without frame decoding the
 * camera box shows the clip's poster. GIF and PNG frames never carry sound.
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
  /** Video: recorded narration and camera (SLIDE-REC-001). */
  narration?: boolean;
}

export interface ShowExportResult {
  bytes: Uint8Array;
  ext: 'mp4' | 'webm' | 'gif' | 'zip';
  mime: string;
  /** Narration was recorded but could not be encoded: the video has no sound. */
  audioDropped?: boolean;
}

/** A recorded clip of a slide in the video. */
interface RecordedClip {
  clip: SlideMediaInfo;
  /** Slide width in points (to scale the clip's box to frame pixels). */
  slideWidth: number;
  /** Where the clip plays on the video timeline (ms). */
  from: number;
  to: number;
}

/** Recorded narration / camera clips of the slides of the video. */
export async function recordedClips(
  engine: EngineHost,
  info: ShowVideoInfo,
): Promise<RecordedClip[]> {
  const out: RecordedClip[] = [];
  for (const span of info.slides) {
    const r = await engine.slideCommand({ cmd: 'show', slide: span.slide });
    const s = r.show;
    for (const m of s?.media ?? []) {
      if (!m.recorded || !m.part) continue;
      out.push({
        clip: m,
        slideWidth: s?.width || 1,
        from: span.startMs + span.transitionMs,
        to: span.startMs + span.lengthMs,
      });
    }
  }
  return out;
}

/** Narration of the recorded clips as one encoded track (`null`: none could be made). */
async function narrationAudio(
  engine: EngineHost,
  media: MediaCaptureService | undefined,
  video: VideoEncoding,
  container: 'mp4' | 'webm',
  clips: RecordedClip[],
  durationMs: number,
  signal?: AbortSignal,
): Promise<EncodedAudioTrack | null> {
  if (!media?.decodeAudio || !video.encodeAudio) return null;
  const decoded: NarrationClip[] = [];
  for (const c of clips) {
    try {
      const bytes = await engine.getMedia(c.clip.part!);
      const audio = await media.decodeAudio(bytes, c.clip.mime ?? '');
      decoded.push({ startMs: c.from, endMs: c.to, audio });
    } catch {
      // An undecodable clip is left out.
    }
  }
  if (!decoded.length) return null;
  return video.encodeAudio(container, narrationTrack(decoded, durationMs), signal);
}

/** A camera clip drawn over the frames. */
interface CameraLayer {
  reader: VideoFrameReader;
  box: { x: number; y: number; w: number; h: number };
  from: number;
  to: number;
}

async function cameraLayers(
  engine: EngineHost,
  media: MediaCaptureService | undefined,
  clips: RecordedClip[],
  frameWidth: number,
): Promise<CameraLayer[]> {
  const out: CameraLayer[] = [];
  if (!media?.videoFrames) return out;
  for (const c of clips) {
    const k = frameWidth / c.slideWidth;
    const [x, y, w, h] = c.clip.rect;
    const box = { x: x * k, y: y * k, w: w * k, h: h * k };
    try {
      const bytes = await engine.getMedia(c.clip.part!);
      const reader = await media.videoFrames(
        bytes,
        c.clip.mime ?? 'video/webm',
        Math.max(2, Math.round(box.w)),
        Math.max(2, Math.round(box.h)),
      );
      out.push({ reader, box, from: c.from, to: c.to });
    } catch {
      // Not decodable here: the poster frame stays.
    }
  }
  return out;
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
  /** Decoding of recorded narration / camera clips (`Platform.media`). */
  media?: MediaCaptureService,
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
    const recorded = req.narration ? await recordedClips(engine, info) : [];
    const cams = await cameraLayers(
      engine,
      media,
      recorded.filter((c) => c.clip.recorded === 'camera'),
      info.width,
    );
    const session = await video.open(choice, info.width, info.height, info.fps);
    try {
      for (let i = 0; i < total; i++) {
        check();
        const { ts, dur } = frameTime(i, info.fps);
        const rgba = await videoFrame.call(engine, i, false);
        const t = ts / 1000;
        for (const c of cams) {
          if (t < c.from || t >= c.to) continue;
          const f = await c.reader.frame(t - c.from);
          if (f) blit(rgba, info.width, info.height, f, c.reader.width, c.reader.height, c.box);
        }
        await session.encode(rgba, ts, dur, isKeyFrame(i, info.fps));
        onProgress(i + 1, total);
      }
      const { chunks, description } = await session.flush();
      check();
      const voice = recorded.filter((c) => c.clip.recorded === 'narration');
      const audio = voice.length
        ? await narrationAudio(
            engine,
            media,
            video,
            choice.container,
            voice,
            info.durationMs,
            signal,
          )
        : null;
      check();
      const audioDropped = voice.length > 0 && !audio;
      if (choice.container === 'mp4') {
        if (!description) throw new Error(VIDEO_UNSUPPORTED);
        return {
          bytes: muxMp4({
            width: info.width,
            height: info.height,
            avcC: description,
            chunks,
            audio: audio
              ? {
                  sampleRate: audio.sampleRate,
                  channels: audio.channels,
                  asc: audio.description,
                  chunks: audio.chunks,
                }
              : undefined,
          }),
          ext: 'mp4',
          mime: 'video/mp4',
          audioDropped,
        };
      }
      return {
        bytes: muxWebm({
          width: info.width,
          height: info.height,
          codecId: choice.codec.startsWith('vp09') ? 'V_VP9' : 'V_VP8',
          chunks,
          audio: audio
            ? {
                sampleRate: audio.sampleRate,
                channels: audio.channels,
                head: audio.description,
                chunks: audio.chunks,
              }
            : undefined,
        }),
        ext: 'webm',
        mime: 'video/webm',
        audioDropped,
      };
    } finally {
      session.close();
      for (const c of cams) c.reader.close();
    }
  } finally {
    await videoEnd.call(engine);
  }
}
