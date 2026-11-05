// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { ImeInput, type ImeEvent } from '../src/ime';

/** happy-dom ignores CompositionEventInit.data; set it explicitly. */
function comp(type: string, data: string): CompositionEvent {
  const e = new CompositionEvent(type);
  Object.defineProperty(e, 'data', { value: data });
  return e;
}

// "việt" with the vowel decomposed: e + U+0323 (dot below) + U+0302 (circumflex).
const DECOMPOSED = 'việt';

describe('ImeInput', () => {
  it('commits composed Vietnamese text once, NFC-normalized', () => {
    const events: ImeEvent[] = [];
    const ime = new ImeInput(document.body, (e) => events.push(e));
    const ta = ime.element;
    ta.dispatchEvent(comp('compositionstart', ''));
    ta.dispatchEvent(comp('compositionupdate', 'vie'));
    // While composing, input events are ignored.
    ta.value = 'vie';
    ta.dispatchEvent(new InputEvent('input', { data: 'e', isComposing: true }));
    ta.dispatchEvent(comp('compositionend', DECOMPOSED));
    expect(events).toEqual([
      { type: 'composition', text: 'vie' },
      { type: 'insert', text: DECOMPOSED.normalize('NFC') },
    ]);
    expect((events[1] as { text: string }).text.length).toBe(4);
    expect(ta.value).toBe('');
    ime.dispose();
  });

  it('reports plain typing and shortcut keys', () => {
    const events: ImeEvent[] = [];
    const ime = new ImeInput(document.body, (e) => events.push(e));
    ime.element.value = 'a';
    ime.element.dispatchEvent(new InputEvent('input', { data: 'a' }));
    ime.element.dispatchEvent(new KeyboardEvent('keydown', { key: 'b', ctrlKey: true }));
    ime.element.dispatchEvent(new KeyboardEvent('keydown', { key: 'x' }));
    expect(events).toEqual([
      { type: 'insert', text: 'a' },
      { type: 'key', key: 'b', ctrl: true, shift: false, alt: false },
    ]);
    ime.dispose();
  });
});
