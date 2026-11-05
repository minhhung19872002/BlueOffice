import type { DisplayCommand, EngineHost, PageDisplayList, Rgba } from '@blueoffice/platform';

/** CSS pixels per point at 100 % zoom (96 dpi screen). */
export const PX_PER_PT = 96 / 72;

/** The subset of CanvasRenderingContext2D the renderer needs (mockable in tests). */
export interface CanvasLike {
  save(): void;
  restore(): void;
  scale(x: number, y: number): void;
  translate(x: number, y: number): void;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void;
  fillRect(x: number, y: number, w: number, h: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  rect(x: number, y: number, w: number, h: number): void;
  stroke(): void;
  clip(): void;
  fill(path: unknown): void;
  fillStyle: unknown;
  strokeStyle: unknown;
  lineWidth: number;
}

export type PathFactory = (svgPath: string) => unknown;

const defaultPathFactory: PathFactory = (d) => new Path2D(d);

export const rgba = (c: Rgba): string => `rgba(${c[0]},${c[1]},${c[2]},${(c[3] / 255).toFixed(3)})`;

interface CachedGlyph {
  path: unknown | null;
  unitsPerEm: number;
}

/** Caches glyph outlines fetched from the engine as Path2D objects. */
export class GlyphStore {
  private glyphs = new Map<string, CachedGlyph>();

  constructor(
    private readonly engine: Pick<EngineHost, 'getGlyphOutlines'>,
    private readonly makePath: PathFactory = defaultPathFactory,
  ) {}

  get size(): number {
    return this.glyphs.size;
  }

  get(face: number, glyph: number): CachedGlyph | undefined {
    return this.glyphs.get(`${face}:${glyph}`);
  }

  /** Fetch every outline used by `page` that is not cached yet (one request per face). */
  async ensure(page: PageDisplayList): Promise<void> {
    const missing = new Map<number, Set<number>>();
    for (const c of page.commands) {
      if (c.op !== 'glyphs') continue;
      for (const g of c.glyphs) {
        if (!this.glyphs.has(`${c.face}:${g[0]}`)) {
          let s = missing.get(c.face);
          if (!s) missing.set(c.face, (s = new Set()));
          s.add(g[0]);
        }
      }
    }
    await Promise.all(
      [...missing].map(async ([face, ids]) => {
        const outlines = await this.engine.getGlyphOutlines(face, [...ids]);
        for (const o of outlines) {
          this.glyphs.set(`${o.face}:${o.glyph}`, {
            path: o.path ? this.makePath(o.path) : null,
            unitsPerEm: o.unitsPerEm,
          });
        }
      }),
    );
  }
}

/**
 * Draw a page display list. `scale` = device pixels per point.
 * The page background is painted white first (documents are always on paper).
 */
export function renderPage(
  ctx: CanvasLike,
  page: PageDisplayList,
  glyphs: GlyphStore,
  scale: number,
): void {
  ctx.save();
  ctx.setTransform(scale, 0, 0, scale, 0, 0);
  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, page.width, page.height);
  for (const cmd of page.commands) drawCommand(ctx, cmd, glyphs);
  ctx.restore();
}

function drawCommand(ctx: CanvasLike, cmd: DisplayCommand, glyphs: GlyphStore): void {
  switch (cmd.op) {
    case 'fillRect':
      ctx.fillStyle = rgba(cmd.color);
      ctx.fillRect(cmd.x, cmd.y, cmd.w, cmd.h);
      break;
    case 'line':
      ctx.strokeStyle = rgba(cmd.color);
      ctx.lineWidth = cmd.width;
      ctx.beginPath();
      ctx.moveTo(cmd.x1, cmd.y1);
      ctx.lineTo(cmd.x2, cmd.y2);
      ctx.stroke();
      break;
    case 'glyphs': {
      ctx.fillStyle = rgba(cmd.color);
      for (const g of cmd.glyphs) {
        const cached = glyphs.get(cmd.face, g[0]);
        if (!cached?.path) continue;
        const s = cmd.size / cached.unitsPerEm;
        ctx.save();
        ctx.translate(cmd.x + g[1], cmd.y + g[2]);
        ctx.scale(s, s);
        ctx.fill(cached.path);
        ctx.restore();
      }
      break;
    }
    case 'pushClip':
      ctx.save();
      ctx.beginPath();
      ctx.rect(cmd.x, cmd.y, cmd.w, cmd.h);
      ctx.clip();
      break;
    case 'popClip':
      ctx.restore();
      break;
  }
}
