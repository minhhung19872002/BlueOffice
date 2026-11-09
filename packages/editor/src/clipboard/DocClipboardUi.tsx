import type { DocumentView } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import { useEffect, useState, type RefObject } from 'react';
import type { DocController } from '../controller';
import type { EditorStore } from '../store';
import { ClipboardPanel } from './ClipboardPanel';
import {
  loadPasteDefaults,
  pasteChoices,
  type DocPasteChoice,
  type DocPasteRecord,
} from './docPaste';
import { PasteDefaultsForm } from './PasteDefaults';
import { PasteOptionsButton } from './PasteOptions';

/**
 * BlueDoc's paste options button (after a paste, at the caret) and clipboard history
 * panel with the default paste settings (CORE-PASTE-001).
 */
export function DocClipboardUi({
  controller,
  ready,
  view,
  store,
  panel,
  onOpenPanel,
  onClosePanel,
}: {
  controller: RefObject<DocController | null>;
  /** Changes once the controller exists (it is created in the editor's effect). */
  ready: boolean;
  view: RefObject<DocumentView | null>;
  store: EditorStore;
  panel: boolean;
  onOpenPanel: () => void;
  onClosePanel: () => void;
}) {
  const { t } = useTranslation();
  const [paste, setPaste] = useState<{ rec: DocPasteRecord; n: number } | null>(null);

  useEffect(() => {
    const c = controller.current;
    if (!c || !ready) return undefined;
    c.setPasteDefaults(loadPasteDefaults());
    let n = 0;
    c.onPasteChange((rec) => setPaste(rec ? { rec, n: ++n } : null));
    return () => c.onPasteChange(null);
  }, [controller, ready]);

  // Just below the caret (the end of the pasted content), in screen pixels.
  const anchor = () => {
    const s = store.getState();
    const c = s.edit?.caret;
    const page = c ? s.doc?.pages[c.page] : undefined;
    const r = c ? view.current?.pageRect(c.page) : null;
    if (!c || !page || !r || page.width <= 0) return null;
    const k = r.width / page.width;
    return { x: r.left + c.x * k, y: r.top + (c.y + c.h) * k };
  };

  const focus = () => view.current?.focus();
  const options = paste
    ? pasteChoices(paste.rec).map((id) => ({ id, label: t(`pasteOptions.${id}`) }))
    : [];

  return (
    <>
      {paste && options.length > 1 && (
        <PasteOptionsButton
          // A new paste or mode places the button again.
          key={paste.n}
          anchor={anchor}
          options={options}
          current={paste.rec.mode}
          onChoose={(m) => void controller.current?.switchPaste(m as DocPasteChoice).then(focus)}
          settings={{ label: t('pasteOptions.setDefaults'), onSelect: onOpenPanel }}
        />
      )}
      {panel && (
        <ClipboardPanel
          onPaste={(item) => void controller.current?.pasteItem(item).then(focus)}
          onClose={() => {
            onClosePanel();
            focus();
          }}
          footer={<PasteDefaultsForm onChange={(d) => controller.current?.setPasteDefaults(d)} />}
        />
      )}
    </>
  );
}
