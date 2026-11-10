import type {
  DocumentRef,
  DocumentStorage,
  FileAccess,
  StoredDocument,
} from '@blueoffice/platform';
import { mimeFor, webOpenExtensions } from '@blueoffice/platform';

const LOCAL_PERMISSIONS = { edit: true, download: true, share: false } as const;

/**
 * Local files: upload/download (or File System Access API) on the web, the real
 * disk on desktop — both through the platform's {@link FileAccess}.
 */
export class LocalStorageProvider implements DocumentStorage {
  readonly id = 'local';

  constructor(
    private readonly files: FileAccess,
    private readonly accept: string[] = [
      '.docx',
      '.docm',
      '.dotx',
      '.dotm',
      '.txt',
      '.md',
      // Web pages and Flat OPC (CORE-FMT-003).
      ...webOpenExtensions('doc'),
      // PDF, best effort: the text of each page (CORE-FMT-004).
      '.pdf',
    ],
  ) {}

  async open(ref?: DocumentRef): Promise<StoredDocument | null> {
    if (ref) throw new Error('storage.local.openByRefUnsupported');
    const f = await this.files.openFile(this.accept);
    if (!f) return null;
    return {
      ref: { provider: this.id, id: f.path ?? f.name, name: f.name },
      bytes: f.bytes,
      version: String(Date.now()),
      permissions: { ...LOCAL_PERMISSIONS },
    };
  }

  async save(doc: StoredDocument, bytes: Uint8Array): Promise<StoredDocument> {
    const saved = await this.files.saveFile(bytes, doc.ref.name, mime(doc.ref.name));
    if (!saved) return doc;
    return { ...doc, bytes, version: String(Date.now()), ref: { ...doc.ref, id: saved } };
  }

  async saveAs(name: string, bytes: Uint8Array): Promise<StoredDocument | null> {
    const saved = await this.files.saveFile(bytes, name, mime(name));
    if (!saved) return null;
    return {
      ref: { provider: this.id, id: saved, name },
      bytes,
      version: String(Date.now()),
      permissions: { ...LOCAL_PERMISSIONS },
    };
  }
}

/** PDF, OOXML documents / templates / macro-enabled variants / slide shows, theme files. */
function mime(name: string): string {
  return mimeFor(name);
}

export * from './server';
