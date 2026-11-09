import { ClipboardPanel, PasteOptionsButton, type ClipItem } from '@blueoffice/editor';
import { useTranslation } from '@blueoffice/i18n';

/** Ways a shape paste can be redone (paste options button, CORE-PASTE-001). */
export type SlidePasteChoice = 'theme' | 'keep' | 'picture' | 'text';

/** Choices for a paste of copied shapes (picture / text only when the copy had them). */
export function slidePasteChoices(copy: { text: string; png?: string }): SlidePasteChoice[] {
  const out: SlidePasteChoice[] = ['theme', 'keep'];
  if (copy.png) out.push('picture');
  if (copy.text.trim()) out.push('text');
  return out;
}

/** BlueSlide's paste options button (at the pasted shapes) and the clipboard history panel. */
export function SlideClipboardUi({
  paste,
  anchor,
  onChoose,
  panel,
  onPasteItem,
  onClosePanel,
}: {
  paste: { n: number; mode: SlidePasteChoice; choices: SlidePasteChoice[] } | null;
  anchor: () => { x: number; y: number } | null;
  onChoose: (choice: SlidePasteChoice) => void;
  panel: boolean;
  onPasteItem: (item: ClipItem) => void;
  onClosePanel: () => void;
}) {
  const { t } = useTranslation();
  return (
    <>
      {paste && (
        <PasteOptionsButton
          key={paste.n}
          anchor={anchor}
          options={paste.choices.map((id) => ({ id, label: t(`pasteOptions.slide.${id}`) }))}
          current={paste.mode}
          onChoose={(id) => onChoose(id as SlidePasteChoice)}
        />
      )}
      {panel && <ClipboardPanel onPaste={onPasteItem} onClose={onClosePanel} />}
    </>
  );
}

/** Bottom-right corner of the selected shapes' boxes on screen. */
export function selectionCorner(): { x: number; y: number } | null {
  const boxes = Array.from(document.querySelectorAll('[data-testid="selection-box"]'));
  if (!boxes.length) return null;
  let x = -Infinity;
  let y = -Infinity;
  for (const b of boxes) {
    const r = b.getBoundingClientRect();
    x = Math.max(x, r.right);
    y = Math.max(y, r.bottom);
  }
  return Number.isFinite(x) ? { x, y } : null;
}
