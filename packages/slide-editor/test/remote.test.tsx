// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  FakeDictationProvider,
  FakeSocket,
  FakeTranslationProvider,
  type RemoteFetch,
  type RemoteState,
} from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import {
  CaptionDialog,
  CaptionOverlay,
  CaptionTrack,
  DEFAULT_CAPTIONS,
  useLiveCaptions,
  type CaptionSettings,
} from '../src/captions';
import { RemotePage } from '../src/RemotePage';
import {
  appBase,
  isLocalOnly,
  RemoteDialog,
  type RemotePresentingState,
} from '../src/remotePresenting';

afterEach(cleanup);

const withI18n = (ui: ReactNode) => (
  <I18nextProvider i18n={createI18n('vi-VN')}>{ui}</I18nextProvider>
);

function memoryStore() {
  const m = new Map<string, string>();
  return {
    m,
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => void m.set(k, v),
    removeItem: (k: string) => void m.delete(k),
  };
}

function claimServer(valid: string) {
  const calls: string[] = [];
  const f: RemoteFetch = async (url, init) => {
    calls.push(url);
    const ok = JSON.parse(String(init?.body)).token === valid;
    return {
      ok,
      status: ok ? 200 : 404,
      json: async () => (ok ? { key: 'PHONE-KEY' } : { code: 'invalidToken' }),
      arrayBuffer: async () => new ArrayBuffer(0),
    };
  };
  return { calls, fetch: f };
}

const state = (over: Partial<RemoteState> = {}): RemoteState => ({
  type: 'remote',
  showing: true,
  slide: 1,
  total: 5,
  at: 2,
  of: 5,
  click: 1,
  clicks: 3,
  black: false,
  white: false,
  ended: false,
  notes: 'Nhắc khán giả về lịch học',
  current: 'data:image/jpeg;base64,AAAA',
  next: 'data:image/jpeg;base64,BBBB',
  startedAt: 1_000,
  ...over,
});

describe('phone remote page', () => {
  it('pairs once, shows the presenter state and sends next / previous / black / end', async () => {
    const sockets: FakeSocket[] = [];
    const srv = claimServer('CODE');
    const store = memoryStore();
    const options = {
      baseUrl: 'http://srv',
      fetch: srv.fetch,
      socket: (u: string) => {
        const s = new FakeSocket(u);
        sockets.push(s);
        return s;
      },
    };
    render(
      <RemotePage
        token="CODE"
        options={options}
        store={store}
        now={() => 66_000}
        language="vi-VN"
      />,
    );
    expect(screen.getByTestId('remote-banner').textContent).toBe('Đang ghép nối…');
    await act(async () => {});
    expect(srv.calls).toEqual(['http://srv/api/present/remote/claim']);
    expect(store.m.get('bo.remote.CODE')).toBe('PHONE-KEY');
    expect(sockets[0]!.url).toBe('ws://srv/api/present/remote/ws?key=PHONE-KEY');
    act(() => {
      sockets[0]!.open();
      sockets[0]!.deliver({ type: 'peers', presenter: true });
      sockets[0]!.deliver(state());
    });
    expect(screen.queryByTestId('remote-banner')).toBeNull();
    expect(screen.getByTestId('remote-notes').textContent).toBe('Nhắc khán giả về lịch học');
    expect(screen.getByTestId('remote-position').textContent).toBe(
      'Trang chiếu 2 (2/5) · hiệu ứng 1/3',
    );
    expect(screen.getByTestId('remote-current').getAttribute('src')).toBe(
      'data:image/jpeg;base64,AAAA',
    );
    expect(screen.getByTestId('remote-next-thumb').getAttribute('src')).toBe(
      'data:image/jpeg;base64,BBBB',
    );
    expect(screen.getByTestId('remote-timer').textContent).toBe('01:05');
    fireEvent.click(screen.getByTestId('remote-next'));
    fireEvent.click(screen.getByTestId('remote-prev'));
    fireEvent.click(screen.getByTestId('remote-black'));
    fireEvent.click(screen.getByTestId('remote-end'));
    expect(sockets[0]!.sent).toEqual(
      ['next', 'prev', 'black', 'end'].map((action) => ({ type: 'command', action })),
    );
    act(() => sockets[0]!.deliver(state({ black: true })));
    expect(screen.getByTestId('remote-black').getAttribute('aria-pressed')).toBe('true');

    // The presenter away: buttons off until it is back.
    act(() => sockets[0]!.deliver({ type: 'peers', presenter: false }));
    expect(screen.getByTestId('remote-banner').textContent).toBe(
      'Máy trình chiếu đang mất kết nối.',
    );
    expect((screen.getByTestId('remote-next') as HTMLButtonElement).disabled).toBe(true);

    // Not showing: the phone offers to start.
    act(() => {
      sockets[0]!.deliver({ type: 'peers', presenter: true });
      sockets[0]!.deliver(state({ showing: false, slide: 0 }));
    });
    fireEvent.click(screen.getByTestId('remote-start'));
    expect(sockets[0]!.sent.at(-1)).toEqual({ type: 'command', action: 'start' });

    // Revoked: the controls go away.
    act(() => {
      sockets[0]!.deliver({ type: 'revoked' });
      sockets[0]!.drop(4401);
    });
    expect(screen.getByTestId('remote-banner').textContent).toBe(
      'Người trình bày đã thu hồi quyền điều khiển của điện thoại này.',
    );
    expect(screen.queryByTestId('remote-next')).toBeNull();
    expect(store.m.has('bo.remote.CODE')).toBe(false);
  });

  it('a reload reuses the key of the tab; a used code says so', async () => {
    const store = memoryStore();
    store.setItem('bo.remote.CODE', 'KEPT');
    const srv = claimServer('OTHER');
    const sockets: FakeSocket[] = [];
    const socket = (u: string) => {
      const s = new FakeSocket(u);
      sockets.push(s);
      return s;
    };
    const { unmount } = render(
      <RemotePage token="CODE" options={{ baseUrl: '', fetch: srv.fetch, socket }} store={store} />,
    );
    await act(async () => {});
    expect(srv.calls).toEqual([]);
    expect(sockets[0]!.url).toMatch(/\/api\/present\/remote\/ws\?key=KEPT$/);
    unmount();

    render(
      <RemotePage token="USED" options={{ baseUrl: '', fetch: srv.fetch, socket }} store={store} />,
    );
    await act(async () => {});
    expect(screen.getByTestId('remote-page').getAttribute('data-phase')).toBe('invalid');
    expect(screen.getByTestId('remote-banner').textContent).toMatch(/không hợp lệ/);
    expect(screen.queryByTestId('remote-next')).toBeNull();
  });
});

