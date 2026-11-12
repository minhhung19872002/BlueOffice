// Desktop speed harness (docs/perf/BASELINE.md): measures the RELEASE desktop app on this
// machine and writes JSON + Markdown, optionally comparing with a committed baseline.
//
//   pnpm perf:desktop [-- options]
//     --exe <bluedoc.exe>      default $BLUEDOC_EXE, else $CARGO_TARGET_DIR|target/release/bluedoc.exe
//     --runs <n>               runs per case, the median is reported (default 5)
//     --doc <file.docx>        large document (default out/perf/perf-big-100.docx, generated with
//                              `bo-cli sample-big --pages 100 --rich`; $BO_CLI or cargo run)
//     --out <dir>              default out/perf (desktop-perf.json, desktop-perf.md)
//     --profile <name>         machine profile; the baseline is docs/perf/baselines/<name>.json
//     --baseline <file>        explicit baseline file (overrides --profile)
//     --update-baseline        write the result as the baseline of the profile
//     --no-webdriver           start-up / open / RAM only (no typing / scrolling / renders)
//     --only-webdriver         typing / scrolling / renders only
//     --soak-only              run the 10-minute edit/scroll/image/undo memory test
//     --soak-ms <n>            soak duration (default 600000; shorter runs are diagnostics)
//     --webview-snapshots      take CDP heap snapshots before/after the soak when available
//     --fail-over <pct>        exit 1 when a metric is slower than the baseline by more (default 20)
//     --warn-over <pct>        flag a metric slower by more (default 5)
//
// Start-up, open and RAM runs start the exe directly with BLUEOFFICE_PERF_LOG (the app logs its
// milestones, src-tauri/src/perf.rs + perf-probe.js). Typing, scrolling and React render counts
// drive the app through tauri-driver + msedgedriver ($MSEDGEDRIVER, matching WebView2), as the
// desktop E2E does. Windows only (process tree memory through CIM).
import { spawn, spawnSync, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { cpus, totalmem, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && i + 1 < args.length ? args[i + 1] : dflt;
};
const flag = (name) => args.includes(`--${name}`);

const targetDir = process.env.CARGO_TARGET_DIR || join(root, 'target');
const exe = resolve(
  opt('exe', process.env.BLUEDOC_EXE || join(targetDir, 'release', 'bluedoc.exe')),
);
const runs = Number(opt('runs', '5'));
if (!Number.isInteger(runs) || runs < 1) throw new Error('--runs must be a positive integer');
const outDir = resolve(opt('out', join(root, 'out', 'perf')));
const profile = opt('profile', process.env.PERF_PROFILE || '');
const baselinePath = opt(
  'baseline',
  profile ? join(root, 'docs', 'perf', 'baselines', `${profile}.json`) : '',
);
const failOver = Number(opt('fail-over', '20'));
const warnOver = Number(opt('warn-over', '5'));
const profileDir = join(tmpdir(), 'perf-desktop-webview2');
// The webdriver build redirects its offline/recovery cache here; never touch the user's cache.
process.env.BLUEOFFICE_E2E_CACHE_DIR = mkdtempSync(join(tmpdir(), 'blueoffice-perf-cache-'));
const smallDoc = join(root, 'tests', 'corpus', 'doc', 'phase1-sample.docx');
const bigDoc = resolve(opt('doc', join(outDir, 'perf-big-100.docx')));
const useWebDriver = !flag('no-webdriver');
const port = Number(process.env.TAURI_DRIVER_PORT ?? 4444);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const median = (v) => {
  const s = v.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  if (!s.length) return null;
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
const pct = (v, p) => {
  const s = v.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * p))] : null;
};
const r1 = (n) => (n == null ? null : Math.round(n * 10) / 10);

if (process.platform !== 'win32') {
  console.error('perf-desktop: Windows only (WebView2 process tree).');
  process.exit(2);
}
if (!existsSync(exe)) {
  console.error(`perf-desktop: missing ${exe} — build the release desktop app first.`);
  process.exit(2);
}
mkdirSync(outDir, { recursive: true });

// ---------------------------------------------------------------- machine + inputs

function powershell(script) {
  return execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
}

function machine() {
  const ps = powershell(`
    $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
    $os = Get-CimInstance Win32_OperatingSystem
    $gpu = @(Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name + ' (' + $_.DriverVersion + ')' })
    $dpi = (Get-ItemProperty 'HKCU:\\Control Panel\\Desktop\\WindowMetrics' -Name AppliedDPI -ErrorAction SilentlyContinue).AppliedDPI
    $load = (Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average
    $wv = $null
    foreach ($k in @('HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
                     'HKCU:\\Software\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}')) {
      $pv = (Get-ItemProperty -Path $k -Name pv -ErrorAction SilentlyContinue).pv
      if ($pv -and $pv -ne '0.0.0.0') { $wv = $pv; break }
    }
    [pscustomobject]@{
      cpu = $cpu.Name.Trim(); cores = $cpu.NumberOfCores; threads = $cpu.NumberOfLogicalProcessors
      ramGB = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 1)
      gpu = $gpu; os = $os.Caption + ' ' + $os.Version + ' (build ' + $os.BuildNumber + ')'
      scalePercent = if ($dpi) { [math]::Round($dpi / 96 * 100) } else { 100 }
      cpuLoadPercent = $load; webView2 = $wv
    } | ConvertTo-Json -Compress`);
  return JSON.parse(ps);
}

function cpuLoad() {
  try {
    return Number(
      powershell(
        '(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average',
      ).trim(),
    );
  } catch {
    return null;
  }
}

function ensureBigDoc() {
  if (existsSync(bigDoc)) return;
  mkdirSync(dirname(bigDoc), { recursive: true });
  const cli = process.env.BO_CLI;
  const a = ['sample-big', bigDoc, '--pages', '100', '--rich'];
  console.log(`generating ${bigDoc}`);
  if (cli) execFileSync(cli, a, { stdio: 'inherit' });
  else
    execFileSync('cargo', ['run', '--release', '-q', '-p', 'bo-cli', '--', ...a], {
      cwd: root,
      stdio: 'inherit',
    });
}

