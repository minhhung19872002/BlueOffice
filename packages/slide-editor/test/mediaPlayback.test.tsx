// @vitest-environment happy-dom
import { CommandRegistry } from '@blueoffice/editor';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  EngineHost,
  SlideAnimInfo,
  SlideCommand,
  SlideEditResult,
  SlideMediaInfo,
  SlideMediaPlayback,
} from '@blueoffice/platform';
import { MEDIA_ACROSS_SLIDES } from '@blueoffice/platform';
import { Ribbon } from '@blueoffice/ui';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  MEDIA_COMMANDS,
  MediaActionsHub,
  formatClock,
  registerMediaCommands,
  secondsToMs,
  useMediaTools,
  type MediaToolsHost,
} from '../src/mediaPlayback';
import { DEFAULT_PLAYBACK } from '../src/showMedia';

afterEach(cleanup);

const i18n = createI18n('vi-VN');

const playback = (p: Partial<SlideMediaPlayback> = {}): SlideMediaPlayback => ({
  ...DEFAULT_PLAYBACK,
  bookmarks: [{ name: 'Mốc 1', timeMs: 3000 }],
  captions: [{ id: '{A}', label: 'Tiếng Việt', lang: 'vi-VN', part: '/ppt/media/track1.vtt' }],
  start: 'seq',
  playClick: 1,
  ...p,
});

const clipOf = (kind: 'video' | 'audio', p?: Partial<SlideMediaPlayback>): SlideMediaInfo => ({
  id: 4,
  name: 'clip4.mp4',
  kind,
  rect: [0, 0, 320, 180],
  part: null,
  mime: null,
  link: null,
  playback: playback(p),
});

const anims = [
  { index: 0, shapeId: 4, shapeName: 'clip4.mp4' },
  { index: 1, shapeId: 20, shapeName: 'Ghi chú' },
] as SlideAnimInfo[];

function Harness(props: MediaToolsHost & { tab?: string }) {
  const { tabs, dialogs } = useMediaTools(props);
  return (
    <>
      <Ribbon tabs={tabs} value={props.tab ?? tabs[0]?.id} />
      {dialogs}
    </>
  );
}

function setup(clip: SlideMediaInfo | null, extra: Partial<MediaToolsHost> = {}, tab?: string) {
  const sent: SlideCommand[] = [];
  const run = vi.fn(async (c: SlideCommand) => {
    sent.push(c);
    return {} as SlideEditResult;
  });
  const hub = new MediaActionsHub();
  const host: MediaToolsHost = {
    engine: {} as EngineHost,
    clip,
    animations: anims,
    run,
    pickFile: async () => null,
    hub,
    ...extra,
  };
  const r = render(
    <I18nextProvider i18n={i18n}>
      <Harness {...host} tab={tab} />
    </I18nextProvider>,
  );
  return { sent, hub, host, ...r };
}

const cmd = (id: string) => {
  const el = document.querySelector(`[data-command="${id}"]`);
  if (!el) throw new Error(`no ${id}`);
  return el as HTMLElement;
};

