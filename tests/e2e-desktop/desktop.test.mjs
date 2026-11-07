// Desktop E2E (prompt §3.4): drives the real Tauri app (`target/release/bluedoc.exe`,
// native engine) through `tauri-driver` + the platform WebView driver (WebDriver
// protocol over plain fetch, no extra dependency).
//
//   cargo install tauri-driver --locked
//   pnpm --filter @blueoffice/desktop tauri build --no-bundle
//   MSEDGEDRIVER=<path to msedgedriver.exe matching WebView2> node --test tests/e2e-desktop
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../..', import.meta.url));
const exe = join(root, 'target/release', process.platform === 'win32' ? 'bluedoc.exe' : 'bluedoc');
const sample = join(root, 'tests/corpus/doc/review-01-insert-delete.docx');
const port = Number(process.env.TAURI_DRIVER_PORT ?? 4444);
const base = `http://127.0.0.1:${port}`;
const ELEMENT = 'element-6066-11e4-a52e-4f735466cecf';

const skip = !existsSync(exe) ? `missing ${exe} (build the desktop app first)` : false;

let driver;
let session;

async function wd(method, path, body) {
  const res = await fetch(`${base}${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await res.json();
  if (!res.ok) throw new Error(`${method} ${path}: ${JSON.stringify(json.value)}`);
  return json.value;
}

const s = (path) => `/session/${session}${path}`;
const exec = (script, args = []) => wd('POST', s('/execute/sync'), { script, args });

async function waitFor(fn, what, timeout = 20_000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try {
      last = await fn();
      if (last) return last;
    } catch (e) {
      last = e;
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`timed out waiting for ${what}: ${last}`);
}

async function find(css) {
  const el = await wd('POST', s('/element'), { using: 'css selector', value: css });
  return el[ELEMENT];
}

describe('desktop app (Tauri, native engine)', { skip }, () => {
  before(async () => {
    const args = ['--port', String(port)];
    if (process.env.MSEDGEDRIVER) args.push('--native-driver', process.env.MSEDGEDRIVER);
    driver = spawn('tauri-driver', args, { stdio: 'ignore' });
    await waitFor(async () => (await fetch(`${base}/status`)).ok, 'tauri-driver');
    const created = await wd('POST', '/session', {
      capabilities: {
        alwaysMatch: {
          browserName: 'wry',
          'tauri:options': { application: exe, args: [sample] },
        },
      },
    });
    session = created.sessionId;
  });

  after(async () => {
    if (session) await wd('DELETE', s('')).catch(() => {});
    driver?.kill();
  });

  it('opens the file passed on the command line (file association)', async () => {
    const title = await waitFor(async () => {
      const t = await wd('GET', s('/title'));
      return t.includes('review-01-insert-delete.docx') ? t : null;
    }, 'document title');
    assert.match(title, /BlueDoc/);
    const canvases = await waitFor(
      async () => (await exec('return document.querySelectorAll("canvas").length;')) || null,
      'page canvas',
    );
    assert.ok(canvases >= 1);
    // The native engine answered (desktop backend, not WASM).
    const backend = await exec('return document.querySelector("[data-app]")?.dataset.app ?? null;');
    assert.equal(backend, 'doc');
  });

  it('types Vietnamese text into the document through the IME input', async () => {
    const before = await exec('return document.querySelector("[role=status]")?.textContent ?? "";');
    const input = await waitFor(() => find('textarea'), 'IME textarea');
    await wd('POST', s(`/element/${input}/value`), { text: ' Xin chào Việt Nam' });
    const afterText = await waitFor(async () => {
      const t = await exec('return document.querySelector("[role=status]")?.textContent ?? "";');
      return t !== before ? t : null;
    }, 'status bar update after typing');
    assert.notEqual(afterText, before);
  });

  it('runs BlueSheet in the same shell with the native sheet engine', async () => {
    await exec('window.location.search = "?app=sheet";');
    await waitFor(
      async () =>
        (await exec('return document.querySelector("[data-app=sheet]") !== null;')) || null,
      'BlueSheet',
    );
    const cell = await waitFor(() => find('input[aria-label]'), 'cell input');
    const editor = await exec(
      'return [...document.querySelectorAll("input")].find(i => /Ô đang nhập|Cell editor/.test(i.getAttribute("aria-label") ?? "")) ?? null;',
    );
    const target = editor?.[ELEMENT] ?? cell;
    await wd('POST', s(`/element/${target}/value`), { text: '41' });
    await wd('POST', s(`/element/${target}/value`), { text: '' }); // Enter
    await wd('POST', s(`/element/${target}/value`), { text: '=A1+1' });
    await wd('POST', s(`/element/${target}/value`), { text: '' }); // ArrowUp
    const bar = await waitFor(async () => {
      const v = await exec(
        'return [...document.querySelectorAll("input")].find(i => /Thanh công thức|Formula bar/.test(i.getAttribute("aria-label") ?? ""))?.value ?? "";',
      );
      return v === '=A1+1' ? v : null;
    }, 'formula bar shows the formula');
    assert.equal(bar, '=A1+1');
  });

  it('runs BlueSlide in the same shell', async () => {
    await exec('window.location.search = "?app=slide";');
    await waitFor(
      async () =>
        (await exec('return document.querySelector("[data-app=slide]") !== null;')) || null,
      'BlueSlide',
    );
    const title = await wd('GET', s('/title'));
    assert.match(title, /BlueSlide/);
  });
});
