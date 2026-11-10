// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  CommandRegistry,
  DOC_REPEATABLE,
  RepeatTracker,
  SHORTCUTS_FORMAT,
  ShortcutsDialog,
  assignShortcut,
  exportShortcuts,
  importShortcuts,
  loadShortcuts,
  saveShortcuts,
  shortcutFromKey,
  shortcutsKey,
  validShortcut,
  type ShortcutMap,
} from '../src';

afterEach(cleanup);

const key = (k: string, mods: Partial<KeyboardEvent> = {}) => ({
  key: k,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  altKey: false,
  ...mods,
});

function registry() {
  const r = new CommandRegistry<null>();
  const bold = vi.fn();
  const italic = vi.fn();
  r.register({ id: 'format.bold', shortcut: 'Ctrl+B', run: bold });
  r.register({ id: 'format.italic', shortcut: 'Ctrl+I', run: italic });
  r.register({ id: 'edit.find', run: vi.fn() });
  return { r, bold, italic };
}

describe('customisable shortcuts (CORE-UI-004)', () => {
  it('user shortcuts replace the defaults when matching keys', () => {
    const { r } = registry();
    r.setShortcutOverrides({ 'format.bold': 'Ctrl+Shift+D', 'edit.find': 'Ctrl+B' });
    expect(r.match(key('d', { ctrlKey: true, shiftKey: true }))?.id).toBe('format.bold');
    // Ctrl+B moved to Find: the user's binding wins over the old owner.
    expect(r.match(key('b', { ctrlKey: true }))?.id).toBe('edit.find');
    expect(r.get('format.bold')?.shortcut).toBe('Ctrl+Shift+D');
    expect(r.defaultShortcut('format.bold')).toBe('Ctrl+B');
    r.setShortcutOverrides({ 'format.italic': null });
    expect(r.match(key('i', { ctrlKey: true }))).toBeUndefined();
    expect(r.shortcut('format.italic')).toBeUndefined();
  });

  it('detects conflicts and reassigns the combo away from the old command', () => {
    const { r } = registry();
    expect(r.conflicts('ctrl+b')).toEqual(['format.bold']);
    expect(r.conflicts('Ctrl+B', 'format.bold')).toEqual([]);
    const next = assignShortcut(
      {},
      'edit.find',
      'Ctrl+B',
      r.conflicts('Ctrl+B', 'edit.find'),
      (id) => r.defaultShortcut(id),
    );
    expect(next).toEqual({ 'format.bold': null, 'edit.find': 'Ctrl+B' });
    // Back to the default: the change is dropped.
    expect(
      assignShortcut(next, 'format.bold', 'Ctrl+B', [], (id) => r.defaultShortcut(id)),
    ).toEqual({ 'edit.find': 'Ctrl+B' });
  });

  it('reads key presses and refuses reserved or bare keys', () => {
    expect(
      shortcutFromKey({ ...key('*', { ctrlKey: true, shiftKey: true }), code: 'Digit8' }),
    ).toBe('Ctrl+Shift+8');
    expect(shortcutFromKey(key('Control', { ctrlKey: true }))).toBeNull();
    expect(validShortcut('Ctrl+K')).toBe(true);
    expect(validShortcut('F4')).toBe(true);
    expect(validShortcut('K')).toBe(false);
    expect(validShortcut('Shift+F10')).toBe(false);
    expect(validShortcut('Tab')).toBe(false);
  });

  it('persists per app in the setting store, falling back to localStorage', async () => {
    const data = new Map<string, string | null>();
    const store = {
      getSetting: async (k: string) => data.get(k) ?? null,
      setSetting: async (k: string, v: string | null) => void data.set(k, v),
    };
    const map: ShortcutMap = { 'format.bold': 'Ctrl+Shift+B', 'edit.find': null, bad: 'K' };
    await saveShortcuts(store, 'doc', map);
    expect(await loadShortcuts(store, 'doc')).toEqual({
      'format.bold': 'Ctrl+Shift+B',
      'edit.find': null,
    });
    expect(await loadShortcuts(store, 'sheet')).toEqual({});
    localStorage.clear();
    await saveShortcuts({}, 'slide', { 'format.bold': 'Ctrl+J' });
    expect(localStorage.getItem(shortcutsKey('slide'))).toContain('Ctrl+J');
    expect(await loadShortcuts({}, 'slide')).toEqual({ 'format.bold': 'Ctrl+J' });
  });

  it('exports and imports a JSON file of the same app only', () => {
    const text = exportShortcuts('doc', { 'format.bold': 'Ctrl+Shift+B' });
    expect(JSON.parse(text)).toMatchObject({ format: SHORTCUTS_FORMAT, app: 'doc' });
    expect(importShortcuts(text, 'doc')).toEqual({
      ok: true,
      map: { 'format.bold': 'Ctrl+Shift+B' },
    });
    expect(importShortcuts(text, 'sheet')).toEqual({ ok: false, error: 'wrongApp' });
    expect(importShortcuts('{', 'doc')).toEqual({ ok: false, error: 'invalid' });
    expect(importShortcuts('{"format":"x"}', 'doc')).toEqual({ ok: false, error: 'invalid' });
  });

  it('the dialog changes a shortcut by key press and warns about conflicts', async () => {
    const { r } = registry();
    const i18n = createI18n('vi-VN');
    let value: ShortcutMap = {};
    const onChange = vi.fn((m: ShortcutMap) => {
      value = m;
    });
    const view = (
      <I18nextProvider i18n={i18n}>
        <ShortcutsDialog
          open
          onClose={vi.fn()}
          registry={r}
          commands={[
            { id: 'format.bold', label: 'Đậm' },
            { id: 'format.italic', label: 'Nghiêng' },
            { id: 'missing', label: 'Không có' },
          ]}
          value={value}
          onChange={onChange}
          onExport={vi.fn()}
          onImport={vi.fn()}
        />
      </I18nextProvider>
    );
    render(view);
    expect(document.querySelectorAll('[data-shortcut-row]')).toHaveLength(2);
    const italicRow = document.querySelector('[data-shortcut-row="format.italic"]')!;
    await act(async () =>
      fireEvent.click(italicRow.querySelector('[data-testid="shortcut-change"]')!),
    );
    const capture = screen.getByTestId('shortcut-capture');
    await act(async () => fireEvent.keyDown(capture, { key: 'b', code: 'KeyB', ctrlKey: true }));
    expect(screen.getByTestId('shortcut-conflict').textContent).toContain('Đậm');
    await act(async () => fireEvent.click(screen.getByTestId('shortcut-assign')));
    expect(onChange).toHaveBeenLastCalledWith({ 'format.bold': null, 'format.italic': 'Ctrl+B' });
  });
});

