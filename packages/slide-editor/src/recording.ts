/**
 * Recording a slide show (SLIDE-REC-001): one take per slide — narration and camera from the
 * platform's recorder (`Platform.media`), the time spent on the slide, when its clicks
 * happened, and ink with the time of every point. Pause / resume stop the clock and the
 * recorder together; coming back to a slide records it again (the newer take wins). The
 * takes become one `saveRecording` command. No DOM; unit-tested.
 */
import type {
  MediaCaptureSession,
  RecordedSegment,
  SegmentRecorder,
  SlideCameraCorner,
  SlideInkStroke,
  SlideRecordingInput,
} from '@blueoffice/platform';
import { toBase64 } from './audience';

export type RecordState = 'idle' | 'recording' | 'paused' | 'finished';

/** What was recorded on one slide. */
export interface SlideTake {
  slide: number;
  durationMs: number;
  /** Times of the clicks that played the slide's effects (ms from the slide's start). */
  clicks: number[];
  segment: RecordedSegment | null;
}

/** An ink stroke with the time of each point (ms from the start of its slide). */
export type TimedStroke = SlideInkStroke & { times?: number[] };

export class ShowRecorder {
  private st: RecordState = 'idle';
  private slide: number | null = null;
  private seg: SegmentRecorder | null = null;
  /** Clock of the current slide: running since `since` (null: paused) plus `acc`. */
  private since: number | null = null;
  private acc = 0;
  private clicks: number[] = [];
  private readonly takes = new Map<number, Promise<SlideTake>>();
  /** Order of the takes (a slide recorded again moves to its new place). */
  private readonly order: number[] = [];

  constructor(
    /** Open microphone / camera (null: timings and ink only). */
    private readonly session: MediaCaptureSession | null,
    private readonly now: () => number = () => Date.now(),
  ) {}

  get state(): RecordState {
    return this.st;
  }

  get current(): number | null {
    return this.slide;
  }

  /** Time on the current slide, pauses left out (ms). */
  elapsed(at = this.now()): number {
    return Math.max(0, Math.round(this.acc + (this.since === null ? 0 : at - this.since)));
  }

  /** Slides recorded so far (latest takes), in recording order. */
  get recorded(): number[] {
    return [...this.order];
  }

  /** Start on `slide`. */
  start(slide: number): void {
    if (this.st !== 'idle') return;
    this.st = 'recording';
    this.open(slide);
  }

  private open(slide: number) {
    this.slide = slide;
    this.acc = 0;
    this.clicks = [];
    this.since = this.st === 'paused' ? null : this.now();
    this.seg = this.session?.segment() ?? null;
    if (this.st === 'paused') this.seg?.pause();
  }

  private close() {
    const slide = this.slide;
    if (slide === null) return;
    const durationMs = this.elapsed();
    const clicks = this.clicks;
    const seg = this.seg;
    this.seg = null;
    this.slide = null;
    this.since = null;
    const take = (seg ? seg.stop() : Promise.resolve(null)).then(
      (segment): SlideTake => ({ slide, durationMs, clicks, segment }),
      (): SlideTake => ({ slide, durationMs, clicks, segment: null }),
    );
    this.takes.set(slide, take);
    const i = this.order.indexOf(slide);
    if (i >= 0) this.order.splice(i, 1);
    this.order.push(slide);
  }

  /** The show moved to `slide` (the end screen: `null`). True when a new take started. */
  enter(slide: number | null): boolean {
    if (this.st !== 'recording' && this.st !== 'paused') return false;
    if (slide === this.slide) return false;
    this.close();
    if (slide === null) return false;
    this.open(slide);
    return true;
  }

  /** A click played the next effect of the current slide. */
  click(): void {
    if (this.slide === null || this.st !== 'recording') return;
    this.clicks.push(this.elapsed());
  }

  /** Time stamp for an ink point drawn now (ms from the slide's start). */
  inkTime(): number {
    return this.elapsed();
  }

  pause(): void {
    if (this.st !== 'recording') return;
    this.acc = this.elapsed();
    this.since = null;
    this.seg?.pause();
    this.st = 'paused';
  }

  resume(): void {
    if (this.st !== 'paused') return;
    this.since = this.now();
    this.seg?.resume();
    this.st = 'recording';
  }

  /** Record the current slide again from its start (its take so far is dropped). */
  restartSlide(): void {
    const slide = this.slide;
    if (slide === null) return;
    this.seg?.cancel();
    this.seg = null;
    this.open(slide);
  }

  /** End the recording: every take (latest per slide), in slide order. */
  async finish(): Promise<SlideTake[]> {
    if (this.st === 'idle' || this.st === 'finished') return [];
    this.close();
    this.st = 'finished';
    const takes = await Promise.all(this.order.map((s) => this.takes.get(s)!));
    this.session?.close();
    return takes.sort((a, b) => a.slide - b.slide);
  }

  /** Drop the recording. */
  cancel(): void {
    this.seg?.cancel();
    this.seg = null;
    this.session?.close();
    this.st = 'finished';
  }
}

/**
 * The `saveRecording` slides of `takes`: duration → timing, clicks, timed ink of the slide
 * (strokes without times are left out), narration and camera in base64.
 */
export function recordingInput(
  takes: SlideTake[],
  ink: (slide: number) => TimedStroke[],
  opts: { narration: boolean; camera: boolean; corner: SlideCameraCorner; aspect: number | null },
): SlideRecordingInput[] {
  return takes.map((t) => {
    const out: SlideRecordingInput = { slide: t.slide, durationMs: t.durationMs };
    if (t.clicks.length) out.clicks = [...t.clicks];
    const strokes = ink(t.slide).filter(
      (s) => s.times && s.times.length === s.points.length && s.points.length > 0,
    );
    if (strokes.length) {
      out.ink = strokes.map((s) => ({
        points: s.points.map((p) => [p[0], p[1]] as [number, number]),
        times: [...(s.times ?? [])],
        color: s.color,
        width: s.width,
        ...(s.highlighter ? { highlighter: true } : {}),
      }));
    }
    const a = t.segment?.audio;
    if (opts.narration && a && a.bytes.length) {
      out.narration = { data: toBase64(a.bytes), mime: a.mime };
    }
    const v = t.segment?.video;
    if (opts.camera && v && v.bytes.length) {
      out.camera = {
        data: toBase64(v.bytes),
        mime: v.mime,
        corner: opts.corner,
        ...(opts.aspect ? { aspect: opts.aspect } : {}),
      };
    }
    return out;
  });
}
