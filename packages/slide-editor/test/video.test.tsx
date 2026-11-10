// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  EncodedVideoChunkData,
  EngineHost,
  ShowVideoInfo,
  ShowVideoOptions,
  VideoCodecChoice,
  VideoEncoding,
} from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ExportVideoDialog, frameHeight } from '../src/exportVideo';
import {
  ExportCancelled,
  exportShow,
  frameTime,
  isKeyFrame,
  VIDEO_UNSUPPORTED,
  type ShowExportRequest,
} from '../src/video/export';
import { MP4_TIMESCALE } from '../src/video/mp4';
import { vint, WEBM_IDS } from '../src/video/webm';

afterEach(cleanup);

const W = 64;
const H = 36;

/** Engine double: `frames` frames at the requested fps, a recognisable byte per frame. */
function fakeEngine(frames: number) {
  const calls: string[] = [];
  let opts: ShowVideoOptions | null = null;
  const engine = {
    videoBegin: vi.fn(async (o: ShowVideoOptions): Promise<ShowVideoInfo> => {
      opts = o;
      calls.push('begin');
      return {
        width: W,
        height: H,
        fps: o.fps,
        frameCount: frames,
        durationMs: (frames * 1000) / o.fps,
        slides: [{ slide: 0, startMs: 0, transitionMs: 0, lengthMs: (frames * 1000) / o.fps }],
      };
    }),
    videoFrame: vi.fn(async (i: number, png?: boolean) =>
      png ? new Uint8Array([0x89, 0x50, i]) : new Uint8Array(W * H * 4).fill(i),
    ),
    videoGifAdd: vi.fn(async () => {
      calls.push('gif');
    }),
    videoGifFinish: vi.fn(async () => new Uint8Array([0x47, 0x49, 0x46])),
    videoEnd: vi.fn(async () => {
      calls.push('end');
    }),
  };
  return { engine: engine as unknown as EngineHost, raw: engine, calls, opts: () => opts };
}

/** Encoder double: each frame becomes a chunk of 3 + (i % 5) bytes. */
function fakeVideo(choice: VideoCodecChoice | null): VideoEncoding & { encoded: number } {
  const v = {
    encoded: 0,
    probe: vi.fn(async () => choice),
    open: vi.fn(async () => {
      const chunks: EncodedVideoChunkData[] = [];
      return {
        encode: async (rgba: Uint8Array, ts: number, dur: number, key: boolean) => {
          v.encoded++;
          const n = 3 + (chunks.length % 5);
          chunks.push({
            data: new Uint8Array(n).fill(rgba[0] ?? 0),
            timestampUs: ts,
            durationUs: dur,
            key,
          });
        },
        flush: async () => ({ chunks, description: new Uint8Array([1, 0x64, 0, 0x28, 0xff]) }),
        close: () => {},
      };
    }),
  };
  return v;
}

const req = (over: Partial<ShowExportRequest> = {}): ShowExportRequest => ({
  format: 'video',
  width: W,
  fps: 10,
  secondsPerSlide: 3,
  useTimings: true,
  loop: true,
  ...over,
});

// ------------------------------------------------------------------ MP4 box reader

interface Box {
  type: string;
  start: number;
  size: number;
  body: Uint8Array;
}

function boxes(b: Uint8Array, from = 0, to = b.length): Box[] {
  const out: Box[] = [];
  const dv = new DataView(b.buffer, b.byteOffset, b.byteLength);
  let o = from;
  while (o + 8 <= to) {
    const size = dv.getUint32(o);
    const type = String.fromCharCode(...b.subarray(o + 4, o + 8));
    out.push({ type, start: o, size, body: b.subarray(o + 8, o + size) });
    o += size;
  }
  expect(o).toBe(to);
  return out;
}

const child = (b: Box, type: string, skip = 0): Box => {
  const all = boxes(b.body, skip, b.body.length);
  const c = all.find((x) => x.type === type);
  if (!c) throw new Error(`no ${type} in ${b.type}: ${all.map((x) => x.type).join(',')}`);
  return c;
};
const path = (b: Box, ...types: string[]) => types.reduce((cur, t) => child(cur, t), b);
const u32 = (b: Uint8Array, o: number) => new DataView(b.buffer, b.byteOffset).getUint32(o);

