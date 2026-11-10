/**
 * Style Inspector and Reveal Formatting task panes (DOC-STYLE-001, ECMA-376 Part 1,
 * §17.7.2 style hierarchy). The Inspector shows the paragraph style plus direct
 * paragraph formatting and the character style plus direct run formatting at the
 * caret, each level with a clear button. Reveal Formatting lists every effective
 * value with where it comes from, and can compare against a remembered selection.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  revealDiff,
  type DirectValue,
  type EditCommand,
  type EditResult,
  type RevealItem,
  type StyleClearLevel,
  type StyleInspectorInfo,
} from '@blueoffice/platform';
import { useEffect, useMemo, useState } from 'react';
import edStyles from './Editor.module.css';
import styles from './StylesPane.module.css';

export interface StyleInspectorPaneProps {
  query: (c: EditCommand) => Promise<EditResult | null>;
  run: (c: EditCommand) => Promise<EditResult | null>;
  /** Changes whenever the caret or the document may have changed. */
  refreshKey: unknown;
}

/** Display text of a language-neutral value. */
function useValue() {
  const { t } = useTranslation();
  return (key: string, value: string) => {
    if (value === 'true') return t('docStyles.reveal.yes');
    if (value === 'false') return t('docStyles.reveal.no');
    if (key === 'color' || key === 'highlight' || key === 'shading') return `#${value}`;
    return value;
  };
}

function DirectList({ items }: { items: DirectValue[] }) {
  const { t } = useTranslation();
  const show = useValue();
  if (items.length === 0) return <p className={styles.note}>{t('docStyles.inspector.noDirect')}</p>;
  return (
    <ul className={styles.list} style={{ flex: 'none' }}>
      {items.map((d) => (
        <li key={d.key} data-testid="inspector-direct" data-key={d.key}>
          {t(`docStyles.reveal.keys.${d.key}`, { defaultValue: d.key })}: {show(d.key, d.value)}
        </li>
      ))}
    </ul>
  );
}

export function StyleInspectorPane({ query, run, refreshKey }: StyleInspectorPaneProps) {
  const { t } = useTranslation();
  const [info, setInfo] = useState<StyleInspectorInfo | null>(null);
  useEffect(() => {
    let live = true;
    void query({ cmd: 'styleInspector' }).then((r) => {
      if (live) setInfo(r?.styleMgr?.inspector ?? null);
    });
    return () => {
      live = false;
    };
  }, [query, refreshKey]);
  const clear = async (level: StyleClearLevel) => {
    const r = await run({ cmd: 'styleClear', level });
    if (r?.styleMgr?.inspector) setInfo(r.styleMgr.inspector);
  };
  const button = (level: StyleClearLevel, key: string, disabled: boolean) => (
    <button
      type="button"
      className={edStyles.dialogButton}
      data-testid={`inspector-clear-${level}`}
      disabled={disabled}
      onClick={() => void clear(level)}
    >
      {t(key)}
    </button>
  );
  if (!info) return <p className={styles.note}>{t('taskPane.styles.loading')}</p>;
  return (
    <div className={styles.pane} data-testid="style-inspector">
      <section className={styles.tools}>
        <strong>{t('docStyles.inspector.paragraph')}</strong>
        <span data-testid="inspector-paragraph-style">
          {info.paragraphStyle?.name ?? t('docStyles.inspector.none')}
        </span>
        {button('paragraphStyle', 'docStyles.inspector.clearParagraphStyle', false)}
        <em>{t('docStyles.inspector.direct')}</em>
        <DirectList items={info.paragraphDirect} />
        {button(
          'paragraphDirect',
          'docStyles.inspector.clearParagraphDirect',
          info.paragraphDirect.length === 0,
        )}
      </section>
      <section className={styles.tools}>
        <strong>{t('docStyles.inspector.character')}</strong>
        <span data-testid="inspector-character-style">
          {info.characterStyle?.name ?? t('docStyles.inspector.none')}
        </span>
        {button('characterStyle', 'docStyles.inspector.clearCharacterStyle', !info.characterStyle)}
        <em>{t('docStyles.inspector.direct')}</em>
        <DirectList items={info.characterDirect} />
        {button(
          'characterDirect',
          'docStyles.inspector.clearCharacterDirect',
          info.characterDirect.length === 0,
        )}
      </section>
    </div>
  );
}

export interface RevealFormattingPaneProps {
  query: (c: EditCommand) => Promise<EditResult | null>;
  refreshKey: unknown;
}

const GROUPS = ['font', 'paragraph', 'section'] as const;

export function RevealFormattingPane({ query, refreshKey }: RevealFormattingPaneProps) {
  const { t } = useTranslation();
  const show = useValue();
  const [items, setItems] = useState<RevealItem[] | null>(null);
  /** Remembered formatting of the compare mode. */
  const [base, setBase] = useState<RevealItem[] | null>(null);
  useEffect(() => {
    let live = true;
    void query({ cmd: 'revealFormatting' }).then((r) => {
      if (live) setItems(r?.styleMgr?.reveal ?? []);
    });
    return () => {
      live = false;
    };
  }, [query, refreshKey]);
  const diff = useMemo(
    () => (base && items ? revealDiff(base, items) : new Set<string>()),
    [base, items],
  );
  if (!items) return <p className={styles.note}>{t('taskPane.styles.loading')}</p>;
  const source = (i: RevealItem) =>
    t(`docStyles.reveal.sources.${i.source}`, { style: i.style ?? '' });
  return (
    <div className={styles.pane} data-testid="reveal-formatting">
      <div className={styles.tools}>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            data-testid="reveal-compare"
            checked={base !== null}
            onChange={(e) => setBase(e.target.checked ? items : null)}
          />
          {t('docStyles.reveal.compare')}
        </label>
        {base && <span className={styles.note}>{t('docStyles.reveal.compareHint')}</span>}
      </div>
      <div className={styles.list}>
        {GROUPS.map((g) => {
          const rows = items.filter((i) => i.group === g);
          if (rows.length === 0) return null;
          return (
            <section key={g}>
              <h4 style={{ margin: '6px 4px' }}>{t(`docStyles.reveal.${g}`)}</h4>
              <dl style={{ margin: 0 }}>
                {rows.map((i) => {
                  const k = `${i.group}.${i.key}`;
                  const differs = diff.has(k);
                  const before = differs ? base?.find((b) => `${b.group}.${b.key}` === k) : null;
                  return (
                    <div
                      key={k}
                      data-testid="reveal-item"
                      data-key={i.key}
                      data-source={i.source}
                      data-differs={differs || undefined}
                      title={source(i)}
                      style={{
                        display: 'flex',
                        gap: 6,
                        padding: '1px 4px',
                        background: differs
                          ? 'var(--bo-accent-subtle, rgb(47 111 214 / 10%))'
                          : undefined,
                      }}
                    >
                      <dt style={{ flex: 'none', width: 120 }}>
                        {t(`docStyles.reveal.keys.${i.key}`, { defaultValue: i.key })}
                      </dt>
                      <dd style={{ margin: 0, flex: 1 }}>
                        {show(i.key, i.value)}
                        {before && (
                          <span aria-label={t('docStyles.reveal.differs')}>
                            {' '}
                            ← {show(before.key, before.value)}
                          </span>
                        )}
                        <br />
                        <small className={styles.kind}>{source(i)}</small>
                      </dd>
                    </div>
                  );
                })}
              </dl>
            </section>
          );
        })}
      </div>
    </div>
  );
}
