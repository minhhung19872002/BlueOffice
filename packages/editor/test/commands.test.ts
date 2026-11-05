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
});
