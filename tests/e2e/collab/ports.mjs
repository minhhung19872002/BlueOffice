// Ports of the collaboration E2E stack (tests/e2e/collab/stack.mjs). Several
// checkouts may run at once: `COLLAB_PORT` sets the first one, default
// `E2E_PORT + 20` (E2E_PORT=4561 → 4581…4584).
import { existsSync, statSync } from 'node:fs';
import { join } from 'node:path';

/** @returns {{ editor: number, host: number, server: number, mock: number }} */
export function collabPorts() {
  const first = Number(process.env.COLLAB_PORT ?? Number(process.env.E2E_PORT ?? 4173) + 20);
  return { editor: first, host: first + 1, server: first + 2, mock: first + 3 };
}

/** Base URLs of the stack. */
export function collabUrls() {
  const p = collabPorts();
  return {
    /** Web shell (`/edit`) with `/api` and `/collab` proxied to the server. */
    editor: `http://localhost:${p.editor}`,
    /** Host page (a BlueDrive stand-in) embedding the editor with `@blueoffice/editor-sdk`. */
    host: `http://localhost:${p.host}`,
    /** BlueOffice server (`apps/server/BlueOffice.Api`). */
    server: `http://localhost:${p.server}`,
    /** `tests/mock-bluedrive` (BOHP, JWKS, `/_admin/*`). */
    mock: `http://localhost:${p.mock}`,
  };
}

/**
 * The built bo-cli the stack runs scheduled scripts with (CORE-SCRIPT-004): `BO_CLI`, else
 * `$CARGO_TARGET_DIR` or `target/` (the most recent of release / debug); null when none.
 * @param {string} root repository root
 * @returns {string | null}
 */
export function findBoCli(root) {
  const exe = process.platform === 'win32' ? 'bo-cli.exe' : 'bo-cli';
  if (process.env.BO_CLI && existsSync(process.env.BO_CLI)) return process.env.BO_CLI;
  const dir = process.env.CARGO_TARGET_DIR || join(root, 'target');
  const built = ['release', 'debug']
    .map((p) => join(dir, p, exe))
    .filter((f) => existsSync(f))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return built[0] ?? null;
}
