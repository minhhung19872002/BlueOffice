/**
 * Hidden `<textarea>` that receives keyboard and IME input (Telex/VNI through the
 * OS or browser IME). Composition text is only committed on `compositionend`, so
 * Vietnamese input methods that rewrite the preceding characters work correctly.
 * While composing, the textarea becomes visible at the caret and shows the
 * pre-edit text (underlined), like a native text field.
 */
export type ImeEvent =
  | { type: 'insert'; text: string }
  | { type: 'composition'; text: string }
  | { type: 'key'; key: string; ctrl: boolean; shift: boolean; alt: boolean }
  | { type: 'paste'; text: string; html: string; internal: string };

/** Custom clipboard type carrying the engine's lossless block format. */
export const INTERNAL_MIME = 'application/x-blueoffice-blocks+json';

/** Keys the editor handles itself (the textarea must not act on them). */
const EDIT_KEYS = new Set([
  'Backspace',
  'Delete',
  'Enter',
  'Tab',
  'ArrowLeft',
  'ArrowRight',
  'ArrowUp',
  'ArrowDown',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  'Escape',
]);

export class ImeInput {
  readonly element: HTMLTextAreaElement;
  private composing = false;
  private baseStyle: string;

  constructor(
    parent: HTMLElement,
    private readonly onEvent: (e: ImeEvent) => void,
  ) {
    const ta = document.createElement('textarea');
    ta.setAttribute('aria-hidden', 'true');
    ta.setAttribute('autocapitalize', 'off');
    ta.setAttribute('autocomplete', 'off');
    ta.dataset.testid = 'ime-input';
    ta.spellcheck = false;
    this.baseStyle =
      'position:absolute;left:0;top:0;width:1px;height:1em;opacity:0;padding:0;border:0;resize:none;overflow:hidden;white-space:pre;z-index:5;';
    ta.style.cssText = this.baseStyle;
    ta.addEventListener('compositionstart', () => {
      this.composing = true;
      ta.style.opacity = '1';
      ta.style.width = 'auto';
      ta.style.minWidth = '1em';
      ta.style.background = 'transparent';
      ta.style.textDecoration = 'underline';
      ta.style.color = '#000';
    });
    ta.addEventListener('compositionupdate', (e) => {
      ta.style.width = `${Math.max(1, (e.data ?? '').length + 1)}ch`;
      this.onEvent({ type: 'composition', text: e.data ?? '' });
    });
    ta.addEventListener('compositionend', (e) => {
      this.composing = false;
      this.resetStyle();
      this.commit(e.data || ta.value);
    });
    ta.addEventListener('input', (e) => {
      if (this.composing || (e as InputEvent).isComposing) return;
      this.commit(ta.value);
    });
    ta.addEventListener('keydown', (e) => {
      if (this.composing || e.isComposing) return;
      if (EDIT_KEYS.has(e.key)) e.preventDefault();
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
    ta.addEventListener('paste', (e) => {
      e.preventDefault();
      const d = e.clipboardData;
      this.onEvent({
        type: 'paste',
        text: d?.getData('text/plain') ?? '',
        html: d?.getData('text/html') ?? '',
        internal: d?.getData(INTERNAL_MIME) ?? '',
      });
    });
    parent.appendChild(ta);
    this.element = ta;
  }

  private resetStyle(): void {
    const t = this.element.style.transform;
    const f = this.element.style.fontSize;
    this.element.style.cssText = this.baseStyle;
    this.element.style.transform = t;
    this.element.style.fontSize = f;
  }

  private commit(text: string): void {
    this.element.value = '';
    if (text) this.onEvent({ type: 'insert', text: text.normalize('NFC') });
  }

  get isComposing(): boolean {
    return this.composing;
  }

  /** Move the hidden textarea to the caret so the IME candidate window appears there. */
  moveTo(x: number, y: number, height?: number): void {
    this.element.style.transform = `translate(${x}px, ${y}px)`;
    if (height) this.element.style.fontSize = `${Math.max(8, height * 0.8)}px`;
  }

  focus(): void {
    this.element.focus({ preventScroll: true });
  }

  dispose(): void {
    this.element.remove();
  }
}
