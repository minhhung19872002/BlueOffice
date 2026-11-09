import { useTranslation } from '@blueoffice/i18n';
import {
  dropsMacros,
  saveVariants,
  variantOf,
  withExtension,
  documentVariant,
  type FileVariant,
  type VariantApp,
} from '@blueoffice/platform';
import { FileTypeDialog } from '@blueoffice/ui';

/**
 * "Save as" type choice shared by BlueDoc, BlueSheet and BlueSlide (CORE-FMT-001):
 * the variants of the app's document type, with a warning when the chosen type
 * cannot hold the macros of the open file.
 */
export function SaveAsTypeDialog({
  open,
  app,
  current,
  macros,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  app: VariantApp;
  /** Variant of the open file (`DocInfo.variant`), preselected. */
  current?: string | null;
  /** The open file has macros. */
  macros?: boolean;
  onConfirm: (ext: string) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const types = saveVariants(app);
  const initial = variantOf(current)?.app === app ? (variantOf(current)?.ext ?? '') : types[0]!.ext;
  return (
    <FileTypeDialog
      open={open}
      initial={initial}
      options={types.map((v) => ({ value: v.ext, label: t(`fileVariants.types.${v.ext}`) }))}
      labels={{
        title: t('fileVariants.saveAsTitle'),
        type: t('fileVariants.type'),
        ok: t('fileVariants.save'),
        cancel: t('fileVariants.cancel'),
      }}
      warning={(ext) => (dropsMacros(macros, ext) ? t('fileVariants.macroDropped') : null)}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  );
}

/** How an opened file is treated: a template starts a new document (CORE-FMT-001). */
export interface OpenPlan {
  /** Variant reported by the engine (by content type), if any. */
  variant?: FileVariant;
  /** The file is a template opened as a new, untitled document. */
  fromTemplate: boolean;
  /** Variant the engine should save to (`saveFormat`), when it differs. */
  saveAs?: string;
  /** Name shown for the document. */
  name: string;
  /** The file opens straight into the slide show. */
  show: boolean;
}

/**
 * Decide how to present an opened file: a template (`.dotx`, `.xltm`, `.potx`, …)
 * becomes an untitled document of the matching type unless `editTemplate`; a slide
 * show (`.ppsx`/`.ppsm`) starts the show.
 */
export function planOpen(
  engineVariant: string | null | undefined,
  name: string,
  editTemplate = false,
): OpenPlan {
  const variant = variantOf(engineVariant);
  if (variant?.template && !editTemplate) {
    const doc = documentVariant(variant);
    return {
      variant,
      fromTemplate: true,
      saveAs: doc.ext,
      name: withExtension(name, doc.ext),
      show: false,
    };
  }
  return { variant, fromTemplate: false, name, show: !!variant?.show };
}

/** File name to save under: `name` with the extension of `variant` (or `fallback`). */
export function saveName(
  name: string,
  variant: string | null | undefined,
  fallback: string,
): string {
  return withExtension(name, variantOf(variant)?.ext ?? fallback);
}
