/**
 * BlueDrive sync folders on the desktop (prompt §6A.3): a local file inside a
 * configured sync folder is a host file; the desktop offers to switch to
 * online collaboration and resolves the file id from its relative path
 * (`GET /api/hosts/{host}/resolve?path=`, docs/bohp.md §A2).
 */

/** A local folder kept in sync with a host (by the host's sync client). */
export interface SyncFolder {
  host: string;
  /** Absolute local path of the sync root. */
  path: string;
}

/** Path with '/' separators, no trailing separator, no `.` segments. */
function normalize(p: string): string[] {
  return p
    .replace(/\\/g, '/')
    .split('/')
    .filter((s, i) => s !== '' || i === 0)
    .filter((s) => s !== '.');
}

/** Windows (drive letter / UNC) paths compare case-insensitively. */
const windowsLike = (p: string): boolean => /^[a-zA-Z]:[\\/]/.test(p) || p.startsWith('\\\\');

/**
 * The sync folder containing `filePath` and the '/'-separated path relative
 * to it, or null. The most specific (longest) folder wins; `..` anywhere
 * disqualifies the file.
 */
export function syncFolderOf(
  filePath: string,
  folders: readonly SyncFolder[],
): { folder: SyncFolder; relative: string } | null {
  const file = normalize(filePath);
  if (file.includes('..')) return null;
  let best: { folder: SyncFolder; relative: string; depth: number } | null = null;
  for (const folder of folders) {
    const root = normalize(folder.path);
    if (root.length === 0 || root.includes('..') || file.length <= root.length) continue;
    const insensitive = windowsLike(folder.path) || windowsLike(filePath);
    const same = (a: string, b: string) =>
      insensitive ? a.toLowerCase() === b.toLowerCase() : a === b;
    if (!root.every((seg, i) => same(seg, file[i] ?? ''))) continue;
    if (!best || root.length > best.depth) {
      best = { folder, relative: file.slice(root.length).join('/'), depth: root.length };
    }
  }
  return best ? { folder: best.folder, relative: best.relative } : null;
}

/** Parses the stored setting (JSON array), dropping malformed entries. */
export function parseSyncFolders(json: string | null): SyncFolder[] {
  if (!json) return [];
  try {
    const v: unknown = JSON.parse(json);
    if (!Array.isArray(v)) return [];
    return v.flatMap((e: unknown) => {
      const o = e as Record<string, unknown> | null;
      return o && typeof o.host === 'string' && typeof o.path === 'string' && o.path.trim()
        ? [{ host: o.host, path: o.path.trim() }]
        : [];
    });
  } catch {
    return [];
  }
}
