/**
 * Hidden `<textarea>` that receives keyboard and IME input (Telex/VNI through the
 * OS or browser IME). Composition text is only committed on `compositionend`, so
 * Vietnamese input methods that rewrite the preceding characters work correctly.
 * Phase 1 only reports events; editing commands consume them in Phase 2.
 */
export type ImeEvent =
  | { type: 'insert'; text: string }
  | { type: 'composition'; text: string }
  | { type: 'key'; key: string; ctrl: boolean; shift: boolean; alt: boolean };

export class ImeInput {
  readonly element: HTMLTextAreaElement;
  private composing = false;

  constructor(
    parent: HTMLElement,
    private readonly onEvent: (e: ImeEvent) => void,
  ) {
    const ta = document.createElement('textarea');
    ta.setAttribute('aria-hidden', 'true');
    ta.setAttribute('autocapitalize', 'off');
    ta.setAttribute('autocomplete', 'off');
    ta.spellcheck = false;
    ta.style.cssText =
      'position:absolute;left:0;top:0;width:1px;height:1em;opacity:0;padding:0;border:0;resize:none;overflow:hidden;white-space:pre;';
    ta.addEventListener('compositionstart', () => {
      this.composing = true;
    });
    ta.addEventListener('compositionupdate', (e) =>
      this.onEvent({ type: 'composition', text: e.data ?? '' }),
    );
    ta.addEventListener('compositionend', (e) => {
      this.composing = false;
      this.commit(e.data || ta.value);
    });
    ta.addEventListener('input', (e) => {
      if (this.composing || (e as InputEvent).isComposing) return;
      this.commit(ta.value);
    });
    ta.addEventListener('keydown', (e) => {
      if (this.composing || e.isComposing) return;
      if (e.key.length > 1 || e.ctrlKey || e.metaKey || e.altKey) {
        this.onEvent({
          type: 'key',
          key: e.key,
          ctrl: e.ctrlKey || e.metaKey,
          shift: e.shiftKey,
          alt: e.altKey,
        });
      }
    });
    parent.appendChild(ta);
    this.element = ta;
  }

  private commit(text: string): void {
    this.element.value = '';
    if (text) this.onEvent({ type: 'insert', text: text.normalize('NFC') });
  }

  /** Move the hidden textarea to the caret so the IME candidate window appears there. */
  moveTo(x: number, y: number): void {
    this.element.style.transform = `translate(${x}px, ${y}px)`;
  }

  focus(): void {
    this.element.focus({ preventScroll: true });
  }

  dispose(): void {
    this.element.remove();
  }
}
