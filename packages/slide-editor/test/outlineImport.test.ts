// @vitest-environment happy-dom
import type { SlideCommand, SlideEditResult, SlideOutlineSlide } from '@blueoffice/platform';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it, vi } from 'vitest';
import {
  decodeText,
  docxOutline,
  docxOutlineFromXml,
  importOutline,
  outlineImportCommand,
  outlineOfFile,
  outlineSlides,
  textOutline,
} from '../src/album/outlineImport';
import { unzipEntries } from '../src/album/unzip';
import { zipStore } from '../src/zip';

/** SLIDE-ALBUM-001: slides from an outline — docx headings → levels, txt indentation. */

const here = dirname(fileURLToPath(import.meta.url));
const corpus = (name: string) =>
  new Uint8Array(readFileSync(join(here, '..', '..', '..', 'tests', 'corpus', 'doc', name)));

const W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"';
const xml = (s: string) => new DOMParser().parseFromString(s, 'application/xml');
const enc = (s: string) => new TextEncoder().encode(s);

const para = (text: string, pPr = '') =>
  `<w:p>${pPr ? `<w:pPr>${pPr}</w:pPr>` : ''}<w:r><w:t>${text}</w:t></w:r></w:p>`;
const styled = (text: string, style: string) => para(text, `<w:pStyle w:val="${style}"/>`);
const doc = (body: string) => `<w:document ${W}><w:body>${body}</w:body></w:document>`;

const STYLES = `<w:styles ${W}>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
  <w:style w:type="paragraph" w:styleId="H1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/></w:style>
  <w:style w:type="paragraph" w:styleId="H2"><w:name w:val="Heading 2"/></w:style>
  <w:style w:type="paragraph" w:styleId="Muc"><w:name w:val="Mục lớn"/><w:pPr><w:outlineLvl w:val="2"/></w:pPr></w:style>
  <w:style w:type="paragraph" w:styleId="MucCon"><w:name w:val="Mục con"/><w:basedOn w:val="Muc"/></w:style>
  <w:style w:type="paragraph" w:styleId="Than"><w:name w:val="Thân"/><w:basedOn w:val="H1"/><w:pPr><w:outlineLvl w:val="9"/></w:pPr></w:style>
  <w:style w:type="character" w:styleId="H9"><w:name w:val="heading 9"/></w:style>
</w:styles>`;

describe('unzip', () => {
  it('reads stored and deflated entries, skips missing ones', async () => {
    const stored = zipStore([
      { name: 'a.txt', data: enc('một') },
      { name: 'dir/b.txt', data: enc('hai') },
    ]);
    const got = await unzipEntries(stored, ['dir/b.txt', 'a.txt', 'none.txt']);
    expect([...got.keys()].sort()).toEqual(['a.txt', 'dir/b.txt']);
    expect(new TextDecoder().decode(got.get('a.txt'))).toBe('một');
    expect(new TextDecoder().decode(got.get('dir/b.txt'))).toBe('hai');
    // A corpus document: its parts are deflated (the uncompressed size is in the ZIP directory).
    const parts = await unzipEntries(corpus('lists-04-heading-linked.docx'), ['word/document.xml']);
    const text = new TextDecoder().decode(parts.get('word/document.xml'));
    expect(text.startsWith('<?xml')).toBe(true);
    expect(text.trimEnd().endsWith('</w:document>')).toBe(true);
    expect(parts.get('word/document.xml')?.length).toBe(1927);
    expect((await unzipEntries(enc('not a zip'), ['a'])).size).toBe(0);
  });
});

