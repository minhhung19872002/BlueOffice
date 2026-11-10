/**
 * Get & Transform data (SHEET-PQ-001): the editor's pure logic — building sources from what
 * the user picked, editing the step list (insert after the selected step, move, delete), the
 * steps a column header menu generates, and the text of each step in the step list.
 */
import type {
  QueryColType,
  QueryCondition,
  QueryDef,
  QueryInspectResult,
  QuerySource,
  QueryStep,
  QueryWebTable,
} from '@blueoffice/platform';

/** Source kinds offered by the ribbon. */
export type QuerySourceKind = 'csv' | 'json' | 'xml' | 'workbook' | 'folder' | 'web';

/** File types each file source accepts. */
export const SOURCE_ACCEPT: Record<'csv' | 'json' | 'xml' | 'workbook', string[]> = {
  csv: ['.csv', '.txt', '.tsv'],
  json: ['.json'],
  xml: ['.xml'],
  workbook: ['.xlsx', '.xlsm', '.xls', '.xlsb', '.ods'],
};

/** Base64 of bytes (for `querySource`). */
export function toBase64(bytes: Uint8Array): string {
  let s = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    s += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(s);
}

/** Query name from a file name or URL: no folder, no extension. */
export function baseName(name: string): string {
  const last = name.split(/[\\/]/).filter(Boolean).pop() ?? name;
  const dot = last.lastIndexOf('.');
  const base = (dot > 0 ? last.slice(0, dot) : last).trim();
  return base || 'Query';
}

/** `name`, or `name (2)`, `name (3)` … not among `taken` (case-insensitive). */
export function uniqueName(name: string, taken: string[]): string {
  const low = new Set(taken.map((n) => n.toLowerCase()));
  if (!low.has(name.toLowerCase())) return name;
  for (let k = 2; ; k++) {
    const n = `${name} (${k})`;
    if (!low.has(n.toLowerCase())) return n;
  }
}

/** The folder file kind most of `names` have (`csv` when none). */
export function guessFolderKind(names: string[]): 'csv' | 'json' | 'xml' | 'workbook' {
  const count = { csv: 0, json: 0, xml: 0, workbook: 0 };
  for (const n of names) {
    const ext = n.toLowerCase().split('.').pop() ?? '';
    if (['csv', 'txt', 'tsv'].includes(ext)) count.csv++;
    else if (ext === 'json') count.json++;
    else if (ext === 'xml') count.xml++;
    else if (['xlsx', 'xlsm', 'xls', 'xlsb', 'ods'].includes(ext)) count.workbook++;
  }
  const best = (Object.entries(count) as [keyof typeof count, number][]).sort(
    (a, b) => b[1] - a[1],
  )[0];
  return best && best[1] > 0 ? best[0] : 'csv';
}

/** The web table a new query starts from: the first with a header row, else the largest. */
export function pickWebTable(tables: QueryWebTable[]): number {
  const withHeader = tables.find((t) => t.header && t.rows > 1);
  if (withHeader) return withHeader.index;
  const largest = [...tables].sort((a, b) => b.rows * b.cols - a.rows * a.cols)[0];
  return largest?.index ?? 0;
}

/** First source of a kind for a key. */
export function initialSource(kind: QuerySourceKind, key: string, fileKind?: string): QuerySource {
  switch (kind) {
    case 'csv':
      return { kind: 'csv', file: key };
    case 'json':
      return { kind: 'json', file: key };
    case 'xml':
      return { kind: 'xml', file: key };
    case 'workbook':
      return { kind: 'workbook', file: key, item: '', itemKind: 'sheet' };
    case 'folder':
      return {
        kind: 'folder',
        folder: key,
        fileKind: (fileKind as 'csv' | 'json' | 'xml' | 'workbook' | undefined) ?? 'csv',
        promoteHeaders: true,
      };
    case 'web':
      return { kind: 'web', url: key, table: 0 };
  }
}

