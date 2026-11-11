// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';

interface S {
  doc: boolean;
}

describe('CommandRegistry', () => {
  it('computes enabled/checked state from editor state', async () => {
    const r = new CommandRegistry<S>();
    const run = vi.fn();
    r.register({ id: 'file.exportPdf', isEnabled: (s) => s.doc, isChecked: () => true, run });
    expect(r.isEnabled('file.exportPdf', { doc: false })).toBe(false);
    expect(await r.execute('file.exportPdf', { doc: false })).toBe(false);
    expect(run).not.toHaveBeenCalled();
    expect(await r.execute('file.exportPdf', { doc: true })).toBe(true);
    expect(run).toHaveBeenCalledTimes(1);
    expect(r.isChecked('file.exportPdf', { doc: true })).toBe(true);
    expect(r.isEnabled('missing', { doc: true })).toBe(false);
  });

  it('rejects duplicate ids and supports unregister', () => {
    const r = new CommandRegistry<S>();
    const un = r.register({ id: 'a', run: () => {} });
    expect(() => r.register({ id: 'a', run: () => {} })).toThrow();
    un();
    expect(r.has('a')).toBe(false);
  });

  it('matches keyboard shortcuts (Ctrl ≡ Cmd)', () => {
    const r = new CommandRegistry<S>();
    r.register({ id: 'file.open', shortcut: 'Ctrl+O', run: () => {} });
    r.register({ id: 'file.exportPdf', shortcut: 'Ctrl+Shift+E', run: () => {} });
    const ev = (key: string, mods: Partial<KeyboardEvent> = {}) => ({
      key,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      altKey: false,
      ...mods,
    });
    expect(r.match(ev('o', { ctrlKey: true }))?.id).toBe('file.open');
    expect(r.match(ev('O', { metaKey: true }))?.id).toBe('file.open');
    expect(r.match(ev('E', { ctrlKey: true, shiftKey: true }))?.id).toBe('file.exportPdf');
    expect(r.match(ev('o'))).toBeUndefined();
  });

  it('matches real key events by physical key: Ctrl+Shift+= (Plus), Ctrl+Shift+>, Ctrl+Space', () => {
    const r = new CommandRegistry<S>();
    r.register({ id: 'format.superscript', shortcut: 'Ctrl+Shift+=', run: () => {} });
    r.register({ id: 'format.growStep', shortcut: 'Ctrl+Shift+>', run: () => {} });
    r.register({ id: 'format.clearChar', shortcut: 'Ctrl+Space', run: () => {} });
    r.register({ id: 'view.formattingMarks', shortcut: 'Ctrl+Shift+8', run: () => {} });
    // A real KeyboardEvent: its flags are getters (a spread copy would lose them).
    const key = (k: string, code: string, shift = true) =>
      new KeyboardEvent('keydown', { key: k, code, ctrlKey: true, shiftKey: shift });
    expect(r.match(key('+', 'Equal'))?.id).toBe('format.superscript');
    expect(r.match(key('>', 'Period'))?.id).toBe('format.growStep');
    expect(r.match(key(' ', 'Space', false))?.id).toBe('format.clearChar');
    expect(r.match(key('*', 'Digit8'))?.id).toBe('view.formattingMarks');
  });
});
