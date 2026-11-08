/**
 * Logic of the "Ký số" (digital signature) dialog and of the signature
 * status shown for an opened document (ADR-0007). Kept free of React so
 * it is unit-tested directly.
 */
import type {
  ChainSummary,
  SignatureReport,
  SignatureSummary,
  SignerKind,
  SigningDefaults,
  SignRequest,
  TokenCertificate,
} from '@blueoffice/platform';

/** What gets signed: the document itself (.docx) or an exported PDF. */
export type SignOutput = 'docx' | 'pdf';

export interface SignForm {
  kind: SignerKind;
  output: SignOutput;
  /** Certificate file (.p12 / .pfx / .pem) chosen by the user. */
  file: { name: string; base64: string } | null;
  password: string;
  /** PKCS#11 module path. */
  module: string;
  tokenCerts: TokenCertificate[];
  /** `<slot>:<id>` of the chosen token certificate. */
  tokenCert: string;
  pin: string;
  remoteUrl: string;
  credentialId: string;
  accessToken: string;
  tsaUrl: string;
  reason: string;
  location: string;
}

/** Non-secret choices remembered between sessions (never PIN/password/token). */
export interface RememberedSigning {
  kind?: SignerKind;
  module?: string;
  remoteUrl?: string;
  credentialId?: string;
  tsaUrl?: string;
  location?: string;
}

export const SIGNING_SETTING_KEY = 'signing';

export function initialForm(
  kinds: SignerKind[],
  defaults: SigningDefaults,
  output: SignOutput,
  remembered: RememberedSigning = {},
): SignForm {
  const kind =
    remembered.kind && kinds.includes(remembered.kind) ? remembered.kind : (kinds[0] ?? 'remote');
  return {
    kind,
    output,
    file: null,
    password: '',
    module: remembered.module ?? defaults.pkcs11Module ?? '',
    tokenCerts: [],
    tokenCert: '',
    pin: '',
    remoteUrl: remembered.remoteUrl ?? defaults.remoteUrl ?? '',
    credentialId: remembered.credentialId ?? '',
    accessToken: '',
    tsaUrl: remembered.tsaUrl ?? defaults.tsaUrl ?? '',
    reason: '',
    location: remembered.location ?? '',
  };
}

export function tokenCertKey(c: TokenCertificate): string {
  return `${c.slot}:${c.id}`;
}

const isHttpUrl = (u: string) => /^https?:\/\/[^\s/]+/i.test(u.trim());

/** i18n key of the first missing/invalid field, or null when the form can be submitted. */
export function validate(form: SignForm): string | null {
  switch (form.kind) {
    case 'file':
      if (!form.file) return 'sign.needFile';
      if (/\.(p12|pfx)$/i.test(form.file.name) && !form.password) return 'sign.needPassword';
      break;
    case 'token':
      if (!form.module.trim()) return 'sign.needModule';
      if (!form.tokenCert) return 'sign.needTokenCert';
      if (!form.pin) return 'sign.needPin';
      break;
    case 'remote':
      if (!isHttpUrl(form.remoteUrl)) return 'sign.needRemoteUrl';
      break;
  }
  if (form.tsaUrl.trim() && !isHttpUrl(form.tsaUrl)) return 'sign.badTsaUrl';
  return null;
}

/** The platform request for a valid form. */
export function toRequest(form: SignForm, documentName: string): SignRequest {
  const opt = (s: string) => (s.trim() ? s.trim() : undefined);
  let signer: SignRequest['signer'];
  switch (form.kind) {
    case 'file':
      signer = {
        kind: 'file',
        fileName: form.file?.name ?? '',
        fileBase64: form.file?.base64 ?? '',
        password: form.password || undefined,
      };
      break;
    case 'token': {
      const c = form.tokenCerts.find((x) => tokenCertKey(x) === form.tokenCert);
      signer = {
        kind: 'token',
        module: form.module.trim(),
        slot: c?.slot,
        id: c?.id,
        pin: form.pin,
      };
      break;
    }
    case 'remote':
      signer = {
        kind: 'remote',
        url: form.remoteUrl.trim(),
        credentialId: opt(form.credentialId),
        accessToken: opt(form.accessToken),
      };
      break;
  }
  return {
    signer,
    tsaUrl: opt(form.tsaUrl),
    reason: form.output === 'pdf' ? opt(form.reason) : undefined,
    location: form.output === 'pdf' ? opt(form.location) : undefined,
    documentName,
  };
}

