/**
 * BlueDoc ↔ `bo-collab` binding: the document body is shared live through
 * the replica's `body` Y.Text (bo_engine::collab maps model ↔ text). Local
 * commands share the change the engine reports; remote changes and
 * collaborative undo/redo come back from the replica and are applied to the
 * engine outside the local history. Presence carries the selection as UTF-16
 * offsets; collaborators' carets are reported through `onPeers`.
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

/** Subset of an editing result used here (`EditResult` in @blueoffice/platform). */
export interface DocResultLike {
  format: { canUndo: boolean; canRedo: boolean };
  collab?: { change: ChangeLike; selection: [number, number] };
}

/** Subset of `EngineHost` used here. */
export interface DocEngineLike {
  open(bytes: Uint8Array, name: string, password?: string): Promise<unknown>;
  newDocument(name: string): Promise<unknown>;
  command(cmd: { cmd: string }): Promise<DocResultLike>;
  insertImage(bytes: Uint8Array, name: string): Promise<DocResultLike>;
  collabStart?(seed: boolean): Promise<ChangeLike>;
  collabApply?(change: string): Promise<DocResultLike>;
  collabPeers?(selections: [number, number][]): Promise<{ caret: unknown; rects: unknown[] }[]>;
}

type PeersOf<H extends DocEngineLike> = Awaited<ReturnType<NonNullable<H['collabPeers']>>>;

/** A collaborator with the geometry of their selection. */
export type DocPeer<H extends DocEngineLike> = PeersOf<H>[number] & {
  clientId: number;
  name: string;
  color: string;
};

export interface DocCollab<H extends DocEngineLike> {
  /** Pass to `CollabClient` (holds frames while the engine is busy). */
  readonly engine: CollabEngine;
  /** Server `seed`: this editor publishes the file's body. */
  seed(): Promise<void>;
  /** Apply what the replica received (also triggered by the frame gate). */
  remoteApplied(): Promise<void>;
  onExternalChange(listener: (r: Awaited<ReturnType<H['command']>>) => void): () => void;
  onPeers(listener: (peers: DocPeer<H>[]) => void): () => void;
  /** Wait for queued engine work. */
  idle(): Promise<void>;
  /** Stops the binding (and disposes the wrapped host). */
  dispose(): void;
}

/**
 * Wraps an engine host so BlueDoc edits are shared through `replica`. Call
 * `flush` to send the replica's outbox (the client's `flush`).
 */
export function collabDocHost<H extends DocEngineLike>(
  host: H,
  replica: ReplicaLike,
  flush: () => void,
  user: CollabUser,
): H & DocCollab<H> {
  type R = Awaited<ReturnType<H['command']>>;
  const results = new Set<(r: R) => void>();
  const peerListeners = new Set<(p: DocPeer<H>[]) => void>();
  let phase: 'closed' | 'live' = 'closed';
  let seedAsked = false;
  let lastSel = '';
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

  /** Shares a result's change and presence. */
  const share = <T extends DocResultLike>(r: T): T => {
    const c = r.collab;
    if (!c) return r;
    if (phase === 'live' && !isEmptyChange(c.change)) replica.applyLocal(JSON.stringify(c.change));
    const sel = c.selection.join(',');
    if (sel !== lastSel) {
      lastSel = sel;
      const p: Presence = { ...user, selection: c.selection };
      replica.setPresence(JSON.stringify(p));
    }
    if (phase === 'live') {
      r.format.canUndo = replica.canUndo();
      r.format.canRedo = replica.canRedo();
    }
    flush();
    return r;
  };

  const applyRemote = async (json: string) => {
    if (!host.collabApply || disposed) return;
    const r = share((await host.collabApply(json)) as R);
    results.forEach((l) => l(r));
  };

  const refreshPeers = async () => {
    if (phase !== 'live' || !host.collabPeers || !peerListeners.size) return;
    const peers = (JSON.parse(replica.peersJson()) as [number, Presence][]).filter(([, p]) =>
      Array.isArray(p.selection),
    );
    const geo = peers.length
      ? ((await host.collabPeers(peers.map(([, p]) => p.selection!))) as PeersOf<H>)
      : ([] as unknown as PeersOf<H>);
    const views = peers.flatMap(([clientId, p], i) => {
      const g = geo[i];
      return g ? [{ ...g, clientId, name: p.name, color: p.color } as DocPeer<H>] : [];
    });
    peerListeners.forEach((l) => l(views));
  };

  const bind = async () => {
    if (!host.collabStart) return;
    if (seedAsked) {
      replica.seed(JSON.stringify(await host.collabStart(true)));
      phase = 'live';
      flush();
    } else {
      await host.collabStart(false);
      phase = 'live';
      // Late joiner: the body becomes the shared text (stays local until it arrives).
      await applyRemote(replica.fullState());
    }
  };

  const local = (f: () => Promise<unknown>): Promise<R> =>
    queue.run(async () => {
      const r = share((await f()) as R);
      if (replica.peersJson() !== '[]') await refreshPeers();
      return r;
    }) as Promise<R>;

  const overrides: DocCollab<H> & Partial<DocEngineLike> = {
    engine: queue.engine,
    open: (bytes: Uint8Array, name: string, password?: string) =>
      queue.run(async () => {
        phase = 'closed';
        const info = await host.open(bytes, name, password);
        await bind();
        return info;
      }),
    newDocument: (name: string) =>
      queue.run(async () => {
        // A new local document leaves the shared session.
        phase = 'closed';
        return host.newDocument(name);
      }),
    command: (cmd: { cmd: string }) =>
      local(async () => {
        if (phase === 'live' && (cmd.cmd === 'undo' || cmd.cmd === 'redo') && host.collabApply) {
          // Collaborative undo: only this user's changes are reverted.
          const done = cmd.cmd === 'undo' ? replica.undo() : replica.redo();
          const ch = done ? replica.takeRemote() : '';
          flush();
          if (ch) return (await host.collabApply(ch)) as R;
          return (await host.command({ cmd: 'state' })) as R;
        }
        return (await host.command(cmd)) as R;
      }),
    insertImage: (bytes: Uint8Array, name: string) =>
      local(async () => (await host.insertImage(bytes, name)) as R),
    seed: () =>
      queue.run(async () => {
        seedAsked = true;
        if (phase !== 'live' || !host.collabStart) return;
        // Bound as a joiner but the room is empty: publish this body.
        const full = JSON.parse(replica.fullState()) as ChangeLike;
        if (isEmptyChange(full)) {
          replica.seed(JSON.stringify(await host.collabStart(true)));
          flush();
        }
      }),
    remoteApplied: () => pull(),
    onExternalChange: (l) => {
      results.add(l);
      return () => results.delete(l);
    },
    onPeers: (l) => {
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
  return overlay(host, overrides) as unknown as H & DocCollab<H>;
}
