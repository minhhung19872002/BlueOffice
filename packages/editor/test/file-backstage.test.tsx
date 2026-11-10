// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { DocProperties, RecentFile } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FileBackstage, OTHER_FORMATS, type FileBackstageProps } from '../src/FileBackstage';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const PROPS: DocProperties = {
  title: 'Báo cáo',
  subject: '',
  author: 'An',
  keywords: '',
  comments: '',
  category: '',
  status: '',
  company: '',
  manager: '',
  lastModifiedBy: 'Bình',
  created: '2026-01-02T03:04:05Z',
  modified: '',
  revision: '3',
};

function setup(over: Partial<FileBackstageProps> = {}) {
  const registered = new Set([
    'file.new',
    'file.open',
    'file.save',
    'file.protect',
    'review.checkDocument',
    'review.versions',
    'file.print',
    'file.printNow',
    'file.exportPdf',
    'file.exportPdfOptions',
    'view.toggleTheme',
    'review.autoCorrect',
  ]);
  const run = vi.fn();
  const onClose = vi.fn();
  const onSaveAs = vi.fn();
  let current = { ...PROPS };
  const save = vi.fn(async (change: Partial<DocProperties>) => {
    current = { ...current, ...change };
    return current;
  });
  const data = new Map<string, string | null>();
  const recent: RecentFile[] = [
    { name: 'a.docx', path: 'C:/a.docx', openedAt: 1 },
    { name: 'b.docx', path: 'C:/b.docx', openedAt: 2 },
  ];
  const openPath = vi.fn();
  const props: FileBackstageProps = {
    app: 'doc',
    open: true,
    onClose,
    commands: {
      has: (id) => registered.has(id),
      enabled: (id) => id !== 'file.exportPdfOptions',
      run,
    },
    hasDocument: true,
    fileName: 'bao-cao.docx',
    variant: 'docm',
    macros: true,
    properties: { load: async () => current, save },
    shell: {
      recentFiles: async () => recent,
      getSetting: async (k) => data.get(k) ?? null,
      setSetting: async (k, v) => void data.set(k, v),
    },
    openPath,
    onSaveAs,
    ...over,
  };
  wrap(<FileBackstage {...props} />);
  return { run, onClose, onSaveAs, save, data, openPath };
}

const nav = (name: string) =>
  within(screen.getByRole('navigation', { name: 'Tệp' })).getByRole('button', { name });

