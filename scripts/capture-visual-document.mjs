// Capture an actual document in desktop WebView2 at the current physical Windows scale.
// Run once per scale; do not label an emulated DPR as a physical DPI measurement.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i < 0 ? fallback : args[i + 1];
};
const exe = resolve(option('exe', process.env.BLUEDOC_EXE ?? 'target/release/bluedoc.exe'));
const file = resolve(option('document', 'tests/corpus/visual/phase1-sample.docx'));
const out = resolve(option('out', 'docs/visual/screens/document-capture'));
const port = Number(option('port', '4444'));
const root = `http://127.0.0.1:${port}`;
const profile = mkdtempSync(join(tmpdir(), 'blueoffice-visual-doc-'));
const driverArgs = ['--port', String(port)];
if (process.env.MSEDGEDRIVER) driverArgs.push('--native-driver', process.env.MSEDGEDRIVER);
const driver = spawn('tauri-driver', driverArgs, {
  env: {
    ...process.env,
    BLUEOFFICE_E2E_CACHE_DIR: join(profile, 'offline'),
    WEBVIEW2_USER_DATA_FOLDER: profile,
  },
  stdio: ['ignore', 'ignore', 'pipe'],
});
let log = '';
driver.stderr.on('data', (chunk) => (log = (log + chunk).slice(-4000)));
let session;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function wd(method, path, body) {
  const res = await fetch(root + path, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await res.json();
  if (!res.ok) throw new Error(`${method} ${path}: ${JSON.stringify(json.value)}`);
  return json.value;
}
const exec = (script, args = []) =>
  wd('POST', `/session/${session}/execute/sync`, { script, args });
async function waitFor(fn, label) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {
      // Startup and navigation are asynchronous.
    }
    await sleep(150);
  }
  throw new Error(`Timed out waiting for ${label}; ${log}`);
}
const registry =
  process.platform === 'win32'
    ? spawnSync(
        'reg',
        ['query', 'HKCU\\Control Panel\\Desktop\\WindowMetrics', '/v', 'AppliedDPI'],
        {
          encoding: 'utf8',
        },
      ).stdout
    : '';
const scale = /REG_DWORD\s+0x([0-9a-f]+)/i.exec(registry ?? '');
const hash = (path) => createHash('sha256').update(readFileSync(path)).digest('hex');
const manifest = {
  exe,
  exeSha256: hash(exe),
  document: file,
  documentSha256: hash(file),
  capturedAt: new Date().toISOString(),
  windowsScalePercent: scale ? Math.round((parseInt(scale[1], 16) / 96) * 100) : null,
  captures: [],
};
try {
  mkdirSync(out, { recursive: true });
  await waitFor(async () => (await fetch(`${root}/status`)).ok, 'driver');
  const created = await wd('POST', '/session', {
    capabilities: {
      alwaysMatch: { browserName: 'wry', 'tauri:options': { application: exe, args: [file] } },
    },
  });
  session = created.sessionId;
  await waitFor(
    () => exec('return window.__TAURI_INTERNALS__?.invoke?.("automation_cache_isolated") ?? false'),
    'isolated WebDriver profile',
  );
  await waitFor(
    () => wd('GET', `/session/${session}/title`).then((t) => t.includes(basename(file))),
    'document title',
  );
  await waitFor(() => exec('return !!document.querySelector(".bo-page canvas")'), 'page canvas');
  for (const theme of ['light', 'dark']) {
    await exec('document.querySelector("[data-testid=file-tab]").click()');
    await waitFor(
      () => exec('return !!document.querySelector("[data-section=options]")'),
      'options',
    );
    await exec('document.querySelector("[data-section=options]").click()');
    await waitFor(
      () => exec('return !!document.querySelector("[data-testid=backstage-theme]")'),
      'theme setting',
    );
    await exec(
      `const select=document.querySelector('[data-testid=backstage-theme]');
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(select, arguments[0]);
      select.dispatchEvent(new Event('change', {bubbles:true}));`,
      [theme],
    );
    await waitFor(
      () =>
        exec(
          'return document.querySelector("[data-theme]")?.getAttribute("data-theme") === arguments[0]',
          [theme],
        ),
      `theme ${theme}`,
    );
    await exec('document.querySelector("[data-testid=backstage-back]").click()');
    await waitFor(
      () => exec('return !document.querySelector("[data-testid=backstage]")'),
      'close backstage',
    );
    for (const zoom of [50, 100, 200, 400]) {
      await exec(
        `const input = document.querySelector('[data-testid="status-zoom-slider"]');
      if (!input) throw Error('zoom slider missing');
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, arguments[0]);
      input.dispatchEvent(new Event('input', {bubbles:true}));
      input.dispatchEvent(new Event('change', {bubbles:true}));`,
        [String(zoom)],
      );
      await waitFor(
        () =>
          exec(
            'return Number(document.querySelector("[data-testid=status-zoom-slider]")?.value) === arguments[0]',
            [zoom],
          ),
        `zoom control ${zoom}`,
      );
      await exec(`const scroller = document.querySelector('.bo-doc-scroller');
      if (!scroller) throw Error('document scroller missing');
      scroller.scrollTop = 0; scroller.scrollLeft = 0;
      document.querySelector('[data-testid=caret]')?.scrollIntoView({block:'nearest', inline:'center'});`);
      const meta = await waitFor(
        () =>
          exec(
            `const input=document.querySelector('[data-testid="status-zoom-slider"]');
      const canvas=document.querySelector('.bo-page canvas'); if (!canvas || Number(input?.value)!==arguments[0]) return null;
      const r=canvas.getBoundingClientRect(), dpr=devicePixelRatio;
      if (Math.abs(canvas.width-r.width*dpr)>2 || Math.abs(canvas.height-r.height*dpr)>2) return null;
      return {dpr, width:innerWidth, height:innerHeight, canvasWidth:canvas.width, canvasHeight:canvas.height,
        cssWidth:r.width, cssHeight:r.height, theme:document.querySelector('[data-theme]')?.getAttribute('data-theme') ?? null};`,
            [zoom],
          ),
        `zoom ${zoom}`,
      );
      await sleep(400);
      const name = `doc-${theme}-${zoom}.png`;
      const png = await wd('GET', `/session/${session}/screenshot`);
      writeFileSync(join(out, name), Buffer.from(png, 'base64'));
      manifest.captures.push({ file: name, zoom, ...meta });
      console.log(name);
    }
  }
} finally {
  writeFileSync(join(out, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  if (session) await wd('DELETE', `/session/${session}`).catch(() => {});
  if (driver.pid) {
    if (process.platform === 'win32')
      spawnSync('taskkill', ['/pid', String(driver.pid), '/T', '/F'], { stdio: 'ignore' });
    else driver.kill('SIGTERM');
  }
}
