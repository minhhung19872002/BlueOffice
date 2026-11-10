/**
 * Microphone and camera recording (SLIDE-REC-001: recording a slide show). The platform owns
 * the devices; editors only open a session, record one segment per slide and get the bytes.
 * The web and the desktop webview (WebView2 / WebKitGTK) record through the W3C Media Capture
 * and Streams API (`navigator.mediaDevices.getUserMedia`) and MediaStream Recording
 * (`MediaRecorder`); decoding for the video export uses Web Audio (`decodeAudioData`) and an
 * offscreen `<video>` element. `FakeMediaCapture` records deterministic silent WAV narration
 * and a placeholder camera clip for tests.
 */

/** What to record. */
export interface MediaCaptureRequest {
  audio: boolean;
  video: boolean;
}

/** A recorded clip. */
export interface RecordedClip {
  bytes: Uint8Array;
  /** MIME type as recorded (`audio/webm;codecs=opus`, `video/webm`, `audio/wav`…). */
  mime: string;
}

/** One recorded segment (one slide). */
export interface RecordedSegment {
  audio: RecordedClip | null;
  video: RecordedClip | null;
  /** Recorded time, pauses left out (ms). */
  durationMs: number;
}

/** A segment being recorded. */
export interface SegmentRecorder {
  pause(): void;
  resume(): void;
  /** Ends the segment; resolves with what was recorded. */
  stop(): Promise<RecordedSegment>;
  /** Ends the segment and drops it. */
  cancel(): void;
}

/** Open devices. */
export interface MediaCaptureSession {
  readonly audio: boolean;
  readonly video: boolean;
  /** Width / height of the camera picture (`null` without camera). */
  readonly aspect: number | null;
  /** Shows the live camera in `el` (nothing without camera); returns the detach function. */
  preview(el: HTMLVideoElement): () => void;
  /** Starts recording a segment. */
  segment(): SegmentRecorder;
  /** Releases the devices (segments still recording are dropped). */
  close(): void;
}

/** Decoded PCM audio. */
export interface DecodedAudio {
  sampleRate: number;
  /** One array per channel. */
  channels: Float32Array[];
}

/** Frames of a recorded video, read at given times. */
export interface VideoFrameReader {
  readonly width: number;
  readonly height: number;
  /** RGBA pixels (`width` × `height`) of the frame shown at `ms`, `null` past the end. */
  frame(ms: number): Promise<Uint8ClampedArray | null>;
  close(): void;
}

/** Recording and decoding of microphone / camera media. */
export interface MediaCaptureService {
  /** Which devices the platform can record from (without asking for permission). */
  available(): { microphone: boolean; camera: boolean };
  /** Opens the devices (asks for permission); rejects with `media.denied` / `media.none`. */
  open(req: MediaCaptureRequest): Promise<MediaCaptureSession>;
  /** Decodes an audio clip (video export); absent where the platform cannot. */
  decodeAudio?(bytes: Uint8Array, mime: string): Promise<DecodedAudio>;
  /** Reads frames of a video clip scaled to `width` × `height` (video export). */
  videoFrames?(
    bytes: Uint8Array,
    mime: string,
    width: number,
    height: number,
  ): Promise<VideoFrameReader>;
}

/** Error messages of `open`. */
export const MEDIA_DENIED = 'media.denied';
export const MEDIA_NONE = 'media.none';

// ------------------------------------------------------------------------------ WAV helper

