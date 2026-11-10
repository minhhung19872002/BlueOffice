/**
 * The copy of an editor that is still on its way to the system clipboard.
 *
 * Copying runs through the engine (to build the clip) and then writes the system
 * clipboard, both asynchronously. A paste right after a copy (Ctrl+C, Ctrl+V) can
 * therefore see the clipboard as it was before the copy. Controllers track every
 * copy here and pastes wait for it: a paste that reads the clipboard itself reads
 * it after the copy landed, and a paste that already carries the clipboard's
 * content (a `paste` event) knows that content is older than the copy.
 */
export class CopyInFlight<T> {
  private pending: Promise<T | null> | null = null;

  /** Track a copy; resolves with its result (null when it failed or copied nothing). */
  track(copy: Promise<T | null>): Promise<T | null> {
    const tracked: Promise<T | null> = copy
      .catch(() => null)
      .finally(() => {
        if (this.pending === tracked) this.pending = null;
      });
    this.pending = tracked;
    // Settles like `copy`, once it is no longer the copy in flight.
    return tracked.then(() => copy);
  }

  /** Is a copy still running? */
  get busy(): boolean {
    return this.pending !== null;
  }

  /**
   * Wait for the copy in flight, if any. `waited` is true when one was running:
   * clipboard content read before this call is then older than that copy, whose
   * result (null when it failed or copied nothing) is `result`.
   */
  async settle(): Promise<{ waited: boolean; result: T | null }> {
    if (!this.pending) return { waited: false, result: null };
    let result: T | null = null;
    // A copy started while waiting is newer still: wait for the latest one.
    while (this.pending) result = await this.pending;
    return { waited: true, result };
  }
}
