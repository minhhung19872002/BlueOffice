/**
 * BlueSlide views and sections (SLIDE-VIEW-001): the commands of
 * `bo_engine::slide_edit::views` / `page_master` and their results. Boxes are in
 * points of the notes / handout page (ECMA-376 Part 1, §19.2.1.23 notesSz).
 */
import type { SlideRect } from './types';

/** One body paragraph of the outline (level 0–8, §21.1.2.2.7 `pPr@lvl`). */
export interface SlideOutlineBullet {
  text: string;
  level: number;
}

/** A slide in the outline. */
export interface SlideOutlineSlide {
  /** Title text (paragraphs separated by `\n`). */
  title: string;
  bullets: SlideOutlineBullet[];
  hasTitle: boolean;
  hasBody: boolean;
}

/** One slide of an edited outline (`slide` absent: a new slide). */
export interface SlideOutlineEdit {
  slide?: number;
  title: string;
  bullets: SlideOutlineBullet[];
}

/** Geometry and text of a notes page. */
export interface SlideNotesPage {
  width: number;
  height: number;
  /** The slide picture (fitted into the slide image placeholder). */
  image: SlideRect;
  /** The notes text box. */
  body: SlideRect;
  text: string;
  /** Notes text size in points. */
  size: number;
  /** Header, date, footer and page number from the notes master (`p:hf` flags). */
  hf?: SlideNotesHf[];
}

/** A header / footer item of a notes page (§19.7.10 `hdr` `dt` `ftr` `sldNum`). */
export interface SlideNotesHf {
  kind: 'header' | 'date' | 'footer' | 'number';
  rect: SlideRect;
  text: string;
  align: 'left' | 'center' | 'right';
  /** Font size in points. */
  size: number;
  /** Text at the bottom of the box (footer, page number). */
  bottom: boolean;
}

/** A section of the deck (`first`: index of its first slide). */
export interface SlideSection {
  name: string;
  id: string;
  first: number;
  count: number;
}

export type SlidePageMasterKind = 'notes' | 'handout';

/** The notes / handout master view's pane. */
export interface SlidePageMasterInfo {
  kind: SlidePageMasterKind;
  header: boolean;
  date: boolean;
  footer: boolean;
  number: boolean;
  slideImage: boolean;
  body: boolean;
  portrait: boolean;
  perPage: number;
  /** Slide boxes of the handout preview (points). */
  slots: SlideRect[];
}

/** Slides per handout page the handout master view can preview. */
export const HANDOUT_PER_PAGE = [1, 2, 3, 4, 6, 9] as const;

export type SlideViewCommand =
  | { cmd: 'outline' }
  | { cmd: 'applyOutline'; slides: SlideOutlineEdit[]; current?: number }
  | { cmd: 'notesPages' }
  | { cmd: 'addSection'; name: string; slide?: number; firstName?: string }
  | { cmd: 'renameSection'; index: number; name: string }
  | { cmd: 'removeSection'; index: number; slides?: boolean }
  | { cmd: 'removeAllSections' }
  | { cmd: 'moveSection'; index: number; to: number }
  | { cmd: 'pageMasterView'; kind?: SlidePageMasterKind; on: boolean }
  | {
      cmd: 'pageMasterElements';
      header?: boolean;
      date?: boolean;
      footer?: boolean;
      number?: boolean;
      slideImage?: boolean;
      body?: boolean;
    }
  | { cmd: 'pageOrientation'; portrait: boolean }
  | { cmd: 'handoutSlides'; perPage: number };
