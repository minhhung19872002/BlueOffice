// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { ModelInfo, PivotInfo, SheetCommand, SheetHost } from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  checkText,
  MeasuresDialog,
  ModelTablesDialog,
  RelationshipsDialog,
  suggest,
  tableRef,
} from '../src/dataModel';
import { modelGroups } from '../src/pivot';

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = i18n.t.bind(i18n) as (k: string, o?: Record<string, unknown>) => string;
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const info: ModelInfo = {
  tables: [
    {
      name: 'Products',
      inModel: true,
      sheet: 0,
      sheetName: 'Sheet1',
      rows: 5,
      columns: [
        { name: 'Id', calculated: false },
        { name: 'Category', calculated: false },
      ],
    },
    {
      name: 'Sales',
      inModel: true,
      sheet: 0,
      sheetName: 'Sheet1',
      rows: 8,
      query: 'Sales query',
      columns: [
        { name: 'ProductId', calculated: false },
        { name: 'Qty', calculated: false },
        { name: 'Amount', calculated: true, expression: 'Sales[Qty] * 2' },
      ],
    },
    { name: 'Bảng giá', inModel: false, columns: [{ name: 'Giá', calculated: false }] },
  ],
  relationships: [
    {
      index: 0,
      fromTable: 'Sales',
      fromColumn: 'ProductId',
      toTable: 'Products',
      toColumn: 'Id',
      active: true,
    },
  ],
  measures: [{ name: 'Total Qty', table: 'Sales', expression: 'SUM(Sales[Qty])' }],
  columns: [{ table: 'Sales', name: 'Amount', expression: 'Sales[Qty] * 2' }],
  functions: [
    { name: 'SUM', min: 1, max: 1 },
    { name: 'SUMX', min: 2, max: 2 },
    { name: 'DIVIDE', min: 2, max: 3 },
  ],
  pivots: 0,
  foreign: [{ name: '/xl/model/item.data', kind: 'model', size: 4096 }],
};

describe('formula suggestions', () => {
  it('suggests columns, measures, functions and tables', () => {
    const labels = (text: string, caret = text.length, table?: string) =>
      suggest(info, text, caret, table).items.map((s) => s.label);
    expect(labels('SUM(Sales[')).toEqual(['ProductId', 'Qty', 'Amount']);
    expect(labels('SUM(Sales[Q')).toEqual(['Qty']);
    expect(suggest(info, 'SUM(Sales[Q', 11).from).toBe(10);
    expect(labels('DIVIDE([T')).toEqual(['Total Qty']);
    // In a calculated column, [ also offers the columns of its table.
    expect(labels('[', 1, 'Products')).toEqual(['Total Qty', 'Id', 'Category']);
    expect(labels('su')).toEqual(['SUM', 'SUMX']);
    expect(labels('COUNTROWS(Sa')).toEqual(['Sales']);
    // Tables outside the model are not offered.
    expect(labels('B')).toEqual([]);
    expect(labels('1 + ')).toEqual([]);
    expect(suggest(info, 'su', 2).items[0]?.insert).toBe('SUM(');
    expect(tableRef('Sales')).toBe('Sales');
    expect(tableRef("Bảng giá'x")).toBe("'Bảng giá''x'");
  });

  it('describes check results', () => {
    expect(checkText(null, t)).toBe('');
    expect(checkText({ ok: true }, t)).toBe('No errors.');
    expect(checkText({ ok: false, error: { message: 'missing )', start: 4, end: 5 } }, t)).toBe(
      'Error at character 5: missing )',
    );
    expect(
      checkText({ ok: false, error: { message: 'unknown table X', start: 0, end: 0 } }, t),
    ).toBe('Error: unknown table X');
  });
});

describe('pivot field list over the model', () => {
  it('groups fields by table, measures under their home table', () => {
    const field = (name: string, table: string | null, measure = false) => ({
      name,
      table,
      column: name.replace(/^.*\[|\]$/g, ''),
      measure,
      items: [],
      formula: null,
      derived: false,
      date: false,
      group: null,
    });
    const p = {
      model: true,
      fields: [
        field('Products[Id]', 'Products'),
        field('Sales[Qty]', 'Sales'),
        field('[Total Qty]', 'Sales', true),
        field('Products[Category]', 'Products'),
      ],
    } as unknown as PivotInfo;
    expect(modelGroups(p)).toEqual([
      { table: 'Products', fields: [0, 3] },
      { table: 'Sales', fields: [1, 2] },
    ]);
    expect(modelGroups({ ...p, model: false })).toEqual([{ table: null, fields: [0, 1, 2, 3] }]);
  });
});

function fakeHost(answers: Partial<Record<string, unknown>>) {
  const calls: SheetCommand[] = [];
  const exec = vi.fn(async (cmd: SheetCommand) => {
    calls.push(cmd);
    const a = answers[(cmd as { cmd: string }).cmd];
    return typeof a === 'function' ? (a as (c: SheetCommand) => unknown)(cmd) : a;
  });
  return { host: { exec } as unknown as SheetHost, calls };
}