/** A WAV file (16-bit PCM) of `channels[0..]` at `sampleRate`. */
export function encodeWav(sampleRate: number, channels: Float32Array[]): Uint8Array {
  const n = channels[0]?.length ?? 0;
  const c = Math.max(1, channels.length);
  const data = n * c * 2;
  const out = new Uint8Array(44 + data);
  const v = new DataView(out.buffer);
  const str = (at: number, s: string) => {
    for (let i = 0; i < s.length; i++) out[at + i] = s.charCodeAt(i);
  };
  str(0, 'RIFF');
  v.setUint32(4, 36 + data, true);
  str(8, 'WAVE');
  str(12, 'fmt ');
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, c, true);
  v.setUint32(24, sampleRate, true);
  v.setUint32(28, sampleRate * c * 2, true);
  v.setUint16(32, c * 2, true);
  v.setUint16(34, 16, true);
  str(36, 'data');
  v.setUint32(40, data, true);
  let at = 44;
  for (let i = 0; i < n; i++) {
    for (let k = 0; k < c; k++) {
      const s = Math.max(-1, Math.min(1, channels[k]?.[i] ?? 0));
      v.setInt16(at, Math.round(s * 32767), true);
      at += 2;
    }
  }
  return out;
}

/** PCM of a 16-bit WAV written by {@link encodeWav} (or any plain PCM WAV); `null` otherwise. */
export function decodeWav(bytes: Uint8Array): DecodedAudio | null {
  if (bytes.length < 44) return null;
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const tag = (at: number) => String.fromCharCode(...bytes.subarray(at, at + 4));
  if (tag(0) !== 'RIFF' || tag(8) !== 'WAVE') return null;
  let at = 12;
  let rate = 0;
  let channels = 0;
  let bits = 0;
  while (at + 8 <= bytes.length) {
    const id = tag(at);
    const size = v.getUint32(at + 4, true);
    const body = at + 8;
    if (id === 'fmt ' && size >= 16) {
      if (v.getUint16(body, true) !== 1) return null;
      channels = v.getUint16(body + 2, true);
      rate = v.getUint32(body + 4, true);
      bits = v.getUint16(body + 14, true);
    } else if (id === 'data' && rate > 0 && channels > 0 && (bits === 16 || bits === 8)) {
      const step = (bits / 8) * channels;
      const n = Math.floor(Math.min(size, bytes.length - body) / step);
      const out = Array.from({ length: channels }, () => new Float32Array(n));
      for (let i = 0; i < n; i++) {
        for (let k = 0; k < channels; k++) {
          const p = body + i * step + (k * bits) / 8;
          out[k]![i] = bits === 16 ? v.getInt16(p, true) / 32768 : (v.getUint8(p) - 128) / 128;
        }
      }
      return { sampleRate: rate, channels: out };
    }
    at = body + size + (size % 2);
  }
  return null;
}

// ------------------------------------------------------------------------------ fake

/** Clock of the fake recorder (ms). */
export type Clock = () => number;

/**
 * Deterministic recorder for tests: narration is a silent WAV as long as the recorded time
 * (8 kHz mono), the camera clip a placeholder `video/webm` of a few bytes.
 */
export class FakeMediaCapture implements MediaCaptureService {
  /** Sessions opened, in order (tests read them). */
  readonly opened: MediaCaptureRequest[] = [];
  /** Segments started. */
  segments = 0;

  constructor(
    private readonly clock: Clock = () => Date.now(),
    private readonly devices = { microphone: true, camera: true },
    /** `open` refuses (permission denied). */
    private readonly deny = false,
  ) {}

  available() {
    return { ...this.devices };
  }