describe('slide show export', () => {
  it('lays frames on the exact fps grid with a key frame every two seconds', () => {
    let sum = 0;
    for (let i = 0; i < 90; i++) {
      const { ts, dur } = frameTime(i, 30);
      expect(ts).toBe(sum);
      sum += dur;
    }
    expect(sum).toBe(3_000_000);
    expect([0, 1, 59, 60, 120].map((i) => isKeyFrame(i, 30))).toEqual([
      true,
      false,
      false,
      true,
      true,
    ]);
  });

  it('muxes the encoded frames into an MP4 whose boxes index every sample', async () => {
    const { engine, raw, calls } = fakeEngine(30);
    const video = fakeVideo({ container: 'mp4', codec: 'avc1.640028' });
    const progress: number[] = [];
    const r = await exportShow(engine, video, req(), (d) => progress.push(d));
    expect(r.ext).toBe('mp4');
    expect(r.mime).toBe('video/mp4');
    expect(video.encoded).toBe(30);
    expect(progress.at(-1)).toBe(30);
    expect(raw.videoFrame).toHaveBeenCalledTimes(30);
    expect(calls.at(-1)).toBe('end');

    const top = boxes(r.bytes);
    expect(top.map((b) => b.type)).toEqual(['ftyp', 'moov', 'mdat']);
    const moov = top[1]!;
    expect(boxes(moov.body).map((b) => b.type)).toEqual(['mvhd', 'trak']);
    // mvhd: timescale 1000, duration 3 s.
    const mvhd = child(moov, 'mvhd').body;
    expect([u32(mvhd, 12), u32(mvhd, 16)]).toEqual([1000, 3000]);
    const stbl = path(moov, 'trak', 'mdia', 'minf', 'stbl');
    expect(boxes(stbl.body).map((b) => b.type)).toEqual([
      'stsd',
      'stts',
      'stss',
      'stsc',
      'stsz',
      'stco',
    ]);
    const mdhd = path(moov, 'trak', 'mdia', 'mdhd').body;
    expect([u32(mdhd, 12), u32(mdhd, 16)]).toEqual([MP4_TIMESCALE, 3 * MP4_TIMESCALE]);
    // stsd → avc1 (78-byte visual sample entry) → avcC = the encoder's description.
    const stsd = child(stbl, 'stsd');
    const avc1 = boxes(stsd.body, 8)[0]!;
    expect(avc1.type).toBe('avc1');
    const avcC = boxes(avc1.body, 78)[0]!;
    expect(avcC.type).toBe('avcC');
    expect(Array.from(avcC.body)).toEqual([1, 0x64, 0, 0x28, 0xff]);
    expect([avc1.body[24]! * 256 + avc1.body[25]!, avc1.body[26]! * 256 + avc1.body[27]!]).toEqual([
      W,
      H,
    ]);
    // stts: one run of 30 samples of 1/10 s.
    const stts = child(stbl, 'stts').body;
    expect([u32(stts, 4), u32(stts, 8), u32(stts, 12)]).toEqual([1, 30, MP4_TIMESCALE / 10]);
    // stss: key frames 1 and 21 (every 2 s at 10 fps).
    const stss = child(stbl, 'stss').body;
    expect([u32(stss, 4), u32(stss, 8), u32(stss, 12)]).toEqual([2, 1, 21]);
    // stsz: 30 sizes; stco points at the first sample inside mdat.
    const stsz = child(stbl, 'stsz').body;
    expect(u32(stsz, 8)).toBe(30);
    const sizes = Array.from({ length: 30 }, (_, i) => u32(stsz, 12 + i * 4));
    expect(sizes.slice(0, 6)).toEqual([3, 4, 5, 6, 7, 3]);
    const mdat = top[2]!;
    expect(mdat.body.length).toBe(sizes.reduce((a, b) => a + b, 0));
    const off = u32(child(stbl, 'stco').body, 8);
    expect(off).toBe(mdat.start + 8);
    expect(Array.from(r.bytes.subarray(off + 3, off + 7))).toEqual([1, 1, 1, 1]);
  });

  it('writes WebM (VP9) with one cluster per key frame', async () => {
    const { engine } = fakeEngine(45);
    const video = fakeVideo({ container: 'webm', codec: 'vp09.00.40.08' });
    const r = await exportShow(engine, video, req(), () => {});
    expect(r.ext).toBe('webm');
    const b = r.bytes;
    expect(Array.from(b.subarray(0, 4))).toEqual([0x1a, 0x45, 0xdf, 0xa3]);
    const text = new TextDecoder('latin1').decode(b);
    expect(text).toContain('webm');
    expect(text).toContain('V_VP9');
    // Count SimpleBlock (0xA3) elements that carry track 1 (0x81), and clusters.
    const count = (id: number[]) => {
      let n = 0;
      for (let i = 0; i + id.length <= b.length; i++) {
        if (id.every((v, k) => b[i + k] === v)) n++;
      }
      return n;
    };
    expect(count([0x1f, 0x43, 0xb6, 0x75])).toBe(3);
    expect(WEBM_IDS.SimpleBlock).toBe(0xa3);
    expect(vint(5)).toEqual([0x85]);
    expect(vint(127)).toEqual([0x40, 0x7f]);
    expect(vint(300)).toEqual([0x41, 0x2c]);
  });

  it('exports an animated GIF and PNG frames through the engine', async () => {
    const gif = fakeEngine(12);
    const r = await exportShow(gif.engine, undefined, req({ format: 'gif', fps: 30 }), () => {});
    expect(r.ext).toBe('gif');
    // A GIF is at most 15 frames per second.
    expect(gif.opts()?.fps).toBe(15);
    expect(gif.calls.filter((c) => c === 'gif')).toHaveLength(12);
    const png = fakeEngine(3);
    const z = await exportShow(png.engine, undefined, req({ format: 'png' }), () => {});
    expect(z.ext).toBe('zip');
    expect(new TextDecoder().decode(z.bytes)).toContain('frame-0003.png');
  });

  it('stops when cancelled or when video is not available', async () => {
    const { engine, calls } = fakeEngine(50);
    const ctl = new AbortController();
    const video = fakeVideo({ container: 'mp4', codec: 'avc1.640028' });
    await expect(
      exportShow(engine, video, req(), (d) => d === 3 && ctl.abort(), ctl.signal),
    ).rejects.toBeInstanceOf(ExportCancelled);
    expect(video.encoded).toBe(3);
    expect(calls.at(-1)).toBe('end');
    await expect(exportShow(engine, fakeVideo(null), req(), () => {})).rejects.toThrow(
      VIDEO_UNSUPPORTED,
    );
    await expect(exportShow(engine, undefined, req(), () => {})).rejects.toThrow(VIDEO_UNSUPPORTED);
  });
});

