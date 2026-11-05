import { describe, expect, it } from 'vitest';
import { FileAccessStorage, IndexedDbCache, mimeFor } from '../src/shared';
import type { FileAccess } from '../src/types';

describe('shared platform services', () => {
  it('maps file names to MIME types', () => {
    expect(mimeFor('a.DOCX')).toContain('wordprocessingml');
    expect(mimeFor('x.pdf')).toBe('application/pdf');
    expect(mimeFor('noext')).toBe('application/octet-stream');
  });

  it('offline cache falls back to memory without IndexedDB', async () => {
    const c = new IndexedDbCache('test');
    await c.put('k', new Uint8Array([1, 2]));
    expect(await c.get('k')).toEqual(new Uint8Array([1, 2]));
    await c.delete('k');
    expect(await c.get('k')).toBeNull();
  });

  it('FileAccessStorage opens and saves through FileAccess', async () => {
    const saved: string[] = [];
    const files: FileAccess = {
      openFile: async () => ({ name: 'a.docx', path: 'C:/a.docx', bytes: new Uint8Array([7]) }),
      saveFile: async (_b, name) => {
        saved.push(name);
        return `C:/${name}`;
      },
    };
    const s = new FileAccessStorage(files);
    const doc = await s.open();
    expect(doc?.ref).toEqual({ provider: 'local', id: 'C:/a.docx', name: 'a.docx' });
    const as = await s.saveAs('b.pdf', new Uint8Array([1]));
    expect(as?.ref.id).toBe('C:/b.pdf');
    expect(saved).toEqual(['b.pdf']);
  });
});
