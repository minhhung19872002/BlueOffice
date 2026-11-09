import { useState } from 'react';
import { Dialog } from './Dialog';

export interface FileTypeOption {
  /** Value returned by `onConfirm` (a file extension, e.g. `dotm`). */
  value: string;
  label: string;
}

export interface FileTypeDialogLabels {
  title: string;
  /** Legend of the type list. */
  type: string;
  ok: string;
  cancel: string;
}

export interface FileTypeDialogProps {
  open: boolean;
  options: FileTypeOption[];
  /** Type selected when the dialog opens. */
  initial: string;
  labels: FileTypeDialogLabels;
  /** Warning shown under the list for the selected type (e.g. macros removed), or null. */
  warning?: (value: string) => string | null;
  onConfirm: (value: string) => void;
  onCancel: () => void;
}

/**
 * "Save as" file type choice (CORE-FMT-001): one radio per type, an optional
 * warning for the selected type, OK / Cancel. The file name itself is asked by
 * the platform's save dialog afterwards.
 */
export function FileTypeDialog(props: FileTypeDialogProps) {
  // Mounted while open: each opening starts from `initial`.
  return props.open ? <OpenFileTypeDialog {...props} /> : null;
}

function OpenFileTypeDialog({
  options,
  initial,
  labels,
  warning,
  onConfirm,
  onCancel,
}: FileTypeDialogProps) {
  const [value, setValue] = useState(initial);
  const note = warning?.(value) ?? null;
  return (
    <Dialog
      open
      onOpenChange={(o) => {
        if (!o) onCancel();
      }}
      title={labels.title}
      testId="file-type-dialog"
      footer={
        <>
          <button type="button" onClick={onCancel} data-testid="file-type-cancel">
            {labels.cancel}
          </button>
          <button
            type="button"
            onClick={() => onConfirm(value)}
            data-testid="file-type-ok"
            disabled={!options.some((o) => o.value === value)}
          >
            {labels.ok}
          </button>
        </>
      }
    >
      <fieldset style={{ border: 'none', margin: 0, padding: 0 }}>
        <legend style={{ marginBottom: 6 }}>{labels.type}</legend>
        {options.map((o) => (
          <label key={o.value} style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <input
              type="radio"
              name="file-type"
              value={o.value}
              checked={value === o.value}
              onChange={() => setValue(o.value)}
              data-testid={`file-type-${o.value}`}
            />
            {o.label}
          </label>
        ))}
      </fieldset>
      {note && (
        <p role="alert" data-testid="file-type-warning" style={{ margin: 0 }}>
          {note}
        </p>
      )}
    </Dialog>
  );
}
