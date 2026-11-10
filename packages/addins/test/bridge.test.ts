/**
 * Add-in bridge (CORE-ADDIN-001) with a fake iframe: the SDK client on one side,
 * the BlueScript hosts on the real WASM engines on the other. Every call is
 * checked against the granted permissions; writes are one undo step each.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, WasmEngine, WasmSheet } from '@blueoffice/engine-wasm';
import {
  AddinError,
  ERROR_CODES,
  METHOD_PERMISSIONS,
  connect,
  request,
  type AddinClient,
  type AddinManifest,
  type MessageEventLike,
} from '@blueoffice/addin-sdk';
import type { EditCommand, EditResult, SheetCommand } from '@blueoffice/platform';
import { API_METHODS, docScriptHost, sheetScriptHost, type ScriptHost } from '@blueoffice/script';
import { AddinBridge, type RateLimit } from '../src/bridge';
import { docSelection, sheetSelection } from '../src/selection';
import { addinStorage, memoryBacking, STORAGE_PREFIX } from '../src/storage';

const root = join(__dirname, '..', '..', '..');
const parse = <T>(s: string) => JSON.parse(s) as T;

beforeAll(() => {
  initSync({
    module: readFileSync(join(root, 'packages', 'engine-wasm', 'pkg', 'bo_wasm_bg.wasm')),
  });
});

function docEngine() {
  const e = new WasmEngine();
  const fontsDir = join(root, 'assets', 'fonts');
  const manifest = parse<{ fonts: string[] }>(
    readFileSync(join(fontsDir, 'manifest.json'), 'utf8'),
  );
  for (const f of manifest.fonts.slice(0, 4)) e.addFont(readFileSync(join(fontsDir, f)));
  e.newDocument('addin.docx');
  const run = async (cmd: EditCommand) => parse<EditResult>(e.command(JSON.stringify(cmd)));
  return { e, run, host: docScriptHost({ run }) };
}

function sheetEngine() {
  const e = new WasmSheet();
  e.setLocale('en-US');
  e.newWorkbook('addin.xlsx');
  const target = {
    info: async () => parse<never>(e.info()),
    cell: async (s: number, r: number, c: number) => parse<never>(e.cell(s, r, c)),
    exec: async <T>(cmd: SheetCommand) => parse<T>(e.exec(JSON.stringify(cmd))),
    addSheet: async (n?: string) => e.addSheet(n),
    renameSheet: async (s: number, n: string) => e.renameSheet(s, n),
  };
  return { e, target, host: sheetScriptHost({ host: target, activeSheet: () => 0, decimal: '.' }) };
}

const manifest = (
  permissions: string[],
  apps: AddinManifest['apps'] = ['doc', 'sheet'],
): AddinManifest => ({
  manifestVersion: 1,
  id: 'vn.example.test',
  name: { vi: 'Thử', en: 'Test' },
  version: '1.2.3',
  publisher: 'Example',
  startUrl: 'https://addins.example.vn/test/',
  allowedOrigins: ['https://addins.example.vn'],
  permissions: permissions as AddinManifest['permissions'],
  apps,
});

/**
 * A fake iframe: the bridge posts into the frame's listeners; the client posts
 * into the editor window, delivered as a message whose `source` is the frame.
 */
function fakeFrame() {
  const frameListeners = new Set<(e: MessageEventLike) => void>();
  let bridge: AddinBridge | null = null;
  const sentToFrame: unknown[] = [];
  const frameWindow = {
    postMessage: (data: unknown, targetOrigin: string) => {
      expect(targetOrigin).toBe('*');
      sentToFrame.push(data);
      queueMicrotask(() => frameListeners.forEach((l) => l({ data, source: editorWindow })));
    },
  };
  const editorWindow = {
    postMessage: (data: unknown) =>
      queueMicrotask(() => bridge?.handle({ data, source: frameWindow })),
  };
  const frameSource = {
    addEventListener: (_t: 'message', l: (e: MessageEventLike) => void) =>
      void frameListeners.add(l),
    removeEventListener: (_t: 'message', l: (e: MessageEventLike) => void) =>
      void frameListeners.delete(l),
  };
  return {
    frameWindow,
    editorWindow,
    sentToFrame,
    attach: (b: AddinBridge) => (bridge = b),
    connect: () => connect({ target: editorWindow, source: frameSource, timeoutMs: 5000 }),
  };
}

