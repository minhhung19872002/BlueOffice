import type { CloseReason, CollabStatus } from '@blueoffice/collab';

/**
 * Editing mode of a hosted session. It starts from `SessionInfo.mode` and
 * follows the server: `mode-changed` (edit permission revoked or restored,
 * docs/bohp.md §D), `session-revoked` / close 4403 (no access any more).
 */
export type SessionModeValue = 'edit' | 'comment' | 'view' | 'revoked';

export class SessionMode {
  private readonly listeners = new Set<(m: SessionModeValue) => void>();

  constructor(private current: SessionModeValue) {}

  get value(): SessionModeValue {
    return this.current;
  }

  /** Local edits are allowed (and shared). */
  get editable(): boolean {
    return this.current === 'edit';
  }

  set(next: SessionModeValue): void {
    // Revoked is final for this session.
    if (next === this.current || this.current === 'revoked') return;
    this.current = next;
    this.listeners.forEach((l) => l(next));
  }

  /** Calls `fn` on every change; returns the unsubscribe function. */
  subscribe(fn: (m: SessionModeValue) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
}

/** Maps a server mode string to {@link SessionModeValue} (unknown → view). */
export function parseMode(mode: unknown): SessionModeValue {
  return mode === 'edit' || mode === 'comment' || mode === 'view' || mode === 'revoked'
    ? mode
    : mode === 'none'
      ? 'revoked'
      : 'view';
}

/** Observable connection state for {@link HostedBanner}. */
export class HostedStatus {
  private status: CollabStatus | null = null;
  private ended: CloseReason | null = null;
  private readonly listeners = new Set<() => void>();
  get value(): { status: CollabStatus | null; ended: CloseReason | null } {
    return { status: this.status, ended: this.ended };
  }
  setStatus(s: CollabStatus): void {
    this.status = s;
    this.listeners.forEach((l) => l());
  }
  setEnded(r: CloseReason): void {
    this.ended = r;
    this.listeners.forEach((l) => l());
  }
  subscribe(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
}
