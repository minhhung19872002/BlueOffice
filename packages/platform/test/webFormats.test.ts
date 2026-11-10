import { describe, expect, it } from 'vitest';
import {
  WEB_FORMATS,
  WEB_FORMAT_COMMANDS,
  appOfXml,
  webFormat,
  webFormats,
  webOpenExtensions,
} from '../src';

const bytes = (s: string) => new TextEncoder().encode(s);

describe('web and single-file XML formats (CORE-FMT-003)', () => {
  it('lists the formats each app saves to', () => {
    expect(WEB_FORMATS.map((f) => f.id)).toEqual(['html', 'mht', 'xml']);
    expect(webFormats('doc').map((f) => f.ext)).toEqual(['html', 'mht', 'xml']);
    expect(webFormats('sheet').map((f) => f.ext)).toEqual(['html', 'xml']);
    // A presentation is not exported as an HTML page set: Flat OPC only.
    expect(webFormats('slide').map((f) => f.ext)).toEqual(['xml']);
    expect(webFormat('mht')?.mime).toBe('multipart/related');
    expect(webFormat('html')?.mime).toBe('text/html');
    expect(webFormat('rtf')).toBeUndefined();
  });

  it('names one command per format', () => {
    expect(WEB_FORMAT_COMMANDS).toEqual({
      html: 'file.saveHtml',
      mht: 'file.saveMht',
      xml: 'file.saveFlatXml',
    });
  });

  it('lists the extensions the open dialogs accept', () => {
    expect(webOpenExtensions('doc')).toEqual(['.html', '.htm', '.mht', '.mhtml', '.xml']);
    expect(webOpenExtensions('sheet')).toEqual(['.html', '.htm', '.mht', '.mhtml', '.xml']);
    expect(webOpenExtensions('slide')).toEqual(['.xml']);
  });

  it('tells which app an .xml file belongs to', () => {
    const flat = (progid: string, body = '') =>
      bytes(
        `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\r\n${progid}<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">${body}</pkg:package>`,
      );
    expect(appOfXml(flat('<?mso-application progid="Word.Document"?>'))).toBe('doc');
    expect(appOfXml(flat('<?mso-application progid="Excel.Sheet"?>'))).toBe('sheet');
    expect(appOfXml(flat('<?mso-application progid="PowerPoint.Show"?>'))).toBe('slide');
    // No processing instruction: the content type of the main part decides.
    const part = (type: string) =>
      `<pkg:part pkg:name="/x.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.${type}+xml"/>`;
    expect(appOfXml(flat('', part('presentationml.presentation.main')))).toBe('slide');
    expect(appOfXml(flat('', part('spreadsheetml.sheet.main')))).toBe('sheet');
    expect(appOfXml(flat('', part('wordprocessingml.document.main')))).toBe('doc');
    expect(appOfXml(flat(''))).toBeUndefined();
    // XML Spreadsheet 2003.
    expect(
      appOfXml(
        bytes(
          '<?xml version="1.0"?><?mso-application progid="Excel.Sheet"?><Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet">',
        ),
      ),
    ).toBe('sheet');
    expect(
      appOfXml(bytes('<ss:Workbook xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet"/>')),
    ).toBe('sheet');
    // Any other XML says nothing.
    expect(appOfXml(bytes('<?xml version="1.0"?><note/>'))).toBeUndefined();
    expect(appOfXml(new Uint8Array())).toBeUndefined();
    // UTF-16 with a byte order mark.
    const text = '<Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet">';
    const utf16 = new Uint8Array(2 + text.length * 2);
    utf16.set([0xff, 0xfe]);
    for (let i = 0; i < text.length; i++) utf16[2 + i * 2] = text.charCodeAt(i);
    expect(appOfXml(utf16)).toBe('sheet');
  });
});