  async open(req: MediaCaptureRequest): Promise<MediaCaptureSession> {
    if (this.deny) throw new Error(MEDIA_DENIED);
    const audio = req.audio && this.devices.microphone;
    const video = req.video && this.devices.camera;
    if (!audio && !video) throw new Error(MEDIA_NONE);
    this.opened.push({ audio, video });
    const clock = this.clock;
    const count = () => this.segments++;
    let closed = false;
    return {
      audio,
      video,
      aspect: video ? 4 / 3 : null,
      preview: () => () => {},
      segment(): SegmentRecorder {
        count();
        let since: number | null = clock();
        let total = 0;
        let live = true;
        const time = () => total + (since === null ? 0 : clock() - since);
        return {
          pause() {
            if (since !== null) total += clock() - since;
            since = null;
          },
          resume() {
            if (since === null) since = clock();
          },
          async stop() {
            const ms = Math.max(0, Math.round(time()));
            live = false;
            since = null;
            if (closed) return { audio: null, video: null, durationMs: ms };
            const rate = 8000;
            return {
              audio: audio
                ? {
                    bytes: encodeWav(rate, [new Float32Array(Math.round((ms * rate) / 1000))]),
                    mime: 'audio/wav',
                  }
                : null,
              video: video
                ? {
                    bytes: new Uint8Array([0x1a, 0x45, 0xdf, 0xa3, 0, 0, 0, 0]),
                    mime: 'video/webm',
                  }
                : null,
              durationMs: ms,
            };
          },
          cancel() {
            live = false;
            void live;
          },
        };
      },
      close() {
        closed = true;
      },
    };
  }

  async decodeAudio(bytes: Uint8Array): Promise<DecodedAudio> {
    const d = decodeWav(bytes);
    if (!d) throw new Error('media.decode');
    return d;
  }

  async videoFrames(_b: Uint8Array, _m: string, width: number, height: number) {
    // A solid mid-grey camera picture.
    return {
      width,
      height,
      frame: async () => new Uint8ClampedArray(width * height * 4).fill(128),
      close: () => {},
    };
  }
}

// ------------------------------------------------------------------------------ web

/** The parts of `MediaStreamTrack` used here. */
interface TrackLike {
  kind: string;
  stop(): void;
  getSettings?(): { width?: number; height?: number; aspectRatio?: number };
}

/** The parts of `MediaStream` used here. */
interface StreamLike {
  getTracks(): TrackLike[];
  getAudioTracks(): TrackLike[];
  getVideoTracks(): TrackLike[];
}

/** The parts of `MediaRecorder` used here. */
interface RecorderLike {
  readonly mimeType: string;
  readonly state: string;
  ondataavailable: ((e: { data: Blob }) => void) | null;
  onstop: (() => void) | null;
  start(timeslice?: number): void;
  stop(): void;
  pause(): void;
  resume(): void;
}

/** What `WebMediaCapture` needs from the browser (injected for tests). */
export interface WebMediaDeps {
  getUserMedia(c: { audio: boolean; video: boolean | object }): Promise<StreamLike>;
  MediaRecorder: {
    new (stream: unknown, options?: { mimeType?: string }): RecorderLike;
    isTypeSupported?(t: string): boolean;
  };
  /** `new MediaStream(tracks)`. */
  makeStream(tracks: TrackLike[]): unknown;
  now(): number;
  /** Web Audio decoding (absent: no narration in the video export). */
  decodeAudioData?(bytes: ArrayBuffer): Promise<DecodedAudio>;
  /** Offscreen video frames (absent: no camera in the video export). */
  videoFrames?: MediaCaptureService['videoFrames'];
  hasMicrophone: boolean;
  hasCamera: boolean;
}

/** Recording formats tried in order (the first the browser has). */
export const AUDIO_TYPES = [
  'audio/webm;codecs=opus',
  'audio/ogg;codecs=opus',
  'audio/mp4',
  'audio/webm',
];
export const VIDEO_TYPES = [
  'video/webm;codecs=vp9',
  'video/webm;codecs=vp8',
  'video/webm',
  'video/mp4',
];

function pickType(deps: WebMediaDeps, list: string[]): string | undefined {
  const ok = deps.MediaRecorder.isTypeSupported;
  if (!ok) return undefined;
  return list.find((t) => {
    try {
      return ok.call(deps.MediaRecorder, t);
    } catch {
      return false;
    }
  });
}

