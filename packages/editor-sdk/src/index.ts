/**
 * `@blueoffice/editor-sdk` — public contract for hosts embedding the editor
 * (prompt §6A.2). Phase 1 fixes the types so the editor can be designed
 * against them; the postMessage transport is implemented in Phase 6A.
 */

export type EditorMode = 'edit' | 'view' | 'comment';
export type EditorTheme = 'light' | 'dark' | 'auto';

export interface EditorUiOptions {
  showShare?: boolean;
  showClose?: boolean;
  brandName?: string;
}

export interface OpenOptions {
  fileId: string;
  accessToken: string;
  host: string;
  mode?: EditorMode;
  lang?: 'vi-VN' | 'en-US';
  theme?: EditorTheme;
  ui?: EditorUiOptions;
  /** Allowed editor origin(s); messages from other origins are ignored. */
  editorOrigin?: string;
}

export type EditorEvent = 'saved' | 'dirty' | 'closed' | 'share-requested' | 'error' | 'collaborators-changed';
export type EditorCommand = 'save' | 'print' | 'export-pdf' | 'close';

export interface EditorHandle {
  on(event: EditorEvent, handler: (payload: unknown) => void): () => void;
  command(cmd: EditorCommand): Promise<void>;
  destroy(): void;
}

/** Message envelope exchanged between host and editor iframe. */
export interface BohpMessage {
  source: 'blueoffice';
  version: 1;
  type: 'event' | 'command' | 'response';
  name: string;
  payload?: unknown;
  requestId?: string;
}

export const BlueOffice = {
  open(_container: HTMLElement, _options: OpenOptions): EditorHandle {
    throw new Error('@blueoffice/editor-sdk: not implemented before Phase 6A');
  },
};
