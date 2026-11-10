/**
 * Video encoding through the browser's WebCodecs `VideoEncoder` (W3C WebCodecs), shared by
 * the web platform and the desktop webview. The encoder belongs to the browser / operating
 * system; BlueOffice only feeds it frames and muxes the result itself. Narration goes through
 * WebCodecs `AudioEncoder`: AAC-LC (`mp4a.40.2`) for MP4, Opus for WebM.
 */
import type {
  AudioSource,
  EncodedAudioChunkData,
  EncodedAudioTrack,
  EncodedVideoChunkData,
  VideoCodecChoice,
  VideoEncoderSession,
  VideoEncoding,
} from './types';

/** Sample frames per `AudioData` handed to the encoder. */
const AUDIO_BLOCK = 4800;

/** Whether this browser has WebCodecs audio encoding. */
export function hasAudioEncoder(): boolean {
  return typeof AudioEncoder !== 'undefined' && typeof AudioData !== 'undefined';
}

/** Encode `source` with WebCodecs (`null` when the codec is not available). */
export async function encodeAudioWebCodecs(
  container: 'mp4' | 'webm',
  source: AudioSource,
  signal?: AbortSignal,
): Promise<EncodedAudioTrack | null> {
  if (!hasAudioEncoder()) return null;
  const codec = container === 'mp4' ? 'aac' : 'opus';
  const cfg: AudioEncoderConfig = {
    codec: codec === 'aac' ? 'mp4a.40.2' : 'opus',
    sampleRate: source.sampleRate,
    numberOfChannels: source.channels,
    bitrate: 96_000,
  };
  if (codec === 'aac') (cfg as AudioEncoderConfig & { aac?: unknown }).aac = { format: 'aac' };
  try {
    const ok = await AudioEncoder.isConfigSupported(cfg);
    if (!ok.supported) return null;
  } catch {
    return null;
  }
  const chunks: EncodedAudioChunkData[] = [];
  let description: Uint8Array | null = null;
  let failure: Error | null = null;
  const enc = new AudioEncoder({
    output: (chunk, meta) => {
      const data = new Uint8Array(chunk.byteLength);
      chunk.copyTo(data);
      chunks.push({ data, timestampUs: chunk.timestamp, durationUs: chunk.duration ?? 0 });
      const d = meta?.decoderConfig?.description;
      if (d && !description) {
        description = ArrayBuffer.isView(d)
          ? new Uint8Array(d.buffer.slice(d.byteOffset, d.byteOffset + d.byteLength))
          : new Uint8Array(d.slice(0));
      }
    },
    error: (e) => {
      failure = e instanceof Error ? e : new Error(String(e));
    },
  });
  try {
    enc.configure(cfg);
    for (let at = 0; at < source.frames; at += AUDIO_BLOCK) {
      if (signal?.aborted) throw new Error('export.cancelled');
      if (failure) throw failure;
      const n = Math.min(AUDIO_BLOCK, source.frames - at);
      const planes = source.read(at, n);
      const data = new Float32Array(n * source.channels);
      planes.forEach((p, k) => data.set(p.subarray(0, n), k * n));
      const frame = new AudioData({
        format: 'f32-planar',
        sampleRate: source.sampleRate,
        numberOfFrames: n,
        numberOfChannels: source.channels,
        timestamp: Math.round((at * 1e6) / source.sampleRate),
        data,
      });
      try {
        enc.encode(frame);
      } finally {
        frame.close();
      }
      while (enc.encodeQueueSize > 16) await new Promise((r) => setTimeout(r, 5));
    }
    await enc.flush();
    if (failure) throw failure;
  } finally {
    if (enc.state !== 'closed') enc.close();
  }
  return { codec, sampleRate: source.sampleRate, channels: source.channels, description, chunks };
}

/** Codecs tried in order: H.264 (MP4) High / Main / Baseline level 4.0, then VP9 and VP8 (WebM). */
export const VIDEO_CODECS: VideoCodecChoice[] = [
  { container: 'mp4', codec: 'avc1.640028' },
  { container: 'mp4', codec: 'avc1.4D0028' },
  { container: 'mp4', codec: 'avc1.42E028' },
  { container: 'webm', codec: 'vp09.00.40.08' },
  { container: 'webm', codec: 'vp8' },
];

/** Bits per second for a frame size: about 0.1 bit per pixel per frame, at least 1 Mb/s. */
export function bitrateFor(width: number, height: number, fps: number): number {
  return Math.max(1_000_000, Math.round(width * height * fps * 0.1));
}

function config(c: VideoCodecChoice, width: number, height: number, fps: number) {
  const cfg: VideoEncoderConfig = {
    codec: c.codec,
    width,
    height,
    framerate: fps,
    bitrate: bitrateFor(width, height, fps),
  };
  // Length-prefixed NAL units + `avcC` description, as MP4 stores them (ISO/IEC 14496-15).
  if (c.container === 'mp4') cfg.avc = { format: 'avc' };
  return cfg;
}

/** Whether this browser has WebCodecs video encoding at all. */
export function hasWebCodecs(): boolean {
  return typeof VideoEncoder !== 'undefined' && typeof VideoFrame !== 'undefined';
}

export class WebCodecsVideo implements VideoEncoding {
  encodeAudio(container: 'mp4' | 'webm', source: AudioSource, signal?: AbortSignal) {
    return encodeAudioWebCodecs(container, source, signal);
  }

  async probe(width: number, height: number, fps: number): Promise<VideoCodecChoice | null> {
    if (!hasWebCodecs()) return null;
    for (const c of VIDEO_CODECS) {
      try {
        const r = await VideoEncoder.isConfigSupported(config(c, width, height, fps));
        if (r.supported) return c;
      } catch {
        // Invalid for this browser: try the next one.
      }
    }
    return null;
  }

  async open(
    choice: VideoCodecChoice,
    width: number,
    height: number,
    fps: number,
  ): Promise<VideoEncoderSession> {
    const chunks: EncodedVideoChunkData[] = [];
    let description: Uint8Array | null = null;
    let failure: Error | null = null;
    const enc = new VideoEncoder({
      output: (chunk, meta) => {
        const data = new Uint8Array(chunk.byteLength);
        chunk.copyTo(data);
        chunks.push({
          data,
          timestampUs: chunk.timestamp,
          durationUs: chunk.duration ?? Math.round(1e6 / fps),
          key: chunk.type === 'key',
        });
        const d = meta?.decoderConfig?.description;
        if (d && !description) {
          description = ArrayBuffer.isView(d)
            ? new Uint8Array(d.buffer.slice(d.byteOffset, d.byteOffset + d.byteLength))
            : new Uint8Array(d.slice(0));
        }
      },
      error: (e) => {
        failure = e instanceof Error ? e : new Error(String(e));
      },
    });
    enc.configure(config(choice, width, height, fps));
    return {
      async encode(rgba, timestampUs, durationUs, key) {
        if (failure) throw failure;
        const frame = new VideoFrame(rgba, {
          format: 'RGBA',
          codedWidth: width,
          codedHeight: height,
          timestamp: timestampUs,
          duration: durationUs,
        });
        try {
          enc.encode(frame, { keyFrame: key });
        } finally {
          frame.close();
        }
        // Back-pressure: do not queue the whole show in the encoder.
        while (enc.encodeQueueSize > 8) {
          await new Promise((r) => setTimeout(r, 5));
        }
      },
      async flush() {
        await enc.flush();
        if (failure) throw failure;
        return { chunks, description };
      },
      close() {
        if (enc.state !== 'closed') enc.close();
      },
    };
  }
}
