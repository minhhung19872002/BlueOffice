import { describe, expect, it, vi } from 'vitest';
import { createDesktopPlatform, type Invoke } from '../src/desktop';

describe('DesktopPlatform', () => {
  it('routes engine calls to Tauri commands with raw binary bodies', async () => {
    const calls: { cmd: string; args: unknown; options: unknown }[] = [];
    const fake = vi.fn(async (cmd: string, args?: unknown, options?: unknown) => {
      calls.push({ cmd, args, options });
      switch (cmd) {
        case 'engine_open':
          return { name: 'a.docx', pageCount: 1, pages: [{ width: 595, height: 842 }], wordCount: 2 };
        case 'engine_page':
          return '{"width":595,"height":842,"commands":[]}';
        case 'engine_glyph_outlines':
          return '[{"face":0,"glyph":36,"unitsPerEm":2048,"path":"M0 0Z"}]';
        case 'engine_export_pdf':
          return new TextEncoder().encode('%PDF-1.7').buffer;
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
    expect(open.options).toEqual({ headers: { 'x-file-name': encodeURIComponent('Tài liệu.docx') } });

    expect((await p.engine.getPage(0)).width).toBe(595);
    expect((await p.engine.getGlyphOutlines(0, [36]))[0]!.unitsPerEm).toBe(2048);
    const pdf = await p.engine.exportPdf();
    expect(new TextDecoder().decode(pdf)).toBe('%PDF-1.7');
  });

  it('returns null when the file dialog is cancelled', async () => {
    const fake = (async () => null) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: fake });
    expect(await p.files.openFile(['.docx'])).toBeNull();
  });
});
