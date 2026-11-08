import { describe, expect, it, vi } from 'vitest';
import { appForPath, createDesktopPlatform, type Invoke } from '../src/desktop';

describe('DesktopPlatform', () => {
  it('routes engine calls to Tauri commands with raw binary bodies', async () => {
    const calls: { cmd: string; args: unknown; options: unknown }[] = [];
    const fake = vi.fn(async (cmd: string, args?: unknown, options?: unknown) => {
      calls.push({ cmd, args, options });
      switch (cmd) {
        case 'engine_open':
          return {
            name: 'a.docx',
            pageCount: 1,
            pages: [{ width: 595, height: 842 }],
            wordCount: 2,
          };
        case 'engine_page':
          return '{"width":595,"height":842,"commands":[]}';
        case 'engine_glyph_outlines':
          return '[{"face":0,"glyph":36,"unitsPerEm":2048,"path":"M0 0Z"}]';
        case 'engine_export_pdf':
          return new TextEncoder().encode('%PDF-1.7').buffer;
        case 'engine_slide_command':
        case 'engine_slide_insert_picture':
          return '{"slide":1,"selection":[],"modified":true}';
        default:
          return null;
      }
    }) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: fake });
    expect(p.kind).toBe('desktop');
    expect(p.engine.backend).toBe('native');

    const info = await p.engine.open(new Uint8Array([1, 2, 3]), 'Tài liệu.docx');
    expect(info.pageCount).toBe(1);
    const open = calls.find((c) => c.cmd === 'engine_open')!;
    expect(open.args).toBeInstanceOf(Uint8Array);
    expect(open.options).toEqual({
      headers: { 'x-file-name': encodeURIComponent('Tài liệu.docx') },
    });

    expect((await p.engine.getPage(0)).width).toBe(595);
    expect((await p.engine.getGlyphOutlines(0, [36]))[0]!.unitsPerEm).toBe(2048);
    const pdf = await p.engine.exportPdf();
    expect(new TextDecoder().decode(pdf)).toBe('%PDF-1.7');

    // Slide editing: JSON command in, JSON result out; pictures as raw bodies.
    const r = await p.engine.slideCommand({ cmd: 'goTo', slide: 1 });
    expect(r.slide).toBe(1);
    const sc = calls.find((c) => c.cmd === 'engine_slide_command')!;
    expect(sc.args).toEqual({ json: '{"cmd":"goTo","slide":1}' });
    await p.engine.slideInsertPicture(new Uint8Array([137, 80]), 'Ảnh.png');
    const pic = calls.find((c) => c.cmd === 'engine_slide_insert_picture')!;
    expect(pic.args).toBeInstanceOf(Uint8Array);
    expect(pic.options).toEqual({ headers: { 'x-file-name': encodeURIComponent('Ảnh.png') } });
  });

  it('returns null when the file dialog is cancelled', async () => {
    const fake = (async () => null) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: fake });
    expect(await p.files.openFile(['.docx'])).toBeNull();
  });
});

describe('appForPath', () => {
  it('sends workbooks to BlueSheet, decks to BlueSlide and the rest to BlueDoc', () => {
    for (const f of ['a.xlsx', 'B.XLSM', 'c.xls', 'd.xlsb', 'e.ods', 'f.csv']) {
      expect(appForPath(f)).toBe('sheet');
    }
    for (const f of ['a.pptx', 'b.ppt', 'c.odp', 'd.potx']) expect(appForPath(f)).toBe('slide');
    for (const f of ['a.docx', 'b.doc', 'c.odt', 'd.rtf', 'e.txt'])
      expect(appForPath(f)).toBe('doc');
  });
});
