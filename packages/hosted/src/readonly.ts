/**
 * View mode of a hosted file: while the session is not editable (opened
 * read-only, or the edit permission was revoked mid-session), commands that
 * would change the document are not run. Navigation, selection, copy, find
 * and export keep working. The editors stay unchanged; the guard wraps the
 * platform's engines (outermost, in front of the collaboration bindings).
 */
import { overlay } from '@blueoffice/collab';
import type {
  EditResult,
  EngineHost,
  Platform,
  SheetCommand,
  SheetHost,
  SlideCommand,
  SlideEditResult,
  TimedCommand,
} from '@blueoffice/platform';
import { DOC_QUERY_COMMANDS } from '@blueoffice/platform';
import type { SessionMode } from './mode';

/** BlueDoc commands that do not change the document. */
export const DOC_READ_COMMANDS: ReadonlySet<string> = new Set([
  // Every pure query (one shared list, see `DOC_QUERY_COMMANDS`).
  ...DOC_QUERY_COMMANDS,
  'state',
  'selectAll',
  'setSelection',
  'click',
  'dragTo',
  'move',
  'copy',
  'find',
  'selectMatch',
  'fontList',
  'notes',
  'review',
  'setRevisionView',
  'setMarkup',
  'nextRevision',
  'nextComment',
  'selectComment',
  'crossReferenceTargets',
  'closeHeaderFooter',
  'cancelFormat',
  // Protection state and session-only switches (CORE-SEC-001).
  'security',
  'setIdentity',
  'setReadOnly',
  'unlockWrite',
  'nextEditableRegion',
  'showEditableRegions',
  // Embedded objects, SmartArt, read aloud, format check: queries only.
  'oleInfo',
  'oleLinks',
  'smartArtInfo',
  'chartInfo',
  'chartGallery',
  'speechText',
  'speechRects',
  'checkAdminFormat',
  // Session settings, not document content (sent when a document opens).
  'autoCorrect',
  'proofOptions',
  // Author, identity, AutoCorrect, state and fonts in one call (CORE-MISC-003).
  'openSetup',
  // Background queries of the editor (spelling marks, navigation pane, status bar word
  // count, style pane) and the macro viewer: they run after a save too, so counting
  // them as edits made a just-saved session dirty again.
  'spelling',
  'spellAt',
  'outline',
  'wordCount',
  'styleList',
  'styleDetails',
  'vbaProject',
]);

/**
 * BlueDoc commands allowed in comment mode (prompt §6A.2 `mode: 'comment'`):
 * reviewers add, answer, resolve and delete comments but cannot change the text.
 */
export const DOC_COMMENT_COMMANDS: ReadonlySet<string> = new Set([
  'addComment',
  'replyComment',
  'resolveComment',
  'deleteComment',
  'setAuthor',
]);

/** BlueSlide commands that do not change the presentation. */
export const SLIDE_READ_COMMANDS: ReadonlySet<string> = new Set([
  'state',
  'goTo',
  'select',
  'selectRect',
  'selectNext',
  'click',
  'hitTest',
  'shapeBoxes',
  'headerFooterInfo',
  'copyFormat',
  'textSelect',
  'textHit',
  'textExit',
  'copy',
  'copySlides',
  'tableSelect',
  'tableHit',
  'tableExit',
  // Slide show data (layers, effects): viewers present too.
  'show',
  // Chart Design previews (CORE-CHART-007).
  'chartGallery',
  // Diagram layout thumbnails and the node under a point (CORE-SMART-004).
  'smartArtPreviews',
  'smartArtNodeAt',
  'security',
  'setIdentity',
  'setReadOnly',
  'unlockWrite',
  // Compare with another version (CORE-VERSION-001).
  'compareDeck',
  // Reuse slides (SLIDE-ALBUM-001): reading the other deck changes nothing.
  'reuseOpen',
  'reuseClose',
  // An empty deck for a photo album: a file, the open deck is untouched.
  'newDeck',
  // Queries: macro viewer, design ideas (previews only), hyperlink / ink / 3D model
  // information, the undo depth.
  'vbaProject',
  'designIdeas',
  'actionInfo',
  'inkInfo',
  'model3dMesh',
  'historyLen',
]);

