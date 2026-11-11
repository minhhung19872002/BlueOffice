/**
 * BlueDoc engine commands that only read (queries): they change neither the document nor
 * where the caret is. One list for every place that must tell a query from an edit —
 * hosted sessions (`@blueoffice/hosted`: dirty tracking and view mode), the BlueScript
 * recorder (`@blueoffice/script`) and "repeat last command" (`@blueoffice/editor`).
 *
 * These lists used to be kept separately and a new background query was missed in one of
 * them three times (a saved session turned dirty again; the recorder wrote queries into the
 * script). Add a new query HERE; `packages/platform/test/queryCommands.test.ts` scans the
 * sources and fails when a query command is sent that this list does not know.
 */
export const DOC_QUERY_COMMANDS: readonly string[] = [
  'state',
  'fontList',
  'historyLen',
  // Background queries of the editor: spelling marks, navigation pane, status bar, style pane.
  'spelling',
  'spellAt',
  'outline',
  'wordCount',
  'styleList',
  'styleDetails',
  // Read aloud, focused reader and the screen-reader text layer.
  'speechText',
  'speechRects',
  'speechLine',
  'speechTree',
  // Object and document information.
  'oleInfo',
  'oleLinks',
  'smartArtInfo',
  'smartArtPreviews',
  'vbaProject',
  'checkAdminFormat',
  'docProperties',
  'crossReferenceTargets',
  'notes',
];
