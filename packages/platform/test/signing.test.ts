import { describe, expect, it, vi } from 'vitest';
import { createDesktopPlatform, type Invoke } from '../src/desktop';
import {
  certificatesToPem,
  fromBase64,
  toBase64,
  WebSigning,
  type SignatureBackend,
} from '../src/web/signing';
import type { SignatureReport } from '../src/types';

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

function backend(): SignatureBackend & { calls: string[] } {
  const calls: string[] = [];
  return {
    calls,
    signPrepare: vi.fn(async (_bytes, certsPem: string, options) => {
      calls.push(`prepare:${certsPem.split('BEGIN CERTIFICATE').length - 1}:${options.timeSecs}`);
      return { id: 7, digest: new Uint8Array(32).fill(9) };
    }),
    signTimestampRequest: vi.fn(async (id, _sig, nonce) => {
      calls.push(`tsq:${id}:${nonce.length}`);
      return new Uint8Array([0x30, 0x00]);
    }),
    signFinish: vi.fn(async (id, sig, ts) => {
      calls.push(`finish:${id}:${sig.length}:${ts ? ts.length : 0}`);
      return new Uint8Array([1, 2, 3]);
    }),
    verifySignatures: vi.fn(async (_b, anchors: string, now: number) => {
      calls.push(`verify:${anchors.length > 0}:${now}`);
      return { kind: 'pdf', signatures: [], error: null } satisfies SignatureReport;
    }),
  };
}

describe('WebSigning (remote signing in the browser)', () => {
  it('runs certificates → prepare → sign (with pending polls) → time-stamp → finish', async () => {
    const b = backend();
    const requests: { url: string; body: unknown; auth?: string }[] = [];
    let polls = 0;
    const fetch = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
      const u = String(url);
      const headers = (init?.headers ?? {}) as Record<string, string>;
      if (u === 'http://tsa.test/') {
        expect(headers['Content-Type']).toBe('application/timestamp-query');
        return new Response(new Uint8Array([0x30, 0x03, 2, 1, 0]), { status: 200 });
      }
      const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
      requests.push({ url: u, body, auth: headers.Authorization });
      if (u.endsWith('/certificates')) return json(200, { certificates: ['AAEC', 'AwQF'] });
      if (u.endsWith('/sign')) {
        expect(body.digest).toBe(toBase64(new Uint8Array(32).fill(9)));
        expect(body.hashAlgorithm).toBe('SHA-256');
        return json(200, { status: 'pending', transactionId: 'tx', retryAfterMs: 5 });
      }
      if (u.endsWith('/sign/status')) {
        polls++;
        return polls < 2
          ? json(200, { status: 'pending', transactionId: 'tx', retryAfterMs: 5 })
          : json(200, { status: 'signed', signature: toBase64(new Uint8Array(128)) });
      }
      return json(404, { message: 'nope' });
    });
    const sleep = vi.fn(async () => {});
    const s = new WebSigning(b, {
      fetch: fetch as unknown as typeof globalThis.fetch,
      now: () => 1_793_000_000_000,
      random: (n) => new Uint8Array(n),
      sleep,
    });
    expect(s.kinds).toEqual(['remote']);
    const out = await s.sign(new Uint8Array([9]), {
      signer: { kind: 'remote', url: 'https://sign.test/api/', accessToken: 'tok' },
      tsaUrl: 'http://tsa.test/',
      documentName: 'a-signed.pdf',
    });
    expect(out).toEqual(new Uint8Array([1, 2, 3]));
    expect(requests.map((r) => r.url)).toEqual([
      'https://sign.test/api/certificates',
      'https://sign.test/api/sign',
      'https://sign.test/api/sign/status',
      'https://sign.test/api/sign/status',
    ]);
    expect(requests.every((r) => r.auth === 'Bearer tok')).toBe(true);
    expect(sleep).toHaveBeenCalledTimes(2);
    expect(b.calls).toEqual(['prepare:2:1793000000', 'tsq:7:8', 'finish:7:128:5']);
  });

  it('reports rejections, HTTP errors and non-remote signers', async () => {
    const b = backend();
    const mk = (resp: Response) =>
      new WebSigning(b, {
        fetch: (async (url: RequestInfo | URL) =>
          String(url).endsWith('/certificates')
            ? json(200, { certificates: ['AAEC'] })
            : resp.clone()) as typeof globalThis.fetch,
      });
    const req = { signer: { kind: 'remote' as const, url: 'https://s' }, documentName: 'a' };
    await expect(
      mk(json(200, { status: 'rejected', message: 'Người dùng từ chối' })).sign(
        new Uint8Array(),
        req,
      ),
    ).rejects.toThrow('Người dùng từ chối');
    await expect(
      mk(json(401, { message: 'bad token' })).sign(new Uint8Array(), req),
    ).rejects.toThrow('HTTP 401: bad token');
    await expect(
      mk(json(200, {})).sign(new Uint8Array(), {
        ...req,
        signer: { kind: 'token', module: 'x', pin: '1' },
      }),
    ).rejects.toThrow('sign.remoteOnly');
  });

  it('verifies with the trust anchors fetched once', async () => {
    const b = backend();
    const fetch = vi.fn(
      async () => new Response('-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----\n'),
    );
    const s = new WebSigning(b, {
      fetch: fetch as unknown as typeof globalThis.fetch,
      anchorsUrl: '/trust/anchors.pem',
      now: () => 5_000,
    });
    await s.verify(new Uint8Array([1]));
    await s.verify(new Uint8Array([1]));
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(b.calls).toEqual(['verify:true:5', 'verify:true:5']);
  });

  it('encodes base64 and PEM', () => {
    const bytes = new Uint8Array([0, 1, 2, 250, 255]);
    expect(fromBase64(toBase64(bytes))).toEqual(bytes);
    const pem = certificatesToPem(['A'.repeat(70)]);
    expect(pem.split('\n')[1]).toHaveLength(64);
    expect(pem).toMatch(/^-----BEGIN CERTIFICATE-----\n/);
  });
});

