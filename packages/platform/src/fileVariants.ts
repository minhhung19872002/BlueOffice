/**
 * Package variants of the three document types (CORE-FMT-001): documents,
 * templates, macro-enabled documents / templates and slide shows, plus theme
 * files. The engine decides the variant of an opened file from its main part's
 * content type (`DocInfo.variant`, `WorkbookInfo.variant`); these helpers choose
 * save-as types, file names, MIME types and the macro warning.
 */

export type VariantApp = 'doc' | 'sheet' | 'slide';

export interface FileVariant {
  /** Extension without the dot (`dotm`). */
  ext: string;
  app: VariantApp;
  /** Can hold a VBA project (kept, never run). */
  macros: boolean;
  /** A template: opening it starts a new document. */
  template: boolean;
  /** A slide show: opens straight into the show. */
  show: boolean;
  /** MIME type for downloads (IANA registrations of the OOXML types). */
  mime: string;
}

const OXML = 'application/vnd.openxmlformats-officedocument.';

const v = (
  ext: string,
  app: VariantApp,
  mime: string,
  flags: { macros?: boolean; template?: boolean; show?: boolean } = {},
): FileVariant => ({
  ext,
  app,
  mime,
  macros: !!flags.macros,
  template: !!flags.template,
  show: !!flags.show,
});

/** Every variant, the plain document type of each app first. */
export const FILE_VARIANTS: readonly FileVariant[] = [
  v('docx', 'doc', `${OXML}wordprocessingml.document`),
  v('docm', 'doc', 'application/vnd.ms-word.document.macroEnabled.12', { macros: true }),
  v('dotx', 'doc', `${OXML}wordprocessingml.template`, { template: true }),
  v('dotm', 'doc', 'application/vnd.ms-word.template.macroEnabled.12', {
    macros: true,
    template: true,
  }),
  v('xlsx', 'sheet', `${OXML}spreadsheetml.sheet`),
  v('xlsm', 'sheet', 'application/vnd.ms-excel.sheet.macroEnabled.12', { macros: true }),
  v('xltx', 'sheet', `${OXML}spreadsheetml.template`, { template: true }),
  v('xltm', 'sheet', 'application/vnd.ms-excel.template.macroEnabled.12', {
    macros: true,
    template: true,
  }),
  v('pptx', 'slide', `${OXML}presentationml.presentation`),
  v('pptm', 'slide', 'application/vnd.ms-powerpoint.presentation.macroEnabled.12', {
    macros: true,
  }),
  v('potx', 'slide', `${OXML}presentationml.template`, { template: true }),
  v('potm', 'slide', 'application/vnd.ms-powerpoint.template.macroEnabled.12', {
    macros: true,
    template: true,
  }),
  v('ppsx', 'slide', `${OXML}presentationml.slideshow`, { show: true }),
  v('ppsm', 'slide', 'application/vnd.ms-powerpoint.slideshow.macroEnabled.12', {
    macros: true,
    show: true,
  }),
];

/** Theme files (`.thmx`). */
export const THEME_EXT = 'thmx';
export const THEME_MIME = 'application/vnd.ms-officetheme';

/** The variant of an extension or file name (`dotm`, `.DOTM`, `Mẫu.dotm`). */
export function variantOf(nameOrExt: string | undefined | null): FileVariant | undefined {
  if (!nameOrExt) return undefined;
  const ext = nameOrExt.toLowerCase().split('.').pop() ?? '';
  return FILE_VARIANTS.find((f) => f.ext === ext);
}

/** Save-as types of an app, the plain document type first. */
export function saveVariants(app: VariantApp): FileVariant[] {
  return FILE_VARIANTS.filter((f) => f.app === app);
}

/** Extensions (with the dot) the open dialog of an app accepts for its OOXML variants. */
export function openExtensions(app: VariantApp): string[] {
  return saveVariants(app).map((f) => `.${f.ext}`);
}

/** Saving a file that has macros as `target` removes them. */
export function dropsMacros(hasMacros: boolean | undefined, target: string): boolean {
  const t = variantOf(target);
  return !!hasMacros && !!t && !t.macros;
}

/** `name` with its extension replaced by `ext` (added when it has none). */
export function withExtension(name: string, ext: string): string {
  const dot = name.lastIndexOf('.');
  const base = dot > 0 ? name.slice(0, dot) : name;
  return `${base}.${ext}`;
}

/**
 * The editable document a template (or slide show) creates: `.dotx` → `.docx`,
 * `.dotm` → `.docm`, `.ppsx` → `.pptx`; documents map to themselves.
 */
export function documentVariant(f: FileVariant): FileVariant {
  const base = saveVariants(f.app);
  return base.find((b) => !b.template && !b.show && b.macros === f.macros) ?? base[0]!;
}

/** MIME type of a file name's variant (`application/octet-stream` when unknown). */
export function variantMime(name: string): string | undefined {
  if (name.toLowerCase().endsWith(`.${THEME_EXT}`)) return THEME_MIME;
  return variantOf(name)?.mime;
}

