/**
 * The icon library data: only reached through the dynamic import of `loadIconLibrary`, so
 * the manifest and the SVG documents stay out of the main bundle. `data.json` is written by
 * `scripts/gen-icon-library.mjs` from the same drawings as `assets/icons/library/` (a test
 * compares the two).
 */
import data from './data.json';
import type { IconLibrary, IconLibraryManifest } from './catalog';

const svg: Record<string, string> = data.svg;

export const ICON_LIBRARY: IconLibrary = {
  manifest: data.manifest as IconLibraryManifest,
  svg: (id) => (Object.hasOwn(svg, id) ? svg[id] : undefined),
};
