import { useMemo } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import {
  IconLibraryPicker,
  iconLocale,
  type IconLibrary,
  type IconLibraryLabels,
  type IconPick,
} from '@blueoffice/ui';
import styles from './SymbolPanel.module.css';

/** Translated labels of the shared icon library (`iconLibrary.*`). */
export function useIconLibraryLabels(): IconLibraryLabels {
  const { t } = useTranslation();
  return useMemo(
    () => ({
      title: t('iconLibrary.title'),
      search: t('iconLibrary.search'),
      category: t('iconLibrary.category'),
      allCategories: t('iconLibrary.allCategories'),
      grid: t('iconLibrary.grid'),
      loading: t('iconLibrary.loading'),
      loadError: t('iconLibrary.loadError'),
      noResults: t('iconLibrary.noResults'),
      found: (count) => t('iconLibrary.found', { count }),
      selected: (count) => t('iconLibrary.selected', { count }),
      clear: t('iconLibrary.clear'),
      insert: t('iconLibrary.insert'),
      close: t('iconLibrary.close'),
    }),
    [t],
  );
}

/**
 * Floating, non-modal icon library shared by BlueDoc, BlueSheet and BlueSlide
 * (CORE-ICON-001): the document keeps its caret / selection while icons are inserted.
 * Escape closes. Icon names (the default alternative text) follow the UI language.
 */
export function IconPanel({
  onInsert,
  onClose,
  load,
}: {
  onInsert: (icons: IconPick[]) => void | Promise<void>;
  onClose: () => void;
  /** Source of the library (tests). */
  load?: () => Promise<IconLibrary>;
}) {
  const { i18n } = useTranslation();
  const labels = useIconLibraryLabels();
  return (
    <div
      className={styles.panel}
      role="dialog"
      aria-label={labels.title}
      data-testid="icon-panel"
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Escape') {
          e.preventDefault();
          onClose();
        }
      }}
    >
      <IconLibraryPicker
        labels={labels}
        locale={iconLocale(i18n.language)}
        onInsert={onInsert}
        onClose={onClose}
        load={load}
      />
    </div>
  );
}
