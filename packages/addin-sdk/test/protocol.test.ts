/** Wire protocol (CORE-ADDIN-001): untrusted messages are checked, the client correlates answers and events. */
import { describe, expect, it } from 'vitest';
import {
  ADDIN_TAG,
  AddinError,
  ERROR_CODES,
  METHOD_PERMISSIONS,
  connect,
  errorResponse,
  isWriteMethod,
  missingPermissions,
  networkOrigins,
  notification,
  parseHostMessage,
  parseRequest,
  plainJson,
  request,
  response,
  type MessageEventLike,
  type MessageSourceLike,
  type PostTarget,
} from '../src';

describe('parseRequest', () => {
  it('ignores messages of other protocols', () => {
    expect(parseRequest(null)).toBeNull();
    expect(parseRequest('hi')).toBeNull();
    expect(parseRequest({ jsonrpc: '2.0', id: 1, method: 'doc.text' })).toBeNull();
  });

  it('accepts a well-formed request and copies its params as plain JSON', () => {
    const r = parseRequest({ ...request(3, 'doc.insertText', ['x']), extra: 1 });
    expect(r).toEqual({ ok: true, request: request(3, 'doc.insertText', ['x']) });
    const d = parseRequest(request(4, 'sheet.setValues', [-1, 'A1', [[new Date(0)]]]));
    expect(d?.ok && d.request.params[2]).toEqual([['1970-01-01T00:00:00.000Z']]);
  });

  it.each([
    [{ ...request(1, 'doc.text', []), v: 2 }, ERROR_CODES.unsupportedVersion],
    [{ ...request(1, 'doc.text', []), jsonrpc: '1.0' }, ERROR_CODES.invalidRequest],
    [{ ...request(1, 'doc.text', []), id: {} }, ERROR_CODES.invalidRequest],
    [{ ...request(1, 'doc.text', []), id: 'x'.repeat(65) }, ERROR_CODES.invalidRequest],
    [request(1, '__proto__.x', []), ERROR_CODES.invalidRequest],
    [request(1, 'doc', []), ERROR_CODES.invalidRequest],
    [{ ...request(1, 'doc.text', []), params: { a: 1 } }, ERROR_CODES.invalidParams],
    [request(1, 'doc.text', new Array(17).fill(0)), ERROR_CODES.invalidParams],
  ])('rejects %j with %i', (msg, code) => {
    const r = parseRequest(msg);
    expect(r && !r.ok && r.error.code).toBe(code);
  });

  it('rejects params that are not JSON (cycles, BigInt) or too large', () => {
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    for (const p of [[cyclic], [10n], ['x'.repeat(2_000_001)]]) {
      const r = parseRequest(request(1, 'doc.insertText', p));
      expect(r && !r.ok && r.error.code).toBe(ERROR_CODES.invalidParams);
    }
    expect(plainJson(() => 1)).toBeUndefined();
  });
});

describe('parseHostMessage', () => {
  it('reads responses, errors and events; drops the rest', () => {
    expect(parseHostMessage(response(1, { a: 1 }))).toEqual({
      kind: 'response',
      response: response(1, { a: 1 }),
    });
    expect(
      parseHostMessage(
        errorResponse(2, {
          code: -32001,
          message: 'no',
          data: { missing: ['storage', 3 as never] },
        }),
      ),
    ).toEqual({
      kind: 'response',
      response: errorResponse(2, { code: -32001, message: 'no', data: { missing: ['storage'] } }),
    });
    expect(parseHostMessage(notification({ type: 'command', command: 'go' }))).toEqual({
      kind: 'event',
      event: { type: 'command', command: 'go' },
    });
    expect(parseHostMessage({ ...notification({ type: 'selectionChanged' }), v: 9 })).toBeNull();
    expect(
      parseHostMessage({
        tag: ADDIN_TAG,
        v: 1,
        jsonrpc: '2.0',
        method: 'event',
        params: [{ type: 'x' }],
      }),
    ).toBeNull();
    expect(parseHostMessage({ tag: ADDIN_TAG, v: 1, jsonrpc: '2.0', id: 1 })).toBeNull();
  });
});

