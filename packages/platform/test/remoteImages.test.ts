import { describe, expect, it, vi } from 'vitest';
import { FetchRemoteImages, MAX_REMOTE_IMAGE_BYTES } from '../src/remoteImages';

const res = (ok: boolean, size: number) => ({
  ok,
  arrayBuffer: async () => new ArrayBuffer(size),
});

describe('FetchRemoteImages', () => {
  it('fetches https pictures without credentials or redirects', async () => {
    const fetchFn = vi.fn(async () => res(true, 3));
    const r = await new FetchRemoteImages(fetchFn).fetchImage('https://a.vn/x.png');
    expect(r).toEqual({ ok: true, bytes: new Uint8Array(3) });
    expect(fetchFn).toHaveBeenCalledWith('https://a.vn/x.png', {
      credentials: 'omit',
      redirect: 'error',
      mode: 'cors',
    });
  });

  it('blocks other schemes and reports connection problems', async () => {
    const fetchFn = vi.fn(async () => res(true, 3));
    const images = new FetchRemoteImages(fetchFn);
    expect(await images.fetchImage('http://a.vn/x.png')).toEqual({ ok: false, error: 'blocked' });
    expect(fetchFn).not.toHaveBeenCalled();
    expect(
      await new FetchRemoteImages(async () => res(false, 3)).fetchImage('https://a.vn'),
    ).toEqual({
      ok: false,
      error: 'connect',
    });
    const thrown = new FetchRemoteImages(async () => {
      throw new TypeError('network');
    });
    expect(await thrown.fetchImage('https://a.vn')).toEqual({ ok: false, error: 'connect' });
    const huge = new FetchRemoteImages(async () => res(true, MAX_REMOTE_IMAGE_BYTES + 1));
    expect(await huge.fetchImage('https://a.vn')).toEqual({ ok: false, error: 'connect' });
  });
});