describe('docx outline', () => {
  it('maps heading styles to levels: by name, by outline level, through basedOn', () => {
    const d = xml(
      doc(
        styled('Chương 1', 'H1') +
          para('Đoạn thường') +
          styled('Mục 1.1', 'H2') +
          styled('Ý cấp ba', 'Muc') +
          styled('Ý cấp ba kế thừa', 'MucCon') +
          styled('Thân bài', 'Than') +
          para('Cấp năm trực tiếp', '<w:outlineLvl w:val="4"/>') +
          para('Thân trực tiếp', '<w:pStyle w:val="H1"/><w:outlineLvl w:val="9"/>') +
          styled('Chương 2', 'H1'),
      ),
    );
    expect(docxOutlineFromXml(d, xml(STYLES))).toEqual([
      { level: 0, text: 'Chương 1' },
      { level: 1, text: 'Mục 1.1' },
      { level: 2, text: 'Ý cấp ba' },
      { level: 2, text: 'Ý cấp ba kế thừa' },
      { level: 4, text: 'Cấp năm trực tiếp' },
      { level: 0, text: 'Chương 2' },
    ]);
  });

  it('joins runs, turns tabs and breaks into spaces, leaves tracked deletions out', () => {
    const p =
      '<w:p><w:pPr><w:pStyle w:val="H1"/></w:pPr>' +
      '<w:r><w:t>Báo</w:t></w:r><w:r><w:tab/><w:t>cáo</w:t><w:br/><w:t>quý</w:t></w:r>' +
      '<w:del><w:r><w:delText>cũ</w:delText></w:r></w:del>' +
      '<w:moveFrom><w:r><w:t>đã chuyển</w:t></w:r></w:moveFrom>' +
      '<w:ins><w:r><w:t xml:space="preserve"> IV</w:t></w:r></w:ins>' +
      '<w:hyperlink><w:r><w:t> (xem)</w:t></w:r></w:hyperlink></w:p>';
    expect(docxOutlineFromXml(xml(doc(p)), xml(STYLES))).toEqual([
      { level: 0, text: 'Báo cáo quý IV (xem)' },
    ]);
  });

  it('gives one slide per paragraph when the document has no headings', () => {
    const d = xml(doc(para('Một') + para('') + para('Hai')));
    expect(docxOutlineFromXml(d, xml(STYLES))).toEqual([
      { level: 0, text: 'Một' },
      { level: 0, text: 'Hai' },
    ]);
    // Without a styles part only direct outline levels count.
    const e = xml(doc(styled('Không rõ kiểu', 'H1') + para('Có cấp', '<w:outlineLvl w:val="0"/>')));
    expect(docxOutlineFromXml(e, null)).toEqual([{ level: 0, text: 'Có cấp' }]);
  });

  it('reads a real document package (corpus file with Heading 1 / Heading 2)', async () => {
    // tests/corpus/doc/lists-04-heading-linked.docx: two chapters with sub-headings and
    // one body paragraph ("Nội dung") that is not part of the outline.
    const entries = await docxOutline(corpus('lists-04-heading-linked.docx'));
    expect(entries).toEqual([
      { level: 0, text: 'Tổng quan' },
      { level: 1, text: 'Phạm vi' },
      { level: 1, text: 'Thuật ngữ' },
      { level: 0, text: 'Thiết kế' },
      { level: 1, text: 'Mô hình' },
    ]);
    expect(outlineSlides(entries ?? [])).toEqual([
      {
        title: 'Tổng quan',
        bullets: [
          { text: 'Phạm vi', level: 0 },
          { text: 'Thuật ngữ', level: 0 },
        ],
      },
      { title: 'Thiết kế', bullets: [{ text: 'Mô hình', level: 0 }] },
    ]);
    // By content, whatever the file is called; a damaged package is unreadable.
    expect(await outlineOfFile('x.txt', corpus('lists-04-heading-linked.docx'))).toEqual(entries);
    expect(await outlineOfFile('hong.docx', enc('PK not really'))).toBeNull();
    const built = zipStore([
      { name: 'word/document.xml', data: enc(doc(styled('Tự dựng', 'H1'))) },
      { name: 'word/styles.xml', data: enc(STYLES) },
    ]);
    expect(await docxOutline(built)).toEqual([{ level: 0, text: 'Tự dựng' }]);
  });
});