/** One MediaRecorder over a stream, collecting its data. */
function recordStream(deps: WebMediaDeps, stream: unknown, types: string[]) {
  const mimeType = pickType(deps, types);
  const rec = new deps.MediaRecorder(stream, mimeType ? { mimeType } : undefined);
  const parts: Blob[] = [];
  rec.ondataavailable = (e) => {
    if (e.data && e.data.size > 0) parts.push(e.data);
  };
  const done = new Promise<RecordedClip | null>((resolve) => {
    rec.onstop = () => {
      const type = rec.mimeType || mimeType || types[types.length - 1]!;
      if (!parts.length) {
        resolve(null);
        return;
      }
      void new Blob(parts, { type })
        .arrayBuffer()
        .then((b) => resolve({ bytes: new Uint8Array(b), mime: type }))
        .catch(() => resolve(null));
    };
  });
  rec.start(1000);
  return { rec, done };
}

export class WebMediaCapture implements MediaCaptureService {
  constructor(private readonly deps: WebMediaDeps) {}

  available() {
    return { microphone: this.deps.hasMicrophone, camera: this.deps.hasCamera };
  }

  async open(req: MediaCaptureRequest): Promise<MediaCaptureSession> {
    const deps = this.deps;
    if (!req.audio && !req.video) throw new Error(MEDIA_NONE);
    let stream: StreamLike;
    try {
      stream = await deps.getUserMedia({
        audio: req.audio,
        video: req.video ? { width: { ideal: 640 }, height: { ideal: 480 } } : false,
      });
    } catch (e) {
      const name = (e as { name?: string })?.name;
      throw new Error(name === 'NotFoundError' ? MEDIA_NONE : MEDIA_DENIED, { cause: e });
    }
    const audioTracks = stream.getAudioTracks();
    const videoTracks = stream.getVideoTracks();
    const s = videoTracks[0]?.getSettings?.();
    const aspect =
      videoTracks.length === 0
        ? null
        : (s?.aspectRatio ?? (s?.width && s?.height ? s.width / s.height : 4 / 3));
    const audioStream = audioTracks.length ? deps.makeStream(audioTracks) : null;
    const videoStream = videoTracks.length ? deps.makeStream(videoTracks) : null;
    let closed = false;
    const live = new Set<{ cancel(): void }>();
    return {
      audio: audioTracks.length > 0,
      video: videoTracks.length > 0,
      aspect,
      preview(el: HTMLVideoElement) {
        if (!videoStream) return () => {};
        (el as unknown as { srcObject: unknown }).srcObject = videoStream;
        el.muted = true;
        void el.play?.()?.catch?.(() => {});
        return () => {
          (el as unknown as { srcObject: unknown }).srcObject = null;
        };
      },
      segment(): SegmentRecorder {
        const a = audioStream ? recordStream(deps, audioStream, AUDIO_TYPES) : null;
        const v = videoStream ? recordStream(deps, videoStream, VIDEO_TYPES) : null;
        let since: number | null = deps.now();
        let total = 0;
        const all = [a, v].filter((x): x is NonNullable<typeof x> => !!x);
        const stopAll = () => {
          for (const r of all) if (r.rec.state !== 'inactive') r.rec.stop();
        };
        const handle = {
          pause() {
            if (since !== null) total += deps.now() - since;
            since = null;
            for (const r of all) if (r.rec.state === 'recording') r.rec.pause();
          },
          resume() {
            if (since === null) since = deps.now();
            for (const r of all) if (r.rec.state === 'paused') r.rec.resume();
          },
          async stop(): Promise<RecordedSegment> {
            const durationMs = Math.round(total + (since === null ? 0 : deps.now() - since));
            since = null;
            live.delete(handle);
            stopAll();
            const [audio, video] = await Promise.all([a?.done ?? null, v?.done ?? null]);
            return { audio: closed ? null : audio, video: closed ? null : video, durationMs };
          },
          cancel() {
            live.delete(handle);
            stopAll();
          },
        };
        live.add(handle);
        return handle;
      },
      close() {
        closed = true;
        for (const h of [...live]) h.cancel();
        for (const t of stream.getTracks()) t.stop();
      },
    };
  }

