/**
 * Links to other workbooks (Data ▸ Queries & connections ▸ Edit links): the list of linked
 * workbooks with Update values, Change source and Break link, like the reference
 * application's Edit Links dialog. The engine (`externalLinks`, `externalLinkUpdate`,
 * `externalLinkBreak`) keeps the links and the values last read from them (§18.14); the
 * linked file is picked by the user (a web page cannot read a path), so Update values
 * also asks for it. Each change is one undo step.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { Platform, SheetHost } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useState } from 'react';
import { toBase64 } from './queryModel';
import styles from './SheetEditor.module.css';

/** One link as the engine lists it. */
export interface ExternalLinkInfo {
  index: number;
  /** File name shown in the list. */
  source: string;
  /** The path or URL the link names. */
  target: string;
  sheets: string[];
  /** Formulas (and names) using the link. */
  formulas: number;
  /** A link to a workbook (not a DDE / OLE link, which is only kept). */
  workbook: boolean;
}

/** What the dialog knows of a link after this session's actions. */
export type LinkStatus = 'unknown' | 'ok' | 'sheetMissing';

/** Status column text key of a link. */
export function statusKey(s: LinkStatus): string {
  return `sheetLinks.status.${s}`;
}

/** Workbook files a link can name. */
export const LINK_SOURCE_EXTENSIONS = ['.xlsx', '.xlsm', '.xltx', '.xltm', '.xlsb', '.xls', '.ods'];

export function EditLinksDialog({
  host,
  platform,
  onClose,
  onDone,
  fail,
}: {
  host: SheetHost;
  platform: Platform;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [links, setLinks] = useState<ExternalLinkInfo[]>([]);
  const [picked, setPicked] = useState(0);
  const [status, setStatus] = useState<Record<number, LinkStatus>>({});
  const [confirmBreak, setConfirmBreak] = useState(false);
  const load = useCallback(async () => {
    const r = await host.exec<{ links: ExternalLinkInfo[] }>({ cmd: 'externalLinks' });
    setLinks(r?.links ?? []);
  }, [host]);
  useEffect(() => {
    void load().catch(fail);
  }, [load, fail]);
  const current = links[picked];

  const readFrom = async (changeSource: boolean) => {
    if (!current) return;
    const f = await platform.files.openFile(LINK_SOURCE_EXTENSIONS);
    if (!f) return;
    try {
      const r = await host.exec<{ missing: string[] }>({
        cmd: 'externalLinkUpdate',
        index: current.index,
        base64: toBase64(f.bytes),
        name: changeSource ? (f.path ?? f.name) : f.name,
        changeSource,
      });
      setStatus({ ...status, [current.index]: r?.missing.length ? 'sheetMissing' : 'ok' });
      await onDone();
      await load();
    } catch (e) {
      fail(e);
    }
  };

  const breakLink = async () => {
    if (!current) return;
    setConfirmBreak(false);
    try {
      await host.exec({ cmd: 'externalLinkBreak', index: current.index });
      setPicked(0);
      setStatus({});
      await onDone();
      await load();
    } catch (e) {
      fail(e);
    }
  };

  const button = (id: string, label: string, run: () => void, enabled = !!current?.workbook) => (
    <button
      type="button"
      className={styles.btn}
      data-testid={`edit-links-${id}`}
      disabled={!enabled}
      onClick={run}
    >
      {label}
    </button>
  );

  return (
    <>
      <Dialog
        open
        onOpenChange={(o) => !o && onClose()}
        title={t('sheetLinks.title')}
        testId="edit-links"
        footer={
          <button
            type="button"
            className={styles.btn}
            data-testid="edit-links-close"
            onClick={onClose}
          >
            {t('sheetNames.close')}
          </button>
        }
      >
        <table className={styles.nameTable} data-testid="edit-links-list">
          <thead>
            <tr>
              <th>{t('sheetLinks.source')}</th>
              <th>{t('sheetLinks.type')}</th>
              <th>{t('sheetLinks.update')}</th>
              <th>{t('sheetLinks.statusTitle')}</th>
            </tr>
          </thead>
          <tbody>
            {links.map((l, i) => (
              <tr
                key={l.index}
                data-source={l.source}
                aria-selected={i === picked}
                className={i === picked ? styles.nameRowPicked : undefined}
                onClick={() => setPicked(i)}
              >
                <td>{l.source}</td>
                <td>{t(l.workbook ? 'sheetLinks.typeWorkbook' : 'sheetLinks.typeOther')}</td>
                <td>{t('sheetLinks.manual')}</td>
                <td data-testid={`edit-links-status-${i}`}>
                  {t(statusKey(status[l.index] ?? 'unknown'))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {links.length === 0 && <p className={styles.hint}>{t('sheetLinks.none')}</p>}
        {current && (
          <p className={styles.hint} data-testid="edit-links-location">
            {t('sheetLinks.location', { target: current.target })}
            <br />
            {t('sheetLinks.used', { count: current.formulas })}
          </p>
        )}
        <div className={styles.row}>
          {button('update', t('sheetLinks.updateValues'), () => void readFrom(false))}
          {button('change', t('sheetLinks.changeSource'), () => void readFrom(true))}
          {button('break', t('sheetLinks.breakLink'), () => setConfirmBreak(true), !!current)}
        </div>
      </Dialog>
      {confirmBreak && (
        <Dialog
          open
          onOpenChange={(o) => !o && setConfirmBreak(false)}
          title={t('sheetLinks.breakTitle')}
          testId="break-link"
          footer={
            <>
              <button type="button" className={styles.btn} onClick={() => setConfirmBreak(false)}>
                {t('sheetEditor.dlg.cancel')}
              </button>
              <button
                type="button"
                className={styles.btnPrimary}
                data-testid="break-link-ok"
                onClick={() => void breakLink()}
              >
                {t('sheetLinks.breakLink')}
              </button>
            </>
          }
        >
          <p>{t('sheetLinks.breakWarning')}</p>
        </Dialog>
      )}
    </>
  );
}
