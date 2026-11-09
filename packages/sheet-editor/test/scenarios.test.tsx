// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ScenarioList, SheetCommand, SheetHost } from '@blueoffice/platform';
import { ScenarioDialog, expandCells } from '../src/scenarios';

afterEach(cleanup);

const i18n = createI18n('en-US');

function fakeHost(list: ScenarioList) {
  const calls: SheetCommand[] = [];
  const host = {
    cell: vi.fn(async (_s: number, row: number, col: number) => ({
      address: '',
      // B1 = 10, B2 = 100, anything else empty.
      input: col === 1 && row === 0 ? '10' : col === 1 && row === 1 ? '100' : '',
      display: '',
    })),
    exec: vi.fn(async (cmd: SheetCommand) => {
      calls.push(cmd);
      switch (cmd.cmd) {
        case 'scenarios':
          return list;
        case 'scenarioSave':
          return { index: list.items.length };
        case 'scenarioMerge':
          return { merged: 2 };
        case 'scenarioSummary':
          return { sheet: 2 };
        default:
          return { ok: true };
      }
    }),
  } as unknown as SheetHost;
  return { host, calls };
}

const twoScenarios = (): ScenarioList => ({
  items: [
    {
      index: 0,
      name: 'Low',
      cells: ['B1', 'B2'],
      values: ['8', '50'],
      comment: 'bad year',
      user: 'An',
      locked: true,
      hidden: false,
    },
    {
      index: 1,
      name: 'High',
      cells: ['B1'],
      values: ['12'],
      comment: null,
      user: null,
      locked: false,
      hidden: false,
    },
  ],
  current: null,
  show: null,
  results: 'B3',
  protected: false,
});

function setup(list: ScenarioList, extra: Partial<Parameters<typeof ScenarioDialog>[0]> = {}) {
  const { host, calls } = fakeHost(list);
  const onDone = vi.fn();
  const onClose = vi.fn();
  const onReport = vi.fn();
  render(
    <I18nextProvider i18n={i18n}>
      <ScenarioDialog
        host={host}
        sheet={0}
        sheets={['Plan', 'Other']}
        selection="B1:B2"
        author="Mai"
        onClose={onClose}
        onDone={onDone}
        onReport={onReport}
        fail={(e) => {
          throw e;
        }}
        {...extra}
      />
    </I18nextProvider>,
  );
  return { calls, onDone, onClose, onReport };
}

describe('expandCells', () => {
  it('expands ranges and lists without duplicates', () => {
    expect(expandCells('$B$1:B2, b2; D4')).toEqual(['B1', 'B2', 'D4']);
    expect(expandCells('Plan!C3')).toEqual(['C3']);
    expect(expandCells('nope')).toBeNull();
    // 6 × 6 = 36 cells, over the 32-cell limit.
    expect(expandCells('A1:F6')).toBeNull();
  });
});

