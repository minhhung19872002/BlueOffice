// Host page standing in for BlueDrive: embeds the editor with the public SDK.
// `?file=&token=&editor=&user=` (the test gets the token from the mock's /_admin/token).
import { BlueOffice, EDITOR_EVENTS, type EditorHandle } from '@blueoffice/editor-sdk';

declare global {
  interface Window {
    boHost: { editor: EditorHandle; events: { event: string; payload: unknown }[] };
  }
}

const q = new URLSearchParams(window.location.search);
document.getElementById('user')!.textContent = q.get('user') ?? '';
const editor = BlueOffice.open(document.getElementById('editor')!, {
  fileId: q.get('file') ?? '',
  accessToken: q.get('token') ?? '',
  host: 'bluedrive',
  editorUrl: q.get('editor') ?? '',
  lang: 'vi-VN',
  ui: { showShare: true, showClose: true, brandName: 'BlueDrive' },
});
const events: { event: string; payload: unknown }[] = [];
for (const e of EDITOR_EVENTS) editor.on(e, (payload) => events.push({ event: e, payload }));
window.boHost = { editor, events };
