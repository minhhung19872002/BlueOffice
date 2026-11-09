// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { ChartGallery, SheetCommand } from '@blueoffice/platform';
import { Ribbon, loadChartTemplates } from '@blueoffice/ui';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  SaveChartTemplateDialog,
  chartDesignGroups,
  templateMenuItems,
  templateOfItem,
} from '../src/chartStyles';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const i18n = createI18n('en-US');
const t = i18n.t.bind(i18n);
const svg = '<svg xmlns="http://www.w3.org/2000/svg"></svg>';
const gallery: ChartGallery = {
  styles: ['clean', 'night'].map((id) => ({ id, svg })),
  colors: [{ id: 'lotus', colors: ['#B23A6F'] }],
  layouts: ['layout3'].map((id) => ({ id, svg })),
  template: { name: '', kind: 'bar', legend: 'b', title: true, format: { seriesFill: 'gradient' } },
};

describe('BlueSheet Chart Design tab', () => {
  it('style, colours and layout run chartDesign on the selected chart', async () => {
    const calls: SheetCommand[] = [];
    const exec = vi.fn(async (c: SheetCommand) => {
      calls.push(c);
      return { ok: true };
    });
    render(
      <I18nextProvider i18n={i18n}>
        <Ribbon
          tabs={[
            {
              id: 'chartDesign',
              label: t('chartDesign.tab'),
              groups: chartDesignGroups({
                t,
                sheet: 1,
                index: 2,
                load: async () => gallery,
                exec,
                onSaveTemplate: () => {},
              }),
            },
          ]}
        />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('chart-styles'));
    const night = await screen.findByTestId('chart-styles-grid-night');
    // Style names come from the locale.
    expect(night.getAttribute('aria-label')).toBe('Night');
    fireEvent.click(night);
    fireEvent.click(screen.getByTestId('chart-colors'));
    fireEvent.click(await screen.findByTestId('chart-colors-lotus'));
    fireEvent.click(screen.getByTestId('chart-layouts'));
    fireEvent.click(await screen.findByTestId('chart-layouts-grid-layout3'));
    await waitFor(() => expect(calls).toHaveLength(3));
    expect(calls[0]).toMatchObject({ cmd: 'chartDesign', sheet: 1, index: 2, style: 'night' });
    expect(calls[1]).toMatchObject({ cmd: 'chartDesign', colors: 'lotus' });
    expect(calls[2]).toMatchObject({
      cmd: 'chartDesign',
      layout: 'layout3',
      texts: { xTitle: 'Axis title', title: 'Chart title' },
    });
  });

  it('save as template, then insert from it', async () => {
    const saved = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <SaveChartTemplateDialog load={async () => gallery} onClose={() => {}} onSaved={saved} />
      </I18nextProvider>,
    );
    fireEvent.change(screen.getByTestId('chart-template-name'), {
      target: { value: ' Doanh thu ' },
    });
    fireEvent.click(screen.getByTestId('chart-template-ok'));
    await waitFor(() => expect(saved).toHaveBeenCalledWith('Doanh thu'));
    const list = loadChartTemplates<{ name: string; kind: string }>();
    expect(list).toEqual([expect.objectContaining({ name: 'Doanh thu', kind: 'bar' })]);
    const items = templateMenuItems(t);
    expect(items).toEqual([{ id: 'template:Doanh thu', label: 'From template: Doanh thu' }]);
    expect(templateOfItem(items[0]!.id)?.kind).toBe('bar');
    expect(templateOfItem('column')).toBeNull();
  });
});
