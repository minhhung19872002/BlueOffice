// Host page standing in for BlueDrive: embeds the editor with the public SDK.
// `?file=&token=&editor=&user=` (the test gets the token from the mock's /_admin/token);
// optional `mode=edit|view|comment`, `theme=light|dark|auto`, `brand=`, `chrome=0` (no brand, Share/Close/Back).
import {
  BlueOffice,
  EDITOR_EVENTS,
  type EditorHandle,
  type EditorMode,
  type EditorTheme,
} from '@blueoffice/editor-sdk';

declare global {
  interface Window {
    boHost: { editor: EditorHandle; events: { event: string; payload: unknown }[] };
  }
}

const q = new URLSearchParams(window.location.search);
document.getElementById('user')!.textContent = q.get('user') ?? '';
const chrome = q.get('chrome') !== '0';
const editor = BlueOffice.open(document.getElementById('editor')!, {
  fileId: q.get('file') ?? '',
  accessToken: q.get('token') ?? '',
  host: 'bluedrive',
  editorUrl: q.get('editor') ?? '',
  lang: 'vi-VN',
  mode: (q.get('mode') as EditorMode | null) ?? 'edit',
  theme: (q.get('theme') as EditorTheme | null) ?? 'light',
  ui: chrome
    ? { showShare: true, showClose: true, showBack: true, brandName: q.get('brand') ?? 'BlueDrive' }
    : {},
});
const events: { event: string; payload: unknown }[] = [];
for (const e of EDITOR_EVENTS) editor.on(e, (payload) => events.push({ event: e, payload }));
window.boHost = { editor, events };
