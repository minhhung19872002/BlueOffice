// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SignatureReport, SignatureSummary, TokenCertificate } from '@blueoffice/platform';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SignaturesPanel, SignDialog } from '../src/SignaturePanels';
import {
  commonName,
  documentState,
  initialForm,
  looksSigned,
  parseRemembered,
  problemKeys,
  remember,
  signatureState,
  signedFileName,
  toRequest,
  validate,
} from '../src/signing';

afterEach(cleanup);

const chain = (trusted: boolean, codes: string[] = []) => ({
  trusted,
  path: ['CN=Nguyễn Văn A,O=Test,C=VN'],
  anchor: trusted ? 'CN=Root CA,C=VN' : null,
  problems: codes.map((code) => ({ code, subject: 'CN=Issuing CA,C=VN', message: code })),
});

const sig = (over: Partial<SignatureSummary> = {}): SignatureSummary => ({
  signer: 'CN=Nguyễn Văn A,O=Test,C=VN',
  signingTime: null,
  coversWholeDocument: true,
  timestamp: null,
  chain: chain(true),
  validatedAt: '2026-11-08T00:00:00Z',
  ...over,
});

const tokenCert: TokenCertificate = {
  slot: 1,
  tokenLabel: 'Token A',
  tokenSerial: '0001',
  label: 'cert',
  id: 'ab01',
  subject: 'CN=Nguyễn Văn A,O=Test,C=VN',
  issuer: 'CN=CA',
  notAfter: '2027-01-01T00:00:00Z',
};

describe('signing form logic', () => {
  it('starts from remembered choices and deployment defaults', () => {
    const f = initialForm(
      ['file', 'token', 'remote'],
      { tsaUrl: 'http://tsa', pkcs11Module: 'p11.dll' },
      'pdf',
      {
        kind: 'remote',
        remoteUrl: 'https://sign.example.vn',
      },
    );
    expect(f.kind).toBe('remote');
    expect(f.remoteUrl).toBe('https://sign.example.vn');
    expect(f.tsaUrl).toBe('http://tsa');
    expect(f.module).toBe('p11.dll');
    // A remembered kind the platform does not offer falls back to the first one.
    expect(initialForm(['remote'], {}, 'docx', { kind: 'token' }).kind).toBe('remote');
  });

  it('validates every signer kind', () => {
    const base = initialForm(['file', 'token', 'remote'], {}, 'docx');
    expect(validate({ ...base, kind: 'file' })).toBe('sign.needFile');
    const p12 = { ...base, kind: 'file' as const, file: { name: 'me.p12', base64: 'AA==' } };
    expect(validate(p12)).toBe('sign.needPassword');
    expect(validate({ ...p12, password: 'x' })).toBeNull();
    expect(validate({ ...p12, file: { name: 'me.pem', base64: 'AA==' } })).toBeNull();
    expect(validate({ ...base, kind: 'token' })).toBe('sign.needModule');
    expect(validate({ ...base, kind: 'token', module: 'a.dll' })).toBe('sign.needTokenCert');
    expect(validate({ ...base, kind: 'token', module: 'a.dll', tokenCert: '1:ab01' })).toBe(
      'sign.needPin',
    );
    expect(validate({ ...base, kind: 'remote', remoteUrl: 'ftp://x' })).toBe('sign.needRemoteUrl');
    const remote = { ...base, kind: 'remote' as const, remoteUrl: 'https://s' };
    expect(validate(remote)).toBeNull();
    expect(validate({ ...remote, tsaUrl: 'tsa.example' })).toBe('sign.badTsaUrl');
  });

  it('builds the platform request without leaking empty fields', () => {
    const base = initialForm(['file', 'token', 'remote'], {}, 'pdf');
    const token = toRequest(
      {
        ...base,
        kind: 'token',
        module: ' a.dll ',
        tokenCerts: [tokenCert],
        tokenCert: '1:ab01',
        pin: '1234',
        reason: 'Duyệt',
      },
      'x-signed.pdf',
    );
    expect(token).toEqual({
      signer: { kind: 'token', module: 'a.dll', slot: 1, id: 'ab01', pin: '1234' },
      tsaUrl: undefined,
      reason: 'Duyệt',
      location: undefined,
      documentName: 'x-signed.pdf',
    });
    const remote = toRequest(
      {
        ...base,
        output: 'docx',
        kind: 'remote',
        remoteUrl: 'https://s',
        tsaUrl: 'http://t',
        reason: 'ignored',
      },
      'a.docx',
    );
    expect(remote.signer).toEqual({
      kind: 'remote',
      url: 'https://s',
      credentialId: undefined,
      accessToken: undefined,
    });
    expect(remote.tsaUrl).toBe('http://t');
    expect(remote.reason).toBeUndefined();
  });

  it('remembers choices but never secrets', () => {
    const f = {
      ...initialForm(['remote'], {}, 'docx'),
      remoteUrl: 'https://s',
      accessToken: 'secret',
      pin: '1234',
      password: 'pw',
    };
    const r = remember(f);
    expect(JSON.stringify(r)).not.toMatch(/secret|1234|pw/);
    expect(parseRemembered(JSON.stringify(r)).remoteUrl).toBe('https://s');
    expect(parseRemembered('not json')).toEqual({});
    expect(parseRemembered(null)).toEqual({});
  });

  it('names the signed copy and detects signed files', () => {
    expect(signedFileName('Hợp đồng.docx', 'pdf')).toBe('Hợp đồng-signed.pdf');
    expect(signedFileName('a.b.docx', 'docx')).toBe('a.b-signed.docx');
    const enc = new TextEncoder();
    expect(looksSigned(enc.encode('PK\x03\x04....._xmlsignatures/sig1.xml'))).toBe(true);
    expect(looksSigned(enc.encode('PK\x03\x04 word/document.xml'))).toBe(false);
    expect(looksSigned(enc.encode('%PDF-1.7 /ByteRange [0 1 2 3]'))).toBe(true);
    expect(looksSigned(enc.encode('plain text'))).toBe(false);
  });

  it('summarises verification reports', () => {
    expect(documentState(null)).toBe('none');
    expect(documentState({ kind: 'pdf', signatures: [], error: null })).toBe('none');
    expect(documentState({ kind: 'pdf', signatures: [sig()], error: 'digest mismatch' })).toBe(
      'invalid',
    );
    expect(documentState({ kind: 'pdf', signatures: [sig()], error: null })).toBe('trusted');
    const untrusted = sig({ chain: chain(false, ['noTrustAnchors']) });
    expect(signatureState(untrusted)).toBe('untrusted');
    const badTsa = sig({
      timestamp: { time: 't', tsa: 'CN=TSA', chain: chain(false, ['expired']) },
    });
    expect(signatureState(badTsa)).toBe('untrusted');
    const changed = sig({ coversWholeDocument: false });
    expect(
      documentState({ kind: 'pdf', signatures: [sig(), untrusted, changed], error: null }),
    ).toBe('changed');
    expect(problemKeys(untrusted.chain)).toEqual([
      { key: 'sign.problem.noTrustAnchors', subject: 'CN=Issuing CA,C=VN' },
    ]);
    expect(commonName('CN=Nguyễn\\, Văn A,O=X')).toBe('Nguyễn, Văn A');
    expect(commonName('O=X')).toBe('O=X');
  });
});