describe('text outline', () => {
  it('uses tabs as levels', () => {
    expect(textOutline('Mở đầu\n\tÝ một\n\t\tÝ con\n\n\tÝ hai\r\nKết luận\r\n')).toEqual([
      { level: 0, text: 'Mở đầu' },
      { level: 1, text: 'Ý một' },
      { level: 2, text: 'Ý con' },
      { level: 1, text: 'Ý hai' },
      { level: 0, text: 'Kết luận' },
    ]);
  });

  it('uses the smallest space indentation as one level and drops list marks', () => {
    expect(textOutline('A\n    - b\n        * c\n    • d\nE')).toEqual([
      { level: 0, text: 'A' },
      { level: 1, text: 'b' },
      { level: 2, text: 'c' },
      { level: 1, text: 'd' },
      { level: 0, text: 'E' },
    ]);
    expect(textOutline('A\n  b\n   c\n    d').map((e) => e.level)).toEqual([0, 1, 1, 2]);
    expect(textOutline('\n \n')).toEqual([]);
  });

  it('decodes UTF-8 and UTF-16 with a byte order mark', async () => {
    const utf8 = new Uint8Array([0xef, 0xbb, 0xbf, ...enc('Tiêu đề\n\tý')]);
    expect(await outlineOfFile('a.txt', utf8)).toEqual([
      { level: 0, text: 'Tiêu đề' },
      { level: 1, text: 'ý' },
    ]);
    // "Đề\n\tý" in UTF-16LE: U+0110 U+1EC1 U+000A U+0009 U+00FD.
    const le = new Uint8Array([0xff, 0xfe, 0x10, 0x01, 0xc1, 0x1e, 0x0a, 0, 0x09, 0, 0xfd, 0]);
    expect(decodeText(le).replace(/^﻿/, '')).toBe('Đề\n\tý');
    expect(await outlineOfFile('b.txt', le)).toEqual([
      { level: 0, text: 'Đề' },
      { level: 1, text: 'ý' },
    ]);
    const be = new Uint8Array([0xfe, 0xff, 0x01, 0x10, 0x1e, 0xc1]);
    expect(textOutline(decodeText(be))).toEqual([{ level: 0, text: 'Đề' }]);
  });
});

describe('outline → slides', () => {
  it('starts an untitled slide for bullets before the first title and caps the level', () => {
    expect(
      outlineSlides([
        { level: 2, text: 'lạc' },
        { level: 0, text: 'T' },
        { level: 12, text: 'sâu' },
      ]),
    ).toEqual([
      { title: '', bullets: [{ text: 'lạc', level: 1 }] },
      { title: 'T', bullets: [{ text: 'sâu', level: 8 }] },
    ]);
    expect(outlineSlides([])).toEqual([]);
  });

  const existing: SlideOutlineSlide[] = [
    { title: 'Một', bullets: [{ text: 'a', level: 0 }], hasTitle: true, hasBody: true },
    { title: 'Hai', bullets: [], hasTitle: true, hasBody: false },
  ];
  const added = [{ title: 'Mới', bullets: [{ text: 'x', level: 0 }] }];

  it('builds one applyOutline that keeps the deck and adds after the current slide', () => {
    expect(outlineImportCommand(existing, 0, added)).toEqual({
      cmd: 'applyOutline',
      slides: [
        { slide: 0, title: 'Một', bullets: [{ text: 'a', level: 0 }] },
        { title: 'Mới', bullets: [{ text: 'x', level: 0 }] },
        { slide: 1, title: 'Hai', bullets: [] },
      ],
      current: 1,
    });
    // After the last slide; an out-of-range position is clamped.
    expect(outlineImportCommand(existing, 7, added).slides.map((s) => s.slide)).toEqual([
      0,
      1,
      undefined,
    ]);
    expect(outlineImportCommand(existing, 7, added).current).toBe(2);
  });

  it('imports with two commands: read the outline, apply once (one undo step)', async () => {
    const sent: SlideCommand[] = [];
    const run = vi.fn(async (cmd: SlideCommand) => {
      sent.push(cmd);
      return { outline: existing } as unknown as SlideEditResult;
    });
    const n = await importOutline(
      run,
      [
        { level: 0, text: 'Mới' },
        { level: 1, text: 'x' },
      ],
      1,
    );
    expect(n).toBe(1);
    expect(sent.map((c) => c.cmd)).toEqual(['outline', 'applyOutline']);
    expect(sent[1]).toEqual(outlineImportCommand(existing, 1, added));
    // Nothing to add: no command at all. A refusal adds nothing.
    sent.length = 0;
    expect(await importOutline(run, [], 0)).toBe(0);
    expect(sent).toEqual([]);
    const refused = vi.fn(
      async () => ({ outline: existing, error: 'readOnly' }) as unknown as SlideEditResult,
    );
    expect(await importOutline(refused, [{ level: 0, text: 'T' }], 0)).toBe(0);
  });
});
