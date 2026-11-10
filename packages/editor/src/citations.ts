/**
 * Citation helpers of BlueDoc (CORE-CITE-001): the per-user master list of sources, the
 * fields each source type shows in the source form (elements of ECMA-376 Part 1, §22.6.2),
 * and names typed as text.
 */
import type { CiteContributor, CitePerson, CiteSource, CiteSourceType } from '@blueoffice/platform';
import { CITE_SOURCE_TYPES } from '@blueoffice/platform';
import type { SettingStore } from '@blueoffice/ui';

/** Setting key of the master list (`ShellIntegration.getSetting`, else `localStorage`). */
export const MASTER_LIST_KEY = 'blueoffice.cite.master';

const str = (v: unknown): string => (typeof v === 'string' ? v : '');

function sanitizePerson(v: unknown): CitePerson | null {
  if (!v || typeof v !== 'object') return null;
  const o = v as Record<string, unknown>;
  const p: CitePerson = { last: str(o.last).trim() };
  if (str(o.first).trim()) p.first = str(o.first).trim();
  if (str(o.middle).trim()) p.middle = str(o.middle).trim();
  return p.last || p.first || p.middle ? p : null;
}

/** A source read from storage or a file: unknown shapes are dropped, strings trimmed. */
export function sanitizeSource(v: unknown): CiteSource | null {
  if (!v || typeof v !== 'object') return null;
  const o = v as Record<string, unknown>;
  const kind = (CITE_SOURCE_TYPES as readonly string[]).includes(str(o.kind))
    ? str(o.kind)
    : 'Misc';
  const out: CiteSource = { tag: str(o.tag).trim(), kind };
  if (Array.isArray(o.contributors)) {
    const list: CiteContributor[] = [];
    for (const c of o.contributors as unknown[]) {
      if (!c || typeof c !== 'object') continue;
      const co = c as Record<string, unknown>;
      const role = str(co.role).trim();
      if (!/^[A-Za-z][A-Za-z0-9]*$/.test(role)) continue;
      const item: CiteContributor = { role };
      if (str(co.corporate).trim()) item.corporate = str(co.corporate).trim();
      const people = Array.isArray(co.people)
        ? (co.people as unknown[]).map(sanitizePerson).filter((p): p is CitePerson => !!p)
        : [];
      if (people.length) item.people = people;
      if (item.corporate || item.people) list.push(item);
    }
    if (list.length) out.contributors = list;
  }
  if (o.fields && typeof o.fields === 'object') {
    const fields: Record<string, string> = {};
    for (const [k, val] of Object.entries(o.fields as Record<string, unknown>)) {
      if (/^[A-Za-z][A-Za-z0-9]*$/.test(k) && str(val).trim()) fields[k] = str(val).trim();
    }
    if (Object.keys(fields).length) out.fields = fields;
  }
  return out;
}

function parseList(text: string | null): CiteSource[] {
  if (!text) return [];
  try {
    const v = JSON.parse(text) as unknown;
    if (!Array.isArray(v)) return [];
    const seen = new Set<string>();
    const out: CiteSource[] = [];
    for (const item of v) {
      const s = sanitizeSource(item);
      if (!s || !s.tag || seen.has(s.tag.toLowerCase())) continue;
      seen.add(s.tag.toLowerCase());
      out.push(s);
    }
    return out;
  } catch {
    return [];
  }
}

/** The user's master list of sources. */
export async function loadMasterList(store: SettingStore): Promise<CiteSource[]> {
  try {
    if (store.getSetting) return parseList(await store.getSetting(MASTER_LIST_KEY));
    return parseList(globalThis.localStorage?.getItem(MASTER_LIST_KEY) ?? null);
  } catch {
    return [];
  }
}

