// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { CfRuleInfo, SheetCommand, SheetHost } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CfManagerDialog, defaultIconOptions, iconCount, IconSetEditor } from '../src/cfRules';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const rules: CfRuleInfo[] = [
  {
    block: 1,
    rule: 0,
    priority: 1,
    kind: 'cellIs',
    range: 'A1:A10',
    stopIfTrue: false,
    pivot: false,
    ext: false,
    spec: {
      kind: 'cellIs',
      operator: 'greaterThan',
      formula1: '5',
      style: { fill: 'FF0000' },
    },
  },
  {
    block: 0,
    rule: 0,
    priority: 2,
    kind: 'dataBar',
    range: 'A1:A10',
    stopIfTrue: false,
    pivot: true,
    ext: false,
    spec: { kind: 'dataBar', color: '4A7BB7' },
  },
];

describe('conditional formatting editors', () => {
  it('counts icons and splits equal percent bands', () => {
    expect(iconCount('5Boxes')).toBe(5);
    expect(iconCount('3Stars')).toBe(3);
    expect(defaultIconOptions(4).thresholds.map((t) => t.value)).toEqual(['25', '50', '75']);
  });

  it('switches to custom icons, one per band', () => {
    const onChange = vi.fn();
    wrap(
      <IconSetEditor
        set="3Flags"
        reverse={false}
        options={defaultIconOptions(3)}
        onChange={onChange}
      />,
    );
    fireEvent.click(screen.getByTestId('cf-icon-custom'));
    const [, , opts] = onChange.mock.calls[0] as [string, boolean, { icons: unknown[] }];
    expect(opts.icons).toEqual([
      { set: '3Flags', id: 0 },
      { set: '3Flags', id: 1 },
      { set: '3Flags', id: 2 },
    ]);
  });

  it('lists rules in priority order and sends manager operations', async () => {
    const sent: SheetCommand[] = [];
    const host = {
      exec: vi.fn(async (cmd: SheetCommand) => {
        sent.push(cmd);
        return cmd.cmd === 'listCf' ? { rules } : { ok: true };
      }),
    } as unknown as SheetHost;
    wrap(
      <CfManagerDialog host={host} sheet={0} onClose={vi.fn()} onDone={vi.fn()} fail={vi.fn()} />,
    );
    await waitFor(() => expect(screen.getByTestId('cf-rule-1')).toBeTruthy());
    fireEvent.click(screen.getByTestId('cf-stop-0'));
    await waitFor(() =>
      expect(sent).toContainEqual({
        cmd: 'editCf',
        sheet: 0,
        block: 1,
        rule: 0,
        op: { op: 'stopIfTrue', value: true },
      }),
    );
    fireEvent.click(screen.getByTestId('cf-rule-1'));
    fireEvent.click(screen.getByTestId('cf-move-up'));
    await waitFor(() =>
      expect(sent).toContainEqual({
        cmd: 'editCf',
        sheet: 0,
        block: 0,
        rule: 0,
        op: { op: 'moveUp' },
      }),
    );
  });
});
