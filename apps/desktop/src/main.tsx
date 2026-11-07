import { BlueEditor } from '@blueoffice/editor';
import {
  HostedBanner,
  hostedText,
  openHosted,
  serverBase,
  type FetchLike,
  type HostedApp,
  type HostedEditor,
} from '@blueoffice/hosted';
import type { HostFileLink, Platform } from '@blueoffice/platform';
import { createDesktopPlatform } from '@blueoffice/platform/desktop';
import { BlueSheetEditor } from '@blueoffice/sheet-editor';
import { BlueSlideEditor } from '@blueoffice/slide-editor';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';

// The desktop app is only a shell: same editor, DesktopPlatform (native engine + real disk).
// One shell, three products: the window URL selects the app (see tauri.*.conf.json).
const query = new URLSearchParams(window.location.search);
const requested = query.get('app');
const app: HostedApp = requested === 'sheet' || requested === 'slide' ? requested : 'doc';
const platform = createDesktopPlatform({ storage: (files) => new LocalStorageProvider(files) });
const language = 'vi-VN';
const root = createRoot(document.getElementById('root')!);

function Editor({ app, platform }: { app: HostedApp; platform: Platform }) {
  return app === 'sheet' ? (
    <BlueSheetEditor platform={platform} />
  ) : app === 'slide' ? (
    <BlueSlideEditor platform={platform} audience={query.get('audience') ?? undefined} />
  ) : (
    <BlueEditor
      platform={platform}
      app={app}
      sampleUrl={`${import.meta.env.BASE_URL}samples/phase1-sample.docx`}
    />
  );
}

/** One-line notice over the editor (opening / errors of host files). */
function Notice({ text }: { text: string }) {
  return (
    <div
      data-testid="hosted-notice"
      style={{
        position: 'fixed',
        top: 8,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 1000,
        padding: '4px 12px',
        borderRadius: 6,
        background: '#e0f2fe',
        color: '#0c4a6e',
        font: '500 13px/20px system-ui, sans-serif',
        boxShadow: '0 1px 4px rgba(0,0,0,.2)',
      }}
    >
      {text}
    </div>
  );
}

let editor: ReactNode = <Editor app={app} platform={platform} />;
const render = (notice?: string) =>
  root.render(
    <StrictMode>
      {notice ? <Notice text={notice} /> : null}
      {editor}
    </StrictMode>,
  );
render();

/**
 * `bluedoc://open?host=…&file=…&token=…` (prompt §6A.3): the file is opened
 * through a BlueOffice server session (server from `BLUEOFFICE_SERVER` or the
 * stored setting), edited live with the web users of the same file, and
 * saved back to the host as checkpoints (`@blueoffice/hosted`).
 */
let current: HostedEditor | null = null;
async function openLink(link: HostFileLink): Promise<void> {
  const server = serverBase((await platform.shell.serverUrl?.()) ?? '');
  if (!server) return render(hostedText(language, 'noServer'));
  if (!link.token) return render(hostedText(language, 'openFailed', { message: 'token' }));
  render(hostedText(language, 'opening', { name: link.fileId }));
  try {
    current?.close();
    const h = await openHosted({
      platform,
      api: server,
      base: server,
      ref: { host: link.host, fileId: link.fileId, accessToken: link.token },
      fetchImpl: window.fetch.bind(window) as FetchLike,
      onEnded: (r) => {
        // After a restore the host content is authoritative: open it again.
        if (r === 'reload' && current === h) void openLink(link);
      },
    });
    current = h;
    void platform.shell.addRecentFile({
      name: h.file.name,
      path: `${link.host}:${link.fileId}`,
      openedAt: Date.now(),
    });
    editor = (
      <>
        <HostedBanner mode={h.mode} status={h.status} language={language} />
        <Editor key={h.session.sessionId} app={h.app} platform={h.platform} />
      </>
    );
    render();
  } catch (e: unknown) {
    render(
      hostedText(language, 'openFailed', { message: e instanceof Error ? e.message : String(e) }),
    );
  }
}

platform.shell.onOpenLink?.((link) => void openLink(link));
