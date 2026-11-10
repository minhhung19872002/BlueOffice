// @vitest-environment happy-dom
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  IconLibraryPicker,
  foldText,
  iconLocale,
  loadIconLibrary,
  searchIcons,
  type IconLibrary,
  type IconLibraryLabels,
  type IconLibraryManifest,
  type IconPick,
} from '../src';

afterEach(cleanup);

const DIR = join(dirname(fileURLToPath(import.meta.url)), '../../../assets/icons/library');
const manifest = JSON.parse(
  readFileSync(join(DIR, 'manifest.json'), 'utf8'),
) as IconLibraryManifest;
const svgFiles = readdirSync(DIR).filter((f) => f.endsWith('.svg'));

/** Elements and attributes the BlueOffice SVG reader converts (`bo_drawingml::svg`). */
const ELEMENTS = new Set(['svg', 'title', 'path', 'rect', 'circle', 'ellipse']);
const ATTRIBUTES = new Set([
  'xmlns',
  'width',
  'height',
  'viewBox',
  'fill',
  'stroke',
  'stroke-width',
  'stroke-linecap',
  'stroke-linejoin',
  'transform',
  'd',
  'x',
  'y',
  'rx',
  'cx',
  'cy',
  'r',
  'ry',
]);
const COLOR = /^(none|#[0-9A-F]{6})$/;

interface Tag {
  name: string;
  attrs: Record<string, string>;
}

/** Strict well-formedness check of the small XML subset the icons use → its start tags. */
function parseXml(xml: string): Tag[] {
  const tags: Tag[] = [];
  const stack: string[] = [];
  let rest = xml.trim();
  let roots = 0;
  while (rest.length > 0) {
    if (!rest.startsWith('<')) {
      const lt = rest.indexOf('<');
      const text = lt < 0 ? rest : rest.slice(0, lt);
      // Character data only inside an element, without raw markup characters.
      if (stack.length === 0 || /[<>]|&(?!(amp|lt|gt|quot|apos);)/.test(text))
        throw new Error(`bad text "${text}"`);
      rest = rest.slice(text.length);
      continue;
    }
    const close = /^<\/([A-Za-z][\w-]*)>/.exec(rest);
    if (close) {
      if (stack.pop() !== close[1]) throw new Error(`unbalanced </${close[1]}>`);
      rest = rest.slice(close[0].length);
      continue;
    }
    const open = /^<([A-Za-z][\w-]*)((?:\s+[A-Za-z][\w:-]*="[^"<&]*")*)\s*(\/?)>/.exec(rest);
    if (!open) throw new Error(`bad markup at "${rest.slice(0, 40)}"`);
    const attrs: Record<string, string> = {};
    for (const m of (open[2] ?? '').matchAll(/([A-Za-z][\w:-]*)="([^"]*)"/g)) {
      const key = m[1] ?? '';
      if (key in attrs) throw new Error(`duplicate attribute ${key}`);
      attrs[key] = m[2] ?? '';
    }
    if (stack.length === 0) roots += 1;
    tags.push({ name: open[1] ?? '', attrs });
    if (!open[3]) stack.push(open[1] ?? '');
    rest = rest.slice(open[0].length);
  }
  if (stack.length > 0) throw new Error(`unclosed <${stack.join('><')}>`);
  if (roots !== 1) throw new Error(`${roots} root elements`);
  return tags;
}

describe('icon library manifest', { timeout: 30_000 }, () => {
  it('rejects malformed XML in the checker itself', () => {
    expect(() => parseXml('<svg><path d="M0 0"></svg>')).toThrow();
    expect(() => parseXml('<svg a=1/>')).toThrow();
    expect(() => parseXml('<svg/><svg/>')).toThrow();
    expect(parseXml('<svg a="1"><title>x &amp; y</title><path d="M0 0"/></svg>')).toHaveLength(3);
  });

  it('lists every SVG file exactly once, with unique ids', () => {
    expect(manifest.icons.length).toBeGreaterThanOrEqual(150);
    const ids = manifest.icons.map((i) => i.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.filter((id) => !/^[a-z][a-z0-9-]*$/.test(id))).toEqual([]);
    expect(manifest.icons.map((i) => i.file).sort()).toEqual([...svgFiles].sort());
    expect(manifest.icons.filter((i) => i.file !== `${i.id}.svg`)).toEqual([]);
  });

  // The per-icon checks collect what is wrong and assert once: one readable failure, and
  // no tens of thousands of assertions.
  it('gives every icon a category, both names and keywords in both languages', () => {
    const categories = new Set(manifest.categories.map((c) => c.id));
    expect(categories.size).toBe(manifest.categories.length);
    const problems: string[] = [];
    for (const c of manifest.categories) {
      if (!c.name.vi.trim() || !c.name.en.trim()) problems.push(`category ${c.id}: name`);
      if (!manifest.icons.some((i) => i.category === c.id))
        problems.push(`category ${c.id}: empty`);
    }
    const names = { vi: new Set<string>(), en: new Set<string>() };
    for (const i of manifest.icons) {
      if (!categories.has(i.category)) problems.push(`${i.id}: category ${i.category}`);
      for (const l of ['vi', 'en'] as const) {
        if (!i.name[l].trim()) problems.push(`${i.id}: no ${l} name`);
        if (names[l].has(i.name[l])) problems.push(`${i.id}: duplicate ${l} name ${i.name[l]}`);
        names[l].add(i.name[l]);
        if (i.keywords[l].length === 0) problems.push(`${i.id}: no ${l} keywords`);
        if (i.keywords[l].some((k) => !k.trim())) problems.push(`${i.id}: empty ${l} keyword`);
      }
      // The unaccented spelling of the Vietnamese name and of every keyword is searchable.
      const vi = new Set(i.keywords.vi);
      for (const k of [i.name.vi, ...i.keywords.vi]) {
        if (!vi.has(foldText(k))) problems.push(`${i.id}: "${k}" has no unaccented keyword`);
      }
      // …and at least one accented form is kept.
      if (!i.keywords.vi.some((k) => foldText(k) !== k.toLowerCase()))
        problems.push(`${i.id}: no accented Vietnamese keyword`);
    }
    expect(problems).toEqual([]);
  });

  it('holds well-formed, self-contained SVG on the 24 × 24 grid', () => {
    expect(manifest.grid).toBe(24);
    const problems: string[] = [];
    for (const i of manifest.icons) {
      const xml = readFileSync(join(DIR, i.file), 'utf8');
      const bad = (what: string) => problems.push(`${i.id}: ${what}`);
      let tags: Tag[];
      try {
        tags = parseXml(xml);
      } catch (e) {
        bad(String(e));
        continue;
      }
      const root = tags[0];
      if (root?.name !== 'svg') bad('root element');
      if (root?.attrs.xmlns !== 'http://www.w3.org/2000/svg') bad('namespace');
      if (root?.attrs.viewBox !== '0 0 24 24') bad('viewBox');
      if (!(Number(root?.attrs.width) > 0) || root?.attrs.height !== root?.attrs.width) bad('size');
      // Only converted elements and plain presentation attributes: no script, style,
      // event handler, link, image, `use` or other reference to anything outside the file.
      const shapes = tags.filter((t) => !['svg', 'title'].includes(t.name));
      if (shapes.length === 0) bad('nothing drawn');
      for (const t of tags) {
        if (!ELEMENTS.has(t.name)) bad(`<${t.name}>`);
        for (const [k, v] of Object.entries(t.attrs)) {
          if (!ATTRIBUTES.has(k)) bad(`${t.name}@${k}`);
          if (k !== 'xmlns' && /url\(|:\/\/|javascript:|data:/i.test(v)) bad(`reference in ${k}`);
        }
      }
      // Explicit colours on every drawn element (no `currentColor`, nothing inherited).
      for (const t of shapes) {
        if (!COLOR.test(t.attrs.fill ?? '')) bad(`fill "${t.attrs.fill}"`);
        if (!COLOR.test(t.attrs.stroke ?? '')) bad(`stroke "${t.attrs.stroke}"`);
        if (t.attrs.fill === 'none' && t.attrs.stroke === 'none') bad('invisible element');
      }
      if (/currentColor/i.test(xml)) bad('currentColor');
      // The <title> is the English name.
      if (!xml.includes(`<title>${i.name.en.replace(/&/g, '&amp;')}</title>`)) bad('title');
    }
    expect(problems).toEqual([]);
  });

  it('stays small', () => {
    const bytes = readdirSync(DIR).reduce((n, f) => n + statSync(join(DIR, f)).size, 0);
    expect(bytes).toBeLessThan(400 * 1024);
  });
});

describe('icon search', () => {
  const ids = (q: string, category?: string) =>
    searchIcons(manifest.icons, q, category).map((i) => i.id);

  it('lists everything for an empty query, or one category', () => {
    expect(ids('')).toEqual(manifest.icons.map((i) => i.id));
    expect(ids('   ')).toHaveLength(manifest.icons.length);
    const arrows = ids('', 'arrows');
    expect(arrows.length).toBeGreaterThan(5);
    expect(arrows).toEqual(manifest.icons.filter((i) => i.category === 'arrows').map((i) => i.id));
  });

  it('finds Vietnamese names with and without diacritics, in any case', () => {
    for (const q of ['Mũi tên phải', 'mui ten phai', 'MUI TEN PHAI', 'mũi tên phai']) {
      expect(ids(q)[0], q).toBe('arrow-right');
    }
    // đ / Đ fold to d.
    expect(ids('con dau')).toContain('round-seal');
    expect(ids('con dấu')).toContain('round-seal');
    expect(ids('Đồng hồ')[0]).toBe('clock');
    expect(ids('dong ho')[0]).toBe('clock');
    expect(ids('chu ky')[0]).toBe('signature');
    expect(ids('chữ ký')[0]).toBe('signature');
  });

  it('finds English names and keywords', () => {
    expect(ids('calendar').slice(0, 2)).toEqual(['calendar', 'calendar-check']);
    expect(ids('Heart')[0]).toBe('heart');
    // Keyword only: "dentist" is not in any name.
    expect(ids('dentist')).toEqual(['tooth']);
    // The exact keyword wins over word prefixes ("nha" also starts "nhãn khoa", ophthalmology).
    expect(ids('nha khoa')[0]).toBe('tooth');
    expect(ids('nha khoa')).toContain('eye');
    expect(ids('nhan khoa')[0]).toBe('eye');
    expect(ids('nhan khoa')).not.toContain('tooth');
  });

  it('requires every word and honours the category filter', () => {
    expect(ids('arrow')).toContain('arrow-left');
    expect(ids('arrow left')).not.toContain('arrow-right');
    expect(ids('arrow left')[0]).toBe('arrow-left');
    expect(ids('zzzz')).toEqual([]);
    // "report" matches icons of several categories; the filter keeps one.
    const all = ids('bao cao');
    const admin = ids('bao cao', 'admin');
    expect(admin.length).toBeGreaterThan(0);
    expect(admin.length).toBeLessThan(all.length);
    expect(admin.every((id) => manifest.icons.find((i) => i.id === id)?.category === 'admin')).toBe(
      true,
    );
  });

  it('ranks a name match above a keyword match', () => {
    // "Thư" is the name of `mail`; other icons only have it inside keywords / longer names.
    expect(ids('thư')[0]).toBe('mail');
    expect(ids('star')[0]).toBe('star');
  });

  it('maps UI languages to a name locale', () => {
    expect(iconLocale('vi-VN')).toBe('vi');
    expect(iconLocale('vi')).toBe('vi');
    expect(iconLocale('en-US')).toBe('en');
    expect(iconLocale(undefined)).toBe('en');
  });
});

describe('icon library chunk', { timeout: 30_000 }, () => {
  it('holds the same manifest and SVG documents as assets/icons/library', async () => {
    const lib = await loadIconLibrary();
    expect(lib.manifest).toEqual(manifest);
    const differing = manifest.icons
      .filter((i) => lib.svg(i.id) !== readFileSync(join(DIR, i.file), 'utf8'))
      .map((i) => i.id);
    expect(differing).toEqual([]);
    expect(lib.svg('no-such-icon')).toBeUndefined();
  });
});

const labels: IconLibraryLabels = {
  title: 'Icons',
  search: 'Search',
  category: 'Category',
  allCategories: 'All',
  grid: 'Icon library',
  loading: 'Loading',
  loadError: 'Load failed',
  noResults: 'Nothing found',
  found: (n) => `${n} found`,
  selected: (n) => `${n} selected`,
  clear: 'Clear',
  insert: 'Insert',
  close: 'Close',
};

const svgOf = (id: string) => readFileSync(join(DIR, `${id}.svg`), 'utf8');
const library: IconLibrary = { manifest, svg: (id) => svgOf(id) };
const load = () => Promise.resolve(library);

async function open(props: Partial<Parameters<typeof IconLibraryPicker>[0]> = {}) {
  const onInsert = vi.fn<(icons: IconPick[]) => void>();
  const onClose = vi.fn();
  render(
    <IconLibraryPicker
      labels={labels}
      locale="vi"
      onInsert={onInsert}
      onClose={onClose}
      load={load}
      {...props}
    />,
  );
  await screen.findByText(`${manifest.icons.length} found`);
  return { onInsert, onClose };
}
// Attribute selectors: role queries compute the accessible name of all 210 cells each time.
const cells = () => [...document.querySelectorAll<HTMLElement>('[role="gridcell"]')];
const findCell = (name: string) =>
  document.querySelector<HTMLElement>(`[role="gridcell"][aria-label="${name}"]`);
const cell = (name: string) => {
  const el = findCell(name);
  if (!el) throw new Error(`no gridcell "${name}"`);
  return el;
};

describe('IconLibraryPicker', { timeout: 30_000 }, () => {
  it('shows a loading state, then every icon with an accessible name', async () => {
    let resolve: (lib: IconLibrary) => void = () => {};
    render(
      <IconLibraryPicker
        labels={labels}
        locale="vi"
        onInsert={() => {}}
        load={() => new Promise<IconLibrary>((r) => (resolve = r))}
      />,
    );
    const grid = screen.getByRole('grid', { name: 'Icon library' });
    expect(grid.getAttribute('aria-busy')).toBe('true');
    expect(screen.getByText('Loading')).toBeTruthy();
    resolve(library);
    await screen.findByText(`${manifest.icons.length} found`);
    expect(grid.getAttribute('aria-busy')).toBe('false');
    expect(grid.getAttribute('aria-multiselectable')).toBe('true');
    const all = cells();
    expect(all).toHaveLength(manifest.icons.length);
    // Names follow the locale; the picture itself is decorative.
    expect(all[0]?.getAttribute('aria-label')).toBe(manifest.icons[0]?.name.vi);
    const img = all[0]?.querySelector('img');
    expect(img?.getAttribute('alt')).toBe('');
    expect(img?.getAttribute('src')).toMatch(/^data:image\/svg\+xml;utf8,/);
    // One tab stop in the grid.
    expect(all.filter((c) => c.tabIndex === 0)).toHaveLength(1);
    // The accessibility tree exposes the cell by its name.
    expect(screen.getByRole('gridcell', { name: 'Con dấu tròn' })).toBe(cell('Con dấu tròn'));
  });

  it('reports a load failure', async () => {
    render(
      <IconLibraryPicker
        labels={labels}
        locale="en"
        onInsert={() => {}}
        load={() => Promise.reject(new Error('offline'))}
      />,
    );
    expect((await screen.findByRole('alert')).textContent).toBe('Load failed');
  });

  it('filters by search text (accents ignored) and by category', async () => {
    await open();
    fireEvent.change(screen.getByTestId('icon-search'), { target: { value: 'con dau' } });
    expect(cell('Con dấu tròn')).toBeTruthy();
    expect(findCell('Mũi tên phải')).toBeNull();
    fireEvent.change(screen.getByTestId('icon-search'), { target: { value: 'zzzz' } });
    expect(screen.getByText('Nothing found')).toBeTruthy();
    expect(cells()).toHaveLength(0);

    fireEvent.change(screen.getByTestId('icon-search'), { target: { value: '' } });
    const select = screen.getByTestId('icon-category') as HTMLSelectElement;
    expect([...select.options].map((o) => o.textContent)).toEqual([
      'All',
      ...manifest.categories.map((c) => c.name.vi),
    ]);
    fireEvent.change(select, { target: { value: 'weather' } });
    const weather = manifest.icons.filter((i) => i.category === 'weather');
    expect(cells()).toHaveLength(weather.length);
    expect(screen.getByText(`${weather.length} found`)).toBeTruthy();
  });

  it('uses English names in the English locale', async () => {
    await open({ locale: 'en' });
    expect(cell('Arrow right')).toBeTruthy();
    expect(screen.getByRole('option', { name: 'Arrows' })).toBeTruthy();
  });

  it('moves the roving focus with the keyboard', async () => {
    await open({ columns: 8 });
    const first = cells()[0]!;
    first.focus();
    const grid = screen.getByTestId('icon-grid');
    const active = () => document.activeElement?.getAttribute('data-index');
    fireEvent.keyDown(grid, { key: 'ArrowRight' });
    await waitFor(() => expect(active()).toBe('1'));
    fireEvent.keyDown(grid, { key: 'ArrowDown' });
    await waitFor(() => expect(active()).toBe('9'));
    fireEvent.keyDown(grid, { key: 'Home' });
    await waitFor(() => expect(active()).toBe('8'));
    fireEvent.keyDown(grid, { key: 'End', ctrlKey: true });
    await waitFor(() => expect(active()).toBe(String(manifest.icons.length - 1)));
    fireEvent.keyDown(grid, { key: 'Home', ctrlKey: true });
    await waitFor(() => expect(active()).toBe('0'));
    // ArrowDown in the search box enters the grid.
    screen.getByTestId('icon-search').focus();
    fireEvent.keyDown(screen.getByTestId('icon-search'), { key: 'ArrowDown' });
    expect(active()).toBe('0');
  });

  it('selects several icons and inserts them in selection order', async () => {
    const { onInsert } = await open();
    const insert = screen.getByTestId('icon-insert') as HTMLButtonElement;
    fireEvent.click(cell('Lịch'));
    fireEvent.click(cell('Mũi tên phải'));
    expect(cell('Lịch').getAttribute('aria-selected')).toBe('true');
    expect(screen.getByTestId('icon-status').textContent).toBe('2 selected');
    // Space toggles the active cell: third icon by keyboard.
    cell('Mũi tên trái').focus();
    fireEvent.click(cell('Mũi tên trái'));
    fireEvent.keyDown(screen.getByTestId('icon-grid'), { key: ' ' });
    expect(cell('Mũi tên trái').getAttribute('aria-selected')).toBe('false');
    fireEvent.click(insert);
    await waitFor(() => expect(onInsert).toHaveBeenCalledTimes(1));
    expect(onInsert.mock.calls[0]?.[0]).toEqual([
      { id: 'calendar', name: 'Lịch', svg: svgOf('calendar') },
      { id: 'arrow-right', name: 'Mũi tên phải', svg: svgOf('arrow-right') },
    ]);
    // The selection is cleared after inserting.
    await waitFor(() =>
      expect(screen.getByTestId('icon-status').textContent).toBe(`${manifest.icons.length} found`),
    );
    // Clear button.
    fireEvent.click(cell('Lịch'));
    fireEvent.click(screen.getByRole('button', { name: 'Clear' }));
    expect(cell('Lịch').getAttribute('aria-selected')).toBe('false');
  });

  it('inserts one icon by double-click, and the active one by Enter', async () => {
    const { onInsert, onClose } = await open({ locale: 'en' });
    fireEvent.doubleClick(cell('Heart'));
    await waitFor(() => expect(onInsert).toHaveBeenCalledTimes(1));
    expect(onInsert.mock.calls[0]?.[0]).toEqual([
      { id: 'heart', name: 'Heart', svg: svgOf('heart') },
    ]);
    await waitFor(() =>
      expect((screen.getByTestId('icon-insert') as HTMLButtonElement).disabled).toBe(false),
    );
    // Nothing selected: Enter inserts the active (first) icon.
    fireEvent.keyDown(screen.getByTestId('icon-grid'), { key: 'Enter' });
    await waitFor(() => expect(onInsert).toHaveBeenCalledTimes(2));
    expect(onInsert.mock.calls[1]?.[0]?.map((i) => i.id)).toEqual([manifest.icons[0]?.id]);
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalled();
  });
});
