import { CheckDocumentDialog, type CheckDocumentBridge } from '@blueoffice/editor';
import type { InspectReport, InspectTarget, SheetCommand } from '@blueoffice/platform';
import { useMemo } from 'react';

type Exec = <T>(cmd: SheetCommand) => Promise<T | null>;

/** A cell range of the target sheet to select. */
export interface SheetGoTo {
  sheet: number;
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

/** BlueSheet bridge of the "Check document" dialog over `SheetHost.exec`. */
export function sheetCheckBridge(exec: Exec, goTo: (to: SheetGoTo) => void): CheckDocumentBridge {
  return {
    inspect: () => exec<InspectReport>({ cmd: 'inspect' }),
    remove: (categories) => exec<InspectReport>({ cmd: 'inspectRemove', categories }),
    keep: (categories) => exec<InspectReport>({ cmd: 'inspectKeep', categories }),
    goTo: (target: InspectTarget) => {
      if (target.kind === 'range') goTo(target);
      else if (target.kind === 'sheet') goTo({ sheet: target.sheet, r0: 0, c0: 0, r1: 0, c1: 0 });
    },
  };
}

/** "Check document" for BlueSheet (Review › Check document). */
export function SheetCheckDocumentDialog({
  exec,
  goTo,
  onClose,
}: {
  exec: Exec;
  goTo: (to: SheetGoTo) => void;
  onClose: () => void;
}) {
  const bridge = useMemo(() => sheetCheckBridge(exec, goTo), [exec, goTo]);
  return <CheckDocumentDialog bridge={bridge} onClose={onClose} />;
}
