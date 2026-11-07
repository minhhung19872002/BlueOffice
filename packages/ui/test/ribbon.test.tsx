// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Popover, Ribbon } from '../src';

afterEach(cleanup);

describe('Ribbon', () => {
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
