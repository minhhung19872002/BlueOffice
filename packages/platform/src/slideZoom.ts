/**
 * BlueSlide zooms (SLIDE-ZOOM-001): "Thu phóng tổng quan / phần / trang chiếu" — the commands of
 * `bo_engine::slide_edit::zoom` and their results. A zoom is a picture of a slide or a
 * section ([MS-PPTX] §2.8–§2.11 `pslz:sldZm`, `psez:sectionZm`, `psuz:summaryZm`); clicked in
 * the slide show it plays that slide or section and, with "return", comes back.
 */

export type SlideZoomKind = 'slide' | 'section' | 'summary';

/** One object of the selected zoom (`zmPr`, p166:CT_ZoomObjectProperties). */
export interface SlideZoomObject {
  /** Target slides (0-based, playing order). */
  slides: number[];
  /** Target section name (section / summary zooms). */
  section: string | null;
  id: string;
  /** `returnToParent`: come back to the zoom afterwards. */
  returnToParent: boolean;
  /** `imageType="cover"`: a custom image instead of the slide preview. */
  cover: boolean;
  /** `transitionDur` (ms); `null` = the target slide's own transition. */
  transitionMs: number | null;
  /** `showBg`: the zoom uses the target's background. */
  showBg: boolean;
}

/** The selected zoom. */
export interface SlideZoomInfo {
  id: number;
  kind: SlideZoomKind;
  objects: SlideZoomObject[];
}

/** Names a summary zoom gives to the slide and sections it creates (`{n}` = number). */
export interface SlideZoomNames {
  title?: string;
  summary?: string;
  section?: string;
  first?: string;
}

/** Zoom transition duration used when it is switched on without one (ms). */
export const ZOOM_TRANSITION_MS = 1000;

export type SlideZoomCommand =
  | { cmd: 'zoomInfo' }
  | { cmd: 'zoomInsertSlides'; slides: number[] }
  | { cmd: 'zoomInsertSections'; sections: number[] }
  | { cmd: 'zoomInsertSummary'; slides: number[]; names?: SlideZoomNames }
  | {
      cmd: 'zoomOptions';
      returnToParent?: boolean;
      transition?: boolean;
      durationMs?: number;
      showBg?: boolean;
    }
  /** A custom image: PNG / JPEG / GIF bytes, base64. */
  | { cmd: 'zoomImage'; data: string }
  | { cmd: 'zoomResetImage' }
  | { cmd: 'zoomRefresh' };
