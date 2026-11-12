// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Profiler, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { sameContent, stableMemo } from '../src/stableMemo';

afterEach(cleanup);

describe('sameContent', () => {
  it('compares plain data and elements by content, functions as equal, instances by identity', () => {
    expect(sameContent({ a: [1, { b: 'x' }] }, { a: [1, { b: 'x' }] })).toBe(true);
    expect(sameContent({ a: [1, { b: 'x' }] }, { a: [1, { b: 'y' }] })).toBe(false);
    expect(sameContent({ f: () => 1 }, { f: () => 2 })).toBe(true);
    expect(sameContent(<b className="k">1</b>, <b className="k">1</b>)).toBe(true);
    expect(sameContent(<b className="k">1</b>, <b className="k">2</b>)).toBe(false);
    expect(sameContent(<b />, <i />)).toBe(false);
    expect(sameContent(new Map(), new Map())).toBe(false);
    const m = new Map();
    expect(sameContent({ m }, { m })).toBe(true);
    expect(sameContent([1], { 0: 1 })).toBe(false);
  });
});

interface BarProps {
  items: { id: string; label: string; onSelect: () => void }[];
  extra?: ReactNode;
}
function Bar({ items, extra }: BarProps) {
  return (
    <div>
      {items.map((i) => (
        <button key={i.id} type="button" onClick={i.onSelect}>
          {i.label}
        </button>
      ))}
      {extra}
    </div>
  );
}
const StableBar = stableMemo(Bar);

describe('stableMemo', () => {
  it('skips renders when only callbacks are new, and calls the latest callback', () => {
    let renders = 0;
    const clicks: number[] = [];
    const tree = (n: number, label = 'Bold') => (
      <Profiler id="bar" onRender={() => (renders += 1)}>
        <StableBar
          items={[{ id: 'b', label, onSelect: () => clicks.push(n) }]}
          extra={<span data-x={1}>{label}</span>}
        />
      </Profiler>
    );
    const { rerender } = render(tree(1));
    expect(renders).toBe(1);
    // A keystroke: the parent rebuilds identical props with fresh closures.
    rerender(tree(2));
    rerender(tree(3));
    // The Profiler counts the wrapper too; the wrapped component itself did not render.
    expect(screen.getAllByRole('button')).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Bold' }));
    expect(clicks).toEqual([3]);
    const before = renders;
    rerender(tree(4, 'Italic'));
    expect(renders).toBeGreaterThan(before);
    expect(screen.getByRole('button', { name: 'Italic' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Italic' }));
    expect(clicks).toEqual([3, 4]);
  });

  it('counts renders of the wrapped component only on content changes', () => {
    let inner = 0;
    const onRender = () => (inner += 1);
    // The Profiler inside the wrapped component commits only when that component renders.
    function Counted(p: { label: string; onClick: () => void }) {
      return (
        <Profiler id="inner" onRender={onRender}>
          <button type="button" onClick={p.onClick}>
            {p.label}
          </button>
        </Profiler>
      );
    }
    const S = stableMemo(Counted);
    const { rerender } = render(<S label="a" onClick={() => {}} />);
    for (let i = 0; i < 5; i++) rerender(<S label="a" onClick={() => {}} />);
    expect(inner).toBe(1);
    rerender(<S label="b" onClick={() => {}} />);
    expect(inner).toBe(2);
  });
});
