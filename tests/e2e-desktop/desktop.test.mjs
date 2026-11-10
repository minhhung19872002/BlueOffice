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
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { createServer, connect } from 'node:net';
import { tmpdir } from 'node:os';
import { inflateRawSync } from 'node:zlib';
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
    // A new window of the shell shows the Start screen; "blank" opens the app.
    await exec('window.location.search = "?app=sheet&start=1";');
    const tile = await waitFor(() => find('[data-testid="start-new-sheet"]'), 'Start screen');
    await wd('POST', s(`/element/${tile}/click`), {});
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
    // A new window of the shell shows the Start screen; "blank" opens the app.
    await exec('window.location.search = "?app=slide&start=1";');
    const tile = await waitFor(() => find('[data-testid="start-new-slide"]'), 'Start screen');
    await wd('POST', s(`/element/${tile}/click`), {});
    await waitFor(
      async () =>
        (await exec('return document.querySelector("[data-app=slide]") !== null;')) || null,
      'BlueSlide',
    );
    const title = await wd('GET', s('/title'));
    assert.match(title, /BlueSlide/);
  });
});

// ---------------------------------------------------------------- shell services: windows, recent files, print, updates, save as

/** Opens the File screen (CORE-UI-002) on one of its pages. */
async function fileScreen(section) {
  await clickCss('[data-testid="file-tab"]');
  await clickCss(`[data-testid="backstage"] [data-section="${section}"]`);
}

async function clickCss(css) {
  const el = await waitFor(() => find(css), css);
  await wd('POST', s(`/element/${el}/click`), {});
}

