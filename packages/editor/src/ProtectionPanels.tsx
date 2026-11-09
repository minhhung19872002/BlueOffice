/**
 * File protection UI shared by BlueDoc, BlueSheet and BlueSlide (CORE-SEC-001):
 * the notice bar of a final / read-only file, the "open read-only or edit"
 * prompt, and the File › Info › Protect dialog (final mark, read-only
 * recommendation, password to modify, label, restricted editing in BlueDoc,
 * protected ranges in BlueSheet). The engines do the work; these components
 * only send security commands.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  SECURITY_ERRORS,
  configuredLabels,
  labelName,
  openProtection,
  type LabelSpec,
  type ProtectedRangeInfo,
  type SecurityCommand,
  type SecurityInfo,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useState } from 'react';
import styles from './Protection.module.css';

/** Answer of a security command (any of the three engines). */
export interface SecurityAnswer {
  security?: SecurityInfo;
  ranges?: ProtectedRangeInfo[];
  error?: string;
}

export type RunSecurity = (cmd: SecurityCommand) => Promise<SecurityAnswer | null>;

type T = ReturnType<typeof useTranslation>['t'];

/** Translated message of an engine refusal (unknown messages as they are). */
export function securityMessage(t: T, error: string): string {
  switch (error) {
    case SECURITY_ERRORS.readOnly:
      return t('protect.err.readOnly');
    case SECURITY_ERRORS.documentProtected:
      return t('protect.err.documentProtected');
    case SECURITY_ERRORS.writePasswordRequired:
      return t('protect.err.writePasswordRequired');
    case SECURITY_ERRORS.wrongPassword:
      return t('protect.err.wrongPassword');
    case SECURITY_ERRORS.unsupportedHash:
      return t('protect.err.unsupportedHash');
    case 'sheet protected':
      return t('protect.err.sheetProtected');
    case 'unsupported':
      return t('protect.err.unsupported');
    default:
      return error;
  }
}

/** Whether an engine error is a protection refusal (shown as a notice, not a failure). */
export function isProtectionRefusal(error: string | undefined): boolean {
  return error === SECURITY_ERRORS.readOnly || error === SECURITY_ERRORS.documentProtected;
}

/**
 * Names the signed-in user is known by for editable regions: the display
 * name and the account id when it is an e-mail. No user: none ("everyone"
 * regions only).
 */
export function identityNames(user: { id: string; name: string } | null | undefined): string[] {
  if (!user) return [];
  const out = [user.name, user.id].map((s) => s.trim()).filter((s) => s.length > 0);
  return out.filter(
    (s, i) => out.indexOf(s) === i && (i === 0 || s.includes('@') || s.includes('\\')),
  );
}

/** i18n key of a protection refusal (status bar notice). */
export function refusalKey(error: string | undefined): string {
  return error === SECURITY_ERRORS.readOnly
    ? 'protect.err.readOnly'
    : 'protect.err.documentProtected';
}

/** BlueOffice's default label list (an administrator may configure another). */
export function defaultLabels(t: T): LabelSpec[] {
  return [
    { id: 'public', name: t('protect.labels.public') },
    {
      id: 'internal',
      name: t('protect.labels.internal'),
      marking: t('protect.labels.internalMark'),
    },
    { id: 'secret', name: t('protect.labels.secret'), marking: t('protect.labels.secretMark') },
  ];
}

/** Status bar text: label name, read-only state. */
export function ProtectionStatus({ info }: { info: SecurityInfo | null }) {
  const { t } = useTranslation();
  if (!info) return null;
  const label = labelName(info);
  return (
    <>
      {label && (
        <span data-testid="status-label" title={t('protect.labelTitle')}>
          {t('protect.statusLabel', { name: label })}
        </span>
      )}
      {info.readOnly && <span data-testid="status-readonly">{t('protect.statusReadOnly')}</span>}
      {info.protection?.enforced && (
        <span data-testid="status-restricted">{t('protect.statusRestricted')}</span>
      )}
    </>
  );
}

/**
 * Notice bar while a file is read-only: final ("edit anyway") or opened
 * read-only on recommendation / without the password to modify ("edit").
 */
