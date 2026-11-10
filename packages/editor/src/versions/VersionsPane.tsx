import { useTranslation } from '@blueoffice/i18n';
import type { FileVersion, OpenedFile, VersionHistory } from '@blueoffice/platform';
import { useCallback, useEffect, useMemo, useState } from 'react';
import styles from './VersionsPane.module.css';

/** One change listed by the pane (built by each editor from its engine's diff). */
export interface ChangeRow<T = unknown> {
  id: string;
  /** Group heading: sheet name, "Slide 3" … */
  group: string;
  /** What changed ("Value changed", "Rows inserted" …). */
  kind: string;
  /** Where: `B4`, `5:7`, a shape name; may be empty. */
  where: string;
  old?: string;
  new?: string;
  /** Passed back to `onSelect` (cell, slide, shape …). */
  target: T;
}

/** What a comparison found; `null`: shown in the document itself (BlueDoc tracked changes). */
export type CompareOutcome<T> = { rows: ChangeRow<T>[]; total: number; truncated: boolean } | null;

/** What the open file is compared with. */
type Source = { kind: 'version'; version: FileVersion } | { kind: 'file'; name: string };

/** Short, locale-formatted date-time of a version (empty when unknown or invalid). */
export function versionDate(iso: string | null | undefined, locale: string): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  return d.toLocaleString(locale, { dateStyle: 'short', timeStyle: 'short' });
}

/**
 * Version history and Show Changes, shared by BlueDoc, BlueSheet and BlueSlide
 * (CORE-VERSION-001): the host's versions (author, time, note), newest first;
 * compare the open file with one of them — or with any file — and list the
 * changes grouped (sheet / slide); restore a version when the session can edit.
 * Non-modal; Escape closes.
 */