export function remember(form: SignForm): RememberedSigning {
  return {
    kind: form.kind,
    module: form.module.trim() || undefined,
    remoteUrl: form.remoteUrl.trim() || undefined,
    credentialId: form.credentialId.trim() || undefined,
    tsaUrl: form.tsaUrl.trim() || undefined,
    location: form.location.trim() || undefined,
  };
}

export function parseRemembered(json: string | null): RememberedSigning {
  if (!json) return {};
  try {
    const v = JSON.parse(json) as unknown;
    return v && typeof v === 'object' ? (v as RememberedSigning) : {};
  } catch {
    return {};
  }
}

/** `hop-dong.docx` → `hop-dong-signed.docx` / `hop-dong-signed.pdf`. */
export function signedFileName(name: string, output: SignOutput): string {
  const stem = name.replace(/\.[^./\\]+$/, '') || 'document';
  return `${stem}-signed.${output}`;
}

/** Base64 of file bytes (for the certificate file). */
export function bytesToBase64(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

export type SignatureState = 'trusted' | 'untrusted' | 'changed';
export type DocumentSignatureState = 'none' | 'invalid' | SignatureState;

/** One signature: trusted, valid but untrusted, or followed by later changes. */
export function signatureState(s: SignatureSummary): SignatureState {
  if (!s.coversWholeDocument) return 'changed';
  const tsOk = !s.timestamp || s.timestamp.chain.trusted;
  return s.chain.trusted && tsOk ? 'trusted' : 'untrusted';
}

/** The worst state of a document's signatures. */
export function documentState(r: SignatureReport | null): DocumentSignatureState {
  if (!r) return 'none';
  if (r.error) return 'invalid';
  if (r.signatures.length === 0) return 'none';
  const states = r.signatures.map(signatureState);
  if (states.includes('changed')) return 'changed';
  if (states.includes('untrusted')) return 'untrusted';
  return 'trusted';
}

/** i18n keys (`sign.problem.<code>`) + subject for the problems of a chain. */
export function problemKeys(c: ChainSummary): { key: string; subject: string | null }[] {
  return c.problems.map((p) => ({ key: `sign.problem.${p.code}`, subject: p.subject }));
}

/** `CN=Nguyễn Văn A,O=…` → `Nguyễn Văn A`. */
export function commonName(subject: string): string {
  const m = /(?:^|,)\s*CN=((?:\\,|[^,])+)/.exec(subject);
  return m ? m[1]!.replace(/\\,/g, ',').trim() : subject;
}

/**
 * Whether a file looks signed, so the editor verifies it on open: an OOXML
 * package with a `_xmlsignatures/` part (ZIP names are stored in clear) or a
 * PDF with a `/ByteRange`.
 */
export function looksSigned(bytes: Uint8Array): boolean {
  const head = String.fromCharCode(...bytes.subarray(0, 4));
  const needle = head.startsWith('%PDF')
    ? '/ByteRange'
    : head.startsWith('PK')
      ? '_xmlsignatures/'
      : '';
  if (!needle) return false;
  const codes = Array.from(needle, (c) => c.charCodeAt(0));
  outer: for (let i = 0; i + codes.length <= bytes.length; i++) {
    for (let j = 0; j < codes.length; j++) if (bytes[i + j] !== codes[j]) continue outer;
    return true;
  }
  return false;
}
