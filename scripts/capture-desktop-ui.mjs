// Captures real Tauri/WebView2 pixels at the machine's current DPI. Run once per desired
// Windows scaling setting; the manifest records DPR rather than labelling emulated DPI as real.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
const args = process.argv.slice(2);
const opt = (key, fallback) => {
  const i = args.indexOf(`--${key}`);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const exe = resolve(
  opt('exe', process.env.BLUEDOC_EXE ?? join(root, 'target/release/bluedoc.exe')),
);
const out = resolve(opt('out', join(root, 'docs/ui/screens/capture')));
const port = Number(opt('port', process.env.TAURI_DRIVER_PORT ?? '4444'));
const base = `http://127.0.0.1:${port}`;
const sleep = (ms) => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
const profile = mkdtempSync(join(tmpdir(), 'blueoffice-ui-capture-'));
const perfLog = join(profile, 'perf.jsonl');
const driverArgs = ['--port', String(port)];
if (process.env.MSEDGEDRIVER) driverArgs.push('--native-driver', process.env.MSEDGEDRIVER);
const driver = spawn('tauri-driver', driverArgs, {
  env: {
    ...process.env,
    BLUEOFFICE_PERF_LOG: perfLog,
    BLUEOFFICE_E2E_CACHE_DIR: join(profile, 'offline'),
    WEBVIEW2_USER_DATA_FOLDER: profile,
  },
  stdio: ['ignore', 'ignore', 'pipe'],
});
let log = '';
driver.stderr.on('data', (chunk) => {
  log = (log + chunk).slice(-4000);
});
let session;
async function wd(method, path, body) {
  const response = await fetch(`${base}${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await response.json();
  if (!response.ok) throw new Error(`${method} ${path}: ${JSON.stringify(json.value)}`);
  return json.value;
}
const execute = (script, a = []) =>
  wd('POST', `/session/${session}/execute/sync`, { script, args: a });
async function waitFor(fn, label, timeout = 30000) {
  const until = Date.now() + timeout;
  while (Date.now() < until) {
    try {
      const result = await fn();
      if (result) return result;
    } catch {
      /* navigation in progress */
    }
    await sleep(150);
  }
  throw new Error(`Timed out waiting for ${label}; ${log}`);
}
const slug = (text) =>
  text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/gi, '-')
    .replace(/^-|-$/g, '')
    .toLowerCase();
const dpiRegistry =
  process.platform === 'win32'
    ? spawnSync(
        'reg',
        ['query', 'HKCU\\Control Panel\\Desktop\\WindowMetrics', '/v', 'AppliedDPI'],
        { encoding: 'utf8' },
      ).stdout
    : '';
const dpiValue = /REG_DWORD\s+0x([0-9a-f]+)/i.exec(dpiRegistry ?? '');
const manifest = {
  exe,
  exeSha256: createHash('sha256').update(readFileSync(exe)).digest('hex'),
  capturedAt: new Date().toISOString(),
  windowsScalePercent: dpiValue ? Math.round((parseInt(dpiValue[1], 16) / 96) * 100) : null,
  captures: [],
  warnings: [],
};
async function capture(name, app, theme) {
  await sleep(250); // settle UI transitions
  const meta = await execute(
    'return {dpr: devicePixelRatio, width: innerWidth, height: innerHeight, app: document.querySelector("[data-app]")?.dataset.app}',
  );
  const png = await wd('GET', `/session/${session}/screenshot`);
  const file = `${app}-${theme}-${slug(name)}.png`;
  writeFileSync(join(out, file), Buffer.from(png, 'base64'));
  manifest.captures.push({ file, app, theme, name, ...meta });
  console.log(file);
}
try {
  mkdirSync(out, { recursive: true });
  await waitFor(async () => (await fetch(`${base}/status`)).ok, 'tauri-driver', 20000);
  const created = await wd('POST', '/session', {
    capabilities: { alwaysMatch: { browserName: 'wry', 'tauri:options': { application: exe } } },
  });
  session = created.sessionId;
  for (const theme of opt('theme', 'light,dark').split(',')) {
    for (const app of opt('app', 'doc,sheet,slide').split(',')) {
      await execute(
        'localStorage.setItem("bo.theme", arguments[0]); window.location.search = `?app=${arguments[1]}&start=1`;',
        [theme, app],
      );
      await waitFor(
        () =>
          execute('return !!document.querySelector(`[data-testid="start-new-${arguments[0]}"]`)', [
            app,
          ]),
        `${app} start`,
      );
      await execute('document.querySelector(`[data-testid="start-new-${arguments[0]}"]`).click()', [
        app,
      ]);
      await waitFor(
        () =>
          execute(
            'return !!document.querySelector(`[data-app="${arguments[0]}"] [data-testid="ribbon"]`)',
            [app],
          ),
        `${app} editor`,
      );
      if (app === 'doc') {
        await waitFor(() => {
          try {
            return readFileSync(perfLog, 'utf8').includes('"event":"cache-isolated"');
          } catch {
            return false;
          }
        }, 'isolated recovery cache');
      }
      const createId = app === 'doc' ? 'new-document' : app === 'slide' ? 'new-presentation' : null;
      if (createId) {
        await waitFor(
          () =>
            execute(
              'const b=document.querySelector(`[data-testid="${arguments[0]}"]`); return !!b && !b.disabled',
              [createId],
            ),
          `${app} new document button`,
        );
        await execute('document.querySelector(`[data-testid="${arguments[0]}"]`).click()', [
          createId,
        ]);
      }
      await waitFor(
        () => execute('return !!document.querySelector("canvas")'),
        `${app} document canvas`,
      );
      if (app === 'doc') {
        manifest.docPixels = await execute(`const c=document.querySelector('.bo-page canvas');
          if (!c) return null;
          const r=c.getBoundingClientRect(), x=r.left+r.width/2, y=r.top+10;
          return { canvasPixel:[...c.getContext('2d').getImageData(Math.floor(c.width/2), 10, 1, 1).data],
            elements:document.elementsFromPoint(x,y).slice(0,5).map(e=>({tag:e.tagName,className:e.className})) };`);
      }
      await capture('home', app, theme);
      const holdMs = Number(opt('hold-ms', '0'));
      if (holdMs > 0) await sleep(holdMs);
      const tabs = await execute(
        'return [...document.querySelectorAll("[data-testid=ribbon] [role=tab]")].map(x => x.textContent?.trim()).filter(Boolean)',
      );
      for (const tab of tabs) {
        const element = await execute(
          'return [...document.querySelectorAll("[data-testid=ribbon] [role=tab]")].find(x=>x.textContent?.trim()===arguments[0]) ?? null;',
          [tab],
        );
        const id = element?.['element-6066-11e4-a52e-4f735466cecf'];
        if (!id) {
          manifest.warnings.push(`${app}/${theme}: missing tab ${tab}`);
          continue;
        }
        await wd('POST', `/session/${session}/element/${id}/click`, {});
        await waitFor(
          () =>
            execute(
              'return [...document.querySelectorAll("[data-testid=ribbon] [role=tab]")].some(x=>x.textContent?.trim()===arguments[0] && x.getAttribute("aria-selected")==="true")',
              [tab],
            ),
          `active tab ${tab}`,
        );
        await capture(`tab-${tab}`, app, theme);
      }
      const file = await execute(
        'const b=document.querySelector("[data-testid=file-tab]"); if(!b)return false; b.click(); return true',
      );
      if (file) {
        await waitFor(
          () => execute('return !!document.querySelector("[data-testid=backstage]")'),
          'backstage',
        );
        await capture('file', app, theme);
      } else manifest.warnings.push(`${app}/${theme}: missing file tab`);
    }
  }
  const gap = 'Dialogs, menus, tooltip and alternate physical DPI are not yet covered.';
  manifest.warnings.push(gap);
  console.warn(`Capture coverage incomplete: ${gap}`);
} finally {
  writeFileSync(join(out, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  if (session) await wd('DELETE', `/session/${session}`).catch(() => {});
  if (driver.pid) {
    if (process.platform === 'win32')
      spawnSync('taskkill', ['/pid', String(driver.pid), '/T', '/F'], { stdio: 'ignore' });
    else driver.kill('SIGTERM');
  }
}
