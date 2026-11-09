/**
 * Embedded and linked workbooks (CORE-EMBED-001) in BlueDoc and BlueSlide: labels of the
 * shared object dialogs, the registry commands behind them and the host functions built
 * from the `Platform` (pick a workbook, read a linked file on desktop, save a copy). The
 * dialogs themselves are `useObjectTools` of `@blueoffice/ui`; edits are `ole*` engine
 * commands (one undo step each).
 */
import {
  OBJECT_COMMANDS,
  type ObjectActions,
  type ObjectToolsHost,
  type ObjectToolsLabels,
} from '@blueoffice/ui';
import type { Platform } from '@blueoffice/platform';
import type { CommandRegistry } from './commands';

type T = (key: string, opts?: Record<string, unknown>) => string;

const XLSX_MIME = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';

/** Labels of the object dialogs. */
export function objectToolsLabels(t: T): ObjectToolsLabels {
  const k = (s: string) => t(`objectTools.${s}`);
  return {
    insertTitle: k('insertTitle'),
    editTitle: k('editTitle'),
    newTable: k('newTable'),
    fromFile: k('fromFile'),
    browse: k('browse'),
    noFile: k('noFile'),
    link: k('link'),
    linkHint: k('linkHint'),
    autoUpdate: k('autoUpdate'),
    range: k('range'),
    rangeHint: k('rangeHint'),
    addRow: k('addRow'),
    addColumn: k('addColumn'),
    removeRow: k('removeRow'),
    removeColumn: k('removeColumn'),
    ok: k('ok'),
    cancel: k('cancel'),
    cell: (p) => t('objectTools.cell', p),
  };
}

/**
 * Platform side of the object tools: workbook picker, reading linked files by path (desktop
 * hosts with `openPath`; absent on the web, where "Update links" reports it) and saving a copy.
 */
export function objectPlatform(
  platform: Platform,
): Pick<ObjectToolsHost, 'pickFile' | 'readPath' | 'saveFile'> {
  const files = platform.files;
  const openPath = files.openPath?.bind(files);
  return {
    pickFile: () => files.openFile(['.xlsx', '.xlsm']),
    readPath: openPath ? async (path) => (await openPath(path)).bytes : undefined,
    saveFile: (bytes, name) => files.saveFile(bytes, name, XLSX_MIME),
  };
}

/**
 * Register the `object.*` commands; they run the actions of the mounted object tools
 * (`actions()` is read when a command runs).
 */
export function registerObjectCommands<S>(
  r: CommandRegistry<S>,
  isEnabled: (s: S) => boolean,
  actions: () => ObjectActions | null,
): void {
  for (const id of OBJECT_COMMANDS) {
    r.register({ id, isEnabled, run: () => actions()?.[id]() });
  }
}
