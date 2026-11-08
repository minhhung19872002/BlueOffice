#!/usr/bin/env node
// Fidelity report (prompt §7): round-trips every OOXML file of tests/corpus
// through `bo-cli convert` (open → save in the same format), validates inputs
// and outputs with OpenXmlValidator (tools/BlueOffice.Validate) and writes
// docs/fidelity-report.md plus lines in tests/fidelity/history.csv: one per format
// (`doc`, `sheet`, `slide`) and one per feature group (`doc:styles`, `sheet:pivots`,
// `slide:slide-anim`, … — from the corpus file-name prefix, see `featureOf`). The report
// shows % pass per feature group and the trend of the last runs from the history.
//
// Usage: node scripts/fidelity-report.mjs [--no-validate]
// Needs: `cargo build -p bo-cli` (target/debug/bo-cli) and the .NET 8 SDK.
import { execFileSync, spawnSync } from 'node:child_process';
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { basename, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const corpus = join(root, 'tests/corpus');
const out = join(root, 'tests/fidelity/output/roundtrip');
const groups = { doc: 'docx', sheet: 'xlsx', slide: 'pptx' };
const validate = !process.argv.includes('--no-validate');
const exe = join(root, 'target/debug', process.platform === 'win32' ? 'bo-cli.exe' : 'bo-cli');

if (!existsSync(exe)) {
  console.error(`missing ${exe} — run: cargo build -p bo-cli`);
  process.exit(2);
}
rmSync(out, { recursive: true, force: true });

// Feature group of a corpus file: the name up to its first numbered segment
// (`tables-02-merge.docx` → `tables`, `cf-1.xlsx` → `cf`); slide files have no
// number and use their first two segments (`slide-anim-fly-wipe.pptx` → `slide-anim`).
// Aliases fold the writer-generated variants into the feature they exercise.
const aliases = [
  [/^writer-pivot(s|-.*)?$/, 'pivots'],
  [/^(writer-charts|chart-anchors)$/, 'charts'],
  [/^writer-format$/, 'styles'],
  [/^writer-sheet$/, 'values'],
  [/^phase1$/, 'sample'],
];
function featureOf(file) {
  const name = file.replace(/\.[^.]+$/, '');
  const parts = name.split('-');
  const numbered = parts.findIndex((p) => /^\d+$/.test(p));
  let key;
  if (numbered > 0) key = parts.slice(0, numbered).join('-');
  else if (parts[0] === 'slide' && parts.length > 1) key = parts.slice(0, 2).join('-');
  else key = parts[0];
  for (const [re, alias] of aliases) if (re.test(key)) return alias;
  return key;
}

const rows = [];
for (const [group, ext] of Object.entries(groups)) {
  const dir = join(corpus, group);
  if (!existsSync(dir)) continue;
  const files = readdirSync(dir)
    .filter((f) => f.toLowerCase().endsWith(`.${ext}`))
    .sort();
  const target = join(out, group);
  mkdirSync(target, { recursive: true });
  let converted = 0;
  const failures = [];
  // Per-file outcome: converted, then (when validating) whether the output is valid.
  const perFile = new Map();
  for (const f of files) {
    const r = spawnSync(exe, ['convert', join(dir, f), '--to', ext, '--out', join(target, f)], {
      encoding: 'utf8',
    });
    if (r.status === 0) converted++;
    else failures.push(`${f}: ${(r.stderr || r.stdout).trim().split('\n').pop()}`);
    perFile.set(f, { converted: r.status === 0, valid: null });
  }
  rows.push({ group, ext, dir, target, files: files.length, converted, failures, perFile });
}

function runValidator(dir) {
  const json = join(out, `validate-${Date.now()}-${Math.random().toString(36).slice(2)}.json`);
  spawnSync(
    'dotnet',
    [
      'run',
      '--project',
      join(root, 'tools/BlueOffice.Validate'),
      '-c',
      'Release',
      '--',
      dir,
      '--json',
      json,
    ],
    { encoding: 'utf8', stdio: ['ignore', 'ignore', 'inherit'] },
  );
  if (!existsSync(json)) return null;
  const results = JSON.parse(readFileSync(json, 'utf8'));
  rmSync(json);
  return results;
}

for (const row of rows) {
  if (!validate) continue;
  // Only the group's own format (e.g. doc/ also holds mail-merge .xlsx data).
  const own = (res) => res && res.filter((r) => r.File.toLowerCase().endsWith(`.${row.ext}`));
  const inRes = own(runValidator(row.dir));
  const outRes = own(runValidator(row.target));
  row.validIn = inRes ? inRes.filter((r) => r.Errors.length === 0).length : null;
  row.validOut = outRes ? outRes.filter((r) => r.Errors.length === 0).length : null;
  row.invalidOut = outRes
    ? outRes
        .filter((r) => r.Errors.length > 0)
        .map((r) => `${relative(out, r.File)}: ${r.Errors[0].Description}`)
    : [];
  for (const r of outRes ?? []) {
    const f = row.perFile.get(basename(r.File));
    if (f) f.valid = r.Errors.length === 0;
  }
}

const pct = (a, b) => (b ? `${((100 * a) / b).toFixed(1)} %` : '—');
const commit = (() => {
  try {
    return execFileSync('git', ['rev-parse', '--short', 'HEAD'], {
      cwd: root,
      encoding: 'utf8',
    }).trim();
  } catch {
    return 'unknown';
  }
})();
const date = new Date().toISOString().slice(0, 10);

let md = `# Báo cáo độ trung thực (fidelity)\n\n`;
md += `Sinh tự động bởi \`scripts/fidelity-report.mjs\` · ${date} · commit \`${commit}\`.\n\n`;
md += `Mỗi file trong \`tests/corpus/{doc,sheet,slide}\` được mở rồi lưu lại cùng định dạng bằng \`bo-cli convert\`; `;
md += `file vào và ra đều qua OpenXmlValidator (DocumentFormat.OpenXml, Office 2019). `;
md += `Mức lossless (XML chuẩn hoá giống hệt) được đo bằng test Rust \`corpus\` của từng crate.\n\n`;
md += `| Nhóm | File | Mở + lưu được | OpenXmlValidator (vào) | OpenXmlValidator (ra) |\n|---|---|---|---|---|\n`;
for (const r of rows) {
  md += `| ${r.group} (.${r.ext}) | ${r.files} | ${r.converted} (${pct(r.converted, r.files)}) | `;
  md += `${r.validIn ?? '—'} | ${r.validOut == null ? '—' : `${r.validOut} (${pct(r.validOut, r.converted)})`} |\n`;
}
// Accepted findings: `<substring> | <reason>` lines in tests/fidelity/known-issues.txt.
const knownFile = join(root, 'tests/fidelity/known-issues.txt');
const known = existsSync(knownFile)
  ? readFileSync(knownFile, 'utf8')
      .split(/\r?\n/)
      .filter((l) => l.trim() && !l.startsWith('#'))
      .map((l) => {
        const [pattern, reason] = l.split(' | ');
        return { pattern: pattern.trim(), reason: (reason ?? '').trim() };
      })
  : [];
const all = rows.flatMap((r) => [...r.failures, ...(r.invalidOut ?? [])]);
const knownFor = (p) => known.find((k) => p.includes(k.pattern));
const problems = all.filter((p) => !knownFor(p));
const accepted = all.filter((p) => knownFor(p));
if (problems.length) {
  md += `\n## Lỗi\n\n${problems.map((p) => `- ${p}`).join('\n')}\n`;
}
if (accepted.length) {
  md += '\n## Đã biết (không tính là lỗi, xem `tests/fidelity/known-issues.txt`)\n\n';
  md += accepted.map((p) => `- ${p} — ${knownFor(p).reason}`).join('\n') + '\n';
}
// Feature groups: a file passes when it converts and (when validated) its output
// is valid; outputs failing only on accepted findings (known-issues.txt) are counted apart.
const features = [];
for (const r of rows) {
  const byFeature = new Map();
  for (const [file, f] of r.perFile) {
    const key = featureOf(file);
    const agg = byFeature.get(key) ?? { files: 0, converted: 0, pass: 0, known: 0 };
    agg.files++;
    if (f.converted) agg.converted++;
    // No validator result (--no-validate or no .NET SDK): conversion alone decides.
    if (f.converted && (f.valid ?? true)) agg.pass++;
    else if (f.converted && accepted.some((p) => p.includes(file))) agg.known++;
    byFeature.set(key, agg);
  }
  for (const [feature, agg] of [...byFeature].sort(([a], [b]) => a.localeCompare(b))) {
    features.push({ key: `${r.group}:${feature}`, group: r.group, feature, ...agg });
  }
}

// Trend: pass % of the previous runs (one point per date + commit) from history.csv.
const history = join(root, 'tests/fidelity/history.csv');
const past = existsSync(history)
  ? readFileSync(history, 'utf8')
      .split(/\r?\n/)
      .slice(1)
      .filter(Boolean)
      .map((l) => {
        const [d, c, group, files, converted, valid] = l.split(',');
        return { run: `${d} ${c}`, group, files: +files, converted: +converted, valid };
      })
  : [];
function trend(key, current) {
  const runs = [];
  for (const h of past) {
    if (h.group !== key || !h.files) continue;
    const p = (100 * (h.valid ? +h.valid : h.converted)) / h.files;
    const last = runs[runs.length - 1];
    if (last && last.run === h.run) last.p = p;
    else runs.push({ run: h.run, p });
  }
  const points = [...runs.slice(-4).map((r) => r.p), current];
  const prev = runs.length ? runs[runs.length - 1].p : null;
  let delta = 'mới';
  if (prev != null) {
    const d = current - prev;
    delta = Math.abs(d) < 0.05 ? '=' : `${d > 0 ? '+' : ''}${d.toFixed(1)}`;
  }
  return `${points.map((p) => p.toFixed(0)).join(' → ')} (${delta})`;
}

md += '\n## Theo nhóm tính năng\n\n';
md += 'Nhóm = tiền tố tên file corpus (`tables-02-merge.docx` → `tables`, `cf-1.xlsx` → `cf`, ';
md += '`slide-anim-…` → `slide-anim`, `writer-pivot-…` → `pivots`). **Đạt** = mở + lưu được và ';
md += 'file ra hợp lệ theo OpenXmlValidator; cột "Đã biết" đếm file ra chỉ vướng lỗi đã chấp nhận ';
md += 'trong `known-issues.txt`. Xu hướng: % đạt của tối đa 4 lần chạy trước (từ ';
md += '`tests/fidelity/history.csv`) → lần này, kèm chênh lệch so với lần trước.\n\n';
md += '| Định dạng | Nhóm tính năng | File | Đạt | % đạt | Đã biết | Xu hướng % |\n';
md += '|---|---|---|---|---|---|---|\n';
for (const f of features) {
  const p = f.files ? (100 * f.pass) / f.files : 0;
  md += `| ${f.group} | ${f.feature} | ${f.files} | ${f.pass} | ${pct(f.pass, f.files)} | `;
  md += `${f.known || ''} | ${trend(f.key, p)} |\n`;
}
md += '\n### Xu hướng theo định dạng\n\n| Định dạng | % đạt (các lần chạy trước → lần này) |\n';
md += '|---|---|\n';
for (const r of rows) {
  const passed = r.validOut ?? r.converted;
  md += `| ${r.group} | ${trend(r.group, r.files ? (100 * passed) / r.files : 0)} |\n`;
}
writeFileSync(join(root, 'docs/fidelity-report.md'), md);

if (!existsSync(history)) writeFileSync(history, 'date,commit,group,files,converted,valid_out\n');
for (const r of rows) {
  appendFileSync(
    history,
    `${date},${commit},${r.group},${r.files},${r.converted},${r.validOut ?? ''}\n`,
  );
}
// Feature rows reuse the columns: group = `<format>:<feature>`, valid_out = files passed.
for (const f of features) {
  const pass = validate ? f.pass : '';
  appendFileSync(history, `${date},${commit},${f.key},${f.files},${f.converted},${pass}\n`);
}
console.log(md);
process.exit(problems.length ? 1 : 0);
