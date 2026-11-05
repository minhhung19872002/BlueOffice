import { BlueEditor } from '@blueoffice/editor';
import { createDesktopPlatform } from '@blueoffice/platform/desktop';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

// The desktop app is only a shell: same editor, DesktopPlatform (native engine + real disk).
const platform = createDesktopPlatform({ storage: (files) => new LocalStorageProvider(files) });

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BlueEditor
      platform={platform}
      app="write"
      sampleUrl={`${import.meta.env.BASE_URL}samples/phase1-sample.docx`}
    />
  </StrictMode>,
);
