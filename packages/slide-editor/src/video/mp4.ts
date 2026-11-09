import type { EncodedVideoChunkData } from '@blueoffice/platform';

/**
 * Minimal MP4 writer for one H.264 video track (ISO/IEC 14496-12 ISO base media file format,
 * ISO/IEC 14496-15 for the `avc1` sample entry and its `avcC` configuration). The index
 * (`moov`) comes before the media data (`mdat`) so players can start at once; every sample
 * sits in one chunk. Samples are in decoding order and presentation = decoding time (the
 * WebCodecs encoders used produce no reordered frames).
 */

/** Media timescale (ticks per second). */
export const MP4_TIMESCALE = 90_000;

export interface Mp4Input {
  width: number;
  height: number;
  /** H.264 `AVCDecoderConfigurationRecord` (WebCodecs `decoderConfig.description`). */
  avcC: Uint8Array;
  chunks: EncodedVideoChunkData[];
}

const enc = new TextEncoder();

function u32(n: number): number[] {
  return [(n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255];
}
function u16(n: number): number[] {
  return [(n >>> 8) & 255, n & 255];
}
const zeros = (n: number): number[] => new Array<number>(n).fill(0);

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

/** A box: 32-bit size, 4-character type, payload. */
function box(type: string, ...payload: Part[]): Uint8Array {
  const body = concat(payload);
  return concat([u32(body.length + 8), Array.from(enc.encode(type)), body]);
}

/** A full box: box + version + 24-bit flags. */
function full(type: string, version: number, flags: number, ...payload: Part[]): Uint8Array {
  return box(type, [version, (flags >>> 16) & 255, (flags >>> 8) & 255, flags & 255], ...payload);
}

/** Unity transformation matrix (§8.2.2 / §8.3.2, 16.16 and 2.30 fixed point). */
const MATRIX = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000].flatMap(u32);

/** Decoding times (timescale ticks) of the samples and the end of the last one. */
export function sampleTimes(chunks: EncodedVideoChunkData[]): { starts: number[]; end: number } {
  const tick = (us: number) => Math.round((us * MP4_TIMESCALE) / 1e6);
  const starts = chunks.map((c) => tick(c.timestampUs));
  const last = chunks[chunks.length - 1];
  const end = last ? tick(last.timestampUs + last.durationUs) : 0;
  return { starts, end };
}

/** Run-length `(count, delta)` pairs of the sample durations (§8.6.1.2 stts). */
function sttsEntries(starts: number[], end: number): [number, number][] {
  const out: [number, number][] = [];
  starts.forEach((s, i) => {
    const d = Math.max(0, (starts[i + 1] ?? end) - s);
    const prev = out[out.length - 1];
    if (prev && prev[1] === d) prev[0]++;
    else out.push([1, d]);
  });
  return out;
}

function moov(input: Mp4Input, mdatDataOffset: number): Uint8Array {
  const { width, height, chunks } = input;
  const { starts, end } = sampleTimes(chunks);
  const durMs = Math.round((end * 1000) / MP4_TIMESCALE);
  const mvhd = full(
    'mvhd',
    0,
    0,
    u32(0),
    u32(0),
    u32(1000),
    u32(durMs),
    u32(0x00010000),
    u16(0x0100),
    zeros(10),
    MATRIX,
    zeros(24),
    u32(2),
  );
  const tkhd = full(
    'tkhd',
    0,
    3, // enabled, in movie
    u32(0),
    u32(0),
    u32(1),
    u32(0),
    u32(durMs),
    zeros(8),
    u16(0),
    u16(0),
    u16(0),
    u16(0),
    MATRIX,
    u32(width * 0x10000),
    u32(height * 0x10000),
  );
  // Language `und` (ISO 639-2/T packed in 5-bit letters).
  const mdhd = full(
    'mdhd',
    0,
    0,
    u32(0),
    u32(0),
    u32(MP4_TIMESCALE),
    u32(end),
    u16(0x55c4),
    u16(0),
  );
  const hdlr = full(
    'hdlr',
    0,
    0,
    u32(0),
    Array.from(enc.encode('vide')),
    zeros(12),
    Array.from(enc.encode('BlueSlide video\0')),
  );
  const vmhd = full('vmhd', 0, 1, zeros(8));
  const dinf = box('dinf', full('dref', 0, 0, u32(1), full('url ', 0, 1)));
  const avc1 = box(
    'avc1',
    zeros(6),
    u16(1), // data_reference_index
    zeros(16),
    u16(width),
    u16(height),
    u32(0x00480000), // 72 dpi
    u32(0x00480000),
    u32(0),
    u16(1), // frame_count
    zeros(32), // compressorname
    u16(0x0018),
    u16(0xffff),
    box('avcC', input.avcC),
  );
  const stsd = full('stsd', 0, 0, u32(1), avc1);
  const runs = sttsEntries(starts, end);
  const stts = full(
    'stts',
    0,
    0,
    u32(runs.length),
    runs.flatMap(([n, d]) => [...u32(n), ...u32(d)]),
  );
  const keys = chunks.flatMap((c, i) => (c.key ? [i + 1] : []));
  const stss = full('stss', 0, 0, u32(keys.length), keys.flatMap(u32));
  const stsc = full('stsc', 0, 0, u32(1), u32(1), u32(chunks.length), u32(1));
  const stsz = full(
    'stsz',
    0,
    0,
    u32(0),
    u32(chunks.length),
    chunks.flatMap((c) => u32(c.data.length)),
  );
  const stco = full('stco', 0, 0, u32(1), u32(mdatDataOffset));
  const stbl = box('stbl', stsd, stts, stss, stsc, stsz, stco);
  const minf = box('minf', vmhd, dinf, stbl);
  const mdia = box('mdia', mdhd, hdlr, minf);
  return box('moov', mvhd, box('trak', tkhd, mdia));
}

/** The MP4 file of an encoded H.264 track. */
export function muxMp4(input: Mp4Input): Uint8Array {
  const ftyp = box(
    'ftyp',
    Array.from(enc.encode('isom')),
    u32(0x200),
    Array.from(enc.encode('isomiso2avc1mp41')),
  );
  // The moov size does not depend on the offset value: size it once, then fill it in.
  const probe = moov(input, 0);
  const dataOffset = ftyp.length + probe.length + 8;
  const media = concat(input.chunks.map((c) => c.data));
  const mdat = concat([u32(media.length + 8), Array.from(enc.encode('mdat')), media]);
  return concat([ftyp, moov(input, dataOffset), mdat]);
}
