/**
 * Digital signatures in the browser (ADR-0007). The private key never
 * reaches the page: the WASM engine computes the digest (two-phase
 * signing), the remote signing service signs it (generic JSON protocol,
 * `docs/deploy/remote-signing.md`), the optional RFC 3161 TSA time-stamps
 * the signature, then WASM embeds both. Only remote signers exist here.
 */
import type {
  SignatureReport,
  SignerKind,
  SigningDefaults,
  SigningService,
  SignRequest,
} from '../types';

/** The WASM side (implemented by `WebEngineHost` through its worker). */
export interface SignatureBackend {
  signPrepare(
    bytes: Uint8Array,
    certsPem: string,
    options: { timeSecs: number; name?: string; reason?: string; location?: string },
  ): Promise<{ id: number; digest: Uint8Array }>;
  signTimestampRequest(id: number, signature: Uint8Array, nonce: Uint8Array): Promise<Uint8Array>;
  signFinish(id: number, signature: Uint8Array, tsResponse?: Uint8Array): Promise<Uint8Array>;
  verifySignatures(
    bytes: Uint8Array,
    anchorsPem: string,
    nowSecs: number,
    revocation?: string[],
  ): Promise<SignatureReport>;
  /** CRL downloads / OCSP requests for the document's certificate paths. */
  revocationRequests?(
    bytes: Uint8Array,
    anchorsPem: string,
    nowSecs: number,
  ): Promise<RevocationRequest[]>;
}

/** `bo_sign::report::RevocationRequest`: what to fetch through the proxy. */
export interface RevocationRequest {
  kind: 'crl' | 'ocsp';
  url: string;
  /** OCSP: the DER request, base64. */
  body?: string;
}

/**
 * The server's revocation proxy (`POST /api/revocation/fetch`): browsers
 * cannot reach CRL distribution points / OCSP responders (CORS).
 */
export interface RevocationProxyOptions {
  url: string;
  /** Headers (e.g. `Authorization`) and editor session id for the request. */
  auth?: () => Promise<{ headers?: Record<string, string>; session?: string }>;
}

export interface WebSigningOptions {
  fetch?: typeof fetch;
  defaults?: SigningDefaults;
  /** PEM bundle of trust anchors (default `<base>trust/anchors.pem`). */
  anchorsUrl?: string;
  now?: () => number;
  random?: (n: number) => Uint8Array;
  sleep?: (ms: number) => Promise<void>;
  /** Maximum wait for a `pending` confirmation (ms). */
  maxWaitMs?: number;
  /** Check certificate revocation through this proxy when verifying. */
  revocationProxy?: RevocationProxyOptions;
}

