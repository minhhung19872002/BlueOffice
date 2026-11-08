// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Platform } from '@blueoffice/platform';
import { DirtyState, HostedChrome, HostedClosed, trackDirty, wantsChrome } from '../src';

afterEach(cleanup);

class Engine {
  calls: string[] = [];
  async command(c: { cmd: string }) {
    this.calls.push(c.cmd);
    return c.cmd === 'bad' ? { error: 'x' } : { cmd: c.cmd };
  }
  async slideCommand(c: { cmd: string }) {
    return { cmd: c.cmd };
  }
  async insertImage() {
    return {};
  }
  async slideInsertPicture() {
    return {};
  }
}

class Sheet {
  async setInputs() {}
  async undo() {
    return false;
  }
  async redo() {
    return true;
  }
  async exec(c: { cmd: string }) {
    return c.cmd;
  }
}

describe('dirty tracking (SDK `dirty`)', () => {
  it('local document changes make the session dirty; reads, failures and setAuthor do not', async () => {
    const state = new DirtyState();
    const seen: boolean[] = [];
    state.subscribe((d) => seen.push(d));
    const p = trackDirty(
      { engine: new Engine(), sheet: new Sheet() } as unknown as Platform,
      state,
    );
    await p.engine.command({ cmd: 'setAuthor', name: 'An' });
    await p.engine.command({ cmd: 'state' });
    await p.engine.command({ cmd: 'move', motion: 'left' } as never);
    await p.engine.command({ cmd: 'bad' } as never);
    await p.engine.slideCommand({ cmd: 'goTo', slide: 1 });
    expect(state.dirty).toBe(false);
    await p.engine.command({ cmd: 'insertText', text: 'a' });
    expect(state.dirty).toBe(true);
    state.set(false); // saved
    await p.sheet!.undo(); // nothing undone
    await p.sheet!.exec({ cmd: 'find' } as never);
    expect(state.dirty).toBe(false);
    await p.sheet!.redo();
    expect(state.dirty).toBe(true);
    state.set(false);
    await p.sheet!.setInputs(0, [[0, 0, '1']]);
    await p.engine.slideCommand({ cmd: 'delete' } as never);
    expect(seen).toEqual([true, false, true, false, true]);
  });
});

describe('host chrome', () => {
  it('shows only what the host asked for and reports the buttons', () => {
    expect(wantsChrome({})).toBe(false);
    expect(wantsChrome({ brandName: 'BlueDrive' })).toBe(true);
    const dirty = new DirtyState();
    const onShare = vi.fn();
    const onClose = vi.fn();
    const onBack = vi.fn();
    render(
      <HostedChrome
        ui={{ brandName: 'BlueDrive', showShare: true, showClose: true }}
        appName="BlueDoc"
        fileName="Báo cáo.docx"
        dirty={dirty}
        language="vi-VN"
        theme="dark"
        onShare={onShare}
        onClose={onClose}
        onBack={onBack}
      />,
    );
    expect(screen.getByTestId('hosted-brand').textContent).toBe('BlueDrive · BlueDoc');
    expect(screen.getByTestId('hosted-chrome').getAttribute('data-theme')).toBe('dark');
    expect(screen.queryByTestId('hosted-back')).toBeNull();
    expect(screen.queryByTestId('hosted-unsaved')).toBeNull();
    fireEvent.click(screen.getByTestId('hosted-share'));
    fireEvent.click(screen.getByTestId('hosted-close'));
    expect(onShare).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.getByText('Chia sẻ')).toBeTruthy();
  });

  it('marks unsaved changes and offers Back; the closed screen follows the theme', async () => {
    const dirty = new DirtyState();
    const onBack = vi.fn();
    render(
      <HostedChrome
        ui={{ showBack: true }}
        appName="BlueSheet"
        fileName="Số liệu.xlsx"
        dirty={dirty}
        language="en-US"
        theme="light"
        onBack={onBack}
      />,
    );
    dirty.set(true);
    expect(await screen.findByTestId('hosted-unsaved')).toBeTruthy();
    fireEvent.click(screen.getByLabelText('Back'));
    expect(onBack).toHaveBeenCalled();
    cleanup();
    render(<HostedClosed language="vi-VN" theme="dark" />);
    expect(screen.getByTestId('hosted-closed').getAttribute('data-theme')).toBe('dark');
    expect(screen.getByText('Đã đóng tài liệu')).toBeTruthy();
  });
});
