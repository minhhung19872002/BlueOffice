import type { CellDetails } from '@blueoffice/platform';

/**
 * Text the cell editor starts with: a cell of a legacy array formula shows `{=…}` in the
 * formula bar, but is edited without the braces (they come back with Ctrl+Shift+Enter).
 */
export function editableInput(cell: Pick<CellDetails, 'input' | 'array'>): string {
  const { input } = cell;
  if (cell.array && input.startsWith('{=') && input.endsWith('}')) return input.slice(1, -1);
  return input;
}

/** Whether a key press is Ctrl+Shift+Enter (Cmd+Shift+Enter), the array formula entry. */
export function isArrayEntry(e: {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}): boolean {
  return e.key === 'Enter' && (e.ctrlKey || e.metaKey) && e.shiftKey && !e.altKey;
}
