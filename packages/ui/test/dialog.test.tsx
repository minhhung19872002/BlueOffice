// @vitest-environment happy-dom
import { act, cleanup, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { Dialog } from '../src';

afterEach(cleanup);

/** An editor that unmounts its dialog when it closes (as BlueSlide / BlueDoc do). */
function Harness({ open }: { open: boolean }) {
  const [, force] = useState(0);
  return (
    <>
      <button type="button" data-testid="opener" onClick={() => force((n) => n + 1)}>
        mở
      </button>
      {open && (
        <Dialog open onOpenChange={() => {}} title="Cỡ trang">
          <input aria-label="Rộng" />
        </Dialog>
      )}
    </>
  );
}

const frame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));

describe('Dialog', () => {
  it('gives the keyboard back to the opener when it unmounts (WCAG 2.4.3)', async () => {
    const { rerender } = render(<Harness open={false} />);
    screen.getByTestId('opener').focus();
    await act(async () => rerender(<Harness open />));
    expect(screen.getByRole('dialog', { name: 'Cỡ trang' })).toBeTruthy();
    // The focus left behind when the dialog goes away lands on <body>…
    await act(async () => {
      (document.activeElement as HTMLElement | null)?.blur();
      rerender(<Harness open={false} />);
    });
    await act(frame);
    // …and is moved back to the element that opened the dialog.
    expect(document.activeElement).toBe(screen.getByTestId('opener'));
  });

  it('does not steal the focus from an element focused after it closed', async () => {
    const { rerender } = render(
      <>
        <Harness open={false} />
        <input data-testid="other" />
      </>,
    );
    screen.getByTestId('opener').focus();
    await act(async () =>
      rerender(
        <>
          <Harness open />
          <input data-testid="other" />
        </>,
      ),
    );
    await act(async () =>
      rerender(
        <>
          <Harness open={false} />
          <input data-testid="other" />
        </>,
      ),
    );
    screen.getByTestId('other').focus();
    await act(frame);
    expect(document.activeElement).toBe(screen.getByTestId('other'));
  });
});
