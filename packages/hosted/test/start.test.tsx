// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useState } from 'react';
import {
  StartScreen,
  SyncPrompt,
  normalizeServerUrl,
  type DriveSession,
  type FetchLike,
  type StartScreenProps,
} from '../src';

afterEach(cleanup);

const listing = (
  folder: { id: string; name: string; parentId: string | null },
  items: unknown[],
) => ({
  folder,
  items,
});

const fetchImpl: FetchLike = async (url, init) => {
  const auth = init?.headers?.authorization;
  const json = (() => {
    if (url.endsWith('/api/hosts'))
      return {
        hosts: [
          {
            name: 'bluedrive',
            displayName: 'BlueDrive',
            protocol: 'bohp',
            browse: true,
            signIn: {
              authorizeUrl: 'https://sso/a',
              tokenUrl: 'https://sso/t',
              clientId: 'c',
              scope: 'openid',
            },
          },
        ],
      };
    if (auth !== 'Bearer good') return null;
    if (url.endsWith('/me')) return { id: 'u2', name: 'Trần Thị Bình' };
    if (url.endsWith('/folders/root/children'))
      return listing({ id: 'root', name: 'BlueDrive', parentId: null }, [
        { kind: 'folder', id: 'd1', name: 'Dự án' },
        { kind: 'file', id: 'f1', name: 'Ghi chú.docx' },
      ]);
    if (url.endsWith('/folders/d1/children'))
      return listing({ id: 'd1', name: 'Dự án', parentId: 'root' }, [
        { kind: 'file', id: 'f2', name: 'Kế hoạch.xlsx', lastModified: '2026-11-08T01:00:00Z' },
      ]);
    if (url.includes('/recent'))
      return { items: [{ kind: 'file', id: 'f2', name: 'Kế hoạch.xlsx' }] };
    if (url.endsWith('/shared'))
      return { items: [{ kind: 'file', id: 'f3', name: 'Chia sẻ.pptx' }] };
    return null;
  })();
  const status = json === null ? 401 : 200;
  return {
    ok: status === 200,
    status,
    json: async () => json,
    arrayBuffer: async () => new ArrayBuffer(0),
  };
};

function Harness(props: Partial<StartScreenProps>) {
  const [drive, setDrive] = useState<DriveSession | null>(null);
  return (
    <StartScreen
      language="vi-VN"
      fetchImpl={fetchImpl}
      serverUrl="https://office"
      onServerUrlChange={() => {}}
      syncFolders={[]}
      onSyncFoldersChange={() => {}}
      recent={[]}
      onNew={() => {}}
      onOpenLocal={() => {}}
      onOpenRecent={() => {}}
      drive={drive}
      onDriveChange={setDrive}
      onOpenDriveFile={() => {}}
      {...props}
    />
  );
}

