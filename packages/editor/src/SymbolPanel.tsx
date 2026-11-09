import { useMemo } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import { SPECIAL_CHARACTERS, SymbolPicker, type SymbolPickerLabels } from '@blueoffice/ui';
import styles from './SymbolPanel.module.css';

/** Translated labels of the shared symbol picker (`symbolPicker.*`). */
export function useSymbolPickerLabels(): SymbolPickerLabels {
  const { t, i18n } = useTranslation();
  return useMemo(
    () => ({
      title: t('symbolPicker.title'),
      symbolsTab: t('symbolPicker.symbolsTab'),
      specialTab: t('symbolPicker.specialTab'),
      search: t('symbolPicker.search'),
      block: t('symbolPicker.block'),
      font: t('symbolPicker.font'),
      recent: t('symbolPicker.recent'),
      noRecent: t('symbolPicker.noRecent'),
      noResults: t('symbolPicker.noResults'),
      grid: t('symbolPicker.grid'),
      insert: t('symbolPicker.insert'),
      close: t('symbolPicker.close'),
      blockName: (id, fallback) =>
        i18n.exists(`symbolPicker.blocks.${id}`) ? t(`symbolPicker.blocks.${id}`) : fallback,
      special: Object.fromEntries(
        SPECIAL_CHARACTERS.map((s) => [s.id, t(`symbolPicker.special.${s.id}`)]),
      ) as SymbolPickerLabels['special'],
    }),
    [t, i18n],
  );
}

/**
 * Floating, non-modal symbol picker shared by BlueDoc, BlueSheet and BlueSlide: the
 * document keeps its caret / edit state while characters are inserted. Escape closes.
 */
export function SymbolPanel({
  onInsert,
  onClose,
  fonts,
  font,
}: {
  onInsert: (char: string, font?: string) => void;
  onClose: () => void;
  fonts?: readonly string[];
  font?: string;
}) {
  const labels = useSymbolPickerLabels();
  return (
    <div
      className={styles.panel}
      role="dialog"
      aria-label={labels.title}
      data-testid="symbol-panel"
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Escape') {
          e.preventDefault();
          onClose();
        }
      }}
    >
      <SymbolPicker
        labels={labels}
        onInsert={onInsert}
        onClose={onClose}
        fonts={fonts}
        font={font}
      />
    </div>
  );
}
