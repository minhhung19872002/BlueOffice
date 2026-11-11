import { describe, expect, it } from 'vitest';
import { editableInput, isArrayEntry } from '../src/arrayFormula';

describe('array formula entry', () => {
  it('edits a legacy array formula without its braces', () => {
    expect(editableInput({ input: '{=A1:A3*2}', array: 'D1:D3' })).toBe('=A1:A3*2');
    // Text that merely looks like braces stays as typed.
    expect(editableInput({ input: '{=x}' })).toBe('{=x}');
    expect(editableInput({ input: '=SUM(A1:A3)' })).toBe('=SUM(A1:A3)');
  });

  it('recognises Ctrl+Shift+Enter only', () => {
    const k = { key: 'Enter', ctrlKey: true, metaKey: false, shiftKey: true, altKey: false };
    expect(isArrayEntry(k)).toBe(true);
    expect(isArrayEntry({ ...k, ctrlKey: false, metaKey: true })).toBe(true);
    expect(isArrayEntry({ ...k, shiftKey: false })).toBe(false);
    expect(isArrayEntry({ ...k, ctrlKey: false })).toBe(false);
    expect(isArrayEntry({ ...k, altKey: true })).toBe(false);
  });
});
