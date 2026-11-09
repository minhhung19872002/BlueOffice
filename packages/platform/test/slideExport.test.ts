import { describe, expect, it, vi } from 'vitest';
import { NativeEngineHost, type Invoke } from '../src/desktop';

describe('exportSlides bridge', () => {
  it('sends the options as JSON to the native engine and returns the bytes', async () => {
    const fake = vi.fn(async (cmd: string) =>
      cmd === 'engine_slide_export' ? new TextEncoder().encode('%PDF-1.7').buffer : null,
    ) as unknown as Invoke;
    const host = new NativeEngineHost(fake);
    const bytes = await host.exportSlides({ kind: 'handout', perPage: 3, paper: 'letter' });
    expect(new TextDecoder().decode(bytes)).toBe('%PDF-1.7');
    expect(fake).toHaveBeenCalledWith('engine_slide_export', {
      json: '{"kind":"handout","perPage":3,"paper":"letter"}',
    });
  });
});
