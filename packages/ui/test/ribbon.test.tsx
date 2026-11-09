// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Popover, Ribbon } from '../src';

afterEach(cleanup);

describe('Ribbon', () => {
  it("shows the icon of each command id, else the item's own (UI-ICONS-002)", () => {
    const noop = () => {};
    const { container } = render(
      <Ribbon
        tabs={[
          {
            id: 'home',
            label: 'Trang đầu',
            groups: [
              {
                id: 'g',
                label: 'Nhóm',
                items: [
                  { id: 'file.save', label: 'Lưu', onSelect: noop },
                  { id: 'format.bold', label: 'Đậm', size: 'small', onSelect: noop },
                  {
                    id: 'x.own',
                    label: 'Riêng',
                    icon: <i data-own />,
                    size: 'small',
                    onSelect: noop,
                  },
                  { id: 'format.italic', label: 'Nghiêng', icon: <i data-old />, onSelect: noop },
                  { id: 'x.unknown', label: 'Không icon', onSelect: noop },
                ],
              },
            ],
          },
        ]}
      />,
    );
    const btn = (id: string) => container.querySelector(`button[data-command="${id}"]`)!;
    // Large button: 24px set icon, visible label.
    const save = btn('file.save').querySelector('svg.bo-icon')!;
    expect(save.getAttribute('data-icon')).toBe('file.save');
    expect(save.getAttribute('width')).toBe('24');
    // Small button: 20px set icon and its label stays visible.
    const bold = btn('format.bold');
    expect(bold.querySelector('svg.bo-icon')!.getAttribute('width')).toBe('20');
    expect(bold.querySelector('span:last-child')!.className).not.toMatch(/srOnly/);
    // The set's icon replaces an item's own icon on the same id.
    expect(btn('format.italic').querySelector('[data-old]')).toBeNull();
    expect(btn('format.italic').querySelector('svg.bo-icon')).not.toBeNull();
    // Ids outside the set keep their own icon; a small button with one shows only the icon.
    const own = btn('x.own');
    expect(own.querySelector('[data-own]')).not.toBeNull();
    expect(own.querySelector('svg.bo-icon')).toBeNull();
    expect(own.querySelector('span:last-child')!.className).toMatch(/srOnly/);
    // No icon in the set: label only.
    expect(btn('x.unknown').querySelector('svg')).toBeNull();
  });

  it('renders tabs and dispatches enabled commands only', () => {
    const open = vi.fn();
    const bold = vi.fn();
    render(
      <Ribbon
        tabs={[
          {
            id: 'home',
            label: 'Trang đầu',
            groups: [
              {
                id: 'g',
                label: 'Tệp',
                items: [
                  { id: 'file.open', label: 'Mở tệp', onSelect: open },
                  { id: 'format.bold', label: 'Đậm', enabled: false, onSelect: bold },
                ],
              },
            ],
          },
          { id: 'view', label: 'Xem', groups: [] },
        ]}
      />,
    );
    expect(screen.getByRole('tab', { name: 'Trang đầu' })).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Xem' })).toBeTruthy();
    fireEvent.click(screen.getByText('Mở tệp'));
    fireEvent.click(screen.getByText('Đậm'));
    expect(open).toHaveBeenCalledTimes(1);
    expect(bold).not.toHaveBeenCalled();
    expect(screen.getByRole('group', { name: 'Tệp' })).toBeTruthy();
  });

  it('renders custom controls and popovers', () => {
    const pick = vi.fn();
    render(
      <Ribbon
        tabs={[
          {
            id: 'home',
            label: 'Trang đầu',
            groups: [
              {
                id: 'font',
                label: 'Phông chữ',
                items: [
                  {
                    id: 'format.color',
                    label: 'Màu chữ',
                    onSelect: () => {},
                    render: (
                      <Popover label="Màu chữ" testId="color" trigger="A">
                        {(close) => (
                          <button
                            type="button"
                            onClick={() => {
                              pick();
                              close();
                            }}
                          >
                            Đỏ
                          </button>
                        )}
                      </Popover>
                    ),
                  },
                ],
              },
            ],
          },
        ]}
      />,
    );
    expect(screen.queryByText('Đỏ')).toBeNull();
    fireEvent.click(screen.getByTestId('color'));
    fireEvent.click(screen.getByText('Đỏ'));
    expect(pick).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('Đỏ')).toBeNull();
  });
});