describe('FileBackstage', () => {
  it('opens on Info with the document properties, and saves an edited one', async () => {
    const { save } = setup();
    expect(screen.getByTestId('backstage').dataset.page).toBe('info');
    expect(screen.getByText('bao-cao.docx')).toBeTruthy();
    const title = (await screen.findByLabelText('Tiêu đề')) as HTMLInputElement;
    expect(title.value).toBe('Báo cáo');
    expect(screen.getByTestId('prop-lastModifiedBy').textContent).toBe('Bình');
    expect(screen.getByTestId('prop-revision').textContent).toBe('3');
    fireEvent.change(title, { target: { value: 'Báo cáo quý' } });
    fireEvent.blur(title);
    await waitFor(() => expect(save).toHaveBeenCalledWith({ title: 'Báo cáo quý' }));
    // Info hosts protection, check document and versions.
    for (const id of ['file.protect', 'review.checkDocument', 'review.versions'])
      expect(document.querySelector(`[data-command="${id}"]`)).toBeTruthy();
    expect(document.querySelector('[data-command="file.sign"]')).toBeNull();
  });

  it('runs a page command after closing the screen', () => {
    const { run, onClose } = setup();
    fireEvent.click(document.querySelector('[data-command="file.protect"]')!);
    expect(onClose).toHaveBeenCalled();
    expect(run).toHaveBeenCalledWith('file.protect');
    fireEvent.click(nav('Lưu'));
    expect(run).toHaveBeenLastCalledWith('file.save');
  });

  it('lists every document type of the app and the other formats on Save As', () => {
    const { onSaveAs, run } = setup();
    fireEvent.click(nav('Lưu thành'));
    const exts = Array.from(document.querySelectorAll('[data-format]')).map(
      (b) => (b as HTMLElement).dataset.format,
    );
    expect(exts).toEqual(['.docx', '.docm', '.dotx', '.dotm', '.pdf']);
    // The current type is marked; a macro-free type warns.
    expect(document.querySelector('[data-format=".docm"]')?.getAttribute('data-current')).toBe(
      'true',
    );
    expect(
      within(document.querySelector('[data-format=".docx"]') as HTMLElement).getByText(/macro/i),
    ).toBeTruthy();
    fireEvent.click(document.querySelector('[data-format=".dotx"]')!);
    expect(onSaveAs).toHaveBeenCalledWith('dotx');
    fireEvent.click(nav('Lưu thành'));
    fireEvent.click(document.querySelector('[data-format=".pdf"]')!);
    expect(run).toHaveBeenCalledWith('file.exportPdf');
  });

  it('offers the web and XML formats of each app on Save As (CORE-FMT-003)', () => {
    // What every app lists beside its document types.
    expect(OTHER_FORMATS.doc.map(([, ext]) => ext)).toEqual([
      'pdf',
      'html',
      'mht',
      'xml',
      'epub',
    ]);
    expect(OTHER_FORMATS.sheet.map(([, ext]) => ext)).toEqual([
      'csv',
      'pdf',
      'html',
      'mht',
      'xml',
      // SYLK, DIF, formatted text: the current sheet (CORE-FMT-004).
      'slk',
      'dif',
      'prn',
      'thmx',
    ]);
    // A presentation has Flat OPC but no HTML page set.
    expect(OTHER_FORMATS.slide.map(([, ext]) => ext)).toEqual(['pdf', 'png', 'gif', 'xml', 'thmx']);
    const web = ['file.saveHtml', 'file.saveMht', 'file.saveFlatXml'];
    const registered = new Set(['file.exportPdf', ...web]);
    const { run } = setup({
      commands: { has: (id) => registered.has(id), enabled: () => true, run: vi.fn() },
    });
    fireEvent.click(nav('Lưu thành'));
    const formats = Array.from(document.querySelectorAll('[data-format]')).map(
      (b) => (b as HTMLElement).dataset.format,
    );
    expect(formats.slice(4)).toEqual(['.pdf', '.html', '.mht', '.xml']);
    // Labels come from both locales' `backstage.formats`.
    expect(screen.getByText('Trang web (.html, một tệp)')).toBeTruthy();
    expect(screen.getByText('Trang web một tệp (.mht)')).toBeTruthy();
    expect(screen.getByText('Tài liệu XML (Flat OPC, .xml)')).toBeTruthy();
    expect(run).not.toHaveBeenCalled();
  });

  it('runs the command of a web format', () => {
    const run = vi.fn();
    const registered = new Set(['file.saveHtml', 'file.saveMht', 'file.saveFlatXml']);
    const { onClose } = setup({
      commands: { has: (id) => registered.has(id), enabled: () => true, run },
    });
    fireEvent.click(nav('Lưu thành'));
    fireEvent.click(document.querySelector('[data-format=".mht"]')!);
    expect(onClose).toHaveBeenCalled();
    expect(run).toHaveBeenCalledWith('file.saveMht');
    fireEvent.click(nav('Lưu thành'));
    fireEvent.click(document.querySelector('[data-format=".xml"]')!);
    expect(run).toHaveBeenLastCalledWith('file.saveFlatXml');
  });

  it('offers EPUB for documents and SYLK, DIF and formatted text for sheets (CORE-FMT-004)', () => {
    const run = vi.fn();
    const doc = new Set(['file.exportPdf', 'file.saveEpub']);
    const first = setup({
      commands: { has: (id) => doc.has(id), enabled: () => true, run },
    });
    fireEvent.click(nav('Lưu thành'));
    const formats = () =>
      Array.from(document.querySelectorAll('[data-format]')).map(
        (b) => (b as HTMLElement).dataset.format,
      );
    expect(formats().slice(4)).toEqual(['.pdf', '.epub']);
    expect(screen.getByText('Sách điện tử (EPUB, .epub)')).toBeTruthy();
    fireEvent.click(document.querySelector('[data-format=".epub"]')!);
    expect(first.onClose).toHaveBeenCalled();
    expect(run).toHaveBeenCalledWith('file.saveEpub');
    cleanup();

    // BlueSheet: the three formats of the current sheet, between Flat OPC and the theme.
    const sheet = new Set([
      'file.saveCsv',
      'file.saveFlatXml',
      'file.saveSlk',
      'file.saveDif',
      'file.savePrn',
      'file.saveThemeFile',
      'file.saveEpub', // registered or not, a sheet has no EPUB entry
    ]);
    setup({
      app: 'sheet',
      commands: { has: (id) => sheet.has(id), enabled: () => true, run },
    });
    fireEvent.click(nav('Lưu thành'));
    expect(formats().filter((f) => !f?.startsWith('.xl'))).toEqual([
      '.csv',
      '.xml',
      '.slk',
      '.dif',
      '.prn',
      '.thmx',
    ]);
    expect(screen.getByText('SYLK (liên kết ký hiệu, .slk)')).toBeTruthy();
    expect(screen.getByText('DIF (định dạng trao đổi dữ liệu, .dif)')).toBeTruthy();
    expect(
      screen.getByText('Văn bản có định dạng (phân cách bằng dấu cách, .prn)'),
    ).toBeTruthy();
    for (const [ext, command] of [
      ['.slk', 'file.saveSlk'],
      ['.dif', 'file.saveDif'],
      ['.prn', 'file.savePrn'],
    ] as const) {
      fireEvent.click(document.querySelector(`[data-format="${ext}"]`)!);
      expect(run).toHaveBeenLastCalledWith(command);
      fireEvent.click(nav('Lưu thành'));
    }
  });

  it('pins a recent file, keeps it first and stores it for the user', async () => {
    const { data, openPath } = setup();
    fireEvent.click(nav('Mở'));
    await screen.findByText('b.docx');
    fireEvent.click(screen.getByRole('button', { name: 'Ghim: b.docx' }));
    await waitFor(() =>
      expect(JSON.parse(data.get('blueoffice.pinned.doc') ?? '[]')).toEqual([
        { name: 'b.docx', path: 'C:/b.docx' },
      ]),
    );
    const items = within(screen.getByRole('list', { name: 'Đã ghim và gần đây' })).getAllByRole(
      'listitem',
    );
    expect(items.map((li) => li.textContent?.slice(0, 6))).toEqual(['b.docx', 'a.docx']);
    expect(items[0]!.dataset.pinned).toBe('true');
    expect(
      screen.getByRole('button', { name: 'Bỏ ghim: b.docx' }).getAttribute('aria-pressed'),
    ).toBe('true');
    fireEvent.click(screen.getByText('a.docx'));
    expect(openPath).toHaveBeenCalledWith('C:/a.docx');
  });

  it('shows print, export and options commands the app registers', () => {
    setup();
    fireEvent.click(nav('In'));
    expect(document.querySelector('[data-command="file.print"]')).toBeTruthy();
    expect(document.querySelector('[data-command="file.printNow"]')).toBeTruthy();
    fireEvent.click(nav('Xuất'));
    expect(
      (document.querySelector('[data-command="file.exportPdfOptions"]') as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(document.querySelector('[data-command="file.exportVideo"]')).toBeNull();
    fireEvent.click(nav('Chia sẻ'));
    expect(screen.getByText(/Chia sẻ có khi/)).toBeTruthy();
    fireEvent.click(nav('Tùy chọn'));
    expect(document.querySelector('[data-command="view.toggleTheme"]')).toBeTruthy();
    expect(document.querySelector('[data-command="review.autoCorrect"]')).toBeTruthy();
    // No density command: the screen offers the setting itself.
    expect(screen.getByTestId('backstage-density')).toBeTruthy();
    // No host close: no Close entry.
    expect(screen.queryByRole('button', { name: 'Đóng' })).toBeNull();
  });

  it('without a document: opens on Open and disables the document pages', () => {
    setup({ hasDocument: false, onCloseDocument: () => {} });
    expect(screen.getByTestId('backstage').dataset.page).toBe('open');
    expect((nav('Thông tin') as HTMLButtonElement).disabled).toBe(true);
    expect((nav('Lưu thành') as HTMLButtonElement).disabled).toBe(true);
    expect(nav('Đóng')).toBeTruthy();
  });
});
