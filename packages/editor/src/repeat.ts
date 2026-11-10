/**
 * "Repeat last action" (F4, CORE-UI-004): the editors remember the last engine command that
 * makes sense at another place (formatting, a style, an insertion, a table edit) and send it
 * again on F4. Caret moves, selections, queries and undo are not repeatable and do not
 * forget the remembered command; typing is remembered as one run of text.
 */

/** BlueDoc commands (`bo_engine::edit::Command`) worth repeating at the new selection. */
export const DOC_REPEATABLE: ReadonlySet<string> = new Set([
  'insertText',
  'formatText',
  'formatParagraph',
  'applyStyle',
  'defineList',
  'setListNumbering',
  'insertTable',
  'insertSectionBreak',
  'tableEdit',
  'tableInsertRow',
  'tableInsertCol',
  'insertField',
  'insertEquation',
  'insertShape',
  'insertTextBox',
]);

/** BlueSlide commands repeated on the current selection / slide. */
export const SLIDE_REPEATABLE: ReadonlySet<string> = new Set([
  'formatText',
  'setFill',
  'setLine',
  'shapeEffects',
  'zOrder',
  'align',
  'distribute',
  'insertShape',
  'insertTextBox',
  'insertTable',
  'tableInsertRow',
  'tableInsertCol',
  'addSlide',
  'setTransition',
  'addAnimation',
]);

/** Commands that leave the remembered one alone (they change nothing, or only where). */
const NEUTRAL = new Set([
  'state',
  'openSetup',
  'historyLen',
  'mergeHistory',
  'spelling',
  'spellAt',
  'hitTest',
  'styleList',
  'fontList',
  'outline',
  'notes',
  'smartArtInfo',
  'chartGallery',
  'equationTemplates',
  'click',
  'dragTo',
  'move',
  'setSelection',
  'select',
  'selectRect',
  'selectNext',
  'textHit',
  'textSelect',
  'cellTextHit',
  'tableHit',
  'tableSelect',
  'goTo',
  'undo',
  'find',
]);

type Cmd = { cmd: string };

/** Remembers the last repeatable command of a stream of engine commands. */
export class RepeatTracker<C extends Cmd> {
  private last: C | null = null;
  /** The last command was typing: the next typed text joins it. */
  private typing = false;

  constructor(private readonly repeatable: ReadonlySet<string>) {}

  /** Note a command sent to the engine. */
  record(c: C): void {
    if (c.cmd === 'insertText' && this.repeatable.has('insertText')) {
      const text = (c as unknown as { text?: unknown }).text;
      if (typeof text !== 'string' || !text) return;
      const prev = this.typing ? (this.last as unknown as { text?: string } | null) : null;
      this.last = (
        prev && typeof prev.text === 'string' ? { ...c, text: prev.text + text } : c
      ) as C;
      this.typing = true;
      return;
    }
    // A caret move ends the typed run (F4 repeats it as a whole elsewhere).
    if (c.cmd === 'move' || c.cmd === 'click' || c.cmd === 'setSelection') this.typing = false;
    if (NEUTRAL.has(c.cmd)) return;
    this.typing = false;
    // Previews while dragging and the like are not edits to repeat.
    if ((c as unknown as { preview?: unknown }).preview) return;
    if (this.repeatable.has(c.cmd)) this.last = c;
  }

  /** The command F4 sends again (a copy without the host stamp), null when none. */
  get(): C | null {
    if (!this.last) return null;
    const copy = { ...this.last } as C & { now?: unknown };
    delete copy.now;
    return copy;
  }

  /** End the typed run (the next text starts a new one). */
  endRun(): void {
    this.typing = false;
  }

  clear(): void {
    this.last = null;
    this.typing = false;
  }
}
