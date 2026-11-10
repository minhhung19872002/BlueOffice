// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  CiteInfo,
  CiteSource,
  CiteSourceInfo,
  EditCommand,
  EditResult,
  FileAccess,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  BibliographyDialog,
  CitationStylePicker,
  importFormat,
  InsertCitationDialog,
  isCitationDialog,
  SourceManagerDialog,
} from '../src/CitationDialogs';
import {
  fieldsFor,
  lcidOf,
  loadMasterList,
  MASTER_LIST_KEY,
  matchesSource,
  mergeSources,
  parsePeople,
  peopleText,
  placeholderTag,
  sanitizeSource,
  saveMasterList,
  sourceLine,
  withContributor,
} from '../src/citations';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const smith: CiteSource = {
  tag: 'Smi20',
  kind: 'Book',
  contributors: [{ role: 'Author', people: [{ last: 'Smith', first: 'John' }] }],
  fields: { Title: 'Research methods', Year: '2020' },
};
const who: CiteSource = {
  tag: 'WHO22',
  kind: 'Report',
  contributors: [{ role: 'Author', corporate: 'World Health Organization' }],
  fields: { Title: 'Global report', Year: '2022' },
};

const info = (s: CiteSource, extra: Partial<CiteSourceInfo> = {}): CiteSourceInfo => ({
  ...s,
  cited: false,
  placeholder: false,
  preview: [{ text: `${s.tag} entry ` }, { text: 'Title', italic: true }],
  citation: `(${s.tag})`,
  ...extra,
});

/** A fake engine: keeps the document's sources and records the commands. */
function engine(initial: CiteSourceInfo[] = [], state: Partial<CiteInfo> = {}) {
  let sources = initial;
  let style = state.style ?? 'APA';
  const sent: EditCommand[] = [];
  const cite = (more: Partial<CiteInfo> = {}): EditResult =>
    ({
      cite: { hasBibliography: false, changed: 0, ...state, style, sources, ...more },
    }) as EditResult;
  const run = vi.fn(async (c: EditCommand): Promise<EditResult | null> => {
    sent.push(c);
    switch (c.cmd) {
      case 'citeSetStyle':
        style = c.style;
        return cite();
      case 'citeSaveSources':
        for (const s of c.sources) {
          const tag = s.tag || 'New1';
          sources = [...sources.filter((x) => x.tag !== tag), info({ ...s, tag })];
        }
        return cite();
      case 'citeDeleteSource': {
        const s = sources.find((x) => x.tag === c.tag);
        if (s?.cited) return { ...cite(), error: 'cited' } as EditResult;
        sources = sources.filter((x) => x.tag !== c.tag);
        return cite();
      }
      case 'citeParse':
        return cite({ parsed: [who] });
      case 'citeExport':
        return cite({ exported: '@book{Smi20,\n}\n' });
      case 'citePreview':
        return cite({ preview: c.sources.map((s) => info(s)) });
      default:
        return cite();
    }
  });
  const only = (cmd: string) => sent.filter((c) => c.cmd === cmd);
  return { run, sent, only };
}

/** A setting store in memory (the master list). */
function settings(list: CiteSource[] = []) {
  const data = new Map<string, string>();
  if (list.length) data.set(MASTER_LIST_KEY, JSON.stringify(list));
  return {
    data,
    getSetting: async (k: string) => data.get(k) ?? null,
    setSetting: async (k: string, v: string | null) => {
      if (v === null) data.delete(k);
      else data.set(k, v);
    },
  };
}

/** The row of source `tag` in a list (`null` cast away: asserted by the callers). */
const row = (list: string, tag: string) =>
  screen.getByTestId(list).querySelector(`li[data-tag="${tag}"]`) as HTMLElement;