export function VersionsPane<T>({
  versions,
  openFile,
  compare,
  onSelect,
  onClose,
  title,
  embedded = false,
}: {
  versions?: VersionHistory | undefined;
  /** Picks another file to compare with (absent: versions only). */
  openFile?: () => Promise<OpenedFile | null>;
  compare: (bytes: Uint8Array, name: string) => Promise<CompareOutcome<T>>;
  onSelect?: (row: ChangeRow<T>) => void;
  onClose: () => void;
  title?: string;
  /** Shown in a task pane (CORE-UI-006): the host draws the title bar and close button. */
  embedded?: boolean;
}) {
  const { t, i18n } = useTranslation();
  const [list, setList] = useState<FileVersion[] | null>(null);
  const [listError, setListError] = useState(false);
  const [source, setSource] = useState<Source | null>(null);
  const [outcome, setOutcome] = useState<CompareOutcome<T> | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [confirm, setConfirm] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  useEffect(() => {
    if (!versions) return;
    let live = true;
    versions.list().then(
      (l) => live && setList(l),
      () => live && setListError(true),
    );
    return () => {
      live = false;
    };
  }, [versions]);

  const run = useCallback(
    async (src: Source, load: () => Promise<{ bytes: Uint8Array; name: string } | null>) => {
      setBusy(true);
      setError(false);
      try {
        const f = await load();
        if (!f) return;
        setSource(src);
        setOutcome(await compare(f.bytes, f.name));
        setFilter('');
      } catch (e) {
        console.error(e);
        setError(true);
      } finally {
        setBusy(false);
      }
    },
    [compare],
  );

  const compareVersion = (v: FileVersion) => {
    if (!versions) return;
    void run({ kind: 'version', version: v }, async () => ({
      bytes: await versions.contents(v.version),
      name: versions.fileName,
    }));
  };
  const compareFile = () => {
    if (!openFile) return;
    openFile().then(
      (f) => (f ? run({ kind: 'file', name: f.name }, async () => f) : undefined),
      (e: unknown) => {
        console.error(e);
        setError(true);
      },
    );
  };
  const restore = async (v: string) => {
    if (!versions) return;
    setConfirm(null);
    setBusy(true);
    try {
      await versions.restore(v);
    } catch (e) {
      console.error(e);
      setError(true);
    } finally {
      setBusy(false);
    }
  };

  const shown = useMemo(() => {
    const rows = outcome?.rows ?? [];
    const q = filter.trim().toLocaleLowerCase();
    if (!q) return rows;
    return rows.filter((r) =>
      [r.group, r.kind, r.where, r.old ?? '', r.new ?? ''].some((s) =>
        s.toLocaleLowerCase().includes(q),
      ),
    );
  }, [outcome, filter]);
  const groups = useMemo(() => {
    const m = new Map<string, ChangeRow<T>[]>();
    for (const r of shown) {
      const g = m.get(r.group);
      if (g) g.push(r);
      else m.set(r.group, [r]);
    }
    return [...m];
  }, [shown]);

  const who = (v: FileVersion) =>
    [v.modifiedBy?.name || t('versions.unknownAuthor'), versionDate(v.modifiedAt, i18n.language)]
      .filter(Boolean)
      .join(' · ');
  const canRestore = versions?.canRestore() ?? false;

  return (
    <div
      className={embedded ? styles.embedded : styles.panel}
      role={embedded ? undefined : 'dialog'}
      aria-label={embedded ? undefined : (title ?? t('versions.title'))}
      data-testid="versions-pane"
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Escape') {
          e.preventDefault();
          onClose();
        }
      }}
    >
      {!embedded && (
        <div className={styles.header}>
          <strong>{title ?? t('versions.title')}</strong>
          <button type="button" aria-label={t('versions.close')} onClick={onClose}>
            ×
          </button>
        </div>
      )}

      <section aria-label={t('versions.history')}>
        <div className={styles.sectionTitle}>{t('versions.history')}</div>
        {!versions ? (
          <div className={styles.note}>{t('versions.unavailable')}</div>
        ) : listError ? (
          <div className={styles.error}>{t('versions.listError')}</div>
        ) : list === null ? (
          <div className={styles.note}>{t('versions.loading')}</div>
        ) : list.length === 0 ? (
          <div className={styles.note}>{t('versions.empty')}</div>
        ) : (
          <ul className={styles.versions}>
            {list.map((v, i) => (
              <li
                key={v.version}
                className={styles.version}
                data-testid="version-item"
                aria-current={
                  source?.kind === 'version' && source.version.version === v.version
                    ? 'true'
                    : undefined
                }
              >
                <div className={styles.versionHead}>
                  <span className={styles.versionNo}>
                    {t('versions.version', { version: v.version })}
                    {i === 0 ? ` (${t('versions.current')})` : ''}
                  </span>
                  <span className={styles.meta}>{who(v)}</span>
                </div>
                {v.note ? <div className={styles.meta}>{v.note}</div> : null}
                <div className={styles.actions}>
                  <button
                    type="button"
                    disabled={busy}
                    data-testid="version-compare"
                    onClick={() => compareVersion(v)}
                  >
                    {t('versions.compare')}
                  </button>
                  {canRestore && i > 0 ? (
                    confirm === v.version ? (
                      <>
                        <span className={styles.meta}>
                          {t('versions.restoreConfirm', { version: v.version })}
                        </span>
                        <button
                          type="button"
                          data-testid="version-restore-yes"
                          onClick={() => void restore(v.version)}
                        >
                          {t('versions.restoreYes')}
                        </button>
                        <button type="button" onClick={() => setConfirm(null)}>
                          {t('versions.cancel')}
                        </button>
                      </>
                    ) : (
                      <button
                        type="button"
                        disabled={busy}
                        data-testid="version-restore"
                        onClick={() => setConfirm(v.version)}
                      >
                        {t('versions.restore')}
                      </button>
                    )
                  ) : null}
                </div>
              </li>
            ))}
          </ul>
        )}
        {openFile ? (
          <button
            type="button"
            className={styles.fileButton}
            disabled={busy}
            data-testid="versions-compare-file"
            onClick={compareFile}
          >
            {t('versions.compareFile')}
          </button>
        ) : null}
      </section>

      {busy ? <div className={styles.note}>{t('versions.comparing')}</div> : null}
      {error ? (
        <div className={styles.error} role="alert">
          {t('versions.error')}
        </div>
      ) : null}

      {source && outcome !== undefined && !busy ? (
        <section aria-label={t('versions.changes')} data-testid="versions-changes">
          <div className={styles.sectionTitle}>
            {source.kind === 'version'
              ? t('versions.since', { version: source.version.version })
              : t('versions.sinceFile', { name: source.name })}
          </div>
          {source.kind === 'version' ? (
            <div className={styles.meta}>{who(source.version)}</div>
          ) : null}
          {outcome === null ? (
            <div className={styles.note}>{t('versions.shownInDocument')}</div>
          ) : outcome.total === 0 ? (
            <div className={styles.note}>{t('versions.noChanges')}</div>
          ) : (
            <>
              <div className={styles.count}>
                {outcome.truncated
                  ? t('versions.truncated', { shown: outcome.rows.length, total: outcome.total })
                  : t('versions.count', { count: outcome.total })}
              </div>
              <input
                className={styles.filter}
                type="search"
                value={filter}
                placeholder={t('versions.filter')}
                aria-label={t('versions.filter')}
                onChange={(e) => setFilter(e.target.value)}
              />
              {groups.map(([g, rows]) => (
                <div key={g} className={styles.group}>
                  <div className={styles.groupTitle}>{g}</div>
                  <ul className={styles.changes}>
                    {rows.map((r) => (
                      <li key={r.id}>
                        <button
                          type="button"
                          className={styles.change}
                          data-testid="change-item"
                          onClick={() => onSelect?.(r)}
                        >
                          <span className={styles.changeHead}>
                            <span className={styles.kind}>{r.kind}</span>
                            {r.where ? <span className={styles.where}>{r.where}</span> : null}
                          </span>
                          {r.old !== undefined || r.new !== undefined ? (
                            <span className={styles.values}>
                              {r.old !== undefined ? (
                                <del className={styles.old}>{r.old}</del>
                              ) : null}
                              {r.old !== undefined && r.new !== undefined ? ' → ' : null}
                              {r.new !== undefined ? (
                                <ins className={styles.new}>{r.new}</ins>
                              ) : null}
                            </span>
                          ) : null}
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </>
          )}
        </section>
      ) : null}
    </div>
  );
}
