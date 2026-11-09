/**
 * Video encoding through the browser's WebCodecs `VideoEncoder` (W3C WebCodecs), shared by
 * the web platform and the desktop webview. The encoder belongs to the browser / operating
 * system; BlueOffice only feeds it frames and muxes the result itself.
 */
import type {
  EncodedVideoChunkData,
  VideoCodecChoice,
  VideoEncoderSession,
  VideoEncoding,
} from './types';

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
