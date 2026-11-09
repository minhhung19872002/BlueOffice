/**
 * Pure slide-show logic: key handling, elapsed-time formatting and fitting a
 * slide into a box. No DOM; unit-tested.
 */

export type ShowAction = 'next' | 'prev' | 'first' | 'last' | 'exit' | 'black' | 'white' | null;

/** Keys of common presentation software and presentation remotes. */
export function showAction(key: string): ShowAction {
  switch (key) {
    case 'ArrowRight':
    case 'ArrowDown':
    case 'PageDown':
    case ' ':
    case 'Enter':
    case 'n':
    case 'N':
      return 'next';
    case 'ArrowLeft':
    case 'ArrowUp':
    case 'PageUp':
    case 'Backspace':
    case 'p':
    case 'P':
      return 'prev';
    case 'Home':
      return 'first';
    case 'End':
      return 'last';
    case 'Escape':
      return 'exit';
    case 'b':
    case 'B':
    case '.':
      return 'black';
    case 'w':
    case 'W':
    case ',':
      return 'white';
    default:
      return null;
  }
}

/**
 * Applies a navigation action. `index === count` is the black "end of show"
 * screen shown after the last slide.
 */
export function step(index: number, count: number, action: ShowAction): number {
  switch (action) {
    case 'next':
      return Math.min(count, index + 1);
    case 'prev':
      return Math.max(0, index - 1);
    case 'first':
      return 0;
    case 'last':
      return Math.max(0, count - 1);
    default:
      return index;
  }
}

/** `mm:ss`, or `h:mm:ss` from one hour. */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const two = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${two(m)}:${two(s)}`;
}

/** Largest size with the slide's aspect ratio that fits in `box` (CSS px). */
export function fit(
  slide: { width: number; height: number },
  box: { width: number; height: number },
): { width: number; height: number } {
  if (slide.width <= 0 || slide.height <= 0 || box.width <= 0 || box.height <= 0) {
    return { width: 0, height: 0 };
  }
  const s = Math.min(box.width / slide.width, box.height / slide.height);
  return { width: Math.floor(slide.width * s), height: Math.floor(slide.height * s) };
}