describe('data model dialogs', () => {
  it('lists the tables and the parts of other software', () => {
    const edit = vi.fn(async () => undefined);
    wrap(
      <ModelTablesDialog
        info={info}
        edit={edit}
        onClose={() => undefined}
        fail={() => undefined}
      />,
    );
    expect(screen.getByText(/Query: Sales query/)).toBeTruthy();
    expect(screen.getByTestId('model-foreign').textContent).toContain('4096 bytes');
    fireEvent.click(screen.getByTestId('model-table-Bảng giá'));
    expect(edit).toHaveBeenCalledWith({ cmd: 'modelAddTable', name: 'Bảng giá' });
    fireEvent.click(screen.getByTestId('model-table-Sales'));
    expect(edit).toHaveBeenCalledWith({ cmd: 'modelRemoveTable', name: 'Sales' });
  });

  it('adds a relationship with the detected cardinality', async () => {
    const { host, calls } = fakeHost({
      modelDetect: (c: SheetCommand) =>
        (c as { fromColumn: string }).fromColumn === 'Category'
          ? { cardinality: 'manyToMany', unmatched: 0 }
          : { cardinality: 'oneToMany', unmatched: 2 },
    });
    const edit = vi.fn(async () => undefined);
    wrap(
      <RelationshipsDialog
        host={host}
        info={info}
        edit={edit}
        onClose={() => undefined}
        fail={() => undefined}
      />,
    );
    expect(screen.getByTestId('model-rel-0').textContent).toContain(
      'Sales[ProductId] (many) → Products[Id] (one)',
    );
    fireEvent.click(screen.getByTestId('model-rel-add'));
    // Products[Id] → Sales[ProductId]: one to many, swapped on save.
    fireEvent.change(screen.getByTestId('model-rel-to-table'), { target: { value: 'Sales' } });
    await waitFor(() =>
      expect(screen.getByTestId('model-rel-cardinality').textContent).toContain('One to many'),
    );
    expect(screen.getByTestId('model-rel-cardinality').textContent).toContain('2 values');
    await act(async () => {
      fireEvent.click(screen.getByTestId('model-rel-save'));
    });
    expect(edit).toHaveBeenCalledWith({
      cmd: 'modelSetRelationship',
      fromTable: 'Products',
      fromColumn: 'Id',
      toTable: 'Sales',
      toColumn: 'ProductId',
      active: true,
    });
    // Many to many cannot be saved.
    fireEvent.click(screen.getByTestId('model-rel-add'));
    fireEvent.change(screen.getByTestId('model-rel-from-column'), {
      target: { value: 'Category' },
    });
    await waitFor(() =>
      expect(screen.getByTestId('model-rel-cardinality').textContent).toContain('Many to many'),
    );
    expect((screen.getByTestId('model-rel-save') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('model-rel-delete-0'));
    expect(edit).toHaveBeenCalledWith({ cmd: 'modelDeleteRelationship', index: 0 });
    expect(calls.every((c) => (c as { cmd: string }).cmd === 'modelDetect')).toBe(true);
  });

  it('checks a measure while typing and inserts suggestions', async () => {
    const { host, calls } = fakeHost({
      modelCheck: (c: SheetCommand) =>
        (c as { expression: string }).expression.endsWith(')')
          ? { ok: true }
          : { ok: false, error: { message: 'missing )', start: 13, end: 13 + 1 } },
    });
    const edit = vi.fn(async () => undefined);
    wrap(<MeasuresDialog host={host} info={info} edit={edit} onClose={() => undefined} />);
    expect(screen.getByTestId('model-measure-list').textContent).toContain('[Total Qty]');
    fireEvent.click(screen.getByTestId('model-measure-new'));
    fireEvent.change(screen.getByTestId('model-measure-name'), { target: { value: 'Avg' } });
    fireEvent.change(screen.getByTestId('model-measure-table'), { target: { value: 'Sales' } });
    const box = screen.getByTestId('model-measure-expression') as HTMLTextAreaElement;
    fireEvent.change(box, { target: { value: 'AVERAGE(Sales[', selectionStart: 14 } });
    // Columns of Sales are suggested; picking one completes the reference.
    fireEvent.click(await screen.findByTestId('model-suggest-Qty'));
    expect(box.value).toBe('AVERAGE(Sales[Qty]');
    await waitFor(() =>
      expect(screen.getByTestId('model-measure-check').textContent).toBe(
        'Error at character 14: missing )',
      ),
    );
    expect((screen.getByTestId('model-measure-save') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(box, { target: { value: 'AVERAGE(Sales[Qty])' } });
    await waitFor(() =>
      expect(screen.getByTestId('model-measure-check').textContent).toBe('No errors.'),
    );
    fireEvent.change(screen.getByTestId('model-measure-format'), { target: { value: '0.0' } });
    await act(async () => {
      fireEvent.click(screen.getByTestId('model-measure-save'));
    });
    expect(edit).toHaveBeenCalledWith({
      cmd: 'modelSetMeasure',
      name: 'Avg',
      table: 'Sales',
      expression: 'AVERAGE(Sales[Qty])',
      format: '0.0',
    });
    expect(calls.some((c) => (c as { cmd: string; name?: string }).name === 'Avg')).toBe(true);
  });

  it('saves a calculated column', async () => {
    const { host } = fakeHost({ modelCheck: { ok: true } });
    const edit = vi.fn(async () => undefined);
    wrap(<MeasuresDialog host={host} info={info} edit={edit} onClose={() => undefined} />);
    fireEvent.click(screen.getByTestId('model-measure-new'));
    fireEvent.change(screen.getByTestId('model-measure-kind'), { target: { value: 'column' } });
    fireEvent.change(screen.getByTestId('model-measure-table'), { target: { value: 'Sales' } });
    fireEvent.change(screen.getByTestId('model-measure-name'), { target: { value: 'Category' } });
    fireEvent.change(screen.getByTestId('model-measure-expression'), {
      target: { value: 'RELATED(Products[Category])' },
    });
    await waitFor(() =>
      expect(screen.getByTestId('model-measure-check').textContent).toBe('No errors.'),
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId('model-measure-save'));
    });
    expect(edit).toHaveBeenCalledWith({
      cmd: 'modelSetColumn',
      table: 'Sales',
      name: 'Category',
      expression: 'RELATED(Products[Category])',
    });
  });
});
