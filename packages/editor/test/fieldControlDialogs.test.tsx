// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { ControlInfo, EditCommand, EditResult, MergePrompt } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ControlDropDown, FieldCodeEditor } from '../src/ControlInPlace';
import { ControlBar } from '../src/DocDialogs';
import { ControlPropsDialog, FieldCodeDialog, FieldPromptDialog } from '../src/FieldControlDialogs';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const control = (patch: Partial<ControlInfo> = {}): ControlInfo => ({
  kind: 'text',
  title: null,
  tag: null,
  items: [],
  checked: null,
  date: null,
  format: null,
  placeholder: false,
  locked: false,
  deleteLocked: false,
  block: false,
  repeating: false,
  frame: [],
  ...patch,
});

const prompt = (key: string, text: string, def: string): MergePrompt => ({
  key,
  kind: key.startsWith('ASK') ? 'ask' : 'fillin',
  prompt: text,
  default: def,
  once: false,
});

describe('field dialogs', () => {
  it('asks the ASK / FILLIN questions one after the other', () => {
    const done = vi.fn();
    wrap(
      <FieldPromptDialog
        prompts={[
          prompt('ASK:Ten', 'Tên khách hàng?', 'Lan'),
          prompt('FILLIN:Ghi chú?', 'Ghi chú?', 'Không'),
        ]}
        onDone={done}
      />,
    );
    expect(screen.getByTestId('field-prompt-text').textContent).toBe('Tên khách hàng?');
    // The default answer is offered.
    expect((screen.getByTestId('field-prompt-input') as HTMLTextAreaElement).value).toBe('Lan');
    fireEvent.change(screen.getByTestId('field-prompt-input'), { target: { value: 'Minh' } });
    fireEvent.click(screen.getByTestId('field-prompt-ok'));
    expect(screen.getByTestId('field-prompt-text').textContent).toBe('Ghi chú?');
    expect((screen.getByTestId('field-prompt-input') as HTMLTextAreaElement).value).toBe('Không');
    // Cancel: this field keeps its default answer.
    fireEvent.click(screen.getByTestId('field-prompt-cancel'));
    expect(done).toHaveBeenCalledWith({ 'ASK:Ten': 'Minh' });
  });

  it('inserts a field from its typed code (Ctrl+F9)', () => {
    const insert = vi.fn();
    wrap(<FieldCodeDialog initial="" onInsert={insert} onClose={() => {}} />);
    const ok = screen.getByTestId('field-code-ok') as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    fireEvent.change(screen.getByTestId('field-code-input'), {
      target: { value: ' DATE \\@ "yyyy" ' },
    });
    fireEvent.keyDown(screen.getByTestId('field-code-input'), { key: 'Enter' });
    expect(insert).toHaveBeenCalledWith('DATE \\@ "yyyy"');
  });
});

