#!/usr/bin/env node
// Fidelity report (prompt §7): round-trips every OOXML file of tests/corpus
// through `bo-cli convert` (open → save in the same format), validates inputs
// and outputs with OpenXmlValidator (tools/BlueOffice.Validate) and writes
// docs/fidelity-report.md plus a line in tests/fidelity/history.csv.
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
import { join, relative } from 'node:path';
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
  for (const f of files) {
    const r = spawnSync(exe, ['convert', join(dir, f), '--to', ext, '--out', join(target, f)], {
      encoding: 'utf8',
    });
    if (r.status === 0) converted++;
    else failures.push(`${f}: ${(r.stderr || r.stdout).trim().split('\n').pop()}`);
  }
  rows.push({ group, ext, dir, target, files: files.length, converted, failures });
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
writeFileSync(join(root, 'docs/fidelity-report.md'), md);

const history = join(root, 'tests/fidelity/history.csv');
if (!existsSync(history)) writeFileSync(history, 'date,commit,group,files,converted,valid_out\n');
for (const r of rows) {
  appendFileSync(
    history,
    `${date},${commit},${r.group},${r.files},${r.converted},${r.validOut ?? ''}\n`,
  );
}
console.log(md);
process.exit(problems.length ? 1 : 0);
