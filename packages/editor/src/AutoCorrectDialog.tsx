import { useTranslation } from '@blueoffice/i18n';
import type {
  AutoCorrectEntry,
  AutoCorrectOptions,
  EditCommand,
  EditResult,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState } from 'react';
import ac from './AutoCorrectDialog.module.css';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

const STORAGE_KEY = 'bo.doc.autocorrect';

/** On/off rules shown as check boxes, in dialog order. */
export const AUTOCORRECT_RULES = [
  'capitalizeSentences',
  'capitalizeCells',
  'twoInitialCaps',
  'smartQuotes',
  'dashes',
  'ordinals',
  'fractions',
  'autoLists',
  'autoLinks',
  'replaceText',
] as const satisfies readonly (keyof AutoCorrectOptions)[];

/** AutoCorrect options kept by this browser / app (null: never changed → engine defaults). */
export function loadAutoCorrect(): AutoCorrectOptions | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const v: unknown = raw ? JSON.parse(raw) : null;
    return v && typeof v === 'object' && Array.isArray((v as AutoCorrectOptions).entries)
      ? (v as AutoCorrectOptions)
      : null;
  } catch {
    return null;
  }
}

/** Keep the options (`null` forgets them, so the engine defaults apply next time). */
export function saveAutoCorrect(options: AutoCorrectOptions | null): void {
  try {
    if (options) localStorage.setItem(STORAGE_KEY, JSON.stringify(options));
    else localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Storage unavailable (private mode): the options last for this session only.
  }
}

/** Command that hands the kept options (or the defaults) to the engine. */
export function autoCorrectCommand(options: AutoCorrectOptions | null): EditCommand {
  return options ? { cmd: 'autoCorrect', options } : { cmd: 'autoCorrect', reset: true };
}

/** The document-open set-up (CORE-MISC-003): author, identity names, AutoCorrect (the
 * kept options, else the engine defaults), state and — `fonts` — the font list. */
export function openSetupCommand(
  author: string,
  names: string[],
  autoCorrect: AutoCorrectOptions | null,
  fonts: boolean,
): EditCommand {
  return {
    cmd: 'openSetup',
    author,
    names,
    ...(autoCorrect ? { autoCorrect } : { resetAutoCorrect: true }),
    fonts,
  };
}

/** Entries of replacement list `list` ('' = every language, else a language tag). */
export function listEntries(o: AutoCorrectOptions, list: string): AutoCorrectEntry[] {
  return list ? (o.languageEntries?.[list] ?? []) : o.entries;
}

/** `o` with replacement list `list` set to `entries`. */
export function withListEntries(
  o: AutoCorrectOptions,
  list: string,
  entries: AutoCorrectEntry[],
): Partial<AutoCorrectOptions> {
  return list ? { languageEntries: { ...o.languageEntries, [list]: entries } } : { entries };
}

const lines = (s: string) =>
  s
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean);

