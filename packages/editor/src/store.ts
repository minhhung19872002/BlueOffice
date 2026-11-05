import type { Language } from '@blueoffice/i18n';
import type { DocInfo } from '@blueoffice/platform';
import type { ThemeSetting } from '@blueoffice/ui';
import { createStore } from 'zustand/vanilla';

export type Status =
  | { kind: 'idle' }
  | { kind: 'busy'; key: 'status.loading' | 'status.exporting' }
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
}

export const initialState = (language: Language, theme: ThemeSetting): EditorState => ({
  doc: null,
  page: 0,
  zoom: 1,
  theme,
  language,
  status: { kind: 'idle' },
  engineReady: false,
});

export const createEditorStore = (language: Language, theme: ThemeSetting) =>
  createStore<EditorState>()(() => initialState(language, theme));

export type EditorStore = ReturnType<typeof createEditorStore>;
