/**
 * Boundary datasets of map charts (CORE-CHART-006): folders of `assets/maps/<id>/` copied next to
 * the app (`maps/<id>/` under the base URL by `scripts/sync-assets.mjs`) holding `dataset.json`, the
 * GeoJSON / TopoJSON it names and an optional alias file. The engine parses and registers them
 * (`registerMapDataset`); this module only reads the files. A missing folder is not an error: the
 * Vietnamese province dataset ships only once its licence is approved.
 */

/** The texts of one dataset folder, as the engine's `registerMapDataset` takes them. */
export interface MapDatasetFiles {
  /** Text of `dataset.json`. */
  manifest: string;
  /** Text of the boundary file. */
  boundaries: string;
  /** Text of the alias file, when the manifest names one. */
  aliases?: string;
}

/** Reads dataset folders. */
export interface MapDatasetSource {
  /** The files of dataset `id`, or `null` when the folder has none. */
  load(id: string): Promise<MapDatasetFiles | null>;
}

/** Dataset folders a sheet editor looks for. */
export const MAP_DATASETS: readonly string[] = ['vn-provinces'];

/** A plain file name of the folder (no path: the files stay inside the dataset folder). */
function fileName(v: unknown): string | null {
  return typeof v === 'string' && v !== '' && !/[\\/]|\.\./.test(v) ? v : null;
}

/** Dataset folders under `baseUrl` + `maps/` (web assets, desktop bundled front end). */
export function assetMapDatasets(
  baseUrl: string,
  fetchImpl: (url: string) => Promise<Response> = (u) => fetch(u),
): MapDatasetSource {
  const base = baseUrl.endsWith('/') ? baseUrl : `${baseUrl}/`;
  const text = async (url: string): Promise<string | null> => {
    const r = await fetchImpl(url);
    if (!r.ok) return null;
    const t = await r.text();
    // A single-page host may answer any path with its HTML page.
    return /^\s*</.test(t) ? null : t;
  };
  return {
    async load(id) {
      if (!/^[a-z0-9-]+$/.test(id)) return null;
      const dir = `${base}maps/${id}/`;
      let manifest: string | null;
      try {
        manifest = await text(`${dir}dataset.json`);
      } catch {
        return null;
      }
      if (manifest === null) return null;
      let meta: { file?: unknown; aliases?: unknown };
      try {
        meta = JSON.parse(manifest) as { file?: unknown; aliases?: unknown };
      } catch {
        // The engine reports the error when registering.
        return { manifest, boundaries: '' };
      }
      const file = fileName(meta.file);
      const boundaries = file ? await text(`${dir}${file}`) : null;
      const aliasFile = fileName(meta.aliases);
      const aliases = aliasFile ? await text(`${dir}${aliasFile}`) : null;
      return {
        manifest,
        boundaries: boundaries ?? '',
        ...(aliases !== null ? { aliases } : {}),
      };
    },
  };
}