describe('citation helpers', () => {
  it('names typed as text', () => {
    expect(parsePeople('Smith, John K.\nJane Doe; Plato', false)).toEqual([
      { last: 'Smith', first: 'John K.' },
      { last: 'Doe', first: 'Jane' },
      { last: 'Plato' },
    ]);
    // A Vietnamese name without comma: the family name comes first.
    expect(parsePeople('Nguyễn Văn An', true)).toEqual([{ last: 'Nguyễn', first: 'Văn An' }]);
    expect(parsePeople('Nguyễn Văn An', false)).toEqual([{ last: 'An', first: 'Nguyễn Văn' }]);
    expect(peopleText([{ last: 'Smith', first: 'John', middle: 'K.' }, { last: 'Plato' }])).toBe(
      'Smith, John K.\nPlato',
    );
    expect(withContributor(smith, 'Author', { corporate: ' ACME ' }).contributors).toEqual([
      { role: 'Author', corporate: 'ACME' },
    ]);
    expect(withContributor(smith, 'Author', {}).contributors).toBeUndefined();
  });

  it('fields per source type, search, tags', () => {
    expect(fieldsFor({ tag: '', kind: 'JournalArticle' }, false)).toEqual([
      'Title',
      'Year',
      'JournalName',
      'Volume',
      'Issue',
      'Pages',
      'DOI',
      'URL',
    ]);
    // Elements a source already has are always shown; LCID is the language select.
    expect(
      fieldsFor({ tag: '', kind: 'Book', fields: { Custom: 'x', LCID: '1066' } }, false),
    ).toContain('Custom');
    expect(fieldsFor({ tag: '', kind: 'Book', fields: { LCID: '1066' } }, true)).not.toContain(
      'LCID',
    );
    expect(fieldsFor({ tag: '', kind: 'Book' }, true)).toContain('Comments');
    expect(matchesSource(smith, 'smith 2020')).toBe(true);
    expect(matchesSource(smith, 'smith 1999')).toBe(false);
    expect(matchesSource(who, 'health')).toBe(true);
    expect(sourceLine(smith)).toBe('Smith (2020). Research methods');
    expect(sourceLine({ tag: 'X1', kind: 'Book' })).toBe('X1');
    expect(placeholderTag(['Placeholder1', 'placeholder2'])).toBe('Placeholder3');
    expect(mergeSources([smith], [{ ...smith, kind: 'Misc' }, who]).map((s) => s.kind)).toEqual([
      'Misc',
      'Report',
    ]);
    expect(lcidOf('vi-VN')).toBe('1066');
    expect(lcidOf('en-US')).toBe('1033');
    expect(importFormat('a.ris', '')).toBe('ris');
    expect(importFormat('a.bib', 'TY  - JOUR')).toBe('bibtex');
    expect(importFormat('a.txt', 'TY  - JOUR\n')).toBe('ris');
    expect(importFormat('a.txt', '@book{x}')).toBe('bibtex');
    expect(isCitationDialog('manageSources')).toBe(true);
    expect(isCitationDialog('insertToa')).toBe(false);
  });

  it('master list: stored per user, damaged data dropped', async () => {
    const store = settings();
    expect(await loadMasterList(store)).toEqual([]);
    await saveMasterList(store, [smith, who]);
    expect(await loadMasterList(store)).toEqual([smith, who]);
    await saveMasterList(store, []);
    expect(store.data.has(MASTER_LIST_KEY)).toBe(false);
    store.data.set(
      MASTER_LIST_KEY,
      JSON.stringify([
        smith,
        { ...smith },
        { tag: '', kind: 'Book' },
        7,
        { tag: 'B', kind: 'Nope', fields: { 'bad key': 'x', Title: ' T ' }, contributors: [{}] },
      ]),
    );
    // Duplicates and entries without a tag are dropped, unknown kinds become Misc.
    expect(await loadMasterList(store)).toEqual([
      smith,
      { tag: 'B', kind: 'Misc', fields: { Title: 'T' } },
    ]);
    store.data.set(MASTER_LIST_KEY, '{not json');
    expect(await loadMasterList(store)).toEqual([]);
    expect(sanitizeSource(null)).toBeNull();
  });
});

describe('style picker', () => {
  it('shows the style of the document and changes it', async () => {
    const e = engine([], { style: 'IEEE' });
    wrap(<CitationStylePicker run={e.run} />);
    const select = screen.getByTestId<HTMLSelectElement>('cite-style');
    await waitFor(() => expect(select.value).toBe('IEEE'));
    expect(select.options.length).toBe(8);
    fireEvent.change(select, { target: { value: 'VNThesis' } });
    await waitFor(() =>
      expect(e.only('citeSetStyle')).toEqual([{ cmd: 'citeSetStyle', style: 'VNThesis' }]),
    );
    expect(select.value).toBe('VNThesis');
  });
});

