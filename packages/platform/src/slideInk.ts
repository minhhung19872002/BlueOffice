/**
 * BlueSlide ink (CORE-INK-001): the commands of `bo_engine::slide_edit::ink` and
 * their results. Ink is stored as InkML content parts ([MS-ODRAWXML] §2.1.4);
 * coordinates are slide points.
 */
import type { PageDisplayList } from './types';

/** A stroke drawn in the editor. */
export interface SlideInkStrokeIn {
  points: [number, number][];
  /** Pen pressure 0..1 per point (pen input only). */
  pressure?: number[];
  /** Milliseconds since the stroke started, per point. */
  times?: number[];
  /** `RRGGBB`. */
  color: string;
  /** Points. */
  width: number;
  highlighter?: boolean;
}

export type SlideEraseMode = 'stroke' | 'point';

export type SlideInkCommand =
  /** Ink of the current slide → `ink`; `backdrop`: also the slide drawn without ink. */
  | { cmd: 'inkInfo'; backdrop?: boolean }
  /** New ink object (one undo step). */
  | { cmd: 'addInk'; stroke: SlideInkStrokeIn; name?: string }
  /** Erase along a path (one undo step). */
  | { cmd: 'eraseInk'; points: [number, number][]; radius: number; mode?: SlideEraseMode };

/** One trace in slide points. */
export interface SlideInkStrokeOut {
  /** `[x, y, width]` per sample (width after pressure). */
  points: [number, number, number][];
  /** Time of each sample (ms), when recorded. */
  times?: number[];
  color: string;
  opacity: number;
  highlighter: boolean;
  rectTip: boolean;
}

/** One ink object (content part). */
export interface SlideInkObject {
  id: number;
  name: string;
  /** Frame `[x, y, w, h]`. */
  rect: [number, number, number, number];
  strokes: SlideInkStrokeOut[];
}

export interface SlideInkInfo {
  objects: SlideInkObject[];
  /** The slide drawn without its ink (`inkInfo` with `backdrop`). */
  backdrop?: PageDisplayList;
}
