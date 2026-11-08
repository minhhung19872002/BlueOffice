// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { UpdateService, UpdateStatus, UserInfo } from '@blueoffice/platform';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AccountChip, UpdatesSection, type AccountSession } from '../src';

afterEach(cleanup);

function service(answers: UpdateStatus[], install = vi.fn(async () => {})): UpdateService {
  const disabled: UpdateStatus = { state: 'disabled' };
  return { check: vi.fn(async (): Promise<UpdateStatus> => answers.shift() ?? disabled), install };
}

describe('Start screen → Updates', () => {
  it('checks, shows the new version and installs it', async () => {
    const install = vi.fn(async () => {});
    const s = service(
      [
        { state: 'upToDate', current: '0.1.0' },
        { state: 'available', current: '0.1.0', version: '0.2.0' },
      ],
      install,
    );
    render(<UpdatesSection language="vi-VN" updates={s} />);
    expect(screen.queryByTestId('settings-update-install')).toBeNull();
    fireEvent.click(screen.getByTestId('settings-update-check'));
    await waitFor(() =>
      expect(screen.getByTestId('settings-update-status').textContent).toBe(
        'Bạn đang dùng phiên bản mới nhất (0.1.0).',
      ),
    );
    fireEvent.click(screen.getByTestId('settings-update-check'));
    await waitFor(() =>
      expect(screen.getByTestId('settings-update-status').dataset.state).toBe('available'),
    );
    expect(screen.getByTestId('settings-update-status').textContent).toContain('0.2.0');
    fireEvent.click(screen.getByTestId('settings-update-install'));
    await waitFor(() => expect(install).toHaveBeenCalledOnce());
  });

  it('reports a failed install and an unconfigured build (English)', async () => {
    const s = service(
      [{ state: 'available', current: '0.1.0', version: '0.3.0' }, { state: 'notConfigured' }],
      vi.fn(async () => {
        throw new Error('bad signature');
      }),
    );
    render(<UpdatesSection language="en-US" updates={s} />);
    fireEvent.click(screen.getByTestId('settings-update-check'));
    fireEvent.click(await screen.findByTestId('settings-update-install'));
    await waitFor(() =>
      expect(screen.getByTestId('settings-update-status').textContent).toBe(
        'Could not install the update: bad signature',
      ),
    );
    fireEvent.click(screen.getByTestId('settings-update-check'));
    await waitFor(() =>
      expect(screen.getByTestId('settings-update-status').textContent).toBe(
        'This installation has no update server configured.',
      ),
    );
  });
});

describe('AccountChip', () => {
  it('signs in, then shows the user and signs out', async () => {
    let user: UserInfo | null = null;
    const listeners = new Set<(u: UserInfo | null) => void>();
    const auth: AccountSession = {
      get user() {
        return user;
      },
      onChange: (fn) => {
        listeners.add(fn);
        return () => listeners.delete(fn);
      },
      signIn: vi.fn(async () => {}),
      signOut: vi.fn(async () => {}),
    };
    render(<AccountChip auth={auth} language="vi-VN" returnTo="/doc" />);
    fireEvent.click(screen.getByTestId('account-sign-in'));
    expect(auth.signIn).toHaveBeenCalledWith('/doc');
    act(() => {
      user = { id: 'u7', name: 'Nguyễn Văn An' };
      listeners.forEach((f) => f(user));
    });
    expect(screen.getByTestId('account').textContent).toBe('Nguyễn Văn An');
    fireEvent.click(screen.getByTestId('account'));
    expect(auth.signOut).toHaveBeenCalled();
  });

  it('reports a sign-in that cannot start', async () => {
    const onError = vi.fn();
    const auth: AccountSession = {
      user: null,
      onChange: () => () => {},
      signIn: async () => {
        throw new Error('signIn.unsupported');
      },
      signOut: async () => {},
    };
    render(<AccountChip auth={auth} language="en-US" onError={onError} />);
    expect(screen.getByTestId('account-sign-in').textContent).toBe('Sign in');
    fireEvent.click(screen.getByTestId('account-sign-in'));
    await waitFor(() => expect(onError).toHaveBeenCalled());
  });
});
