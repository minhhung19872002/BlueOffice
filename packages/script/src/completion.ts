/**
 * Autocompletion for the BlueScript editor (CORE-SCRIPT-001), from the names in
 * `bluescript.d.ts`: after `doc.` the members of `BlueDocument`, after a variable
 * named like a type (`shape.`, `range.`, `para.`) that type's members, else the
 * application's globals and keywords.
 */
import DTS from '../bluescript.d.ts?raw';
import type { ScriptApp } from './sandbox';

/** The API typings shipped with BlueScript. */
export const BLUESCRIPT_DTS: string = DTS;

export interface Member {
  name: string;
  /** `method`: called with `(…)`. */
  kind: 'method' | 'property';
  /** The declaration line (shown as a hint). */
  signature: string;
}

/** Interfaces of the typings and their members. */
export function parseDts(dts: string): Map<string, Member[]> {
  const out = new Map<string, Member[]>();
  let current: Member[] | null = null;
  for (const line of dts.split('\n')) {
    const head = /^interface (\w+)/.exec(line);
    if (head) {
      current = [];
      out.set(head[1]!, current);
      continue;
    }
    if (/^}/.test(line)) {
      current = null;
      continue;
    }
    if (!current) continue;
    const m = /^ {2}(?:readonly )?(\w+)\??(\(|:)/.exec(line);
    if (m && !current.some((x) => x.name === m[1])) {
      current.push({
        name: m[1]!,
        kind: m[2] === '(' ? 'method' : 'property',
        signature: line.trim(),
      });
    }
  }
  return out;
}

const INTERFACES = parseDts(DTS);

const APP_TYPES: Record<ScriptApp, { globals: Record<string, string>; types: string[] }> = {
  doc: { globals: { doc: 'BlueDocument', console: 'ScriptConsole' }, types: ['Paragraph'] },
  sheet: {
    globals: { workbook: 'Workbook', sheet: 'Sheet', console: 'ScriptConsole' },
    types: ['Range'],
  },
  slide: {
    globals: { presentation: 'Presentation', console: 'ScriptConsole' },
    types: ['Slide', 'Shape'],
  },
};

/** Variable names that suggest a type (`para` → Paragraph). */
const ALIASES: Record<string, string> = {
  p: 'Paragraph',
  para: 'Paragraph',
  paragraph: 'Paragraph',
  r: 'Range',
  range: 'Range',
  cells: 'Range',
  ws: 'Sheet',
  s: 'Shape',
  shape: 'Shape',
  slide: 'Slide',
  sl: 'Slide',
};

const KEYWORDS = [
  'await',
  'async',
  'const',
  'let',
  'for',
  'of',
  'if',
  'else',
  'return',
  'function',
  'while',
  'break',
  'continue',
  'try',
  'catch',
  'throw',
  'new',
  'true',
  'false',
  'null',
];

export interface Completion {
  /** Start of the word being completed (string index). */
  from: number;
  items: Member[];
}

/** Members of the type a variable named `name` probably has, in `app`. */
function membersFor(app: ScriptApp, name: string): Member[] {
  const spec = APP_TYPES[app];
  const type =
    spec.globals[name] ??
    ALIASES[name.toLowerCase()] ??
    [...Object.values(spec.globals), ...spec.types].find(
      (t) => t.toLowerCase() === name.toLowerCase(),
    );
  if (type && (spec.types.includes(type) || Object.values(spec.globals).includes(type))) {
    return INTERFACES.get(type) ?? [];
  }
  // Unknown receiver: every member of the app's types.
  const all = new Map<string, Member>();
  for (const t of [...Object.values(spec.globals), ...spec.types]) {
    for (const m of INTERFACES.get(t) ?? []) if (!all.has(m.name)) all.set(m.name, m);
  }
  return [...all.values()];
}

/** Suggestions at `caret` in `text`; null when there is nothing to complete. */
export function complete(
  app: ScriptApp,
  text: string,
  caret: number,
  force = false,
): Completion | null {
  const before = text.slice(0, caret);
  const word = /[A-Za-z_$][\w$]*$/.exec(before)?.[0] ?? '';
  const from = caret - word.length;
  const dot = /([A-Za-z_$][\w$]*)\s*\.\s*$/.exec(before.slice(0, from));
  let pool: Member[];
  if (dot) {
    pool = membersFor(app, dot[1]!);
  } else {
    if (!word && !force) return null;
    // Not after a dot inside a string or comment line: keep it simple, only code.
    pool = [
      ...Object.entries(APP_TYPES[app].globals).map(([name, type]) => ({
        name,
        kind: 'property' as const,
        signature: `${name}: ${type}`,
      })),
      ...KEYWORDS.map((k) => ({ name: k, kind: 'property' as const, signature: k })),
    ];
  }
  const lower = word.toLowerCase();
  const items = pool
    .filter((m) => m.name.toLowerCase().startsWith(lower) && m.name !== word)
    .sort((a, b) => a.name.localeCompare(b.name));
  if (!items.length) return null;
  if (!dot && !word && !force) return null;
  return { from, items };
}

/** The text to insert for a member (methods get `()` and the caret goes inside). */
export function insertion(m: Member): { text: string; caretBack: number } {
  if (m.kind !== 'method') return { text: m.name, caretBack: 0 };
  const noArgs = /\(\)/.test(m.signature);
  return { text: `${m.name}()`, caretBack: noArgs ? 0 : 1 };
}

/** Every global and member name of `app` (tests, docs). */
export function apiNames(app: ScriptApp): string[] {
  const spec = APP_TYPES[app];
  const names = new Set<string>(Object.keys(spec.globals));
  for (const t of [...Object.values(spec.globals), ...spec.types]) {
    for (const m of INTERFACES.get(t) ?? []) names.add(`${t}.${m.name}`);
  }
  return [...names];
}
