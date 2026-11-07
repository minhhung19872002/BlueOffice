import { describe, expect, it } from 'vitest';
import {
  connectEditor,
  connectHost,
  editorFrameUrl,
  envelope,
  type IncomingMessage,
  type MessageSource,
  type MessageTarget,
} from '../src';

/** Fake window: delivers posted messages asynchronously to its listeners. */
class FakeWindow implements MessageSource {
  private listeners = new Set<(ev: IncomingMessage) => void>();
  constructor(public readonly origin: string) {}
  addEventListener(_t: 'message', l: (ev: IncomingMessage) => void): void {
    this.listeners.add(l);
  }
  removeEventListener(_t: 'message', l: (ev: IncomingMessage) => void): void {
    this.listeners.delete(l);
  }
  deliver(data: unknown, origin: string): void {
    queueMicrotask(() => this.listeners.forEach((l) => l({ data, origin })));
  }
  /** A handle that other windows use to post to this one. */
  from(sender: FakeWindow, log?: { msg: unknown; target: string }[]): MessageTarget {
    return {
      postMessage: (msg, targetOrigin) => {
        log?.push({ msg, target: targetOrigin });
        if (targetOrigin === this.origin) this.deliver(msg, sender.origin);
      },
    };
  }
}

const HOST = 'https://drive.example.vn';
const EDITOR = 'https://office.example.vn';
const tick = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

function pair(allowed = [HOST]) {
  const hostWin = new FakeWindow(HOST);
  const editorWin = new FakeWindow(EDITOR);
  const toEditorLog: { msg: unknown; target: string }[] = [];
  const handle = connectEditor(editorWin.from(hostWin, toEditorLog), hostWin, {
    fileId: 'f1',
    accessToken: 'secret-token',
    host: 'bluedrive',
    editorUrl: EDITOR,
    timeoutMs: 200,
  });
  const conn = connectHost({
    parent: hostWin.from(editorWin),
    self: editorWin,
    allowedOrigins: allowed,
  });
  return { handle, conn, toEditorLog, hostWin, editorWin };
}

describe('editor-sdk', () => {
  it('builds iframe URL without the access token', () => {
    const url = editorFrameUrl({
      editorUrl: EDITOR,
      fileId: 'a b',
      host: 'bluedrive',
      mode: 'view',
    });
    expect(url).toBe(`${EDITOR}/edit?file=a+b&host=bluedrive&mode=view&embed=1`);
    expect(url).not.toContain('token');
  });

  it('hands over init payload after hello, only to the editor origin', async () => {
    const { conn, toEditorLog } = pair();
    const init = await conn.ready;
    expect(init.accessToken).toBe('secret-token');
    expect(init.mode).toBe('edit');
    expect(init.lang).toBe('vi-VN');
    expect(toEditorLog.every((e) => e.target === EDITOR)).toBe(true);
  });

  it('forwards events and resolves commands', async () => {
    const { handle, conn } = pair();
    await conn.ready;
    const saved: unknown[] = [];
    handle.on('saved', (p) => saved.push(p));
    conn.emit('saved', { version: '3' });
    conn.onCommand((cmd) => (cmd === 'save' ? { ok: true } : undefined));
    await expect(handle.command('save')).resolves.toEqual({ ok: true });
    expect(saved).toEqual([{ version: '3' }]);
  });

  it('propagates command errors', async () => {
    const { handle, conn } = pair();
    await conn.ready;
    conn.onCommand(() => {
      throw new Error('read-only');
    });
    await expect(handle.command('save')).rejects.toThrow('read-only');
  });

  it('ignores messages from non-whitelisted origins', async () => {
    const { handle, conn, hostWin, editorWin } = pair();
    await conn.ready;
    const dirty: unknown[] = [];
    handle.on('dirty', (p) => dirty.push(p));
    hostWin.deliver(envelope('event', 'dirty', { payload: 1 }), 'https://evil.example');
    // Editor ignores init/commands from an attacker origin.
    editorWin.deliver(envelope('command', 'close', { requestId: 'x' }), 'https://evil.example');
    await tick();
    expect(dirty).toEqual([]);
  });

  it('editor never binds when host origin is not whitelisted', async () => {
    const { conn } = pair(['https://other.example']);
    const r = await Promise.race([conn.ready.then(() => 'ready'), tick().then(() => 'pending')]);
    expect(r).toBe('pending');
  });

  it('times out commands and rejects pending on destroy', async () => {
    const { handle, conn } = pair();
    await conn.ready;
    conn.close();
    await expect(handle.command('print')).rejects.toThrow('timed out');
    const p = handle.command('close');
    handle.destroy();
    await expect(p).rejects.toThrow('destroyed');
  });
});
