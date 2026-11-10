export type AppId = 'doc' | 'sheet' | 'slide';

/** Legacy route segments and the app they now map to. */
const LEGACY: Record<string, AppId> = {
  write: 'doc',
  docs: 'doc',
  sheets: 'sheet',
  slides: 'slide',
};

/**
 * Phone remote (`/remote/<pairing code>`) and online presentation viewer (`/watch/<link token>`)
 * pages of BlueSlide (SLIDE-REMOTE-001); `null` for other paths.
 */
export function presentRoute(
  pathname: string,
  base = '/',
): { kind: 'remote' | 'watch'; token: string } | null {
  const rel = pathname.startsWith(base) ? pathname.slice(base.length) : pathname.replace(/^\//, '');
  const m = /^(remote|watch)\/([A-Za-z0-9_-]{16,64})\/?$/.exec(rel);
  return m ? { kind: m[1] as 'remote' | 'watch', token: m[2]! } : null;
}

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