describe('media playback tab', () => {
  it('shows the Playback and Video format tabs for a video, only Playback for audio', () => {
    setup(clipOf('video'));
    expect(screen.getAllByRole('tab').map((t) => t.textContent)).toEqual([
      'Phát lại',
      'Định dạng video',
    ]);
    cleanup();
    setup(clipOf('audio'));
    expect(screen.getAllByRole('tab').map((t) => t.textContent)).toEqual(['Phát lại']);
    // No full-screen option for a sound.
    expect(document.querySelector('[data-command="media.fullScreen"]')).toBeNull();
    cleanup();
    const { hub } = setup(null);
    expect(screen.queryAllByRole('tab')).toEqual([]);
    expect(hub.get()).toBeNull();
  });

  it('toggles options, sets start, volume and fades through mediaPlayback', async () => {
    const { sent } = setup(clipOf('video', { loop: true, slides: 1 }));
    expect(cmd('media.loop').getAttribute('aria-pressed')).toBe('true');
    expect(cmd('media.rewind').getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(cmd('media.loop'));
    fireEvent.click(cmd('media.acrossSlides'));
    fireEvent.click(cmd('media.hide'));
    fireEvent.click(cmd('media.fullScreen'));
    fireEvent.change(screen.getByTestId('media-start'), { target: { value: 'auto' } });
    fireEvent.change(screen.getByTestId('media-volume'), { target: { value: '25' } });
    const fade = screen.getByTestId('media-fade-in');
    fireEvent.change(fade, { target: { value: '1.5' } });
    fireEvent.blur(fade);
    await waitFor(() => expect(sent).toHaveLength(7));
    expect(sent).toEqual([
      { cmd: 'mediaPlayback', id: 4, loop: false },
      { cmd: 'mediaPlayback', id: 4, slides: MEDIA_ACROSS_SLIDES },
      { cmd: 'mediaPlayback', id: 4, hideWhenStopped: true },
      { cmd: 'mediaPlayback', id: 4, fullScreen: true },
      { cmd: 'mediaPlayback', id: 4, start: 'auto' },
      { cmd: 'mediaPlayback', id: 4, volume: 25 },
      { cmd: 'mediaPlayback', id: 4, fadeInMs: 1500 },
    ]);
  });

  it('runs every media.* command through the registry', async () => {
    const { sent, hub } = setup(clipOf('video'));
    const reg = new CommandRegistry<object>();
    registerMediaCommands(reg, hub);
    for (const id of MEDIA_COMMANDS) expect(reg.get(id)).toBeTruthy();
    expect(reg.isEnabled('media.mute', {})).toBe(true);
    await act(async () => {
      await reg.execute('media.mute', {});
      await reg.execute('media.start.click', {});
      await reg.execute('media.volume.0', {});
    });
    expect(sent).toEqual([
      { cmd: 'mediaPlayback', id: 4, mute: true },
      { cmd: 'mediaPlayback', id: 4, start: 'click' },
      { cmd: 'mediaPlayback', id: 4, volume: 0 },
    ]);
  });

  it('manages bookmarks and the effects they start', async () => {
    const { sent } = setup(clipOf('video'));
    fireEvent.click(cmd('media.bookmarks'));
    const dialog = await screen.findByTestId('media-bookmarks-dialog');
    // Not playable here: times are typed.
    expect(screen.getByTestId('media-no-preview')).toBeTruthy();
    expect(screen.getAllByTestId('media-bookmark')).toHaveLength(1);
    fireEvent.change(screen.getByTestId('media-bookmark-name'), { target: { value: 'Cao trào' } });
    fireEvent.change(screen.getByTestId('media-bookmark-time'), { target: { value: '12.5' } });
    fireEvent.click(screen.getByTestId('media-bookmark-add'));
    // The clip's own effect is not offered as a target.
    const select = screen.getByTestId('media-bookmark-trigger') as HTMLSelectElement;
    expect(Array.from(select.options).map((o) => o.value)).toEqual(['', '1']);
    fireEvent.change(select, { target: { value: '1' } });
    fireEvent.click(screen.getByTestId('media-bookmark-remove'));
    await waitFor(() => expect(sent).toHaveLength(3));
    expect(sent).toEqual([
      { cmd: 'mediaBookmarkAdd', id: 4, timeMs: 12_500, name: 'Cao trào' },
      { cmd: 'mediaBookmarkTrigger', index: 1, media: 4, bookmark: 'Mốc 1' },
      { cmd: 'mediaBookmarkRemove', id: 4, name: 'Mốc 1' },
    ]);
    expect(dialog).toBeTruthy();
  });

  it('trims with typed times when the clip cannot be previewed', async () => {
    const { sent } = setup(clipOf('video', { trimStartMs: 500, trimEndMs: 1000 }));
    fireEvent.click(cmd('media.trim'));
    await screen.findByTestId('media-trim-dialog');
    expect((screen.getByTestId('media-trim-start') as HTMLInputElement).value).toBe('0.5');
    // The length is unknown: the end stays as it is.
    expect((screen.getByTestId('media-trim-end') as HTMLInputElement).disabled).toBe(true);
    fireEvent.change(screen.getByTestId('media-trim-start'), { target: { value: '2.25' } });
    fireEvent.click(screen.getByTestId('media-trim-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({ cmd: 'mediaPlayback', id: 4, trimStartMs: 2250, trimEndMs: 1000 });
  });

  it('inserts WebVTT captions, refuses other files, removes tracks', async () => {
    const vtt = 'WEBVTT\n\n00:01.000 --> 00:02.000\nMột\n\n00:03.000 --> 00:04.000\nHai\n';
    let file = { bytes: new TextEncoder().encode(vtt), name: 'phu-de.vtt' };
    const { sent } = setup(clipOf('video'), { pickFile: async () => file });
    fireEvent.click(cmd('media.captionsInsert'));
    await screen.findByTestId('media-caption-add-dialog');
    expect(screen.getByTestId('media-caption-cues').textContent).toBe(
      '2 câu phụ đề trong tệp phu-de.vtt',
    );
    expect((screen.getByTestId('media-caption-label') as HTMLInputElement).value).toBe('phu-de');
    fireEvent.change(screen.getByTestId('media-caption-lang'), { target: { value: 'vi-VN' } });
    fireEvent.click(screen.getByTestId('media-caption-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'mediaCaptionAdd',
      id: 4,
      text: vtt,
      label: 'phu-de',
      lang: 'vi-VN',
    });
    file = { bytes: new TextEncoder().encode('1\n00:00:01,000 --> x'), name: 'a.srt' };
    fireEvent.click(cmd('media.captionsInsert'));
    expect((await screen.findByTestId('media-error')).textContent).toBe(
      'Tệp này không phải phụ đề WebVTT (.vtt).',
    );
    fireEvent.click(cmd('media.captionsManage'));
    await screen.findByTestId('media-captions-dialog');
    fireEvent.click(screen.getByTestId('media-caption-remove'));
    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[1]).toEqual({ cmd: 'mediaCaptionRemove', id: 4, track: '{A}' });
  });

  it('sets the poster frame from a picture file', async () => {
    const png = new Uint8Array([137, 80, 78, 71]);
    const { sent } = setup(
      clipOf('video'),
      { pickFile: async () => ({ bytes: png, name: 'a.png' }) },
      'mediaFormat',
    );
    expect(cmd('media.posterFrame')).toBeTruthy();
    fireEvent.click(cmd('media.posterFile'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({ cmd: 'pictureChange', data: 'iVBORw==' });
  });
});

describe('time helpers', () => {
  it('formats and reads times', () => {
    expect(formatClock(0)).toBe('0:00.0');
    expect(formatClock(62_500)).toBe('1:02.5');
    expect(formatClock(3_599_950)).toBe('60:00.0');
    expect(secondsToMs('1,5')).toBe(1500);
    expect(secondsToMs('-1')).toBeNull();
    expect(secondsToMs('x')).toBeNull();
  });
});
