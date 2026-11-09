import { describe, expect, it } from 'vitest';
import {
  FILE_VARIANTS,
  THEME_MIME,
  documentVariant,
  dropsMacros,
  mimeFor,
  openExtensions,
  saveVariants,
  variantOf,
  withExtension,
} from '../src';

describe('file variants (CORE-FMT-001)', () => {
  it('lists the fourteen OOXML variants, six of them for decks', () => {
    expect(FILE_VARIANTS.map((v) => v.ext)).toEqual([
      'docx',
      'docm',
      'dotx',
      'dotm',
      'xlsx',
      'xlsm',
      'xltx',
      'xltm',
      'pptx',
      'pptm',
      'potx',
      'potm',
      'ppsx',
      'ppsm',
    ]);
    expect(saveVariants('slide')).toHaveLength(6);
    expect(openExtensions('doc')).toEqual(['.docx', '.docm', '.dotx', '.dotm']);
  });

  it('finds a variant by name or extension', () => {
    expect(variantOf('Mẫu công văn.DOTM')).toMatchObject({
      app: 'doc',
      macros: true,
      template: true,
      show: false,
    });
    expect(variantOf('.ppsx')).toMatchObject({ app: 'slide', show: true, macros: false });
    expect(variantOf('xltx')?.template).toBe(true);
    expect(variantOf('a.doc')).toBeUndefined();
    expect(variantOf(null)).toBeUndefined();
  });

  it('maps templates and shows to the document they create', () => {
    const doc = (ext: string) => documentVariant(variantOf(ext)!).ext;
    expect(doc('dotx')).toBe('docx');
    expect(doc('dotm')).toBe('docm');
    expect(doc('xltm')).toBe('xlsm');
    expect(doc('potx')).toBe('pptx');
    expect(doc('ppsm')).toBe('pptm');
    expect(doc('xlsx')).toBe('xlsx');
  });

  it('warns only when macros would be dropped', () => {
    expect(dropsMacros(true, 'docx')).toBe(true);
    expect(dropsMacros(true, 'dotx')).toBe(true);
    expect(dropsMacros(true, 'dotm')).toBe(false);
    expect(dropsMacros(false, 'docx')).toBe(false);
    expect(dropsMacros(true, 'pdf')).toBe(false);
  });

  it('renames and gives MIME types', () => {
    expect(withExtension('Báo cáo.v2.pptx', 'ppsx')).toBe('Báo cáo.v2.ppsx');
    expect(withExtension('Không đuôi', 'xltx')).toBe('Không đuôi.xltx');
    expect(mimeFor('a.dotx')).toBe(
      'application/vnd.openxmlformats-officedocument.wordprocessingml.template',
    );
    expect(mimeFor('a.ppsm')).toBe('application/vnd.ms-powerpoint.slideshow.macroEnabled.12');
    expect(mimeFor('a.thmx')).toBe(THEME_MIME);
    expect(mimeFor('a.pdf')).toBe('application/pdf');
    expect(mimeFor('a.bin')).toBe('application/octet-stream');
  });
});
