import type { Language } from '@blueoffice/i18n';
import type {
  SpellInfo,
  DocInfo,
  EditResult,
  FontChoice,
  MergeState,
  OpenedFile,
  OutlineItem,
  PeerCursor,
  ReviewState,
  SecurityInfo,
  SignatureReport,
  StoredDocument,
} from '@blueoffice/platform';
import type { ThemeSetting } from '@blueoffice/ui';
import { createStore } from 'zustand/vanilla';
import type { ReferenceDialogId } from './ReferenceDialogs';

export type Status =
  | { kind: 'idle' }
  | {
      kind: 'busy';
      key: 'status.loading' | 'status.exporting' | 'status.saving' | 'status.signing';
    }
  | { kind: 'info'; key: string; params?: Record<string, string> }
  | { kind: 'error'; key: string; message: string };

/** BlueDoc task panes (CORE-UI-006): Styles, Comments, Version history (CORE-VERSION-001), BlueScript (CORE-SCRIPT-001). */
export type DocTaskPane =
  | 'styles'
  | 'comments'
  | 'versions'
  | 'script'
  /** Style Inspector and Reveal Formatting (DOC-STYLE-001). */
  | 'styleInspector'
  | 'revealFormatting';

/** `panes` with `id` shown (`on`) or hidden. */
export function togglePane(panes: DocTaskPane[], id: DocTaskPane, on = !panes.includes(id)) {
  const rest = panes.filter((p) => p !== id);
  return on ? [...rest, id] : rest;
}

export interface EditorState {
  doc: DocInfo | null;
  page: number;
  zoom: number;
  theme: ThemeSetting;
  language: Language;
  status: Status;
  engineReady: boolean;
  /** Latest editing result (selection, ribbon state, modified flag). */
  edit: EditResult | null;
  /** Latest review state (track changes, revisions, comments). */
  review: ReviewState | null;
  /** Mail-merge data source and preview. */
  merge: MergeState | null;
  panel:
    | 'find'
    | 'replace'
    | 'symbols'
    | 'icons'
    | 'review'
    | 'caption'
    | 'crossref'
    | 'merge'
    | 'savePassword'
    | 'notes'
    | 'sign'
    | 'signPdf'
    | 'signatures'
    | 'xmlMapping'
    | null;
  /** Verification of the opened file's digital signatures (null: unsigned / not checked). */
  signatures: SignatureReport | null;
  /** Docked list of changes and comments (null: hidden). */
  reviewingPane: 'left' | 'bottom' | null;
  /** Field codes shown instead of field results (Alt+F9). */
  fieldCodes: boolean;
  /** Modal dialog. */
  dialog:
    | 'pageSetup'
    | 'pasteSpecial'
    | 'tableProperties'
    | 'tableSort'
    | 'tableFormula'
    | 'textToTable'
    | 'tableToText'
    | 'picturePosition'
    | 'templates'
    | 'equation'
    | 'field'
    | 'control'
    | 'formField'
    | 'protectForms'
    | 'lockTracking'
    | 'defineList'
    | 'setNumbering'
    | 'autoCorrect'
    | 'adminFormat'
    | 'pageBorders'
    | 'watermark'
    | 'saveBlock'
    | 'blocksOrganizer'
    | 'checkDocument'
    | 'spellCheck'
    | 'language'
    | 'wordCount'
    | 'columns'
    | 'lineNumbers'
    | 'hyphenation'
    | 'pdfExport'
    | 'textDirection'
    | 'dropCap'
    | 'fontEffects'
    | 'phoneticGuide'
    | 'combineChars'
    | 'signatureLine'
    | 'signLine'
    | ReferenceDialogId
    | import('./CitationDialogs').CitationDialogId
    | import('./MergeDialogs').MailMergeDialogId
    | null;
  /** Format Painter: off, once (single click) or sticky (double click; Esc stops). */
  painter: 'off' | 'once' | 'sticky';
  /** Font picker entries from the engine. */
  fonts: FontChoice[];
  /** Show the rulers. */
  showRuler: boolean;
  /** Name of a document with an autosave waiting to be restored. */
  recovery: string | null;
  /** A protected file waiting for its password ([MS-OFFCRYPTO]). */
  passwordPrompt: { file: OpenedFile; doc: StoredDocument | null; wrong: boolean } | null;
  /** The open file has macros (kept, never run): show the warning bar. */
  macroWarning: boolean;
  /** Runs converted from TCVN3 / VNI to Unicode on open (0 = no notice). */
  legacyNotice: number;
  /** Collaborators editing the same document (live collaboration). */
  peers: PeerCursor[];
  /** Draw collaborators' carets. */
  showPeers: boolean;
  /** Spelling as you type (red wavy underline). */
  spellingOn: boolean;
  /** Grammar and style as you type (blue wavy underline). */
  grammarOn: boolean;
  /** Spelling menu open at a point (right click on a misspelt word). */
  spellMenu: { x: number; y: number; info: SpellInfo } | null;
  /** Final mark, read-only state, label, restricted editing (CORE-SEC-001). */
  security: SecurityInfo | null;
  /** Protection UI: the Protect dialog, or the "view only / edit" prompt on open. */
  protectUi: 'panel' | 'prompt' | null;
  /** Split view (View › Window): the first pane's share of the height; `null` = one pane. */
  split: number | null;
  /** Overtype mode (Insert key): typed text replaces what follows the caret. */
  overtype: boolean;
  /** Side by side with another window, and whether the two scroll together. */
  sideBySide: boolean;
  syncScroll: boolean;
  /** Proofing marks in the document (`null`: not checked yet). */
  proofErrors: number | null;
  /** BlueDoc view (View › Views): print, web or draft layout, outline view, read mode. */
  viewMode: 'print' | 'web' | 'draft' | 'outline' | 'read';
  /** Focus mode: ribbon and status bar hidden. */
  focusMode: boolean;
  /** Navigation pane tab (null: closed). */
  navPane: 'headings' | 'pages' | 'results' | null;
  /** Open task panes besides navigation (CORE-UI-006), in opening order. */
  taskPanes: DocTaskPane[];
  /** Formatting marks (¶ · →) shown. */
  showMarks: boolean;
  /** Document outline (outline view, navigation pane headings). */
  outline: OutlineItem[] | null;
  /** Bumped whenever page content changes (thumbnails, outline follow it). */
  layoutVersion: number;
}

export const initialState = (language: Language, theme: ThemeSetting): EditorState => ({
  doc: null,
  page: 0,
  zoom: 1,
  theme,
  language,
  status: { kind: 'idle' },
  engineReady: false,
  edit: null,
  review: null,
  merge: null,
  panel: null,
  dialog: null,
  painter: 'off',
  fonts: [],
  showRuler: true,
  recovery: null,
  passwordPrompt: null,
  macroWarning: false,
  legacyNotice: 0,
  signatures: null,
  reviewingPane: null,
  fieldCodes: false,
  peers: [],
  showPeers: true,
  spellingOn: true,
  grammarOn: true,
  spellMenu: null,
  security: null,
  protectUi: null,
  split: null,
  overtype: false,
  sideBySide: false,
  syncScroll: true,
  proofErrors: null,
  viewMode: 'print',
  focusMode: false,
  navPane: null,
  taskPanes: [],
  showMarks: false,
  outline: null,
  layoutVersion: 0,
});

export const createEditorStore = (language: Language, theme: ThemeSetting) =>
  createStore<EditorState>()(() => initialState(language, theme));

export type EditorStore = ReturnType<typeof createEditorStore>;