describe(
  'desktop shell services (windows, recent files, print, updates, save as)',
  { skip },
  () => {
    let saveDir;

    before(async () => {
      saveDir = mkdtempSync(join(tmpdir(), 'bo-save-'));
      base = `http://127.0.0.1:${port + 50}`;
      const args = ['--port', String(port + 50), '--native-port', String(port + 51)];
      if (process.env.MSEDGEDRIVER) args.push('--native-driver', process.env.MSEDGEDRIVER);
      driver = spawn('tauri-driver', args, {
        stdio: 'ignore',
        env: {
          ...process.env,
          // The print window opens without the (modal) print dialog; saves skip the OS dialog.
          BLUEOFFICE_PRINT_DIALOG: '0',
          BLUEOFFICE_E2E_SAVE_DIR: saveDir,
          BLUEOFFICE_UPDATE_URL: 'https://127.0.0.1:9/latest.json',
        },
      });
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
      await waitFor(
        async () => (await wd('GET', s('/title'))).includes('review-01-insert-delete.docx'),
        'document title',
      );
      await waitFor(
        () =>
          exec(
            'return document.querySelector(".bo-page[data-page=\\"1\\"][data-rendered=true]") !== null;',
          ),
        'page 1 rendered',
      );
    });

    after(async () => {
      if (session) await wd('DELETE', s('')).catch(() => {});
      session = null;
      killTree(driver);
      if (saveDir) rmSync(saveDir, { recursive: true, force: true });
    });

    it('prints through a print window showing the PDF (OS print dialog)', async () => {
      const main = await wd('GET', s('/window'));
      const before = await wd('GET', s('/window/handles'));
      await fileScreen('print');
      await clickCss('[data-testid="backstage"] [data-command="file.print"]');
      // The print panel (CORE-PRINT-001): print the job it previews.
      await clickCss('[data-testid="print-go"]:not([disabled])');
      const handles = await waitFor(async () => {
        const h = await wd('GET', s('/window/handles'));
        return h.length > before.length ? h : null;
      }, 'print window');
      const printWin = handles.find((h) => !before.includes(h));
      await wd('POST', s('/window'), { handle: printWin });
      const url = await waitFor(async () => {
        const u = await wd('GET', s('/url'));
        return /boprint/.test(u) ? u : null;
      }, 'print window URL');
      assert.match(url, /boprint[^/]*\/\d+\.pdf$/);
      await wd('DELETE', s('/window'));
      await wd('POST', s('/window'), { handle: main });
    });

    it('checks for updates from the File screen', async () => {
      await fileScreen('options');
      await clickCss('[data-testid="backstage"] [data-command="file.checkUpdates"]');
      const info = await waitFor(async () => {
        const t = await exec(
          'return document.querySelector("[data-testid=status-info]")?.textContent ?? "";',
        );
        return t && !/Đang kiểm tra/.test(t) ? t : null;
      }, 'update check result');
      // Placeholder key → "not configured"; a dev build without the feature → "disabled".
      assert.match(
        info,
        /chưa được cấu hình máy chủ cập nhật|không có cập nhật tự động|Không kiểm tra được/,
      );
    });

    it('saves as a new file (Save as)', async () => {
      await fileScreen('saveAs');
      await clickCss('[data-testid="backstage"] [data-format=".docx"]');
      const saved = join(saveDir, 'review-01-insert-delete.docx');
      await waitFor(() => existsSync(saved), 'saved file');
      const bytes = await waitFor(() => {
        const b = readFileSync(saved);
        return b.length > 100 ? b : null;
      }, 'saved content');
      assert.equal(bytes.subarray(0, 2).toString(), 'PK');
      assert.ok(zipEntry(bytes, 'word/document.xml'), 'a .docx package');
    });

    it('lists the opened file in the recent files of the Start screen and reopens it', async () => {
      await exec('window.location.search = "?app=doc&start=1";');
      const list = await waitFor(async () => {
        const t = await exec(
          'return document.querySelector("[data-testid=start-recent]")?.textContent ?? "";',
        );
        return t.includes('review-01-insert-delete.docx') ? t : null;
      }, 'recent files');
      assert.match(list, /review-01-insert-delete\.docx/);
      await clickCss('[data-testid="start-recent"] button');
      await waitFor(
        async () => (await wd('GET', s('/title'))).includes('review-01-insert-delete.docx'),
        'reopened document',
      );
    });

    it('opens a second window and keeps both', async () => {
      const main = await wd('GET', s('/window'));
      const before = await wd('GET', s('/window/handles'));
      // WindowManager.openNew of the desktop platform (same command).
      const label = await exec(
        'return window.__TAURI_INTERNALS__.invoke("open_window", { query: "app=sheet&start=1" });',
      );
      assert.match(String(label), /^doc-\d+$/);
      const handles = await waitFor(async () => {
        const h = await wd('GET', s('/window/handles'));
        return h.length > before.length ? h : null;
      }, 'second window');
      const second = handles.find((h) => !before.includes(h));
      await wd('POST', s('/window'), { handle: second });
      await waitFor(() => find('[data-testid="start-new-sheet"]'), 'Start screen of BlueSheet');
      await wd('DELETE', s('/window'));
      await wd('POST', s('/window'), { handle: main });
      assert.ok((await wd('GET', s('/window/handles'))).includes(main));
    });

    it('Start screen settings: "Check for updates"', async () => {
      await exec('window.location.search = "?app=doc&start=1";');
      await clickCss('[data-testid="start-tab-settings"]');
      await clickCss('[data-testid="settings-update-check"]');
      const state = await waitFor(async () => {
        const v = await exec(
          'return document.querySelector("[data-testid=settings-update-status]")?.dataset.state ?? "";',
        );
        return v && v !== 'check' ? v : null;
      }, 'update status');
      assert.ok(['notConfigured', 'disabled', 'failed'].includes(state), state);
    });
  },
);

// ---------------------------------------------------------------- P1.10: the same page on web and desktop (pixels)

const paritySkip =
  skip || (!existsSync(join(root, 'apps/web/dist/index.html')) ? 'build apps/web first' : false);

