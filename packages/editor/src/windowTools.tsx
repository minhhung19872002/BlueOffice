/**
 * View › Window (CORE-UI-007), shared by BlueDoc, BlueSheet and BlueSlide: New Window,
 * Arrange All, View Side by Side with Synchronous Scrolling and Reset Window Position.
 * The windows find each other on the platform's window channel (`WindowLink`); the
 * editor only gives its scrollable view and its document.
 */
import {
  WINDOWS_CHANNEL,
  WINDOW_PARAM,
  WindowLink,
  newWindowId,
  openDocumentWindow,
  type Platform,
  type ScrollTarget,
  type WindowApp,
  type WindowPeer,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

type T = (key: string, options?: Record<string, unknown>) => string;

/** A DOM scroller as the view scrolled in sync. */
export function elementScrollTarget(el: HTMLElement): ScrollTarget {
  return {
    metrics: () => ({
      top: el.scrollTop,
      left: el.scrollLeft,
      maxTop: Math.max(0, el.scrollHeight - el.clientHeight),
      maxLeft: Math.max(0, el.scrollWidth - el.clientWidth),
    }),
    scrollTo: (pos) => el.scrollTo({ top: pos.top, left: pos.left }),
    onScroll: (fn) => {
      el.addEventListener('scroll', fn, { passive: true });
      return () => el.removeEventListener('scroll', fn);
    },
  };
}

export interface WindowToolsOptions {
  platform: Platform;
  app: WindowApp;
  /** Document name (shown to the other windows). */
  title: string;
  /** The editor's main scroller (`null` while there is none). */
  scroller: () => HTMLElement | null;
  /** The open document as a file, for New Window (`null`: nothing open). */
  getFile: () => Promise<{ bytes: Uint8Array; name: string } | null>;
  /** One-line message for the user (status bar). */
  notify: (text: string) => void;
  t: T;
}

export interface WindowTools {
  /** Other open windows of this app. */
  peers: WindowPeer[];
  /** The window this one is side by side with. */
  paired: WindowPeer | null;
  synchronous: boolean;
  /** This window belongs to a New Window session (edits are shared). */
  shared: boolean;
  newWindow: () => Promise<void>;
  arrangeAll: () => Promise<void>;
  /** Toggle View Side by Side. */
  sideBySide: () => void;
  toggleSynchronous: () => void;
  resetPosition: () => void;
  /** The "compare with" chooser (render it once). */
  element: ReactNode;
}

export function useWindowTools(o: WindowToolsOptions): WindowTools {
  const { platform, app, t } = o;
  const [peers, setPeers] = useState<WindowPeer[]>([]);
  const [paired, setPaired] = useState<WindowPeer | null>(null);
  const [synchronous, setSynchronous] = useState(true);
  const [choosing, setChoosing] = useState(false);
  const link = useRef<WindowLink | null>(null);
  const opts = useRef(o);
  useEffect(() => {
    opts.current = o;
  });
  const shared = useMemo(
    () =>
      typeof window !== 'undefined' &&
      new URLSearchParams(window.location.search).has(WINDOW_PARAM),
    [],
  );

  useEffect(() => {
    const bus = platform.channel?.(WINDOWS_CHANNEL);
    if (!bus) return;
    const l = new WindowLink(bus, { id: newWindowId(), app, title: opts.current.title }, () => {
      const el = opts.current.scroller();
      return el ? elementScrollTarget(el) : null;
    });
    link.current = l;
    const offPeers = l.on('peers', setPeers);
    const offPaired = l.on('paired', (p) => {
      setPaired(p);
      setSynchronous(l.synchronous);
    });
    const bye = () => l.dispose();
    window.addEventListener('pagehide', bye);
    return () => {
      window.removeEventListener('pagehide', bye);
      offPeers();
      offPaired();
      l.dispose();
      link.current = null;
    };
  }, [platform, app]);

  useEffect(() => {
    link.current?.setTitle(o.title);
  }, [o.title]);

  const newWindow = useCallback(async () => {
    const { platform: p, notify } = opts.current;
    if (p.kind === 'desktop') {
      // One native engine serves every window of the desktop shell, so a second window
      // cannot show the same document: it opens the app for another one.
      await p.window.openNew(`?app=${app}&start=1`);
      notify(t('windows.newWindowDesktop'));
      return;
    }
    const file = await opts.current.getFile();
    if (!file) {
      await p.window.openNew(`${window.location.pathname}${window.location.search}`);
      return;
    }
    await openDocumentWindow(p, file, { location: window.location });
  }, [app, t]);

  const arrangeAll = useCallback(async () => {
    const { platform: p, notify } = opts.current;
    if (!p.window.arrangeAll) {
      notify(t('windows.arrangeHint'));
      return;
    }
    const n = await p.window.arrangeAll();
    notify(t('windows.arranged', { count: n }));
  }, [t]);

  const sideBySide = useCallback(() => {
    const l = link.current;
    if (!l) {
      // No window channel on this platform: there is nothing to compare with.
      opts.current.notify(t('windows.noOtherWindow'));
      return;
    }
    if (l.pairedWith) {
      l.unpair();
      return;
    }
    const others = l.peers();
    if (others.length === 0) opts.current.notify(t('windows.noOtherWindow'));
    else if (others.length === 1) l.pair(others[0]!.id);
    else setChoosing(true);
  }, [t]);

  const toggleSynchronous = useCallback(() => {
    const l = link.current;
    if (!l?.pairedWith) return;
    l.setSynchronous(!l.synchronous);
    setSynchronous(l.synchronous);
  }, []);

  const resetPosition = useCallback(() => {
    link.current?.resetPosition();
    // On the desktop the two windows are also put next to each other again.
    void opts.current.platform.window.arrangeAll?.().catch(() => {});
  }, []);

  const element = choosing ? (
    <Dialog
      open
      onOpenChange={(open) => !open && setChoosing(false)}
      title={t('windows.compareWith')}
      testId="window-compare-dialog"
      footer={
        <button type="button" onClick={() => setChoosing(false)}>
          {t('windows.cancel')}
        </button>
      }
    >
      <ul style={{ listStyle: 'none', margin: 0, padding: 0, display: 'grid', gap: 4 }}>
        {peers.map((p) => (
          <li key={p.id}>
            <button
              type="button"
              style={{ width: '100%', textAlign: 'left' }}
              data-testid="window-compare-item"
              onClick={() => {
                setChoosing(false);
                link.current?.pair(p.id);
              }}
            >
              {p.title || t('windows.untitled')}
            </button>
          </li>
        ))}
      </ul>
    </Dialog>
  ) : null;

  return {
    peers,
    paired,
    synchronous,
    shared,
    newWindow,
    arrangeAll,
    sideBySide,
    toggleSynchronous,
    resetPosition,
    element,
  };
}
