import { describe, expect, it } from 'vitest';
import type { FileAccess } from '@blueoffice/platform';
import { LocalStorageProvider } from '../src';

function fakeFiles(cancel = false) {
  const saves: { name: string; mime: string; size: number }[] = [];
  const files: FileAccess = {
    openFile: async () => (cancel ? null : { name: 'Báo cáo.docx', bytes: new Uint8Array([80, 75]) }),
    saveFile: async (bytes, name, mime) => {
      saves.push({ name, mime, size: bytes.length });
      return cancel ? null : `/home/u/${name}`;
    },
  };
  return { files, saves };
}

describe('LocalStorageProvider', () => {
  it('opens through FileAccess with full local permissions', async () => {
    const p = new LocalStorageProvider(fakeFiles().files);
    const doc = await p.open();
    expect(doc?.ref).toEqual({ provider: 'local', id: 'Báo cáo.docx', name: 'Báo cáo.docx' });
    expect(doc?.permissions.edit).toBe(true);
    await expect(p.open({ provider: 'local', id: 'x', name: 'x' })).rejects.toThrow();
  });

  it('saves and save-as with MIME types', async () => {
    const { files, saves } = fakeFiles();
    const p = new LocalStorageProvider(files);
    const doc = (await p.open())!;
    const saved = await p.save(doc, new Uint8Array(3));
    expect(saved.ref.id).toBe('/home/u/Báo cáo.docx');
    const pdf = await p.saveAs('a.pdf', new Uint8Array(5));
    expect(pdf?.ref.name).toBe('a.pdf');
    expect(saves.map((s) => s.mime)).toEqual([
      'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
      'application/pdf',
    ]);
  });

  it('handles cancellation', async () => {
    const p = new LocalStorageProvider(fakeFiles(true).files);
    expect(await p.open()).toBeNull();
    expect(await p.saveAs('a.pdf', new Uint8Array())).toBeNull();
  });
});
