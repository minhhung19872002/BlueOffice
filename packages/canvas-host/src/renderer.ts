import type {
  DisplayCommand,
  EngineHost,
  PageDisplayList,
  Paint,
  PathSeg,
  Rgba,
} from '@blueoffice/platform';

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
  /** Without argument: the current path; with a Path2D: that path. */
  stroke(path?: unknown): void;
  clip(path?: unknown): void;
  fill(path: unknown): void;
  fillStyle: unknown;
  strokeStyle: unknown;
  lineWidth: number;
  setLineDash?(segments: number[]): void;
  rotate?(angle: number): void;
  /** Multiply the current transform (slides: shape/group transforms). */
  transform?(a: number, b: number, c: number, d: number, e: number, f: number): void;
  createLinearGradient?(x0: number, y0: number, x1: number, y1: number): GradientLike;
  createRadialGradient?(
    x0: number,
    y0: number,
    r0: number,
    x1: number,
    y1: number,
    r1: number,
  ): GradientLike;
  lineCap?: string;
  lineJoin?: string;
  /** Group opacity (pictures with alphaModFix). */
  globalAlpha?: number;
  /** CSS filter (blurred shadows). */
  filter?: string;
  getTransform?(): { a: number; b: number };
  drawImage?(
    img: unknown,
    sx: number,
    sy: number,
    sw: number,
    sh: number,
    dx: number,
    dy: number,
    dw: number,
    dh: number,
  ): void;
}

/** A decoded picture. */
export interface LoadedImage {
  image: unknown;
  width: number;
  height: number;
}

export type ImageDecoder = (bytes: Uint8Array) => Promise<LoadedImage | null>;

const defaultDecoder: ImageDecoder = async (bytes) => {
  if (typeof createImageBitmap === 'undefined') return null;
  try {
    const bmp = await createImageBitmap(new Blob([bytes as BlobPart]));
    return { image: bmp, width: bmp.width, height: bmp.height };
  } catch {
    return null; // unsupported format (EMF/WMF…): placeholder box
  }
};

/** Caches pictures referenced by `image` commands (bytes fetched from the engine). */
export class ImageStore {
  private images = new Map<string, LoadedImage | null>();
  /** Bumped by `clear`: decodes started for an earlier document are dropped. */
  private generation = 0;

  constructor(
    private readonly engine: Pick<EngineHost, 'getMedia'>,
    private readonly decode: ImageDecoder = defaultDecoder,
  ) {}

  get(media: string): LoadedImage | null | undefined {
    return this.images.get(media);
  }

  /** Number of decoded pictures held (tests, CORE-PERF-001). */
  get size(): number {
    return this.images.size;
  }

  /**
   * Forget everything (new document, view closed). Decoded `ImageBitmap`s are closed so
   * their pixel memory is released now rather than whenever the GC runs.
   */
  clear(): void {
    this.generation++;
    for (const img of this.images.values()) release(img);
    this.images.clear();
  }

  async ensure(page: PageDisplayList): Promise<void> {
    const missing = new Set<string>();
    for (const c of page.commands) {
      if (c.op === 'image' && !this.images.has(c.image)) missing.add(c.image);
    }
    const gen = this.generation;
    await Promise.all(
      [...missing].map(async (m) => {
        const bytes = await this.engine.getMedia(m);
        const img = bytes.length ? await this.decode(bytes) : null;
        // Cleared meanwhile (another document) or decoded twice: keep nothing stale.
        if (gen !== this.generation || this.images.has(m)) release(img);
        else this.images.set(m, img);
      }),
    );
  }
}

/** Close a decoded picture that has a `close()` (ImageBitmap). */
function release(img: LoadedImage | null | undefined): void {
  const bmp = img?.image as { close?: () => void } | null | undefined;
  if (bmp && typeof bmp.close === 'function') bmp.close();
}

export type PathFactory = (svgPath: string) => unknown;

export interface GradientLike {
  addColorStop(offset: number, color: string): void;
}

/** `bo_render::PathSeg` list → SVG path data. */
export function segsToSvg(segs: PathSeg[]): string {
  const out: string[] = [];
  for (const s of segs) {
    if (s === 'Z') out.push('Z');
    else if ('M' in s) out.push(`M${s.M[0]} ${s.M[1]}`);
    else if ('L' in s) out.push(`L${s.L[0]} ${s.L[1]}`);
    else if ('Q' in s) out.push(`Q${s.Q.join(' ')}`);
    else out.push(`C${s.C.join(' ')}`);
  }
  return out.join('');
}

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

  /** Builds a path object from SVG path data (shared with shape drawing). */
  path(d: string): unknown {
    return this.makePath(d);
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
  images?: ImageStore,
): void {
  ctx.save();
  ctx.setTransform(scale, 0, 0, scale, 0, 0);
  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, page.width, page.height);
  for (const cmd of page.commands) drawCommand(ctx, cmd, glyphs, images);
  ctx.restore();
}

