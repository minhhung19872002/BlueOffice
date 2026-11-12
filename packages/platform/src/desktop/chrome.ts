/**
 * Window chrome of the desktop apps (ADR-0016). On Windows the window has no system frame:
 * the app title bar is the caption (drag, double-click, window buttons, system menu) and the
 * native shell puts a snap-layouts hit area over its maximise button. Mica sits behind the
 * title bar and ribbon on Windows 11. macOS and Linux keep their native frame; every desktop
 * gets its system UI font through `<html data-os>` (theme.css).
 */
import type {
  MaximizeButtonState,
  WindowBackdrop,
  WindowChrome,
  WindowChromeState,
  WindowManager,
} from '../types';

/** What `chrome_info` returns (apps/desktop/src-tauri/src/chrome_cmds.rs). */
export interface ChromeInfo {
  os: 'windows' | 'macos' | 'linux';
  build: number;
  custom: boolean;
  mica: boolean;
}

/** The window operations the chrome needs (Tauri's current window; tests inject a fake). */
export interface ChromeWindowApi {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  startDragging(): Promise<void>;
  isMaximized(): Promise<boolean>;
  isFocused(): Promise<boolean>;
  onResized(handler: () => void): Promise<() => void>;
  onFocusChanged(handler: (focused: boolean) => void): Promise<() => void>;
  /** Listens to an event sent to this window. */
  listen<T>(event: string, handler: (payload: T) => void): Promise<() => void>;
}

type Call = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

/** Event of the native maximise button (chrome_cmds.rs `MAX_BUTTON_EVENT`). */
export const MAX_BUTTON_EVENT = 'bo-chrome:max-button';

/** Subscribes through an async listener; the returned function also cancels a pending one. */
function subscribe(start: () => Promise<() => void>): () => void {
  let off: (() => void) | null = null;
  let disposed = false;
  void start()
    .then((f) => {
      if (disposed) f();
      else off = f;
    })
    .catch(() => {});
  return () => {
    disposed = true;
    off?.();
  };
}

/** Caption of a frameless desktop window. */
export class DesktopChrome implements WindowChrome {
  private dpr: () => number;
  constructor(
    readonly os: WindowChrome['os'],
    private readonly mica: boolean,
    private readonly call: Call,
    private readonly win: ChromeWindowApi,
    private readonly root: HTMLElement | null = typeof document === 'undefined'
      ? null
      : document.documentElement,
    dpr?: () => number,
  ) {
    this.dpr = dpr ?? (() => (typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1));
  }

  minimize(): void {
    void this.win.minimize().catch(() => {});
  }
  toggleMaximize(): void {
    void this.win.toggleMaximize().catch(() => {});
  }
  close(): void {
    void this.win.close().catch(() => {});
  }
  startDragging(): void {
    void this.win.startDragging().catch(() => {});
  }
  showSystemMenu(x: number, y: number): void {
    void this.call('chrome_system_menu', { x, y, scale: this.dpr() }).catch(() => {});
  }

  async getState(): Promise<WindowChromeState> {
    const [maximized, focused] = await Promise.all([
      this.win.isMaximized().catch(() => false),
      this.win.isFocused().catch(() => true),
    ]);
    return { maximized, focused };
  }

  onStateChange(handler: (state: WindowChromeState) => void): () => void {
    let last: WindowChromeState | null = null;
    const update = () =>
      void this.getState().then((s) => {
        if (last && last.maximized === s.maximized && last.focused === s.focused) return;
        last = s;
        this.root?.setAttribute('data-window-focused', String(s.focused));
        handler(s);
      });
    update();
    const offResize = subscribe(() => this.win.onResized(update));
    const offFocus = subscribe(() => this.win.onFocusChanged(update));
    return () => {
      offResize();
      offFocus();
    };
  }

  setMaximizeButton(rect: { x: number; y: number; width: number; height: number } | null): void {
    const r = rect && {
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,
    };
    void this.call('chrome_set_max_button', { rect: r, scale: this.dpr() }).catch(() => {});
  }

  onMaximizeButton(handler: (state: MaximizeButtonState) => void): () => void {
    return subscribe(() => this.win.listen<MaximizeButtonState>(MAX_BUTTON_EVENT, handler));
  }

  async setAppearance({ dark, solid }: { dark: boolean; solid: boolean }): Promise<WindowBackdrop> {
    const wanted = !this.mica || solid ? 'none' : dark ? 'micaDark' : 'micaLight';
    const on = await this.call<boolean>('chrome_set_backdrop', { backdrop: wanted }).catch(
      () => false,
    );
    const backdrop: WindowBackdrop = on ? 'mica' : 'none';
    this.root?.setAttribute('data-backdrop', backdrop);
    return backdrop;
  }
}

/** Tauri's current window as a {@link ChromeWindowApi}. */
async function tauriWindowApi(): Promise<ChromeWindowApi> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const w = getCurrentWindow();
  return {
    minimize: () => w.minimize(),
    toggleMaximize: () => w.toggleMaximize(),
    close: () => w.close(),
    startDragging: () => w.startDragging(),
    isMaximized: () => w.isMaximized(),
    isFocused: () => w.isFocused(),
    onResized: (h) => w.onResized(() => h()),
    onFocusChanged: (h) => w.onFocusChanged((e) => h(e.payload)),
    listen: (event, h) => w.listen(event, (e) => h(e.payload as never)),
  };
}

/**
 * Asks the shell about the window chrome, tags `<html>` (`data-os`; `data-chrome="custom"` when
 * the app title bar is the caption) and gives `window` its {@link WindowChrome}. Call once
 * before the first render; failures leave the native frame and the web title bar as they are.
 */
export async function initWindowChrome(
  window: WindowManager,
  call: Call,
  api: () => Promise<ChromeWindowApi> = tauriWindowApi,
  root: HTMLElement | null = typeof document === 'undefined' ? null : document.documentElement,
): Promise<ChromeInfo | null> {
  const info = await call<ChromeInfo>('chrome_info').catch(() => null);
  if (!info) return null;
  root?.setAttribute('data-os', info.os);
  if (info.custom) {
    root?.setAttribute('data-chrome', 'custom');
    root?.setAttribute('data-backdrop', 'none');
    window.chrome = new DesktopChrome(info.os, info.mica, call, await api(), root);
  }
  return info;
}
