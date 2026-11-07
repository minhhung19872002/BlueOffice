import type { EditCommand, Motion } from '@blueoffice/platform';

/** A key press reported by the IME textarea. */
export interface KeyPress {
  key: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
}

const MOTIONS: Record<string, [Motion, Motion]> = {
  // key: [plain, with Ctrl]
  ArrowLeft: ['left', 'wordLeft'],
  ArrowRight: ['right', 'wordRight'],
  ArrowUp: ['up', 'up'],
  ArrowDown: ['down', 'down'],
  Home: ['lineStart', 'docStart'],
  End: ['lineEnd', 'docEnd'],
  PageUp: ['pageUp', 'pageUp'],
  PageDown: ['pageDown', 'pageDown'],
};

/**
 * Editing keys handled directly by the document (caret movement, deletion,
 * Enter, Tab). Ctrl+letter shortcuts go through the command registry instead.
 */
export function keyToCommand(k: KeyPress): EditCommand | null {
  const m = MOTIONS[k.key];
  if (m) return { cmd: 'move', motion: k.ctrl ? m[1] : m[0], extend: k.shift };
  if (k.alt) return null;
  switch (k.key) {
    case 'Backspace':
      return { cmd: 'deleteBackward' };
    case 'Delete':
      return { cmd: 'deleteForward' };
    case 'Enter':
      if (k.ctrl) return null; // Ctrl+Enter = page break (registry)
      // Shift+Enter: line break inside the paragraph (§17.3.3.1 br).
      return k.shift ? { cmd: 'insertText', text: ' ' } : { cmd: 'splitParagraph' };
    case 'Tab':
      return k.ctrl ? null : { cmd: 'insertText', text: '\t' };
    default:
      return null;
  }
}
