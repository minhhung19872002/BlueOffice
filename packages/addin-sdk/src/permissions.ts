/**
 * Add-in permissions (CORE-ADDIN-001, ADR-0013). A manifest lists the permissions
 * it needs; the user consents to all of them at install time; the host checks
 * every call against the granted set.
 *
 * The document API is the BlueScript object API (ADR-0012): every method name
 * below is one host call of `packages/script` (`API_METHODS`), plus a few
 * add-in-only methods (`addin.*`, `storage.*`). A method missing from this
 * table is refused.
 */

/** Permissions without an argument. */
export const BASE_PERMISSIONS = [
  'read-document',
  'write-document',
  'selection',
  'ui:ribbon',
  'storage',
] as const;

export type BasePermission = (typeof BASE_PERMISSIONS)[number];

/** `network:<origin>`: the add-in page may contact this origin (connect-src). */
export type NetworkPermission = `network:${string}`;

export type AddinPermission = BasePermission | NetworkPermission;

export const NETWORK_PREFIX = 'network:';

const R: readonly BasePermission[] = ['read-document'];
const W: readonly BasePermission[] = ['write-document'];
const S: readonly BasePermission[] = ['selection'];
/** The raw command escape hatch can both read and change anything. */
const RW: readonly BasePermission[] = ['read-document', 'write-document'];
const NONE: readonly BasePermission[] = [];
const ST: readonly BasePermission[] = ['storage'];

/**
 * Permissions each method needs (all of them). Methods that change the
 * document are run as one undo step each.
 */
export const METHOD_PERMISSIONS: Readonly<Record<string, readonly BasePermission[]>> = {
  // Handshake and add-in services.
  'addin.hello': NONE,
  'addin.selection': S,
  'storage.get': ST,
  'storage.set': ST,
  'storage.remove': ST,
  'storage.keys': ST,
  // BlueDoc (`doc` of BlueScript).
  'doc.info': R,
  'doc.text': R,
  'doc.paragraphs': R,
  'doc.paragraphStyle': R,
  'doc.find': R,
  'doc.selection': S,
  'doc.select': S,
  'doc.selectAll': S,
  'doc.setParagraphText': W,
  'doc.setParagraphStyle': W,
  'doc.formatParagraph': W,
  'doc.setParagraphFormat': W,
  'doc.insertParagraphAfter': W,
  'doc.deleteParagraph': W,
  'doc.appendParagraph': W,
  'doc.insertText': W,
  'doc.format': W,
  'doc.replace': W,
  'doc.insertTable': W,
  'doc.insertPageBreak': W,
  'doc.command': RW,
  // BlueSheet (`workbook` / `sheet`).
  'sheet.list': R,
  'sheet.active': R,
  'sheet.getValues': R,
  'sheet.getFormulas': R,
  'sheet.charts': R,
  'sheet.add': W,
  'sheet.rename': W,
  'sheet.setValues': W,
  'sheet.setFormulas': W,
  'sheet.fill': W,
  'sheet.format': W,
  'sheet.clear': W,
  'sheet.merge': W,
  'sheet.borders': W,
  'sheet.addChart': W,
  'sheet.command': RW,
  // BlueSlide (`presentation`).
  'slide.info': R,
  'slide.shapes': R,
  'slide.getNotes': R,
  'slide.addSlide': W,
  'slide.deleteSlide': W,
  'slide.duplicateSlide': W,
  'slide.setNotes': W,
  'slide.addTextBox': W,
  'slide.addShape': W,
  'slide.addTable': W,
  'slide.addChart': W,
  'slide.setText': W,
  'slide.formatText': W,
  'slide.setFill': W,
  'slide.setLine': W,
  'slide.move': W,
  'slide.deleteShape': W,
  'slide.command': RW,
};

/** Whether `method` changes the document (run as one undo step). */
export function isWriteMethod(method: string): boolean {
  return METHOD_PERMISSIONS[method]?.includes('write-document') ?? false;
}

/** Which application a document method belongs to (`null`: any). */
export function methodApp(method: string): 'doc' | 'sheet' | 'slide' | null {
  const ns = method.slice(0, method.indexOf('.'));
  return ns === 'doc' || ns === 'sheet' || ns === 'slide' ? ns : null;
}

/** The permissions `method` needs that `granted` lacks (empty: allowed); null: unknown method. */
export function missingPermissions(
  method: string,
  granted: readonly string[],
): BasePermission[] | null {
  if (!Object.prototype.hasOwnProperty.call(METHOD_PERMISSIONS, method)) return null;
  return METHOD_PERMISSIONS[method]!.filter((p) => !granted.includes(p));
}

/** Origins of the `network:` permissions. */
export function networkOrigins(permissions: readonly string[]): string[] {
  return permissions
    .filter((p) => p.startsWith(NETWORK_PREFIX))
    .map((p) => p.slice(NETWORK_PREFIX.length));
}
