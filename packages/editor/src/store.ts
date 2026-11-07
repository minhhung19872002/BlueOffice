import type { Language } from '@blueoffice/i18n';
import type {
  DocInfo,
  EditResult,
  FontChoice,
  MergeState,
  OpenedFile,
  PeerCursor,
  ReviewState,
  StoredDocument,
} from '@blueoffice/platform';
import type { ThemeSetting } from '@blueoffice/ui';
import { createStore } from 'zustand/vanilla';

export type Status =
  | { kind: 'idle' }
  | { kind: 'busy'; key: 'status.loading' | 'status.exporting' | 'status.saving' }
  | { kind: 'info'; key: string; params?: Record<string, string> }
  | { kind: 'error'; key: string; message: string };

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
    | 'review'
    | 'comments'
    | 'caption'
    | 'crossref'
    | 'merge'
    | 'savePassword'
    | 'notes'
    | null;
  /** Modal dialog. */
  dialog: 'pageSetup' | 'pasteSpecial' | 'tableProperties' | 'templates' | null;
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
  /** Collaborators editing the same document (live collaboration). */
  peers: PeerCursor[];
  /** Draw collaborators' carets. */
  showPeers: boolean;
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
  peers: [],
  showPeers: true,
});

export const createEditorStore = (language: Language, theme: ThemeSetting) =>
  createStore<EditorState>()(() => initialState(language, theme));

export type EditorStore = ReturnType<typeof createEditorStore>;
