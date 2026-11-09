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
