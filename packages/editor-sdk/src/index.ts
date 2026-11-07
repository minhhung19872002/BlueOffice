/**
 * `@blueoffice/editor-sdk` — lets hosts (BlueDrive, …) embed and control the
 * BlueOffice editor over `postMessage` (prompt §6A.2).
 *
 * Host:   `BlueOffice.open(container, { fileId, accessToken, host, editorUrl })`
 * Editor: `connectHost({ parent: window.parent, self: window, allowedOrigins })`
 */
export * from './protocol';
export { BlueOffice, connectEditor, editorFrameUrl } from './host';
export type { EditorHandle, EventHandler, OpenOptions } from './host';
export { connectHost } from './embedded';
export type { CommandHandler, ConnectHostOptions, HostConnection } from './embedded';