// ---------------------------------------------------------------- processes + memory

/** All processes: pid → { ppid, name, ws, priv } (bytes; private working set from perf data). */
function processTable() {
  const json = powershell(`
    $p = @{}
    Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | ForEach-Object { $p[[int]$_.IDProcess] = [int64]$_.WorkingSetPrivate }
    Get-CimInstance Win32_Process | ForEach-Object {
      [pscustomobject]@{ pid = [int]$_.ProcessId; ppid = [int]$_.ParentProcessId; name = $_.Name;
        ws = [int64]$_.WorkingSetSize; priv = $p[[int]$_.ProcessId]; path = $_.ExecutablePath } } | ConvertTo-Json -Compress`);
  return JSON.parse(json);
}

/** Memory of `pid` and every descendant (the WebView2 browser, renderer, GPU, … processes). */
function treeMemory(pid) {
  const all = processTable();
  const kids = new Map();
  for (const p of all) {
    if (!kids.has(p.ppid)) kids.set(p.ppid, []);
    kids.get(p.ppid).push(p);
  }
  const self = all.find((p) => p.pid === pid);
  if (!self) return null;
  const tree = [self];
  for (let i = 0; i < tree.length; i++) tree.push(...(kids.get(tree[i].pid) ?? []));
  const mb = (b) => Math.round((b / 1048576) * 10) / 10;
  const app = tree.filter((p) => !/^msedgewebview2/i.test(p.name));
  const web = tree.filter((p) => /^msedgewebview2/i.test(p.name));
  const sum = (l, k) => l.reduce((s, p) => s + (p[k] ?? 0), 0);
  return {
    processes: tree.length,
    privateMB: mb(sum(tree, 'priv')),
    workingSetMB: mb(sum(tree, 'ws')),
    appPrivateMB: mb(sum(app, 'priv')),
    webviewPrivateMB: mb(sum(web, 'priv')),
  };
}

/** Processes started from `exe` (not other copies of the app running on the machine). */
function findOurs() {
  return processTable().filter(
    (p) => p.path && resolve(p.path).toLowerCase() === exe.toLowerCase(),
  );
}

function killTree(pid) {
  if (pid) spawnSync('taskkill', ['/pid', String(pid), '/T', '/F'], { stdio: 'ignore' });
}

/** Leftovers of an earlier (interrupted) run of this exe would share its WebView2 profile. */
async function waitNoOtherInstance() {
  for (let i = 0; i < 60; i++) {
    const left = findOurs();
    if (left.length === 0) return;
    if (i === 0) console.log(`waiting for ${left.length} earlier ${exe} process(es) to exit…`);
    await sleep(1000);
  }
  throw new Error(`${exe} keeps running: stop it first`);
}

// ---------------------------------------------------------------- perf log

function readLog(file) {
  if (!existsSync(file)) return [];
  return readFileSync(file, 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return null;
      }
    })
    .filter(Boolean);
}

const at = (log, event) => {
  const e = log.find((x) => x.event === event);
  return e ? (e.at ?? e.t) : null;
};

async function waitLog(file, event, timeout = 90_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    const log = readLog(file);
    if (log.some((e) => e.event === event)) return log;
    await sleep(50);
  }
  throw new Error(`timed out waiting for ${event} in ${file}`);
}

/** Starts the exe with `file`, waits for `until`, measures memory, stops it. */
async function launch(file, until) {
  await waitNoOtherInstance();
  const log = join(tmpdir(), `perf-desktop-${process.pid}-${Date.now()}.jsonl`);
  const t0 = Date.now();
  const child = spawn(exe, file ? [file] : [], {
    // A measured run is its own instance (src-tauri/src/perf.rs) with its own WebView2
    // profile, kept between runs (warm start), so another copy of the app on the machine
    // neither takes the launch nor shares its browser process.
    env: { ...process.env, BLUEOFFICE_PERF_LOG: log, WEBVIEW2_USER_DATA_FOLDER: profileDir },
    stdio: 'ignore',
  });
  try {
    try {
      await waitLog(log, until, 45_000);
      if (file) await waitLog(log, 'cache-isolated', 10_000);
    } catch (e) {
      // A launch that never got there is reported (and left out), not fatal for the others.
      const seen = readLog(log).map((x) => x.event);
      console.log(`launch ${file ?? '(no file)'}: ${e.message}; seen: ${seen.join(', ')}`);
      return { t0, events: [], mem: null, failed: true };
    }
    await sleep(3000); // settle: background work after the first frame
    const mem = treeMemory(child.pid);
    const events = readLog(log);
    return { t0, events, mem };
  } finally {
    killTree(child.pid);
    // The WebView2 browser process lets go of the profile folder a moment later.
    await sleep(3000);
    rmSync(log, { force: true });
  }
}

const since = (events, event, t0) => {
  const t = at(events, event);
  return t == null ? null : t - t0;
};