describe('pairing dialog', () => {
  const base: RemotePresentingState = {
    available: true,
    busy: false,
    error: null,
    pairing: null,
    paired: false,
    phone: false,
    online: null,
    viewers: 0,
    canBroadcast: true,
  };

  it('shows the QR code of the link until a phone pairs', () => {
    const url = 'http://192.168.1.20:8081/remote/Q2x5XbQh-1pk_8aW3rT0Nw';
    const { rerender } = render(
      withI18n(
        <RemoteDialog
          state={{ ...base, pairing: { url, expiresAt: Date.now() + 299_000 } }}
          onPair={() => {}}
          onRevoke={() => {}}
          onClose={() => {}}
        />,
      ),
    );
    const qr = screen.getByTestId('remote-qr');
    expect(qr.getAttribute('data-version')).toBe('4');
    expect(qr.querySelector('path')!.getAttribute('d')!.length).toBeGreaterThan(100);
    expect(screen.getByTestId('remote-url').textContent).toBe(url);
    expect(screen.getByTestId('remote-expiry').textContent).toMatch(/^Mã hết hạn sau 4:5\d$/);
    expect(screen.getByTestId('remote-status').textContent).toBe('Đang chờ điện thoại quét mã…');
    expect(screen.queryByTestId('remote-local-warning')).toBeNull();
    rerender(
      withI18n(
        <RemoteDialog
          state={{ ...base, pairing: { url, expiresAt: 0 }, paired: true, phone: true }}
          onPair={() => {}}
          onRevoke={() => {}}
          onClose={() => {}}
        />,
      ),
    );
    expect(screen.queryByTestId('remote-qr')).toBeNull();
    expect(screen.getByTestId('remote-status').textContent).toBe(
      'Điện thoại đã kết nối và đang điều khiển.',
    );
  });

  it('warns about addresses a phone cannot open and explains a server without it', () => {
    expect(isLocalOnly('http://localhost:5173/remote/x')).toBe(true);
    expect(isLocalOnly('http://10.1.2.3/remote/x')).toBe(false);
    expect(appBase('https://office.example.vn/bo/slide?x=1#a')).toBe(
      'https://office.example.vn/bo/',
    );
    expect(appBase('http://h:5173/edit')).toBe('http://h:5173/');
    render(
      withI18n(
        <RemoteDialog
          state={{ ...base, available: false }}
          onPair={() => {}}
          onRevoke={() => {}}
          onClose={() => {}}
        />,
      ),
    );
    expect(screen.getByTestId('remote-unavailable').textContent).toMatch(/chưa bật/);
    expect(screen.queryByTestId('remote-new-code')).toBeNull();
  });
});

