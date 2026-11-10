/**
 * BlueSlide presenting (SLIDE-SHOW-001): the commands of
 * `bo_engine::slide_edit::present` and their results, plus the display
 * service that puts the audience screen on another monitor.
 */

/** §19.2.1.30 EG_ShowType: presented by a speaker, browsed in a window, at a kiosk. */
export type SlideShowType = 'speaker' | 'browse' | 'kiosk';

/** §19.2.1.30 EG_SlideListChoice: every slide, slide numbers `from..to` (1-based), a custom show. */
export type SlideShowRange =
  { kind: 'all' } | { kind: 'range'; from: number; to: number } | { kind: 'custom'; id: number };

/** §19.2.1.30 showPr (schema defaults when the deck has none). */
export interface SlideShowSetup {
  showType: SlideShowType;
  /** `browse@showScrollbar`. */
  scrollbar: boolean;
  /** `kiosk@restart`: inactivity before the show restarts (ms). */
  restartMs: number;
  loop: boolean;
  narration: boolean;
  animation: boolean;
  useTimings: boolean;
  range: SlideShowRange;
  /** `penClr` (`RRGGBB`) when it is an sRGB colour. */
  penColor: string | null;
}

export const DEFAULT_SHOW_SETUP: SlideShowSetup = {
  showType: 'speaker',
  scrollbar: true,
  restartMs: 300_000,
  loop: false,
  narration: false,
  animation: true,
  useTimings: true,
  range: { kind: 'all' },
  penColor: null,
};

/** A custom show (§19.2.1.6): named list of 0-based slides in playing order. */
export interface SlideCustomShow {
  id: number;
  name: string;
  slides: number[];
}

/** What the Slide Show tab needs about the deck. */
export interface SlidePresentInfo {
  customShows: SlideCustomShow[];
  setup: SlideShowSetup;
  /** Slides left out of the show (`sld@show="0"`). */
  hidden: number[];
  /** `advTm` of each slide (ms), `null` = no automatic advance. */
  timings: (number | null)[];
}

/** What a hyperlink does in the show (§21.1.2.3.5 hlinkClick `ppaction://` URLs). */
export type SlideLinkAction =
  | { kind: 'none' }
  | { kind: 'customShow'; id: number; return?: boolean }
  | { kind: 'slide'; slide: number }
  | { kind: 'next' | 'prev' | 'first' | 'last' | 'lastViewed' | 'end' }
  /**
   * A zoom (SLIDE-ZOOM-001): play `slides`, zooming in from the zoom's box `rect` (points,
   * filled in by the show from the link); with `return`, come back afterwards.
   * `transitionMs` `null` = the target's own transition.
   */
  | {
      kind: 'zoom';
      slides: number[];
      return?: boolean;
      transitionMs?: number | null;
      rect?: [number, number, number, number];
    };

/** A shape whose hyperlink the show follows, with its box `[x, y, w, h]` (points). */
export interface SlideShowLink {
  shapeId: number;
  rect: [number, number, number, number];
  action: SlideLinkAction;
}

/** One ink stroke in slide points. */
export interface SlideInkStroke {
  points: [number, number][];
  /** `RRGGBB`. */
  color: string;
  /** Points. */
  width: number;
  highlighter?: boolean;
}

export type SlidePresentCommand =
  | { cmd: 'setCustomShow'; id?: number; name: string; slides: number[] }
  | { cmd: 'deleteCustomShow'; id: number }
  | { cmd: 'setShowSetup'; setup: Partial<SlideShowSetup> }
  | {
      cmd: 'setTimings';
      timings: { slide: number; ms: number | null }[];
      useTimings?: boolean;
    }
  | { cmd: 'setShowLink'; action: SlideLinkAction }
  | { cmd: 'keepInk'; ink: { slide: number; strokes: SlideInkStroke[] }[]; name?: string };

// ------------------------------------------------------------------ displays

/** A screen the host can open the audience window on. */
export interface DisplayScreen {
  id: string;
  label: string;
  primary: boolean;
  left: number;
  top: number;
  width: number;
  height: number;
}

/** What the host can do to show a second screen (capability detection). */
export interface DisplayCapabilities {
  /** The host can list the screens and place a window on one (Window Management API, Tauri monitors). */
  screens: boolean;
  /** Presentation API (web): the browser picks a presentation display. */
  presentation: boolean;
  /** A second window (always, where `Platform.channel` exists). */
  window: boolean;
}

/** Where the audience screen opens. */
export type DisplayTarget =
  { kind: 'window' } | { kind: 'screen'; id: string } | { kind: 'presentation' };

/** Puts the audience screen on another monitor where the host can. */
export interface DisplayService {
  capabilities(): DisplayCapabilities;
  /** Screens of the system (may ask the user for permission on the web). */
  screens(): Promise<DisplayScreen[]>;
  /**
   * Open the app page `query` (`?app=slide&audience=…`) on `target`. Returns the bus
   * to talk to it when it is not reachable through `Platform.channel` (Presentation API).
   */
  open(query: string, target: DisplayTarget): Promise<import('./types').MessageBus | null>;
}

// ------------------------------------------------------------------ recording (SLIDE-REC-001)

/** Corner of the slide for the recorded camera picture. */
export type SlideCameraCorner = 'topLeft' | 'topRight' | 'bottomLeft' | 'bottomRight';

/** A recorded clip for `saveRecording`: base64 bytes and MIME type. */
export interface SlideRecordedClip {
  data: string;
  mime: string;
}

/** One ink stroke drawn while recording; `times` are ms from the slide's start. */
export interface SlideRecordedStroke {
  points: [number, number][];
  times: number[];
  color: string;
  width: number;
  highlighter?: boolean;
}

/** The recording of one slide (`bo_engine::slide_edit::record::SlideRecordingIn`). */
export interface SlideRecordingInput {
  slide: number;
  /** Time on the slide → `advTm`. */
  durationMs?: number;
  /** Times of the clicks that played effects (ms from the slide's start). */
  clicks?: number[];
  ink?: SlideRecordedStroke[];
  narration?: SlideRecordedClip;
  camera?: SlideRecordedClip & { corner?: SlideCameraCorner; aspect?: number };
}

/** What one slide has recorded. */
export interface SlideRecordingInfo {
  slide: number;
  /** Shape id of the narration clip. */
  narration: number | null;
  camera: number | null;
  /** Shape ids of recorded ink. */
  ink: number[];
  /** `advTm` (ms). */
  timingMs: number | null;
}

export type SlideRecordCommand =
  | { cmd: 'recordingInfo' }
  | {
      cmd: 'saveRecording';
      slides: SlideRecordingInput[];
      names?: { narration?: string; camera?: string; ink?: string };
    }
  | {
      cmd: 'clearRecording';
      scope?: 'current' | 'all';
      what?: 'recording' | 'timings' | 'all';
    };