export function ProtectionBar({
  info,
  onEdit,
  onDismiss,
  inline,
}: {
  info: SecurityInfo;
  onEdit: () => void;
  onDismiss?: () => void;
  inline?: boolean;
}) {
  const { t } = useTranslation();
  if (!info.readOnly) return null;
  const final = info.markedFinal;
  return (
    <div
      className={styles.bar}
      style={inline ? { position: 'relative' } : undefined}
      role="status"
      data-testid="protection-bar"
    >
      <strong>{t(final ? 'protect.finalTitle' : 'protect.readOnlyTitle')}</strong>
      <span>{t(final ? 'protect.finalNotice' : 'protect.readOnlyNotice')}</span>
      <button type="button" data-testid="protection-edit" onClick={onEdit}>
        {t(final ? 'protect.editAnyway' : 'protect.enableEditing')}
      </button>
      {onDismiss && (
        <button type="button" onClick={onDismiss}>
          {t('protect.close')}
        </button>
      )}
    </div>
  );
}

/**
 * Prompt on open of a file that recommends read-only or has a password to
 * modify: open read-only, or edit (with the password).
 */
export function OpenProtectionDialog({
  info,
  name,
  run,
  onDone,
}: {
  info: SecurityInfo;
  name: string;
  run: RunSecurity;
  onDone: (s: SecurityInfo | null) => void;
}) {
  const { t } = useTranslation();
  const kind = openProtection(info);
  const [pw, setPw] = useState('');
  const [error, setError] = useState<string | null>(null);
  if (kind !== 'recommended' && kind !== 'password') return null;
  const edit = async () => {
    const r =
      kind === 'password'
        ? await run({ cmd: 'unlockWrite', password: pw })
        : await run({ cmd: 'setReadOnly', on: false });
    if (r?.error) setError(securityMessage(t, r.error));
    else onDone(r?.security ?? null);
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onDone(null)}
      title={t('protect.openTitle')}
      testId="open-protection"
      footer={
        <>
          <button type="button" data-testid="open-read-only" onClick={() => onDone(null)}>
            {t('protect.openReadOnly')}
          </button>
          <button
            type="button"
            data-testid="open-edit"
            disabled={kind === 'password' && !pw}
            onClick={() => void edit()}
          >
            {t('protect.openEdit')}
          </button>
        </>
      }
    >
      <div>
        {t(kind === 'password' ? 'protect.openPasswordPrompt' : 'protect.openRecommended', {
          name,
        })}
      </div>
      {kind === 'password' && (
        <div className={styles.row}>
          <input
            autoFocus
            type="password"
            data-testid="open-write-password"
            aria-label={t('protect.writePassword')}
            placeholder={t('protect.writePassword')}
            value={pw}
            onChange={(e) => setPw(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && pw) void edit();
            }}
          />
        </div>
      )}
      {error && (
        <div className={styles.error} role="alert">
          {error}
        </div>
      )}
    </Dialog>
  );
}

