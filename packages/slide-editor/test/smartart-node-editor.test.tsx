// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SmartArtNodeEditor } from '../src/SmartArtNodeEditor';

afterEach(cleanup);

const node = { id: 7, node: 1, text: 'Xử lý', x: 100, y: 50, w: 120, h: 60 };

// CORE-SMART-004: the text of a diagram node is edited in place on the slide.
describe('SmartArtNodeEditor', () => {
  it('sits over the node box and commits on Enter', () => {
    const onDone = vi.fn();
    render(<SmartArtNodeEditor node={node} scale={2} label="Edit the node text" onDone={onDone} />);
    const box = screen.getByTestId('smartart-node-editor') as HTMLTextAreaElement;
    expect(box.value).toBe('Xử lý');
    expect(box.getAttribute('aria-label')).toBe('Edit the node text');
    expect(box.style.left).toBe('200px');
    expect(box.style.top).toBe('100px');
    expect(box.style.width).toBe('240px');
    expect(box.style.height).toBe('120px');
    expect(document.activeElement).toBe(box);
    fireEvent.change(box, { target: { value: 'Thẩm định' } });
    fireEvent.keyDown(box, { key: 'Enter' });
    expect(onDone).toHaveBeenCalledWith('Thẩm định');
    // Leaving the box afterwards does not commit a second time.
    fireEvent.blur(box);
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('gives up on Escape, commits when left, keeps Shift+Enter and the slide shortcuts out', () => {
    const onDone = vi.fn();
    const onSlideKey = vi.fn();
    render(
      <div onKeyDown={onSlideKey}>
        <SmartArtNodeEditor node={node} scale={1} label="x" onDone={onDone} />
      </div>,
    );
    const box = screen.getByTestId('smartart-node-editor');
    fireEvent.keyDown(box, { key: 'Delete' });
    fireEvent.keyDown(box, { key: 'Enter', shiftKey: true });
    expect(onDone).not.toHaveBeenCalled();
    expect(onSlideKey).not.toHaveBeenCalled();
    fireEvent.keyDown(box, { key: 'Escape' });
    expect(onDone).toHaveBeenCalledWith(null);
    cleanup();
    const left = vi.fn();
    render(<SmartArtNodeEditor node={node} scale={1} label="x" onDone={left} />);
    const again = screen.getByTestId('smartart-node-editor');
    fireEvent.change(again, { target: { value: 'Mới' } });
    fireEvent.blur(again);
    expect(left).toHaveBeenCalledWith('Mới');
  });
});
