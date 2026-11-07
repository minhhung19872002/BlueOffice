#!/usr/bin/env node
// Visual regression (prompt §7): renders corpus pages with `bo-cli render` and
// pixel-diffs them against
//   1. reference images supplied by the user from their licensed software,
//      tests/fidelity/reference/<doc|sheet|slide>/<file>.page-<n>.png
//      (per-file thresholds in tests/fidelity/thresholds.json, default 2 %);
//   2. BlueOffice's own baselines (page 1 of every corpus file, scale 0.5),
//      tests/fidelity/baseline/<group>/<file>.page-1.png — catches rendering
//      regressions (`--update-baseline` rewrites them after a reviewed change).
// Writes docs/fidelity-visual.md and diff images to tests/fidelity/output/diff.
//
// Usage: node scripts/visual-diff.mjs [--update-baseline]
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import pixelmatch from 'pixelmatch';
import pngjs from 'pngjs';

const { PNG } = pngjs;
const root = fileURLToPath(new URL('..', import.meta.url));
const corpus = join(root, 'tests/corpus');
const fidelity = join(root, 'tests/fidelity');
const outDir = join(fidelity, 'output/diff');
const exe = join(root, 'target/debug', process.platform === 'win32' ? 'bo-cli.exe' : 'bo-cli');
const update = process.argv.includes('--update-baseline');
const groups = { doc: ['docx'], sheet: ['xlsx'], slide: ['pptx'] };
const BASELINE_SCALE = 0.5;
/** Self-baseline tolerance: our own renders must stay (almost) identical. */
const BASELINE_THRESHOLD = 0.001;

if (!existsSync(exe)) {
  console.error(`missing ${exe} — run: cargo build -p bo-cli`);
  process.exit(2);
}
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
const thresholds = existsSync(join(fidelity, 'thresholds.json'))
  ? JSON.parse(readFileSync(join(fidelity, 'thresholds.json'), 'utf8'))
  : {};

function render(file, page, scale, out) {
  const r = spawnSync(
    exe,
    ['render', file, '--page', String(page), '--scale', String(scale), '--out', out],
    {
      encoding: 'utf8',
    },
  );
  if (r.status !== 0) throw new Error((r.stderr || r.stdout).trim().split('\n').pop());
  return PNG.sync.read(readFileSync(out));
}

/** Top-left `w × h` crop of an RGBA image. */
function crop(img, w, h) {
  if (img.width === w && img.height === h) return img.data;
  const out = Buffer.alloc(w * h * 4);
  for (let y = 0; y < h; y++)
    img.data.copy(out, y * w * 4, y * img.width * 4, y * img.width * 4 + w * 4);
  return out;
}

/**
 * Compares two images. Up to 2 px size difference (resolution rounding) is
 * tolerated by comparing the overlap; larger differences count as 100 %.
 */
function compare(a, b, diffPath) {
  if (Math.abs(a.width - b.width) > 2 || Math.abs(a.height - b.height) > 2) return 1;
  const w = Math.min(a.width, b.width);
  const h = Math.min(a.height, b.height);
  const diff = new PNG({ width: w, height: h });
  const n = pixelmatch(crop(a, w, h), crop(b, w, h), diff.data, w, h, { threshold: 0.1 });
  writeFileSync(diffPath, PNG.sync.write(diff));
  return n / (w * h);
}

