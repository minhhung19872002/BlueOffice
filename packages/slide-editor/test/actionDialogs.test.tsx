// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  SlideActionInfo,
  SlideActionOut,
  SlideCustomShow,
  SlideLinkAction,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ActionButtonsDialog,
  ActionDialog,
  LinkDialog,
  ShowLinkPrompt,
} from '../src/actionDialogs';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const wrap = (ui: ReactNode) => <I18nextProvider i18n={i18n}>{ui}</I18nextProvider>;

const out = (target: SlideLinkAction, more: Partial<SlideActionOut> = {}): SlideActionOut => ({
  target,
  highlight: false,
  safety: 'internal',
  locked: false,
  ...more,
});

const info = (more: Partial<SlideActionInfo> = {}): SlideActionInfo => ({
  scope: 'shape',
  click: null,
  hover: null,
  text: '',
  slides: ['Mở đầu', '', 'Kết quả\nquý IV'],
  ...more,
});

const shows: SlideCustomShow[] = [
  { id: 0, name: 'Ngắn', slides: [0, 2] },
  { id: 4, name: 'Phụ lục', slides: [2] },
];

const type = (testId: string, value: string) =>
  fireEvent.change(screen.getByTestId(testId), { target: { value } });

describe('Insert › Link dialog', () => {
  const open = (i: SlideActionInfo, onApply = vi.fn(), onRemove = vi.fn()) => {
    const onClose = vi.fn();
    render(
      wrap(
        <LinkDialog
          info={i}
          count={3}
          customShows={shows}
          thumb={(s, w) => <span data-testid={`thumb-pic-${s}`}>{w}</span>}
          onClose={onClose}
          onApply={onApply}
          onRemove={onRemove}
        />,
      ),
    );
    return { onApply, onRemove, onClose };
  };

  it('links to a web address, adding https:// and refusing other schemes', () => {
    const { onApply } = open(info());
    const ok = screen.getByTestId('link-ok') as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    expect(screen.queryByTestId('link-remove')).toBeNull();
    type('link-url', 'javascript:alert(1)');
    expect(ok.disabled).toBe(true);
    type('link-url', 'file:///C:/Windows/System32/calc.exe');
    expect(ok.disabled).toBe(true);
    type('link-url', 'example.vn/tai-lieu');
    type('link-tooltip', ' Tài liệu ');
    expect(ok.disabled).toBe(false);
    fireEvent.click(ok);
    expect(onApply).toHaveBeenCalledWith({
      target: { kind: 'url', url: 'https://example.vn/tai-lieu' },
      tooltip: 'Tài liệu',
      highlight: false,
      sound: { kind: 'keep' },
    });
  });

  it('picks a slide of this deck from thumbnails', () => {
    const { onApply } = open(info({ scope: 'text', text: 'Xem trang 3' }));
    expect(screen.getByTestId('link-scope').textContent).toContain('Xem trang 3');
    fireEvent.click(screen.getByTestId('link-kind-place'));
    // Thumbnails and titles of every slide; relative places next to them.
    expect(screen.getByTestId('thumb-pic-2').textContent).toBe('112');
    expect(screen.getByTestId('link-slide-1').textContent).toContain('1. Mở đầu');
    expect(screen.getByTestId('link-slide-3').textContent).toContain('3. Kết quả');
    expect(screen.getByTestId('link-place-next').getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByTestId('link-slide-3'));
    expect(screen.getByTestId('link-slide-3').getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByTestId('link-place-next').getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(screen.getByTestId('link-ok'));
    expect(onApply).toHaveBeenCalledWith({
      target: { kind: 'slide', slide: 2 },
      highlight: false,
      sound: { kind: 'keep' },
    });
    fireEvent.click(screen.getByTestId('link-place-last'));
    fireEvent.click(screen.getByTestId('link-ok'));
    expect(onApply.mock.calls[1]![0].target).toEqual({ kind: 'last' });
  });

  it('links to a custom show, a mail address and a file', () => {
    const { onApply } = open(info());
    fireEvent.click(screen.getByTestId('link-kind-show'));
    type('link-show', '4');
    fireEvent.click(screen.getByTestId('link-show-return'));
    fireEvent.click(screen.getByTestId('link-ok'));
    expect(onApply.mock.calls[0]![0].target).toEqual({ kind: 'customShow', id: 4, return: false });

    fireEvent.click(screen.getByTestId('link-kind-mail'));
    const ok = screen.getByTestId('link-ok') as HTMLButtonElement;
    type('link-mail', 'không phải thư');
    expect(ok.disabled).toBe(true);
    type('link-mail', 'lienhe@example.vn');
    type('link-subject', 'Hỏi về BlueSlide');
    fireEvent.click(ok);
    expect(onApply.mock.calls[1]![0].target).toEqual({
      kind: 'url',
      url: 'mailto:lienhe@example.vn?subject=H%E1%BB%8Fi%20v%E1%BB%81%20BlueSlide',
    });

    fireEvent.click(screen.getByTestId('link-kind-file'));
    type('link-file', 'bao-cao.docx');
    fireEvent.click(ok);
    expect(onApply.mock.calls[2]![0].target).toEqual({ kind: 'file', path: 'bao-cao.docx' });
  });

  it('shows the current link, keeps its highlight and removes it', () => {
    const current = out(
      { kind: 'url', url: 'mailto:a@example.vn?subject=Ch%C3%A0o' },
      { tooltip: 'Thư', highlight: true, safety: 'external' },
    );
    const { onApply, onRemove } = open(info({ click: current }));
    expect(screen.getByTestId('link-kind-mail').getAttribute('aria-selected')).toBe('true');
    expect((screen.getByTestId('link-mail') as HTMLInputElement).value).toBe('a@example.vn');
    expect((screen.getByTestId('link-subject') as HTMLInputElement).value).toBe('Chào');
    expect((screen.getByTestId('link-tooltip') as HTMLInputElement).value).toBe('Thư');
    fireEvent.click(screen.getByTestId('link-ok'));
    expect(onApply.mock.calls[0]![0]).toMatchObject({ tooltip: 'Thư', highlight: true });
    fireEvent.click(screen.getByTestId('link-remove'));
    expect(onRemove).toHaveBeenCalledTimes(1);
  });

  it('cannot link a media clip or nothing at all, and shows a refusal', () => {
    open(info({ click: out({ kind: 'unknown', action: 'ppaction://media' }, { locked: true }) }));
    type('link-url', 'https://example.vn');
    expect((screen.getByTestId('link-ok') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByTestId('link-remove')).toBeNull();
    cleanup();
    render(
      wrap(
        <LinkDialog
          info={info({ scope: 'none' })}
          count={3}
          customShows={[]}
          error="badLink"
          onClose={() => undefined}
          onApply={() => undefined}
          onRemove={() => undefined}
        />,
      ),
    );
    type('link-url', 'https://example.vn');
    expect((screen.getByTestId('link-ok') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('link-error').textContent).toBe(
      'Địa chỉ hoặc đường dẫn không hợp lệ.',
    );
    fireEvent.click(screen.getByTestId('link-kind-show'));
    expect(screen.queryByTestId('link-show')).toBeNull();
  });
});

describe('Insert › Action dialog', () => {
  const open = (i: SlideActionInfo, extra: { pickSound?: () => Promise<never> } = {}) => {
    const onApply = vi.fn();
    const onClose = vi.fn();
    const preview = vi.fn();
    render(
      wrap(
        <ActionDialog
          info={i}
          count={3}
          customShows={shows}
          preview={preview}
          onClose={onClose}
          onApply={onApply}
          {...extra}
        />,
      ),
    );
    return { onApply, onClose, preview };
  };

  it('sets a click jump and a hover sound with a highlight, each on its tab', () => {
    const { onApply, preview } = open(info());
    expect(screen.getByTestId('action-tab-click').getAttribute('aria-selected')).toBe('true');
    expect((screen.getByTestId('action-none') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('action-to') as HTMLSelectElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('action-link'));
    type('action-to', 'lastViewed');

    fireEvent.click(screen.getByTestId('action-tab-hover'));
    expect(screen.getByTestId('action-panel-hover')).toBeTruthy();
    expect((screen.getByTestId('action-none') as HTMLInputElement).checked).toBe(true);
    type('action-sound', 'chime');
    fireEvent.click(screen.getByTestId('action-sound-preview'));
    expect(preview).toHaveBeenCalledTimes(1);
    expect(String.fromCharCode(...(preview.mock.calls[0]![0] as Uint8Array).subarray(0, 4))).toBe(
      'RIFF',
    );
    fireEvent.click(screen.getByTestId('action-highlight'));

    fireEvent.click(screen.getByTestId('action-ok'));
    expect(onApply).toHaveBeenCalledTimes(1);
    const changes = onApply.mock.calls[0]![0];
    expect(changes).toHaveLength(2);
    expect(changes[0]).toEqual({
      on: 'click',
      spec: { target: { kind: 'lastViewed' }, highlight: false, sound: { kind: 'none' } },
    });
    expect(changes[1].on).toBe('hover');
    expect(changes[1].spec).toMatchObject({
      target: { kind: 'none' },
      highlight: true,
      sound: { kind: 'file', name: 'chime.wav' },
    });
  });

  it('links to a slide, a custom show and a web address', () => {
    const { onApply } = open(info());
    fireEvent.click(screen.getByTestId('action-link'));
    type('action-to', 'slide');
    expect(screen.getByTestId('action-slide').textContent).toContain('3. Kết quả');
    type('action-slide', '2');
    fireEvent.click(screen.getByTestId('action-ok'));
    expect(onApply.mock.calls[0]![0][0].spec.target).toEqual({ kind: 'slide', slide: 2 });

    type('action-to', 'customShow');
    type('action-show', '4');
    fireEvent.click(screen.getByTestId('action-ok'));
    expect(onApply.mock.calls[1]![0][0].spec.target).toEqual({
      kind: 'customShow',
      id: 4,
      return: true,
    });

    type('action-to', 'url');
    const ok = screen.getByTestId('action-ok') as HTMLButtonElement;
    expect(ok.disabled).toBe(true);
    type('action-url', 'vbscript:msgbox(1)');
    expect(ok.disabled).toBe(true);
    type('action-url', 'https://example.vn');
    expect(ok.disabled).toBe(false);
    fireEvent.click(ok);
    expect(onApply.mock.calls[2]![0][0].spec.target).toEqual({
      kind: 'url',
      url: 'https://example.vn',
    });
  });

  it('shows the current actions and changes nothing when nothing was touched', () => {
    const { onApply, onClose } = open(
      info({
        click: out({ kind: 'end' }, { highlight: true }),
        hover: out(
          { kind: 'none' },
          { sound: { stop: false, part: '/ppt/media/a.wav', name: 'chuông.wav', loop: false } },
        ),
      }),
    );
    expect((screen.getByTestId('action-link') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('action-to') as HTMLSelectElement).value).toBe('end');
    expect((screen.getByTestId('action-highlight') as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByTestId('action-tab-hover'));
    const sound = screen.getByTestId('action-sound') as HTMLSelectElement;
    expect(sound.value).toBe('keep');
    expect(sound.selectedOptions[0]!.textContent).toBe('chuông.wav');
    fireEvent.click(screen.getByTestId('action-ok'));
    expect(onApply).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('keeps a program action unless the user replaces it, and never offers to run it', () => {
    const program = out(
      { kind: 'program', path: 'file:///C:/Windows/System32/calc.exe' },
      { safety: 'blocked' },
    );
    const { onApply } = open(info({ click: program }));
    expect(screen.getByTestId('action-kept').textContent).toContain('chạy chương trình');
    expect(screen.getByTestId('action-kept').textContent).toContain('không bao giờ chạy');
    expect((screen.getByTestId('action-none') as HTMLInputElement).checked).toBe(false);
    expect((screen.getByTestId('action-link') as HTMLInputElement).checked).toBe(false);
    // No choice of the dialog stands for a program or a macro.
    const options = [...(screen.getByTestId('action-to') as HTMLSelectElement).options].map(
      (o) => o.value,
    );
    expect(options).toEqual([
      'next',
      'prev',
      'first',
      'last',
      'lastViewed',
      'end',
      'slide',
      'customShow',
      'url',
      'file',
    ]);
    // A highlight alone does not touch the kept action.
    fireEvent.click(screen.getByTestId('action-highlight'));
    fireEvent.click(screen.getByTestId('action-ok'));
    expect(onApply).not.toHaveBeenCalled();
    cleanup();
    const again = open(info({ click: program }));
    fireEvent.click(screen.getByTestId('action-none'));
    fireEvent.click(screen.getByTestId('action-ok'));
    expect(again.onApply.mock.calls[0]![0]).toEqual([
      {
        on: 'click',
        spec: { target: { kind: 'none' }, highlight: false, sound: { kind: 'none' } },
      },
    ]);
  });
});

describe('action buttons and the prompts of the show', () => {
  it('lists the twelve action buttons', () => {
    const onPick = vi.fn();
    render(wrap(<ActionButtonsDialog onClose={() => undefined} onPick={onPick} />));
    expect(screen.getAllByTestId(/^action-button-actionButton/)).toHaveLength(12);
    fireEvent.click(screen.getByTestId('action-button-actionButtonEnd'));
    expect(onPick).toHaveBeenCalledWith('actionButtonEnd');
    expect(screen.getByTestId('action-button-actionButtonHome').getAttribute('title')).toBe(
      'Trang chủ',
    );
  });

  it('asks before an address leaves the app and does not let the click reach the show', () => {
    const onShowClick = vi.fn();
    const onOpen = vi.fn();
    const onCancel = vi.fn();
    render(
      wrap(
        <div onClick={onShowClick}>
          <ShowLinkPrompt
            ask={{ kind: 'external', url: 'https://example.vn/a', mail: false }}
            notice={null}
            onOpen={onOpen}
            onCancel={onCancel}
            onDismiss={() => undefined}
          />
        </div>,
      ),
    );
    expect(screen.getByTestId('show-link-url').textContent).toBe('https://example.vn/a');
    fireEvent.click(screen.getByTestId('show-link-ask'));
    fireEvent.click(screen.getByTestId('show-link-cancel'));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onOpen).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('show-link-open'));
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onShowClick).not.toHaveBeenCalled();
    // Esc on the prompt answers "no"; other keys do nothing.
    fireEvent.keyDown(screen.getByTestId('show-link-open'), { key: 'Enter' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByTestId('show-link-open'), { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(2);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it('tells that a program was not run — with no way to run it', () => {
    const onDismiss = vi.fn();
    render(
      wrap(
        <ShowLinkPrompt
          ask={null}
          notice={{ reason: 'program', detail: 'file:///C:/Windows/System32/calc.exe' }}
          onOpen={() => undefined}
          onCancel={() => undefined}
          onDismiss={onDismiss}
        />,
      ),
    );
    const box = screen.getByTestId('show-link-blocked');
    expect(box.getAttribute('data-reason')).toBe('program');
    expect(box.textContent).toContain('BlueSlide không chạy chương trình');
    expect(box.textContent).toContain('calc.exe');
    expect(screen.queryByTestId('show-link-open')).toBeNull();
    expect(box.querySelectorAll('button')).toHaveLength(1);
    expect(box.querySelector('a')).toBeNull();
    fireEvent.click(screen.getByTestId('show-link-dismiss'));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it('renders nothing when there is nothing to ask or tell', () => {
    const { container } = render(
      wrap(
        <ShowLinkPrompt
          ask={null}
          notice={null}
          onOpen={() => undefined}
          onCancel={() => undefined}
          onDismiss={() => undefined}
        />,
      ),
    );
    expect(container.innerHTML).toBe('');
  });
});
