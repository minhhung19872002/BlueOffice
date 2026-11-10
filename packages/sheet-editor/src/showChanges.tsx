import { VersionsPane, toBase64, type ChangeRow, type CompareOutcome } from '@blueoffice/editor';
import { useTranslation } from '@blueoffice/i18n';
import type {
  OpenedFile,
  SheetChange,
  SheetCommand,
  SheetCompareResult,
  VersionHistory,
} from '@blueoffice/platform';
import { useCallback } from 'react';
import type { SheetGoTo } from './inspect';

type Exec = <T>(cmd: SheetCommand) => Promise<T | null>;
type T = (key: string, opts?: Record<string, unknown>) => string;

/** Rows of the Show Changes pane: grouped by sheet, kind and address in the user's words. */
export function sheetChangeRows(changes: SheetChange[], t: T): ChangeRow<SheetChange>[] {
  return changes.map((c, i) => {
    let kind = t(`versions.sheet.${c.kind}`, { count: c.count ?? 1 });
    if (c.kind === 'sheetRenamed' && c.oldSheet) {
      kind = t('versions.sheet.sheetRenamedFrom', { name: c.oldSheet });
    }
    const where =
      c.oldAddress && c.oldAddress !== c.address
        ? c.address
          ? `${c.address} (${t('versions.was', { where: c.oldAddress })})`
          : c.oldAddress
        : c.address;
    return {
      id: `${i}`,
      group: c.sheet,
      kind,
      where,
      ...(c.old !== undefined ? { old: c.old } : {}),
      ...(c.new !== undefined ? { new: c.new } : {}),
      target: c,
    };
  });
}

/** Where a change is in the open workbook (`null`: removed sheet). */
export function changeTarget(c: SheetChange): SheetGoTo | null {
  if (c.sheetIndex === null) return null;
  const n = Math.max(1, c.count ?? 1);
  const isRows = c.kind === 'rowsInserted' || c.kind === 'rowsDeleted';
  const isCols = c.kind === 'colsInserted' || c.kind === 'colsDeleted';
  const row = c.row ?? 0;
  const col = c.col ?? 0;
  if (isRows) return { sheet: c.sheetIndex, r0: row, c0: 0, r1: row + n - 1, c1: 0 };
  if (isCols) return { sheet: c.sheetIndex, r0: 0, c0: col, r1: 0, c1: col + n - 1 };
  return { sheet: c.sheetIndex, r0: row, c0: col, r1: row, c1: col };
}

/** Show Changes of BlueSheet (Review › Show Changes, CORE-VERSION-001). */
export function SheetShowChangesPane({
  exec,
  versions,
  openFile,
  goTo,
  onClose,
  embedded = false,
}: {
  exec: Exec;
  versions?: VersionHistory | undefined;
  openFile?: () => Promise<OpenedFile | null>;
  goTo: (to: SheetGoTo) => void;
  onClose: () => void;
  /** Inside a task pane (CORE-UI-006). */
  embedded?: boolean;
}) {
  const { t } = useTranslation();
  const compare = useCallback(
    async (bytes: Uint8Array, name: string): Promise<CompareOutcome<SheetChange>> => {
      const r = await exec<SheetCompareResult>({
        cmd: 'compareWorkbook',
        name,
        base64: toBase64(bytes),
      });
      if (!r) throw new Error('compareWorkbook');
      return { rows: sheetChangeRows(r.changes, t), total: r.total, truncated: r.truncated };
    },
    [exec, t],
  );
  return (
    <VersionsPane
      title={t('versions.sheetTitle')}
      versions={versions}
      {...(openFile ? { openFile } : {})}
      compare={compare}
      onSelect={(row) => {
        const to = changeTarget(row.target);
        if (to) goTo(to);
      }}
      onClose={onClose}
      embedded={embedded}
    />
  );
}
