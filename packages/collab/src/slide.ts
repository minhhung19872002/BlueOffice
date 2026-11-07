/**
 * BlueSlide ↔ `bo-collab` binding: slides (properties and order) and their
 * objects are shared live through the replica's `slides` / `shapes` maps,
 * last writer wins per object (bo_engine::slide_edit::collab). Presence
 * carries the selected object; collaborators' selections are reported
 * through `onSlidePeers`.
 */
import {
  LiveQueue,
  isEmptyChange,
  overlay,
  type ChangeLike,
  type CollabUser,
  type ReplicaLike,
} from './live';
import type { CollabEngine } from './index';
import type { Presence } from './sheet';

/** Subset of a slide editing result used here (`SlideEditResult`). */
export interface SlideResultLike {
  canUndo: boolean;
  canRedo: boolean;
  collab?: { change: ChangeLike; shape?: string };
}

/** Subset of `EngineHost` used here. */
export interface SlideEngineLike {
  open(bytes: Uint8Array, name: string, password?: string): Promise<unknown>;
  slideCommand(cmd: { cmd: string }): Promise<SlideResultLike>;
  slideInsertPicture(bytes: Uint8Array, name: string): Promise<SlideResultLike>;
  slideCollabStart?(seed: boolean, clientId: number): Promise<ChangeLike>;
  slideCollabApply?(change: string): Promise<SlideResultLike>;
  slideCollabPeers?(keys: string[]): Promise<({ slide: number; rect: unknown } | null)[]>;
}

type BoxOf<H extends SlideEngineLike> = NonNullable<
  Awaited<ReturnType<NonNullable<H['slideCollabPeers']>>>[number]
>;

/** A collaborator's selected object. */
export type SlidePeerView<H extends SlideEngineLike> = BoxOf<H> & {
  clientId: number;
  name: string;
  color: string;
};

export interface SlideCollab<H extends SlideEngineLike> {
  readonly engine: CollabEngine;
  seed(): Promise<void>;
  remoteApplied(): Promise<void>;
  onSlideExternalChange(listener: (r: Awaited<ReturnType<H['slideCommand']>>) => void): () => void;
  onSlidePeers(listener: (peers: SlidePeerView<H>[]) => void): () => void;
  idle(): Promise<void>;
  dispose(): void;
}

/** Wraps an engine host so BlueSlide edits are shared through `replica`. */
export function collabSlideHost<H extends SlideEngineLike>(
  host: H,
  replica: ReplicaLike,
  flush: () => void,
  user: CollabUser,
): H & SlideCollab<H> {
  type R = Awaited<ReturnType<H['slideCommand']>>;
  const results = new Set<(r: R) => void>();
  const peerListeners = new Set<(p: SlidePeerView<H>[]) => void>();
  let phase: 'closed' | 'live' = 'closed';
  let seedAsked = false;
  let lastShape: string | undefined | null = null;
  let disposed = false;

  const pull = (): Promise<void> =>
    queue.run(async () => {
      if (phase === 'live') {
        const ch = replica.takeRemote();
        if (ch) await applyRemote(ch);
      }
      await refreshPeers();
    });
  const queue = new LiveQueue(replica, flush, () => void pull().catch(() => {}));

  const share = <T extends SlideResultLike>(r: T): T => {
    const c = r.collab;
    if (!c) return r;
    if (phase === 'live' && !isEmptyChange(c.change)) replica.applyLocal(JSON.stringify(c.change));
    if (c.shape !== lastShape) {
      lastShape = c.shape;
      const p: Presence = { ...user, ...(c.shape ? { shape: c.shape } : {}) };
      replica.setPresence(JSON.stringify(p));
    }
    if (phase === 'live') {
      r.canUndo = replica.canUndo();
      r.canRedo = replica.canRedo();
    }
    flush();
    return r;
  };

  const applyRemote = async (json: string) => {
    if (!host.slideCollabApply || disposed) return;
    const r = share((await host.slideCollabApply(json)) as R);
    results.forEach((l) => l(r));
  };

  const refreshPeers = async () => {
    if (phase !== 'live' || !host.slideCollabPeers || !peerListeners.size) return;
    const peers = (JSON.parse(replica.peersJson()) as [number, Presence][]).filter(
      ([, p]) => typeof p.shape === 'string',
    );
    const boxes = peers.length ? await host.slideCollabPeers(peers.map(([, p]) => p.shape!)) : [];
    const views = peers.flatMap(([clientId, p], i) => {
      const b = boxes[i] as BoxOf<H> | null | undefined;
      return b ? [{ ...b, clientId, name: p.name, color: p.color } as SlidePeerView<H>] : [];
    });
    peerListeners.forEach((l) => l(views));
  };

  const bind = async () => {
    if (!host.slideCollabStart) return;
    const id = replica.clientId();
    if (seedAsked) {
      replica.seed(JSON.stringify(await host.slideCollabStart(true, id)));
      phase = 'live';
      flush();
    } else {
      await host.slideCollabStart(false, id);
      phase = 'live';
      await applyRemote(replica.fullState());
    }
  };

  const local = (f: () => Promise<unknown>): Promise<R> =>
    queue.run(async () => {
      const r = share((await f()) as R);
      if (replica.peersJson() !== '[]') await refreshPeers();
      return r;
    }) as Promise<R>;

  const overrides: SlideCollab<H> & Partial<SlideEngineLike> = {
    engine: queue.engine,
    open: (bytes: Uint8Array, name: string, password?: string) =>
      queue.run(async () => {
        phase = 'closed';
        const info = await host.open(bytes, name, password);
        await bind();
        return info;
      }),
    slideCommand: (cmd: { cmd: string }) =>
      local(async () => {
        if (
          phase === 'live' &&
          (cmd.cmd === 'undo' || cmd.cmd === 'redo') &&
          host.slideCollabApply
        ) {
          // Collaborative undo: only this user's changes are reverted.
          const done = cmd.cmd === 'undo' ? replica.undo() : replica.redo();
          const ch = done ? replica.takeRemote() : '';
          flush();
          if (ch) return (await host.slideCollabApply(ch)) as R;
          return (await host.slideCommand({ cmd: 'state' })) as R;
        }
        return (await host.slideCommand(cmd)) as R;
      }),
    slideInsertPicture: (bytes: Uint8Array, name: string) =>
      local(async () => (await host.slideInsertPicture(bytes, name)) as R),
    seed: () =>
      queue.run(async () => {
        seedAsked = true;
        if (phase !== 'live' || !host.slideCollabStart) return;
        if (isEmptyChange(JSON.parse(replica.fullState()) as ChangeLike)) {
          replica.seed(JSON.stringify(await host.slideCollabStart(true, replica.clientId())));
          flush();
        }
      }),
    remoteApplied: () => pull(),
    onSlideExternalChange: (l) => {
      results.add(l);
      return () => results.delete(l);
    },
    onSlidePeers: (l) => {
      peerListeners.add(l);
      void queue.run(refreshPeers).catch(() => {});
      return () => peerListeners.delete(l);
    },
    idle: () => queue.idle(),
    dispose: () => {
      disposed = true;
      results.clear();
      peerListeners.clear();
      (host as { dispose?: () => void }).dispose?.();
    },
  };
  return overlay(host, overrides) as unknown as H & SlideCollab<H>;
}