/** Page 1 canvas scaled to a fixed width, as grey levels (same script in both targets). */
const PAGE_PIXELS = `
  const c = document.querySelector('.bo-page[data-page="1"] canvas');
  if (!c || !c.width) return null;
  // Box average of the canvas pixels in plain JS (no GPU scaling filter in between).
  const src = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
  const W = 300, H = Math.round((W * c.height) / c.width);
  const sum = new Float64Array(W * H), cnt = new Float64Array(W * H);
  for (let y = 0; y < c.height; y++) {
    const row = Math.min(H - 1, Math.floor((y * H) / c.height)) * W;
    for (let x = 0; x < c.width; x++) {
      const i = (y * c.width + x) * 4, a = src[i + 3] / 255;
      // Transparent pixels are white paper.
      const v = ((src[i] + src[i + 1] + src[i + 2]) / 3) * a + 255 * (1 - a);
      const k = row + Math.min(W - 1, Math.floor((x * W) / c.width));
      sum[k] += v; cnt[k] += 1;
    }
  }
  const g = Array.from(sum, (v, k) => Math.round(v / (cnt[k] || 1)));
  return { w: W, h: H, g, dpr: window.devicePixelRatio, cw: c.width, vw: window.innerWidth, vh: window.innerHeight };
`;

describe(
  'P1.10 parity: a page rendered in the Tauri window matches the web build',
  { skip: paritySkip },
  () => {
    let preview;
    let browser;
    const webPort = Number(process.env.PARITY_WEB_PORT ?? 4397);

    before(async () => {
      preview = spawn(
        process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm',
        ['--filter', '@blueoffice/web', 'preview', '--port', String(webPort), '--strictPort'],
        { cwd: root, stdio: 'ignore', shell: process.platform === 'win32' },
      );
      base = `http://127.0.0.1:${port + 60}`;
      const args = ['--port', String(port + 60), '--native-port', String(port + 61)];
      if (process.env.MSEDGEDRIVER) args.push('--native-driver', process.env.MSEDGEDRIVER);
      // Bundled fonts only: the web build has no access to the installed fonts.
      driver = spawn('tauri-driver', args, {
        stdio: 'ignore',
        env: { ...process.env, BLUEOFFICE_SYSTEM_FONTS: '0' },
      });
      await waitFor(async () => (await fetch(`${base}/status`)).ok, 'tauri-driver');
      const docPath = join(root, 'tests/corpus/doc/phase1-sample.docx');
      const created = await wd('POST', '/session', {
        capabilities: {
          alwaysMatch: {
            browserName: 'wry',
            'tauri:options': { application: exe, args: [docPath] },
          },
        },
      });
      session = created.sessionId;
    });

    after(async () => {
      if (session) await wd('DELETE', s('')).catch(() => {});
      session = null;
      killTree(driver);
      await browser?.close();
      killTree(preview);
    });

    it('page 1 of phase1-sample.docx differs by less than the threshold', async () => {
      const desktop = await waitFor(
        async () => {
          const ready = await exec(
            'return document.querySelector(".bo-page[data-page=\\"1\\"]")?.dataset.rendered === "true";',
          );
          if (!ready) return null;
          // Let late glyph outlines land before sampling.
          await new Promise((r) => setTimeout(r, 1500));
          return exec(PAGE_PIXELS);
        },
        'desktop page 1',
        60_000,
      );

      await waitFor(
        async () => (await fetch(`http://localhost:${webPort}/doc`)).ok,
        'web preview',
        60_000,
      );
      const { chromium } = createRequire(join(root, 'tests/e2e/package.json'))('@playwright/test');
      browser = await chromium.launch();
      // Same CSS viewport and device pixel ratio as the Tauri window (the canvas resolution follows it).
      const page = await browser.newPage({
        viewport: { width: desktop.vw, height: desktop.vh },
        deviceScaleFactor: desktop.dpr,
      });
      await page.goto(`http://localhost:${webPort}/doc`);
      await page.getByTestId('open-sample').click();
      await page
        .locator('.bo-page[data-page="1"][data-rendered="true"]')
        .waitFor({ timeout: 60_000 });
      await page.waitForTimeout(1500);
      const web = await page.evaluate(new Function(PAGE_PIXELS));

      assert.ok(desktop && web, 'both pages sampled');
      if (process.env.PARITY_DUMP) {
        writeFileSync(process.env.PARITY_DUMP, JSON.stringify({ desktop, web }));
      }
      assert.equal(desktop.w, web.w);
      assert.ok(Math.abs(desktop.h - web.h) <= 1, `page heights ${desktop.h} vs ${web.h}`);
      const n = Math.min(desktop.g.length, web.g.length);
      let sum = 0;
      let off = 0;
      for (let i = 0; i < n; i++) {
        const d = Math.abs(desktop.g[i] - web.g[i]);
        sum += d;
        if (d > 48) off++;
      }
      const mean = sum / n / 255;
      const share = off / n;
      // Same renderer, fonts and display list: only anti-aliasing may differ.
      const sizes = `canvas ${desktop.cw} px (dpr ${desktop.dpr}) vs ${web.cw} px (dpr ${web.dpr})`;
      console.log(
        `parity: mean ${(mean * 100).toFixed(3)} %, differing ${(share * 100).toFixed(3)} %, ${sizes}`,
      );
      assert.ok(mean < 0.01, `mean difference ${(mean * 100).toFixed(2)} %, ${sizes}`);
      assert.ok(share < 0.02, `${(share * 100).toFixed(2)} % of the pixels differ`);
      // And the page is not blank on either side.
      const ink = desktop.g.filter((v) => v < 160).length / n;
      assert.ok(ink > 0.005, `ink ${ink}`);
    });
  },
);

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