interface Setup {
  permissions: string[];
  granted?: string[];
  app?: 'doc' | 'sheet';
  host: () => ScriptHost | null;
  selection?: () => Promise<unknown>;
  rateLimit?: RateLimit;
  now?: () => number;
  backing?: ReturnType<typeof memoryBacking>;
}

async function setup(s: Setup) {
  const f = fakeFrame();
  const backing = s.backing ?? memoryBacking();
  const calls: [string, number][] = [];
  const m = manifest(s.permissions);
  const bridge = new AddinBridge({
    manifest: m,
    app: s.app ?? 'doc',
    granted: s.granted ?? s.permissions,
    frame: f.frameWindow,
    language: () => 'vi-VN',
    host: s.host,
    ...(s.selection ? { selection: s.selection } : {}),
    storage: addinStorage(m.id, backing),
    ...(s.rateLimit ? { rateLimit: s.rateLimit } : {}),
    ...(s.now ? { now: s.now } : {}),
    onCall: (method, code) => calls.push([method, code]),
  });
  f.attach(bridge);
  const client = await f.connect();
  return { f, bridge, client, calls, backing };
}

const code = (p: Promise<unknown>) =>
  p.then(
    () => 0,
    (e: unknown) => (e instanceof AddinError ? e.code : -1),
  );