export async function saveMasterList(store: SettingStore, list: CiteSource[]): Promise<void> {
  const text = list.length ? JSON.stringify(list) : null;
  if (store.setSetting) {
    await store.setSetting(MASTER_LIST_KEY, text);
    return;
  }
  try {
    if (text) globalThis.localStorage?.setItem(MASTER_LIST_KEY, text);
    else globalThis.localStorage?.removeItem(MASTER_LIST_KEY);
  } catch {
    // Storage unavailable: the list lasts for this session.
  }
}

/** `list` with `sources` added; a source with the same tag is replaced. */
export function mergeSources(list: CiteSource[], sources: CiteSource[]): CiteSource[] {
  const out = [...list];
  for (const s of sources) {
    const i = out.findIndex((x) => x.tag.toLowerCase() === s.tag.toLowerCase());
    if (i >= 0) out[i] = s;
    else out.push(s);
  }
  return out;
}

/** The source without the fields the engine adds for listing. */
export function plainSource(s: CiteSource): CiteSource {
  const out: CiteSource = { tag: s.tag, kind: s.kind };
  if (s.contributors?.length) out.contributors = s.contributors;
  if (s.fields && Object.keys(s.fields).length) out.fields = s.fields;
  return out;
}

/** Elements every source type shows first. */
const COMMON = ['Title', 'Year'];

/** The elements the source form shows for each type (after title and year). */
const BY_TYPE: Record<string, string[]> = {
  Book: ['City', 'Publisher', 'Edition', 'StandardNumber', 'URL'],
  BookSection: ['BookTitle', 'Pages', 'City', 'Publisher', 'Edition'],
  JournalArticle: ['JournalName', 'Volume', 'Issue', 'Pages', 'DOI', 'URL'],
  ArticleInAPeriodical: ['PeriodicalTitle', 'Month', 'Day', 'Pages', 'URL'],
  ConferenceProceedings: ['ConferenceName', 'Pages', 'City', 'Publisher', 'DOI'],
  Report: ['Publisher', 'City', 'ThesisType', 'Institution', 'URL'],
  InternetSite: [
    'InternetSiteTitle',
    'Month',
    'Day',
    'URL',
    'YearAccessed',
    'MonthAccessed',
    'DayAccessed',
  ],
  DocumentFromInternetSite: [
    'InternetSiteTitle',
    'Month',
    'Day',
    'URL',
    'YearAccessed',
    'MonthAccessed',
    'DayAccessed',
  ],
  Case: ['CaseNumber', 'Court', 'Month', 'Day', 'URL'],
};
const OTHER = ['Publisher', 'City', 'Medium', 'URL'];

/** Every element the form can show ("show all fields"). */
export const ALL_FIELDS = [
  'Title',
  'ShortTitle',
  'Year',
  'Month',
  'Day',
  'City',
  'StateProvince',
  'CountryRegion',
  'Publisher',
  'Edition',
  'Volume',
  'NumberVolumes',
  'Issue',
  'Pages',
  'JournalName',
  'PeriodicalTitle',
  'BookTitle',
  'ConferenceName',
  'Institution',
  'Department',
  'ThesisType',
  'InternetSiteTitle',
  'URL',
  'DOI',
  'YearAccessed',
  'MonthAccessed',
  'DayAccessed',
  'CaseNumber',
  'Court',
  'StandardNumber',
  'Medium',
  'Comments',
];

/** Elements kept by the engine that the form does not edit as text. */
const HIDDEN = ['LCID', 'Guid', 'RefOrder'];

/** The elements to show for a source: its type's, or all; always the ones it has. */
export function fieldsFor(source: CiteSource, all: boolean): string[] {
  const base = all ? ALL_FIELDS : [...COMMON, ...(BY_TYPE[source.kind as CiteSourceType] ?? OTHER)];
  const extra = Object.keys(source.fields ?? {}).filter(
    (k) => !base.includes(k) && !HIDDEN.includes(k),
  );
  return [...base, ...extra];
}

/** People as text, one per line: `Family, Given Middle`. */
export function peopleText(people: CitePerson[] | undefined): string {
  return (people ?? [])
    .map((p) => {
      const given = [p.first, p.middle].filter(Boolean).join(' ');
      return given ? `${p.last}, ${given}` : p.last;
    })
    .join('\n');
}