/** BlueSheet `exec` commands that only read. */
export const SHEET_READ_COMMANDS: ReadonlySet<string> = new Set([
  'presets',
  'formatState',
  'region',
  'dataEdge',
  'columnEntries',
  'filterValues',
  'sortColors',
  'sortLists',
  'validation',
  'charts',
  'find',
  'names',
  'resolve',
  'copy',
  'pageSetup',
  'pivotSource',
  'pivots',
  'chartGallery',
  'chartTemplateExport',
  'chartTemplateImport',
  'security',
  'setIdentity',
  'setReadOnly',
  'unlockWrite',
  'protectedRanges',
  'unlockProtectedRange',
  // Show Changes (CORE-VERSION-001).
  'compareWorkbook',
  'smartArtInfo',
  'smartArtPreviews',
  // Map chart boundary datasets (CORE-CHART-006): engine set-up, not document edits.
  'mapDatasets',
  'registerMapDataset',
]);

export const READ_ONLY_ERROR = 'session.readOnly';

/**
 * Wraps the platform's engines so that edits are ignored while `mode` is not
 * editable; in comment mode BlueDoc's comment commands still run.
 */
export function guardPlatform(p: Platform, mode: SessionMode): Platform {
  const engine = p.engine;
  const docState = () => engine.command({ cmd: 'state' });
  const slideState = () => engine.slideCommand({ cmd: 'state' });
  const guardedEngine = overlay(engine, {
    command: (cmd: TimedCommand): Promise<EditResult> =>
      mode.editable ||
      DOC_READ_COMMANDS.has(cmd.cmd) ||
      (mode.canComment && DOC_COMMENT_COMMANDS.has(cmd.cmd))
        ? engine.command(cmd)
        : docState(),
    insertImage: (bytes: Uint8Array, name: string): Promise<EditResult> =>
      mode.editable ? engine.insertImage(bytes, name) : docState(),
    slideCommand: (cmd: SlideCommand): Promise<SlideEditResult> =>
      mode.editable || SLIDE_READ_COMMANDS.has(cmd.cmd) ? engine.slideCommand(cmd) : slideState(),
    slideInsertPicture: (bytes: Uint8Array, name: string): Promise<SlideEditResult> =>
      mode.editable ? engine.slideInsertPicture(bytes, name) : slideState(),
  }) as EngineHost;
  const sheet = p.sheet;
  const guardedSheet: SheetHost | undefined = sheet
    ? (overlay(sheet, {
        setInputs: async (s: number, cells: [number, number, string][]) => {
          if (mode.editable) await sheet.setInputs(s, cells);
        },
        structural: async (...args: Parameters<SheetHost['structural']>) => {
          if (mode.editable) await sheet.structural(...args);
        },
        undo: async () => (mode.editable ? sheet.undo() : false),
        redo: async () => (mode.editable ? sheet.redo() : false),
        addSheet: async (name?: string) => {
          if (!mode.editable) throw new Error(READ_ONLY_ERROR);
          return sheet.addSheet(name);
        },
        renameSheet: async (s: number, name: string) => {
          if (mode.editable) await sheet.renameSheet(s, name);
        },
        deleteSheet: async (s: number) => {
          if (mode.editable) await sheet.deleteSheet(s);
        },
        exec: <T = unknown>(cmd: SheetCommand): Promise<T> =>
          mode.editable || SHEET_READ_COMMANDS.has(cmd.cmd)
            ? sheet.exec<T>(cmd)
            : Promise.reject(new Error(READ_ONLY_ERROR)),
      }) as SheetHost)
    : undefined;
  return { ...p, engine: guardedEngine, ...(guardedSheet ? { sheet: guardedSheet } : {}) };
}