/**
 * Web and single-file XML formats (CORE-FMT-003): one HTML file, a single-file
 * web page (MHTML) and Flat OPC — the whole package as one XML file; and the EPUB 3
 * export of documents (CORE-FMT-004).
 */
export interface WebFormat {
  /** Format id of the engine (`exportFile`, `SheetHost.save`). */
  id: 'html' | 'mht' | 'xml' | 'epub';
  /** Extension of saved files, without the dot. */
  ext: string;
  mime: string;
  /** Apps that save to the format. */
  apps: readonly VariantApp[];
}

export const WEB_FORMATS: readonly WebFormat[] = [
  { id: 'html', ext: 'html', mime: 'text/html', apps: ['doc', 'sheet'] },
  { id: 'mht', ext: 'mht', mime: 'multipart/related', apps: ['doc', 'sheet'] },
  { id: 'xml', ext: 'xml', mime: 'application/xml', apps: ['doc', 'sheet', 'slide'] },
  { id: 'epub', ext: 'epub', mime: 'application/epub+zip', apps: ['doc'] },
];

/** Web formats an app saves to. */
export function webFormats(app: VariantApp): WebFormat[] {
  return WEB_FORMATS.filter((f) => f.apps.includes(app));
}

/** A web format by id. */
export function webFormat(id: string): WebFormat | undefined {
  return WEB_FORMATS.find((f) => f.id === id);
}

/**
 * Extensions (with the dot) of the web and XML formats an app opens: HTML and MHTML
 * pages (BlueDoc; BlueSheet reads their tables), Flat OPC (`.xml`, every app) and
 * XML Spreadsheet 2003 (`.xml`, BlueSheet). The engine tells them apart by content.
 */
export function webOpenExtensions(app: VariantApp): string[] {
  return app === 'slide' ? ['.xml'] : ['.html', '.htm', '.mht', '.mhtml', '.xml'];
}

/** Command of each web format in the editors' command registries (`file.saveHtml`, …). */
export const WEB_FORMAT_COMMANDS: Record<WebFormat['id'], string> = {
  html: 'file.saveHtml',
  mht: 'file.saveMht',
  xml: 'file.saveFlatXml',
  epub: 'file.saveEpub',
};

/**
 * The app an `.xml` file belongs to, from its first bytes: XML Spreadsheet 2003 is a
 * workbook; Flat OPC names its application in the `mso-application` processing
 * instruction or by the content type of its main part. `undefined` when the text
 * tells nothing (the caller falls back to the file name).
 */
export function appOfXml(bytes: Uint8Array): VariantApp | undefined {
  const utf16 = bytes[0] === 0xff && bytes[1] === 0xfe;
  const head = new TextDecoder(utf16 ? 'utf-16le' : 'utf-8').decode(bytes.subarray(0, 262_144));
  const start = head.slice(0, 4096);
  if (
    start.includes('urn:schemas-microsoft-com:office:spreadsheet') &&
    /<(\w+:)?Workbook[\s>]/.test(start)
  ) {
    return 'sheet';
  }
  if (!start.includes('http://schemas.microsoft.com/office/2006/xmlPackage')) return undefined;
  const progid = /<\?mso-application\s+progid="([^"]+)"/.exec(start)?.[1] ?? '';
  if (progid.startsWith('Word.')) return 'doc';
  if (progid.startsWith('Excel.')) return 'sheet';
  if (progid.startsWith('PowerPoint.')) return 'slide';
  if (/wordprocessingml\.(document|template)\.main|ms-word\./.test(head)) return 'doc';
  if (/spreadsheetml\.(sheet|template)\.main|ms-excel\./.test(head)) return 'sheet';
  if (/presentationml\.(presentation|slideshow|template)\.main|ms-powerpoint\./.test(head)) {
    return 'slide';
  }
  return undefined;
}

/**
 * Legacy tabular formats of BlueSheet (CORE-FMT-004): SYLK, DIF and formatted text
 * (space delimited) hold one sheet and are opened and saved; dBASE tables (`.dbf`)
 * are opened only.
 */
export interface TabularFormat {
  /** Format id of the engine (`SheetHost.save`). */
  id: 'slk' | 'dif' | 'prn';
  /** Extension of saved files, without the dot. */
  ext: string;
  mime: string;
  /** Command of the format in the sheet editor's command registry. */
  command: string;
}

export const TABULAR_FORMATS: readonly TabularFormat[] = [
  { id: 'slk', ext: 'slk', mime: 'text/plain', command: 'file.saveSlk' },
  { id: 'dif', ext: 'dif', mime: 'text/plain', command: 'file.saveDif' },
  { id: 'prn', ext: 'prn', mime: 'text/plain', command: 'file.savePrn' },
];

/** Extensions (with the dot) of the legacy tabular files BlueSheet opens. */
export const TABULAR_OPEN_EXTENSIONS: readonly string[] = [
  '.slk',
  '.sylk',
  '.dif',
  '.prn',
  '.dbf',
];

/** Whether a file name has the extension of a legacy tabular format. */
export function isTabularFile(name: string): boolean {
  const lower = name.toLowerCase();
  return TABULAR_OPEN_EXTENSIONS.some((e) => lower.endsWith(e));
}
