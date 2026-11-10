import { describe, expect, it } from 'vitest';
import type { GlyphOutline, PageDisplayList, PathSeg } from '@blueoffice/platform';
import {
  GlyphStore,
  ImageStore,
  renderPage,
  rgba,
  segsToSvg,
  type CanvasLike,
} from '../src/renderer';

function recorder() {
  const ops: string[] = [];
  const ctx: CanvasLike = {
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 1,
    save: () => ops.push('save'),
    restore: () => ops.push('restore'),
    scale: (x, y) => ops.push(`scale ${x.toFixed(4)} ${y.toFixed(4)}`),
    translate: (x, y) => ops.push(`translate ${x} ${y}`),
    setTransform: (a, _b, _c, d) => ops.push(`setTransform ${a} ${d}`),
    fillRect: (x, y, w, h) => ops.push(`fillRect ${x} ${y} ${w} ${h} ${String(ctx.fillStyle)}`),
    beginPath: () => ops.push('beginPath'),
    moveTo: (x, y) => ops.push(`moveTo ${x} ${y}`),
    lineTo: (x, y) => ops.push(`lineTo ${x} ${y}`),
    rect: (x, y, w, h) => ops.push(`rect ${x} ${y} ${w} ${h}`),
    stroke: (p?: unknown) => ops.push(p ? `stroke ${String(p)}` : 'stroke'),
    clip: (p?: unknown) => ops.push(p ? `clip ${String(p)}` : 'clip'),
    transform: (a, b, c, d, e, f) => ops.push(`transform ${[a, b, c, d, e, f].join(' ')}`),
    createLinearGradient: () => ({
      addColorStop: (o: number, c: string) => ops.push(`stop ${o} ${c}`),
      toString: () => 'gradient',
    }),
    fill: (p) => ops.push(`fill ${String(p)}`),
    setLineDash: (d) => ops.push(`dash ${d.join(',')}`),
    rotate: (a) => ops.push(`rotate ${a.toFixed(3)}`),
    drawImage: (img, sx, sy, sw, sh, dx, dy, dw, dh) =>
      ops.push(`drawImage ${String(img)} ${sx} ${sy} ${sw} ${sh} ${dx} ${dy} ${dw} ${dh}`),
  };
  return { ctx, ops };
}

const page: PageDisplayList = {
  width: 100,
  height: 200,
  commands: [
    {
      op: 'glyphs',
      face: 0,
      size: 10,
      x: 5,
      y: 20,
      color: [255, 0, 0, 255],
      text: 'AB',
      glyphs: [
        [36, 0, 0, 0],
        [37, 6, 0, 1],
      ],
    },
    { op: 'line', x1: 5, y1: 22, x2: 17, y2: 22, width: 0.5, color: [0, 0, 0, 255] },
    { op: 'pushClip', x: 0, y: 0, w: 50, h: 50 },
    { op: 'fillRect', x: 1, y: 2, w: 3, h: 4, color: [0, 0, 255, 128] },
    { op: 'popClip' },
  ],
};

