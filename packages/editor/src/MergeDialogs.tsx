/**
 * Extended mail merge dialogs (DOC-MERGE-001): recipient list editing, rule fields
 * (ECMA-376 Part 1, §17.16.5 IF / SKIPIF / NEXTIF / NEXT / MERGEREC / MERGESEQ / ASK /
 * FILLIN / SET), prompts, envelopes, label sheets, recipients from a host data provider
 * and the merge outputs (document, one PDF per recipient, e-mail per recipient).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  EditCommand,
  EditResult,
  EnvelopeSize,
  FilterOp,
  FilterRule,
  LabelMode,
  LabelSheet,
  MailAttach,
  MailMessage,
  MailProvider,
  MailSendResult,
  MergeAnswers,
  MergeDataProvider,
  MergePrompt,
  MergeRecords,
  MergeRule,
  MergeSourceInfo,
  MergeState,
  RecipientOptions,
  RuleOp,
  SortKey,
} from '@blueoffice/platform';
import { Dialog, Popover } from '@blueoffice/ui';
import { ChevronDown } from 'lucide-react';
import { useEffect, useMemo, useState, type ReactNode } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Dialogs of this file (the editor's `dialog` state). */
export type MailMergeDialogId =
  | 'mergeRecipients'
  | 'mergeRule'
  | 'mergeEnvelope'
  | 'mergeLabels'
  | 'mergeFinish'
  | 'mergeProvider';

export type RuleKind = MergeRule['kind'];

/** Rule kinds in menu order. */
export const RULE_KINDS: RuleKind[] = [
  'if',
  'skipIf',
  'nextIf',
  'next',
  'mergeRec',
  'mergeSeq',
  'ask',
  'fillIn',
  'set',
];

export const RULE_OPS: RuleOp[] = ['=', '<>', '<', '<=', '>', '>='];

export const FILTER_OPS: FilterOp[] = [
  'eq',
  'ne',
  'lt',
  'le',
  'gt',
  'ge',
  'empty',
  'notEmpty',
  'contains',
  'notContains',
];

/** How the merge finishes. */
export type FinishMode = 'document' | 'pdfs' | 'email';

/** Footer with Cancel / OK. */
function Buttons({
  onCancel,
  onOk,
  okTestId,
  okDisabled,
  okLabel,
}: {
  onCancel: () => void;
  onOk: () => void;
  okTestId: string;
  okDisabled?: boolean;
  okLabel?: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.dialogButton} onClick={onCancel}>
        {t('mailMerge.cancel')}
      </button>
      <button
        type="button"
        className={styles.dialogPrimary}
        data-testid={okTestId}
        disabled={okDisabled}
        onClick={onOk}
      >
        {okLabel ?? t('mailMerge.ok')}
      </button>
    </>
  );
}

function ErrorLine({ error, testId }: { error: string | null; testId: string }) {
  return error ? (
    <p role="alert" data-testid={testId}>
      {error}
    </p>
  ) : null;
}

/** A menu entry backed by a registry command. */
export interface MenuEntry {
  id: string;
  label: string;
  enabled: boolean;
  onSelect: () => void;
}

/** A ribbon dropdown of commands (Rules ▾, Finish ▾). */
export function CommandMenu({
  label,
  icon,
  entries,
  testId,
  disabled,
}: {
  label: string;
  icon: ReactNode;
  entries: MenuEntry[];
  testId: string;
  disabled?: boolean;
}) {
  return (
    <Popover
      label={label}
      disabled={disabled}
      testId={testId}
      trigger={
        <>
          {icon}
          <span>{label}</span>
          <ChevronDown size={12} />
        </>
      }
    >
      {(close) => (
        <div role="menu" aria-label={label}>
          {entries.map((e) => (
            <button
              key={e.id}
              type="button"
              role="menuitem"
              className={styles.dialogButton}
              style={{ display: 'block', width: '100%', textAlign: 'start' }}
              data-command={e.id}
              disabled={!e.enabled}
              onClick={() => {
                close();
                e.onSelect();
              }}
            >
              {e.label}
            </button>
          ))}
        </div>
      )}
    </Popover>
  );
}

/** Insert `«Field»` at the end of `text` (envelope / label templates). */
export function withField(text: string, field: string): string {
  return `${text}«${field}»`;
}

