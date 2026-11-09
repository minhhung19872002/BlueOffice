// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import type { Balloon, PageMarkup } from '@blueoffice/platform';
import { drawMarkup } from '../src/markup';

const balloon = (over: Partial<Balloon> = {}): Balloon => ({
  id: 3,
  kind: 'comment',
  author: 'An',
  date: null,
  text: 'Kiểm tra',
  done: false,
  color: '#1f5fa8',
  x: 605,
  y: 90,
  w: 164,
  h: 28,
  anchorX: 120,
  anchorY: 100,
  icon: false,
  ...over,
});

const markup = (balloons: Balloon[], width = 180): PageMarkup => ({
  width,
  bars: [{ x: 60, top: 90, bottom: 110, color: '#595959' }],
  balloons,
});

describe('drawMarkup', () => {
  it('draws the markup area, change bars, balloons and connectors', () => {
    const layer = document.createElement('div');
    const onSelect = vi.fn();
    drawMarkup(layer, markup([balloon(), balloon({ id: 4, kind: 'delete', y: 122 })]), {
      k: 2,
      pageWidth: 595,
      pageHeight: 842,
      label: (k) => (k === 'delete' ? 'Xoá' : k),
      open: new Set(),
      onSelect,
    });
    const area = layer.querySelector<HTMLElement>('[data-testid="markup-area"]')!;
    expect(area.style.left).toBe('1190px');
    expect(area.style.width).toBe('360px');
    expect(layer.querySelectorAll('[data-testid="change-bar"]')).toHaveLength(1);
    const balloons = layer.querySelectorAll<HTMLElement>('[data-testid="markup-balloon"]');
    expect(balloons).toHaveLength(2);
    expect(balloons[0]!.style.top).toBe('180px');
    // Change kinds get their label; comments show their text only.
    expect(balloons[0]!.textContent).toBe('AnKiểm tra');
    expect(balloons[1]!.textContent).toBe('AnXoá: Kiểm tra');
    expect(layer.querySelectorAll('[data-testid="balloon-connector"]')).toHaveLength(2);
    // Clicking a balloon selects its mark; mousedown does not reach the page.
    const down = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
    const pageDown = vi.fn();
    layer.addEventListener('mousedown', pageDown);
    balloons[1]!.dispatchEvent(down);
    expect(pageDown).not.toHaveBeenCalled();
    balloons[1]!.click();
    expect(onSelect).toHaveBeenCalledWith({ kind: 'delete', id: 4 });
  });

  it('simple markup: comment icons open and close their balloon', () => {
    const layer = document.createElement('div');
    const open = new Set<string>();
    const opts = {
      k: 1,
      pageWidth: 595,
      pageHeight: 842,
      open,
      onToggle: () => drawMarkup(layer, m, opts),
    };
    const m = markup([balloon({ icon: true, x: 599, w: 12, h: 12 })], 24);
    drawMarkup(layer, m, opts);
    const icon = () => layer.querySelector<HTMLButtonElement>('[data-testid="markup-icon"]')!;
    expect(icon().getAttribute('aria-expanded')).toBe('false');
    expect(layer.querySelector('[data-testid="markup-balloon"]')).toBeNull();
    expect(layer.querySelector('[data-testid="balloon-connector"]')).toBeNull();
    icon().click();
    expect(icon().getAttribute('aria-expanded')).toBe('true');
    const b = layer.querySelector<HTMLElement>('[data-testid="markup-balloon"]')!;
    expect(b.style.left).toBe('615px');
    icon().click();
    expect(layer.querySelector('[data-testid="markup-balloon"]')).toBeNull();
  });

  it('draws nothing without markup', () => {
    const layer = document.createElement('div');
    layer.appendChild(document.createElement('span'));
    drawMarkup(layer, undefined, { k: 1, pageWidth: 595, pageHeight: 842, open: new Set() });
    expect(layer.childElementCount).toBe(0);
  });
});
