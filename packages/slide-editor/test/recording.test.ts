import {
  FakeMediaCapture,
  decodeWav,
  encodeWav,
  type AudioSource,
  type EncodedVideoChunkData,
  type EngineHost,
  type ShowVideoInfo,
  type ShowVideoOptions,
  type SlideMediaInfo,
  type VideoEncoding,
} from '@blueoffice/platform';
import { describe, expect, it, vi } from 'vitest';
import { InkLayer } from '../src/ink';
import { ShowRecorder, recordingInput } from '../src/recording';
import { blit, narrationTrack } from '../src/video/audioMix';
import { exportShow } from '../src/video/export';

/** A clock the test moves. */
function clock(start = 1000) {
  let t = start;
  return { now: () => t, tick: (ms: number) => (t += ms) };
}

describe('show recorder (SLIDE-REC-001)', () => {
  it('records one take per slide with clicks, pauses left out', async () => {
    const c = clock();
    const media = new FakeMediaCapture(c.now);
    const session = await media.open({ audio: true, video: true });
    const r = new ShowRecorder(session, c.now);
    expect(r.state).toBe('idle');
    r.start(0);
    expect(r.state).toBe('recording');
    c.tick(1500);
    r.click();
    c.tick(500);
    r.pause();
    expect(r.state).toBe('paused');
    c.tick(10_000); // paused: not counted, clicks ignored
    r.click();
    r.resume();
    c.tick(1000);
    expect(r.elapsed()).toBe(3000);
    expect(r.enter(0)).toBe(false);
    expect(r.enter(2)).toBe(true);
    c.tick(4000);
    expect(r.enter(null)).toBe(false); // end screen closes the take
    const takes = await r.finish();
    expect(r.state).toBe('finished');
    expect(takes.map((t) => [t.slide, t.durationMs, t.clicks])).toEqual([
      [0, 3000, [1500]],
      [2, 4000, []],
    ]);
    // Narration as long as the slide's take (8 kHz WAV), a camera clip.
    const a = takes[0]!.segment!.audio!;
    expect(a.mime).toBe('audio/wav');
    expect(decodeWav(a.bytes)!.channels[0]!.length).toBe(24_000);
    expect(takes[1]!.segment!.video!.mime).toBe('video/webm');
    expect(media.segments).toBe(2);
  });

  it('records a slide shown again anew and restarts the current slide', async () => {
    const c = clock();
    const r = new ShowRecorder(null, c.now);
    r.start(1);
    c.tick(2000);
    r.enter(2);
    c.tick(1000);
    r.enter(1);
    c.tick(700);
    r.click();
    c.tick(100);
    r.restartSlide();
    c.tick(900);
    r.click();
    const takes = await r.finish();
    expect(takes.map((t) => [t.slide, t.durationMs, t.clicks])).toEqual([
      [1, 900, [900]],
      [2, 1000, []],
    ]);
    expect(takes[0]!.segment).toBeNull();
    // Nothing more after the end.
    expect(r.enter(3)).toBe(false);
    expect(await r.finish()).toEqual([]);
  });

  it('turns takes and timed ink into saveRecording slides', async () => {
    const c = clock();
    const media = new FakeMediaCapture(c.now);
    const session = await media.open({ audio: true, video: true });
    const r = new ShowRecorder(session, c.now);
    const ink = new InkLayer();
    ink.setClock(() => r.inkTime());
    r.start(0);
    c.tick(1200);
    ink.begin(0, 'pen', 'D7261E', [10, 10]);
    c.tick(40);
    ink.extend([20, 15]);
    c.tick(40);
    ink.extend([30, 10]);
    ink.end();
    ink.setClock(null);
    ink.begin(0, 'pen', '000000', [1, 1]); // not timed: left out
    ink.end();
    c.tick(800);
    const takes = await r.finish();
    const slides = recordingInput(takes, (s) => ink.timed(s), {
      narration: true,
      camera: true,
      corner: 'topLeft',
      aspect: 16 / 9,
    });
    expect(slides).toHaveLength(1);
    const s = slides[0]!;
    expect(s.durationMs).toBe(2080);
    expect(s.ink).toEqual([
      {
        points: [
          [10, 10],
          [20, 15],
          [30, 10],
        ],
        times: [1200, 1240, 1280],
        color: 'D7261E',
        width: 2,
      },
    ]);
    expect(s.narration?.mime).toBe('audio/wav');
    expect(atob(s.narration!.data).slice(0, 4)).toBe('RIFF');
    expect(s.camera).toMatchObject({ mime: 'video/webm', corner: 'topLeft', aspect: 16 / 9 });
    // keepInk payload never carries times.
    expect(JSON.stringify(ink.toCommand())).not.toContain('times');
    // Without narration / camera chosen, only timings and ink.
    const bare = recordingInput(takes, () => [], {
      narration: false,
      camera: false,
      corner: 'topLeft',
      aspect: null,
    });
    expect(bare[0]).toEqual({ slide: 0, durationMs: 2080 });
  });

  it('refuses when devices are denied', async () => {
    await expect(
      new FakeMediaCapture(Date.now, { microphone: true, camera: true }, true).open({
        audio: true,
        video: false,
      }),
    ).rejects.toThrow('media.denied');
    await expect(
      new FakeMediaCapture(Date.now, { microphone: false, camera: false }).open({
        audio: true,
        video: true,
      }),
    ).rejects.toThrow('media.none');
  });
});