async function startRuns() {
  const out = { start: [], startScreen: [], open: [] };
  // One discarded launch of each kind first (disk cache, WebView2 profile, font index cache).
  for (let i = 0; i <= runs; i++) {
    const small = await launch(smallDoc, 'web:editor-ready');
    const sc = await launch(null, 'web:start-screen');
    const b = await launch(bigDoc, 'web:editor-ready');
    if (i === 0) continue;
    const ev = small.events;
    out.start.push({
      processMainMs: since(ev, 'process-main', small.t0),
      setupMs: since(ev, 'setup', small.t0),
      pageLoadStartedMs: since(ev, 'page-load-started', small.t0),
      navStartMs: since(ev, 'web:nav-start', small.t0),
      firstPaintMs: since(ev, 'web:first-contentful-paint', small.t0),
      shellVisibleMs: since(ev, 'web:shell-visible', small.t0),
      ribbonVisibleMs: since(ev, 'web:ribbon-visible', small.t0),
      pagePresentedMs: since(ev, 'web:page-presented', small.t0),
      editorReadyMs: since(ev, 'web:editor-ready', small.t0),
      systemFontsReadyMs: since(ev, 'system-fonts-ready', small.t0),
      privateMB: small.mem?.privateMB ?? null,
    });
    out.startScreen.push({
      shellVisibleMs: since(sc.events, 'web:shell-visible', sc.t0),
      startScreenMs: since(sc.events, 'web:start-screen', sc.t0),
      privateMB: sc.mem?.privateMB ?? null,
    });
    const be = b.events;
    const readStart = at(be, 'web:ipc:read_file') ?? at(be, 'web:ipc:engine_open');
    const presented = at(be, 'web:page-presented');
    const engineOpen = be.find((e) => e.event === 'engine' && e.cmd === 'engine_open');
    const openIpc = be.find((e) => e.event === 'web:ipc:engine_open');
    const readIpc = be.find((e) => e.event === 'web:ipc:read_file');
    const firstPages = be.filter((e) => e.event === 'engine' && e.cmd === 'engine_page');
    out.open.push({
      openToFirstPageMs: presented != null && readStart != null ? presented - readStart : null,
      launchToFirstPageMs: since(be, 'web:page-presented', b.t0),
      launchToEditorReadyMs: since(be, 'web:editor-ready', b.t0),
      readFileIpcMs: readIpc?.ms ?? null,
      engineOpenIpcMs: openIpc?.ms ?? null,
      engineOpenNativeMs: engineOpen?.ms ?? null,
      firstPageNativeMs: firstPages[0]?.ms ?? null,
      privateMB: b.mem?.privateMB ?? null,
      workingSetMB: b.mem?.workingSetMB ?? null,
      appPrivateMB: b.mem?.appPrivateMB ?? null,
      webviewPrivateMB: b.mem?.webviewPrivateMB ?? null,
      processes: b.mem?.processes ?? null,
    });
    console.log(
      `run ${i}: start ${r1(out.start.at(-1).editorReadyMs)} ms, open ${r1(out.open.at(-1).openToFirstPageMs)} ms, RAM ${out.open.at(-1).privateMB} MB`,
    );
  }
  return out;
}

// ---------------------------------------------------------------- WebDriver runs

let base = `http://127.0.0.1:${port}`;
let session = null;

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
const exec = (script, a = []) => wd('POST', s('/execute/sync'), { script, args: a });
const execAsync = (script, a = []) => wd('POST', s('/execute/async'), { script, args: a });

