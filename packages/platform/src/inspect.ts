/**
 * Check document (CORE-INSPECT-001): the report of `bo_engine::inspect` —
 * document inspector, accessibility checker and compatibility checker —
 * shared by BlueDoc (`inspect*` edit commands), BlueSheet (`inspect*` sheet
 * commands) and BlueSlide (`inspect*` slide commands). The engine sends ids;
 * the UI words them through i18n keys.
 */

/** What the document inspector looks for. */
export type InspectCategory =
  | 'comments'
  | 'revisions'
  | 'properties'
  | 'customProperties'
  | 'customXml'
  | 'serverProperties'
  | 'macros'
  | 'hiddenText'
  | 'hiddenRows'
  | 'hiddenColumns'
  | 'hiddenSheets'
  | 'hiddenSlides'
  | 'offSlide'
  | 'notes'
  | 'headersFooters'
  | 'watermarks'
  | 'invisibleObjects'
  | 'embedded';

/** `remove`: now, one undo step · `onSave`: dropped when saved · `acceptAll`: tracked changes. */
export type InspectAction = 'remove' | 'onSave' | 'acceptAll' | 'none';

export interface InspectFinding {
  category: InspectCategory;
  count: number;
  /** A few names / values found (authors, sheet names, slide numbers …). */
  details: string[];
  action: InspectAction;
  /** Removal is queued for the next save. */
  pending: boolean;
}

export type A11ySeverity = 'error' | 'warning' | 'tip';

export type A11yRule =
  | 'altText'
  | 'tableHeader'
  | 'mergedCells'
  | 'headingOrder'
  | 'noHeadings'
  | 'contrast'
  | 'linkText'
  | 'slideTitle'
  | 'duplicateTitle'
  | 'readingOrder'
  | 'sheetName'
  | 'layoutTable'
  | 'documentTitle';

/** Where an issue is. */
export type InspectTarget =
  | { kind: 'document' }
  | { kind: 'paragraph'; para: number }
  | { kind: 'drawing'; para: number; at: number }
  | { kind: 'table'; para: number }
  | { kind: 'slide'; slide: number }
  | { kind: 'shape'; slide: number; id: number }
  | { kind: 'sheet'; sheet: number }
  | { kind: 'range'; sheet: number; r0: number; c0: number; r1: number; c1: number };

/** `unmerge` / `rename`: BlueSheet (merged cells, default sheet names). */
export type A11yFixKind = 'altText' | 'headerRow' | 'unmerge' | 'rename';

export interface A11yIssue {
  rule: A11yRule;
  severity: A11ySeverity;
  target: InspectTarget;
  /** Object name, title or text sample. */
  detail: string;
  /** Contrast ratio of `contrast` issues. */
  ratio?: number;
  fix?: A11yFixKind;
}

/** A quick fix request. */
export type InspectFix =
  | { kind: 'altText'; target: InspectTarget; text: string }
  | { kind: 'headerRow'; target: InspectTarget }
  | { kind: 'unmerge'; target: InspectTarget }
  | { kind: 'rename'; target: InspectTarget; text: string };

/** `legacy`: the 97–2003 binary formats · `odf`: OpenDocument as BlueOffice writes it. */
export type CompatFormat = 'legacy' | 'odf';

export type CompatFeature =
  | 'contentControls'
  | 'equations'
  | 'smartArt'
  | 'charts'
  | 'modernCharts'
  | 'tables'
  | 'drawings'
  | 'comments'
  | 'commentThreads'
  | 'trackedChanges'
  | 'headersFooters'
  | 'notes'
  | 'textEffects'
  | 'longBookmarks'
  | 'wideTables'
  | 'fields'
  | 'beyondGrid'
  | 'newFunctions'
  | 'dynamicArrays'
  | 'slicers'
  | 'timelines'
  | 'sparklines'
  | 'conditionalFormats'
  | 'advancedConditionalFormats'
  | 'dataValidation'
  | 'pivotTables'
  | 'longSheetNames'
  | 'manyCellFormats'
  | 'media'
  | 'transitions'
  | 'morph'
  | 'animations'
  | 'sections'
  | 'models3d';

export interface CompatItem {
  feature: CompatFeature;
  count: number;
  formats: CompatFormat[];
}

export interface InspectReport {
  inspector: InspectFinding[];
  /** At most 500 issues; `issueCount` is the total. */
  accessibility: A11yIssue[];
  issueCount: number;
  /** Features present in the document only. */
  compatibility: CompatItem[];
}

/** The inspector commands, the same in the three bridges. */
export type InspectCommand =
  | { cmd: 'inspect' }
  | { cmd: 'inspectRemove'; categories: InspectCategory[] }
  | { cmd: 'inspectKeep'; categories: InspectCategory[] }
  | { cmd: 'inspectFix'; fix: InspectFix };
