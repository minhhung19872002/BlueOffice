/**
 * Shared machinery of the BlueDoc / BlueSlide bindings: engine work runs one
 * task at a time, and frames from the server are held while a task is in
 * flight. The engine and the replica therefore always see the same order of
 * changes: a local change is applied to the replica relative to the state the
 * engine computed it against, and remote changes reach the engine in the
 * order the replica received them.
 */
import type { CollabEngine } from './index';
import { unpack } from './sheet';

/** The `WasmCollab` surface used by the BlueDoc / BlueSlide bindings. */
export interface ReplicaLike {
  setTime(unixMs: number): void;
  start(): Uint8Array;
  receive(frame: Uint8Array): Uint8Array;
  drainOutbox(): Uint8Array;
  /** Full state as one Yjs v1 update. */
  snapshot?(): Uint8Array;
  setPresence(json: string): void;
  peersJson(): string;
  clientId(): number;
  applyLocal(json: string): void;
  seed(json: string): void;
  /** JSON `Change`, or `""` when nothing is pending. */
  takeRemote(): string;
  fullState(): string;
  undo(): boolean;
  redo(): boolean;
  canUndo(): boolean;
  canRedo(): boolean;
}

/** Who this replica's user is (awareness). */
export interface CollabUser {
  userId: string;
  name: string;
  color: string;
}

/** A change of the shared state (`bo_collab::Change`). */
export interface ChangeLike {
  body?: unknown[][];
  slides?: Record<string, string | null>;
  shapes?: Record<string, string | null>;
  blobs?: Record<string, string | null>;
}

export const isEmptyChange = (c: ChangeLike | null | undefined): boolean =>
  !c ||
  ((c.body ?? []).every((d) => d.length === 0) &&
    !Object.keys(c.slides ?? {}).length &&
    !Object.keys(c.shapes ?? {}).length &&
    !Object.keys(c.blobs ?? {}).length);

/** Distinct, readable caret colours (not tied to any product palette). */
const COLORS = ['#c2410c', '#15803d', '#7c3aed', '#be185d', '#0e7490', '#a16207', '#4d7c0f'];

export const colorFor = (clientId: number): string => COLORS[clientId % COLORS.length] ?? '#c2410c';

/** Serial engine queue + frame gate. */
export class LiveQueue {
  private tail: Promise<unknown> = Promise.resolve();
  private pending = 0;
  private held: Uint8Array[] = [];
  private replies: Uint8Array[] = [];
  /** Give this to `CollabClient` instead of the raw replica. */
  readonly engine: CollabEngine;

  constructor(
    private readonly replica: ReplicaLike,
    private readonly flush: () => void,
    private readonly onFrames: () => void,
  ) {
    this.engine = {
      start: () => {
        replica.setTime(Date.now());
        return replica.start();
      },
      receive: (frame) => {
        if (this.pending > 0) {
          this.held.push(frame);
          return [];
        }
        replica.setTime(Date.now());
        const replies = unpack(replica.receive(frame));
        this.onFrames();
        return replies;
      },
      drainOutbox: () => [...this.replies.splice(0), ...unpack(replica.drainOutbox())],
      ...(replica.snapshot ? { snapshot: () => replica.snapshot!() } : {}),
    };
  }

  get busy(): boolean {
    return this.pending > 0;
  }

  /** Runs `f` after the queued tasks. */
  run<T>(f: () => Promise<T>): Promise<T> {
    this.pending += 1;
    const p = this.tail.then(f);
    this.tail = p
      .then(
        () => {},
        () => {},
      )
      .then(() => {
        this.pending -= 1;
        if (this.pending === 0) this.release();
      });
    return p;
  }

  /** Waits for the queued tasks. */
  idle(): Promise<void> {
    return this.tail.then(() => {});
  }

  /** Delivers the frames held while busy (their replies go out with the next flush). */
  private release(): void {
    if (!this.held.length) return;
    this.replica.setTime(Date.now());
    for (const f of this.held.splice(0)) {
      try {
        this.replies.push(...unpack(this.replica.receive(f)));
      } catch {
        // A malformed frame is dropped; the next sync step repairs the state.
      }
    }
    this.onFrames();
    this.flush();
  }
}

/** Forwards everything to `target` except `overrides` (keeps `this` of class hosts). */
export function overlay<H extends object, O extends object>(target: H, overrides: O): H & O {
  return new Proxy(target, {
    get(t, prop) {
      if (Object.prototype.hasOwnProperty.call(overrides, prop)) {
        return (overrides as Record<PropertyKey, unknown>)[prop];
      }
      const v: unknown = Reflect.get(t, prop, t);
      return typeof v === 'function' ? (v as (...a: unknown[]) => unknown).bind(t) : v;
    },
    has(t, prop) {
      return Object.prototype.hasOwnProperty.call(overrides, prop) || Reflect.has(t, prop);
    },
  }) as H & O;
}