/** File › Info › Protect. */
export function ProtectDialog({
  app,
  info,
  run,
  onClose,
  sheet,
  selectionRef,
  ranges: initialRanges,
}: {
  app: 'doc' | 'sheet' | 'slide';
  info: SecurityInfo;
  run: RunSecurity;
  onClose: () => void;
  /** BlueSheet: current sheet index. */
  sheet?: number;
  /** BlueSheet: the selection as a reference (`B2:D9`). */
  selectionRef?: string;
  ranges?: ProtectedRangeInfo[];
}) {
  const { t } = useTranslation();
  const [s, setS] = useState<SecurityInfo>(info);
  const [error, setError] = useState<string | null>(null);
  const [writePw, setWritePw] = useState('');
  const [protectPw, setProtectPw] = useState('');
  const [editor, setEditor] = useState('');
  const [ranges, setRanges] = useState<ProtectedRangeInfo[]>(initialRanges ?? []);
  const [rangeName, setRangeName] = useState('');
  const [rangeRef, setRangeRef] = useState(selectionRef ?? '');
  const [rangePw, setRangePw] = useState('');
  const [marking, setMarking] = useState(true);
  const labels = configuredLabels(defaultLabels(t));
  const send = async (cmd: SecurityCommand) => {
    const r = await run(cmd);
    if (r?.security) setS(r.security);
    if (r?.ranges) setRanges(r.ranges);
    setError(r?.error ? securityMessage(t, r.error) : null);
    return r;
  };
  const current = s.label?.name ?? '';
  const locked = s.readOnly;
  const restricted = !!s.protection?.enforced && s.protection.edit === 'readOnly';
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('protect.title')}
      testId="protect-panel"
      width={480}
      footer={
        <button type="button" data-testid="protect-close" onClick={onClose}>
          {t('protect.close')}
        </button>
      }
    >
      <div className={styles.section}>
        <h3>{t('protect.finalHeading')}</h3>
        <div className={styles.hint}>
          {t(s.markedFinal ? 'protect.finalOn' : 'protect.finalOff')}
        </div>
        <div className={styles.row}>
          <button
            type="button"
            data-testid="protect-final"
            onClick={() => void send({ cmd: 'markFinal', on: !s.markedFinal })}
            disabled={locked && !s.markedFinal}
          >
            {t(s.markedFinal ? 'protect.unmarkFinal' : 'protect.markFinal')}
          </button>
        </div>
      </div>

      <div className={styles.section}>
        <h3>{t('protect.writeHeading')}</h3>
        {app !== 'slide' && (
          <label className={styles.row}>
            <input
              type="checkbox"
              data-testid="protect-recommended"
              checked={s.readOnlyRecommended}
              disabled={locked}
              onChange={(e) => void send({ cmd: 'setReadOnlyRecommended', on: e.target.checked })}
            />
            {t('protect.recommended')}
          </label>
        )}
        <div className={styles.row}>
          <input
            type="password"
            data-testid="protect-write-password"
            aria-label={t('protect.writePassword')}
            placeholder={t('protect.writePassword')}
            value={writePw}
            disabled={locked}
            onChange={(e) => setWritePw(e.target.value)}
          />
          <button
            type="button"
            data-testid="protect-write-set"
            disabled={locked || !writePw}
            onClick={() => {
              void send({ cmd: 'setWritePassword', password: writePw });
              setWritePw('');
            }}
          >
            {t('protect.set')}
          </button>
          {s.writePassword && (
            <button
              type="button"
              disabled={locked}
              onClick={() => void send({ cmd: 'setWritePassword', password: '' })}
            >
              {t('protect.remove')}
            </button>
          )}
        </div>
        <div className={styles.hint}>
          {t(s.writePassword ? 'protect.writeOn' : 'protect.writeHint')}
        </div>
      </div>

      <div className={styles.section}>
        <h3>{t('protect.labelHeading')}</h3>
        <div className={styles.row}>
          <select
            data-testid="protect-label"
            aria-label={t('protect.labelHeading')}
            value={current}
            disabled={locked}
            onChange={(e) => {
              const l = labels.find((x) => x.name === e.target.value);
              void send({
                cmd: 'setLabel',
                label: l
                  ? {
                      id: l.id ?? null,
                      name: l.name,
                      marking: app === 'doc' && marking ? (l.marking ?? null) : null,
                    }
                  : null,
              });
            }}
          >
            <option value="">{t('protect.noLabel')}</option>
            {labels.map((l) => (
              <option key={l.id ?? l.name} value={l.name}>
                {l.name}
              </option>
            ))}
            {current && !labels.some((l) => l.name === current) && (
              <option value={current}>{current}</option>
            )}
          </select>
        </div>
        {app === 'doc' && (
          <label className={styles.row}>
            <input
              type="checkbox"
              checked={marking}
              onChange={(e) => setMarking(e.target.checked)}
            />
            {t('protect.marking')}
          </label>
        )}
        {s.foreignLabels.length > 0 && (
          <ul className={styles.list} data-testid="protect-foreign-labels">
            {s.foreignLabels.map((l) => (
              <li key={l.id}>
                <span>{l.name ?? l.id}</span>
                <span className={styles.tag}>
                  {t(l.enabled ? 'protect.foreignOn' : 'protect.foreignOff')}
                </span>
              </li>
            ))}
          </ul>
        )}
        {s.foreignLabels.length > 0 && (
          <div className={styles.hint}>{t('protect.foreignHint')}</div>
        )}
      </div>

      {app === 'doc' && (
        <div className={styles.section}>
          <h3>{t('protect.restrictHeading')}</h3>
          <div className={styles.hint}>
            {t(restricted ? 'protect.restrictOn' : 'protect.restrictHint')}
          </div>
          {!restricted && (
            <>
              <div className={styles.row}>
                <button
                  type="button"
                  data-testid="protect-region-everyone"
                  disabled={locked}
                  onClick={() => void send({ cmd: 'addEditableRegion' })}
                >
                  {t('protect.regionEveryone')}
                </button>
              </div>
              <div className={styles.row}>
                <input
                  type="text"
                  aria-label={t('protect.regionUser')}
                  placeholder={t('protect.regionUser')}
                  value={editor}
                  onChange={(e) => setEditor(e.target.value)}
                />
                <button
                  type="button"
                  disabled={locked || !editor.trim()}
                  onClick={() => void send({ cmd: 'addEditableRegion', editor: editor.trim() })}
                >
                  {t('protect.regionAdd')}
                </button>
              </div>
            </>
          )}
          {(s.regions ?? []).length > 0 && (
            <ul className={styles.list} data-testid="protect-regions">
              {(s.regions ?? []).map((r) => (
                <li key={r.id}>
                  <span>
                    {r.editor ??
                      (r.group === 'everyone' ? t('protect.everyone') : (r.group ?? r.id))}
                  </span>
                  {r.mine && <span className={styles.tag}>{t('protect.mine')}</span>}
                  {!restricted && (
                    <button
                      type="button"
                      onClick={() => void send({ cmd: 'removeEditableRegion', id: r.id })}
                    >
                      {t('protect.remove')}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
          <div className={styles.row}>
            <input
              type="password"
              data-testid="protect-doc-password"
              aria-label={t('protect.protectPassword')}
              placeholder={t('protect.protectPassword')}
              value={protectPw}
              onChange={(e) => setProtectPw(e.target.value)}
            />
            {restricted ? (
              <button
                type="button"
                data-testid="protect-stop"
                onClick={() => {
                  void send({ cmd: 'unprotectDocument', password: protectPw });
                  setProtectPw('');
                }}
              >
                {t('protect.stop')}
              </button>
            ) : (
              <button
                type="button"
                data-testid="protect-start"
                disabled={locked}
                onClick={() => {
                  void send({ cmd: 'protectDocument', password: protectPw });
                  setProtectPw('');
                }}
              >
                {t('protect.start')}
              </button>
            )}
          </div>
          <label className={styles.row}>
            <input
              type="checkbox"
              data-testid="protect-show-regions"
              checked={s.showRegions}
              onChange={(e) => void send({ cmd: 'showEditableRegions', on: e.target.checked })}
            />
            {t('protect.showRegions')}
          </label>
          <div className={styles.row}>
            <button
              type="button"
              data-testid="protect-next-region"
              onClick={() => void send({ cmd: 'nextEditableRegion', forward: true })}
            >
              {t('protect.nextRegion')}
            </button>
          </div>
        </div>
      )}

      {app === 'sheet' && sheet !== undefined && (
        <div className={styles.section}>
          <h3>{t('protect.rangesHeading')}</h3>
          <div className={styles.hint}>{t('protect.rangesHint')}</div>
          {ranges.length > 0 && (
            <ul className={styles.list} data-testid="protect-ranges">
              {ranges.map((r) => (
                <RangeRow key={r.name} range={r} sheet={sheet} send={send} />
              ))}
            </ul>
          )}
          <div className={styles.row}>
            <input
              type="text"
              aria-label={t('protect.rangeName')}
              placeholder={t('protect.rangeName')}
              value={rangeName}
              onChange={(e) => setRangeName(e.target.value)}
            />
            <input
              type="text"
              aria-label={t('protect.rangeRef')}
              placeholder={t('protect.rangeRef')}
              value={rangeRef}
              onChange={(e) => setRangeRef(e.target.value)}
            />
          </div>
          <div className={styles.row}>
            <input
              type="password"
              aria-label={t('protect.rangePassword')}
              placeholder={t('protect.rangePassword')}
              value={rangePw}
              onChange={(e) => setRangePw(e.target.value)}
            />
            <button
              type="button"
              data-testid="protect-range-add"
              disabled={locked || !rangeName.trim() || !rangeRef.trim()}
              onClick={() => {
                void send({
                  cmd: 'addProtectedRange',
                  sheet,
                  name: rangeName.trim(),
                  sqref: rangeRef.trim(),
                  password: rangePw,
                });
                setRangePw('');
              }}
            >
              {t('protect.rangeAdd')}
            </button>
          </div>
        </div>
      )}
      {error && (
        <div className={styles.error} role="alert" data-testid="protect-error">
          {error}
        </div>
      )}
    </Dialog>
  );
}

/** Protection state of an editor (BlueSheet, BlueSlide): info, open UI, actions. */
export interface FileProtection {
  info: SecurityInfo | null;
  ui: 'panel' | 'prompt' | null;
  setUi: (ui: 'panel' | 'prompt' | null) => void;
  run: RunSecurity;
  /** Call once a file is open: identity, then the prompt when it opens read-only. */
  opened: () => Promise<void>;
  /** The notice bar's button: edit anyway / allow editing. */
  edit: () => void;
}

export function useFileProtection(
  run: RunSecurity,
  user: { id: string; name: string } | null | undefined,
): FileProtection {
  const [info, setInfo] = useState<SecurityInfo | null>(null);
  const [ui, setUi] = useState<'panel' | 'prompt' | null>(null);
  const wrapped = useCallback<RunSecurity>(
    async (c) => {
      const r = await run(c);
      if (r?.security) setInfo(r.security);
      return r;
    },
    [run],
  );
  const opened = useCallback(async () => {
    setUi(null);
    setInfo(null);
    const r = await wrapped({ cmd: 'setIdentity', names: identityNames(user) });
    if (r?.security && openProtection(r.security) !== 'none' && !r.security.markedFinal)
      setUi('prompt');
  }, [wrapped, user]);
  const edit = useCallback(() => {
    if (info?.markedFinal) void wrapped({ cmd: 'markFinal', on: false });
    else if (info?.writePassword) setUi('prompt');
    else void wrapped({ cmd: 'setReadOnly', on: false });
  }, [info, wrapped]);
  return { info, ui, setUi, run: wrapped, opened, edit };
}

/** Notice bar, open prompt and Protect dialog of an editor using [`useFileProtection`]. */
export function FileProtectionUi({
  app,
  p,
  name,
  sheet,
  selectionRef,
}: {
  app: 'doc' | 'sheet' | 'slide';
  p: FileProtection;
  name: string;
  sheet?: number;
  selectionRef?: string;
}) {
  const [ranges, setRanges] = useState<ProtectedRangeInfo[] | null>(null);
  useEffect(() => {
    if (p.ui !== 'panel' || app !== 'sheet' || sheet === undefined) return;
    let live = true;
    void p.run({ cmd: 'protectedRanges', sheet }).then((r) => {
      if (live) setRanges(r?.ranges ?? []);
    });
    return () => {
      live = false;
    };
  }, [p, app, sheet]);
  if (!p.info) return null;
  return (
    <>
      {p.info.readOnly && <ProtectionBar inline info={p.info} onEdit={p.edit} />}
      {p.ui === 'prompt' && (
        <OpenProtectionDialog info={p.info} name={name} run={p.run} onDone={() => p.setUi(null)} />
      )}
      {p.ui === 'panel' && (app !== 'sheet' || ranges) && (
        <ProtectDialog
          app={app}
          info={p.info}
          run={p.run}
          onClose={() => {
            p.setUi(null);
            setRanges(null);
          }}
          sheet={sheet}
          selectionRef={selectionRef}
          ranges={ranges ?? []}
        />
      )}
    </>
  );
}

function RangeRow({
  range,
  sheet,
  send,
}: {
  range: ProtectedRangeInfo;
  sheet: number;
  send: (cmd: SecurityCommand) => Promise<SecurityAnswer | null>;
}) {
  const { t } = useTranslation();
  const [pw, setPw] = useState('');
  return (
    <li>
      <span>
        {range.name} — {range.sqref}
      </span>
      {range.hasPassword && !range.unlocked && (
        <>
          <input
            type="password"
            aria-label={t('protect.rangePassword')}
            placeholder={t('protect.rangePassword')}
            value={pw}
            onChange={(e) => setPw(e.target.value)}
          />
          <button
            type="button"
            onClick={() =>
              void send({ cmd: 'unlockProtectedRange', sheet, name: range.name, password: pw })
            }
          >
            {t('protect.unlock')}
          </button>
        </>
      )}
      {range.unlocked && <span className={styles.tag}>{t('protect.editable')}</span>}
      <button
        type="button"
        onClick={() => void send({ cmd: 'removeProtectedRange', sheet, name: range.name })}
      >
        {t('protect.remove')}
      </button>
    </li>
  );
}
