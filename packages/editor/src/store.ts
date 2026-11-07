import type { Language } from '@blueoffice/i18n';
import type { DocInfo, EditResult, ReviewState } from '@blueoffice/platform';
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
  panel: 'find' | 'replace' | 'symbols' | 'review' | 'comments' | null;
  /** Name of a document with an autosave waiting to be restored. */
  recovery: string | null;
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
  panel: null,
  recovery: null,
});

export const createEditorStore = (language: Language, theme: ThemeSetting) =>
  createStore<EditorState>()(() => initialState(language, theme));

export type EditorStore = ReturnType<typeof createEditorStore>;
