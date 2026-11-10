import { describe, expect, it } from 'vitest';
import {
  TABULAR_FORMATS,
  TABULAR_OPEN_EXTENSIONS,
  WEB_FORMAT_COMMANDS,
  isTabularFile,
  webFormats,
} from '../src';
import { appForPath } from '../src/desktop';

describe('secondary file formats (CORE-FMT-004)', () => {
  it('lists the legacy tabular formats BlueSheet saves a sheet to', () => {
    expect(TABULAR_FORMATS.map((f) => [f.id, f.ext, f.command])).toEqual([
      ['slk', 'slk', 'file.saveSlk'],
      ['dif', 'dif', 'file.saveDif'],
      ['prn', 'prn', 'file.savePrn'],
    ]);
    // All three are text files.
    expect(new Set(TABULAR_FORMATS.map((f) => f.mime))).toEqual(new Set(['text/plain']));
    // One command per format, none shared with the web formats.
    const commands = TABULAR_FORMATS.map((f) => f.command);
    expect(new Set(commands).size).toBe(commands.length);
    for (const c of commands) expect(Object.values(WEB_FORMAT_COMMANDS)).not.toContain(c);
  });

  it('opens SYLK, DIF, formatted text and dBASE tables in BlueSheet', () => {
    expect(TABULAR_OPEN_EXTENSIONS).toEqual(['.slk', '.sylk', '.dif', '.prn', '.dbf']);
    // Every saved format can be opened again; dBASE tables are opened only.
    for (const f of TABULAR_FORMATS) expect(TABULAR_OPEN_EXTENSIONS).toContain(`.${f.ext}`);
    expect(TABULAR_FORMATS.some((f) => f.ext === 'dbf')).toBe(false);
    for (const name of ['a.slk', 'B.SYLK', 'c.dif', 'd.Prn', 'khách hàng.dbf']) {
      expect(isTabularFile(name)).toBe(true);
      expect(appForPath(name)).toBe('sheet');
    }
    for (const name of ['a.xlsx', 'slk', 'a.slk.docx', 'dbf.txt']) {
      expect(isTabularFile(name)).toBe(false);
    }
  });

  it('exports documents, and only documents, as EPUB', () => {
    const epub = (app: 'doc' | 'sheet' | 'slide') => webFormats(app).some((f) => f.id === 'epub');
    expect(epub('doc')).toBe(true);
    expect(epub('sheet')).toBe(false);
    expect(epub('slide')).toBe(false);
    expect(WEB_FORMAT_COMMANDS.epub).toBe('file.saveEpub');
  });
});