function drawCommand(
  ctx: CanvasLike,
  cmd: DisplayCommand,
  glyphs: GlyphStore,
  images?: ImageStore,
): void {
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
      if (cmd.dash) ctx.setLineDash?.(cmd.dash);
      ctx.stroke();
      if (cmd.dash) ctx.setLineDash?.([]);
      break;
    case 'image': {
      const img = images?.get(cmd.image);
      ctx.save();
      ctx.translate(cmd.x + cmd.w / 2, cmd.y + cmd.h / 2);
      if (cmd.rotation) ctx.rotate?.((cmd.rotation * Math.PI) / 180);
      if (img && ctx.drawImage) {
        const [l, t, r, b] = cmd.crop ?? [0, 0, 0, 0];
        const sw = img.width * Math.max(0.0001, 1 - l - r);
        const sh = img.height * Math.max(0.0001, 1 - t - b);
        ctx.drawImage(
          img.image,
          img.width * l,
          img.height * t,
          sw,
          sh,
          -cmd.w / 2,
          -cmd.h / 2,
          cmd.w,
          cmd.h,
        );
      } else {
        ctx.fillStyle = '#e0e0e0';
        ctx.fillRect(-cmd.w / 2, -cmd.h / 2, cmd.w, cmd.h);
      }
      ctx.restore();
      break;
    }
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
    case 'path': {
      const path = glyphs.path(segsToSvg(cmd.segs));
      if (cmd.fill) {
        ctx.fillStyle = paintStyle(ctx, cmd.fill);
        ctx.fill(path);
      }
      if (cmd.stroke && cmd.stroke.width > 0) {
        ctx.strokeStyle = rgba(cmd.stroke.color);
        ctx.lineWidth = cmd.stroke.width;
        ctx.lineCap = cmd.stroke.cap;
        ctx.lineJoin = cmd.stroke.join;
        ctx.setLineDash?.(cmd.stroke.dash);
        ctx.stroke(path);
        ctx.setLineDash?.([]);
      }
      break;
    }
    case 'pushTransform':
      ctx.save();
      ctx.transform?.(...cmd.m);
      break;
    case 'popTransform':
      ctx.restore();
      break;
    case 'pushClipPath':
      ctx.save();
      ctx.clip(glyphs.path(segsToSvg(cmd.segs)));
      break;
    case 'pushOpacity':
      ctx.save();
      ctx.globalAlpha = (ctx.globalAlpha ?? 1) * cmd.alpha;
      break;
    case 'popOpacity':
      ctx.restore();
      break;
    case 'blurPath': {
      // Canvas filter blur (CSS px of the device): radius points × current scale.
      ctx.save();
      const m = ctx.getTransform?.() ?? { a: 1, b: 0 };
      const scale = Math.hypot(m.a, m.b);
      ctx.filter = `blur(${(cmd.radius * scale) / 2}px)`;
      ctx.fillStyle = rgba(cmd.color);
      const path = glyphs.path(segsToSvg(cmd.segs));
      ctx.fill(path);
      if (cmd.spread) {
        // Glow (§20.1.8.32): the outline grown by `spread` before the blur.
        ctx.strokeStyle = rgba(cmd.color);
        ctx.lineWidth = cmd.spread * 2;
        ctx.lineJoin = 'round';
        ctx.stroke(path);
      }
      ctx.restore();
      break;
    }
  }
}

function paintStyle(ctx: CanvasLike, p: Paint): unknown {
  if (p.type === 'solid') return rgba(p.color);
  const g =
    p.type === 'linear'
      ? ctx.createLinearGradient?.(p.x1, p.y1, p.x2, p.y2)
      : ctx.createRadialGradient?.(p.cx, p.cy, 0, p.cx, p.cy, p.r);
  if (!g) return rgba(p.stops[0]?.color ?? [0, 0, 0, 255]);
  for (const s of p.stops) g.addColorStop(Math.min(1, Math.max(0, s.offset)), rgba(s.color));
  return g;
}
