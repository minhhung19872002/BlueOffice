/**
 * Signature lines (CORE-SIGLINE-001): the dialog that inserts a line, the dialog
 * that chooses a line to sign and how (typed name or picture, comment) before the
 * usual "Ký số" dialog picks the signer, and the per-line verification list of
 * the signatures panel. The helpers are free of React so they are unit-tested.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  EditCommand,
  EditResult,
  SignatureLineInfo,
  SignatureLineSignRequest,
  SignatureLineSpec,
  SignatureLineStatus,
  SignatureReport,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState } from 'react';
import styles from './Editor.module.css';
import { bytesToBase64, commonName } from './signing';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** `SignatureText` is at most 100 characters, `SignatureComments` 255 ([MS-OFFCRYPTO]). */
export const MAX_TEXT = 100;
export const MAX_COMMENTS = 255;

export const EMPTY_SPEC: SignatureLineSpec = {
  suggestedSigner: '',
  suggestedSigner2: '',
  suggestedSignerEmail: '',
  signingInstructions: '',
  allowComments: false,
  showSignDate: true,
};

/** A random braced GUID (RFC 9562 version 4) for a new line. */
export function newLineId(random: (n: number) => Uint8Array = randomBytes): string {
  const b = random(16);
  b[6] = (b[6]! & 0x0f) | 0x40;
  b[8] = (b[8]! & 0x3f) | 0x80;
  const h = Array.from(b, (x) => x.toString(16).padStart(2, '0').toUpperCase()).join('');
  return `{${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}}`;
}

function randomBytes(n: number): Uint8Array {
  const b = new Uint8Array(n);
  crypto.getRandomValues(b);
  return b;
}

/** What the signer chose for a line (typed name and/or picture, comment). */
export interface LineChoice {
  id: string;
  text: string;
  /** Picture file name and bytes (base64). */
  picture: { name: string; base64: string } | null;
  comments: string;
}

/** Lines that can still be signed. */
export function unsignedLines(lines: SignatureLineInfo[]): SignatureLineInfo[] {
  return lines.filter((l) => !l.line.signed);
}

/** i18n key of what is missing, or null. */
export function validateChoice(c: LineChoice): string | null {
  if (!c.id) return 'sigline.needLine';
  if (!c.text.trim() && !c.picture) return 'sigline.needName';
  return null;
}

/** `YYYY-MM-DD` of a local date. */
export function isoDay(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** The engine command producing the line pictures for `c` signed on `date`. */
export function imagesCommand(c: LineChoice, date: string): EditCommand {
  return {
    cmd: 'signatureLineImages',
    id: c.id,
    text: c.text.trim().slice(0, MAX_TEXT),
    imageBase64: c.picture?.base64 ?? null,
    date,
  };
}

/** `SignRequest.signatureLine` from the choice and the engine's pictures. */
export function lineSignRequest(
  c: LineChoice,
  images: { valid: string; invalid: string },
): SignatureLineSignRequest {
  return {
    id: c.id,
    text: c.text.trim().slice(0, MAX_TEXT),
    imageBase64: c.picture?.base64,
    comments: c.comments.trim().slice(0, MAX_COMMENTS) || undefined,
    validImageBase64: images.valid,
    invalidImageBase64: images.invalid,
  };
}

/**
 * Bytes to sign: the opened file itself while the document is unchanged (so the
 * signatures already in it keep verifying), else the saved document.
 */
export function bytesToSign(
  modified: boolean,
  opened: Uint8Array | null,
  save: () => Promise<Uint8Array>,
): Promise<Uint8Array> {
  const isPackage = !!opened && opened[0] === 0x50 && opened[1] === 0x4b;
  return !modified && opened && isPackage ? Promise.resolve(opened) : save();
}

/** The engine command showing a report's per-line results on the lines. */
export function statusCommand(report: SignatureReport | null): EditCommand | null {
  const lines = report?.signatureLines;
  if (!lines?.length) return null;
  return {
    cmd: 'setSignatureLineStatus',
    lines: lines.map((l) => ({ id: l.id, valid: l.valid })),
  };
}

/** Label of a line in the chooser. */
function lineLabel(l: SignatureLineInfo, none: string): string {
  const who = [l.line.suggestedSigner, l.line.suggestedSigner2].filter(Boolean).join(', ');
  return who || none;
}

/** "Dòng chữ ký": insert a signature line at the caret. */
export function InsertSignatureLineDialog({
  onInsert,
  onClose,
}: {
  onInsert: (spec: SignatureLineSpec) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [spec, setSpec] = useState<SignatureLineSpec>(EMPTY_SPEC);
  const set = (p: Partial<SignatureLineSpec>) => setSpec((s) => ({ ...s, ...p }));
  const text = (
    key: 'suggestedSigner' | 'suggestedSigner2' | 'suggestedSignerEmail',
    testId: string,
    type = 'text',
  ) => (
    <label className={styles.field}>
      {t(`sigline.${key}`)}
      <input
        data-testid={testId}
        type={type}
        value={spec[key]}
        onChange={(e) => set({ [key]: e.target.value })}
      />
    </label>
  );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sigline.insertTitle')}
      testId="sigline-insert-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('sigline.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="sigline-insert-ok"
            onClick={() => onInsert(spec)}
          >
            {t('sigline.insert')}
          </button>
        </>
      }
    >
      {text('suggestedSigner', 'sigline-signer')}
      {text('suggestedSigner2', 'sigline-title')}
      {text('suggestedSignerEmail', 'sigline-email', 'email')}
      <label className={styles.field}>
        {t('sigline.instructions')}
        <textarea
          data-testid="sigline-instructions"
          rows={2}
          value={spec.signingInstructions}
          onChange={(e) => set({ signingInstructions: e.target.value })}
        />
      </label>
      <label>
        <input
          type="checkbox"
          data-testid="sigline-allow-comments"
          checked={spec.allowComments}
          onChange={(e) => set({ allowComments: e.target.checked })}
        />
        {t('sigline.allowComments')}
      </label>
      <label>
        <input
          type="checkbox"
          data-testid="sigline-show-date"
          checked={spec.showSignDate}
          onChange={(e) => set({ showSignDate: e.target.checked })}
        />
        {t('sigline.showDate')}
      </label>
    </Dialog>
  );
}