/**
 * People typed one per line (or separated by `;`). `Family, Given` is taken as written.
 * Without a comma the family name is the last word — or, for a Vietnamese source, the
 * first one (`Nguyễn Văn An`: family name `Nguyễn`, given names `Văn An`).
 */
export function parsePeople(text: string, vietnamese: boolean): CitePerson[] {
  const out: CitePerson[] = [];
  for (const raw of text.split(/[\n;]/)) {
    const name = raw.trim().replace(/\s+/g, ' ');
    if (!name) continue;
    const comma = name.indexOf(',');
    if (comma >= 0) {
      const last = name.slice(0, comma).trim();
      const first = name.slice(comma + 1).trim();
      out.push(first ? { last, first } : { last });
      continue;
    }
    const words = name.split(' ');
    if (words.length === 1) {
      out.push({ last: name });
    } else if (vietnamese) {
      out.push({ last: words[0] ?? '', first: words.slice(1).join(' ') });
    } else {
      out.push({ last: words[words.length - 1] ?? '', first: words.slice(0, -1).join(' ') });
    }
  }
  return out;
}

/** The contributor of `role` in a source. */
export function contributor(source: CiteSource, role: string): CiteContributor | undefined {
  return source.contributors?.find((c) => c.role === role);
}

/** `source` with the contributor of `role` replaced (removed when it names nobody). */
export function withContributor(
  source: CiteSource,
  role: string,
  value: { corporate?: string; people?: CitePerson[] },
): CiteSource {
  const rest = (source.contributors ?? []).filter((c) => c.role !== role);
  const corporate = value.corporate?.trim();
  const people = value.people ?? [];
  const next: CiteContributor[] = corporate
    ? [...rest, { role, corporate }]
    : people.length
      ? [...rest, { role, people }]
      : rest;
  // Authors first, as they are read.
  next.sort((a, b) => Number(b.role === 'Author') - Number(a.role === 'Author'));
  return { ...source, contributors: next.length ? next : undefined };
}

/** `source` with element `name` set (removed when empty). */
export function withField(source: CiteSource, name: string, value: string): CiteSource {
  const fields = { ...(source.fields ?? {}) };
  if (value.trim()) fields[name] = value;
  else delete fields[name];
  return { ...source, fields };
}

/** One line describing a source in a list: authors, year, title. */
export function sourceLine(s: CiteSource): string {
  const a = contributor(s, 'Author');
  const who = a?.corporate ?? (a?.people ?? []).map((p) => p.last).join(', ');
  const year = s.fields?.Year ?? '';
  const title = s.fields?.Title ?? '';
  const head = [who, year && `(${year})`].filter(Boolean).join(' ');
  return [head, title].filter(Boolean).join('. ') || s.tag;
}

/** Whether `s` matches the search text (tag, names, any element). */
export function matchesSource(s: CiteSource, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const hay = [
    s.tag,
    ...(s.contributors ?? []).flatMap((c) => [
      c.corporate ?? '',
      ...(c.people ?? []).flatMap((p) => [p.last, p.first ?? '', p.middle ?? '']),
    ]),
    ...Object.values(s.fields ?? {}),
  ]
    .join(' ')
    .toLowerCase();
  return q.split(/\s+/).every((w) => hay.includes(w));
}

/** The next free placeholder tag (`Placeholder1`, `Placeholder2`, …). */
export function placeholderTag(taken: string[]): string {
  const used = new Set(taken.map((t) => t.toLowerCase()));
  let n = 1;
  while (used.has(`placeholder${n}`)) n += 1;
  return `Placeholder${n}`;
}

/** LCID of the UI language for the words a style adds (Vietnamese 1066, else English 1033). */
export function lcidOf(language: string): string {
  return language.toLowerCase().startsWith('vi') ? '1066' : '1033';
}