describe('narration track and camera in the video export', () => {
  it('places, cuts, mixes and resamples narration', () => {
    const one = (n: number, v: number) => new Float32Array(n).fill(v);
    const src = narrationTrack(
      [
        // 8 kHz, 0.5 s at 0.5 → 1 s..1.5 s.
        { startMs: 1000, endMs: 5000, audio: { sampleRate: 8000, channels: [one(4000, 0.5)] } },
        // Stereo averaged (0.2, 0.4 → 0.3), cut at its slide's end (2.1 s).
        {
          startMs: 2000,
          endMs: 2100,
          audio: { sampleRate: 1000, channels: [one(1000, 0.2), one(1000, 0.4)] },
        },
      ],
      3000,
      1000,
    );
    expect([src.sampleRate, src.channels, src.frames]).toEqual([1000, 1, 3000]);
    const all = src.read(0, 3000)[0]!;
    expect(all[999]).toBe(0);
    expect(all[1000]).toBeCloseTo(0.5);
    expect(all[1499]).toBeCloseTo(0.5);
    expect(all[1500]).toBe(0);
    expect(all[2050]).toBeCloseTo(0.3);
    expect(all[2100]).toBe(0);
    // Blocks read the same samples.
    expect(Array.from(src.read(1495, 10)[0]!)).toEqual(Array.from(all.subarray(1495, 1505)));
  });

  it('draws a picture into a box of the frame', () => {
    const dst = new Uint8Array(4 * 4 * 4);
    const src = new Uint8ClampedArray(2 * 2 * 4).fill(200);
    blit(dst, 4, 4, src, 2, 2, { x: 2, y: 2, w: 2, h: 2 });
    expect(dst[0]).toBe(0);
    expect(dst[(2 * 4 + 2) * 4]).toBe(200);
    expect(dst[(3 * 4 + 3) * 4 + 3]).toBe(255);
  });

  const W = 32;
  const H = 18;

  function engine(): EngineHost {
    const wav = encodeWav(8000, [new Float32Array(8000).fill(0.25)]);
    const media: SlideMediaInfo[] = [
      {
        id: 5,
        name: 'Narration 5',
        kind: 'audio',
        rect: [0, 0, 1, 1],
        part: '/ppt/media/media1.wav',
        mime: 'audio/wav',
        link: null,
        recorded: 'narration',
      },
      {
        id: 6,
        name: 'Camera 6',
        kind: 'video',
        rect: [0, 0, 480, 270],
        part: '/ppt/media/media2.webm',
        mime: 'video/webm',
        link: null,
        recorded: 'camera',
      },
    ];
    return {
      videoBegin: async (o: ShowVideoOptions): Promise<ShowVideoInfo> => ({
        width: W,
        height: H,
        fps: o.fps,
        frameCount: 20,
        durationMs: 2000,
        slides: [{ slide: 0, startMs: 0, transitionMs: 500, lengthMs: 2000 }],
      }),
      videoFrame: async () => new Uint8Array(W * H * 4),
      videoGifAdd: async () => {},
      videoGifFinish: async () => new Uint8Array(),
      videoEnd: async () => {},
      slideCommand: async () => ({ show: { width: 960, media } }),
      getMedia: async (part: string) => (part.endsWith('.wav') ? wav : new Uint8Array([1])),
    } as unknown as EngineHost;
  }

  function encoder(container: 'mp4' | 'webm', audio: boolean) {
    const frames: Uint8Array[] = [];
    const heard: AudioSource[] = [];
    const v: VideoEncoding = {
      probe: async () => ({ container, codec: container === 'mp4' ? 'avc1.640028' : 'vp8' }),
      open: async () => {
        const chunks: EncodedVideoChunkData[] = [];
        return {
          encode: async (rgba, ts, dur, key) => {
            frames.push(rgba.slice());
            chunks.push({ data: new Uint8Array([7]), timestampUs: ts, durationUs: dur, key });
          },
          flush: async () => ({ chunks, description: new Uint8Array([1, 0x64, 0, 0x28]) }),
          close: () => {},
        };
      },
      ...(audio
        ? {
            encodeAudio: vi.fn(async (_c: 'mp4' | 'webm', s: AudioSource) => {
              heard.push(s);
              return {
                codec: container === 'mp4' ? ('aac' as const) : ('opus' as const),
                sampleRate: s.sampleRate,
                channels: 1,
                description: null,
                chunks: [0, 1, 2].map((i) => ({
                  data: new Uint8Array([0xaa, i]),
                  timestampUs: i * 21_333,
                  durationUs: 21_333,
                })),
              };
            }),
          }
        : {}),
    };
    return { v, frames, heard };
  }

  it('muxes the narration as an AAC track and draws the camera (MP4)', async () => {
    const enc = encoder('mp4', true);
    const media = new FakeMediaCapture();
    const req = {
      format: 'video' as const,
      width: W,
      fps: 10,
      secondsPerSlide: 2,
      useTimings: true,
      loop: false,
      narration: true,
    };
    const r = await exportShow(engine(), enc.v, req, () => {}, undefined, media);
    expect(r.audioDropped).toBe(false);
    const text = new TextDecoder('latin1').decode(r.bytes);
    expect(text).toContain('soun');
    expect(text).toContain('mp4a');
    expect(text).toContain('esds');
    // The narration starts after the transition (0.5 s) and lasts 1 s at 48 kHz.
    const s = enc.heard[0]!;
    expect(s.frames).toBe(96_000);
    const pcm = s.read(0, s.frames)[0]!;
    expect(pcm[23_999]).toBe(0);
    expect(pcm[24_100]).toBeCloseTo(0.25);
    expect(pcm[72_100]).toBe(0);
    // Camera: frames from 0.5 s on have its grey picture in the top-left quarter.
    expect(enc.frames[2]![0]).toBe(0);
    expect(enc.frames[5]![0]).toBe(128);
    expect(enc.frames[5]![(H - 1) * W * 4 + (W - 1) * 4]).toBe(0);
  });

  it('writes an Opus track in WebM and drops the sound without an audio encoder', async () => {
    const req = {
      format: 'video' as const,
      width: W,
      fps: 10,
      secondsPerSlide: 2,
      useTimings: true,
      loop: false,
      narration: true,
    };
    const media = new FakeMediaCapture();
    const webm = encoder('webm', true);
    const r = await exportShow(engine(), webm.v, req, () => {}, undefined, media);
    const text = new TextDecoder('latin1').decode(r.bytes);
    expect(text).toContain('A_OPUS');
    expect(text).toContain('OpusHead');
    const mute = encoder('mp4', false);
    const r2 = await exportShow(engine(), mute.v, req, () => {}, undefined, media);
    expect(r2.audioDropped).toBe(true);
    expect(new TextDecoder('latin1').decode(r2.bytes)).not.toContain('soun');
    // Narration off: no recorded clip is looked at.
    const off = encoder('mp4', true);
    const r3 = await exportShow(
      engine(),
      off.v,
      { ...req, narration: false },
      () => {},
      undefined,
      media,
    );
    expect(r3.audioDropped).toBe(false);
    expect(off.heard).toHaveLength(0);
  });
});