const rows = [];
for (const [group, exts] of Object.entries(groups)) {
  const dir = join(corpus, group);
  if (!existsSync(dir)) continue;
  const files = readdirSync(dir)
    .filter((f) => exts.some((e) => f.toLowerCase().endsWith(`.${e}`)))
    .sort();
  // 1. User-supplied references.
  const refDir = join(fidelity, 'reference', group);
  if (existsSync(refDir)) {
    for (const ref of readdirSync(refDir)
      .filter((f) => f.endsWith('.png'))
      .sort()) {
      const m = /^(.*)\.page-(\d+)\.png$/.exec(ref);
      if (!m) continue;
      const [, name, pageNo] = m;
      const src = join(dir, name);
      const kind = 'reference';
      if (!existsSync(src)) {
        rows.push({
          group,
          kind,
          file: ref,
          ratio: null,
          limit: null,
          error: 'corpus file missing',
        });
        continue;
      }
      try {
        const expected = PNG.sync.read(readFileSync(join(refDir, ref)));
        // Render at the reference's resolution.
        const probe = render(src, pageNo, 1, join(outDir, `${name}.probe.png`));
        const scale = expected.width / probe.width;
        const actual = render(src, pageNo, scale, join(outDir, `${name}.page-${pageNo}.png`));
        const limit = thresholds[`${group}/${ref}`] ?? thresholds.default ?? 0.02;
        const ratio = compare(actual, expected, join(outDir, `${name}.page-${pageNo}.diff.png`));
        rows.push({ group, kind, file: ref, ratio, limit });
      } catch (e) {
        rows.push({
          group,
          kind,
          file: ref,
          ratio: null,
          limit: null,
          error: String(e.message ?? e),
        });
      }
    }
  }
  // 2. Self baselines (page 1).
  const baseDir = join(fidelity, 'baseline', group);
  mkdirSync(baseDir, { recursive: true });
  for (const f of files) {
    const target = join(baseDir, `${f}.page-1.png`);
    try {
      const actual = render(join(dir, f), 1, BASELINE_SCALE, join(outDir, `${f}.page-1.png`));
      if (update || !existsSync(target)) {
        writeFileSync(target, PNG.sync.write(actual));
        rows.push({
          group,
          kind: 'baseline',
          file: basename(target),
          ratio: 0,
          limit: BASELINE_THRESHOLD,
          created: true,
        });
        continue;
      }
      const ratio = compare(
        actual,
        PNG.sync.read(readFileSync(target)),
        join(outDir, `${f}.page-1.diff.png`),
      );
      rows.push({
        group,
        kind: 'baseline',
        file: basename(target),
        ratio,
        limit: BASELINE_THRESHOLD,
      });
    } catch (e) {
      rows.push({
        group,
        kind: 'baseline',
        file: `${f}.page-1.png`,
        ratio: null,
        limit: null,
        error: String(e.message ?? e),
      });
    }
  }
}

const pct = (x) => (x == null ? '—' : `${(100 * x).toFixed(2)} %`);
const failed = rows.filter((r) => r.error || (r.ratio != null && r.ratio > r.limit));
const refs = rows.filter((r) => r.kind === 'reference' && r.ratio != null);
const avg = refs.length ? refs.reduce((s, r) => s + r.ratio, 0) / refs.length : null;

let md = '# Visual diff\n\n';
md += `Sinh bởi \`scripts/visual-diff.mjs\` (${new Date().toISOString().slice(0, 10)}). `;
md +=
  'Ảnh tham chiếu do người dùng cung cấp đặt ở `tests/fidelity/reference/<nhóm>/<file>.page-<n>.png`; ';
md +=
  'baseline tự sinh (trang 1, scale 0,5) ở `tests/fidelity/baseline/` để phát hiện hồi quy hiển thị.\n\n';
md += `- Ảnh tham chiếu: **${refs.length}** trang${avg == null ? ' (chưa có — cần người dùng cung cấp)' : `, sai lệch trung bình **${pct(avg)}** (mục tiêu Phase 2 < 2 %)`}\n`;
md += `- Baseline: ${rows.filter((r) => r.kind === 'baseline').length} trang, vượt ngưỡng ${pct(BASELINE_THRESHOLD)}: ${failed.filter((r) => r.kind === 'baseline').length}\n\n`;
md += '| Nhóm | Loại | Ảnh | Sai lệch | Ngưỡng | |\n|---|---|---|---|---|---|\n';
for (const r of rows) {
  const ok = r.error ? `❌ ${r.error}` : r.created ? '🆕' : r.ratio <= r.limit ? '✅' : '❌';
  md += `| ${r.group} | ${r.kind} | ${r.file} | ${pct(r.ratio)} | ${pct(r.limit)} | ${ok} |\n`;
}
writeFileSync(join(root, 'docs/fidelity-visual.md'), md);
console.log(md.split('\n').slice(0, 6).join('\n'));
console.log(`${failed.length} failing`);
process.exit(failed.length ? 1 : 0);
