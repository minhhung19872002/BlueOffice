// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { ControlInfo, EditCommand, EditResult, MergePrompt } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
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
});
