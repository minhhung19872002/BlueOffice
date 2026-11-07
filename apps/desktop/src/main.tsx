import { BlueEditor } from '@blueoffice/editor';
import { createDesktopPlatform } from '@blueoffice/platform/desktop';
import { BlueSheetEditor } from '@blueoffice/sheet-editor';
import { BlueSlideEditor } from '@blueoffice/slide-editor';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

// The desktop app is only a shell: same editor, DesktopPlatform (native engine + real disk).
// One shell, three products: the window URL selects the app (see tauri.*.conf.json).
const requested = new URLSearchParams(window.location.search).get('app');
const app = requested === 'sheet' || requested === 'slide' ? requested : 'doc';
const platform = createDesktopPlatform({ storage: (files) => new LocalStorageProvider(files) });

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {app === 'sheet' ? (
      <BlueSheetEditor platform={platform} />
    ) : app === 'slide' ? (
      <BlueSlideEditor platform={platform} />
    ) : (
      <BlueEditor
        platform={platform}
        app={app}
        sampleUrl={`${import.meta.env.BASE_URL}samples/phase1-sample.docx`}
      />
    )}
  </StrictMode>,
);
