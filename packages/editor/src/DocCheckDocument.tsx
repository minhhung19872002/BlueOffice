import type { EditCommand, EditResult, InspectReport, InspectTarget } from '@blueoffice/platform';
import { useMemo } from 'react';
import { CheckDocumentDialog, type CheckDocumentBridge, type CheckTab } from './CheckDocument';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Bytes of the object replacement character in the index text (U+FFFC, UTF-8). */
const OBJECT_BYTES = 3;

/** BlueDoc bridge of the "Check document" dialog over the edit command bridge. */
export function docCheckBridge(run: Run): CheckDocumentBridge {
  const report = async (cmd: EditCommand): Promise<InspectReport | null> =>
    (await run(cmd))?.inspection ?? null;
  return {
    inspect: () => report({ cmd: 'inspect' }),
    remove: (categories) => report({ cmd: 'inspectRemove', categories }),
    keep: (categories) => report({ cmd: 'inspectKeep', categories }),
    fix: (fix) => report({ cmd: 'inspectFix', fix }),
    acceptAll: async () => {
      await run({ cmd: 'acceptAllRevisions' });
      return report({ cmd: 'inspect' });
    },
    goTo: (target: InspectTarget) => {
      const at = (p: number, o: number, end = o) =>
        void run({ cmd: 'setSelection', selection: { anchor: { p, o }, focus: { p, o: end } } });
      if (target.kind === 'paragraph' || target.kind === 'table') at(target.para, 0);
      else if (target.kind === 'drawing') at(target.para, target.at, target.at + OBJECT_BYTES);
    },
  };
}

/** "Check document" for BlueDoc (Review › Check document). */
export function DocCheckDocumentDialog({
  run,
  onClose,
  initialTab,
}: {
  run: Run;
  onClose: () => void;
  initialTab?: CheckTab;
}) {
  const bridge = useMemo(() => docCheckBridge(run), [run]);
  return <CheckDocumentDialog bridge={bridge} onClose={onClose} initialTab={initialTab} />;
}