describe('content control UI', () => {
  it('edits the title, tag and locks', () => {
    const apply = vi.fn();
    wrap(
      <ControlPropsDialog
        control={control({ title: 'Họ tên', deleteLocked: true })}
        onApply={apply}
        onClose={() => {}}
      />,
    );
    expect((screen.getByTestId('control-props-title') as HTMLInputElement).value).toBe('Họ tên');
    expect((screen.getByTestId('control-props-delete-locked') as HTMLInputElement).checked).toBe(
      true,
    );
    fireEvent.change(screen.getByTestId('control-props-tag'), { target: { value: 'ho_ten' } });
    fireEvent.click(screen.getByTestId('control-props-content-locked'));
    fireEvent.click(screen.getByTestId('control-props-ok'));
    expect(apply).toHaveBeenCalledWith({
      title: 'Họ tên',
      tag: 'ho_ten',
      deleteLocked: true,
      contentLocked: true,
      appearance: 'boundingBox',
      color: '',
      temporary: false,
    });
  });

  it('offers "+" in a repeating section, a picture button and the properties', () => {
    const commands: EditCommand[] = [];
    const run = vi.fn(async (c: EditCommand) => {
      commands.push(c);
      return null as EditResult | null;
    });
    const props = vi.fn();
    const pic = vi.fn();
    wrap(
      <ControlBar
        control={control({ kind: 'repeatingSection', block: true, repeating: true })}
        run={run}
        onProperties={props}
        onPicture={pic}
      />,
    );
    expect(screen.getByTestId('control-kind-label').textContent).toBe('Phần lặp lại');
    expect(screen.queryByTestId('control-picture')).toBeNull();
    fireEvent.click(screen.getByTestId('control-add-item'));
    expect(commands).toEqual([{ cmd: 'addRepeatingItem' }]);
    fireEvent.click(screen.getByTestId('control-properties'));
    expect(props).toHaveBeenCalled();
    cleanup();
    wrap(<ControlBar control={control({ kind: 'picture' })} run={run} onPicture={pic} />);
    expect(screen.queryByTestId('control-add-item')).toBeNull();
    fireEvent.click(screen.getByTestId('control-picture'));
    expect(pic).toHaveBeenCalled();
  });

  it('edits list items, how the control shows, its colour and "remove when edited"', () => {
    const apply = vi.fn();
    wrap(
      <ControlPropsDialog
        control={control({ kind: 'dropDown', items: ['Huế'], values: ['HUE'] })}
        onApply={apply}
        onClose={() => {}}
      />,
    );
    fireEvent.change(screen.getByTestId('control-props-item-display'), {
      target: { value: 'Hà Nội' },
    });
    fireEvent.click(screen.getByTestId('control-props-item-add'));
    // The new item is selected: up moves it first.
    fireEvent.click(screen.getByTestId('control-props-item-up'));
    // Modify the second one (Huế): its value.
    fireEvent.change(screen.getByTestId('control-props-items'), { target: { value: '1' } });
    expect((screen.getByTestId('control-props-item-value') as HTMLInputElement).value).toBe('HUE');
    fireEvent.change(screen.getByTestId('control-props-item-value'), { target: { value: 'H' } });
    fireEvent.click(screen.getByTestId('control-props-item-modify'));
    fireEvent.change(screen.getByTestId('control-props-appearance'), {
      target: { value: 'hidden' },
    });
    fireEvent.change(screen.getByTestId('control-props-color'), { target: { value: '#c00000' } });
    fireEvent.click(screen.getByTestId('control-props-temporary'));
    fireEvent.click(screen.getByTestId('control-props-ok'));
    expect(apply).toHaveBeenCalledWith(
      expect.objectContaining({
        appearance: 'hidden',
        color: 'C00000',
        temporary: true,
        items: [
          { displayText: 'Hà Nội', value: 'Hà Nội' },
          { displayText: 'Huế', value: 'H' },
        ],
      }),
    );
  });

  it('changes the language and format of a date control', () => {
    const apply = vi.fn();
    wrap(
      <ControlPropsDialog
        control={control({ kind: 'date', format: 'dd/MM/yyyy', lid: 'vi-VN' })}
        onApply={apply}
        onClose={() => {}}
      />,
    );
    // Another language offers its own formats, the first one chosen.
    fireEvent.change(screen.getByTestId('control-props-lid'), { target: { value: 'en-US' } });
    expect((screen.getByTestId('control-props-date-format') as HTMLInputElement).value).toBe(
      'M/d/yyyy',
    );
    fireEvent.click(screen.getByTestId('control-props-ok'));
    expect(apply).toHaveBeenCalledWith(
      expect.objectContaining({ dateLid: 'en-US', dateFormat: 'M/d/yyyy' }),
    );
  });

  it('opens the list on the control and picks with the keyboard', () => {
    const pick = vi.fn();
    const open = vi.fn();
    const toClient = () => ({ left: 10, top: 10, width: 50, height: 16 });
    const c = control({
      kind: 'dropDown',
      items: ['Hà Nội', 'Huế'],
      frame: [{ page: 0, x: 0, y: 0, w: 10, h: 10 }],
    });
    wrap(
      <ControlDropDown
        control={c}
        toClient={toClient}
        open
        onOpenChange={open}
        onPick={pick}
        blocks={[]}
        lang="vi-VN"
      />,
    );
    const list = screen.getByTestId('control-list');
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    fireEvent.keyDown(list, { key: 'Enter' });
    expect(pick).toHaveBeenCalledWith('Huế');
    fireEvent.keyDown(list, { key: 'Escape' });
    expect(open).toHaveBeenCalledWith(false);
    // A locked control has no arrow.
    cleanup();
    wrap(
      <ControlDropDown
        control={{ ...c, locked: true }}
        toClient={toClient}
        open={false}
        onOpenChange={open}
        onPick={pick}
        blocks={[]}
        lang="vi-VN"
      />,
    );
    expect(screen.queryByTestId('control-drop')).toBeNull();
  });

  it('picks a date in the calendar with the keyboard', () => {
    const pick = vi.fn();
    wrap(
      <ControlDropDown
        control={control({
          kind: 'date',
          date: '2026-11-12T00:00:00Z',
          frame: [{ page: 0, x: 0, y: 0, w: 10, h: 10 }],
        })}
        toClient={() => ({ left: 0, top: 0, width: 40, height: 16 })}
        open
        onOpenChange={() => {}}
        onPick={pick}
        blocks={[]}
        lang="vi-VN"
      />,
    );
    const grid = screen.getByRole('grid');
    // A week later, then a day back: 18 November; PageDown: a month later.
    fireEvent.keyDown(grid, { key: 'ArrowDown' });
    fireEvent.keyDown(grid, { key: 'ArrowLeft' });
    fireEvent.keyDown(grid, { key: 'Enter' });
    expect(pick).toHaveBeenLastCalledWith('2026-11-18');
    fireEvent.keyDown(grid, { key: 'PageDown' });
    fireEvent.keyDown(grid, { key: 'Enter' });
    expect(pick).toHaveBeenLastCalledWith('2026-12-18');
    // A click on a day.
    fireEvent.click(screen.getByText('25', { selector: 'td[data-date="2026-12-25"]' }));
    expect(pick).toHaveBeenLastCalledWith('2026-12-25');
  });

  it('types a field code in place: Enter keeps it, F9 updates', () => {
    const commit = vi.fn();
    wrap(
      <FieldCodeEditor
        rect={{ page: 0, x: 0, y: 0, w: 10, h: 12 }}
        initial=""
        toClient={() => ({ left: 0, top: 0, width: 10, height: 16 })}
        onCommit={commit}
      />,
    );
    const input = screen.getByTestId('field-code-input');
    fireEvent.change(input, { target: { value: '= 6*7' } });
    fireEvent.keyDown(input, { key: 'F9' });
    expect(commit).toHaveBeenCalledWith('= 6*7', true);
    // Committed once only (leaving afterwards does nothing).
    fireEvent.blur(input);
    expect(commit).toHaveBeenCalledTimes(1);
  });
});
