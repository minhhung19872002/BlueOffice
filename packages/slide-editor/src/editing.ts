import type { SlideCommand } from '@blueoffice/platform';

/** Line break inside a paragraph in the engine's editing format (§21.1.2.2.1 br). */
export const ENGINE_BR = '\u000b';
/** How a line break is shown in the text editor (a textarea line break that is not a paragraph). */
export const EDITOR_BR = ' ';

/** Engine text → textarea value. */
export function toEditorText(text: string): string {
  return text.split(ENGINE_BR).join(EDITOR_BR);
}

/** Textarea value → engine text. */
export function fromEditorText(text: string): string {
  return text.replace(/\r\n?/g, '\n').split(EDITOR_BR).join(ENGINE_BR);
}

/** Nudge step in points (Shift = large step). */
export function nudgeStep(shift: boolean): number {
  return shift ? 10 : 1;
}

/** Arrow key → move command for the selection. */
export function nudgeCommand(key: string, shift: boolean): SlideCommand | null {
  const s = nudgeStep(shift);
  const d: Record<string, [number, number]> = {
    ArrowLeft: [-s, 0],
    ArrowRight: [s, 0],
    ArrowUp: [0, -s],
    ArrowDown: [0, s],
  };
  const v = d[key];
  if (!v) return null;
  return { cmd: 'transform', transform: { kind: 'move', dx: v[0], dy: v[1] }, snap: false };
}

/** Normalise a colour input value (`#rrggbb`) to the engine's `RRGGBB`. */
export function hexOf(value: string): string | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(value.trim());
  return m ? m[1]!.toUpperCase() : null;
}
