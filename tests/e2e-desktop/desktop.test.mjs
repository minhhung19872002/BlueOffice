// Desktop E2E (prompt §3.4): drives the real Tauri app (`target/release/bluedoc.exe`,
// native engine) through `tauri-driver` + the platform WebView driver (WebDriver
// protocol over plain fetch, no extra dependency).
//
//   cargo install tauri-driver --locked
//   pnpm --filter @blueoffice/desktop tauri build --no-bundle
//   MSEDGEDRIVER=<path to msedgedriver.exe matching WebView2> node --test tests/e2e-desktop
//
// The deep-link collaboration case also starts tests/e2e/collab/stack.mjs (mock
// BlueDrive + BlueOffice server + web editor; needs the built web app and .NET 8)
// and a Playwright Chromium for the web user (COLLAB_E2E=0 skips it).
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../..', import.meta.url));
const exe =
  process.env.BLUEDOC_EXE ??
  join(root, 'target/release', process.platform === 'win32' ? 'bluedoc.exe' : 'bluedoc');
const sample = join(root, 'tests/corpus/doc/review-01-insert-delete.docx');
const port = Number(process.env.TAURI_DRIVER_PORT ?? 4444);
let base = `http://127.0.0.1:${port}`;
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

/** Stops a child and its children (tauri-driver starts msedgedriver, which would keep its port). */
function killTree(child) {
  if (!child?.pid || child.exitCode !== null) return;
  if (process.platform === 'win32') {
    spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
  } else child.kill('SIGTERM');
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
    killTree(driver);
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

// ---------------------------------------------------------------- deep link + live collaboration

const collabSkip =
  skip ||
  (process.env.COLLAB_E2E === '0' ? 'COLLAB_E2E=0' : false) ||
  (!existsSync(join(root, 'apps/web/dist/index.html')) ? 'build apps/web first' : false);

async function api(method, url, body, headers = {}) {
  const res = await fetch(url, {
    method,
    headers: { 'content-type': 'application/json', ...headers },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`${method} ${url}: ${res.status}`);
  const text = await res.text();
  return text ? JSON.parse(text) : null;
}

const firstNumber = (text) => Number(/(\d[\d.]*)/.exec(text ?? '')?.[1]?.replace(/\./g, '') ?? NaN);

describe('desktop deep link joins the web users of a BlueDrive file', { skip: collabSkip }, () => {
  /** Warnings / errors of both editors, reported when a sync step times out. */
  const webLog = [];
  let stack;
  let browser;
  let web;
  let urls;
  let fileId;

  before(async () => {
    const { collabUrls } = await import('../e2e/collab/ports.mjs');
    urls = collabUrls();
    stack = spawn(process.execPath, [join(root, 'tests/e2e/collab/stack.mjs')], {
      cwd: join(root, 'tests/e2e'),
      stdio: 'ignore',
    });
    await waitFor(async () => (await fetch(`${urls.host}/`)).ok, 'collab stack', 300_000);

    // A shared file: u1 (web) owns it, u2 (desktop) may edit.
    const content = readFileSync(join(root, 'tests/corpus/doc/phase1-sample.docx')).toString(
      'base64',
    );
    ({ id: fileId } = await api('POST', `${urls.mock}/_admin/files`, {
      name: 'Họp giao ban.docx',
      ownerId: 'u1',
      content,
    }));
    await api('PUT', `${urls.mock}/_admin/files/${fileId}/permissions/u2`, {
      view: true,
      comment: true,
      edit: true,
      share: false,
      download: true,
    });
    const token = async (user) =>
      (await api('POST', `${urls.mock}/_admin/token`, { userId: user, fileId, ttlSeconds: 600 }))
        .accessToken;

    // Web user (u1) in the host page, through @blueoffice/editor-sdk.
    const { chromium } = createRequire(join(root, 'tests/e2e/package.json'))('@playwright/test');
    browser = await chromium.launch();
    web = await browser.newPage({ viewport: { width: 1366, height: 900 } });
    web.on('console', (m) => {
      if (m.type() === 'warning' || m.type() === 'error') webLog.push(m.text());
    });
    const q = new URLSearchParams({ file: fileId, token: await token('u1'), editor: urls.editor });
    await web.goto(`${urls.host}/?${q}`);
    await waitFor(
      () => web.evaluate(() => globalThis.boHost.events.some((e) => e.event === 'ready')),
      'web editor ready',
      60_000,
    );

    // Desktop user (u2) opens the BlueDrive deep link (server from BLUEOFFICE_SERVER).
    // Own driver ports (the default native port 4445 may still be held by the previous suite).
    base = `http://127.0.0.1:${port + 30}`;
    const args = ['--port', String(port + 30), '--native-port', String(port + 31)];
    if (process.env.MSEDGEDRIVER) args.push('--native-driver', process.env.MSEDGEDRIVER);
    driver = spawn('tauri-driver', args, {
      stdio: 'ignore',
      env: { ...process.env, BLUEOFFICE_SERVER: urls.server },
    });
    await waitFor(async () => (await fetch(`${base}/status`)).ok, 'tauri-driver');
    const link = `bluedoc://open?host=bluedrive&file=${encodeURIComponent(fileId)}&token=${await token('u2')}`;
    const created = await wd('POST', '/session', {
      capabilities: {
        alwaysMatch: { browserName: 'wry', 'tauri:options': { application: exe, args: [link] } },
      },
    });
    session = created.sessionId;
  });

  after(async () => {
    if (session) await wd('DELETE', s('')).catch(() => {});
    killTree(driver);
    await browser?.close();
    killTree(stack);
  });

  it('opens the file from the server session and edits it live with the web user', async () => {
    await waitFor(
      async () => {
        if ((await wd('GET', s('/title'))).includes('Họp giao ban.docx')) return true;
        // Surface the shell's notice (no server, open failure) in the timeout message.
        const notice = await exec(
          'return document.querySelector("[data-testid=hosted-notice]")?.textContent ?? "";',
        );
        if (notice) throw new Error(notice);
        return false;
      },
      'document title',
      60_000,
    );
    const count = async () =>
      firstNumber(
        await exec('return document.querySelector("[data-testid=status-words]")?.textContent;'),
      );
    const frame = web.frameLocator('iframe');
    const webCount = async () => firstNumber(await frame.getByTestId('status-words').textContent());
    const initial = await waitFor(async () => (await count()) || null, 'desktop word count');
    await waitFor(async () => (await webCount()) === initial, 'same document on web and desktop');

    // The web user types: the desktop app shows it.
    const page1 = frame.locator('.bo-page[data-page="1"]');
    const box = await page1.boundingBox();
    await page1.click({ position: { x: box.width * 0.3, y: box.height * 0.12 } });
    await web.keyboard.press('Control+Home');
    await web.keyboard.insertText('Từ trình duyệt: xin chào máy tính để bàn. ');
    const afterWeb = await waitFor(async () => {
      const n = await webCount();
      return n !== initial ? n : null;
    }, 'web edit');
    await waitFor(async () => (await count()) === afterWeb || null, 'web edit on the desktop');

    // The desktop user types: the web user sees it.
    await exec(
      'window.__boLog = []; for (const k of ["warn", "error"]) { const f = console[k].bind(console); console[k] = (...a) => { window.__boLog.push(a.map((x) => (x instanceof Error ? x.stack : String(x))).join(" ")); f(...a); }; }',
    );
    const input = await waitFor(() => find('textarea'), 'IME textarea');
    // Put the caret into the first page (WebDriver clicks the element's centre).
    const page = await find('.bo-page[data-page="1"]');
    await wd('POST', s(`/element/${page}/click`), {});
    await wd('POST', s(`/element/${input}/value`), { text: 'Từ máy tính để bàn ' });
    // WebDriver types key by key: wait until the desktop count settles (an
    // intermediate count would never match the web's final one).
    let last = null;
    const afterDesktop = await waitFor(async () => {
      const n = await count();
      const settled = n !== afterWeb && n === last;
      last = n;
      return settled ? n : null;
    }, 'desktop edit');
    try {
      await waitFor(
        async () => (await webCount()) === afterDesktop || null,
        'desktop edit on the web',
      );
    } catch (e) {
      const desktopLog = await exec('return window.__boLog ?? [];').catch(() => []);
      const words = `desktop ${await count()} (typed ${afterDesktop}), web ${await webCount()}`;
      throw new Error(
        `${e.message}
words: ${words}
web log: ${JSON.stringify(webLog)}
desktop log: ${JSON.stringify(desktopLog)}`,
        { cause: e },
      );
    }

    // Saving on the desktop (Lưu) sends a checkpoint: a new BlueDrive version by both users.
    const save = await find('[data-command="file.save"]');
    await wd('POST', s(`/element/${save}/click`), {});
    const v = await waitFor(async () => {
      const st = await api('GET', `${urls.mock}/_admin/files/${fileId}`);
      return st.versions.length > 1 ? st.versions.at(-1) : null;
    }, 'saved version');
    assert.deepEqual([...v.editors].sort(), ['u1', 'u2']);
  });
});
