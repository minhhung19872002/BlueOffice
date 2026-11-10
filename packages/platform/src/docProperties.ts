/**
 * Document properties of the File › Info page (CORE-UI-002): core properties
 * (ECMA-376 Part 2 §11, `docProps/core.xml`) and extended properties (Part 1
 * §22.2, `docProps/app.xml`), `bo_engine::props::DocProperties`. Shared by
 * BlueDoc (edit commands), BlueSheet (sheet commands) and BlueSlide (slide
 * commands). Empty string = not set.
 */
export interface DocProperties {
  title: string;
  subject: string;
  author: string;
  keywords: string;
  comments: string;
  category: string;
  status: string;
  company: string;
  manager: string;
  /** Read only. */
  lastModifiedBy: string;
  /** Read only (W3CDTF date). */
  created: string;
  /** Read only (W3CDTF date). */
  modified: string;
  /** Read only. */
  revision: string;
}

/** Editable fields, in the order the Info page shows them. */
export const EDITABLE_DOC_PROPERTIES = [
  'title',
  'subject',
  'author',
  'keywords',
  'comments',
  'category',
  'status',
  'company',
  'manager',
] as const satisfies readonly (keyof DocProperties)[];

export type EditableDocProperty = (typeof EDITABLE_DOC_PROPERTIES)[number];

/**
 * Read (`docProperties`) or change (`setDocProperties`, written on the next
 * save; fields left out stay, an empty string removes one) the properties. BlueDoc / BlueSlide answer in `docProperties`; BlueSheet
 * returns the `DocProperties` object.
 */
export type DocPropertiesCommand =
  | { cmd: 'docProperties' }
  | { cmd: 'setDocProperties'; props: Partial<Pick<DocProperties, EditableDocProperty>> };
