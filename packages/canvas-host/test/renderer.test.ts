import { describe, expect, it } from 'vitest';
import type { GlyphOutline, PageDisplayList } from '@blueoffice/platform';
import { GlyphStore, ImageStore, renderPage, rgba, type CanvasLike } from '../src/renderer';

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
    stroke: () => ops.push('stroke'),
    clip: () => ops.push('clip'),
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

  it('formats colours', () => {
    expect(rgba([1, 2, 3, 255])).toBe('rgba(1,2,3,1.000)');
  });
});