describe('ScenarioDialog', () => {
  it('lists the scenarios and applies the chosen one', async () => {
    const { calls, onDone } = setup(twoScenarios());
    expect(await screen.findByTestId('scenario-item-0')).toHaveProperty('textContent', 'Low');
    expect(screen.getByTestId('scenario-details').textContent).toContain('B1, B2');
    expect(screen.getByTestId('scenario-details').textContent).toContain('bad year');
    fireEvent.click(screen.getByTestId('scenario-item-1'));
    fireEvent.click(screen.getByTestId('scenario-apply'));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(calls).toContainEqual({ cmd: 'scenarioShow', sheet: 0, index: 1 });
    // The remembered result cells fill the report field.
    expect((screen.getByTestId('scenario-results') as HTMLInputElement).value).toBe('B3');
  });

  it('adds a scenario with the selection and its current values', async () => {
    const { calls } = setup({ ...twoScenarios(), items: [] });
    await screen.findByTestId('scenario-empty');
    fireEvent.click(screen.getByTestId('scenario-add'));
    const b2 = (await screen.findByTestId('scenario-value-B2')) as HTMLInputElement;
    await waitFor(() => expect(b2.value).toBe('100'));
    expect((screen.getByTestId('scenario-value-B1') as HTMLInputElement).value).toBe('10');
    // No name yet: refused in the dialog.
    fireEvent.click(screen.getByTestId('scenario-save'));
    expect(screen.getByRole('alert').textContent).not.toBe('');
    fireEvent.change(screen.getByTestId('scenario-name'), { target: { value: ' Best ' } });
    fireEvent.change(b2, { target: { value: '250' } });
    fireEvent.change(screen.getByTestId('scenario-comment'), { target: { value: 'hope' } });
    fireEvent.click(screen.getByTestId('scenario-hidden'));
    fireEvent.click(screen.getByTestId('scenario-save'));
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: 'scenarioSave',
        sheet: 0,
        index: null,
        name: 'Best',
        cells: 'B1,B2',
        values: ['10', '250'],
        comment: 'hope',
        user: 'Mai',
        locked: true,
        hidden: true,
      }),
    );
    await screen.findByTestId('scenario-dialog');
  });

  it('edits keeping stored values and refuses a clashing name', async () => {
    const { calls } = setup(twoScenarios());
    await screen.findByTestId('scenario-item-0');
    fireEvent.click(screen.getByTestId('scenario-edit-btn'));
    expect((screen.getByTestId('scenario-value-B2') as HTMLInputElement).value).toBe('50');
    fireEvent.change(screen.getByTestId('scenario-name'), { target: { value: 'high' } });
    fireEvent.click(screen.getByTestId('scenario-save'));
    expect(screen.getByRole('alert')).toBeTruthy();
    expect(calls.some((c) => c.cmd === 'scenarioSave')).toBe(false);
    fireEvent.change(screen.getByTestId('scenario-name'), { target: { value: 'Low 2' } });
    fireEvent.click(screen.getByTestId('scenario-save'));
    await waitFor(() =>
      expect(calls).toContainEqual(
        expect.objectContaining({
          cmd: 'scenarioSave',
          index: 0,
          name: 'Low 2',
          values: ['8', '50'],
        }),
      ),
    );
  });

  it('merges from another sheet and builds the report', async () => {
    const { calls, onReport, onClose } = setup(twoScenarios());
    await screen.findByTestId('scenario-item-0');
    fireEvent.change(screen.getByTestId('scenario-from'), { target: { value: '1' } });
    fireEvent.click(screen.getByTestId('scenario-merge'));
    await waitFor(() => expect(calls).toContainEqual({ cmd: 'scenarioMerge', sheet: 0, from: 1 }));
    fireEvent.change(screen.getByTestId('scenario-results'), { target: { value: 'B3, B4' } });
    fireEvent.click(screen.getByTestId('scenario-report'));
    await waitFor(() => expect(onReport).toHaveBeenCalledWith(2));
    expect(onClose).toHaveBeenCalled();
    const summary = calls.find((c) => c.cmd === 'scenarioSummary');
    expect(summary).toMatchObject({ sheet: 0, results: 'B3, B4' });
    expect(summary && 'labels' in summary && summary.labels?.title).toContain('{sheet}');
  });

  it('locks editing while the scenarios are protected', async () => {
    setup({ ...twoScenarios(), protected: true });
    await screen.findByTestId('scenario-item-0');
    expect(screen.getByTestId('scenario-add')).toHaveProperty('disabled', true);
    // "Low" is locked; "High" is not.
    expect(screen.getByTestId('scenario-edit-btn')).toHaveProperty('disabled', true);
    expect(screen.getByTestId('scenario-delete')).toHaveProperty('disabled', true);
    fireEvent.click(screen.getByTestId('scenario-item-1'));
    expect(screen.getByTestId('scenario-delete')).toHaveProperty('disabled', false);
    expect(screen.getByTestId('scenario-apply')).toHaveProperty('disabled', false);
  });
});