describe('insert citation', () => {
  it('cites sources of the document and the master list with pages and hidden parts', async () => {
    const e = engine([info(smith, { cited: true })]);
    const onClose = vi.fn();
    wrap(
      <InsertCitationDialog
        run={e.run}
        settings={settings([smith, who])}
        language="en-US"
        onClose={onClose}
      />,
    );
    await waitFor(() => expect(row('cite-source-list', 'WHO22')).toBeTruthy());
    // The master-only source is marked; nothing is chosen yet.
    expect(row('cite-source-list', 'WHO22').textContent).toContain('master list');
    expect(screen.getByTestId<HTMLButtonElement>('cite-insert-ok').disabled).toBe(true);
    // Search narrows the list.
    fireEvent.change(screen.getByTestId('cite-search'), { target: { value: 'health' } });
    expect(within(screen.getByTestId('cite-source-list')).getAllByRole('option').length).toBe(1);
    fireEvent.change(screen.getByTestId('cite-search'), { target: { value: '' } });
    fireEvent.click(row('cite-source-list', 'Smi20'));
    fireEvent.click(row('cite-source-list', 'WHO22'));
    const first = within(
      screen.getByTestId('cite-items').querySelector('[data-tag="Smi20"]') as HTMLElement,
    );
    fireEvent.change(first.getByTestId('cite-item-pages'), { target: { value: '12-15' } });
    fireEvent.click(first.getByTestId('cite-item-noAuthor'));
    fireEvent.click(screen.getByTestId('cite-insert-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(e.only('citeInsert')).toEqual([
      {
        cmd: 'citeInsert',
        citation: {
          items: [{ tag: 'Smi20', pages: '12-15', noAuthor: true }, { tag: 'WHO22' }],
          lcid: '1033',
        },
        // The source taken from the master list joins the document.
        sources: [who],
      },
    ]);
  });

  it('makes a new source and a placeholder while citing', async () => {
    const e = engine();
    const store = settings();
    const onClose = vi.fn();
    wrap(<InsertCitationDialog run={e.run} settings={store} language="vi-VN" onClose={onClose} />);
    await waitFor(() => expect(e.only('citeState').length).toBe(1));
    fireEvent.click(screen.getByTestId('cite-new-source'));
    // Nothing to save yet.
    expect(screen.getByTestId<HTMLButtonElement>('cite-source-save').disabled).toBe(true);
    fireEvent.change(screen.getByTestId('cite-kind'), { target: { value: 'JournalArticle' } });
    fireEvent.change(screen.getByTestId('cite-author'), {
      target: { value: 'Nguyễn Văn An\nTrần, Thị Bình' },
    });
    fireEvent.change(screen.getByTestId('cite-field-Title'), { target: { value: 'Lúa lai' } });
    fireEvent.change(screen.getByTestId('cite-field-JournalName'), {
      target: { value: 'Tạp chí' },
    });
    fireEvent.click(screen.getByTestId('cite-source-save'));
    await waitFor(() => expect(screen.getByTestId('cite-insert-dialog')).toBeTruthy());
    expect(e.only('citeSaveSources')).toEqual([
      {
        cmd: 'citeSaveSources',
        sources: [
          {
            tag: '',
            kind: 'JournalArticle',
            // The Vietnamese interface starts a Vietnamese source: family name first.
            fields: { LCID: '1066', Title: 'Lúa lai', JournalName: 'Tạp chí' },
            contributors: [
              {
                role: 'Author',
                people: [
                  { last: 'Nguyễn', first: 'Văn An' },
                  { last: 'Trần', first: 'Thị Bình' },
                ],
              },
            ],
          },
        ],
      },
    ]);
    // Kept in the master list too, and chosen for the citation.
    await waitFor(() => expect(store.data.get(MASTER_LIST_KEY)).toContain('"tag":"New1"'));
    expect(screen.getByTestId('cite-items').querySelector('[data-tag="New1"]')).toBeTruthy();
    fireEvent.click(screen.getByTestId('cite-new-placeholder'));
    expect(row('cite-source-list', 'Placeholder1').textContent).toContain('placeholder');
    fireEvent.click(screen.getByTestId('cite-insert-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(e.only('citeInsert')).toEqual([
      {
        cmd: 'citeInsert',
        citation: { items: [{ tag: 'New1' }, { tag: 'Placeholder1' }], lcid: '1066' },
        sources: [{ tag: 'Placeholder1', kind: 'Book' }],
      },
    ]);
  });

  it('edits the citation at the caret', async () => {
    const e = engine([info(smith, { cited: true }), info(who)], {
      citation: { items: [{ tag: 'Smi20', pages: '5' }], lcid: '1066' },
    });
    const onClose = vi.fn();
    wrap(
      <InsertCitationDialog run={e.run} settings={settings()} language="en-US" onClose={onClose} />,
    );
    const pages = await screen.findByTestId<HTMLInputElement>('cite-item-pages');
    expect(pages.value).toBe('5');
    fireEvent.change(pages, { target: { value: '' } });
    fireEvent.click(screen.getByTestId('cite-item-noYear'));
    fireEvent.click(row('cite-source-list', 'WHO22'));
    fireEvent.click(screen.getByTestId('cite-insert-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(e.only('citeInsert')).toEqual([]);
    expect(e.only('citeEdit')).toEqual([
      {
        cmd: 'citeEdit',
        // The citation keeps its language.
        citation: {
          items: [{ tag: 'Smi20', pages: '', noYear: true }, { tag: 'WHO22' }],
          lcid: '1066',
        },
      },
    ]);
  });
});

describe('manage sources', () => {
  const files = (over: Partial<FileAccess> = {}): FileAccess => ({
    openFile: vi.fn(async () => null),
    saveFile: vi.fn(async () => 'sources.bib'),
    ...over,
  });

  it('copies between the lists, previews, deletes', async () => {
    const e = engine([info(smith, { cited: true })]);
    const store = settings([who]);
    wrap(
      <SourceManagerDialog
        run={e.run}
        settings={store}
        files={files()}
        language="en-US"
        onClose={() => {}}
      />,
    );
    await waitFor(() => expect(row('cite-master-list', 'WHO22')).toBeTruthy());
    expect(row('cite-document-list', 'Smi20').textContent).toContain('cited');
    // Master → document.
    fireEvent.click(row('cite-master-list', 'WHO22'));
    await waitFor(() =>
      expect(screen.getByTestId('cite-preview').textContent).toContain('(WHO22)'),
    );
    expect(screen.getByTestId('cite-preview').querySelector('em')?.textContent).toBe('Title');
    fireEvent.click(screen.getByTestId('cite-copy-to-document'));
    await waitFor(() => expect(row('cite-document-list', 'WHO22')).toBeTruthy());
    expect(e.only('citeSaveSources')).toEqual([{ cmd: 'citeSaveSources', sources: [who] }]);
    // Document → master.
    fireEvent.click(row('cite-document-list', 'Smi20'));
    fireEvent.click(screen.getByTestId('cite-copy-to-master'));
    await waitFor(() => expect(row('cite-master-list', 'Smi20')).toBeTruthy());
    expect(JSON.parse(store.data.get(MASTER_LIST_KEY) ?? '[]')).toEqual([who, smith]);
    // A cited source cannot be deleted: the refusal is shown.
    fireEvent.click(screen.getByTestId('cite-delete-source'));
    expect((await screen.findByTestId('cite-error')).textContent).toContain(
      'cited in the document',
    );
    expect(row('cite-document-list', 'Smi20')).toBeTruthy();
    // An uncited one can; a master entry is removed from the master list only.
    fireEvent.click(row('cite-document-list', 'WHO22'));
    fireEvent.click(screen.getByTestId('cite-delete-source'));
    await waitFor(() =>
      expect(
        screen.getByTestId('cite-document-list').querySelector('[data-tag="WHO22"]'),
      ).toBeNull(),
    );
    fireEvent.click(row('cite-master-list', 'WHO22'));
    fireEvent.click(screen.getByTestId('cite-delete-source'));
    await waitFor(() =>
      expect(JSON.parse(store.data.get(MASTER_LIST_KEY) ?? '[]')).toEqual([smith]),
    );
  });

  it('edits a source of the document (its citations follow a new tag)', async () => {
    const e = engine([info(smith, { cited: true })]);
    wrap(
      <SourceManagerDialog
        run={e.run}
        settings={settings()}
        files={files()}
        language="en-US"
        onClose={() => {}}
      />,
    );
    await waitFor(() => expect(row('cite-document-list', 'Smi20')).toBeTruthy());
    fireEvent.click(row('cite-document-list', 'Smi20'));
    fireEvent.click(screen.getByTestId('cite-edit-source'));
    expect(screen.getByTestId<HTMLTextAreaElement>('cite-author').value).toBe('Smith, John');
    fireEvent.change(screen.getByTestId('cite-field-Year'), { target: { value: '2021' } });
    fireEvent.change(screen.getByTestId('cite-tag'), { target: { value: 'Smith 2021!' } });
    // Every field of the schema can be shown.
    fireEvent.click(screen.getByTestId('cite-all-fields'));
    fireEvent.change(screen.getByTestId('cite-field-Comments'), { target: { value: 'note' } });
    fireEvent.change(screen.getByTestId('cite-role-Translator'), { target: { value: 'Lan Tran' } });
    fireEvent.click(screen.getByTestId('cite-source-save'));
    await waitFor(() => expect(screen.getByTestId('cite-manager-dialog')).toBeTruthy());
    expect(e.only('citeSaveSources')).toEqual([
      {
        cmd: 'citeSaveSources',
        replace: 'Smi20',
        sources: [
          {
            // Characters a field instruction cannot hold are dropped from the tag.
            tag: 'Smith2021',
            kind: 'Book',
            contributors: [
              { role: 'Author', people: [{ last: 'Smith', first: 'John' }] },
              { role: 'Translator', people: [{ last: 'Tran', first: 'Lan' }] },
            ],
            fields: { Title: 'Research methods', Year: '2021', Comments: 'note' },
          },
        ],
      },
    ]);
  });

  it('imports BibTeX / RIS and exports BibTeX', async () => {
    const e = engine([info(smith)]);
    const store = settings();
    const f = files({
      openFile: vi.fn(async () => ({
        name: 'refs.ris',
        bytes: new TextEncoder().encode('TY  - RPRT\nER  - \n'),
      })),
    });
    wrap(
      <SourceManagerDialog
        run={e.run}
        settings={store}
        files={f}
        language="en-US"
        onClose={() => {}}
      />,
    );
    await waitFor(() => expect(row('cite-document-list', 'Smi20')).toBeTruthy());
    fireEvent.click(screen.getByTestId('cite-import'));
    await waitFor(() =>
      expect(screen.getByTestId('cite-note').textContent).toBe('Sources imported: 1.'),
    );
    expect(f.openFile).toHaveBeenCalledWith(['.bib', '.ris', '.txt']);
    expect(e.only('citeParse')).toEqual([
      { cmd: 'citeParse', format: 'ris', text: 'TY  - RPRT\nER  - \n', taken: [] },
    ]);
    // The imported sources join the document and the master list.
    expect(row('cite-document-list', 'WHO22')).toBeTruthy();
    expect(JSON.parse(store.data.get(MASTER_LIST_KEY) ?? '[]')).toEqual([who]);
    fireEvent.click(screen.getByTestId('cite-export'));
    await waitFor(() => expect(f.saveFile).toHaveBeenCalled());
    const [bytes, name, mime] = (f.saveFile as ReturnType<typeof vi.fn>).mock.calls[0] as [
      Uint8Array,
      string,
      string,
    ];
    expect(new TextDecoder().decode(bytes)).toBe('@book{Smi20,\n}\n');
    expect([name, mime]).toEqual(['sources.bib', 'application/x-bibtex']);
  });
});

describe('bibliography', () => {
  it('inserts with a preset heading, a custom one or none', async () => {
    const e = engine();
    const onClose = vi.fn();
    const { unmount } = wrap(<BibliographyDialog run={e.run} language="en-US" onClose={onClose} />);
    fireEvent.click(screen.getByTestId('cite-bibliography-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    unmount();
    wrap(<BibliographyDialog run={e.run} language="vi-VN" onClose={onClose} />);
    fireEvent.change(screen.getByTestId('cite-heading'), { target: { value: 'custom' } });
    fireEvent.change(screen.getByTestId('cite-heading-text'), {
      target: { value: ' TÀI LIỆU THAM KHẢO ' },
    });
    fireEvent.click(screen.getByTestId('cite-bibliography-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(2));
    fireEvent.change(screen.getByTestId('cite-heading'), { target: { value: 'none' } });
    fireEvent.click(screen.getByTestId('cite-bibliography-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(3));
    fireEvent.change(screen.getByTestId('cite-heading'), { target: { value: 'worksCited' } });
    fireEvent.click(screen.getByTestId('cite-bibliography-ok'));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(4));
    expect(e.only('citeBibliography')).toEqual([
      { cmd: 'citeBibliography', heading: 'References', lcid: '1033' },
      { cmd: 'citeBibliography', heading: 'TÀI LIỆU THAM KHẢO', lcid: '1066' },
      { cmd: 'citeBibliography', heading: undefined, lcid: '1066' },
      { cmd: 'citeBibliography', heading: 'Works Cited', lcid: '1066' },
    ]);
  });
});
