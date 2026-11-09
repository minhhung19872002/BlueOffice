import { CheckDocumentDialog, type CheckDocumentBridge } from '@blueoffice/editor';
import type {
  InspectReport,
  InspectTarget,
  SlideCommand,
  SlideEditResult,
} from '@blueoffice/platform';
import { useMemo } from 'react';

type Run = (cmd: SlideCommand) => Promise<SlideEditResult | null>;

/** BlueSlide bridge of the "Check document" dialog over the slide command bridge. */
export function slideCheckBridge(run: Run): CheckDocumentBridge {
  const report = async (cmd: SlideCommand): Promise<InspectReport | null> =>
    (await run(cmd))?.inspection ?? null;
  return {
    inspect: () => report({ cmd: 'inspect' }),
    remove: (categories) => report({ cmd: 'inspectRemove', categories }),
    keep: (categories) => report({ cmd: 'inspectKeep', categories }),
    fix: (fix) => report({ cmd: 'inspectFix', fix }),
    goTo: (target: InspectTarget) => {
      if (target.kind === 'slide') void run({ cmd: 'goTo', slide: target.slide });
      else if (target.kind === 'shape') {
        void run({ cmd: 'goTo', slide: target.slide }).then(() =>
          run({ cmd: 'select', ids: [target.id] }),
        );
      }
    },
  };
}

/** "Check document" for BlueSlide (Review › Check document). */
export function SlideCheckDocumentDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const bridge = useMemo(() => slideCheckBridge(run), [run]);
  return <CheckDocumentDialog bridge={bridge} onClose={onClose} />;
}
