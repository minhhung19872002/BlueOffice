// Ports of the collaboration E2E stack (tests/e2e/collab/stack.mjs). Several
// checkouts may run at once: `COLLAB_PORT` sets the first one, default
// `E2E_PORT + 20` (E2E_PORT=4561 → 4581…4584).

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
