import { BlueEditor } from '@blueoffice/editor';
import { createWebPlatform } from '@blueoffice/platform/web';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { resolveRoute } from './routes';

// The web app is only a shell: it builds the WebPlatform and embeds the editor.
const platform = createWebPlatform({ storage: (files) => new LocalStorageProvider(files) });
const base = import.meta.env.BASE_URL;
const { app, redirect } = resolveRoute(window.location.pathname, base);
if (redirect) {
  // Old routes (/write, /docs, /sheets, /slides) keep working.
  window.history.replaceState(null, '', redirect + window.location.search + window.location.hash);
}
const params = new URLSearchParams(window.location.search);
const language = params.get('lang') === 'en-US' ? 'en-US' : 'vi-VN';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BlueEditor
      platform={platform}
      app={app}
      language={language}
      sampleUrl={`${base}samples/phase1-sample.docx`}
    />
  </StrictMode>,
);