function withI18n(ui: React.ReactElement) {
  return render(<I18nextProvider i18n={createI18n('en-US')}>{ui}</I18nextProvider>);
}

describe('SignDialog', () => {
  it('signs with a token after reading its certificates', async () => {
    const onSign = vi.fn();
    const list = vi.fn(async () => [tokenCert]);
    withI18n(
      <SignDialog
        kinds={['file', 'token', 'remote']}
        defaults={{ pkcs11Module: 'C:/p11.dll' }}
        output="pdf"
        documentName="a-signed.pdf"
        pickFile={async () => null}
        listTokenCertificates={list}
        onSign={onSign}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId('sign-kind-token'));
    expect(screen.getByTestId('sign-submit')).toHaveProperty('disabled', true);
    fireEvent.click(screen.getByTestId('sign-load-token'));
    await waitFor(() => expect(screen.getByTestId('sign-token-cert')).toBeTruthy());
    expect(list).toHaveBeenCalledWith('C:/p11.dll');
    fireEvent.change(screen.getByTestId('sign-pin'), { target: { value: '0000' } });
    fireEvent.click(screen.getByTestId('sign-submit'));
    expect(onSign).toHaveBeenCalledTimes(1);
    expect(onSign.mock.calls[0]![0].signer).toEqual({
      kind: 'token',
      module: 'C:/p11.dll',
      slot: 1,
      id: 'ab01',
      pin: '0000',
    });
  });

  it('offers only remote signing on the web and loads a certificate file elsewhere', async () => {
    const onSign = vi.fn();
    const { unmount } = withI18n(
      <SignDialog
        kinds={['remote']}
        defaults={{ remoteUrl: 'https://sign.example.vn' }}
        output="docx"
        documentName="a-signed.docx"
        pickFile={async () => null}
        onSign={onSign}
        onClose={() => {}}
      />,
    );
    expect(screen.queryByTestId('sign-kind-file')).toBeNull();
    expect(screen.queryByTestId('sign-reason')).toBeNull();
    fireEvent.click(screen.getByTestId('sign-submit'));
    expect(onSign.mock.calls[0]![0].signer.url).toBe('https://sign.example.vn');
    unmount();

    const onFile = vi.fn();
    withI18n(
      <SignDialog
        kinds={['file', 'remote']}
        defaults={{}}
        output="docx"
        documentName="a-signed.docx"
        pickFile={async () => ({ name: 'me.p12', bytes: new Uint8Array([1, 2, 3]) })}
        onSign={onFile}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId('sign-pick-file'));
    await waitFor(() => expect(screen.getByTestId('sign-file-name').textContent).toBe('me.p12'));
    expect(screen.getByTestId('sign-problem').textContent).toMatch(/password/i);
    fireEvent.change(screen.getByTestId('sign-password'), { target: { value: 'pw' } });
    fireEvent.click(screen.getByTestId('sign-submit'));
    expect(onFile.mock.calls[0]![0].signer).toEqual({
      kind: 'file',
      fileName: 'me.p12',
      fileBase64: 'AQID',
      password: 'pw',
    });
  });
});

describe('SignaturesPanel', () => {
  it('shows trust state, time-stamp and problems', () => {
    const report: SignatureReport = {
      kind: 'ooxml',
      error: null,
      signatures: [
        sig({
          timestamp: { time: '2026-11-08T01:02:03Z', tsa: 'CN=Test TSA', chain: chain(true) },
        }),
        sig({ chain: chain(false, ['unknownIssuer']) }),
      ],
    };
    withI18n(<SignaturesPanel report={report} onClose={() => {}} />);
    expect(screen.getByTestId('signatures-state').dataset.state).toBe('untrusted');
    const items = screen.getAllByTestId('signature-item');
    expect(items[0]!.dataset.state).toBe('trusted');
    expect(items[0]!.textContent).toContain('Test TSA');
    expect(items[1]!.textContent).toContain('Issuing CA');
  });
});