/** AutoCorrect options: rules, replacement list and exceptions (CORE-AUTOCORR-001). */
export function AutoCorrectDialog({
  run,
  onClose,
  onSaved,
}: {
  run: Run;
  onClose: () => void;
  /** The options the user confirmed (`null` after restoring the defaults). */
  onSaved?: (options: AutoCorrectOptions | null) => void;
}) {
  const { t } = useTranslation();
  const [opts, setOpts] = useState<AutoCorrectOptions | null>(null);
  const [exceptions, setExceptions] = useState('');
  const [caps, setCaps] = useState('');
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [isDefault, setIsDefault] = useState(false);
  // Replacement list being edited: '' = every language, else a language tag.
  const [list, setList] = useState('');

  const show = (o: AutoCorrectOptions) => {
    setOpts(o);
    setExceptions(o.exceptions.join('\n'));
    setCaps(o.capsExceptions.join('\n'));
  };

  useEffect(() => {
    let live = true;
    void run({ cmd: 'autoCorrect' }).then((r) => {
      if (live && r?.autocorrect) show(r.autocorrect);
    });
    return () => {
      live = false;
    };
  }, [run]);

  const patch = (p: Partial<AutoCorrectOptions>) => {
    setIsDefault(false);
    setOpts((o) => (o ? { ...o, ...p } : o));
  };

  const add = () => {
    const f = from.trim();
    if (!opts || !f || !to) return;
    const rest = listEntries(opts, list).filter((e) => e.from !== f);
    patch(withListEntries(opts, list, [...rest, { from: f, to }]));
    setFrom('');
    setTo('');
  };

  const reset = async () => {
    const r = await run({ cmd: 'autoCorrect', reset: true });
    if (r?.autocorrect) show(r.autocorrect);
    setIsDefault(true);
  };

  const ok = async () => {
    if (!opts) return;
    const next: AutoCorrectOptions = {
      ...opts,
      exceptions: lines(exceptions),
      capsExceptions: lines(caps),
    };
    await run({ cmd: 'autoCorrect', options: next });
    const kept = isDefault ? null : next;
    saveAutoCorrect(kept);
    onSaved?.(kept);
    onClose();
  };

  // Matching entries when the "typed" box has text (the list is long).
  const entries = opts ? listEntries(opts, list) : [];
  const shown = entries.filter((e) => !from.trim() || e.from.includes(from.trim()));
  const tags = Object.keys(opts?.languageEntries ?? {}).sort();

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('autoCorrect.title')}
      testId="autocorrect-dialog"
      width={620}
      footer={
        <>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="autocorrect-reset"
            onClick={() => void reset()}
          >
            {t('autoCorrect.reset')}
          </button>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('autoCorrect.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="autocorrect-ok"
            disabled={!opts}
            onClick={() => void ok()}
          >
            {t('autoCorrect.ok')}
          </button>
        </>
      }
    >
      {opts && (
        <>
          <label className={ac.section}>
            <input
              type="checkbox"
              data-testid="autocorrect-enabled"
              checked={opts.enabled}
              onChange={(e) => patch({ enabled: e.target.checked })}
            />{' '}
            {t('autoCorrect.enabled')}
          </label>
          <fieldset className={ac.rules}>
            <legend>{t('autoCorrect.rules')}</legend>
            {AUTOCORRECT_RULES.map((k) => (
              <label key={k}>
                <input
                  type="checkbox"
                  data-testid={`autocorrect-${k}`}
                  checked={opts[k]}
                  disabled={!opts.enabled}
                  onChange={(e) => patch({ [k]: e.target.checked })}
                />
                <span>{t(`autoCorrect.${k}`)}</span>
              </label>
            ))}
          </fieldset>
          <div className={ac.section}>
            {t('autoCorrect.entries')}{' '}
            <span className={ac.count}>{t('autoCorrect.count', { count: entries.length })}</span>
            {tags.length > 0 && (
              <label className={ac.language}>
                {t('autoCorrect.language')}{' '}
                <select
                  data-testid="autocorrect-language"
                  value={list}
                  onChange={(e) => setList(e.target.value)}
                >
                  <option value="">{t('autoCorrect.allLanguages')}</option>
                  {tags.map((tag) => (
                    <option key={tag} value={tag}>
                      {t(`autoCorrect.lang.${tag}`, { defaultValue: tag })}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>
          <div className={ac.addRow}>
            <input
              aria-label={t('autoCorrect.from')}
              placeholder={t('autoCorrect.from')}
              data-testid="autocorrect-from"
              value={from}
              onChange={(e) => setFrom(e.target.value)}
            />
            <input
              aria-label={t('autoCorrect.to')}
              placeholder={t('autoCorrect.to')}
              data-testid="autocorrect-to"
              value={to}
              onChange={(e) => setTo(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && add()}
            />
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="autocorrect-add"
              disabled={!from.trim() || !to}
              onClick={add}
            >
              {t('autoCorrect.add')}
            </button>
          </div>
          <ul className={ac.entries} data-testid="autocorrect-entries">
            {shown.map((e) => (
              <li key={e.from}>
                <span>{e.from}</span>
                <span>{e.to}</span>
                <button
                  type="button"
                  aria-label={`${t('autoCorrect.remove')} ${e.from}`}
                  onClick={() =>
                    patch(
                      withListEntries(
                        opts,
                        list,
                        entries.filter((x) => x.from !== e.from),
                      ),
                    )
                  }
                >
                  {t('autoCorrect.remove')}
                </button>
              </li>
            ))}
          </ul>
          <div className={ac.lists}>
            <label>
              {t('autoCorrect.exceptions')}
              <textarea
                data-testid="autocorrect-exceptions"
                value={exceptions}
                onChange={(e) => {
                  setIsDefault(false);
                  setExceptions(e.target.value);
                }}
              />
            </label>
            <label>
              {t('autoCorrect.capsExceptions')}
              <textarea
                data-testid="autocorrect-caps"
                value={caps}
                onChange={(e) => {
                  setIsDefault(false);
                  setCaps(e.target.value);
                }}
              />
            </label>
          </div>
        </>
      )}
    </Dialog>
  );
}
