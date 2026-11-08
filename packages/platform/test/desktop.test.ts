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

  it('keeps the offline cache in the local cache folder through Tauri commands', async () => {
    const disk = new Map<string, Uint8Array>();
    const fake = vi.fn(async (cmd: string, args?: unknown, options?: unknown) => {
      const key =
        cmd === 'cache_put'
          ? decodeURIComponent(
              (options as { headers: Record<string, string> }).headers['x-cache-key']!,
            )
          : (args as { key: string }).key;
      if (cmd === 'cache_put') disk.set(key, args as Uint8Array);
      if (cmd === 'cache_delete') disk.delete(key);
      if (cmd === 'cache_get') {
        const v = disk.get(key);
        if (!v) throw new Error('cache:missing');
        return v.buffer;
      }
      return null;
    }) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: fake });
    expect(await p.offline.get('hosted.pending.bluedrive.f1')).toBeNull();
    await p.offline.put('hosted.pending.bluedrive.f1', new Uint8Array([7, 8]));
    expect([...((await p.offline.get('hosted.pending.bluedrive.f1')) ?? [])]).toEqual([7, 8]);
    await p.offline.delete('hosted.pending.bluedrive.f1');
    expect(await p.offline.get('hosted.pending.bluedrive.f1')).toBeNull();
  });

  it('exposes the Start screen shell services', async () => {
    const calls: { cmd: string; args: unknown }[] = [];
    const fake = vi.fn(async (cmd: string, args?: unknown) => {
      calls.push({ cmd, args });
      if (cmd === 'has_launch_request') return true;
      if (cmd === 'server_url') return 'http://localhost:4583';
      if (cmd === 'read_file') return new Uint8Array([1]).buffer;
      return null;
    }) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: fake });
    expect(await p.shell.hasLaunchRequest?.()).toBe(true);
    expect(await p.shell.serverFromEnvironment?.()).toBe(true);
    await p.shell.openExternal?.('https://sso.example.vn/auth');
    expect(calls.at(-1)).toEqual({ cmd: 'open_url', args: { url: 'https://sso.example.vn/auth' } });
    const f = await p.files.openPath?.('C:\\BlueDrive\\Dự án\\a.docx');
    expect(f?.name).toBe('a.docx');
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
