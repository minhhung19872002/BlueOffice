import { describe, expect, it, vi } from 'vitest';
import type { SocketLike } from '@blueoffice/collab';
import type { Platform } from '@blueoffice/platform';
import { SessionMode, collabSocketUrl, startCollab, type Replica } from '../src';

class FakeSocket implements SocketLike {
  static all: FakeSocket[] = [];
  binaryType = 'blob';
  readyState = 0;
  sent: (string | Uint8Array)[] = [];
  onopen: SocketLike['onopen'] = null;
  onmessage: SocketLike['onmessage'] = null;
  onclose: SocketLike['onclose'] = null;
  onerror: SocketLike['onerror'] = null;
  constructor(public url: string) {
    FakeSocket.all.push(this);
  }
  send(d: string | Uint8Array): void {
    this.sent.push(d);
  }
  close(code = 1000): void {
    this.readyState = 3;
    this.onclose?.({ code, reason: '' });
  }
  open(): void {
    this.readyState = 1;
    this.onopen?.({});
  }
  control(m: object): void {
    this.onmessage?.({ data: JSON.stringify(m) });
  }
}

const replica = (): Replica =>
  ({
    setTime() {},
    start: () => new Uint8Array([0, 0, 1, 0]),
    receive: () => new Uint8Array(),
    drainOutbox: () => new Uint8Array(),
    setCell: vi.fn(),
    cellsJson: () => '{}',
    setPresence() {},
    peersJson: () => '[]',
    clientId: () => 7,
  }) as unknown as Replica;

function sheetPlatform() {
  const info = vi.fn(async () => ({ sheets: [{ name: 'Sheet1', lastRow: 0, lastCol: 0 }] }));
  const sheet = {
    info,
    setInputs: async () => {},
    exec: async () => null,
    cell: async () => ({ input: '=1+1' }),
  };
  return { p: { sheet, engine: {}, auth: { user: null } } as unknown as Platform, info };
}

const session = {
  sessionId: 'S1',
  sessionToken: 'tok',
  mode: 'edit' as const,
  file: { id: 'f2', name: 'Bảng.xlsx' },
  user: { id: 'u2', name: 'Trần Thị Bình' },
  allowedOrigins: [],
  collab: { url: '/collab/f2?session=S1' },
};

describe('startCollab', () => {
  it('resolves the room URL against the server', () => {
    expect(collabSocketUrl('/collab/f2?session=S1', 'https://office.example.vn/edit')).toBe(
      'wss://office.example.vn/collab/f2?session=S1',
    );
    expect(collabSocketUrl('/collab/f2', 'http://localhost:5000')).toBe(
      'ws://localhost:5000/collab/f2',
    );
  });

  it('seeds on welcome, follows mode changes and answers save requests only when editable', async () => {
    FakeSocket.all = [];
    const { p, info } = sheetPlatform();
    const mode = new SessionMode('edit');
    const saveNow = vi.fn();
    const ended: string[] = [];
    const r = replica();
    const h = startCollab({
      platform: p,
      app: 'sheet',
      session,
      base: 'http://localhost:4573',
      replica: r,
      mode,
      saveNow,
      onEnded: (e) => ended.push(e),
      socket: (u) => new FakeSocket(u),
    });
    const ws = FakeSocket.all[0]!;
    expect(ws.url).toBe('ws://localhost:4573/collab/f2?session=S1&token=tok');
    ws.open();
    ws.control({ type: 'welcome', mode: 'edit', seed: true });
    await vi.waitFor(() => expect(r.setCell).toHaveBeenCalledWith('Sheet1', 'A1', '=1+1'));
    expect(info).toHaveBeenCalled();
    ws.control({ type: 'save-request', reason: 'autosave' });
    expect(saveNow).toHaveBeenCalledTimes(1);

    ws.control({ type: 'mode-changed', mode: 'view', reason: 'permissions' });
    expect(mode.value).toBe('view');
    ws.control({ type: 'save-request', reason: 'autosave' });
    expect(saveNow).toHaveBeenCalledTimes(1);

    ws.control({ type: 'mode-changed', mode: 'edit' });
    expect(mode.editable).toBe(true);
    ws.control({ type: 'session-revoked' });
    expect(mode.value).toBe('revoked');
    ws.close(4403);
    expect(ended).toEqual(['revoked']);
    expect(h.platform.sheet).not.toBe(p.sheet);
    h.stop();
  });
});
