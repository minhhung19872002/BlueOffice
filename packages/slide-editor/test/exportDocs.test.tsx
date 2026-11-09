// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  defaultExportState,
  ExportDocsDialog,
  exportPlan,
  type ExportDocsState,
} from '../src/exportDocs';

afterEach(cleanup);

const base = defaultExportState('09/11/2026');

describe('exportPlan', () => {
  it('builds handout options; the master supplies header and footer by default', () => {
    expect(exportPlan(base, 'deck', 0, 7)).toEqual({
      options: {
        kind: 'handout',
        perPage: 6,
        order: 'horizontal',
        frame: true,
        paper: 'a4',
      },
      name: 'deck-handout-6.pdf',
      mime: 'application/pdf',
    });
    const own: ExportDocsState = {
      ...base,
      kind: 'notes',
      paper: 'letter',
      fromMaster: false,
      header: 'Lớp 10A',
      showDate: false,
      pageNumbers: false,
    };
    expect(exportPlan(own, 'deck', 0, 7)).toEqual({
      options: {
        kind: 'notes',
        paper: 'letter',
        header: 'Lớp 10A',
        footer: '',
        date: '',
        pageNumbers: false,
      },
      name: 'deck-notes.pdf',
      mime: 'application/pdf',
    });
  });

  it('exports the current slide as one picture or all slides as a zip', () => {
    const jpg: ExportDocsState = { ...base, kind: 'image', format: 'jpg', quality: 75 };
    expect(exportPlan(jpg, 'deck', 2, 7)).toEqual({
      options: { kind: 'image', format: 'jpg', slides: [2], width: 1280, quality: 75 },
      name: 'deck-3.jpg',
      mime: 'image/jpeg',
    });
    const all: ExportDocsState = {
      ...base,
      kind: 'image',
      format: 'svg',
      scope: 'all',
      width: 99999,
    };
    expect(exportPlan(all, 'deck', 0, 7)).toEqual({
      options: { kind: 'image', format: 'svg', width: 8192 },
      name: 'deck-svg.zip',
      mime: 'application/zip',
    });
    expect(exportPlan({ ...base, kind: 'package' }, 'deck', 0, 7)).toEqual({
      options: { kind: 'package' },
      name: 'deck-package.zip',
      mime: 'application/zip',
    });
  });
});

function renderDialog(onExport: (s: ExportDocsState) => void) {
  return render(
    <I18nextProvider i18n={createI18n('en-US')}>
      <ExportDocsDialog
        open
        count={7}
        slide={{ width: 960, height: 540 }}
        today="09/11/2026"
        busy={false}
        onClose={() => {}}
        onExport={onExport}
      />
    </I18nextProvider>,
  );
}

describe('ExportDocsDialog', () => {
  it('collects handout settings', () => {
    const onExport = vi.fn();
    renderDialog(onExport);
    expect(screen.getByTestId('export-docs-dialog').textContent).toContain('Handouts (PDF)');
    fireEvent.change(screen.getByTestId('export-per-page'), { target: { value: '3' } });
    // Three per page is a single column: the order does not apply.
    expect((screen.getByTestId('export-order') as HTMLSelectElement).disabled).toBe(true);
    fireEvent.change(screen.getByTestId('export-per-page'), { target: { value: '9' } });
    fireEvent.change(screen.getByTestId('export-order'), { target: { value: 'vertical' } });
    fireEvent.click(screen.getByTestId('export-frame'));
    fireEvent.change(screen.getByTestId('export-paper'), { target: { value: 'letter' } });
    fireEvent.click(screen.getByTestId('export-from-master'));
    fireEvent.change(screen.getByTestId('export-footer'), { target: { value: 'Bluestar' } });
    fireEvent.click(screen.getByTestId('export-docs-ok'));
    const s = onExport.mock.calls[0]![0] as ExportDocsState;
    expect(s).toMatchObject({
      kind: 'handout',
      perPage: 9,
      order: 'vertical',
      frame: false,
      paper: 'letter',
      fromMaster: false,
      footer: 'Bluestar',
      date: '09/11/2026',
    });
  });

  it('shows picture options with the computed height', () => {
    const onExport = vi.fn();
    renderDialog(onExport);
    fireEvent.click(screen.getByTestId('export-kind-image'));
    expect(screen.queryByTestId('export-paper')).toBeNull();
    fireEvent.change(screen.getByTestId('export-width'), { target: { value: '1920' } });
    expect(screen.getByTestId('export-size').textContent).toBe('1920 × 1080 pixels');
    fireEvent.change(screen.getByTestId('export-format'), { target: { value: 'jpg' } });
    fireEvent.change(screen.getByTestId('export-quality'), { target: { value: '60' } });
    fireEvent.click(screen.getByTestId('export-scope-all'));
    fireEvent.click(screen.getByTestId('export-docs-ok'));
    expect(onExport.mock.calls[0]![0]).toMatchObject({
      kind: 'image',
      format: 'jpg',
      width: 1920,
      quality: 60,
      scope: 'all',
    });
    fireEvent.click(screen.getByTestId('export-kind-package'));
    expect(screen.getByTestId('export-package-note').textContent).toContain('Open Font License');
  });
});