describe('AddinBridge', () => {
  it('hello reports app, granted permissions and the methods allowed', async () => {
    const d = docEngine();
    const { client } = await setup({
      permissions: ['read-document', 'storage'],
      granted: ['read-document', 'storage', 'write-document'],
      host: () => d.host,
    });
    expect(client.info).toMatchObject({
      protocol: 1,
      app: 'doc',
      addinId: 'vn.example.test',
      version: '1.2.3',
      // A grant outside the manifest is not in effect.
      permissions: ['read-document', 'storage'],
      language: 'vi-VN',
      hasDocument: true,
    });
    expect(client.info.methods).toContain('doc.text');
    expect(client.info.methods).toContain('storage.get');
    expect(client.info.methods).not.toContain('doc.insertText');
    expect(client.info.methods).not.toContain('sheet.getValues');
  });

  it('reads the selection and inserts text in BlueDoc, one undo step per call', async () => {
    const d = docEngine();
    await d.run({ cmd: 'insertText', text: 'Xin chào thế giới' });
    // Select "chào".
    const { client } = await setup({
      permissions: ['selection', 'write-document'],
      host: () => d.host,
      selection: () => docSelection(d.host),
    });
    await client.doc.select(0, 4, 8);
    const sel = await client.selection.get();
    expect(sel).toEqual({
      app: 'doc',
      anchor: { paragraph: 0, offset: 4 },
      focus: { paragraph: 0, offset: 8 },
      text: 'chào',
    });
    await client.doc.insertText('bạn');
    await client.doc.insertText('!');
    expect(
      parse<EditResult>(d.e.command(JSON.stringify({ cmd: 'speechText' }))).speech?.paras?.[0]
        ?.text,
    ).toBe('Xin bạn! thế giới');
    // A call made of several engine steps (split + type) is one undo step.
    const paras = () =>
      (
        parse<EditResult>(d.e.command(JSON.stringify({ cmd: 'speechText' }))).speech?.paras ?? []
      ).map((p) => p.text);
    expect(await client.doc.appendParagraph('Dòng hai')).toBe(1);
    expect(paras()).toEqual(['Xin bạn! thế giới', 'Dòng hai']);
    await d.run({ cmd: 'undo' });
    expect(paras()).toEqual(['Xin bạn! thế giới']);
    // Reading the whole document needs read-document.
    await expect(client.doc.text()).rejects.toMatchObject({
      code: ERROR_CODES.permissionDenied,
      missing: ['read-document'],
    });
  });

  it('writes and reads cells in BlueSheet; the selection includes its values', async () => {
    const s = sheetEngine();
    const { client } = await setup({
      app: 'sheet',
      permissions: ['selection', 'write-document', 'read-document'],
      host: () => s.host,
      selection: () =>
        sheetSelection(s.host, {
          sheet: 0,
          sheetName: 'Sheet1',
          range: { r0: 1, c0: 1, r1: 1, c1: 2 },
          active: { row: 1, col: 1 },
        }),
    });
    await client.workbook.setValues(-1, 'B2:C2', [['Tổng', 42]]);
    expect(await client.workbook.getValues(0, 'B2:C2')).toEqual([['Tổng', 42]]);
    expect(await client.selection.get()).toEqual({
      app: 'sheet',
      sheet: 0,
      sheetName: 'Sheet1',
      address: 'B2:C2',
      active: 'B2',
      values: [['Tổng', 42]],
    });
    // Methods of another application do not exist here.
    expect(await code(client.doc.insertText('x'))).toBe(ERROR_CODES.methodNotFound);
  });

  it('checks every method of the API against its permissions', async () => {
    const d = docEngine();
    const s = sheetEngine();
    for (const app of ['doc', 'sheet'] as const) {
      // Granted nothing: every document method is refused with the permissions it lacks.
      const { client, calls } = await setup({
        app,
        permissions: [],
        host: () => (app === 'doc' ? d.host : s.host),
        rateLimit: { perSecond: 1000, burst: 1000 },
      });
      for (const [method, perms] of Object.entries(METHOD_PERMISSIONS)) {
        if (method === 'addin.hello') continue;
        const ns = method.split('.')[0];
        const err = (await client.call(method).then(
          () => null,
          (e: unknown) => e,
        )) as AddinError;
        if (ns === 'doc' || ns === 'sheet' || ns === 'slide') {
          if (ns !== app) {
            expect(err.code, method).toBe(ERROR_CODES.methodNotFound);
            continue;
          }
        }
        expect(err.code, method).toBe(ERROR_CODES.permissionDenied);
        expect(err.missing, method).toEqual(perms);
      }
      // Unknown methods (also prototype names) are refused.
      for (const m of ['doc.evil', 'storage.clear', 'addin.constructor']) {
        expect(await code(client.call(m))).toBe(ERROR_CODES.methodNotFound);
      }
      // Only the handshake succeeded.
      expect(calls[0]).toEqual(['addin.hello', 0]);
      expect(calls.slice(1).every(([, c]) => c !== 0)).toBe(true);
    }
  });

  it('classifies exactly the BlueScript API methods (no second API)', () => {
    const script = Object.values(API_METHODS).flat() as string[];
    const table = Object.keys(METHOD_PERMISSIONS).filter((m) => /^(doc|sheet|slide)\./.test(m));
    expect(new Set(table)).toEqual(new Set(script));
  });

  it('refuses calls before the handshake and malformed requests', async () => {
    const d = docEngine();
    const f = fakeFrame();
    const bridge = new AddinBridge({
      manifest: manifest(['read-document']),
      app: 'doc',
      granted: ['read-document'],
      frame: f.frameWindow,
      language: () => 'en-US',
      host: () => d.host,
      storage: addinStorage('x.y', memoryBacking()),
    });
    bridge.handle({ data: request(1, 'doc.text', []), source: f.frameWindow });
    bridge.handle({
      data: { tag: 'bo-addin', v: 1, jsonrpc: '2.0', id: 2, method: 'x' },
      source: f.frameWindow,
    });
    // Another window's messages are ignored entirely.
    bridge.handle({ data: request(3, 'addin.hello', []), source: {} });
    await bridge.idle();
    await new Promise((r) => setTimeout(r, 0));
    expect(f.sentToFrame).toEqual([
      expect.objectContaining({
        id: 2,
        error: expect.objectContaining({ code: ERROR_CODES.invalidRequest }),
      }),
      expect.objectContaining({
        id: 1,
        error: expect.objectContaining({ code: ERROR_CODES.notReady }),
      }),
    ]);
  });

  it('rate-limits calls', async () => {
    const d = docEngine();
    let now = 0;
    const { client } = await setup({
      permissions: ['read-document'],
      host: () => d.host,
      rateLimit: { perSecond: 2, burst: 3 },
      now: () => now,
    });
    // The handshake used one token; two remain.
    const codes = await Promise.all([1, 2, 3, 4].map(() => code(client.doc.info())));
    expect(codes).toEqual([0, 0, ERROR_CODES.rateLimited, ERROR_CODES.rateLimited]);
    now += 1000;
    expect(await code(client.doc.info())).toBe(0);
  });

  it('gives each add-in its own storage; uninstall clears it', async () => {
    const backing = memoryBacking();
    const other = addinStorage('vn.example.other', backing);
    other.set('secret', 'theirs');
    const { client } = await setup({ permissions: ['storage'], host: () => null, backing });
    await client.storage.set('count', 3);
    await client.storage.set('obj', { a: [1, 2] });
    expect(await client.storage.get('count')).toBe(3);
    expect(await client.storage.get('secret')).toBeNull();
    expect(await client.storage.keys()).toEqual(['count', 'obj']);
    await client.storage.remove('count');
    expect(await client.storage.keys()).toEqual(['obj']);
    expect(await code(client.storage.set('', 1))).toBe(ERROR_CODES.invalidParams);
    expect(await code(client.storage.set('__proto__', 1))).toBe(ERROR_CODES.invalidParams);
    expect(await code(client.storage.set('big', 'x'.repeat(1_000_001)))).toBe(
      ERROR_CODES.invalidParams,
    );
    expect(Object.keys(backing.dump()).sort()).toEqual([
      `${STORAGE_PREFIX}vn.example.other`,
      `${STORAGE_PREFIX}vn.example.test`,
    ]);
    addinStorage('vn.example.test', backing).clear();
    expect(Object.keys(backing.dump())).toEqual([`${STORAGE_PREFIX}vn.example.other`]);
    expect(other.get('secret')).toBe('theirs');
  });

  it('no document: document calls fail with noDocument', async () => {
    const { client } = await setup({
      permissions: ['read-document', 'selection'],
      host: () => null,
    });
    expect(client.info.hasDocument).toBe(false);
    expect(await code(client.doc.text())).toBe(ERROR_CODES.noDocument);
  });

  it('sends events only with the matching permission, queued until the handshake', async () => {
    const d = docEngine();
    const f = fakeFrame();
    const bridge = new AddinBridge({
      manifest: manifest(['selection', 'ui:ribbon']),
      app: 'doc',
      granted: ['selection', 'ui:ribbon'],
      frame: f.frameWindow,
      language: () => 'vi-VN',
      host: () => d.host,
      storage: addinStorage('vn.example.test', memoryBacking()),
    });
    f.attach(bridge);
    // Before hello: queued.
    bridge.emit({ type: 'command', command: 'greet' });
    expect(f.sentToFrame).toEqual([]);
    const seen: string[] = [];
    const client: AddinClient = await f.connect();
    client.on('command', (e) => seen.push(e.type));
    client.on('selectionChanged', (e) => seen.push(e.type));
    client.on('documentChanged', (e) => seen.push(e.type));
    bridge.emit({ type: 'selectionChanged' });
    // Not granted read-document: no document events.
    bridge.emit({ type: 'documentChanged' });
    await new Promise((r) => setTimeout(r, 0));
    expect(seen).toEqual(['selectionChanged']);
    // The queued command went right after the hello answer (before the client subscribed).
    expect(f.sentToFrame.filter((m) => (m as { method?: string }).method === 'event')).toHaveLength(
      2,
    );
    bridge.dispose();
    bridge.emit({ type: 'selectionChanged' });
    await new Promise((r) => setTimeout(r, 0));
    expect(seen).toEqual(['selectionChanged']);
  });
});