// ---------------------------------------------------------------- Start screen, sync folder, offline, BlueSheet/BlueSlide links

/** One member of a zip (stored or deflated), enough for docx/xlsx parts. */
function zipEntry(buf, name) {
  for (let i = 0; i + 30 < buf.length; i++) {
    if (buf.readUInt32LE(i) !== 0x04034b50) continue;
    const method = buf.readUInt16LE(i + 8);
    const size = buf.readUInt32LE(i + 18);
    const nameLen = buf.readUInt16LE(i + 26);
    const extra = buf.readUInt16LE(i + 28);
    const entry = buf.subarray(i + 30, i + 30 + nameLen).toString();
    const start = i + 30 + nameLen + extra;
    if (entry === name) {
      const data = buf.subarray(start, start + size);
      return method === 8 ? inflateRawSync(data) : data;
    }
  }
  return null;
}

/** TCP proxy in front of the BlueOffice server: `off()` drops every connection (the desktop goes offline). */
function serverProxy(target) {
  const t = new URL(target);
  const sockets = new Set();
  let up = true;
  const proxy = createServer((client) => {
    if (!up) return client.destroy();
    const upstream = connect(Number(t.port), t.hostname);
    sockets.add(client).add(upstream);
    const drop = () => {
      client.destroy();
      upstream.destroy();
      sockets.delete(client);
      sockets.delete(upstream);
    };
    client.on('error', drop).on('close', drop);
    upstream.on('error', drop).on('close', drop);
    client.pipe(upstream).pipe(client);
  });
  return {
    listen: () => new Promise((r) => proxy.listen(0, '127.0.0.1', () => r(proxy.address().port))),
    off() {
      up = false;
      for (const sock of sockets) sock.destroy();
      sockets.clear();
    },
    on() {
      up = true;
    },
    close: () => proxy.close(),
  };
}