/** Choose the line to sign, the typed name or picture and the comment. */
export function SignLineDialog({
  run,
  pickPicture,
  onContinue,
  onClose,
}: {
  run: Run;
  pickPicture: () => Promise<{ name: string; bytes: Uint8Array } | null>;
  onContinue: (choice: LineChoice) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [lines, setLines] = useState<SignatureLineInfo[] | null>(null);
  const [c, setC] = useState<LineChoice>({ id: '', text: '', picture: null, comments: '' });
  const set = (p: Partial<LineChoice>) => setC((x) => ({ ...x, ...p }));
  useEffect(() => {
    let live = true;
    void run({ cmd: 'signatureLines' }).then((r) => {
      if (!live) return;
      const open = unsignedLines(r?.signatureLines ?? []);
      setLines(open);
      const first = open[0];
      if (first) setC((x) => ({ ...x, id: first.line.id, text: first.line.suggestedSigner }));
    });
    return () => {
      live = false;
    };
  }, [run]);
  const line = lines?.find((l) => l.line.id === c.id)?.line;
  const problem = validateChoice(c);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sigline.signTitle')}
      testId="sigline-sign-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('sigline.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="sigline-sign-next"
            disabled={!!problem}
            onClick={() => onContinue(c)}
          >
            {t('sigline.next')}
          </button>
        </>
      }
    >
      {lines && lines.length === 0 && (
        <div className={styles.panelMessage} data-testid="sigline-none">
          {t('sigline.noLines')}
        </div>
      )}
      {lines && lines.length > 0 && (
        <>
          <label className={styles.field}>
            {t('sigline.line')}
            <select
              data-testid="sigline-line"
              value={c.id}
              onChange={(e) => {
                const l = lines.find((x) => x.line.id === e.target.value);
                set({ id: e.target.value, text: l?.line.suggestedSigner ?? '', comments: '' });
              }}
            >
              {lines.map((l, i) => (
                <option key={l.line.id} value={l.line.id}>
                  {`${i + 1}. ${lineLabel(l, t('sigline.unnamed'))}`}
                </option>
              ))}
            </select>
          </label>
          {line?.signingInstructions && (
            <div className={styles.panelMessage} data-testid="sigline-instructions-text">
              {line.signingInstructions}
            </div>
          )}
          <label className={styles.field}>
            {t('sigline.name')}
            <input
              data-testid="sigline-name"
              maxLength={MAX_TEXT}
              value={c.text}
              onChange={(e) => set({ text: e.target.value })}
            />
          </label>
          <div className={styles.panelRow}>
            <button
              type="button"
              data-testid="sigline-pick-picture"
              onClick={() =>
                void pickPicture().then((f) => {
                  if (f) set({ picture: { name: f.name, base64: bytesToBase64(f.bytes) } });
                })
              }
            >
              {t('sigline.choosePicture')}
            </button>
            <span data-testid="sigline-picture-name">
              {c.picture?.name ?? t('sigline.noPicture')}
            </span>
            {c.picture && (
              <button type="button" onClick={() => set({ picture: null })}>
                {t('sigline.removePicture')}
              </button>
            )}
          </div>
          {line?.allowComments && (
            <label className={styles.field}>
              {t('sigline.comments')}
              <input
                data-testid="sigline-comments"
                maxLength={MAX_COMMENTS}
                value={c.comments}
                onChange={(e) => set({ comments: e.target.value })}
              />
            </label>
          )}
          {problem && (
            <div className={styles.panelMessage} data-testid="sigline-problem">
              {t(problem)}
            </div>
          )}
        </>
      )}
    </Dialog>
  );
}

/** Per-line results in the signatures panel. */
export function SignatureLineStatusList({ lines }: { lines: SignatureLineStatus[] }) {
  const { t } = useTranslation();
  if (!lines.length) return null;
  return (
    <div data-testid="sigline-status">
      <strong>{t('sigline.statusTitle')}</strong>
      {lines.map((l) => (
        <div key={l.id} data-testid="sigline-status-item" data-valid={l.valid}>
          {l.valid ? t('sigline.valid') : t('sigline.invalid')}
          {' — '}
          {l.text || (l.signer ? commonName(l.signer) : l.id)}
          {!l.valid && l.error && <div className={styles.panelMessage}>{l.error}</div>}
        </div>
      ))}
    </div>
  );
}