function Captions({
  dictation,
  translation,
  initial,
}: {
  dictation: FakeDictationProvider | undefined;
  translation?: FakeTranslationProvider;
  initial: CaptionSettings;
}) {
  const [settings, setSettings] = useState(initial);
  const [open, setOpen] = useState(false);
  const c = useLiveCaptions({ dictation, translation, settings, active: settings.enabled });
  return (
    <>
      <button type="button" data-testid="open" onClick={() => setOpen(true)} />
      {settings.enabled && <CaptionOverlay captions={c} settings={settings} />}
      {open && (
        <CaptionDialog
          settings={settings}
          captions={c}
          onChange={setSettings}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}

describe('live captions', () => {
  it('keeps the last phrases that fit', () => {
    const tr = new CaptionTrack(30);
    tr.addFinal('Xin chào các bạn');
    tr.setInterim('hôm');
    expect(tr.interim).toBe('hôm');
    tr.addFinal('hôm nay chúng ta học bài mới');
    expect(tr.interim).toBe('');
    // "Xin chào các bạn hôm nay…" is 45 characters: the last 30 cut at the first space.
    expect(tr.text).toBe('hôm nay chúng ta học bài mới');
    expect(tr.text.length).toBeLessThanOrEqual(30);
  });

  it('shows interim and final speech at the chosen place and size, restarting after silence', async () => {
    const d = new FakeDictationProvider(['vi-VN']);
    render(
      withI18n(
        <Captions
          dictation={d}
          initial={{ ...DEFAULT_CAPTIONS, enabled: true, position: 'top', size: 'large' }}
        />,
      ),
    );
    await act(async () => {});
    expect(d.sessions).toHaveLength(1);
    expect(d.sessions[0]!.lang).toBe('vi-VN');
    act(() => void d.interim('xin chào'));
    expect(screen.getByTestId('captions-interim').textContent).toBe('xin chào');
    act(() => void d.final('Xin chào các bạn.'));
    expect(screen.getByTestId('captions-text').textContent).toBe('Xin chào các bạn.');
    expect(screen.queryByTestId('captions-interim')).toBeNull();
    const overlay = screen.getByTestId('show-captions');
    expect(overlay.getAttribute('data-position')).toBe('top');
    expect(overlay.getAttribute('data-size')).toBe('large');
    expect(overlay.getAttribute('aria-live')).toBe('polite');

    // The recogniser stops on its own: listening starts again.
    act(() => d.end());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 300));
    });
    expect(d.sessions).toHaveLength(2);
    act(() => void d.final('Bài hôm nay'));
    expect(screen.getByTestId('captions-text').textContent).toBe('Xin chào các bạn. Bài hôm nay');

    // A recogniser failure is shown instead of text.
    act(() => d.fail('noPermission'));
    expect(screen.getByTestId('captions-error').textContent).toMatch(/micrô/);
  });

  it('translates final phrases through the translation provider', async () => {
    const d = new FakeDictationProvider(['vi-VN']);
    const tr = new FakeTranslationProvider(['vi', 'en'], { 'Xin chào': 'Hello' });
    render(
      withI18n(
        <Captions
          dictation={d}
          translation={tr}
          initial={{ ...DEFAULT_CAPTIONS, enabled: true, translateTo: 'en' }}
        />,
      ),
    );
    await act(async () => {});
    act(() => void d.interim('Xin'));
    // Untranslated guesses are not shown when translating.
    expect(screen.queryByTestId('show-captions')).toBeNull();
    await act(async () => {
      d.final('Xin chào');
    });
    expect(tr.calls).toEqual([{ texts: ['Xin chào'], from: 'vi', to: 'en' }]);
    expect(screen.getByTestId('captions-text').textContent).toBe('Hello');
    await act(async () => {
      d.final('các bạn');
    });
    expect(screen.getByTestId('captions-text').textContent).toBe('Hello [en] các bạn');
  });

  it('is off, with the reason, without a recogniser; settings apply', async () => {
    render(
      withI18n(
        <Captions dictation={new FakeDictationProvider([], false)} initial={DEFAULT_CAPTIONS} />,
      ),
    );
    fireEvent.click(screen.getByTestId('open'));
    await act(async () => {});
    expect(screen.getByTestId('captions-unavailable').textContent).toMatch(/chưa có bộ nhận dạng/);
    expect((screen.getByTestId('captions-on') as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId('captions-apply') as HTMLButtonElement).disabled).toBe(true);
    cleanup();

    const d = new FakeDictationProvider(['vi-VN', 'en-US']);
    render(withI18n(<Captions dictation={d} initial={DEFAULT_CAPTIONS} />));
    fireEvent.click(screen.getByTestId('open'));
    await act(async () => {});
    expect(screen.getByTestId('captions-no-translation')).toBeTruthy();
    expect((screen.getByTestId('captions-translate') as HTMLSelectElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('captions-on'));
    fireEvent.change(screen.getByTestId('captions-lang'), { target: { value: 'en-US' } });
    fireEvent.change(screen.getByTestId('captions-size'), { target: { value: 'small' } });
    fireEvent.click(screen.getByTestId('captions-apply'));
    await act(async () => {});
    expect(d.sessions.at(-1)!.lang).toBe('en-US');
    act(() => void d.final('Good morning'));
    expect(screen.getByTestId('show-captions').getAttribute('data-size')).toBe('small');
    expect(screen.getByTestId('show-captions').getAttribute('data-position')).toBe('bottom');
  });
});
