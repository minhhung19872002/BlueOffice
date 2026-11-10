import type { EncodedAudioChunkData, EncodedVideoChunkData } from '@blueoffice/platform';

/**
 * Minimal WebM writer for one VP8 / VP9 video track and an optional Opus narration track
 * (EBML, RFC 8794; Matroska elements as documented by the IETF CELLAR Matroska specification;
 * WebM restricts the codecs; Opus in Matroska: `A_OPUS` with the RFC 7845 §5.1 `OpusHead` as
 * CodecPrivate). Clusters start at key frames (and at least every 30 s, so SimpleBlock times
 * fit in 16 bits); the timestamp scale is 1 ms.
 */

export interface WebmInput {
  width: number;
  height: number;
  /** `V_VP9` or `V_VP8`. */
  codecId: string;
  chunks: EncodedVideoChunkData[];
  /** Narration track (Opus), if any. */
  audio?: WebmAudio;
}

export interface WebmAudio {
  sampleRate: number;
  channels: number;
  /** `OpusHead` from the encoder; built when absent. */
  head: Uint8Array | null;
  chunks: EncodedAudioChunkData[];
}

/** RFC 7845 §5.1 identification header (channel mapping family 0). */
export function opusHead(channels: number, sampleRate: number, preSkip = 312): Uint8Array {
  const b = new Uint8Array(19);
  b.set(enc.encode('OpusHead'));
  const v = new DataView(b.buffer);
  v.setUint8(8, 1);
  v.setUint8(9, channels);
  v.setUint16(10, preSkip, true);
  v.setUint32(12, sampleRate, true);
  v.setInt16(16, 0, true);
  v.setUint8(18, 0);
  return b;
}

const enc = new TextEncoder();

type Part = Uint8Array | number[];

function concat(parts: Part[]): Uint8Array {
  const len = parts.reduce((s, p) => s + p.length, 0);
  const out = new Uint8Array(len);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

/** Element ID bytes (IDs keep their marker bits). */
function idBytes(id: number): number[] {
  const out: number[] = [];
  for (let v = id; v > 0; v = Math.floor(v / 256)) out.unshift(v % 256);
  return out;
}

/** EBML variable-size integer of `n` (shortest form; all-ones is reserved). */
export function vint(n: number): number[] {
  let len = 1;
  while (len < 8 && n >= 2 ** (7 * len) - 1) len++;
  const out = new Array<number>(len).fill(0);
  let v = n;
  for (let i = len - 1; i >= 0; i--) {
    out[i] = v % 256;
    v = Math.floor(v / 256);
  }
  out[0] = (out[0] ?? 0) | (0x80 >> (len - 1));
  return out;
}

function el(id: number, ...payload: Part[]): Uint8Array {
  const body = concat(payload);
  return concat([idBytes(id), vint(body.length), body]);
}

function uint(id: number, n: number): Uint8Array {
  const bytes: number[] = [];
  for (let v = n; v > 0; v = Math.floor(v / 256)) bytes.unshift(v % 256);
  return el(id, bytes.length ? bytes : [0]);
}

function str(id: number, s: string): Uint8Array {
  return el(id, enc.encode(s));
}

function float(id: number, x: number): Uint8Array {
  const b = new Uint8Array(8);
  new DataView(b.buffer).setFloat64(0, x);
  return el(id, b);
}

/** Element IDs used. */
export const WEBM_IDS = {
  EBML: 0x1a45dfa3,
  Segment: 0x18538067,
  Info: 0x1549a966,
  Tracks: 0x1654ae6b,
  TrackEntry: 0xae,
  Video: 0xe0,
  Cluster: 0x1f43b675,
  Timestamp: 0xe7,
  SimpleBlock: 0xa3,
} as const;

/** The WebM file of an encoded VP8 / VP9 track. */
export function muxWebm(input: WebmInput): Uint8Array {
  const ms = (us: number) => Math.round(us / 1000);
  const last = input.chunks[input.chunks.length - 1];
  const durationMs = last ? ms(last.timestampUs + last.durationUs) : 0;
  const header = el(
    WEBM_IDS.EBML,
    uint(0x4286, 1), // EBMLVersion
    uint(0x42f7, 1), // EBMLReadVersion
    uint(0x42f2, 4), // EBMLMaxIDLength
    uint(0x42f3, 8), // EBMLMaxSizeLength
    str(0x4282, 'webm'), // DocType
    uint(0x4287, 4), // DocTypeVersion
    uint(0x4285, 2), // DocTypeReadVersion
  );
  const info = el(
    WEBM_IDS.Info,
    uint(0x2ad7b1, 1_000_000), // TimestampScale: 1 ms
    str(0x4d80, 'BlueSlide'), // MuxingApp
    str(0x5741, 'BlueSlide'), // WritingApp
    float(0x4489, durationMs), // Duration
  );
  const tracks = el(
    WEBM_IDS.Tracks,
    el(
      WEBM_IDS.TrackEntry,
      uint(0xd7, 1), // TrackNumber
      uint(0x73c5, 1), // TrackUID
      uint(0x83, 1), // TrackType: video
      str(0x86, input.codecId), // CodecID
      el(WEBM_IDS.Video, uint(0xb0, input.width), uint(0xba, input.height)),
    ),
    ...(input.audio
      ? [
          el(
            WEBM_IDS.TrackEntry,
            uint(0xd7, 2), // TrackNumber
            uint(0x73c5, 2), // TrackUID
            uint(0x83, 2), // TrackType: audio
            str(0x86, 'A_OPUS'), // CodecID
            el(
              0x63a2, // CodecPrivate
              input.audio.head && input.audio.head.length
                ? input.audio.head
                : opusHead(input.audio.channels, input.audio.sampleRate),
            ),
            el(
              0xe1, // Audio
              float(0xb5, input.audio.sampleRate), // SamplingFrequency
              uint(0x9f, input.audio.channels), // Channels
            ),
          ),
        ]
      : []),
  );
  // Video and audio frames in time order (video first at equal times).
  type Ev = { t: number; track: 1 | 2; key: boolean; data: Uint8Array };
  const events: Ev[] = input.chunks.map((c) => ({
    t: ms(c.timestampUs),
    track: 1,
    key: c.key,
    data: c.data,
  }));
  for (const a of input.audio?.chunks ?? []) {
    events.push({ t: ms(a.timestampUs), track: 2, key: true, data: a.data });
  }
  events.sort((a, b) => a.t - b.t || a.track - b.track);
  const clusters: Uint8Array[] = [];
  let blocks: Uint8Array[] = [];
  let base = 0;
  const close = () => {
    if (blocks.length)
      clusters.push(el(WEBM_IDS.Cluster, uint(WEBM_IDS.Timestamp, base), ...blocks));
    blocks = [];
  };
  for (const c of events) {
    const t = c.t;
    if (!blocks.length || (c.track === 1 && c.key) || t - base > 30_000) {
      close();
      base = t;
    }
    const rel = t - base;
    blocks.push(
      el(
        WEBM_IDS.SimpleBlock,
        [0x80 | c.track, (rel >> 8) & 255, rel & 255, c.key ? 0x80 : 0],
        c.data,
      ),
    );
  }
  close();
  return concat([header, el(WEBM_IDS.Segment, info, tracks, ...clusters)]);
}
