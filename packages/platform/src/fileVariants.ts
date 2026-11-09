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