/** The source refined with the navigator's first choice (JSON array, XML element, sheet, web table). */
export function refineSource(source: QuerySource, inspect: QueryInspectResult): QuerySource {
  switch (source.kind) {
    case 'json': {
      const p = inspect.jsonPaths?.[0];
      return p && p !== '$' && !source.path ? { ...source, path: p } : source;
    }
    case 'xml': {
      const el = inspect.xmlElements?.[0]?.name;
      return el && !source.element ? { ...source, element: el } : source;
    }
    case 'workbook': {
      if (source.item) return source;
      const items = inspect.items ?? [];
      const it = items.find((i) => i.kind === 'table') ?? items[0];
      return it ? { ...source, item: it.name, itemKind: it.kind } : source;
    }
    case 'web':
      return inspect.tables?.length ? { ...source, table: pickWebTable(inspect.tables) } : source;
    case 'csv':
      return inspect.csv && !source.delimiter
        ? { ...source, delimiter: inspect.csv.delimiter, encoding: inspect.csv.encoding }
        : source;
    default:
      return source;
  }
}

/** Steps after inserting `step` after step `after` (-1: right after the source); the new index. */
export function insertStep(
  steps: QueryStep[],
  after: number,
  step: QueryStep,
): { steps: QueryStep[]; index: number } {
  const at = Math.max(0, Math.min(after + 1, steps.length));
  // Two type changes in a row become one step (later types win).
  const prev = steps[at - 1];
  if (step.op === 'changeType' && prev?.op === 'changeType' && at === steps.length) {
    const columns = [...prev.columns];
    for (const c of step.columns) {
      const i = columns.findIndex((x) => x.column === c.column);
      if (i >= 0) columns[i] = c;
      else columns.push(c);
    }
    const merged = [...steps];
    merged[at - 1] = { ...prev, columns };
    return { steps: merged, index: at - 1 };
  }
  const out = [...steps];
  out.splice(at, 0, step);
  return { steps: out, index: at };
}

/** Steps with step `i` moved one place up (`-1`) or down (`1`). */
export function moveStep(steps: QueryStep[], i: number, dir: -1 | 1): QueryStep[] {
  const j = i + dir;
  if (i < 0 || j < 0 || i >= steps.length || j >= steps.length) return steps;
  const out = [...steps];
  [out[i], out[j]] = [out[j]!, out[i]!];
  return out;
}

/** Steps without step `i`. */
export function removeStep(steps: QueryStep[], i: number): QueryStep[] {
  return steps.filter((_, k) => k !== i);
}

/** Column header menu actions that make a step without asking anything. */
export type ColumnAction =
  | 'sortAsc'
  | 'sortDesc'
  | 'remove'
  | 'removeOthers'
  | 'removeDuplicates'
  | 'removeBlank'
  | 'unpivot'
  | 'unpivotOthers'
  | `type:${QueryColType}`;

/** The step a column header menu action adds for `column` (of `columns`). */
export function columnStep(action: ColumnAction, column: string, columns: string[]): QueryStep {
  if (action.startsWith('type:')) {
    return { op: 'changeType', columns: [{ column, type: action.slice(5) as QueryColType }] };
  }
  switch (action) {
    case 'sortAsc':
      return { op: 'sort', keys: [{ column, descending: false }] };
    case 'sortDesc':
      return { op: 'sort', keys: [{ column, descending: true }] };
    case 'remove':
      return { op: 'removeColumns', columns: [column] };
    case 'removeOthers':
      return { op: 'keepColumns', columns: columns.filter((c) => c === column) };
    case 'removeDuplicates':
      return { op: 'removeDuplicates', columns: [column] };
    case 'removeBlank':
      return { op: 'filterRows', column, condition: 'isNotBlank' };
    case 'unpivot':
      return { op: 'unpivot', columns: [column] };
    default:
      return { op: 'unpivot', columns: [column], other: true };
  }
}

/** The query a preview of steps `0..upto` evaluates. */
export function previewQuery(def: QueryDef, upto: number): { query: QueryDef; upto: number } {
  return { query: def, upto: Math.max(0, Math.min(upto, def.steps.length)) };
}

