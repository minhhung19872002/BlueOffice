import { ClipboardPanel, PasteOptionsButton, type ClipItem } from '@blueoffice/editor';
import { useTranslation } from '@blueoffice/i18n';
import type { SheetCommand, SheetRange } from '@blueoffice/platform';

/** Ways a cell paste can be redone (paste options button, CORE-PASTE-001). */
export type SheetPasteChoice =
  'all' | 'values' | 'formulas' | 'formats' | 'transpose' | 'colWidths' | 'noBorders';

export const SHEET_PASTE_CHOICES: readonly SheetPasteChoice[] = [
  'all',
  'values',
  'formulas',
  'formats',
  'transpose',
  'colWidths',
  'noBorders',
];

/** Where a paste went: its top-left and the selection it filled. */
export interface SheetPasteBase {
  sheet: number;
  row: number;
  col: number;
  range: SheetRange;
}

/** The engine command pasting the internal clipboard in `choice`. */
export function sheetPasteCommand(choice: SheetPasteChoice, base: SheetPasteBase): SheetCommand {
  const mode =
    choice === 'values' || choice === 'formulas' || choice === 'formats' ? choice : 'all';
  return {
    cmd: 'paste',
    ...base,
    mode,
    transpose: choice === 'transpose',
    skipBlanks: false,
    colWidths: choice === 'colWidths',
    noBorders: choice === 'noBorders',
  };
}

/** BlueSheet's paste options button and the shared clipboard history panel. */
export function SheetClipboardUi({
  paste,
  anchor,
  onChoose,
  panel,
  onPasteItem,
  onClosePanel,
}: {
  /** The last paste while its mode can change (`n` changes with each paste / switch). */
  paste: { n: number; mode: SheetPasteChoice } | null;
  anchor: () => { x: number; y: number } | null;
  onChoose: (choice: SheetPasteChoice) => void;
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
          options={SHEET_PASTE_CHOICES.map((id) => ({
            id,
            label: t(`pasteOptions.sheet.${id}`),
          }))}
          current={paste.mode}
          onChoose={(id) => onChoose(id as SheetPasteChoice)}
        />
      )}
      {panel && <ClipboardPanel onPaste={onPasteItem} onClose={onClosePanel} />}
    </>
  );
}
