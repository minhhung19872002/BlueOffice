/**
 * Files pinned on the File › Open page (CORE-UI-002), per app, remembered for the
 * user in the platform's setting store (`ShellIntegration.getSetting` /
 * `setSetting`), or in `localStorage` where the platform has none.
 */
import { useCallback, useEffect, useState } from 'react';

/** The part of the platform shell that stores small user settings. */
export interface SettingStore {
  getSetting?(key: string): Promise<string | null>;
  setSetting?(key: string, value: string | null): Promise<void>;
}

export interface PinnedFile {
  name: string;
  path: string;
}

export const pinnedKey = (app: string) => `blueoffice.pinned.${app}`;

function parse(text: string | null): PinnedFile[] {
  if (!text) return [];
  try {
    const v: unknown = JSON.parse(text);
    if (!Array.isArray(v)) return [];
    return v.filter(
      (f): f is PinnedFile =>
        typeof f === 'object' &&
        f !== null &&
        typeof (f as PinnedFile).name === 'string' &&
        typeof (f as PinnedFile).path === 'string',
    );
  } catch {
    return [];
  }
}

export async function loadPinned(store: SettingStore, app: string): Promise<PinnedFile[]> {
  const key = pinnedKey(app);
  if (store.getSetting) return parse(await store.getSetting(key));
  try {
    return parse(globalThis.localStorage?.getItem(key) ?? null);
  } catch {
    return [];
  }
}

export async function savePinned(
  store: SettingStore,
  app: string,
  files: PinnedFile[],
): Promise<void> {
  const key = pinnedKey(app);
  const text = files.length ? JSON.stringify(files) : null;
  if (store.setSetting) {
    await store.setSetting(key, text);
    return;
  }
  try {
    if (text) globalThis.localStorage?.setItem(key, text);
    else globalThis.localStorage?.removeItem(key);
  } catch {
    // Storage blocked: the pin lasts for this session only.
  }
}

/** `files` with `file` pinned (first) or unpinned. */
export function togglePinned(files: readonly PinnedFile[], file: PinnedFile): PinnedFile[] {
  return files.some((f) => f.path === file.path)
    ? files.filter((f) => f.path !== file.path)
    : [file, ...files];
}

/** Pinned files of `app` and a toggle that stores them. */
export function usePinnedFiles(store: SettingStore, app: string) {
  const [pinned, setPinned] = useState<PinnedFile[]>([]);
  useEffect(() => {
    let live = true;
    void loadPinned(store, app).then(
      (p) => live && setPinned(p),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [store, app]);
  const toggle = useCallback(
    (file: PinnedFile) => {
      setPinned((cur) => {
        const next = togglePinned(cur, file);
        void savePinned(store, app, next).catch(() => {});
        return next;
      });
    },
    [store, app],
  );
  return { pinned, toggle };
}