describe('export video dialog', () => {
  const i18n = createI18n('en-US');
  const slide = { width: 960, height: 540 };

  it('offers GIF / PNG only when the browser cannot encode video', async () => {
    const run = vi.fn(async (_r: ShowExportRequest) => {});
    const onClose = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <ExportVideoDialog slide={slide} video={fakeVideo(null)} onClose={onClose} run={run} />
      </I18nextProvider>,
    );
    await screen.findByTestId('export-video-unsupported');
    expect((screen.getByTestId('export-video-format-video') as HTMLInputElement).disabled).toBe(
      true,
    );
    expect((screen.getByTestId('export-video-format-gif') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('export-video-narration') as HTMLInputElement).disabled).toBe(true);
    expect(screen.getByTestId('export-video-width').textContent).toContain('1080 × 608 pixels');
    fireEvent.change(screen.getByTestId('export-video-fps'), { target: { value: '15' } });
    fireEvent.change(screen.getByTestId('export-video-seconds'), { target: { value: '2.5' } });
    fireEvent.click(screen.getByTestId('export-video-loop'));
    fireEvent.click(screen.getByTestId('export-video-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(run.mock.calls[0]![0]).toEqual({
      format: 'gif',
      width: 720,
      fps: 15,
      secondsPerSlide: 2.5,
      useTimings: true,
      loop: false,
    });
  });

  it('picks video when supported, shows progress and cancels', async () => {
    const h: { signal?: AbortSignal; report?: (d: number, t: number) => void } = {};
    const run = vi.fn(
      (_r: ShowExportRequest, onProgress: (d: number, t: number) => void, s: AbortSignal) =>
        new Promise<void>((_ok, fail) => {
          h.signal = s;
          h.report = onProgress;
          s.addEventListener('abort', () => fail(new ExportCancelled()));
        }),
    );
    const onClose = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <ExportVideoDialog
          slide={slide}
          video={fakeVideo({ container: 'mp4', codec: 'avc1.640028' })}
          onClose={onClose}
          run={run}
        />
      </I18nextProvider>,
    );
    await waitFor(() =>
      expect((screen.getByTestId('export-video-format-video') as HTMLInputElement).checked).toBe(
        true,
      ),
    );
    expect(screen.getByText('Video (MP4)')).toBeTruthy();
    fireEvent.click(screen.getByTestId('export-video-ok'));
    await waitFor(() => expect(run).toHaveBeenCalled());
    expect(run.mock.calls[0]![0]).toMatchObject({ format: 'video', fps: 30, narration: true });
    act(() => h.report?.(5, 20));
    expect(screen.getByText('Frame 5 of 20')).toBeTruthy();
    expect(screen.getByTestId('export-video-progress').getAttribute('value')).toBe('0.25');
    fireEvent.click(screen.getByTestId('export-video-cancel'));
    expect(h.signal?.aborted).toBe(true);
    await screen.findByText('Export cancelled');
    expect(onClose).not.toHaveBeenCalled();
    expect(frameHeight(480, slide)).toBe(270);
  });
});