describe(
  'desktop Start screen, sync folders, offline edits, BlueSheet/BlueSlide links',
  { skip: collabSkip },
  () => {
    let stack;
    let proxy;
    let browser;
    let urls;
    let syncRoot;
    const corpus = (p) => readFileSync(join(root, 'tests/corpus', p)).toString('base64');
    const token = async (user, fileId) =>
      (await api('POST', `${urls.mock}/_admin/token`, { userId: user, fileId, ttlSeconds: 600 }))
        .accessToken;
    const share = (fileId, user = 'u2') =>
      api('PUT', `${urls.mock}/_admin/files/${fileId}/permissions/${user}`, {
        view: true,
        comment: true,
        edit: true,
        share: false,
        download: true,
      });
    const createFile = async (name, content, folderId) =>
      (await api('POST', `${urls.mock}/_admin/files`, { name, ownerId: 'u1', content, folderId }))
        .id;
    const title = () => wd('GET', s('/title'));
    const text = (css) =>
      exec(`return document.querySelector(${JSON.stringify(css)})?.textContent ?? "";`);
    const click = async (css) =>
      wd('POST', s(`/element/${await waitFor(() => find(css), css)}/click`), {});
    async function launch(args) {
      if (session) await wd('DELETE', s('')).catch(() => {});
      session = null;
      const created = await wd('POST', '/session', {
        capabilities: {
          alwaysMatch: { browserName: 'wry', 'tauri:options': { application: exe, args } },
        },
      });
      session = created.sessionId;
    }
    const opens = async (fileId) =>
      (await api('GET', `${urls.mock}/_admin/webhooks`)).filter(
        (w) => w.fileId === fileId && w.event === 'audit' && w.body.includes('"action":"open"'),
      ).length;

    before(async () => {
      const { collabUrls } = await import('../e2e/collab/ports.mjs');
      urls = collabUrls();
      stack = spawn(process.execPath, [join(root, 'tests/e2e/collab/stack.mjs')], {
        cwd: join(root, 'tests/e2e'),
        stdio: 'ignore',
      });
      await waitFor(async () => (await fetch(`${urls.host}/`)).ok, 'collab stack', 300_000);
      proxy = serverProxy(urls.server);
      const proxyPort = await proxy.listen();

      // The desktop reaches the server through the proxy (so the test can cut the network).
      base = `http://127.0.0.1:${port + 40}`;
      const args = ['--port', String(port + 40), '--native-port', String(port + 41)];
      if (process.env.MSEDGEDRIVER) args.push('--native-driver', process.env.MSEDGEDRIVER);
      driver = spawn('tauri-driver', args, {
        stdio: 'ignore',
        env: { ...process.env, BLUEOFFICE_SERVER: `http://127.0.0.1:${proxyPort}` },
      });
      await waitFor(async () => (await fetch(`${base}/status`)).ok, 'tauri-driver');
      syncRoot = mkdtempSync(join(tmpdir(), 'bo-sync-'));
    });

    after(async () => {
      if (session) await wd('DELETE', s('')).catch(() => {});
      session = null;
      killTree(driver);
      await browser?.close();
      proxy?.close();
      killTree(stack);
      if (syncRoot) rmSync(syncRoot, { recursive: true, force: true });
    });

    it('Start screen: BlueDrive sign-in, browse, open; a sync-folder file switches to collaboration', async () => {
      const folder = (
        await api('POST', `${urls.mock}/_admin/folders`, { name: 'Dự án', parentId: 'root' })
      ).id;
      const fileId = await createFile('Kế hoạch.docx', corpus('doc/phase1-sample.docx'), folder);
      await share(fileId);
      mkdirSync(join(syncRoot, 'Dự án'));
      const local = join(syncRoot, 'Dự án', 'Kế hoạch.docx');
      copyFileSync(join(root, 'tests/corpus/doc/phase1-sample.docx'), local);

      await launch([]);
      await waitFor(() => find('[data-testid="start-screen"]'), 'Start screen', 60_000);

      // Settings: the server comes from BLUEOFFICE_SERVER; add the sync folder.
      await click('[data-testid="start-tab-settings"]');
      const path = await waitFor(() => find('[data-testid="settings-sync-path"]'), 'sync input');
      await wd('POST', s(`/element/${path}/value`), { text: syncRoot });
      await click('[data-testid="settings-sync-add"]');
      await waitFor(
        async () => (await text('[data-testid="settings-sync-list"]')).includes(syncRoot),
        'sync folder saved',
      );

      // BlueDrive tab: SSO is offered; sign in with a mock token (u2, not scoped to a file).
      await click('[data-testid="start-tab-drive"]');
      await waitFor(() => find('[data-testid="drive-signin-sso"]'), 'SSO button');
      const box = await waitFor(() => find('[data-testid="drive-token"]'), 'token box');
      await wd('POST', s(`/element/${box}/value`), { text: await token('u2') });
      await click('[data-testid="drive-signin-token"]');
      await waitFor(
        async () => (await text('[data-testid="drive-user"]')).includes('Trần Thị Bình'),
        'signed in',
      );

      // "Shared with me" lists the file; My files → folder → file opens through a server session.
      await click('[data-testid="start-tab-drive-shared"]');
      await waitFor(
        async () => (await text('[data-testid="drive-list-shared"]')).includes('Kế hoạch.docx'),
        'shared list',
      );
      await click('[data-testid="start-tab-drive-files"]');
      await click('[data-testid="drive-item"][data-name="Dự án"]');
      await click('[data-testid="drive-item"][data-name="Kế hoạch.docx"]');
      await waitFor(async () => (await title()).includes('Kế hoạch.docx'), 'hosted file', 60_000);
      await waitFor(async () => (await opens(fileId)) >= 1, 'session opened on the server');
      const recentOnHost = await api(
        'GET',
        `${urls.server}/api/hosts/bluedrive/recent`,
        undefined,
        {
          authorization: `Bearer ${await token('u2')}`,
        },
      );
      assert.equal(recentOnHost.items[0]?.id, fileId);

      // Back to Start (Alt+Home); the local copy in the sync folder is a recent file.
      await exec(
        `localStorage.setItem("blueoffice.desktop.recent", JSON.stringify([{ name: "Kế hoạch.docx", path: ${JSON.stringify(local)}, openedAt: Date.now() }]));
       window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", altKey: true }));`,
      );
      await click('[data-testid="start-tab-home"]');
      await click('[data-testid="start-recent"] button');
      await waitFor(
        async () => (await text('[data-testid="sync-prompt"]')).includes('Kế hoạch.docx'),
        'sync prompt',
      );
      await click('[data-testid="sync-prompt-online"]');
      await waitFor(async () => (await opens(fileId)) >= 2, 'online collaboration session', 60_000);
      await waitFor(async () => (await title()).includes('Kế hoạch.docx'), 'collab title', 60_000);
    });

    it('offline: saves locally while the server is unreachable; reopening creates the conflict copy', async () => {
      const fileId = await createFile('Biên bản.docx', corpus('doc/phase1-sample.docx'), 'root');
      await share(fileId);
      const link = async () =>
        `bluedoc://open?host=bluedrive&file=${encodeURIComponent(fileId)}&token=${await token('u2', fileId)}`;
      await launch([await link()]);
      await waitFor(async () => (await title()).includes('Biên bản.docx'), 'hosted file', 60_000);
      // Let the live connection come up before the network goes away.
      await new Promise((r) => setTimeout(r, 3000));

      proxy.off();
      await waitFor(
        async () =>
          (await exec(
            'return document.querySelector("[data-testid=hosted-banner]")?.dataset.state ?? "";',
          )) === 'offline',
        'offline banner',
        30_000,
      );
      const input = await waitFor(() => find('textarea'), 'IME textarea');
      await click('.bo-page[data-page="1"]');
      await wd('POST', s(`/element/${input}/value`), { text: 'Sửa khi mất mạng ' });
      await new Promise((r) => setTimeout(r, 1000));
      await click('[data-command="file.save"]');
      await waitFor(
        async () => (await text('[data-testid="hosted-notice"]')).includes('ngoại tuyến'),
        'saved offline',
      );

      // Meanwhile someone uploads a new version in BlueDrive; the desktop app is closed.
      await fetch(`${urls.mock}/_admin/files/${fileId}/contents`, {
        method: 'PUT',
        body: readFileSync(join(root, 'tests/corpus/doc/review-01-insert-delete.docx')),
      });
      await wd('DELETE', s(''));
      session = null;
      proxy.on();

      // Reopened online: the offline save goes to …/sync → conflict copy; the original is kept.
      await launch([await link()]);
      await waitFor(
        async () => (await text('[data-testid="hosted-notice"]')).includes('xung đột'),
        'conflict notice',
        60_000,
      );
      const files = await api('GET', `${urls.mock}/_admin/folders/root/files`);
      const copy = files.find((f) => f.name.startsWith('Biên bản (xung đột - Trần Thị Bình - '));
      assert.ok(copy, `conflict copy in ${JSON.stringify(files.map((f) => f.name))}`);
      assert.match(
        copy.name,
        /\(xung đột - Trần Thị Bình - \d{4}-\d{2}-\d{2} \d{2}-\d{2}\)\.docx$/,
      );
      const original = await api('GET', `${urls.mock}/_admin/files/${fileId}`);
      assert.equal(original.versions.length, 2);
      assert.equal(original.versions[1].note, 'External upload');
      const copied = await api('GET', `${urls.mock}/_admin/files/${copy.id}`);
      const xml =
        zipEntry(
          Buffer.from(copied.versions[0].content, 'base64'),
          'word/document.xml',
        )?.toString() ?? '';
      assert.match(xml, /mất mạng/);
    });

    it('bluesheet:// joins the web user live in BlueSheet; blueslide:// opens BlueSlide', async () => {
      const sheetId = await createFile('Bảng giờ.xlsx', corpus('sheet/sst-1.xlsx'), 'root');
      await share(sheetId);
      const { chromium } = createRequire(join(root, 'tests/e2e/package.json'))('@playwright/test');
      browser = await chromium.launch();
      const web = await browser.newPage({ viewport: { width: 1366, height: 900 } });
      const q = new URLSearchParams({
        file: sheetId,
        token: await token('u1', sheetId),
        editor: urls.editor,
      });
      await web.goto(`${urls.host}/?${q}`);
      await waitFor(
        () => web.evaluate(() => globalThis.boHost.events.some((e) => e.event === 'ready')),
        'web ready',
        60_000,
      );

      await launch([
        `bluesheet://open?host=bluedrive&file=${sheetId}&token=${await token('u2', sheetId)}`,
      ]);
      await waitFor(
        async () =>
          (await exec('return document.querySelector("[data-app=sheet]") !== null;')) || null,
        'BlueSheet',
        60_000,
      );
      await waitFor(async () => (await title()).includes('Bảng giờ.xlsx'), 'sheet title', 60_000);
      await new Promise((r) => setTimeout(r, 3000));

      // The web user types a formula; the desktop replica has it and saves it to BlueDrive.
      const frame = web.frameLocator('iframe');
      const nameBox = frame.getByLabel('Hộp tên');
      await nameBox.click();
      await nameBox.fill('H20');
      await nameBox.press('Enter');
      await web.keyboard.type('=2*21');
      await web.keyboard.press('Enter');
      let sheetXml = '';
      await waitFor(
        async () => {
          await click('[data-command="file.save"]');
          const st = await api('GET', `${urls.mock}/_admin/files/${sheetId}`);
          const v = st.versions.at(-1);
          sheetXml =
            zipEntry(Buffer.from(v.content, 'base64'), 'xl/worksheets/sheet1.xml')?.toString() ??
            '';
          return st.versions.length > 1 && /<f>2\*21<\/f>/.test(sheetXml);
        },
        "desktop save with the web user's formula",
        60_000,
      );
      assert.match(sheetXml, /<c r="H20"[^>]*>\s*<f>2\*21<\/f>/);

      const deckId = await createFile(
        'Trình bày.pptx',
        corpus('slide/slide-text-bullets.pptx'),
        'root',
      );
      await share(deckId);
      await launch([
        `blueslide://open?host=bluedrive&file=${deckId}&token=${await token('u2', deckId)}`,
      ]);
      await waitFor(
        async () =>
          (await exec('return document.querySelector("[data-app=slide]") !== null;')) || null,
        'BlueSlide',
        60_000,
      );
      await waitFor(async () => (await opens(deckId)) >= 1, 'slide session');
    });
  },
);
