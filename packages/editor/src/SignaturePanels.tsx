import { useTranslation } from '@blueoffice/i18n';
import type {
  SignatureReport,
  SignerKind,
  SigningDefaults,
  SignRequest,
  TokenCertificate,
} from '@blueoffice/platform';
import { useState } from 'react';
import styles from './Editor.module.css';
import {
  bytesToBase64,
  commonName,
  documentState,
  initialForm,
  problemKeys,
  type RememberedSigning,
  type SignForm,
  type SignOutput,
  signatureState,
  toRequest,
  tokenCertKey,
  validate,
} from './signing';

const panelStyle = { left: '50%', right: 'auto', top: 80, transform: 'translateX(-50%)' } as const;

/** "Ký số": choose the signer (certificate file, USB token, remote service) and sign. */
export function SignDialog({
  kinds,
  defaults,
  remembered,
  output,
  documentName,
  pickFile,
  listTokenCertificates,
  onSign,
  onClose,
}: {
  kinds: SignerKind[];
  defaults: SigningDefaults;
  remembered?: RememberedSigning;
  output: SignOutput;
  documentName: string;
  pickFile: () => Promise<{ name: string; bytes: Uint8Array } | null>;
  listTokenCertificates?: (module: string) => Promise<TokenCertificate[]>;
  onSign: (request: SignRequest, form: SignForm) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [form, setForm] = useState<SignForm>(() =>
    initialForm(kinds, defaults, output, remembered),
  );
  const [tokenError, setTokenError] = useState<string | null>(null);
  const set = (patch: Partial<SignForm>) => setForm((f) => ({ ...f, ...patch }));
  const problem = validate(form);

  const loadToken = async () => {
    if (!listTokenCertificates || !form.module.trim()) return;
    setTokenError(null);
    try {
      const certs = await listTokenCertificates(form.module.trim());
      set({ tokenCerts: certs, tokenCert: certs[0] ? tokenCertKey(certs[0]) : '' });
      if (!certs.length) setTokenError(t('sign.noTokenCerts'));
    } catch (e) {
      setTokenError(e instanceof Error ? e.message : String(e));
    }
  };

  const submit = () => {
    if (problem) return;
    onSign(toRequest(form, documentName), form);
  };

  return (
    <div
      className={styles.panel}
      role="dialog"
      aria-modal="true"
      aria-label={t(output === 'pdf' ? 'sign.titlePdf' : 'sign.title')}
      data-testid="sign-dialog"
      style={panelStyle}
      onKeyDown={(e) => {
        if (e.key === 'Escape') onClose();
      }}
    >
      <strong>{t(output === 'pdf' ? 'sign.titlePdf' : 'sign.title')}</strong>
      <div className={styles.panelRow} role="radiogroup" aria-label={t('sign.signer')}>
        {kinds.map((k) => (
          <label key={k}>
            <input
              type="radio"
              name="signer-kind"
              data-testid={`sign-kind-${k}`}
              checked={form.kind === k}
              onChange={() => set({ kind: k })}
            />{' '}
            {t(`sign.kind.${k}`)}
          </label>
        ))}
      </div>

      {form.kind === 'file' && (
        <>
          <div className={styles.panelRow}>
            <button
              type="button"
              data-testid="sign-pick-file"
              onClick={() =>
                void pickFile().then((f) => {
                  if (f) set({ file: { name: f.name, base64: bytesToBase64(f.bytes) } });
                })
              }
            >
              {t('sign.chooseFile')}
            </button>
            <span data-testid="sign-file-name">{form.file?.name ?? t('sign.noFile')}</span>
          </div>
          <div className={styles.panelRow}>
            <input
              type="password"
              data-testid="sign-password"
              aria-label={t('sign.password')}
              placeholder={t('sign.password')}
              value={form.password}
              onChange={(e) => set({ password: e.target.value })}
            />
          </div>
        </>
      )}

      {form.kind === 'token' && (
        <>
          <div className={styles.panelRow}>
            <input
              data-testid="sign-module"
              aria-label={t('sign.module')}
              placeholder={t('sign.modulePlaceholder')}
              value={form.module}
              onChange={(e) => set({ module: e.target.value, tokenCerts: [], tokenCert: '' })}
            />
            <button type="button" data-testid="sign-load-token" onClick={() => void loadToken()}>
              {t('sign.readToken')}
            </button>
          </div>
          {form.tokenCerts.length > 0 && (
            <div className={styles.panelRow}>
              <select
                data-testid="sign-token-cert"
                aria-label={t('sign.certificate')}
                value={form.tokenCert}
                onChange={(e) => set({ tokenCert: e.target.value })}
              >
                {form.tokenCerts.map((c) => (
                  <option key={tokenCertKey(c)} value={tokenCertKey(c)}>
                    {commonName(c.subject)} — {c.tokenLabel} (
                    {t('sign.until', { date: c.notAfter.slice(0, 10) })})
                  </option>
                ))}
              </select>
            </div>
          )}
          {tokenError && (
            <div className={styles.panelMessage} role="alert">
              {tokenError}
            </div>
          )}
          <div className={styles.panelRow}>
            <input
              type="password"
              data-testid="sign-pin"
              aria-label={t('sign.pin')}
              placeholder={t('sign.pin')}
              value={form.pin}
              onChange={(e) => set({ pin: e.target.value })}
            />
          </div>
        </>
      )}

      {form.kind === 'remote' && (
        <>
          <div className={styles.panelRow}>
            <input
              data-testid="sign-remote-url"
              aria-label={t('sign.remoteUrl')}
              placeholder={t('sign.remoteUrl')}
              value={form.remoteUrl}
              onChange={(e) => set({ remoteUrl: e.target.value })}
            />
          </div>
          <div className={styles.panelRow}>
            <input
              data-testid="sign-credential"
              aria-label={t('sign.credentialId')}
              placeholder={t('sign.credentialId')}
              value={form.credentialId}
              onChange={(e) => set({ credentialId: e.target.value })}
            />
            <input
              type="password"
              data-testid="sign-access-token"
              aria-label={t('sign.accessToken')}
              placeholder={t('sign.accessToken')}
              value={form.accessToken}
              onChange={(e) => set({ accessToken: e.target.value })}
            />
          </div>
          <div className={styles.panelMessage}>{t('sign.remoteHint')}</div>
        </>
      )}

      <div className={styles.panelRow}>
        <input
          data-testid="sign-tsa"
          aria-label={t('sign.tsaUrl')}
          placeholder={t('sign.tsaUrl')}
          value={form.tsaUrl}
          onChange={(e) => set({ tsaUrl: e.target.value })}
        />
      </div>
      {output === 'pdf' && (
        <div className={styles.panelRow}>
          <input
            data-testid="sign-reason"
            aria-label={t('sign.reason')}
            placeholder={t('sign.reason')}
            value={form.reason}
            onChange={(e) => set({ reason: e.target.value })}
          />
          <input
            data-testid="sign-location"
            aria-label={t('sign.location')}
            placeholder={t('sign.location')}
            value={form.location}
            onChange={(e) => set({ location: e.target.value })}
          />
        </div>
      )}
      {problem && (
        <div className={styles.panelMessage} data-testid="sign-problem">
          {t(problem)}
        </div>
      )}
      <div className={styles.panelRow}>
        <button type="button" data-testid="sign-submit" disabled={!!problem} onClick={submit}>
          {t('sign.sign')}
        </button>
        <button type="button" onClick={onClose}>
          {t('sign.cancel')}
        </button>
      </div>
    </div>
  );
}

/** Verification status of the opened document's signatures. */
export function SignaturesPanel({
  report,
  onClose,
}: {
  report: SignatureReport | null;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const state = documentState(report);
  return (
    <div
      className={styles.panel}
      role="dialog"
      aria-label={t('sign.statusTitle')}
      data-testid="signatures-panel"
    >
      <strong>{t('sign.statusTitle')}</strong>
      <div data-testid="signatures-state" data-state={state} className={styles.panelMessage}>
        {t(`sign.state.${state}`)}
      </div>
      {report?.error && <div className={styles.panelMessage}>{report.error}</div>}
      {report?.signatures.map((s, i) => {
        const st = signatureState(s);
        return (
          <div key={i} data-testid="signature-item" data-state={st}>
            <div>
              <strong>{commonName(s.signer)}</strong> — {t(`sign.state.${st}`)}
            </div>
            {s.timestamp ? (
              <div>
                {t('sign.timestampBy', {
                  time: s.timestamp.time,
                  tsa: commonName(s.timestamp.tsa),
                })}
              </div>
            ) : (
              <div>{t('sign.noTimestamp')}</div>
            )}
            {s.chain.trusted && s.chain.anchor && (
              <div>{t('sign.anchor', { name: commonName(s.chain.anchor) })}</div>
            )}
            {[...problemKeys(s.chain), ...(s.timestamp ? problemKeys(s.timestamp.chain) : [])].map(
              (p, j) => (
                <div key={j} className={styles.panelMessage}>
                  {t(p.key, { subject: p.subject ? commonName(p.subject) : '' })}
                </div>
              ),
            )}
          </div>
        );
      })}
      <div className={styles.panelRow}>
        <button type="button" onClick={onClose}>
          {t('sign.close')}
        </button>
      </div>
    </div>
  );
}