  async decodeAudio(bytes: Uint8Array): Promise<DecodedAudio> {
    const wav = decodeWav(bytes);
    if (wav) return wav;
    if (!this.deps.decodeAudioData) throw new Error('media.decode');
    const copy = new Uint8Array(bytes.byteLength);
    copy.set(bytes);
    return this.deps.decodeAudioData(copy.buffer);
  }

  get videoFrames(): MediaCaptureService['videoFrames'] {
    return this.deps.videoFrames;
  }
}

/** Frames of a clip through an offscreen `<video>` element seeked to each time. */
async function elementFrames(
  doc: Document,
  bytes: Uint8Array,
  mime: string,
  width: number,
  height: number,
): Promise<VideoFrameReader> {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  const url = URL.createObjectURL(new Blob([copy.buffer], { type: mime }));
  const video = doc.createElement('video');
  video.muted = true;
  video.preload = 'auto';
  video.src = url;
  await new Promise<void>((resolve, reject) => {
    video.onloadeddata = () => resolve();
    video.onerror = () => reject(new Error('media.decode'));
  });
  const canvas = doc.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) throw new Error('media.decode');
  // Recorded WebM often has no duration until it has been read to the end.
  const duration = Number.isFinite(video.duration) ? video.duration * 1000 : Infinity;
  return {
    width,
    height,
    async frame(ms: number) {
      if (ms > duration) return null;
      const t = Math.max(0, ms / 1000);
      if (Math.abs(video.currentTime - t) > 1e-3) {
        await new Promise<void>((resolve) => {
          const done = () => {
            video.removeEventListener('seeked', done);
            resolve();
          };
          video.addEventListener('seeked', done);
          video.currentTime = t;
          setTimeout(done, 2000);
        });
      }
      if (video.ended && ms > 0) return null;
      ctx.drawImage(video, 0, 0, width, height);
      return ctx.getImageData(0, 0, width, height).data;
    },
    close() {
      video.removeAttribute('src');
      URL.revokeObjectURL(url);
    },
  };
}

/** The platform's recorder over the browser's media APIs, `undefined` where it has none. */
export function webMediaCapture(w: unknown = globalThis): MediaCaptureService | undefined {
  const g = w as {
    navigator?: {
      mediaDevices?: {
        getUserMedia?(c: unknown): Promise<StreamLike>;
      };
    };
    MediaRecorder?: WebMediaDeps['MediaRecorder'];
    MediaStream?: new (tracks: TrackLike[]) => unknown;
    AudioContext?: new () => {
      decodeAudioData(b: ArrayBuffer): Promise<{
        sampleRate: number;
        numberOfChannels: number;
        getChannelData(i: number): Float32Array;
      }>;
      close?(): Promise<void>;
    };
    document?: Document;
  };
  const md = g.navigator?.mediaDevices;
  const Rec = g.MediaRecorder;
  const Stream = g.MediaStream;
  if (!md?.getUserMedia || typeof Rec !== 'function' || typeof Stream !== 'function') {
    return undefined;
  }
  const Ctx = g.AudioContext;
  const doc = g.document;
  return new WebMediaCapture({
    getUserMedia: (c) => md.getUserMedia!.call(md, c),
    MediaRecorder: Rec,
    makeStream: (tracks) => new Stream(tracks),
    now: () => (typeof performance !== 'undefined' ? performance.now() : Date.now()),
    decodeAudioData: Ctx
      ? async (b) => {
          const ctx = new Ctx();
          try {
            const buf = await ctx.decodeAudioData(b);
            return {
              sampleRate: buf.sampleRate,
              channels: Array.from({ length: buf.numberOfChannels }, (_, i) =>
                buf.getChannelData(i).slice(),
              ),
            };
          } finally {
            void ctx.close?.().catch(() => {});
          }
        }
      : undefined,
    videoFrames: doc ? (b, m, wd, ht) => elementFrames(doc, b, m, wd, ht) : undefined,
    hasMicrophone: true,
    hasCamera: true,
  });
}