describe('permissions table', () => {
  it('classifies methods', () => {
    expect(missingPermissions('doc.text', ['read-document'])).toEqual([]);
    expect(missingPermissions('doc.insertText', ['read-document'])).toEqual(['write-document']);
    expect(missingPermissions('doc.command', [])).toEqual(['read-document', 'write-document']);
    expect(missingPermissions('doc.evil', ['read-document'])).toBeNull();
    expect(missingPermissions('toString', [])).toBeNull();
    expect(missingPermissions('addin.hello', [])).toEqual([]);
    expect(isWriteMethod('sheet.setValues')).toBe(true);
    expect(isWriteMethod('sheet.getValues')).toBe(false);
    expect(networkOrigins(['storage', 'network:https://a.example'])).toEqual(['https://a.example']);
    for (const [m, perms] of Object.entries(METHOD_PERMISSIONS)) {
      expect(m).toMatch(/^[a-z]+\.[a-zA-Z]+$/);
      expect(perms.length).toBeLessThanOrEqual(2);
    }
  });
});

/** Two fake windows: the editor (host) and the add-in frame. */
function wires() {
  const toHost: unknown[] = [];
  const listeners = new Set<(e: MessageEventLike) => void>();
  const editor: PostTarget = { postMessage: (m) => void toHost.push(m) };
  const frame: MessageSourceLike = {
    addEventListener: (_t, l) => void listeners.add(l),
    removeEventListener: (_t, l) => void listeners.delete(l),
  };
  const deliver = (data: unknown, source: unknown = editor) =>
    listeners.forEach((l) => l({ data, source }));
  return { toHost, editor, frame, deliver, listeners };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe('client', () => {
  it('connects with addin.hello, correlates answers by id and only trusts the editor window', async () => {
    const w = wires();
    const p = connect({ target: w.editor, source: w.frame, timeoutMs: 1000 });
    await tick();
    expect(w.toHost).toEqual([request(1, 'addin.hello', [{ protocols: [1] }])]);
    // A message from elsewhere (another frame) with the right id is ignored.
    w.deliver(response(1, { app: 'evil' }), {});
    w.deliver(response(1, { protocol: 1, app: 'doc', permissions: ['selection'], methods: [] }));
    const client = await p;
    expect(client.app).toBe('doc');
    expect(client.has('selection')).toBe(true);
    expect(client.has('storage')).toBe(false);

    const a = client.doc.insertText('a');
    const b = client.workbook.getValues(-1, 'A1');
    await tick();
    expect(w.toHost.slice(1)).toEqual([
      request(2, 'doc.insertText', ['a']),
      request(3, 'sheet.getValues', [-1, 'A1']),
    ]);
    w.deliver(response(3, [[1]]));
    w.deliver(
      errorResponse(2, {
        code: ERROR_CODES.permissionDenied,
        message: 'denied',
        data: { missing: ['write-document'] },
      }),
    );
    await expect(b).resolves.toEqual([[1]]);
    const err = await a.catch((e: unknown) => e);
    expect(err).toBeInstanceOf(AddinError);
    expect((err as AddinError).code).toBe(ERROR_CODES.permissionDenied);
    expect((err as AddinError).missing).toEqual(['write-document']);

    const seen: string[] = [];
    const off = client.on('command', (e) => seen.push(e.type === 'command' ? e.command : ''));
    w.deliver(notification({ type: 'command', command: 'greet' }));
    off();
    w.deliver(notification({ type: 'command', command: 'again' }));
    expect(seen).toEqual(['greet']);

    client.close();
    expect(w.listeners.size).toBe(0);
    await expect(client.call('doc.text')).rejects.toBeInstanceOf(AddinError);
  });

  it('refuses non-JSON parameters locally and times out without an answer', async () => {
    const w = wires();
    const p = connect({ target: w.editor, source: w.frame, timeoutMs: 30 });
    await tick();
    w.deliver(response(1, { protocol: 1, app: 'sheet', permissions: [], methods: [] }));
    const client = await p;
    await expect(client.call('doc.insertText', () => 1)).rejects.toMatchObject({
      code: ERROR_CODES.invalidParams,
    });
    await expect(client.doc.text()).rejects.toMatchObject({ code: ERROR_CODES.internal });
  });

  it('fails to connect when the editor refuses the handshake', async () => {
    const w = wires();
    const p = connect({ target: w.editor, source: w.frame, timeoutMs: 1000 });
    await tick();
    w.deliver(errorResponse(1, { code: ERROR_CODES.unsupportedVersion, message: 'old' }));
    await expect(p).rejects.toMatchObject({ code: ERROR_CODES.unsupportedVersion });
    expect(w.listeners.size).toBe(0);
  });
});
