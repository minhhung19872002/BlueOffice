import { describe, expect, it, vi } from 'vitest';
import {
  DesktopChrome,
  initWindowChrome,
  MAX_BUTTON_EVENT,
  type ChromeInfo,
  type ChromeWindowApi,
} from '../src/desktop';
import type { WindowManager } from '../src/types';

/** `<html>` stand-in: only the attributes matter. */
function fakeRoot() {
  const attrs = new Map<string, string>();
  return {
    attrs,
    el: {
      setAttribute: (k: string, v: string) => attrs.set(k, v),
    } as unknown as HTMLElement,
  };
}

function fakeWindowApi(): ChromeWindowApi & { emit: (e: string, p: unknown) => void } {
  const listeners = new Map<string, ((p: unknown) => void)[]>();
  return {
    minimize: vi.fn(async () => {}),
    toggleMaximize: vi.fn(async () => {}),
    close: vi.fn(async () => {}),
    startDragging: vi.fn(async () => {}),
    isMaximized: vi.fn(async () => true),
    isFocused: vi.fn(async () => false),
    onResized: vi.fn(async () => () => {}),
    onFocusChanged: vi.fn(async () => () => {}),
    listen: vi.fn(async (e: string, h: (p: never) => void) => {
      listeners.set(e, [...(listeners.get(e) ?? []), h as (p: unknown) => void]);
      return () => {};
    }),
    emit: (e, p) => listeners.get(e)?.forEach((h) => h(p)),
  };
}

const windows = (): WindowManager => ({ setTitle: () => {}, openNew: async () => {} });

describe('desktop window chrome (ADR-0016)', () => {
  it('Windows: tags <html> and gives the window its caption controls', async () => {
    const info: ChromeInfo = { os: 'windows', build: 22631, custom: true, mica: true };
    const call = vi.fn(async () => info) as never;
    const root = fakeRoot();
    const w = windows();
    expect(await initWindowChrome(w, call, async () => fakeWindowApi(), root.el)).toEqual(info);
    expect(root.attrs.get('data-os')).toBe('windows');
    expect(root.attrs.get('data-chrome')).toBe('custom');
    expect(root.attrs.get('data-backdrop')).toBe('none');
    expect(w.chrome?.os).toBe('windows');
  });

  it('macOS / Linux keep the native frame: only the UI font tag', async () => {
    const root = fakeRoot();
    const w = windows();
    const call = vi.fn(async () => ({
      os: 'macos',
      build: 0,
      custom: false,
      mica: false,
    })) as never;
    await initWindowChrome(w, call, async () => fakeWindowApi(), root.el);
    expect(root.attrs.get('data-os')).toBe('macos');
    expect(root.attrs.has('data-chrome')).toBe(false);
    expect(w.chrome).toBeUndefined();
    // No shell (tests, old builds): nothing changes.
    const none = fakeRoot();
    const failing = vi.fn(async () => {
      throw new Error('unknown command');
    }) as never;
    expect(await initWindowChrome(windows(), failing, async () => fakeWindowApi(), none.el)).toBe(
      null,
    );
    expect(none.attrs.size).toBe(0);
  });

  it('maps the window operations and the snap-layouts button to the shell', async () => {
    const calls: [string, unknown][] = [];
    const call = vi.fn(async (cmd: string, args?: unknown) => {
      calls.push([cmd, args]);
      return cmd === 'chrome_set_backdrop';
    }) as never;
    const api = fakeWindowApi();
    const root = fakeRoot();
    const c = new DesktopChrome('windows', true, call, api, root.el, () => 1.5);
    c.minimize();
    c.toggleMaximize();
    c.close();
    c.startDragging();
    expect(api.minimize).toHaveBeenCalledOnce();
    expect(api.toggleMaximize).toHaveBeenCalledOnce();
    expect(api.close).toHaveBeenCalledOnce();
    expect(api.startDragging).toHaveBeenCalledOnce();
    c.setMaximizeButton({ x: 1188, y: 0, width: 46, height: 40 });
    c.setMaximizeButton(null);
    c.showSystemMenu(12, 40);
    await Promise.resolve();
    expect(calls).toEqual([
      ['chrome_set_max_button', { rect: { x: 1188, y: 0, width: 46, height: 40 }, scale: 1.5 }],
      ['chrome_set_max_button', { rect: null, scale: 1.5 }],
      ['chrome_system_menu', { x: 12, y: 40, scale: 1.5 }],
    ]);
    expect(await c.getState()).toEqual({ maximized: true, focused: false });
    const seen: unknown[] = [];
    c.onMaximizeButton((s) => seen.push(s));
    await Promise.resolve();
    api.emit(MAX_BUTTON_EVENT, { hover: true, pressed: false });
    expect(seen).toEqual([{ hover: true, pressed: false }]);
    const states: unknown[] = [];
    c.onStateChange((s) => states.push(s));
    await vi.waitFor(() => expect(states).toEqual([{ maximized: true, focused: false }]));
    expect(root.attrs.get('data-window-focused')).toBe('false');
  });

  it('Mica follows the theme; solid for high contrast and without Mica', async () => {
    const asked: unknown[] = [];
    const call = vi.fn(async (cmd: string, args?: { backdrop?: string }) => {
      if (cmd === 'chrome_set_backdrop') asked.push(args?.backdrop);
      return args?.backdrop !== 'none';
    }) as never;
    const root = fakeRoot();
    const c = new DesktopChrome('windows', true, call, fakeWindowApi(), root.el);
    expect(await c.setAppearance({ dark: true, solid: false })).toBe('mica');
    expect(root.attrs.get('data-backdrop')).toBe('mica');
    expect(await c.setAppearance({ dark: false, solid: false })).toBe('mica');
    expect(await c.setAppearance({ dark: false, solid: true })).toBe('none');
    expect(root.attrs.get('data-backdrop')).toBe('none');
    expect(asked).toEqual(['micaDark', 'micaLight', 'none']);
    // Windows 10: never Mica.
    const w10 = new DesktopChrome('windows', false, call, fakeWindowApi(), root.el);
    expect(await w10.setAppearance({ dark: false, solid: false })).toBe('none');
  });
});
