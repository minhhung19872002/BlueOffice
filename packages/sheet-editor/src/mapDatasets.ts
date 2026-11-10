/**
 * Map chart boundary datasets (CORE-CHART-006): the folders the platform finds (the Vietnamese
 * provinces once an approved dataset is dropped into `assets/maps/vn-provinces/`) are registered
 * with the engine through the command bridge (`registerMapDataset`), once per engine.
 */
import { MAP_DATASETS, type MapDatasetSource, type SheetHost } from '@blueoffice/platform';

const done = new WeakMap<object, Promise<string[]>>();

/** Registers the datasets `source` has; resolves to the ids registered (errors are skipped). */
export function registerMapDatasets(
  host: Pick<SheetHost, 'exec'>,
  source: MapDatasetSource | undefined,
  ids: readonly string[] = MAP_DATASETS,
): Promise<string[]> {
  if (!source) return Promise.resolve([]);
  const known = done.get(host);
  if (known) return known;
  const run = (async () => {
    const out: string[] = [];
    for (const id of ids) {
      try {
        const files = await source.load(id);
        if (!files) continue;
        const r = await host.exec<{ id: string }>({ cmd: 'registerMapDataset', ...files });
        out.push(r.id);
      } catch (e) {
        // A broken dataset leaves province maps on their "dataset needed" message.
        console.warn(`map dataset ${id}:`, e);
      }
    }
    return out;
  })();
  done.set(host, run);
  return run;
}
