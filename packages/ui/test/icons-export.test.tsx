// The icon set exported for the self-drawn Rust interface (ADR-0018): each icon as SVG path data
// on its 24×24 grid. `BO_WRITE_ICONS=1 pnpm --filter @blueoffice/ui test` rewrites the file.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ICONS } from '../src/icons/Icon';

const jsonPath = fileURLToPath(new URL('../../../assets/icons/ui-icons.json', import.meta.url));

interface Part {
  /** SVG path data (shapes converted to paths). */
  d: string;
  /** Filled with the icon colour. */
  fill?: boolean;
  /** Stroke: the icon colour ("current"), the colour on an accent fill ("on") or none. */
  stroke: 'current' | 'on' | 'none';
  /** Drawn in the app accent. */
  accent?: boolean;
}

const num = (v: string | undefined) => Number(v ?? 0);

function attrs(tag: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of tag.matchAll(/([a-zA-Z][a-zA-Z0-9-]*)="([^"]*)"/g)) out[m[1]!] = m[2]!;
  return out;
}

function shapePath(name: string, a: Record<string, string>): string | null {
  switch (name) {
    case 'path':
      return a.d ?? null;
    case 'line':
      return `M${a.x1} ${a.y1}L${a.x2} ${a.y2}`;
    case 'polyline':
    case 'polygon': {
      const pts = (a.points ?? '').trim().split(/[\s,]+/);
      let d = '';
      for (let i = 0; i + 1 < pts.length; i += 2) d += `${i ? 'L' : 'M'}${pts[i]} ${pts[i + 1]}`;
      return name === 'polygon' ? `${d}Z` : d;
    }
    case 'circle':
    case 'ellipse': {
      const [cx, cy] = [num(a.cx), num(a.cy)];
      const [rx, ry] = name === 'circle' ? [num(a.r), num(a.r)] : [num(a.rx), num(a.ry)];
      return `M${cx - rx} ${cy}A${rx} ${ry} 0 1 0 ${cx + rx} ${cy}A${rx} ${ry} 0 1 0 ${cx - rx} ${cy}Z`;
    }
    case 'rect': {
      const [x, y, w, h] = [num(a.x), num(a.y), num(a.width), num(a.height)];
      const r = Math.min(num(a.rx ?? a.ry), w / 2, h / 2);
      if (!r) return `M${x} ${y}h${w}v${h}h${-w}Z`;
      return (
        `M${x + r} ${y}h${w - 2 * r}a${r} ${r} 0 0 1 ${r} ${r}v${h - 2 * r}` +
        `a${r} ${r} 0 0 1 ${-r} ${r}h${-(w - 2 * r)}a${r} ${r} 0 0 1 ${-r} ${-r}` +
        `v${-(h - 2 * r)}a${r} ${r} 0 0 1 ${r} ${-r}Z`
      );
    }
    default:
      return null;
  }
}

function exportIcons(): string {
  const icons: Record<string, Part[]> = {};
  for (const name of Object.keys(ICONS).sort()) {
    const Svg = ICONS[name]!.svg;
    const markup = renderToStaticMarkup(<Svg size={24} strokeWidth={1.5} />);
    const root = attrs(markup.slice(0, markup.indexOf('>')));
    const rootFill = root.fill && root.fill !== 'none';
    const parts: Part[] = [];
    for (const m of markup.matchAll(/<(path|line|polyline|polygon|circle|ellipse|rect)\b([^>]*)>/g)) {
      const a = attrs(m[2]!);
      const d = shapePath(m[1]!, a);
      if (!d) continue;
      const fill = a.fill ? a.fill !== 'none' : !!rootFill;
      const strokeAttr = a.stroke ?? root.stroke ?? 'none';
      const stroke =
        strokeAttr === 'none' ? 'none' : strokeAttr.includes('icon-on') ? 'on' : 'current';
      const part: Part = { d, stroke };
      if (fill) part.fill = true;
      if ((a.class ?? '').includes('bo-icon-accent')) part.accent = true;
      parts.push(part);
    }
    icons[name] = parts;
  }
  return `${JSON.stringify({ grid: 24, stroke: 1.5, icons }, null, 1)}\n`;
}

describe('icon export for the Rust interface', () => {
  it('assets/icons/ui-icons.json matches the icon set', () => {
    const json = exportIcons();
    if (process.env.BO_WRITE_ICONS === '1') writeFileSync(jsonPath, json);
    expect(readFileSync(jsonPath, 'utf8').replace(/\r\n/g, '\n')).toBe(json);
    const parsed = JSON.parse(json) as { icons: Record<string, Part[]> };
    expect(parsed.icons['format.bold']?.length).toBeGreaterThan(0);
  });
});