async function waitFor(fn, what, timeout = 90_000) {
  const end = Date.now() + timeout;
  let last;
  while (Date.now() < end) {
    try {
      last = await fn();
      if (last) return last;
    } catch (e) {
      last = e;
    }
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${what}: ${last}`);
}

async function startDriver() {
  const a = ['--port', String(port)];
  if (process.env.MSEDGEDRIVER) a.push('--native-driver', process.env.MSEDGEDRIVER);
  const child = spawn('tauri-driver', a, { stdio: ['ignore', 'ignore', 'ignore'] });
  await waitFor(async () => (await fetch(`${base}/status`)).ok, 'tauri-driver', 20_000);
  return child;
}

const KEYS = { End: '', Home: '', Control: '', Enter: '' };
async function press(key, mods = []) {
  const v = KEYS[key] ?? key;
  await wd('POST', s('/actions'), {
    actions: [
      {
        type: 'key',
        id: 'kb',
        actions: [
          ...mods.map((m) => ({ type: 'keyDown', value: KEYS[m] })),
          { type: 'keyDown', value: v },
          { type: 'keyUp', value: v },
          ...mods.map((m) => ({ type: 'keyUp', value: KEYS[m] })),
        ],
      },
    ],
  });
  await wd('DELETE', s('/actions')).catch(() => {});
}

// In-page: the key press at/after `t0` → page painted → frame presented; what happened between.
const MEASURE_KEY = `
const [t0, done] = [arguments[0], arguments[arguments.length - 1]];
const P = window.__boPerf;
const afterFrame = (fn) => requestAnimationFrame(() => { const c = new MessageChannel();
  c.port1.onmessage = () => fn(performance.now()); c.port2.postMessage(0); });
const limit = performance.now() + 5000;
const tick = () => {
  const input = P.inputs.find((i) => i.type === 'keydown' && i.seen >= t0);
  const paint = input && P.paints.find((p) => p.start >= input.t && p.end > 0);
  if (paint) return afterFrame((present) => setTimeout(() => {
    const ipc = P.ipc.filter((r) => r.start >= input.t && r.start <= present)
      .map((r) => ({ cmd: r.cmd, start: r.start - input.t, end: (r.end || r.response) - input.t }));
    const paints = P.paints.filter((p) => p.start >= input.t && p.start <= present)
      .map((p) => ({ start: p.start - input.t, end: p.end - input.t }));
    const commits = P.commits.filter((c) => c.t >= input.t && c.t <= present + 100)
      .map((c) => ({ byRegion: c.byRegion, names: c.names }));
    done({ latency: present - input.t, inputEpoch: performance.timeOrigin + input.t,
      presentEpoch: performance.timeOrigin + present, ipc, paints, commits });
  }, 0));
  if (performance.now() > limit) return done({ latency: null });
  requestAnimationFrame(tick);
};
tick();`;

const MEASURE_SCROLL = `
const [px, ms, done] = [arguments[0], arguments[1], arguments[arguments.length - 1]];
const sc = document.querySelector('.bo-doc-scroller');
if (!sc) return done({ error: 'no scroller' });
sc.scrollTop = 0;
setTimeout(() => {
  const P = window.__boPerf; const p0 = P.paints.length;
  const frames = []; let blank = 0; let last = performance.now(); const end = last + ms;
  const pages = [...document.querySelectorAll('.bo-page')];
  const step = (t) => {
    frames.push(t - last); last = t;
    const top = sc.getBoundingClientRect().top, bottom = top + sc.clientHeight;
    if (pages.some((p) => { const r = p.getBoundingClientRect();
        return r.bottom > top && r.top < bottom && !p.querySelector('canvas'); })) blank++;
    sc.scrollTop += px;
    if (t < end) requestAnimationFrame(step);
    else done({ frames: frames.slice(1), blank, paints: P.paints.slice(p0).map((p) => p.end - p.start),
      scrolled: sc.scrollTop, pageHeight: pages[0]?.offsetHeight ?? 0 });
  };
  requestAnimationFrame((t) => { last = t; requestAnimationFrame(step); });
}, 800);`;

async function webdriverRun(log) {
  const created = await wd('POST', '/session', {
    capabilities: {
      alwaysMatch: { browserName: 'wry', 'tauri:options': { application: exe, args: [bigDoc] } },
    },
  });
  session = created.sessionId;
  await wd('POST', s('/timeouts'), { script: 60_000 });
  await waitFor(
    async () => (await exec('return window.__boPerf?.marks["editor-ready"] > 0;')) || null,
    'editor ready',
  );
  await waitLog(log, 'cache-isolated', 10000);
  const view = await exec(`
    const c = document.querySelector('.bo-page canvas');
    return { pages: document.querySelectorAll('.bo-page').length, dpr: window.devicePixelRatio,
      backing: c ? c.width / c.getBoundingClientRect().width : null,
      timeOrigin: performance.timeOrigin,
      // WASM modules the web view fetched (the engine must be the native one: none).
      wasm: performance.getEntriesByType('resource').map((e) => e.name).filter((n) => n.includes('.wasm')),
      engine: document.querySelector('[data-testid=status-engine]')?.textContent ?? null };`);
  // Caret at the end of the first line of the 100-page document, IME input focused.
  await exec('document.querySelector("textarea")?.focus();');
  await press('End');
  await sleep(300);
  const keys = 'chungtoivietbaocaohangthang';
  for (const k of 'abcde') {
    const t0 = await exec('return performance.now();');
    await press(k);
    await execAsync(MEASURE_KEY, [t0]);
  }
  const typing = [];
  for (const k of keys) {
    const t0 = await exec('return performance.now();');
    await press(k);
    typing.push(await execAsync(MEASURE_KEY, [t0]));
  }
  // React renders per keystroke (separate pass: walking the fiber tree costs time itself).
  await exec('window.__boPerf.countRenders = true;');
  const renders = [];
  for (const k of 'renderpass') {
    const t0 = await exec('return performance.now();');
    await press(k);
    await sleep(150); // late commits (status bar word count, …)
    renders.push(await execAsync(MEASURE_KEY, [t0]));
  }
  await exec('window.__boPerf.countRenders = false;');
  const scroll = await execAsync(MEASURE_SCROLL, [40, 5000]);
  const pid = findOurs()[0]?.pid;
  const mem = pid ? treeMemory(pid) : null;
  await wd('DELETE', s('')).catch(() => {});
  session = null;
  await sleep(1500);
  return { view, typing, renders, scroll, mem, events: readLog(log) };
}

function breakdown(k, events) {
  const first = (re) => k.ipc.find((r) => re.test(r.cmd));
  const cmd = first(/^engine_command$/);
  const page = first(/^engine_page$/);
  const glyphs = k.ipc.filter((r) => r.cmd === 'engine_glyph_outlines');
  const paint = k.paints[0];
  // Native time of the IPC call `r`: the engine event that ended while it was in flight.
  const rust = (name, r) => {
    if (!r) return null;
    const from = k.inputEpoch + r.start;
    const to = k.inputEpoch + r.end + 1;
    const e = events.find(
      (x) => x.event === 'engine' && x.cmd === name && x.t >= from && x.t <= to,
    );
    return e ? e.ms : null;
  };
  const layout = cmd
    ? events.find(
        (e) =>
          e.event === 'layout' &&
          e.t >= k.inputEpoch + cmd.start &&
          e.t <= k.inputEpoch + cmd.end + 1,
      )
    : null;
  const allNative = events
    .filter((e) => e.event === 'engine' && e.t >= k.inputEpoch && e.t <= k.presentEpoch)
    .reduce((s, e) => s + e.ms, 0);
  return {
    latency: k.latency,
    beforeCommandIpc: cmd ? cmd.start : null,
    commandIpc: cmd ? cmd.end - cmd.start : null,
    commandNative: rust('engine_command', cmd),
    layoutNative: layout?.ms ?? null,
    commandToPageRequest: cmd && page ? page.start - cmd.end : null,
    pageIpc: page ? page.end - page.start : null,
    pageNative: rust('engine_page', page),
    allNativeInWindow: allNative,
    ipcCalls: k.ipc.length,
    pageCalls: k.ipc.filter((r) => r.cmd === 'engine_page').length,
    glyphIpc: glyphs.reduce((s, g) => s + (g.end - g.start), 0),
    pageToDraw: page && paint ? paint.start - page.end : null,
    draw: paint ? paint.end - paint.start : null,
    paints: k.paints.length,
    drawToPresent: paint ? k.latency - paint.end : null,
  };
}

async function interactionRuns() {
  const out = [];
  const log = join(tmpdir(), `perf-desktop-wd-${process.pid}.jsonl`);
  process.env.BLUEOFFICE_PERF_LOG = log;
  const driver = await startDriver();
  try {
    for (let i = 0; i < runs; i++) {
      await waitNoOtherInstance();
      rmSync(log, { force: true });
      const r = await webdriverRun(log);
      const keys = r.typing.filter((k) => k.latency != null).map((k) => breakdown(k, r.events));
      const field = (f) => keys.map((k) => k[f]);
      const regions = {};
      for (const k of r.renders)
        for (const c of k.commits ?? [])
          for (const [reg, n] of Object.entries(c.byRegion)) regions[reg] = (regions[reg] ?? 0) + n;
      const perKey = Object.fromEntries(
        Object.entries(regions).map(([reg, n]) => [reg, r1(n / Math.max(1, r.renders.length))]),
      );
      const names = new Map();
      for (const k of r.renders)
        for (const c of k.commits ?? [])
          for (const n of c.names) names.set(n, (names.get(n) ?? 0) + 1);
      const fr = r.scroll.frames ?? [];
      const total = fr.reduce((a, b) => a + b, 0);
      out.push({
        view: r.view,
        keySamplesMs: field('latency'),
        keyBreakdownSamples: keys,
        keyToPaintMs: { median: median(field('latency')), p95: pct(field('latency'), 0.95) },
        keyBreakdownMs: Object.fromEntries(
          [
            'beforeCommandIpc',
            'commandIpc',
            'commandNative',
            'layoutNative',
            'commandToPageRequest',
            'pageIpc',
            'pageNative',
            'allNativeInWindow',
            'ipcCalls',
            'pageCalls',
            'glyphIpc',
            'pageToDraw',
            'draw',
            'paints',
            'drawToPresent',
          ].map((f) => [f, r1(median(field(f)))]),
        ),
        rendersPerKey: perKey,
        commitsPerKey: r1(
          r.renders.reduce((s, k) => s + (k.commits?.length ?? 0), 0) /
            Math.max(1, r.renders.length),
        ),
        topRendered: [...names.entries()].sort((a, b) => b[1] - a[1]).slice(0, 15),
        scroll: {
          fps: total ? (fr.length / total) * 1000 : null,
          frameMedianMs: median(fr),
          frameP95Ms: pct(fr, 0.95),
          longFrames: fr.filter((f) => f > 20).length,
          frames: fr.length,
          blankFrames: r.scroll.blank,
          drawMedianMs: median(r.scroll.paints ?? []),
          pagesScrolled: r.scroll.pageHeight ? r.scroll.scrolled / r.scroll.pageHeight : null,
        },
        mem: r.mem,
      });
      const last = out.at(-1);
      console.log(
        `webdriver run ${i + 1}: key→paint ${r1(last.keyToPaintMs.median)} ms (p95 ${r1(last.keyToPaintMs.p95)}), scroll ${r1(last.scroll.fps)} fps, renders/key ${JSON.stringify(perKey)}`,
      );
    }
  } finally {
    if (session) await wd('DELETE', s('')).catch(() => {});
    killTree(driver.pid);
    rmSync(log, { force: true });
  }
  return out;
}

async function webviewHeapSnapshot(capabilities, file) {
  const address =
    capabilities?.['ms:edgeOptions']?.debuggerAddress ??
    capabilities?.['goog:chromeOptions']?.debuggerAddress;
  if (!address) throw new Error('WebDriver did not expose a WebView2 debugger address');
  const currentUrl = await exec('return location.href');
  const targets = await (await fetch(`http://${address}/json/list`)).json();
  const target = targets.find((entry) => entry.type === 'page' && entry.url === currentUrl);
  if (!target?.webSocketDebuggerUrl)
    throw new Error('could not identify the active WebView2 debugging target');
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  const chunks = [];
  await new Promise((resolveSnapshot, rejectSnapshot) => {
    const timer = setTimeout(
      () => rejectSnapshot(new Error('WebView2 heap snapshot timed out')),
      120000,
    );
    socket.addEventListener('open', () =>
      socket.send(
        JSON.stringify({
          id: 1,
          method: 'HeapProfiler.takeHeapSnapshot',
          params: { reportProgress: false },
        }),
      ),
    );
    socket.addEventListener('error', () => {
      clearTimeout(timer);
      rejectSnapshot(new Error('WebView2 debugger connection failed'));
    });
    socket.addEventListener('message', (event) => {
      const data = JSON.parse(String(event.data));
      if (data.method === 'HeapProfiler.addHeapSnapshotChunk') chunks.push(data.params.chunk);
      if (data.id === 1) {
        clearTimeout(timer);
        if (data.error) rejectSnapshot(new Error(data.error.message));
        else resolveSnapshot();
      }
    });
  }).finally(() => socket.close());
  writeFileSync(file, chunks.join(''));
  return { file, bytes: chunks.reduce((sum, chunk) => sum + Buffer.byteLength(chunk), 0) };
}

/** Exercise editing, scrolling, clipboard images and undo in one document over time. */
async function memorySoak() {
  const duration = Number(opt('soak-ms', '600000'));
  if (!Number.isFinite(duration) || duration <= 0) throw new Error('invalid --soak-ms');
  const log = join(tmpdir(), `perf-desktop-soak-${process.pid}.jsonl`);
  process.env.BLUEOFFICE_PERF_LOG = log;
  const driver = await startDriver();
  const samples = [];
  // A deterministic 256×256 image exercises decoding and undo more realistically than a
  // one-pixel placeholder, without reading the user's files or using external resources.
  const picture = new PNG({ width: 256, height: 256 });
  for (let i = 0; i < picture.data.length; i += 4) {
    const pixel = i / 4;
    picture.data[i] = pixel % 256;
    picture.data[i + 1] = Math.floor(pixel / 256);
    picture.data[i + 2] = (pixel * 31) % 256;
    picture.data[i + 3] = 255;
  }
  const imageBase64 = PNG.sync.write(picture).toString('base64');
  let images = 0;
  let edits = 0;
  try {
    await waitNoOtherInstance();
    const created = await wd('POST', '/session', {
      capabilities: {
        alwaysMatch: { browserName: 'wry', 'tauri:options': { application: exe, args: [bigDoc] } },
      },
    });
    session = created.sessionId;
    await waitFor(() => exec('return window.__boPerf?.marks["editor-ready"] > 0'), 'editor ready');
    await waitLog(log, 'cache-isolated', 10000);
    await exec(`const original=window.fetch; window.__boSoakImageStatuses=[];
      window.fetch=function(input,...args){const url=typeof input==='string'?input:input?.url;
        return original.call(this,input,...args).then(response=>{
          if(url?.includes('/engine_insert_image')) window.__boSoakImageStatuses.push(response.status);
          return response;
        });};`);
    const snapshots = [];
    if (flag('webview-snapshots'))
      snapshots.push(
        await webviewHeapSnapshot(
          created.capabilities,
          join(outDir, 'webview-before.heapsnapshot'),
        ),
      );
    await exec('document.querySelector("textarea")?.focus()');
    const pid = findOurs()[0]?.pid;
    if (!pid) throw new Error('desktop process not found for memory sampling');
    const started = Date.now();
    let nextSample = started;
    let nextImage = started + 10000;
    while (Date.now() - started < duration) {
      await press('a');
      edits++;
      await exec(`const s=document.querySelector('.bo-doc-scroller');
        if(s){const next=s.scrollTop+450; s.scrollTop=next>=s.scrollHeight-s.clientHeight?0:next;}`);
      const now = Date.now();
      if (now >= nextImage) {
        const count = await exec(
          'return window.__boPerf.ipc.filter(x=>x.cmd==="engine_insert_image").length',
        );
        await exec(
          `const bytes=Uint8Array.from(atob(arguments[0]),c=>c.charCodeAt(0));
          const d=new DataTransfer(); d.items.add(new File([bytes], 'soak.png', {type:'image/png'}));
          document.querySelector('textarea').dispatchEvent(new ClipboardEvent('paste',{clipboardData:d,bubbles:true,cancelable:true}));`,
          [imageBase64],
        );
        await waitFor(
          async () =>
            await exec(
              'const a=window.__boPerf.ipc.filter(x=>x.cmd==="engine_insert_image"); return a.length>arguments[0] && a.at(-1).end>0;',
              [count],
            ),
          'completed pasted image IPC',
          10000,
        );
        const imageStatus = await exec('return window.__boSoakImageStatuses.at(-1)');
        if (!(imageStatus >= 200 && imageStatus < 300))
          throw new Error(`image insertion failed with HTTP status ${imageStatus}`);
        await sleep(500);
        await press('z', ['Control']);
        images++;
        nextImage = Date.now() + 10000;
      }
      if (now >= nextSample) {
        const mem = treeMemory(pid);
        if (!Number.isFinite(mem?.privateMB)) throw new Error('memory sample unavailable');
        samples.push({ elapsedMs: now - started, privateMB: mem.privateMB });
        console.log(
          `soak ${(now - started) / 1000}s: ${mem.privateMB} MB, ${edits} edits, ${images} images`,
        );
        nextSample = now + 30000;
      }
      await sleep(1000);
    }
    const final = treeMemory(pid);
    if (!Number.isFinite(final?.privateMB)) throw new Error('final memory sample unavailable');
    samples.push({ elapsedMs: Date.now() - started, privateMB: final.privateMB });
    const tail = samples.slice(Math.floor(samples.length / 3));
    const elapsedMinutes = (tail.at(-1).elapsedMs - tail[0].elapsedMs) / 60000;
    const growthMB = tail.at(-1).privateMB - tail[0].privateMB;
    const growing = elapsedMinutes > 0 && growthMB > 32 && growthMB / elapsedMinutes > 1;
    if (flag('webview-snapshots'))
      snapshots.push(
        await webviewHeapSnapshot(created.capabilities, join(outDir, 'webview-after.heapsnapshot')),
      );
    return {
      durationMs: Date.now() - started,
      edits,
      images,
      samples,
      growthMB: r1(growthMB),
      growing,
      peakPrivateMB: Math.max(...samples.map((sample) => sample.privateMB)),
      snapshots,
    };
  } finally {
    if (session) await wd('DELETE', s('')).catch(() => {});
    killTree(driver.pid);
    rmSync(log, { force: true });
  }
}

// ---------------------------------------------------------------- report

function med(list, f) {
  return r1(median(list.map(f)));
}

function summarize(st, wdRuns) {
  const m = {
    'start.editorReadyMs': med(st.start, (x) => x.editorReadyMs),
    'start.shellVisibleMs': med(st.start, (x) => x.shellVisibleMs),
    'start.firstPaintMs': med(st.start, (x) => x.firstPaintMs),
    'start.ribbonVisibleMs': med(st.start, (x) => x.ribbonVisibleMs),
    'start.processMainMs': med(st.start, (x) => x.processMainMs),
    'start.pageLoadStartedMs': med(st.start, (x) => x.pageLoadStartedMs),
    'start.startScreenMs': med(st.startScreen, (x) => x.startScreenMs),
    'open100.openToFirstPageMs': med(st.open, (x) => x.openToFirstPageMs),
    'open100.launchToFirstPageMs': med(st.open, (x) => x.launchToFirstPageMs),
    'open100.engineOpenNativeMs': med(st.open, (x) => x.engineOpenNativeMs),
    'open100.engineOpenIpcMs': med(st.open, (x) => x.engineOpenIpcMs),
    'open100.readFileIpcMs': med(st.open, (x) => x.readFileIpcMs),
    'ram.oneDocPrivateMB': med(st.open, (x) => x.privateMB),
    'ram.oneDocWorkingSetMB': med(st.open, (x) => x.workingSetMB),
    'ram.appPrivateMB': med(st.open, (x) => x.appPrivateMB),
    'ram.webviewPrivateMB': med(st.open, (x) => x.webviewPrivateMB),
    'ram.smallDocPrivateMB': med(st.start, (x) => x.privateMB),
  };
  if (wdRuns?.length) {
    Object.assign(m, {
      'type.keyToPaintMedianMs': med(wdRuns, (x) => x.keyToPaintMs.median),
      'type.keyToPaintP95Ms': med(wdRuns, (x) => x.keyToPaintMs.p95),
      'type.keyToPaintPooledP95Ms': r1(
        pct(
          wdRuns.flatMap((x) => x.keySamplesMs),
          0.95,
        ),
      ),
      'scroll.fps': med(wdRuns, (x) => x.scroll.fps),
      'scroll.frameP95Ms': med(wdRuns, (x) => x.scroll.frameP95Ms),
      'scroll.blankFrames': med(wdRuns, (x) => x.scroll.blankFrames),
      'ram.afterEditingPrivateMB': med(wdRuns, (x) => x.mem?.privateMB),
      'renders.ribbonPerKey': med(wdRuns, (x) => x.rendersPerKey.ribbon ?? 0),
      'renders.statusPerKey': med(wdRuns, (x) => x.rendersPerKey.status ?? 0),
      'renders.titlePerKey': med(wdRuns, (x) => x.rendersPerKey.title ?? 0),
      'renders.panePerKey': med(wdRuns, (x) => x.rendersPerKey.pane ?? 0),
      'renders.otherPerKey': med(wdRuns, (x) => x.rendersPerKey.other ?? 0),
    });
    for (const f of Object.keys(wdRuns[0].keyBreakdownMs))
      m[`typeBreakdown.${f}`] = med(wdRuns, (x) => x.keyBreakdownMs[f]);
  }
  return m;
}

// Percentiles are computed over runs (and, for key latency, over all measured keystrokes).
// Keep the count so an incomplete run cannot be mistaken for ten observations.
function distributions(st, wdRuns, metrics) {
  const samples = Object.fromEntries(Object.keys(metrics).map((key) => [key, []]));
  const count = Math.max(st.start.length, st.startScreen.length, st.open.length, wdRuns.length);
  for (let i = 0; i < count; i++) {
    const one = summarize(
      {
        start: st.start[i] ? [st.start[i]] : [],
        startScreen: st.startScreen[i] ? [st.startScreen[i]] : [],
        open: st.open[i] ? [st.open[i]] : [],
      },
      wdRuns[i] ? [wdRuns[i]] : [],
    );
    for (const [key, value] of Object.entries(one))
      if (Number.isFinite(value) && samples[key]) samples[key].push(value);
  }
  const stats = (values) => ({
    n: values.length,
    median: r1(median(values)),
    p95: r1(pct(values, 0.95)),
  });
  const result = Object.fromEntries(
    Object.entries(samples).map(([key, values]) => [key, stats(values)]),
  );
  result['type.keyToPaintAllKeysMs'] = stats(wdRuns.flatMap((run) => run.keySamplesMs));
  for (const key of Object.keys(wdRuns[0]?.keyBreakdownMs ?? {}))
    result[`typeBreakdownAllKeys.${key}`] = stats(
      wdRuns.flatMap((run) =>
        run.keyBreakdownSamples.map((sample) => sample[key]).filter(Number.isFinite),
      ),
    );
  return result;
}

// Lower is better except these.
const HIGHER_IS_BETTER = new Set(['scroll.fps']);
// Compared with the baseline (the rest is informative).
const GATED = [
  'start.editorReadyMs',
  'start.shellVisibleMs',
  'open100.openToFirstPageMs',
  'open100.engineOpenNativeMs',
  'ram.oneDocPrivateMB',
  'type.keyToPaintMedianMs',
  'type.keyToPaintP95Ms',
  'type.keyToPaintPooledP95Ms',
  'scroll.fps',
  'renders.ribbonPerKey',
  'renders.statusPerKey',
];
// Differences smaller than this are noise whatever the percentage.
const FLOOR = { ms: 5, MB: 10, fps: 2, renders: 0.5 };
const floorOf = (k) =>
  k.startsWith('renders.')
    ? FLOOR.renders
    : k.endsWith('MB')
      ? FLOOR.MB
      : k === 'scroll.fps'
        ? FLOOR.fps
        : FLOOR.ms;

function compare(metrics, baseline) {
  const rows = [];
  let fail = false;
  for (const k of GATED) {
    const cur = metrics[k];
    const old = baseline?.metrics?.[k];
    if (cur == null || old == null) {
      rows.push({ k, cur, old, status: 'n/a' });
      continue;
    }
    const worse = HIGHER_IS_BETTER.has(k) ? old - cur : cur - old;
    const rel = old === 0 ? (worse > 0 ? Infinity : 0) : (worse / Math.abs(old)) * 100;
    let status = 'ok';
    if (worse > floorOf(k) && rel > failOver) {
      status = 'FAIL';
      fail = true;
    } else if (worse > floorOf(k) && rel > warnOver) status = 'warn';
    rows.push({ k, cur, old, rel: r1(rel), status });
  }
  return { rows, fail };
}

const TARGETS = [
  ['App start → editor ready (small doc)', 'start.editorReadyMs', '< 1 500 ms', (v) => v < 1500],
  ['Window shell visible', 'start.shellVisibleMs', '< 500 ms', (v) => v < 500],
  ['Open 100-page .docx → first page', 'open100.openToFirstPageMs', '< 2 000 ms', (v) => v < 2000],
  ['Key press → glyph painted (median)', 'type.keyToPaintMedianMs', '< 16 ms', (v) => v < 16],
  [
    'Key press → glyph painted (pooled p95)',
    'type.keyToPaintPooledP95Ms',
    '< 20 ms',
    (v) => v < 20,
  ],
  ['Scrolling', 'scroll.fps', '60 FPS', (v) => v >= 58],
  [
    'RAM, one document (private WS, app + WebView2)',
    'ram.oneDocPrivateMB',
    '< 400 MB',
    (v) => v < 400,
  ],
  ['Ribbon renders per keystroke', 'renders.ribbonPerKey', '0', (v) => v === 0],
];

function markdown(result, cmp) {
  const m = result.metrics;
  const L = [];
  L.push(`# Desktop performance — ${result.date}`, '');
  L.push(
    `Exe: \`${result.exe}\` · runs: ${result.runs} (median) · profile: ${result.profile || '—'}`,
  );
  const mc = result.machine;
  L.push(
    `Machine: ${mc.cpu} (${mc.cores}C/${mc.threads}T), ${mc.ramGB} GB RAM, GPU ${[].concat(mc.gpu).join('; ')}, ${mc.os}, scale ${mc.scalePercent} %, WebView2 ${mc.webView2}; CPU load before/after ${result.loadBefore} / ${result.loadAfter} %`,
    '',
  );
  L.push('| Target | Value | Goal | Met |', '|---|---|---|---|');
  for (const [name, k, goal, ok] of TARGETS)
    L.push(
      `| ${name} | ${m[k] ?? 'n/a'} | ${goal} | ${m[k] == null ? '—' : ok(m[k]) ? 'yes' : 'no'} |`,
    );
  const v = result.webdriver?.[0]?.view;
  if (v)
    L.push(
      '',
      `Web view: ${v.pages} pages, engine status "${v.engine ?? '—'}", WASM fetched: ${v.wasm?.length ? v.wasm.join(', ') : 'none'}, devicePixelRatio ${v.dpr}, page canvas backing / CSS px ${v.backing == null ? '—' : r1(v.backing * 100) / 100}`,
    );
  L.push('', '## Distributions', '', '| Metric | N | Median | p95 |', '|---|---:|---:|---:|');
  for (const [k, v] of Object.entries(result.distributions))
    L.push(`| \`${k}\` | ${v.n} | ${v.median ?? 'n/a'} | ${v.p95 ?? 'n/a'} |`);
  if (cmp) {
    L.push('', `## Against the baseline (warn > ${warnOver} %, fail > ${failOver} %)`, '');
    L.push('| Metric | Baseline | Now | Worse by | Status |', '|---|---|---|---|---|');
    for (const r of cmp.rows)
      L.push(
        `| \`${r.k}\` | ${r.old ?? '—'} | ${r.cur ?? '—'} | ${r.rel ?? '—'} % | ${r.status} |`,
      );
  }
  if (result.webdriver?.[0]?.topRendered?.length) {
    L.push('', '## Components rendered during 10 keystrokes (region:name × count)', '');
    L.push(result.webdriver[0].topRendered.map(([n, c]) => `\`${n}\` × ${c}`).join(', '));
  }
  return L.join('\n') + '\n';
}

// ---------------------------------------------------------------- main

const loadBefore = cpuLoad();
const mach = machine();
ensureBigDoc();
console.log(`perf-desktop: ${exe}, ${runs} runs, ${mach.cpu}, load ${loadBefore} %`);
const st =
  flag('only-webdriver') || flag('soak-only')
    ? { start: [], startScreen: [], open: [] }
    : await startRuns();
const wdRuns = useWebDriver && !flag('soak-only') ? await interactionRuns() : [];
const soak = flag('soak-only') ? await memorySoak() : null;
const result = {
  date: new Date().toISOString(),
  exe,
  exeSha256: createHash('sha256').update(readFileSync(exe)).digest('hex'),
  runs,
  profile,
  machine: { ...mach, nodeCpus: cpus().length, totalMemGB: Math.round(totalmem() / 2 ** 30) },
  loadBefore,
  loadAfter: cpuLoad(),
  metrics: summarize(st, wdRuns),
  raw: st,
  webdriver: wdRuns,
  soak,
};
result.distributions = distributions(st, wdRuns, result.metrics);
let cmp = null;
if (baselinePath && existsSync(baselinePath) && !flag('update-baseline')) {
  cmp = compare(result.metrics, JSON.parse(readFileSync(baselinePath, 'utf8')));
} else if (baselinePath && !flag('update-baseline')) {
  console.log(`no baseline at ${baselinePath}: report only (commit desktop-perf.json there).`);
}
writeFileSync(join(outDir, 'desktop-perf.json'), JSON.stringify(result, null, 2));
writeFileSync(join(outDir, 'desktop-perf.md'), markdown(result, cmp));
if (flag('update-baseline') && baselinePath) {
  mkdirSync(dirname(baselinePath), { recursive: true });
  const slim = { ...result };
  delete slim.raw;
  delete slim.webdriver;
  writeFileSync(baselinePath, JSON.stringify(slim, null, 2) + '\n');
  console.log(`baseline written: ${baselinePath}`);
}
console.log(readFileSync(join(outDir, 'desktop-perf.md'), 'utf8'));
if (process.env.GITHUB_STEP_SUMMARY)
  writeFileSync(process.env.GITHUB_STEP_SUMMARY, markdown(result, cmp), { flag: 'a' });
if (soak) {
  console.log(
    `Soak: ${soak.edits} edits, ${soak.images} image/undo cycles, peak ${soak.peakPrivateMB} MB, tail growth ${soak.growthMB} MB`,
  );
  if (!soak.images || soak.growing || soak.peakPrivateMB >= 400) process.exitCode = 1;
}
// A failed measurement must not look like a successful regression check in CI.
if (process.env.CI && !flag('only-webdriver') && !flag('soak-only') && useWebDriver) {
  const missing = GATED.filter((key) => !Number.isFinite(result.metrics[key]));
  if (missing.length) {
    console.error(`::error::Desktop performance measurements missing: ${missing.join(', ')}`);
    process.exit(1);
  }
}
if (cmp) {
  for (const r of cmp.rows.filter((x) => x.status === 'warn'))
    console.log(`::warning::${r.k} ${r.rel} % slower than the baseline (${r.old} → ${r.cur})`);
  if (cmp.fail) {
    for (const r of cmp.rows.filter((x) => x.status === 'FAIL'))
      console.log(`::error::${r.k} ${r.rel} % slower than the baseline (${r.old} → ${r.cur})`);
    process.exit(1);
  }
}
