import { useTranslation } from '@blueoffice/i18n';
import type {
  A11yIssue,
  A11ySeverity,
  CompatFormat,
  InspectCategory,
  InspectFix,
  InspectReport,
  InspectTarget,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import styles from './Editor.module.css';
import cd from './CheckDocument.module.css';

/** The three checkers of the "Check document" dialog. */
export type CheckTab = 'inspector' | 'accessibility' | 'compatibility';

/** What a host app plugs into the dialog (CORE-INSPECT-001). */
export interface CheckDocumentBridge {
  /** Run `inspect`. */
  inspect: () => Promise<InspectReport | null>;
  /** Run `inspectRemove` for the categories. */
  remove: (categories: InspectCategory[]) => Promise<InspectReport | null>;
  /** Run `inspectKeep` (cancel a removal queued for the next save). */
  keep: (categories: InspectCategory[]) => Promise<InspectReport | null>;
  /** Apply a quick fix; absent when the app has none. */
  fix?: (fix: InspectFix) => Promise<InspectReport | null>;
  /** Accept every tracked change (BlueDoc). */
  acceptAll?: () => Promise<InspectReport | null>;
  /** Select / scroll to the object of an issue. */
  goTo?: (target: InspectTarget) => void;
}

const SEVERITIES: A11ySeverity[] = ['error', 'warning', 'tip'];

/** Inline text editor of one issue (alternative text, or a new sheet name). */
function AltTextFix({
  onApply,
  rename = false,
}: {
  onApply: (text: string) => void;
  rename?: boolean;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState('');
  return (
    <span className={cd.fixRow}>
      <input
        className={cd.input}
        value={text}
        placeholder={t(rename ? 'inspect.renamePlaceholder' : 'inspect.altTextPlaceholder')}
        aria-label={t(rename ? 'inspect.renameLabel' : 'inspect.altTextLabel')}
        data-testid={rename ? 'inspect-rename-input' : 'inspect-alt-input'}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && text.trim()) onApply(text.trim());
        }}
      />
      <button
        type="button"
        className={styles.dialogButton}
        disabled={!text.trim()}
        data-testid="inspect-alt-apply"
        onClick={() => onApply(text.trim())}
      >
        {t('inspect.apply')}
      </button>
    </span>
  );
}

/**
 * "Check document": document inspector (find and remove hidden or personal
 * content), accessibility checker and compatibility checker, shared by BlueDoc,
 * BlueSheet and BlueSlide.
 */
