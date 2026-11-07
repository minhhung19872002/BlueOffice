// Test helpers for the collaboration E2E: mock BlueDrive admin API, the
// BlueOffice server's host notification, opening the host page for a user,
// and reading parts of saved OOXML packages.
import {
  expect,
  type Browser,
  type BrowserContext,
  type FrameLocator,
  type Page,
} from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';
import { collabUrls } from './ports.mjs';

export const urls = collabUrls();
const here = dirname(fileURLToPath(import.meta.url));
export const corpus = (path: string) => join(here, '..', '..', 'corpus', path);

export const USERS = { u1: 'Nguyễn Văn An', u2: 'Trần Thị Bình', u3: 'Lê Văn Cường' } as const;
export type UserId = keyof typeof USERS;

export interface Perms {
  view: boolean;
  comment: boolean;
  edit: boolean;
  share: boolean;
  download: boolean;
}

async function call<T>(
  method: string,
  url: string,
  body?: unknown,
  headers: Record<string, string> = {},
) {
  const res = await fetch(url, {
    method,
    headers: { 'content-type': 'application/json', ...headers },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`${method} ${url}: ${res.status} ${await res.text()}`);
  const text = await res.text();
  return (text ? JSON.parse(text) : null) as T;
}

/** Creates a file owned by u1 (all rights) and shares it with u2 (edit) and u3 (view). */
export async function createFile(name: string, bytes: Uint8Array): Promise<string> {
  const { id } = await call<{ id: string }>('POST', `${urls.mock}/_admin/files`, {
    name,
    ownerId: 'u1',
    content: Buffer.from(bytes).toString('base64'),
  });
  await setPerms(id, 'u2', { view: true, comment: true, edit: true, share: false, download: true });
  await setPerms(id, 'u3', {
    view: true,
    comment: false,
    edit: false,
    share: false,
    download: true,
  });
  return id;
}

export const createFromCorpus = (path: string, name: string) =>
  createFile(name, new Uint8Array(readFileSync(corpus(path))));

export function setPerms(fileId: string, user: UserId, p: Perms) {
  return call('PUT', `${urls.mock}/_admin/files/${fileId}/permissions/${user}`, p);
}

export async function token(user: string, fileId?: string): Promise<string> {
  const r = await call<{ accessToken: string }>('POST', `${urls.mock}/_admin/token`, {
    userId: user,
    fileId: fileId ?? null,
    ttlSeconds: 600,
  });
  return r.accessToken;
}

export interface MockVersion {
  version: string;
  modifiedBy: string;
  note: string | null;
  editors: string[];
  content: string;
}

export function fileState(fileId: string) {
  return call<{ name: string; version: string; lockId: string | null; versions: MockVersion[] }>(
    'GET',
    `${urls.mock}/_admin/files/${fileId}`,
  );
}

export function folderFiles(folderId = 'root') {
  return call<{ id: string; name: string; version: string }[]>(
    'GET',
    `${urls.mock}/_admin/folders/${folderId}/files`,
  );
}

/** BlueDrive tells BlueOffice that permissions changed (service token, docs/bohp.md §D). */
export async function notifyPermissions(fileId: string, userId?: string) {
  const service = await token('bluedrive-service');
  return call<{ sessions: number }>(
    'POST',
    `${urls.server}/api/hosts/bluedrive/notify`,
    { event: 'permissions-changed', fileId, userId: userId ?? null },
    { authorization: `Bearer ${service}` },
  );
}

/** One user's browser with the host page embedding the editor for `fileId`. */
export interface Participant {
  context: BrowserContext;
  page: Page;
  frame: FrameLocator;
  errors: string[];
}

export async function openAs(browser: Browser, user: UserId, fileId: string): Promise<Participant> {
  const context = await browser.newContext({ viewport: { width: 1366, height: 900 } });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const q = new URLSearchParams({
    file: fileId,
    token: await token(user, fileId),
    editor: urls.editor,
    user: USERS[user],
  });
  await page.goto(`${urls.host}/?${q}`);
  const frame = page.frameLocator('iframe');
  await expect
    .poll(() => page.evaluate(() => window.boHost.events.map((e) => e.event)), {
      message: `${user}: editor ready`,
      timeout: 60_000,
    })
    .toContain('ready');
  return { context, page, frame, errors };
}

/** Events the host page received from the editor. */
export const hostEvents = (p: Participant) =>
  p.page.evaluate(() => window.boHost.events.map((e) => ({ ...e })));

/** Runs an SDK command (`save`, …) from the host page. */
export const hostCommand = (p: Participant, cmd: 'save' | 'print' | 'export-pdf' | 'close') =>
  p.page.evaluate((c) => window.boHost.editor.command(c), cmd);

// ---------------------------------------------------------------- OOXML

/** Reads one entry of a ZIP package (stored or deflated). */
export function zipEntry(zip: Uint8Array, name: string): Buffer | null {
  const buf = Buffer.from(zip);
  // End of central directory (no ZIP64 needed for test files).
  let eocd = -1;
  for (let i = buf.length - 22; i >= 0; i--) {
    if (buf.readUInt32LE(i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) return null;
  const count = buf.readUInt16LE(eocd + 10);
  let p = buf.readUInt32LE(eocd + 16);
  for (let n = 0; n < count; n++) {
    const method = buf.readUInt16LE(p + 10);
    const size = buf.readUInt32LE(p + 20);
    const nameLen = buf.readUInt16LE(p + 28);
    const extra = buf.readUInt16LE(p + 30);
    const comment = buf.readUInt16LE(p + 32);
    const local = buf.readUInt32LE(p + 42);
    const entry = buf.subarray(p + 46, p + 46 + nameLen).toString('utf8');
    if (entry === name) {
      const start = local + 30 + buf.readUInt16LE(local + 26) + buf.readUInt16LE(local + 28);
      const data = buf.subarray(start, start + size);
      return method === 0 ? Buffer.from(data) : inflateRawSync(data);
    }
    p += 46 + nameLen + extra + comment;
  }
  return null;
}

/** `word/document.xml` of a saved version. */
export const documentXml = (v: MockVersion) =>
  zipEntry(Buffer.from(v.content, 'base64'), 'word/document.xml')?.toString('utf8') ?? '';
