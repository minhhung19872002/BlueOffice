import { describe, expect, it } from 'vitest';
import { assetMapDatasets } from '../src/mapDatasets';

const response = (status: number, body: string) =>
  ({ ok: status >= 200 && status < 300, status, text: async () => body }) as Response;

describe('map chart boundary datasets (CORE-CHART-006)', () => {
  it('reads dataset.json and the files it names', async () => {
    const files: Record<string, string> = {
      '/app/maps/vn-provinces/dataset.json': JSON.stringify({
        file: 'provinces.geojson',
        aliases: 'aliases.json',
      }),
      '/app/maps/vn-provinces/provinces.geojson': '{"type":"FeatureCollection","features":[]}',
      '/app/maps/vn-provinces/aliases.json': '{"A":"B"}',
    };
    const asked: string[] = [];
    const src = assetMapDatasets('/app', async (u) => {
      asked.push(u);
      return files[u] ? response(200, files[u]) : response(404, '');
    });
    const got = await src.load('vn-provinces');
    expect(got).toEqual({
      manifest: files['/app/maps/vn-provinces/dataset.json'],
      boundaries: files['/app/maps/vn-provinces/provinces.geojson'],
      aliases: '{"A":"B"}',
    });
    expect(asked).toHaveLength(3);
  });

  it('a missing folder (404 or an HTML fallback page) is no dataset', async () => {
    expect(await assetMapDatasets('/', async () => response(404, '')).load('vn-provinces')).toBe(
      null,
    );
    const html = assetMapDatasets('/', async () => response(200, '<!doctype html><html>'));
    expect(await html.load('vn-provinces')).toBe(null);
    const failing = assetMapDatasets('/', async () => {
      throw new Error('offline');
    });
    expect(await failing.load('vn-provinces')).toBe(null);
  });

  it('never leaves the dataset folder', async () => {
    const asked: string[] = [];
    const src = assetMapDatasets('/', async (u) => {
      asked.push(u);
      return u.endsWith('dataset.json')
        ? response(200, JSON.stringify({ file: '../secret.json' }))
        : response(200, 'x');
    });
    expect(await src.load('../x')).toBe(null);
    const got = await src.load('vn-provinces');
    expect(got?.boundaries).toBe('');
    expect(asked).toEqual(['/maps/vn-provinces/dataset.json']);
  });
});
