import type { AudioSource, DecodedAudio } from '@blueoffice/platform';

/**
 * The narration track of a video export (SLIDE-REC-001): every slide's recorded narration
 * placed where the slide starts playing (after its transition), cut at the slide's end, mixed
 * down to one channel and resampled (linear interpolation) to the track's rate. Silence
 * elsewhere. No DOM; unit-tested.
 */

/** A clip on the video timeline. */
export interface NarrationClip {
  /** Where it starts (ms from the start of the video). */
  startMs: number;
  /** Where it must stop (ms). */
  endMs: number;
  audio: DecodedAudio;
}

/** Sample rate of the exported narration (Opus and AAC both take it). */
export const NARRATION_RATE = 48_000;

/** Mono samples of `a` (channels averaged). */
function mono(a: DecodedAudio): Float32Array {
  const n = a.channels[0]?.length ?? 0;
  if (a.channels.length <= 1) return a.channels[0] ?? new Float32Array(0);
  const out = new Float32Array(n);
  for (const c of a.channels)
    for (let i = 0; i < n; i++) out[i]! += (c[i] ?? 0) / a.channels.length;
  return out;
}

export function narrationTrack(
  clips: NarrationClip[],
  durationMs: number,
  rate = NARRATION_RATE,
): AudioSource {
  const frames = Math.max(0, Math.round((durationMs * rate) / 1000));
  const placed = clips.map((c) => {
    const pcm = mono(c.audio);
    const start = Math.round((c.startMs * rate) / 1000);
    const len = Math.round((pcm.length * rate) / Math.max(1, c.audio.sampleRate));
    const end = Math.min(start + len, Math.round((c.endMs * rate) / 1000), frames);
    return { pcm, start, end, step: c.audio.sampleRate / rate };
  });
  return {
    sampleRate: rate,
    channels: 1,
    frames,
    read(at: number, count: number): Float32Array[] {
      const out = new Float32Array(count);
      for (const p of placed) {
        const from = Math.max(at, p.start);
        const to = Math.min(at + count, p.end);
        for (let f = from; f < to; f++) {
          const x = (f - p.start) * p.step;
          const i = Math.floor(x);
          const a = p.pcm[i] ?? 0;
          const b = p.pcm[i + 1] ?? a;
          const v = a + (b - a) * (x - i);
          out[f - at] = Math.max(-1, Math.min(1, out[f - at]! + v));
        }
      }
      return [out];
    },
  };
}

/** Draw `src` (`sw` × `sh` RGBA) into `dst` (`dw` px wide) at the box (nearest pixel). */
export function blit(
  dst: Uint8Array,
  dw: number,
  dh: number,
  src: Uint8ClampedArray,
  sw: number,
  sh: number,
  box: { x: number; y: number; w: number; h: number },
): void {
  const x0 = Math.max(0, Math.round(box.x));
  const y0 = Math.max(0, Math.round(box.y));
  const x1 = Math.min(dw, Math.round(box.x + box.w));
  const y1 = Math.min(dh, Math.round(box.y + box.h));
  if (box.w <= 0 || box.h <= 0) return;
  for (let y = y0; y < y1; y++) {
    const sy = Math.min(sh - 1, Math.floor(((y - box.y) * sh) / box.h));
    for (let x = x0; x < x1; x++) {
      const sx = Math.min(sw - 1, Math.floor(((x - box.x) * sw) / box.w));
      const s = (sy * sw + sx) * 4;
      const d = (y * dw + x) * 4;
      dst[d] = src[s]!;
      dst[d + 1] = src[s + 1]!;
      dst[d + 2] = src[s + 2]!;
      dst[d + 3] = 255;
    }
  }
}
