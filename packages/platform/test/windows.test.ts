import { describe, expect, it, vi } from 'vitest';
import {
  SPLIT_DEFAULT,
  SPLIT_MIN,
  SyncScroll,
  WINDOW_PARAM,
  WindowLink,
  clampSplit,
  dragSplit,
  fractionToScroll,
  openDocumentWindow,
  scrollFraction,
  splitPanes,
  splitSizes,
  takeWindowDocument,
  unsplitPanes,
  updatePane,
  windowSessionUrl,
  type MessageBus,
  type OfflineCache,
  type ScrollMetrics,
  type ScrollTarget,
} from '../src';

describe('split panes', () => {
  it('keeps both panes at least SPLIT_MIN of the height', () => {
    expect(clampSplit(0)).toBe(SPLIT_MIN);
    expect(clampSplit(1)).toBe(1 - SPLIT_MIN);
    expect(clampSplit(0.4)).toBe(0.4);
    expect(clampSplit(Number.NaN)).toBe(SPLIT_DEFAULT);
  });

  it('shares the container between the panes and the divider', () => {
    expect(splitSizes(606, 0.5)).toEqual([300, 300]);
    expect(splitSizes(1006, 0.25, 6)).toEqual([250, 750]);
    const [a, b] = splitSizes(777, 0.33);
    expect(a + b).toBe(771);
  });

  it('dragging sets the ratio; dropping at an edge removes the split', () => {
    expect(dragSplit(606, 303)).toBeCloseTo(0.5, 5);
    expect(dragSplit(606, 100)).toBeCloseTo(97 / 600, 5);
    // Close to an edge but not on it: clamped, the split stays.
    expect(dragSplit(606, 60)).toBe(SPLIT_MIN);
    expect(dragSplit(606, 10)).toBeNull();
    expect(dragSplit(606, 600)).toBeNull();
  });

  it('panes start at the same place and then scroll and zoom independently', () => {
    const s = splitPanes({ x: 0, y: 420, zoom: 1.25 });
    expect(s.panes[0]).toEqual(s.panes[1]);
    expect(s.panes[0]).not.toBe(s.panes[1]);
    const moved = updatePane(s, 1, { y: 2000, zoom: 0.75 });
    expect(moved.panes[0]).toEqual({ x: 0, y: 420, zoom: 1.25 });
    expect(moved.panes[1]).toEqual({ x: 0, y: 2000, zoom: 0.75 });
    // The first split is untouched (no shared objects).
    expect(s.panes[1].y).toBe(420);
    // Removing the split keeps the view of the pane that has the keyboard.
    expect(unsplitPanes({ ...moved, active: 1 })).toEqual({ x: 0, y: 2000, zoom: 0.75 });
    expect(unsplitPanes(moved)).toEqual({ x: 0, y: 420, zoom: 1.25 });
  });
});

const metrics = (top: number, maxTop: number, left = 0, maxLeft = 0): ScrollMetrics => ({
  top,
  left,
  maxTop,
  maxLeft,
});

describe('synchronous scrolling', () => {
  it('maps positions proportionally between views of different lengths', () => {
    expect(scrollFraction(metrics(500, 2000))).toEqual({ fx: 0, fy: 0.25 });
    expect(scrollFraction(metrics(10, 0))).toEqual({ fx: 0, fy: 0 });
    expect(scrollFraction(metrics(3000, 2000)).fy).toBe(1);
    // A quarter of a 2 000 px document is a quarter of an 8 000 px one.
    expect(fractionToScroll({ fx: 0, fy: 0.25 }, { maxTop: 8000, maxLeft: 0 })).toEqual({
      top: 2000,
      left: 0,
    });
    expect(
      fractionToScroll({ fx: 0.5, fy: 0.9 }, { maxTop: 1000, maxLeft: 400 }, { fx: 0, fy: 0.2 }),
    ).toEqual({ top: 1000, left: 200 });
  });

  it('keeps the distance the views had when paired; reset drops it', () => {
    const a = new SyncScroll();
    const b = new SyncScroll();
    // A is at 10 %, B at 30 % when side by side starts.
    const ma = metrics(100, 1000);
    const mb = metrics(1200, 4000);
    a.pair(ma, scrollFraction(mb));
    b.pair(mb, scrollFraction(ma));
    // A scrolls to 40 %: B follows to 60 %.
    const sent = a.local(metrics(400, 1000));
    expect(sent).not.toBeNull();
    const to = b.remote(sent!, mb);
    expect(to.top).toBe(2400);
    // B's own scroll event for that position is not sent back.
    expect(b.local(metrics(2400, 4000))).toBeNull();
    // A real scroll of B afterwards is: to 80 % → A goes to 60 %.
    const back = b.local(metrics(3200, 4000));
    expect(a.remote(back!, metrics(400, 1000)).top).toBe(600);
    // Reset: both at the same relative place.
    const f = scrollFraction(metrics(600, 1000));
    expect(b.reset(metrics(3200, 4000), f).top).toBe(2400);
    a.reset(metrics(600, 1000), f);
    expect(b.remote(a.local(metrics(700, 1000))!, metrics(2400, 4000)).top).toBe(2800);
  });

  it('clamps at the ends', () => {
    const a = new SyncScroll();
    a.pair(metrics(900, 1000), { fx: 0, fy: 0.1 });
    expect(a.remote({ fx: 0, fy: 0.5 }, metrics(900, 1000)).top).toBe(1000);
    expect(a.remote({ fx: 0, fy: -2 }, metrics(900, 1000)).top).toBe(0);
  });
});

