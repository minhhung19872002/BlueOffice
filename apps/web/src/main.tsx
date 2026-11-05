import { BlueEditor } from '@blueoffice/editor';
import { createWebPlatform } from '@blueoffice/platform/web';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

// The web app is only a shell: it builds the WebPlatform and embeds the editor.
const platform = createWebPlatform({ storage: (files) => new LocalStorageProvider(files) });
const base = import.meta.env.BASE_URL;
const route = window.location.pathname.replace(base, '/').split('/')[1];
const app = route === 'sheet' || route === 'slide' ? route : 'write';
const params = new URLSearchParams(window.location.search);
const language = params.get('lang') === 'en-US' ? 'en-US' : 'vi-VN';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BlueEditor platform={platform} app={app} language={language} sampleUrl={`${base}samples/phase1-sample.docx`} />
  </StrictMode>,
);