describe('Start screen', () => {
  it('signs in with a token, browses folders, recent and shared files, and opens a file', async () => {
    const onOpenDriveFile = vi.fn();
    render(<Harness onOpenDriveFile={onOpenDriveFile} tab="drive" />);
    const token = await screen.findByTestId('drive-token');
    fireEvent.change(token, { target: { value: 'bad' } });
    fireEvent.click(screen.getByTestId('drive-signin-token'));
    expect((await screen.findByTestId('drive-error')).textContent).toContain(
      'Đăng nhập không thành công',
    );

    fireEvent.change(token, { target: { value: 'good' } });
    fireEvent.click(screen.getByTestId('drive-signin-token'));
    expect((await screen.findByTestId('drive-user')).textContent).toBe(
      'Đã đăng nhập: Trần Thị Bình',
    );
    fireEvent.click(await screen.findByText('Dự án'));
    expect((await screen.findByTestId('drive-folder')).textContent).toBe('Dự án');
    fireEvent.click(await screen.findByText('Kế hoạch.xlsx'));
    expect(onOpenDriveFile).toHaveBeenCalledWith(
      'bluedrive',
      expect.objectContaining({ id: 'f2' }),
      'good',
    );
    fireEvent.click(screen.getByTestId('drive-up'));
    await screen.findByText('Ghi chú.docx');

    fireEvent.click(screen.getByTestId('start-tab-drive-recent'));
    await waitFor(() =>
      expect(screen.getByTestId('drive-list-recent').textContent).toContain('Kế hoạch.xlsx'),
    );
    fireEvent.click(screen.getByTestId('start-tab-drive-shared'));
    await waitFor(() =>
      expect(screen.getByTestId('drive-list-shared').textContent).toContain('Chia sẻ.pptx'),
    );

    fireEvent.click(screen.getByText('Đăng xuất'));
    await screen.findByTestId('drive-token');
  });

  it('signs in through the browser when the host has SSO', async () => {
    const sso = vi.fn(async () => 'good');
    render(<Harness tab="drive" signInWithBrowser={sso} />);
    fireEvent.click(await screen.findByTestId('drive-signin-sso'));
    expect((await screen.findByTestId('drive-user')).textContent).toContain('Trần Thị Bình');
    expect(sso).toHaveBeenCalledWith(expect.objectContaining({ name: 'bluedrive' }));
  });

  it('asks for a server first, then saves the server and the sync folders', async () => {
    const onServerUrlChange = vi.fn();
    const onSyncFoldersChange = vi.fn();
    render(
      <Harness
        serverUrl={null}
        onServerUrlChange={onServerUrlChange}
        onSyncFoldersChange={onSyncFoldersChange}
      />,
    );
    fireEvent.click(screen.getByTestId('start-tab-drive'));
    expect(screen.getByTestId('drive-no-server')).toBeTruthy();
    fireEvent.click(screen.getByTestId('start-tab-settings'));
    fireEvent.change(screen.getByTestId('settings-server'), { target: { value: 'ftp://x' } });
    fireEvent.click(screen.getByTestId('settings-server-save'));
    expect(screen.getByTestId('settings-note').textContent).toContain('không hợp lệ');
    fireEvent.change(screen.getByTestId('settings-server'), {
      target: { value: ' https://office.congty.vn/ ' },
    });
    fireEvent.click(screen.getByTestId('settings-server-save'));
    await waitFor(() => expect(onServerUrlChange).toHaveBeenCalledWith('https://office.congty.vn'));
    fireEvent.change(screen.getByTestId('settings-sync-path'), {
      target: { value: 'C:\\BlueDrive' },
    });
    fireEvent.click(screen.getByTestId('settings-sync-add'));
    expect(onSyncFoldersChange).toHaveBeenCalledWith([
      { host: 'bluedrive', path: 'C:\\BlueDrive' },
    ]);
  });

  it('offers new documents and recent local files', () => {
    const onNew = vi.fn();
    const onOpenRecent = vi.fn();
    const recent = [{ name: 'Tờ trình.docx', path: 'D:\\Tờ trình.docx', openedAt: 1 }];
    render(<Harness onNew={onNew} onOpenRecent={onOpenRecent} recent={recent} />);
    fireEvent.click(screen.getByTestId('start-new-sheet'));
    expect(onNew).toHaveBeenCalledWith('sheet');
    fireEvent.click(screen.getByText(/Tờ trình\.docx/));
    expect(onOpenRecent).toHaveBeenCalledWith(recent[0]);
  });

  it('asks before switching a sync-folder file to online collaboration', () => {
    const onOnline = vi.fn();
    const onLocal = vi.fn();
    render(
      <SyncPrompt language="en-US" name="Kế hoạch.docx" onOnline={onOnline} onLocal={onLocal} />,
    );
    expect(screen.getByRole('dialog').textContent).toContain(
      '“Kế hoạch.docx” is in a BlueDrive sync folder',
    );
    fireEvent.click(screen.getByTestId('sync-prompt-online'));
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onOnline).toHaveBeenCalledOnce();
    expect(onLocal).toHaveBeenCalledOnce();
  });

  it('normalises server addresses like the desktop shell', () => {
    expect(normalizeServerUrl(' http://localhost:4583/ ')).toBe('http://localhost:4583');
    expect(normalizeServerUrl('https://x.vn/office')).toBe('https://x.vn/office');
    expect(normalizeServerUrl('https://user@x.vn')).toBeNull();
    expect(normalizeServerUrl('https://x.vn/?a=1')).toBeNull();
    expect(normalizeServerUrl('javascript:alert(1)')).toBeNull();
  });
});
