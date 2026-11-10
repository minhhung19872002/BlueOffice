import { describe, expect, it } from 'vitest';
import {
  AUDIO_TYPES,
  FakeMediaCapture,
  MEDIA_DENIED,
  MEDIA_NONE,
  WebMediaCapture,
  decodeWav,
  encodeWav,
  webMediaCapture,
  type WebMediaDeps,
} from '../src/mediaCapture';

/** MediaRecorder double: emits one blob per start and on stop. */
class FakeRecorder {
  static made: FakeRecorder[] = [];
  static isTypeSupported = (t: string) => t === 'audio/webm;codecs=opus' || t === 'video/webm';
  state = 'inactive';
  ondataavailable: ((e: { data: Blob }) => void) | null = null;
  onstop: (() => void) | null = null;
  readonly log: string[] = [];
  constructor(
    readonly stream: unknown,
    readonly options?: { mimeType?: string },
  ) {
    FakeRecorder.made.push(this);
  }
  get mimeType() {
    return this.options?.mimeType ?? '';
  }
  start() {
    this.state = 'recording';
    this.log.push('start');
  }
  pause() {
    this.state = 'paused';
    this.log.push('pause');
  }
  resume() {
    this.state = 'recording';
    this.log.push('resume');
  }
  stop() {
    this.state = 'inactive';
    this.log.push('stop');
    this.ondataavailable?.({ data: new Blob([new Uint8Array([1, 2, 3])]) });
    this.onstop?.();
  }
}

function deps(over: Partial<WebMediaDeps> = {}) {
  let t = 0;
  const stopped: string[] = [];
  const track = (kind: string) => ({
    kind,
    stop: () => stopped.push(kind),
    getSettings: () => (kind === 'video' ? { width: 1280, height: 720 } : {}),
  });
  const asked: unknown[] = [];
  const d: WebMediaDeps = {
    getUserMedia: async (c) => {
      asked.push(c);
      const tracks = [...(c.audio ? [track('audio')] : []), ...(c.video ? [track('video')] : [])];
      return {
        getTracks: () => tracks,
        getAudioTracks: () => tracks.filter((x) => x.kind === 'audio'),
        getVideoTracks: () => tracks.filter((x) => x.kind === 'video'),
      };
    },
    MediaRecorder: FakeRecorder as unknown as WebMediaDeps['MediaRecorder'],
    makeStream: (tracks) => ({ tracks }),
    now: () => t,
    hasMicrophone: true,
    hasCamera: true,
    ...over,
  };
  return { d, tick: (ms: number) => (t += ms), stopped, asked };
}

describe('media capture (SLIDE-REC-001)', () => {
  it('WAV round trip', () => {
    const pcm = new Float32Array([0, 0.5, -0.5, 1]);
    const w = encodeWav(8000, [pcm]);
    expect(String.fromCharCode(...w.subarray(0, 4))).toBe('RIFF');
    const back = decodeWav(w)!;
    expect(back.sampleRate).toBe(8000);
    expect(Array.from(back.channels[0]!).map((v) => Math.round(v * 100) / 100)).toEqual([
      0, 0.5, -0.5, 1,
    ]);
    expect(decodeWav(new Uint8Array(10))).toBeNull();
  });

  it('records audio and camera segments with their own recorders', async () => {
    FakeRecorder.made = [];
    const { d, tick, stopped, asked } = deps();
    const m = new WebMediaCapture(d);
    const s = await m.open({ audio: true, video: true });
    expect(asked[0]).toMatchObject({ audio: true });
    expect([s.audio, s.video, s.aspect]).toEqual([true, true, 1280 / 720]);
    const seg = s.segment();
    expect(FakeRecorder.made.map((r) => r.mimeType)).toEqual([AUDIO_TYPES[0], 'video/webm']);
    tick(1000);
    seg.pause();
    tick(5000);
    seg.resume();
    tick(500);
    const r = await seg.stop();
    expect(r.durationMs).toBe(1500);
    expect(r.audio).toEqual({ bytes: new Uint8Array([1, 2, 3]), mime: 'audio/webm;codecs=opus' });
    expect(r.video?.mime).toBe('video/webm');
    expect(FakeRecorder.made[0]!.log).toEqual(['start', 'pause', 'resume', 'stop']);
    s.close();
    expect(stopped.sort()).toEqual(['audio', 'video']);
  });

  it('maps refusals and missing devices', async () => {
    const denied = deps({
      getUserMedia: async () => {
        throw Object.assign(new Error('x'), { name: 'NotAllowedError' });
      },
    });
    await expect(new WebMediaCapture(denied.d).open({ audio: true, video: false })).rejects.toThrow(
      MEDIA_DENIED,
    );
    const none = deps({
      getUserMedia: async () => {
        throw Object.assign(new Error('x'), { name: 'NotFoundError' });
      },
    });
    await expect(new WebMediaCapture(none.d).open({ audio: true, video: false })).rejects.toThrow(
      MEDIA_NONE,
    );
    await expect(
      new WebMediaCapture(deps().d).open({ audio: false, video: false }),
    ).rejects.toThrow(MEDIA_NONE);
    // Without the browser APIs there is no recorder.
    expect(webMediaCapture({})).toBeUndefined();
  });

  it('fake recorder decodes its own narration', async () => {
    let t = 0;
    const f = new FakeMediaCapture(() => t);
    const s = await f.open({ audio: true, video: false });
    const seg = s.segment();
    t += 250;
    const r = await seg.stop();
    expect(r.video).toBeNull();
    const d = await f.decodeAudio(r.audio!.bytes);
    expect(d.channels[0]!.length).toBe(2000);
    expect(f.opened).toEqual([{ audio: true, video: false }]);
  });
});
