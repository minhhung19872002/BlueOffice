// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { ScreenRect, SheetHost, SheetPeer, SheetView } from '@blueoffice/platform';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { PeerSelections, peersOnSheet, usePeerBoxes } from '../src/peers';

afterEach(cleanup);

const peer = (clientId: number, name: string, sheet: string, range: string): SheetPeer => ({
  clientId,
  name,
  color: '#7c3aed',
  sheet,
  range,
});

/** A host whose cells are 64 × 20 pt and that reports presence through `emit`. */
function fakeHost() {
  let listener: ((p: SheetPeer[]) => void) | null = null;
  const host = {
    rangeRect: (_v: SheetView, r0: number, c0: number, r1: number, c1: number) =>
      Promise.resolve<ScreenRect>({
        x: c0 * 64,
        y: r0 * 20,
        w: (c1 - c0 + 1) * 64,
        h: (r1 - r0 + 1) * 20,
      }),
    onSheetPeers: (l: (p: SheetPeer[]) => void) => {
      listener = l;
      l([]);
      return () => {
        listener = null;
      };
    },
  } as unknown as SheetHost;
  return { host, emit: (p: SheetPeer[]) => listener?.(p) };
}

const view = { sheet: 0, x: 0, y: 0, width: 800, height: 600 } as unknown as SheetView;
const px = (r: ScreenRect | null) => r && { x: r.x * 2, y: r.y * 2, w: r.w * 2, h: r.h * 2 };

function Layer({ host, sheet }: { host: SheetHost; sheet: string }) {
  const boxes = usePeerBoxes(host, view, sheet, px, 0);
  return <PeerSelections boxes={boxes} />;
}

describe('BlueSheet collaborators', () => {
  it('keeps the peers of the shown sheet with their ranges parsed', () => {
    const on = peersOnSheet(
      [
        peer(1, 'An', 'Sheet1', 'B2:C4'),
        peer(2, 'Bình', 'Dữ liệu', 'A1'),
        peer(3, 'X', 'Sheet1', '??'),
      ],
      'Sheet1',
    );
    expect(on.map((p) => [p.clientId, p.cells])).toEqual([[1, [1, 1, 3, 2]]]);
  });

  it('draws a coloured border with a name tag around each selected range', async () => {
    const { host, emit } = fakeHost();
    render(
      <I18nextProvider i18n={createI18n('vi-VN')}>
        <Layer host={host} sheet="Sheet1" />
      </I18nextProvider>,
    );
    expect(screen.queryAllByTestId('peer-selection')).toHaveLength(0);
    act(() => emit([peer(1, 'Trần Bình', 'Sheet1', 'B2:C3'), peer(2, 'Chi', 'Sheet2', 'A1')]));
    const box = await waitFor(() => screen.getByTestId('peer-selection'));
    expect(screen.getAllByTestId('peer-selection')).toHaveLength(1);
    expect(box.getAttribute('aria-label')).toBe('Trần Bình đang chọn B2:C3');
    expect(box.textContent).toBe('Trần Bình');
    // Screen pixels: B2 is at column 1, row 1 of a 64 × 20 grid, zoomed ×2.
    expect([box.style.left, box.style.top, box.style.width, box.style.height]).toEqual([
      '128px',
      '40px',
      '256px',
      '80px',
    ]);
    expect(box.style.borderColor).toMatch(/7c3aed|124, 58, 237/i);
    act(() => emit([]));
    await waitFor(() => expect(screen.queryAllByTestId('peer-selection')).toHaveLength(0));
  });
});