export function CheckDocumentDialog({
  bridge,
  onClose,
  initialTab = 'inspector',
}: {
  bridge: CheckDocumentBridge;
  onClose: () => void;
  initialTab?: CheckTab;
}) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<CheckTab>(initialTab);
  const [report, setReport] = useState<InspectReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<number | null>(null);
  // After "Go to" the checker becomes a side pane: the document shows the object and
  // stays usable while the list stays open (as the accessibility pane of word processors).
  const [docked, setDocked] = useState(false);

  const apply = useCallback(async (p: Promise<InspectReport | null>) => {
    setBusy(true);
    try {
      const r = await p;
      if (r) setReport(r);
    } finally {
      setBusy(false);
    }
  }, []);

  // Check once when the dialog opens (hosts may pass a new bridge on every render).
  const first = useRef(bridge);
  useEffect(() => {
    void apply(first.current.inspect());
  }, [apply]);

  const removable = (report?.inspector ?? []).filter(
    (f) => f.count > 0 && (f.action === 'remove' || (f.action === 'onSave' && !f.pending)),
  );

  const tabs: CheckTab[] = ['inspector', 'accessibility', 'compatibility'];

  const inspector = (
    <div role="tabpanel" data-testid="inspect-panel-inspector">
      <p className={cd.intro}>{t('inspect.inspectorIntro')}</p>
      <ul className={cd.list}>
        {(report?.inspector ?? []).map((f) => (
          <li key={f.category} className={cd.item} data-testid={`inspect-cat-${f.category}`}>
            <div className={cd.itemHead}>
              <span className={cd.itemTitle}>{t(`inspect.category.${f.category}`)}</span>
              <span className={f.count > 0 ? cd.found : cd.clean}>
                {f.count > 0 ? t('inspect.found', { count: f.count }) : t('inspect.noneFound')}
              </span>
            </div>
            {f.count > 0 && f.details.length > 0 && (
              <div className={cd.details}>{f.details.join(' · ')}</div>
            )}
            {f.count > 0 && f.pending && (
              <div className={cd.note}>{t('inspect.pendingOnSave')}</div>
            )}
            {f.count > 0 && (
              <div className={cd.actions}>
                {f.action === 'remove' && (
                  <button
                    type="button"
                    className={styles.dialogButton}
                    disabled={busy}
                    data-testid={`inspect-remove-${f.category}`}
                    onClick={() => void apply(bridge.remove([f.category]))}
                  >
                    {t('inspect.removeAll')}
                  </button>
                )}
                {f.action === 'onSave' && !f.pending && (
                  <button
                    type="button"
                    className={styles.dialogButton}
                    disabled={busy}
                    data-testid={`inspect-remove-${f.category}`}
                    onClick={() => void apply(bridge.remove([f.category]))}
                  >
                    {t('inspect.removeOnSave')}
                  </button>
                )}
                {f.action === 'onSave' && f.pending && (
                  <button
                    type="button"
                    className={styles.dialogButton}
                    disabled={busy}
                    data-testid={`inspect-keep-${f.category}`}
                    onClick={() => void apply(bridge.keep([f.category]))}
                  >
                    {t('inspect.keep')}
                  </button>
                )}
                {f.action === 'acceptAll' && bridge.acceptAll && (
                  <button
                    type="button"
                    className={styles.dialogButton}
                    disabled={busy}
                    data-testid={`inspect-accept-${f.category}`}
                    onClick={() => bridge.acceptAll && void apply(bridge.acceptAll())}
                  >
                    {t('inspect.acceptAll')}
                  </button>
                )}
                {f.action === 'none' && <span className={cd.note}>{t('inspect.reportOnly')}</span>}
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );

  const issue = (i: A11yIssue, n: number) => {
    const canGo = bridge.goTo && i.target.kind !== 'document';
    return (
      <li key={n} className={cd.item} data-testid={`inspect-issue-${i.rule}`}>
        <div className={cd.itemHead}>
          <span className={cd.itemTitle}>{t(`inspect.rule.${i.rule}.title`)}</span>
          {i.ratio !== undefined && (
            <span className={cd.found}>{t('inspect.ratio', { ratio: i.ratio.toFixed(2) })}</span>
          )}
        </div>
        {i.detail && <div className={cd.details}>{i.detail}</div>}
        <div className={cd.hint}>{t(`inspect.rule.${i.rule}.hint`)}</div>
        <div className={cd.actions}>
          {canGo && (
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="inspect-goto"
              onClick={() => {
                bridge.goTo?.(i.target);
                setDocked(true);
              }}
            >
              {t('inspect.goTo')}
            </button>
          )}
          {(i.fix === 'altText' || i.fix === 'rename') && bridge.fix && editing !== n && (
            <button
              type="button"
              className={styles.dialogButton}
              disabled={busy}
              data-testid={`inspect-fix-${i.fix}`}
              onClick={() => setEditing(n)}
            >
              {t(`inspect.fix.${i.fix}`)}
            </button>
          )}
          {i.fix === 'unmerge' && bridge.fix && (
            <button
              type="button"
              className={styles.dialogButton}
              disabled={busy}
              data-testid="inspect-fix-unmerge"
              onClick={() =>
                bridge.fix && void apply(bridge.fix({ kind: 'unmerge', target: i.target }))
              }
            >
              {t('inspect.fix.unmerge')}
            </button>
          )}
          {i.fix === 'headerRow' && bridge.fix && (
            <button
              type="button"
              className={styles.dialogButton}
              disabled={busy}
              data-testid="inspect-fix-headerRow"
              onClick={() =>
                bridge.fix && void apply(bridge.fix({ kind: 'headerRow', target: i.target }))
              }
            >
              {t('inspect.fix.headerRow')}
            </button>
          )}
        </div>
        {editing === n && bridge.fix && (
          <AltTextFix
            rename={i.fix === 'rename'}
            onApply={(text) => {
              setEditing(null);
              const kind = i.fix === 'rename' ? 'rename' : 'altText';
              if (bridge.fix) void apply(bridge.fix({ kind, target: i.target, text }));
            }}
          />
        )}
      </li>
    );
  };

  const issues = report?.accessibility ?? [];
  const accessibility = (
    <div role="tabpanel" data-testid="inspect-panel-accessibility">
      {report && issues.length === 0 ? (
        <p className={cd.intro}>{t('inspect.noIssues')}</p>
      ) : (
        SEVERITIES.map((sev) => {
          const list = issues.map((i, n) => [i, n] as const).filter(([i]) => i.severity === sev);
          if (list.length === 0) return null;
          return (
            <section key={sev} className={cd.group} data-testid={`inspect-severity-${sev}`}>
              <h3 className={cd.groupTitle}>
                {t(`inspect.severity.${sev}`)} ({list.length})
              </h3>
              <ul className={cd.list}>{list.map(([i, n]) => issue(i, n))}</ul>
            </section>
          );
        })
      )}
      {report && report.issueCount > issues.length && (
        <p className={cd.note}>
          {t('inspect.moreIssues', { count: report.issueCount - issues.length })}
        </p>
      )}
    </div>
  );

  const compat = report?.compatibility ?? [];
  const compatibility = (
    <div role="tabpanel" data-testid="inspect-panel-compatibility">
      <p className={cd.intro}>{t('inspect.compatIntro')}</p>
      {report && compat.length === 0 ? (
        <p className={cd.intro}>{t('inspect.compatNone')}</p>
      ) : (
        <ul className={cd.list}>
          {compat.map((c) => (
            <li key={c.feature} className={cd.item} data-testid={`inspect-compat-${c.feature}`}>
              <div className={cd.itemHead}>
                <span className={cd.itemTitle}>{t(`inspect.feature.${c.feature}`)}</span>
                <span className={cd.found}>{t('inspect.occurrences', { count: c.count })}</span>
              </div>
              <div className={cd.chips}>
                {c.formats.map((f: CompatFormat) => (
                  <span key={f} className={cd.chip}>
                    {t(`inspect.format.${f}`)}
                  </span>
                ))}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );

  const errors = issues.filter((i) => i.severity === 'error').length;
  return (
    <Dialog
      // Docking makes a new (modeless) dialog: the modal one's focus trap goes away.
      key={docked ? 'pane' : 'dialog'}
      open
      modeless={docked}
      onOpenChange={(o) => !o && onClose()}
      title={t('inspect.title')}
      width={docked ? 380 : 560}
      testId="inspect-dialog"
      footer={
        <>
          {tab === 'inspector' && (
            <button
              type="button"
              className={styles.dialogButton}
              disabled={busy || removable.length === 0}
              data-testid="inspect-remove-everything"
              onClick={() => void apply(bridge.remove(removable.map((f) => f.category)))}
            >
              {t('inspect.removeEverything')}
            </button>
          )}
          <button
            type="button"
            className={styles.dialogButton}
            disabled={busy}
            data-testid="inspect-refresh"
            onClick={() => void apply(bridge.inspect())}
          >
            {t('inspect.recheck')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            onClick={onClose}
            data-testid="inspect-close"
          >
            {t('inspect.close')}
          </button>
        </>
      }
    >
      <div role="tablist" className={cd.tabs} aria-label={t('inspect.title')}>
        {tabs.map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            className={tab === k ? cd.tabOn : cd.tab}
            data-testid={`inspect-tab-${k}`}
            onClick={() => setTab(k)}
          >
            {t(`inspect.tab.${k}`)}
            {k === 'accessibility' && report && report.issueCount > 0 && (
              <span className={errors > 0 ? cd.badgeError : cd.badge}>{report.issueCount}</span>
            )}
          </button>
        ))}
      </div>
      <div className={cd.body} aria-busy={busy}>
        {!report && <p className={cd.intro}>{t('inspect.checking')}</p>}
        {report && tab === 'inspector' && inspector}
        {report && tab === 'accessibility' && accessibility}
        {report && tab === 'compatibility' && compatibility}
      </div>
    </Dialog>
  );
}