export function toBase64(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

export function fromBase64(b64: string): Uint8Array {
  const s = atob(b64.trim());
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

/** PEM bundle of base64 DER certificates (RFC 7468 §5). */
export function certificatesToPem(certs: string[]): string {
  return certs
    .map((c) => {
      const lines = c.replace(/\s+/g, '').match(/.{1,64}/g) ?? [];
      return `-----BEGIN CERTIFICATE-----\n${lines.join('\n')}\n-----END CERTIFICATE-----\n`;
    })
    .join('');
}

interface SignResponse {
  status: 'signed' | 'pending' | 'rejected';
  signature?: string;
  transactionId?: string;
  retryAfterMs?: number;
  message?: string;
}

export class WebSigning implements SigningService {
  readonly kinds: SignerKind[] = ['remote'];
  private readonly fetch: typeof fetch;
  private readonly now: () => number;
  private readonly random: (n: number) => Uint8Array;
  private readonly sleep: (ms: number) => Promise<void>;
  private anchors: Promise<string> | null = null;

  constructor(
    private readonly backend: SignatureBackend,
    private readonly opts: WebSigningOptions = {},
  ) {
    this.fetch = opts.fetch ?? ((input, init) => globalThis.fetch(input, init));
    this.now = opts.now ?? (() => Date.now());
    this.random = opts.random ?? ((n) => crypto.getRandomValues(new Uint8Array(n)));
    this.sleep = opts.sleep ?? ((ms) => new Promise((r) => setTimeout(r, ms)));
  }

  async defaults(): Promise<SigningDefaults> {
    return this.opts.defaults ?? {};
  }

  private async post<T>(url: string, body: unknown, token?: string): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    };
    if (token) headers.Authorization = `Bearer ${token}`;
    const res = await this.fetch(url, { method: 'POST', headers, body: JSON.stringify(body) });
    if (!res.ok) {
      let detail = '';
      try {
        detail = ((await res.json()) as { message?: string }).message ?? '';
      } catch {
        // not JSON
      }
      throw new Error(`HTTP ${res.status}${detail ? `: ${detail}` : ''}`);
    }
    return (await res.json()) as T;
  }

  async sign(bytes: Uint8Array, req: SignRequest): Promise<Uint8Array> {
    const s = req.signer;
    if (s.kind !== 'remote') throw new Error('sign.remoteOnly');
    const base = s.url.replace(/\/+$/, '');
    const credentialId = s.credentialId || null;
    const { certificates } = await this.post<{ certificates: string[] }>(
      `${base}/certificates`,
      { credentialId },
      s.accessToken,
    );
    if (!certificates?.length) throw new Error('sign.noCertificate');
    const { id, digest } = await this.backend.signPrepare(bytes, certificatesToPem(certificates), {
      timeSecs: Math.floor(this.now() / 1000),
      name: req.name,
      reason: req.reason,
      location: req.location,
    });
    let resp = await this.post<SignResponse>(
      `${base}/sign`,
      {
        credentialId,
        hashAlgorithm: 'SHA-256',
        signatureAlgorithm: 'RSASSA-PKCS1-v1_5',
        digest: toBase64(digest),
        documentName: req.documentName,
      },
      s.accessToken,
    );
    const deadline = this.now() + (this.opts.maxWaitMs ?? 120_000);
    while (resp.status === 'pending') {
      if (!resp.transactionId) throw new Error('sign.badResponse');
      if (this.now() >= deadline) throw new Error('sign.timeout');
      await this.sleep(Math.min(Math.max(resp.retryAfterMs ?? 2000, 10), 10_000));
      resp = await this.post<SignResponse>(
        `${base}/sign/status`,
        { transactionId: resp.transactionId },
        s.accessToken,
      );
    }
    if (resp.status === 'rejected') throw new Error(resp.message || 'sign.rejected');
    if (resp.status !== 'signed' || !resp.signature) throw new Error('sign.badResponse');
    const signature = fromBase64(resp.signature);

    let tsResponse: Uint8Array | undefined;
    const tsa = req.tsaUrl?.trim();
    if (tsa) {
      const query = await this.backend.signTimestampRequest(id, signature, this.random(8));
      const res = await this.fetch(tsa, {
        method: 'POST',
        headers: { 'Content-Type': 'application/timestamp-query' },
        body: query as BodyInit,
      });
      if (!res.ok) throw new Error(`TSA HTTP ${res.status}`);
      tsResponse = new Uint8Array(await res.arrayBuffer());
    }
    return this.backend.signFinish(id, signature, tsResponse);
  }

  private trustAnchors(): Promise<string> {
    const url = this.opts.anchorsUrl;
    if (!url) return Promise.resolve('');
    this.anchors ??= this.fetch(url)
      .then((r) => (r.ok ? r.text() : ''))
      .catch(() => '');
    return this.anchors;
  }

  async verify(bytes: Uint8Array): Promise<SignatureReport> {
    const anchors = await this.trustAnchors();
    const now = Math.floor(this.now() / 1000);
    const revocation = await this.revocationData(bytes, anchors, now);
    return this.backend.verifySignatures(bytes, anchors, now, revocation);
  }

  /** CRLs / OCSP responses fetched through the proxy (failures are skipped). */
  private async revocationData(bytes: Uint8Array, anchors: string, now: number): Promise<string[]> {
    const proxy = this.opts.revocationProxy;
    if (!proxy || !this.backend.revocationRequests) return [];
    const requests = await this.backend.revocationRequests(bytes, anchors, now);
    if (requests.length === 0) return [];
    const auth = (await proxy.auth?.()) ?? {};
    const answers = await Promise.all(
      requests.map(async (r) => {
        try {
          const res = await this.fetch(proxy.url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', ...auth.headers },
            body: JSON.stringify({ ...r, session: auth.session }),
          });
          return res.ok ? toBase64(new Uint8Array(await res.arrayBuffer())) : null;
        } catch {
          return null;
        }
      }),
    );
    return answers.filter((a): a is string => a !== null);
  }
}