function FieldPicker({
  fields,
  onPick,
  testId,
}: {
  fields: string[];
  onPick: (f: string) => void;
  testId: string;
}) {
  const { t } = useTranslation();
  return (
    <select
      aria-label={t('mailMerge.insertField')}
      data-testid={testId}
      value=""
      disabled={fields.length === 0}
      onChange={(e) => e.target.value && onPick(e.target.value)}
    >
      <option value="">{t('mailMerge.insertField')}</option>
      {fields.map((f) => (
        <option key={f} value={f}>
          {f}
        </option>
      ))}
    </select>
  );
}

// ---------------------------------------------------------------- recipients

/** The list options from the dialog state: rules with a field, used sort levels. */
export function recipientOptions(
  filter: FilterRule[],
  sort: SortKey[],
  excluded: Set<number>,
): RecipientOptions {
  return {
    filter: filter.filter((r) => r.field !== ''),
    sort: sort.filter((s) => s.field !== '').slice(0, 3),
    excluded: [...excluded].sort((a, b) => a - b),
  };
}

/** Edit the recipient list: filter, sort, include / exclude, duplicates. */
export function RecipientsDialog({
  merge,
  run,
  onClose,
}: {
  merge: MergeState | null;
  run: Run;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const fields = merge?.fields ?? [];
  const opts = merge?.options;
  const [records, setRecords] = useState<MergeRecords | null>(null);
  const [filter, setFilter] = useState<FilterRule[]>(
    opts?.filter?.length ? opts.filter : [{ field: '', op: 'eq', value: '' }],
  );
  const [sort, setSort] = useState<SortKey[]>(() => {
    const s = [...(opts?.sort ?? [])];
    while (s.length < 3) s.push({ field: '' });
    return s;
  });
  const [excluded, setExcluded] = useState<Set<number>>(new Set(opts?.excluded ?? []));
  const [dupes, setDupes] = useState<number[][] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async (o: RecipientOptions) => {
    const r = await run({ cmd: 'mergeSetRecipients', options: o });
    if (r?.error) setError(r.error);
    const rec = await run({ cmd: 'mergeRecords' });
    setRecords(rec?.mergeOut?.records ?? null);
  };
  useEffect(() => {
    void run({ cmd: 'mergeRecords' }).then((r) => setRecords(r?.mergeOut?.records ?? null));
  }, [run]);

  const current = () => recipientOptions(filter, sort, excluded);
  const setRule = (i: number, patch: Partial<FilterRule>) =>
    setFilter((f) => f.map((r, k) => (k === i ? { ...r, ...patch } : r)));
  const setKey = (i: number, patch: Partial<SortKey>) =>
    setSort((s) => s.map((k, n) => (n === i ? { ...k, ...patch } : k)));
  const toggle = (index: number) =>
    setExcluded((ex) => {
      const n = new Set(ex);
      if (n.has(index)) n.delete(index);
      else n.add(index);
      return n;
    });
  const dupeRows = new Set((dupes ?? []).flat());
  const findDupes = async () => {
    const r = await run({ cmd: 'mergeDuplicates' });
    setDupes(r?.mergeOut?.duplicates ?? []);
  };
  const ok = async () => {
    const r = await run({ cmd: 'mergeSetRecipients', options: current() });
    if (r?.error) {
      setError(r.error);
      return;
    }
    onClose();
  };
  const noValue = (op: FilterOp) => op === 'empty' || op === 'notEmpty';
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('mailMerge.recipients.title')}
      testId="merge-recipients"
      width={720}
      footer={<Buttons onCancel={onClose} onOk={() => void ok()} okTestId="merge-recipients-ok" />}
    >
      <div className={styles.formGrid}>
        <fieldset className={styles.field}>
          <legend>{t('mailMerge.recipients.filter')}</legend>
          {filter.map((r, i) => (
            <div key={i} data-testid={`merge-filter-${i}`}>
              {i > 0 && (
                <select
                  aria-label={t('mailMerge.recipients.join')}
                  data-testid={`merge-filter-join-${i}`}
                  value={r.or ? 'or' : 'and'}
                  onChange={(e) => setRule(i, { or: e.target.value === 'or' })}
                >
                  <option value="and">{t('mailMerge.recipients.and')}</option>
                  <option value="or">{t('mailMerge.recipients.or')}</option>
                </select>
              )}
              <select
                aria-label={t('mailMerge.field')}
                data-testid={`merge-filter-field-${i}`}
                value={r.field}
                onChange={(e) => setRule(i, { field: e.target.value })}
              >
                <option value="">{t('mailMerge.none')}</option>
                {fields.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
              <select
                aria-label={t('mailMerge.recipients.comparison')}
                data-testid={`merge-filter-op-${i}`}
                value={r.op}
                onChange={(e) => setRule(i, { op: e.target.value as FilterOp })}
              >
                {FILTER_OPS.map((op) => (
                  <option key={op} value={op}>
                    {t(`mailMerge.filterOp.${op}`)}
                  </option>
                ))}
              </select>
              {!noValue(r.op) && (
                <input
                  aria-label={t('mailMerge.value')}
                  data-testid={`merge-filter-value-${i}`}
                  value={r.value ?? ''}
                  onChange={(e) => setRule(i, { value: e.target.value })}
                />
              )}
            </div>
          ))}
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="merge-filter-add"
            onClick={() => setFilter((f) => [...f, { field: '', op: 'eq', value: '' }])}
          >
            {t('mailMerge.recipients.addCondition')}
          </button>
        </fieldset>
        <fieldset className={styles.field}>
          <legend>{t('mailMerge.recipients.sort')}</legend>
          {sort.map((k, i) => (
            <div key={i}>
              <select
                aria-label={t('mailMerge.recipients.sortLevel', { n: i + 1 })}
                data-testid={`merge-sort-field-${i}`}
                value={k.field}
                onChange={(e) => setKey(i, { field: e.target.value })}
              >
                <option value="">{t('mailMerge.none')}</option>
                {fields.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
              <select
                aria-label={t('mailMerge.recipients.order')}
                data-testid={`merge-sort-order-${i}`}
                value={k.descending ? 'desc' : 'asc'}
                disabled={!k.field}
                onChange={(e) => setKey(i, { descending: e.target.value === 'desc' })}
              >
                <option value="asc">{t('mailMerge.recipients.ascending')}</option>
                <option value="desc">{t('mailMerge.recipients.descending')}</option>
              </select>
            </div>
          ))}
        </fieldset>
        <div>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="merge-apply"
            onClick={() => void refresh(current())}
          >
            {t('mailMerge.recipients.apply')}
          </button>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="merge-duplicates"
            onClick={() => void findDupes()}
          >
            {t('mailMerge.recipients.duplicates')}
          </button>
          {dupes && (
            <span data-testid="merge-duplicates-count">
              {t('mailMerge.recipients.duplicateGroups', { count: dupes.length })}
            </span>
          )}
          {records && (
            <span data-testid="merge-active">
              {t('mailMerge.recipients.active', {
                active: records.active,
                total: records.total,
              })}
            </span>
          )}
        </div>
        <div style={{ maxHeight: 260, overflow: 'auto' }}>
          <table data-testid="merge-records">
            <thead>
              <tr>
                <th>{t('mailMerge.recipients.include')}</th>
                {fields.map((f) => (
                  <th key={f}>{f}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {(records?.rows ?? []).map((r) => (
                <tr
                  key={r.index}
                  data-testid={`merge-record-${r.index}`}
                  data-active={r.position !== undefined}
                  data-duplicate={dupeRows.has(r.index) || undefined}
                  style={{
                    opacity: r.position === undefined && !excluded.has(r.index) ? 0.5 : 1,
                    fontWeight: dupeRows.has(r.index) ? 600 : undefined,
                  }}
                >
                  <td>
                    <input
                      type="checkbox"
                      aria-label={t('mailMerge.recipients.includeRecord', { n: r.index + 1 })}
                      data-testid={`merge-include-${r.index}`}
                      checked={!excluded.has(r.index)}
                      onChange={() => toggle(r.index)}
                    />
                  </td>
                  {r.values.map((v, i) => (
                    <td key={i}>{v}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <ErrorLine error={error} testId="merge-recipients-error" />
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- rules

/** The rule the dialog's fields describe; `null` while incomplete. */
export function buildRule(
  kind: RuleKind,
  v: {
    field: string;
    op: RuleOp;
    value: string;
    then: string;
    otherwise: string;
    name: string;
    prompt: string;
    default: string;
    once: boolean;
  },
): MergeRule | null {
  switch (kind) {
    case 'if':
      return v.field
        ? { kind, field: v.field, op: v.op, value: v.value, then: v.then, else: v.otherwise }
        : null;
    case 'skipIf':
    case 'nextIf':
      return v.field ? { kind, field: v.field, op: v.op, value: v.value } : null;
    case 'next':
    case 'mergeRec':
    case 'mergeSeq':
      return { kind };
    case 'ask':
      return v.name.trim()
        ? { kind, name: v.name.trim(), prompt: v.prompt, default: v.default, once: v.once }
        : null;
    case 'fillIn':
      return { kind, prompt: v.prompt, default: v.default, once: v.once };
    case 'set':
      return v.name.trim() ? { kind, name: v.name.trim(), value: v.value } : null;
  }
}

/** Insert a rule field at the caret. */
export function RuleDialog({
  kind,
  fields,
  run,
  onClose,
}: {
  kind: RuleKind;
  fields: string[];
  run: Run;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [v, setV] = useState({
    field: fields[0] ?? '',
    op: '=' as RuleOp,
    value: '',
    then: '',
    otherwise: '',
    name: '',
    prompt: '',
    default: '',
    once: false,
  });
  const [error, setError] = useState<string | null>(null);
  const set = (patch: Partial<typeof v>) => setV((x) => ({ ...x, ...patch }));
  const rule = buildRule(kind, v);
  const ok = async () => {
    if (!rule) return;
    const r = await run({ cmd: 'mergeInsertRule', rule });
    if (r?.error) {
      setError(r.error);
      return;
    }
    onClose();
  };
  const compare = kind === 'if' || kind === 'skipIf' || kind === 'nextIf';
  const prompted = kind === 'ask' || kind === 'fillIn';
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(`mailMerge.rule.${kind}`)}
      testId="merge-rule"
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => void ok()}
          okTestId="merge-rule-ok"
          okDisabled={!rule}
        />
      }
    >
      <div className={styles.formGrid}>
        <p>{t(`mailMerge.ruleHelp.${kind}`)}</p>
        {compare && (
          <div>
            <select
              aria-label={t('mailMerge.field')}
              data-testid="merge-rule-field"
              value={v.field}
              onChange={(e) => set({ field: e.target.value })}
            >
              {fields.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
            <select
              aria-label={t('mailMerge.recipients.comparison')}
              data-testid="merge-rule-op"
              value={v.op}
              onChange={(e) => set({ op: e.target.value as RuleOp })}
            >
              {RULE_OPS.map((op) => (
                <option key={op} value={op}>
                  {op}
                </option>
              ))}
            </select>
            <input
              aria-label={t('mailMerge.value')}
              data-testid="merge-rule-value"
              value={v.value}
              onChange={(e) => set({ value: e.target.value })}
            />
          </div>
        )}
        {kind === 'if' && (
          <>
            <label className={styles.field}>
              <span>{t('mailMerge.rule.then')}</span>
              <input
                data-testid="merge-rule-then"
                value={v.then}
                onChange={(e) => set({ then: e.target.value })}
              />
            </label>
            <label className={styles.field}>
              <span>{t('mailMerge.rule.else')}</span>
              <input
                data-testid="merge-rule-else"
                value={v.otherwise}
                onChange={(e) => set({ otherwise: e.target.value })}
              />
            </label>
          </>
        )}
        {(kind === 'ask' || kind === 'set') && (
          <label className={styles.field}>
            <span>{t('mailMerge.rule.bookmark')}</span>
            <input
              data-testid="merge-rule-name"
              value={v.name}
              onChange={(e) => set({ name: e.target.value })}
            />
          </label>
        )}
        {kind === 'set' && (
          <label className={styles.field}>
            <span>{t('mailMerge.value')}</span>
            <input
              data-testid="merge-rule-set-value"
              value={v.value}
              onChange={(e) => set({ value: e.target.value })}
            />
          </label>
        )}
        {prompted && (
          <>
            <label className={styles.field}>
              <span>{t('mailMerge.rule.prompt')}</span>
              <input
                data-testid="merge-rule-prompt"
                value={v.prompt}
                onChange={(e) => set({ prompt: e.target.value })}
              />
            </label>
            <label className={styles.field}>
              <span>{t('mailMerge.rule.default')}</span>
              <input
                data-testid="merge-rule-default"
                value={v.default}
                onChange={(e) => set({ default: e.target.value })}
              />
            </label>
            <label className={styles.field}>
              <input
                type="checkbox"
                data-testid="merge-rule-once"
                checked={v.once}
                onChange={(e) => set({ once: e.target.checked })}
              />
              <span>{t('mailMerge.rule.once')}</span>
            </label>
          </>
        )}
        <ErrorLine error={error} testId="merge-rule-error" />
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- prompts + finish

/** Answers sent to the engine: per-record lists keep their length, others one value. */
export function answersOf(
  prompts: MergePrompt[],
  once: Record<string, string>,
  perRecord: Record<string, string[]>,
  perRecordOn: Record<string, boolean>,
): MergeAnswers {
  const out: MergeAnswers = {};
  for (const p of prompts) {
    if (!p.once && perRecordOn[p.key]) {
      out[p.key] = (perRecord[p.key] ?? []).map((v) => v ?? p.default);
    } else {
      out[p.key] = [once[p.key] ?? p.default];
    }
  }
  return out;
}

/** Most records the per-record prompt grid shows. */
export const PER_RECORD_LIMIT = 200;

/** Decode base64 to bytes. */
export function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Hooks the finish dialog calls for what it produced. */
export interface FinishHost {
  /** The open document was replaced by the merged one. */
  onMerged: (res: EditResult) => Promise<void> | void;
  /** Save the zip of PDFs. */
  saveZip: (bytes: Uint8Array, name: string) => Promise<unknown>;
  /** Send e-mails (absent: the e-mail mode is unavailable). */
  mail?: MailProvider;
}

/** Answer the prompts, then merge to one document, to PDFs (zip) or to e-mails. */
export function FinishDialog({
  mode,
  merge,
  run,
  host,
  onClose,
}: {
  mode: FinishMode;
  merge: MergeState | null;
  run: Run;
  host: FinishHost;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const fields = merge?.fields ?? [];
  const records = Math.min(merge?.active ?? merge?.count ?? 0, PER_RECORD_LIMIT);
  const [prompts, setPrompts] = useState<MergePrompt[]>([]);
  const [once, setOnce] = useState<Record<string, string>>({});
  const [perRecord, setPerRecord] = useState<Record<string, string[]>>({});
  const [perRecordOn, setPerRecordOn] = useState<Record<string, boolean>>({});
  const [pattern, setPattern] = useState(fields[0] ? `«${fields[0]}»` : '');
  const emailField = fields.find((f) => /e-?mail|thư/i.test(f)) ?? fields[0] ?? '';
  const [to, setTo] = useState(emailField);
  const [subject, setSubject] = useState('');
  const [attach, setAttach] = useState<MailAttach>('none');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  useEffect(() => {
    void run({ cmd: 'mergePrompts' }).then((r) => {
      const ps = r?.mergeOut?.prompts ?? [];
      setPrompts(ps);
      setOnce(Object.fromEntries(ps.map((p) => [p.key, p.default])));
    });
  }, [run]);
  const go = async () => {
    setBusy(true);
    setError(null);
    try {
      if (prompts.length > 0) {
        await run({
          cmd: 'mergeSetAnswers',
          answers: answersOf(prompts, once, perRecord, perRecordOn),
        });
      }
      if (mode === 'document') {
        const r = await run({ cmd: 'mergeToDocument' });
        if (!r || r.error) throw new Error(r?.error ?? t('mailMerge.failed'));
        await host.onMerged(r);
        onClose();
      } else if (mode === 'pdfs') {
        const r = await run({ cmd: 'mergeToPdfs', pattern });
        const out = r?.mergeOut;
        if (!r || r.error || !out?.zip) throw new Error(r?.error ?? t('mailMerge.failed'));
        await host.saveZip(fromBase64(out.zip), `${merge?.name ?? 'merge'}.zip`);
        setDone(t('mailMerge.finish.pdfsDone', { count: out.files?.length ?? 0 }));
      } else {
        if (!host.mail) throw new Error(t('mailMerge.finish.noMail'));
        const r = await run({ cmd: 'mergeToEmails', to, subject, attach });
        const mails: MailMessage[] = r?.mergeOut?.mails ?? [];
        if (!r || r.error) throw new Error(r?.error ?? t('mailMerge.failed'));
        const sent: MailSendResult = await host.mail.send(mails);
        setDone(
          t('mailMerge.finish.mailDone', {
            sent: sent.sent,
            failed: sent.failed.length,
            skipped: r.mergeOut?.skipped ?? 0,
          }),
        );
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  const disabled =
    busy || (mode === 'email' && (!to || !subject.trim() || !host.mail)) || done !== null;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(`mailMerge.finish.${mode}`)}
      testId="merge-finish"
      width={560}
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => void go()}
          okTestId="merge-finish-ok"
          okDisabled={disabled}
          okLabel={t('mailMerge.finish.run')}
        />
      }
    >
      <div className={styles.formGrid}>
        {prompts.map((p) => (
          <fieldset key={p.key} className={styles.field} data-testid={`merge-prompt-${p.key}`}>
            <legend>{p.prompt || p.name || p.key}</legend>
            {!p.once && (
              <label>
                <input
                  type="checkbox"
                  data-testid={`merge-prompt-per-record-${p.key}`}
                  checked={!!perRecordOn[p.key]}
                  onChange={(e) => setPerRecordOn((x) => ({ ...x, [p.key]: e.target.checked }))}
                />
                <span>{t('mailMerge.finish.perRecord')}</span>
              </label>
            )}
            {!p.once && perRecordOn[p.key] ? (
              Array.from({ length: records }, (_, i) => (
                <label key={i} className={styles.field}>
                  <span>{t('mailMerge.finish.recordN', { n: i + 1 })}</span>
                  <input
                    data-testid={`merge-answer-${p.key}-${i}`}
                    value={perRecord[p.key]?.[i] ?? p.default}
                    onChange={(e) =>
                      setPerRecord((x) => {
                        const v = [
                          ...(x[p.key] ?? Array.from({ length: records }, () => p.default)),
                        ];
                        v[i] = e.target.value;
                        return { ...x, [p.key]: v };
                      })
                    }
                  />
                </label>
              ))
            ) : (
              <input
                aria-label={p.prompt}
                data-testid={`merge-answer-${p.key}`}
                value={once[p.key] ?? ''}
                onChange={(e) => setOnce((x) => ({ ...x, [p.key]: e.target.value }))}
              />
            )}
          </fieldset>
        ))}
        {mode === 'pdfs' && (
          <label className={styles.field}>
            <span>{t('mailMerge.finish.fileName')}</span>
            <input
              data-testid="merge-pattern"
              value={pattern}
              onChange={(e) => setPattern(e.target.value)}
            />
            <FieldPicker
              fields={fields}
              testId="merge-pattern-field"
              onPick={(f) => setPattern((p) => withField(p, f))}
            />
          </label>
        )}
        {mode === 'email' && (
          <>
            <label className={styles.field}>
              <span>{t('mailMerge.finish.to')}</span>
              <select
                data-testid="merge-mail-to"
                value={to}
                onChange={(e) => setTo(e.target.value)}
              >
                {fields.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
            </label>
            <label className={styles.field}>
              <span>{t('mailMerge.finish.subject')}</span>
              <input
                data-testid="merge-mail-subject"
                value={subject}
                onChange={(e) => setSubject(e.target.value)}
              />
              <FieldPicker
                fields={fields}
                testId="merge-subject-field"
                onPick={(f) => setSubject((s) => withField(s, f))}
              />
            </label>
            <label className={styles.field}>
              <span>{t('mailMerge.finish.attach')}</span>
              <select
                data-testid="merge-mail-attach"
                value={attach}
                onChange={(e) => setAttach(e.target.value as MailAttach)}
              >
                <option value="none">{t('mailMerge.finish.attachNone')}</option>
                <option value="pdf">{t('mailMerge.finish.attachPdf')}</option>
                <option value="docx">{t('mailMerge.finish.attachDocx')}</option>
              </select>
            </label>
            {!host.mail && <p>{t('mailMerge.finish.noMail')}</p>}
          </>
        )}
        {busy && <p data-testid="merge-busy">{t('mailMerge.finish.working')}</p>}
        {done && (
          <p role="status" data-testid="merge-done">
            {done}
          </p>
        )}
        <ErrorLine error={error} testId="merge-finish-error" />
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- envelopes, labels

function usePresets(run: Run) {
  const [presets, setPresets] = useState<{
    labels: LabelSheet[];
    envelopes: EnvelopeSize[];
  } | null>(null);
  useEffect(() => {
    void run({ cmd: 'mergePresets' }).then((r) => setPresets(r?.mergeOut?.presets ?? null));
  }, [run]);
  return presets;
}

/** Number formatting of millimetres in preset names (no trailing zeros). */
export function mmText(v: number): string {
  return String(Math.round(v * 100) / 100);
}

/** Name of a label sheet preset, built from its geometry. */
export function labelPresetName(s: LabelSheet): string {
  const page = s.id.split('-')[0]?.toUpperCase() ?? '';
  return `${page} · ${s.cols} × ${s.rows} · ${mmText(s.labelWidth)} × ${mmText(s.labelHeight)} mm`;
}

/** Replace the document by an envelope addressed with merge fields. */
export function EnvelopeDialog({
  fields,
  run,
  onDone,
  onClose,
}: {
  fields: string[];
  run: Run;
  onDone: (res: EditResult) => Promise<void> | void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const presets = usePresets(run);
  const [id, setId] = useState('dl');
  const [custom, setCustom] = useState({ width: 220, height: 110 });
  const [delivery, setDelivery] = useState(fields[0] ? `«${fields[0]}»` : '');
  const [ret, setRet] = useState('');
  const [place, setPlace] = useState<{ x: string; y: string }>({ x: '', y: '' });
  const [error, setError] = useState<string | null>(null);
  const size: EnvelopeSize =
    id === 'custom'
      ? { id: 'custom', ...custom }
      : (presets?.envelopes.find((e) => e.id === id) ?? { id, ...custom });
  const at: [number, number] | null =
    place.x !== '' && place.y !== '' ? [Number(place.x), Number(place.y)] : null;
  const ok = async () => {
    const r = await run({ cmd: 'mergeEnvelope', size, delivery, returnAddress: ret, at });
    if (!r || r.error) {
      setError(r?.error ?? t('mailMerge.failed'));
      return;
    }
    await onDone(r);
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('mailMerge.envelope.title')}
      testId="merge-envelope"
      width={520}
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => void ok()}
          okTestId="merge-envelope-ok"
          okDisabled={!delivery.trim()}
        />
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('mailMerge.envelope.size')}</span>
          <select
            data-testid="merge-envelope-size"
            value={id}
            onChange={(e) => setId(e.target.value)}
          >
            {(presets?.envelopes ?? []).map((e) => (
              <option key={e.id} value={e.id}>
                {t(`mailMerge.envelope.preset.${e.id}`, {
                  w: mmText(e.width),
                  h: mmText(e.height),
                })}
              </option>
            ))}
            <option value="custom">{t('mailMerge.custom')}</option>
          </select>
        </label>
        {id === 'custom' && (
          <div>
            <input
              type="number"
              aria-label={t('mailMerge.width')}
              data-testid="merge-envelope-width"
              value={custom.width}
              onChange={(e) => setCustom((c) => ({ ...c, width: Number(e.target.value) }))}
            />
            <input
              type="number"
              aria-label={t('mailMerge.height')}
              data-testid="merge-envelope-height"
              value={custom.height}
              onChange={(e) => setCustom((c) => ({ ...c, height: Number(e.target.value) }))}
            />
          </div>
        )}
        <label className={styles.field}>
          <span>{t('mailMerge.envelope.delivery')}</span>
          <textarea
            rows={4}
            data-testid="merge-envelope-delivery"
            value={delivery}
            onChange={(e) => setDelivery(e.target.value)}
          />
          <FieldPicker
            fields={fields}
            testId="merge-envelope-field"
            onPick={(f) => setDelivery((d) => withField(d, f))}
          />
        </label>
        <label className={styles.field}>
          <span>{t('mailMerge.envelope.return')}</span>
          <textarea
            rows={3}
            data-testid="merge-envelope-return"
            value={ret}
            onChange={(e) => setRet(e.target.value)}
          />
        </label>
        <fieldset className={styles.field}>
          <legend>{t('mailMerge.envelope.position')}</legend>
          <input
            type="number"
            aria-label={t('mailMerge.envelope.fromLeft')}
            placeholder={t('mailMerge.auto')}
            data-testid="merge-envelope-x"
            value={place.x}
            onChange={(e) => setPlace((p) => ({ ...p, x: e.target.value }))}
          />
          <input
            type="number"
            aria-label={t('mailMerge.envelope.fromTop')}
            placeholder={t('mailMerge.auto')}
            data-testid="merge-envelope-y"
            value={place.y}
            onChange={(e) => setPlace((p) => ({ ...p, y: e.target.value }))}
          />
        </fieldset>
        <p>{t('mailMerge.replacesDocument')}</p>
        <ErrorLine error={error} testId="merge-envelope-error" />
      </div>
    </Dialog>
  );
}

const GEOMETRY: (keyof LabelSheet)[] = [
  'pageWidth',
  'pageHeight',
  'rows',
  'cols',
  'top',
  'left',
  'labelWidth',
  'labelHeight',
  'hPitch',
  'vPitch',
];

/** Replace the document by a label sheet (one record per label, or a full page). */
export function LabelsDialog({
  fields,
  run,
  onDone,
  onClose,
}: {
  fields: string[];
  run: Run;
  onDone: (res: EditResult) => Promise<void> | void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const presets = usePresets(run);
  const [id, setId] = useState('a4-3x8');
  const [sheet, setSheet] = useState<LabelSheet | null>(null);
  const [text, setText] = useState(fields[0] ? `«${fields[0]}»` : '');
  const [mode, setMode] = useState<LabelMode>('perRecord');
  const [error, setError] = useState<string | null>(null);
  const chosen = useMemo(
    () => presets?.labels.find((l) => l.id === id) ?? presets?.labels[0] ?? null,
    [presets, id],
  );
  const current = id === 'custom' ? sheet : chosen;
  const pick = (v: string) => {
    setId(v);
    if (v === 'custom' && chosen) setSheet({ ...chosen, id: 'custom' });
  };
  const ok = async () => {
    if (!current) return;
    const r = await run({ cmd: 'mergeLabels', sheet: current, text, mode });
    if (!r || r.error) {
      setError(r?.error ?? t('mailMerge.failed'));
      return;
    }
    await onDone(r);
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('mailMerge.labels.title')}
      testId="merge-labels"
      width={560}
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => void ok()}
          okTestId="merge-labels-ok"
          okDisabled={!current}
        />
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('mailMerge.labels.sheet')}</span>
          <select
            data-testid="merge-labels-sheet"
            value={id}
            onChange={(e) => pick(e.target.value)}
          >
            {(presets?.labels ?? []).map((l) => (
              <option key={l.id} value={l.id}>
                {labelPresetName(l)}
              </option>
            ))}
            <option value="custom">{t('mailMerge.custom')}</option>
          </select>
        </label>
        {id === 'custom' && sheet && (
          <div className={styles.formGrid} data-testid="merge-labels-geometry">
            {GEOMETRY.map((k) => (
              <label key={k} className={styles.field}>
                <span>{t(`mailMerge.labels.${k}`)}</span>
                <input
                  type="number"
                  data-testid={`merge-labels-${k}`}
                  value={sheet[k] as number}
                  onChange={(e) => setSheet((s) => (s ? { ...s, [k]: Number(e.target.value) } : s))}
                />
              </label>
            ))}
          </div>
        )}
        {current && (
          <p data-testid="merge-labels-per-page">
            {t('mailMerge.labels.perPage', { count: current.rows * current.cols })}
          </p>
        )}
        <label className={styles.field}>
          <span>{t('mailMerge.labels.text')}</span>
          <textarea
            rows={4}
            data-testid="merge-labels-text"
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
          <FieldPicker
            fields={fields}
            testId="merge-labels-field"
            onPick={(f) => setText((x) => withField(x, f))}
          />
        </label>
        <label>
          <input
            type="radio"
            name="merge-label-mode"
            data-testid="merge-labels-per-record"
            checked={mode === 'perRecord'}
            onChange={() => setMode('perRecord')}
          />
          <span>{t('mailMerge.labels.perRecord')}</span>
        </label>
        <label>
          <input
            type="radio"
            name="merge-label-mode"
            data-testid="merge-labels-same"
            checked={mode === 'same'}
            onChange={() => setMode('same')}
          />
          <span>{t('mailMerge.labels.same')}</span>
        </label>
        <p>{t('mailMerge.replacesDocument')}</p>
        <ErrorLine error={error} testId="merge-labels-error" />
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- data provider

/** Recipients from the host's data provider (e.g. a SQL query run by the server). */
export function ProviderDialog({
  provider,
  run,
  onLoaded,
  onClose,
}: {
  provider: MergeDataProvider;
  run: Run;
  onLoaded: (res: EditResult) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [sources, setSources] = useState<MergeSourceInfo[] | null>(null);
  const [id, setId] = useState('');
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    provider
      .sources()
      .then((s) => {
        setSources(s);
        setId(s[0]?.id ?? '');
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  }, [provider]);
  const source = sources?.find((s) => s.id === id);
  const ok = async () => {
    setBusy(true);
    setError(null);
    try {
      const table = await provider.load(id, source?.query ? query : undefined);
      const r = await run({ cmd: 'mergeTable', ...table });
      if (!r || r.error) throw new Error(r?.error ?? t('mailMerge.failed'));
      onLoaded(r);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('mailMerge.provider.title')}
      testId="merge-provider"
      width={520}
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => void ok()}
          okTestId="merge-provider-ok"
          okDisabled={busy || !id || (!!source?.query && !query.trim())}
        />
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('mailMerge.provider.source')}</span>
          <select
            data-testid="merge-provider-source"
            value={id}
            onChange={(e) => setId(e.target.value)}
          >
            {(sources ?? []).map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </select>
        </label>
        {source?.query && (
          <label className={styles.field}>
            <span>{t('mailMerge.provider.query')}</span>
            <textarea
              rows={5}
              data-testid="merge-provider-query"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </label>
        )}
        {sources && sources.length === 0 && <p>{t('mailMerge.provider.none')}</p>}
        <ErrorLine error={error} testId="merge-provider-error" />
      </div>
    </Dialog>
  );
}
