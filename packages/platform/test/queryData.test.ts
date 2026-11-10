import { describe, expect, it, vi } from 'vitest';
import { createBrowserQueryData, isWebUrl } from '../src/queryData';

describe('Get & Transform sources in the browser', () => {
  it('has no web pages unless a proxy is configured', () => {
    const pickFolder = vi.fn(async () => null);
    const d = createBrowserQueryData({ pickFolder });
    expect(d.fetchWeb).toBeUndefined();
    expect(d.pickFolder).toBe(pickFolder);
    expect(createBrowserQueryData({ proxyBaseUrl: null }).fetchWeb).toBeUndefined();
  });

  it('fetches pages through the server proxy and maps its answers', async () => {
    const fetch = vi.fn(
      async (_url: string, _init?: RequestInit) => new Response(new Uint8Array([60, 116])),
    );
    const d = createBrowserQueryData({
      proxyBaseUrl: 'https://office.example/',
      fetch,
      headers: async () => ({ Authorization: 'Bearer x' }),
    });
    expect(await d.fetchWeb!('https://example.com/prices')).toEqual(new Uint8Array([60, 116]));
    const [url, init] = fetch.mock.calls[0]!;
    expect(url).toBe('https://office.example/api/data/web');
    expect(init?.method).toBe('POST');
    expect(JSON.parse(init?.body as string)).toEqual({ url: 'https://example.com/prices' });
    expect((init?.headers as Record<string, string>).Authorization).toBe('Bearer x');
    fetch.mockResolvedValueOnce(new Response(null, { status: 404 }));
    await expect(d.fetchWeb!('https://example.com/')).rejects.toThrow('unavailable');
    fetch.mockResolvedValueOnce(new Response(null, { status: 413 }));
    await expect(d.fetchWeb!('https://example.com/')).rejects.toThrow('tooLarge');
    fetch.mockResolvedValueOnce(new Response(null, { status: 502 }));
    await expect(d.fetchWeb!('https://example.com/')).rejects.toThrow('status');
    fetch.mockRejectedValueOnce(new TypeError('offline'));
    await expect(d.fetchWeb!('https://example.com/')).rejects.toThrow('network');
    // Only http(s) URLs without credentials leave the page.
    await expect(d.fetchWeb!('file:///etc/passwd')).rejects.toThrow('network');
    expect(fetch).toHaveBeenCalledTimes(5);
    expect(isWebUrl('https://a.b/c')).toBe(true);
    expect(isWebUrl('http://u:p@a.b/')).toBe(false);
    expect(isWebUrl('javascript:alert(1)')).toBe(false);
  });
});