describe('canvas renderer', () => {
  it('fetches each missing glyph once per face', async () => {
    const requests: number[][] = [];
    const store = new GlyphStore(
      {
        getGlyphOutlines: async (face, glyphs): Promise<GlyphOutline[]> => {
          requests.push(glyphs);
          return glyphs.map((g) => ({
            face,
            glyph: g,
            unitsPerEm: 1000,
            path: g === 37 ? '' : `P${g}`,
          }));
        },
      },
      (d) => `path(${d})`,
    );
    await store.ensure(page);
    await store.ensure(page);
    expect(requests).toEqual([[36, 37]]);
    expect(store.get(0, 36)?.path).toBe('path(P36)');
    expect(store.get(0, 37)?.path).toBeNull();
  });

  it('draws commands in points scaled to device pixels', async () => {
    const store = new GlyphStore(
      {
        getGlyphOutlines: async (face, glyphs) =>
          glyphs.map((g) => ({ face, glyph: g, unitsPerEm: 1000, path: `P${g}` })),
      },
      (d) => d,
    );
    await store.ensure(page);
    const { ctx, ops } = recorder();
    renderPage(ctx, page, store, 2);
    expect(ops[0]).toBe('save');
    expect(ops[1]).toBe('setTransform 2 2');
    expect(ops[2]).toBe('fillRect 0 0 100 200 #ffffff');
    expect(ops).toContain('translate 5 20');
    expect(ops).toContain('scale 0.0100 0.0100');
    expect(ops).toContain('fill P36');
    expect(ops).toContain('translate 11 20');
    expect(ops).toContain('fillRect 1 2 3 4 rgba(0,0,255,0.502)');
    // Clip is balanced.
    expect(ops.filter((o) => o === 'save').length).toBe(ops.filter((o) => o === 'restore').length);
  });

  it('draws pictures (crop, rotation), placeholders and dashed lines', async () => {
    const pic: PageDisplayList = {
      width: 100,
      height: 100,
      commands: [
        {
          op: 'image',
          x: 10,
          y: 10,
          w: 20,
          h: 10,
          image: '/a.png',
          crop: [0.5, 0, 0, 0],
          rotation: 90,
        },
        { op: 'image', x: 0, y: 50, w: 5, h: 5, image: '/b.emf' },
        { op: 'line', x1: 0, y1: 0, x2: 9, y2: 0, width: 1, color: [0, 0, 0, 255], dash: [3, 2] },
      ],
    };
    const fetched: string[] = [];
    const images = new ImageStore(
      {
        getMedia: async (m) => {
          fetched.push(m);
          return m === '/a.png' ? new Uint8Array([1]) : new Uint8Array();
        },
      },
      async () => ({ image: 'IMG', width: 40, height: 20 }),
    );
    await images.ensure(pic);
    await images.ensure(pic);
    expect(fetched).toEqual(['/a.png', '/b.emf']);
    const { ctx, ops } = recorder();
    renderPage(ctx, pic, new GlyphStore({ getGlyphOutlines: async () => [] }), 1, images);
    expect(ops).toContain('translate 20 15');
    expect(ops).toContain('rotate 1.571');
    expect(ops).toContain('drawImage IMG 20 0 20 20 -10 -5 20 10');
    expect(ops).toContain('fillRect -2.5 -2.5 5 5 #e0e0e0');
    expect(ops).toContain('dash 3,2');
    expect(ops).toContain('dash ');
  });

  it('releases decoded pictures when 100 documents are opened and closed (CORE-PERF-001)', async () => {
    // Fake engine + decoder counting live bitmap handles (ImageBitmap.close()).
    let live = 0;
    let peak = 0;
    const decode = async () => {
      live++;
      peak = Math.max(peak, live);
      let open = true;
      const bmp = {
        close: () => {
          if (open) live--;
          open = false;
        },
      };
      return { image: bmp, width: 10, height: 10 };
    };
    const page = (doc: number): PageDisplayList => ({
      width: 100,
      height: 100,
      commands: Array.from({ length: 5 }, (_, k) => ({
        op: 'image' as const,
        image: `/doc${doc}/image${k}.png`,
        x: 0,
        y: 0,
        w: 10,
        h: 10,
      })),
    });
    const images = new ImageStore({ getMedia: async () => new Uint8Array([1]) }, decode);
    for (let doc = 0; doc < 100; doc++) {
      await images.ensure(page(doc));
      expect(images.size).toBe(5);
      // Close (or open the next document) while a decode is still in flight.
      const late = images.ensure(page(doc + 1000));
      images.clear();
      await late;
      expect(images.size).toBe(0);
    }
    expect(live).toBe(0);
    expect(peak).toBeLessThanOrEqual(10);
  });

  it('draws slide paths, gradients, transforms and clip paths', async () => {
    const { ctx, ops } = recorder();
    const glyphs = new GlyphStore({ getGlyphOutlines: async () => [] }, (d) => `P(${d})`);
    const tri: PathSeg[] = [{ M: [0, 0] }, { L: [10, 0] }, { C: [1, 2, 3, 4, 5, 6] }, 'Z'];
    expect(segsToSvg(tri)).toBe('M0 0L10 0C1 2 3 4 5 6Z');
    renderPage(
      ctx,
      {
        width: 10,
        height: 10,
        commands: [
          { op: 'pushTransform', m: [1, 0, 0, 1, 50, 50] },
          { op: 'pushClipPath', segs: [{ M: [0, 0] }, { L: [5, 5] }, 'Z'] },
          {
            op: 'path',
            segs: tri,
            fill: {
              type: 'linear',
              x1: 0,
              y1: 0,
              x2: 10,
              y2: 0,
              stops: [{ offset: 0, color: [255, 0, 0, 255] }],
            },
            stroke: { width: 2, color: [0, 0, 0, 255], dash: [3, 1], cap: 'round', join: 'miter' },
          },
          { op: 'popClip' },
          { op: 'popTransform' },
        ],
      },
      glyphs,
      1,
    );
    expect(ops).toContain('transform 1 0 0 1 50 50');
    expect(ops).toContain('clip P(M0 0L5 5Z)');
    expect(ops).toContain('stop 0 rgba(255,0,0,1.000)');
    expect(ops).toContain('fill P(M0 0L10 0C1 2 3 4 5 6Z)');
    expect(ops).toContain('stroke P(M0 0L10 0C1 2 3 4 5 6Z)');
    expect(ops).toContain('dash 3,1');
    expect(ctx.lineCap).toBe('round');
    // Every push is balanced by a restore.
    expect(ops.filter((o) => o === 'save').length).toBe(ops.filter((o) => o === 'restore').length);
  });

  it('draws blurred shadows and picture opacity', () => {
    const { ctx, ops } = recorder();
    const glyphs = new GlyphStore({ getGlyphOutlines: async () => [] }, (d) => `P(${d})`);
    const c = ctx as typeof ctx & { globalAlpha?: number; filter?: string };
    const seen: (string | number | undefined)[] = [];
    const fill = c.fill.bind(c);
    c.fill = (p: unknown) => {
      seen.push(c.filter, c.globalAlpha);
      fill(p);
    };
    renderPage(
      c,
      {
        width: 10,
        height: 10,
        commands: [
          {
            op: 'blurPath',
            segs: [{ M: [0, 0] }, { L: [5, 5] }, 'Z'],
            color: [0, 0, 0, 128],
            radius: 4,
          },
          { op: 'pushOpacity', alpha: 0.5 },
          {
            op: 'path',
            segs: [{ M: [0, 0] }, 'Z'],
            fill: { type: 'solid', color: [1, 2, 3, 255] },
            stroke: null,
          },
          { op: 'popOpacity' },
        ],
      },
      glyphs,
      2,
    );
    expect(seen[0]).toBe('blur(2px)');
    expect(seen[3]).toBe(0.5);
    expect(ops.filter((o) => o === 'save').length).toBe(ops.filter((o) => o === 'restore').length);
  });

  it('draws a glow as a grown, blurred outline', () => {
    const { ctx } = recorder();
    const glyphs = new GlyphStore({ getGlyphOutlines: async () => [] }, (d) => `P(${d})`);
    const c = ctx as typeof ctx & { filter?: string };
    const strokes: (string | number | undefined)[] = [];
    const stroke = c.stroke.bind(c);
    c.stroke = (p?: unknown) => {
      strokes.push(c.filter, c.lineWidth);
      stroke(p);
    };
    renderPage(
      c,
      {
        width: 10,
        height: 10,
        commands: [
          {
            op: 'blurPath',
            segs: [{ M: [0, 0] }, { L: [5, 5] }, 'Z'],
            color: [255, 192, 0, 160],
            radius: 8,
            spread: 4,
          },
        ],
      },
      glyphs,
      1,
    );
    expect(strokes).toEqual(['blur(4px)', 8]);
  });

  it('formats colours', () => {
    expect(rgba([1, 2, 3, 255])).toBe('rgba(1,2,3,1.000)');
  });
});
