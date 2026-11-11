// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { DocChartInfo, EditCommand, EditResult, FormatState } from '@blueoffice/platform';
import type { RibbonItem } from '@blueoffice/ui';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';
import { ChartDialog, type ChartDraft } from '../src/chart/ChartDialog';
import {
  chartCommand,
  chartDraftOf,
  DOC_CHART_WRAPS,
  docChartTabs,
  docGalleryLoader,
  newChartDraft,
  registerDocChartCommands,
  type ChartState,
} from '../src/chart/docCharts';

// CORE-CHART-007 in BlueDoc: the chart commands go through the document command bridge.

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = i18n.t.bind(i18n);

const info: DocChartInfo = {
  kind: 'column',
  title: 'Sales',
  legend: 'b',
  data: { categories: ['Q1', 'Q2'], series: [{ name: 'North', values: [1, 2] }] },
  embedded: true,
  width: 425.2,
  height: 255.1,
  wrap: 'inline',
  part: '/word/charts/chart1.xml',
};

function setup() {
  const sent: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    sent.push(c);
    return { chart: info } as unknown as EditResult;
  });
  const open = vi.fn();
  const r = new CommandRegistry<ChartState>();
  registerDocChartCommands(r, { t, run, open });
  return { r, sent, open };
}

const selected: ChartState = {
  doc: {},
  edit: { format: { chart: true } as FormatState, chart: { ...info, wrap: 'square' } },
};

describe('BlueDoc chart commands', () => {
  it('opens the insert dialog with a localized column chart', async () => {
    const { r, open, sent } = setup();
    expect(r.isEnabled('insert.chart', { doc: null })).toBe(false);
    await r.execute('insert.chart', { doc: {} });
    const [mode, draft] = open.mock.calls[0] as [string, ChartDraft];
    expect(mode).toBe('insert');
    expect(draft.kind).toBe('column');
    expect(draft.data.categories).toEqual(['Item 1', 'Item 2', 'Item 3', 'Item 4']);
    expect(draft.data.series.map((s) => s.name)).toEqual(['Series 1', 'Series 2', 'Series 3']);
    expect(sent).toEqual([]);
  });

  it('edits the selected chart: dialog from chartInfo, type and wrapping as setChart', async () => {
    const { r, open, sent } = setup();
    const none: ChartState = { doc: {}, edit: { format: {} as FormatState } };
    expect(r.isEnabled('chart.edit', none)).toBe(false);
    expect(r.isEnabled('chart.type.line', none)).toBe(false);
    await r.execute('chart.edit', selected);
    expect(sent[0]).toEqual({ cmd: 'chartInfo' });
    expect(open).toHaveBeenCalledWith('edit', chartDraftOf(info));
    await r.execute('chart.type.line', selected);
    await r.execute('chart.wrap.tight', selected);
    expect(sent.slice(1)).toEqual([
      { cmd: 'setChart', kind: 'line' },
      { cmd: 'setChart', wrap: 'tight' },
    ]);
    expect(r.isChecked('chart.wrap.square', selected)).toBe(true);
    expect(r.isChecked('chart.type.column', selected)).toBe(true);
    for (const w of DOC_CHART_WRAPS) expect(r.has(`chart.wrap.${w}`)).toBe(true);
  });

  it('builds insertChart / setChart from the dialog, a template adding its titles', () => {
    const d = newChartDraft(t);
    expect(chartCommand('insert', d, t)).toEqual({
      cmd: 'insertChart',
      kind: 'column',
      title: '',
      legend: 'b',
      data: d.data,
    });
    const tpl = { name: 'Mine', kind: 'line' };
    expect(chartCommand('edit', { ...d, template: tpl }, t)).toMatchObject({
      cmd: 'setChart',
      template: tpl,
      texts: { title: expect.any(String) as string },
    });
  });

  it('loads the Chart Design gallery through the document command channel', async () => {
    const gallery = { styles: [], colors: [], layouts: [], template: { name: '', kind: 'column' } };
    const run = vi.fn(async () => ({ chartGallery: gallery }) as unknown as EditResult);
    expect(await docGalleryLoader(run)()).toEqual(gallery);
    expect(run).toHaveBeenCalledWith({ cmd: 'chartGallery', width: 100, height: 68 });
  });

  it('shows the Chart Design tab only for a selected chart', () => {
    const item = (id: string, label: string): RibbonItem => ({ id, label, onSelect: () => {} });
    const args = {
      t,
      item,
      icons: { edit: null, types: {}, wrap: null },
      load: async () => null,
      apply: () => {},
      onSaveTemplate: () => {},
    };
    expect(docChartTabs({ ...args, shown: false })).toEqual([]);
    const [tab] = docChartTabs({ ...args, shown: true });
    expect(tab?.context).toBe('chart');
    const ids = tab?.groups.flatMap((g) => g.items.map((i) => i.id)) ?? [];
    expect(ids).toEqual(
      expect.arrayContaining([
        'chart.edit',
        'chart.type.line',
        'chart.quickLayout',
        'chart.colors',
        'chart.styles',
        'chart.saveTemplate',
        'chart.wrap.inline',
        'chart.wrap.inFront',
      ]),
    );
  });
});

describe('shared ChartDialog', () => {
  it('edits two values and the type in the data grid', () => {
    const onApply = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <ChartDialog
          open
          mode="edit"
          initial={chartDraftOf(info)}
          onClose={() => {}}
          onApply={onApply}
        />
      </I18nextProvider>,
    );
    fireEvent.change(screen.getByTestId('chart-cell-0-0'), { target: { value: '7,5' } });
    fireEvent.change(screen.getByTestId('chart-cell-1-0'), { target: { value: '9' } });
    fireEvent.change(screen.getByTestId('chart-kind'), { target: { value: 'line' } });
    fireEvent.click(screen.getByTestId('chart-ok'));
    expect(onApply).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: 'line',
        data: { categories: ['Q1', 'Q2'], series: [{ name: 'North', values: [7.5, 9] }] },
      }),
    );
  });
});