/** A translator (i18next `t`). */
export type Tr = (key: string, opts?: Record<string, unknown>) => string;

const list = (xs: string[] | undefined) => (xs ?? []).join(', ');

/** Text of a step in the step list. */
export function stepLabel(step: QueryStep, t: Tr): string {
  const k = (op: string, o?: Record<string, unknown>) => t(`sheetQuery.stepText.${op}`, o);
  switch (step.op) {
    case 'changeType':
      return k('changeType', {
        columns: step.columns
          .map((c) => `${c.column} → ${t(`sheetQuery.types.${c.type}`)}`)
          .join(', '),
      });
    case 'filterRows':
      return k('filterRows', {
        column: step.column,
        condition: t(`sheetQuery.conditions.${step.condition}`),
        value: step.value ?? '',
      });
    case 'removeColumns':
    case 'keepColumns':
    case 'reorderColumns':
    case 'removeDuplicates':
      return k(step.op, { columns: list(step.columns) || t('sheetQuery.allColumns') });
    case 'renameColumns':
      return k('renameColumns', {
        renames: step.renames.map((r) => `${r.from} → ${r.to}`).join(', '),
      });
    case 'splitColumn':
      return k('splitColumn', { column: step.column, delimiter: step.delimiter });
    case 'replaceValues':
      return k('replaceValues', {
        find: step.find,
        replace: step.replace,
        columns: list(step.columns) || t('sheetQuery.allColumns'),
      });
    case 'groupBy':
      return k('groupBy', {
        keys: list(step.keys),
        aggregates: step.aggregates
          .map((a) => `${a.name} = ${t(`sheetQuery.aggregates.${a.op}`)}(${a.column ?? ''})`)
          .join(', '),
      });
    case 'sort':
      return k('sort', {
        keys: step.keys.map((s) => `${s.column} ${s.descending ? '↓' : '↑'}`).join(', '),
      });
    case 'merge':
      return k('merge', {
        query: step.query,
        kind: t(`sheetQuery.joins.${step.kind ?? 'leftOuter'}`),
      });
    case 'append':
      return k('append', { queries: list(step.queries) });
    case 'unpivot':
      return k(step.other ? 'unpivotOther' : 'unpivot', { columns: list(step.columns) });
    case 'addColumn':
      return k('addColumn', { name: step.name, formula: step.formula });
    case 'keepTop':
    case 'removeTop':
      return k(step.op, { count: step.count });
    default:
      return k(step.op);
  }
}

/** Text of the source in the step list. */
export function sourceLabel(source: QuerySource, t: Tr): string {
  const key =
    source.kind === 'web'
      ? source.url
      : source.kind === 'folder'
        ? source.folder
        : 'file' in source
          ? source.file
          : source.query;
  return t(`sheetQuery.sourceText.${source.kind}`, { key: baseName(key) || key });
}

/** Conditions of a filter, in menu order. */
export const CONDITIONS: QueryCondition[] = [
  'equals',
  'notEquals',
  'greaterThan',
  'greaterOrEqual',
  'lessThan',
  'lessOrEqual',
  'contains',
  'notContains',
  'beginsWith',
  'endsWith',
  'isBlank',
  'isNotBlank',
];

/** Column types, in menu order. */
export const COL_TYPES: QueryColType[] = ['text', 'number', 'integer', 'date', 'bool', 'any'];

/** Text of an engine error (`missingSource: a.csv` …) for the user. */
export function errorText(
  e: { code: string; message: string; step: number | null } | undefined,
  t: Tr,
): string {
  if (!e) return '';
  const detail = e.message.includes(': ')
    ? e.message.slice(e.message.indexOf(': ') + 2)
    : e.message;
  const what = t(`sheetQuery.errors.${e.code}`, { detail, defaultValue: e.message });
  return e.step === null ? what : t('sheetQuery.errors.atStep', { step: e.step + 1, error: what });
}