describe('DesktopSigning', () => {
  it('routes to the Tauri sign commands', async () => {
    const calls: { cmd: string; args: unknown; options: unknown }[] = [];
    const fake = (async (cmd: string, args?: unknown, options?: unknown) => {
      calls.push({ cmd, args, options });
      if (cmd === 'sign_document') return new Uint8Array([7]).buffer;
      if (cmd === 'verify_document') return '{"kind":"pdf","signatures":[],"error":null}';
      if (cmd === 'sign_defaults') return { tsaUrl: 'http://tsa' };
      return [];
    }) as unknown as Invoke;
    const p = createDesktopPlatform({ invoke: fake });
    const signing = p.signing!;
    expect(signing.kinds).toEqual(['file', 'token', 'remote']);
    expect(await signing.defaults()).toEqual({ tsaUrl: 'http://tsa' });
    const request = {
      signer: { kind: 'token' as const, module: 'C:/p11.dll', pin: '1' },
      documentName: 'Hợp đồng.pdf',
    };
    expect(await signing.sign(new Uint8Array([1]), request)).toEqual(new Uint8Array([7]));
    const sign = calls.find((c) => c.cmd === 'sign_document')!;
    expect(sign.args).toBeInstanceOf(Uint8Array);
    const header = (sign.options as { headers: Record<string, string> }).headers['x-sign-request']!;
    expect(JSON.parse(decodeURIComponent(header))).toEqual(request);
    expect((await signing.verify(new Uint8Array([1]))).kind).toBe('pdf');
    await signing.listTokenCertificates!('C:/p11.dll');
    expect(calls.at(-1)).toMatchObject({
      cmd: 'sign_token_certificates',
      args: { module: 'C:/p11.dll' },
    });
  });
});