describe('repeat last action (F4)', () => {
  it('remembers the last repeatable command; moves and queries do not forget it', () => {
    const t = new RepeatTracker<{ cmd: string; [k: string]: unknown }>(DOC_REPEATABLE);
    expect(t.get()).toBeNull();
    t.record({ cmd: 'formatText', format: { kind: 'bold' }, now: 1 });
    t.record({ cmd: 'click', page: 0, x: 1, y: 1 });
    t.record({ cmd: 'spelling' });
    expect(t.get()).toEqual({ cmd: 'formatText', format: { kind: 'bold' } });
    t.record({ cmd: 'transform', preview: true });
    expect(t.get()?.cmd).toBe('formatText');
  });

  it('typing is one run of text, ended by a caret move', () => {
    const t = new RepeatTracker<{ cmd: string; text?: string }>(DOC_REPEATABLE);
    t.record({ cmd: 'insertText', text: 'a' });
    t.record({ cmd: 'insertText', text: 'b' });
    expect(t.get()).toEqual({ cmd: 'insertText', text: 'ab' });
    t.record({ cmd: 'move' });
    t.record({ cmd: 'insertText', text: 'c' });
    expect(t.get()).toEqual({ cmd: 'insertText', text: 'c' });
  });

  it('registry-level repeat tracks the last accepted command', async () => {
    const { r, bold } = registry();
    r.trackRepeat((id) => id.startsWith('format.'));
    await r.execute('edit.find', null);
    expect(r.lastRepeatable).toBeNull();
    await r.execute('format.bold', null);
    await r.execute('edit.find', null);
    expect(r.lastRepeatable).toBe('format.bold');
    expect(bold).toHaveBeenCalledTimes(1);
  });
});
