export type AppId = 'doc' | 'sheet' | 'slide';

/** Legacy route segments and the app they now map to. */
const LEGACY: Record<string, AppId> = {
  write: 'doc',
  docs: 'doc',
  sheets: 'sheet',
  slides: 'slide',
};

/**
 * Map a pathname to an app (`/doc`, `/sheet`, `/slide`); `/` opens BlueDoc.
 * Legacy routes return a `redirect` to the canonical path.
 */
export function resolveRoute(pathname: string, base = '/'): { app: AppId; redirect?: string } {
  const rel = pathname.startsWith(base) ? pathname.slice(base.length) : pathname.replace(/^\//, '');
  const seg = rel.split('/')[0] ?? '';
  if (seg === 'doc' || seg === 'sheet' || seg === 'slide') return { app: seg };
  const legacy = LEGACY[seg];
  if (legacy) return { app: legacy, redirect: `${base}${legacy}` };
  return { app: 'doc', redirect: seg === '' ? `${base}doc` : undefined };
}
