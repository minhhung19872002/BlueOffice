/**
 * Reuse slides (SLIDE-ALBUM-001): another presentation is read next to the open
 * deck, listed with thumbnails, and chosen slides are inserted after the current
 * slide. Mirrors `bo_engine::slide_edit::reuse`.
 */

/** One slide of the deck to reuse. */
export interface SlideReuseSlide {
  /** Title text (empty without a title placeholder). */
  title: string;
  /** Thumbnail, PNG in base64 (absent for very long decks). */
  png?: string;
}

/** The deck read by `reuseOpen`. */
export interface SlideReuseSource {
  name: string;
  slides: SlideReuseSlide[];
}

export type SlideReuseCommand =
  /** A new deck with one empty title slide (photo album) → `file` (`.pptx`, base64). */
  | { cmd: 'newDeck'; title?: string }
  /** Read a deck (`.pptx`, `.odp`, `.ppt`) → `reuse` (error `reuseRead` when unreadable). */
  | { cmd: 'reuseOpen'; name: string; base64: string; width?: number }
  /** Forget the deck read by `reuseOpen`. */
  | { cmd: 'reuseClose' }
  /**
   * Insert slides of that deck after the current slide, in the order given (one
   * undo step). `keepSource`: they keep their own layout, master and theme
   * instead of taking a matching layout of the open deck.
   */
  | { cmd: 'reuseSlides'; indices: number[]; keepSource?: boolean };
