// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { AdminFormatReport, EditCommand, EditResult } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  AdminFormatDialog,
  TEMPLATE_THUMBNAILS,
  TEMPLATES,
  TemplateDialog,
} from '../src/TemplateDialogs';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

describe('template gallery', () => {
  // `bo-cli` tests check that the ids match the engine's and their files are committed.
  it('lists the eight Nghị định 30 templates with thumbnails and names', () => {
    expect(TEMPLATES).toHaveLength(8);
    for (const id of TEMPLATES) {
      expect(TEMPLATE_THUMBNAILS[id]).toMatch(new RegExp(`${id}\\.png$`));
      for (const key of ['name', 'description', 'file']) {
        expect(i18n.exists(`templates.${id}.${key}`)).toBe(true);
      }
    }
  });

  it('shows a thumbnail per template and creates a document from the picked one', () => {
    const onCreate = vi.fn();
    const onClose = vi.fn();
    wrap(<TemplateDialog onCreate={onCreate} onClose={onClose} />);
    const cards = screen.getAllByRole('listitem');
    expect(cards).toHaveLength(8);
    expect(cards.every((c) => c.querySelector('img')?.getAttribute('src'))).toBe(true);
    expect(screen.getByTestId('template-quyet-dinh').textContent).toContain('Quyết định');
    fireEvent.click(screen.getByTestId('template-cong-van'));
    expect(onCreate).toHaveBeenCalledWith('cong-van', 'Công văn.docx');
  });
});

const report = (r: AdminFormatReport): EditResult => ({ adminFormat: r }) as unknown as EditResult;

describe('administrative format check', () => {
  it('lists the deviations and moves to the paragraph of one', async () => {
    const commands: EditCommand[] = [];
    const run = vi.fn(async (c: EditCommand) => {
      commands.push(c);
      return c.cmd === 'checkAdminFormat'
        ? report({
            found: ['quocHieu'],
            issues: [
              {
                id: 'quocHieu.size',
                component: 'quocHieu',
                property: 'size',
                severity: 'error',
                expected: '12–13 pt',
                actual: '14 pt',
                para: 4,
                text: 'CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM',
                count: 1,
                fixable: true,
              },
              {
                id: 'tieuNgu.underline',
                component: 'tieuNgu',
                property: 'underline',
                severity: 'error',
                expected: 'yes',
                actual: 'no',
                para: 5,
                count: 1,
                fixable: false,
              },
              {
                id: 'page.marginLeft',
                component: 'page',
                property: 'marginLeft',
                severity: 'error',
                expected: '30–35 mm',
                actual: '25.0 mm',
                section: 0,
                count: 1,
                fixable: true,
              },
            ],
          })
        : null;
    });
    wrap(<AdminFormatDialog run={run} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('admin-format-issues')).toBeTruthy());
    expect(commands[0]).toEqual({ cmd: 'checkAdminFormat' });
    expect(screen.getByTestId('admin-format-summary').textContent).toContain('3');
    const size = document.querySelector('[data-issue="quocHieu.size"]')!;
    expect(size.textContent).toContain('Quốc hiệu');
    expect(size.textContent).toContain('Cỡ chữ');
    expect(size.textContent).toContain('Quy định: 12–13 pt');
    expect(size.textContent).toContain('Hiện tại: 14 pt');
    const line = document.querySelector('[data-issue="tieuNgu.underline"]')!;
    expect(line.textContent).toContain('Đường kẻ bên dưới');
    expect(line.textContent).toContain('Hiện tại: không');
    // A page rule has no paragraph to go to.
    const margin = document.querySelector('[data-issue="page.marginLeft"] button')!;
    expect(margin.hasAttribute('disabled')).toBe(true);
    fireEvent.click(size.querySelector('button')!);
    expect(commands.at(-1)).toEqual({
      cmd: 'setSelection',
      selection: { anchor: { p: 4, o: 0 }, focus: { p: 4, o: 0 } },
    });
    fireEvent.click(screen.getByTestId('admin-format-recheck'));
    expect(commands.filter((c) => c.cmd === 'checkAdminFormat')).toHaveLength(2);
  });

  it('fixes one deviation, then all fixable ones', async () => {
    const issue = (id: string, fixable: boolean, para?: number) => {
      const [component = '', property = ''] = id.split('.');
      return {
        id,
        component,
        property,
        severity: 'error' as const,
        expected: 'yes',
        actual: 'no',
        count: 2,
        fixable,
        ...(para === undefined ? {} : { para }),
      };
    };
    let issues = [
      issue('page.marginTop', true),
      issue('quocHieu.bold', true, 3),
      issue('tieuNgu.underline', false, 4),
      issue('soKyHieu.missing', false),
    ];
    const commands: EditCommand[] = [];
    const run = vi.fn(async (c: EditCommand) => {
      commands.push(c);
      if (c.cmd === 'fixAdminFormat') {
        issues = issues.filter((i) => !i.fixable || (c.ids !== undefined && !c.ids.includes(i.id)));
      }
      return report({ found: [], issues });
    });
    wrap(<AdminFormatDialog run={run} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByTestId('admin-format-issues')).toBeTruthy());
    // Only fixable deviations offer "Sửa"; the footer counts them.
    expect(screen.getAllByTestId('admin-format-fix')).toHaveLength(2);
    expect(
      document.querySelector('[data-issue="tieuNgu.underline"] [data-testid="admin-format-fix"]'),
    ).toBeNull();
    const all = screen.getByTestId('admin-format-fix-all');
    expect(all.textContent).toBe('Sửa tất cả (2)');
    expect(document.querySelector('[data-issue="quocHieu.bold"]')!.textContent).toContain('Lỗi');
    expect(document.querySelector('[data-issue="quocHieu.bold"]')!.textContent).toContain('×2');

    fireEvent.click(
      document.querySelector('[data-issue="quocHieu.bold"] [data-testid="admin-format-fix"]')!,
    );
    await waitFor(() => expect(document.querySelector('[data-issue="quocHieu.bold"]')).toBeNull());
    expect(commands.at(-1)).toEqual({ cmd: 'fixAdminFormat', ids: ['quocHieu.bold'] });
    expect(screen.getByTestId('admin-format-summary').textContent).toContain('Hoàn tác');
    expect(all.textContent).toBe('Sửa tất cả (1)');

    fireEvent.click(all);
    await waitFor(() => expect(document.querySelector('[data-issue="page.marginTop"]')).toBeNull());
    expect(commands.at(-1)).toEqual({ cmd: 'fixAdminFormat' });
    // What cannot be fixed stays listed; nothing is left to fix.
    expect(screen.getByTestId('admin-format-summary').textContent).toContain('2');
    expect(screen.getByTestId('admin-format-fix-all').hasAttribute('disabled')).toBe(true);
    expect(screen.queryAllByTestId('admin-format-fix')).toHaveLength(0);
  });

  it('confirms a conforming document', async () => {
    const run = vi.fn(async () => report({ found: ['quocHieu', 'tieuNgu'], issues: [] }));
    wrap(<AdminFormatDialog run={run} onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByTestId('admin-format-summary').textContent).toContain(
        'Không phát hiện sai lệch',
      ),
    );
  });
});
