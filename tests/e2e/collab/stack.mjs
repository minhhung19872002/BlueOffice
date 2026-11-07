// Real-server stack for the collaboration E2E (prompt §6A acceptance):
//
//   tests/mock-bluedrive          BOHP host + JWKS + /_admin/*        (mock port)
//   apps/server/BlueOffice.Api    SQLite + filesystem store, in-memory queue (server port)
//   apps/web/dist (vite preview)  /edit, with /api /hosting /collab proxied to the server (editor port)
//   collab/host (vite dev)        host page embedding the editor via @blueoffice/editor-sdk (host port)
//
// Usage: `node tests/e2e/collab/stack.mjs` (needs `pnpm --filter @blueoffice/web build` and the .NET 8 SDK).
// Playwright starts it as the webServer of playwright.collab.config.ts; the desktop E2E reuses it.
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { collabPorts, collabUrls } from './ports.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..', '..');
const ports = collabPorts();
const urls = collabUrls();
const data = mkdtempSync(join(tmpdir(), 'bo-collab-e2e-'));
const children = [];
const verbose = !!process.env.COLLAB_STACK_VERBOSE;

function run(name, cmd, args, env) {
  const child = spawn(cmd, args, {
    cwd: root,
    env: { ...process.env, DOTNET_CLI_TELEMETRY_OPTOUT: '1', DOTNET_NOLOGO: '1', ...env },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const log = (chunk) => {
    const text = chunk.toString();
    // Errors always go through; the rest only with COLLAB_STACK_VERBOSE=1.
    if (verbose || /\b(fail|error|exception|unhandled)\b/i.test(text)) {
      process.stderr.write(text.replace(/^/gm, `[${name}] `));
    }
  };
  child.stdout.on('data', log);
  child.stderr.on('data', log);
  child.on('exit', (code) => {
    if (!stopping) {
      console.error(`[stack] ${name} exited with code ${code}`);
      stop(1);
    }
  });
  children.push(child);
  return child;
}

let stopping = false;
const servers = [];
function stop(code = 0) {
  if (stopping) return;
  stopping = true;
  for (const s of servers) void s.close?.().catch?.(() => {});
  for (const c of children) {
    if (c.exitCode !== null) continue;
    // `dotnet run` starts the app as a child process: stop the whole tree.
    if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(c.pid), '/T', '/F']);
    else c.kill('SIGTERM');
  }
  try {
    rmSync(data, { recursive: true, force: true });
  } catch {
    // SQLite may still hold the file for a moment; it is a temp dir.
  }
  process.exit(code);
}
for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(sig, () => stop(0));

async function waitFor(url, what, timeoutMs = 240_000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`timed out waiting for ${what} (${url})`);
}

// Build both .NET projects first (sequentially: they share obj/ outputs of referenced projects).
for (const project of ['tests/mock-bluedrive', 'apps/server/BlueOffice.Api']) {
  const r = spawnSync('dotnet', ['build', project, '-nologo', '-v', 'q'], {
    cwd: root,
    stdio: verbose ? 'inherit' : ['ignore', 'ignore', 'inherit'],
  });
  if (r.status !== 0) {
    console.error(`[stack] dotnet build ${project} failed`);
    process.exit(1);
  }
}

run(
  'mock',
  'dotnet',
  [
    'run',
    '--no-build',
    '--no-launch-profile',
    '--project',
    'tests/mock-bluedrive',
    '--',
    '--urls',
    urls.mock,
  ],
  { ASPNETCORE_ENVIRONMENT: 'Development', Mock__Issuer: 'http://mock-bluedrive' },
);
run(
  'server',
  'dotnet',
  [
    'run',
    '--no-build',
    '--no-launch-profile',
    '--project',
    'apps/server/BlueOffice.Api',
    '--',
    '--urls',
    urls.server,
  ],
  {
    ASPNETCORE_ENVIRONMENT: 'Development',
    ConnectionStrings__Default: `Data Source=${join(data, 'blueoffice.db')}`,
    Database__Provider: 'Sqlite',
    Storage__Provider: 'FileSystem',
    Storage__Root: join(data, 'objects'),
    Jobs__Provider: 'InMemory',
    Worker__Enabled: 'false',
    Otel__Enabled: 'false',
    Serilog__MinimumLevel__Default: verbose ? 'Information' : 'Warning',
    // Development-only signing key (refused in Production); the E2E only uses host tokens.
    Auth__DevSigningKey: 'collab-e2e-development-signing-key-0123456789',
    Auth__DevIssuer: 'blueoffice-dev',
    // Saves happen when the test asks (host command) or when the last participant leaves.
    Collab__AutosaveInterval: '00:10:00',
    Collab__PermissionRecheckInterval: '00:10:00',
    Collab__TickInterval: '00:00:00.500',
    Hosts__0__Name: 'bluedrive',
    Hosts__0__Protocol: 'Bohp',
    Hosts__0__BaseUrl: `${urls.mock}/bohp/v1`,
    Hosts__0__JwksUrl: `${urls.mock}/.well-known/jwks.json`,
    Hosts__0__Issuer: 'http://mock-bluedrive',
    Hosts__0__Audience: 'blueoffice',
    Hosts__0__WebhookUrl: `${urls.mock}/webhooks/blueoffice`,
    Hosts__0__WebhookSecret: 'mock-webhook-secret',
    Hosts__0__AllowedOrigins__0: urls.host,
    // Desktop web view origins (Windows/Linux, macOS) for the deep-link E2E.
    Cors__Origins__0: 'http://tauri.localhost',
    Cors__Origins__1: 'tauri://localhost',
  },
);

try {
  await waitFor(`${urls.mock}/.well-known/jwks.json`, 'mock BlueDrive');
  await waitFor(`${urls.server}/health`, 'BlueOffice server');
  process.env.BLUEOFFICE_API_PROXY = urls.server;
  const { createServer, preview } = await import('vite');
  servers.push(
    await preview({
      root: join(root, 'apps', 'web'),
      configFile: join(root, 'apps', 'web', 'vite.config.ts'),
      logLevel: verbose ? 'info' : 'warn',
      preview: { port: ports.editor, strictPort: true, host: 'localhost' },
    }),
  );
  const host = await createServer({
    root: join(here, 'host'),
    configFile: false,
    logLevel: verbose ? 'info' : 'warn',
    server: { port: ports.host, strictPort: true, host: 'localhost' },
  });
  await host.listen();
  servers.push(host);
  await waitFor(`${urls.editor}/edit`, 'editor');
  await waitFor(`${urls.host}/`, 'host page');
  console.log(
    `[stack] ready: host ${urls.host} · editor ${urls.editor} · server ${urls.server} · mock ${urls.mock}`,
  );
} catch (e) {
  console.error(`[stack] ${e instanceof Error ? e.message : String(e)}`);
  stop(1);
}
