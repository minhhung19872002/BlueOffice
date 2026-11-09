// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { ChartGallery, SlideCommand, SlideEditResult } from '@blueoffice/platform';
import { Ribbon } from '@blueoffice/ui';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { slideChartDesignGroups, slideGalleryLoader } from '../src/chartStyles';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const t = i18n.t.bind(i18n);
const svg = '<svg xmlns="http://www.w3.org/2000/svg"></svg>';
const gallery: ChartGallery = {
  styles: [{ id: 'paper', svg }],
  colors: [{ id: 'monoAccent1', colors: ['#123456'] }],
  layouts: [{ id: 'layout6', svg }],
  template: { name: '', kind: 'column' },
};

describe('BlueSlide chart design', () => {
  it('loads the gallery through the slide command channel and applies with setChart', async () => {
    const sent: SlideCommand[] = [];
    const engine = vi.fn(async (c: SlideCommand) => {
      sent.push(c);
      return { chartGallery: gallery } as unknown as SlideEditResult;
    });
    const load = slideGalleryLoader(engine);
    expect(await load()).toEqual(gallery);
    expect(sent[0]).toMatchObject({ cmd: 'chartGallery' });
    const run = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <Ribbon
          tabs={[
            {
              id: 'chart',
              label: 'Biểu đồ',
              groups: slideChartDesignGroups({
                t,
                load,
                run,
                enabled: true,
                onSaveTemplate: () => {},
              }),
            },
          ]}
        />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('chart-styles'));
    const paper = await screen.findByTestId('chart-styles-grid-paper');
    expect(paper.getAttribute('aria-label')).toBe('Giấy');
    fireEvent.click(paper);
    fireEvent.click(screen.getByTestId('chart-colors'));
    fireEvent.click(await screen.findByTestId('chart-colors-monoAccent1'));
    await waitFor(() => expect(run).toHaveBeenCalledTimes(2));
    expect(run.mock.calls[0]![0]).toMatchObject({ cmd: 'setChart', style: 'paper' });
    expect(run.mock.calls[1]![0]).toMatchObject({
      cmd: 'setChart',
      colors: 'monoAccent1',
      texts: { title: 'Tiêu đề biểu đồ' },
    });
  });
});