/** Buses of one channel: a message goes to every other bus. */
function hub() {
  const handlers = new Set<{ bus: MessageBus; fn: (m: unknown) => void }>();
  const bus = (): MessageBus => {
    const self: MessageBus = {
      post: (m) => {
        const copy: unknown = JSON.parse(JSON.stringify(m));
        for (const h of [...handlers]) if (h.bus !== self) h.fn(copy);
      },
      onMessage: (fn) => {
        const h = { bus: self, fn };
        handlers.add(h);
        return () => handlers.delete(h);
      },
      close: () => {
        for (const h of [...handlers]) if (h.bus === self) handlers.delete(h);
      },
    };
    return self;
  };
  return bus;
}

function scroller(max: number): ScrollTarget & { top: number; fire(): void } {
  const fns = new Set<() => void>();
  const t = {
    top: 0,
    metrics: () => ({ top: t.top, left: 0, maxTop: max, maxLeft: 0 }),
    scrollTo: (pos: { top: number; left: number }) => {
      t.top = pos.top;
      t.fire();
    },
    onScroll: (fn: () => void) => {
      fns.add(fn);
      return () => fns.delete(fn);
    },
    fire: () => fns.forEach((f) => f()),
  };
  return t;
}

describe('window link', () => {
  it('lists the other windows of the same app and pairs two of them', () => {
    const bus = hub();
    const sa = scroller(1000);
    const sb = scroller(3000);
    const a = new WindowLink(bus(), { id: 'a', app: 'doc', title: 'A.docx' }, () => sa);
    const b = new WindowLink(bus(), { id: 'b', app: 'doc', title: 'B.docx' }, () => sb);
    const c = new WindowLink(bus(), { id: 'c', app: 'sheet', title: 'C.xlsx' }, () => null);
    expect(a.peers().map((p) => p.title)).toEqual(['B.docx']);
    expect(b.peers().map((p) => p.id)).toEqual(['a']);
    expect(c.peers()).toEqual([]);

    const paired = vi.fn();
    b.on('paired', paired);
    a.pair('b');
    expect(a.pairedWith?.id).toBe('b');
    expect(b.pairedWith?.id).toBe('a');
    expect(paired).toHaveBeenCalledWith(expect.objectContaining({ id: 'a' }));

    // A scrolls half way: B follows proportionally, without echoing back.
    sa.top = 500;
    sa.fire();
    expect(sb.top).toBe(1500);
    expect(sa.top).toBe(500);
    // …and the other way round.
    sb.top = 3000;
    sb.fire();
    expect(sa.top).toBe(1000);

    // Synchronous scrolling off: A scrolls alone; on again keeps the new distance.
    a.setSynchronous(false);
    sa.top = 0;
    sa.fire();
    expect(sb.top).toBe(3000);
    a.setSynchronous(true);
    sa.top = 100;
    sa.fire();
    expect(sb.top).toBe(3000);
    // Reset position: both at the same relative place.
    a.resetPosition();
    expect(sb.top).toBe(300);
    sa.top = 400;
    sa.fire();
    expect(sb.top).toBe(1200);

    // Closing a window ends side by side in the other one.
    a.dispose();
    expect(b.pairedWith).toBeNull();
    expect(b.peers()).toEqual([]);
    expect(paired).toHaveBeenLastCalledWith(null);
    b.dispose();
    c.dispose();
  });

  it('renaming a document reaches the other windows', () => {
    const bus = hub();
    const a = new WindowLink(bus(), { id: 'a', app: 'slide', title: '' }, () => null);
    const b = new WindowLink(bus(), { id: 'b', app: 'slide', title: 'B' }, () => null);
    const peers = vi.fn();
    b.on('peers', peers);
    a.setTitle('Deck.pptx');
    expect(b.peers()[0]?.title).toBe('Deck.pptx');
    expect(peers).toHaveBeenCalled();
    a.dispose();
    b.dispose();
  });
});

function memoryCache(): OfflineCache {
  const map = new Map<string, Uint8Array>();
  return {
    get: async (k) => map.get(k) ?? null,
    put: async (k, v) => void map.set(k, v),
    delete: async (k) => void map.delete(k),
  };
}

describe('new window', () => {
  it('hands the document over and opens the session in both windows', async () => {
    const offline = memoryCache();
    const openNew = vi.fn(async () => {});
    const navigate = vi.fn();
    const id = await openDocumentWindow(
      { offline, window: { setTitle() {}, openNew } },
      { bytes: new Uint8Array([1, 2, 3]), name: 'Báo cáo.docx' },
      { location: { pathname: '/doc', search: '?lang=en-US' }, navigate, id: 'w1' },
    );
    expect(id).toBe('w1');
    const url = `/doc?lang=en-US&${WINDOW_PARAM}=w1`;
    expect(openNew).toHaveBeenCalledWith(url);
    expect(navigate).toHaveBeenCalledWith(url);
    expect(await takeWindowDocument(offline, 'w1')).toEqual({
      bytes: new Uint8Array([1, 2, 3]),
      name: 'Báo cáo.docx',
    });
    expect(await takeWindowDocument(offline, 'other')).toBeNull();
  });

  it('replaces an earlier session parameter', () => {
    expect(windowSessionUrl('n2', { pathname: '/sheet', search: '?window=n1' })).toBe(
      '/sheet?window=n2',
    );
  });
});
