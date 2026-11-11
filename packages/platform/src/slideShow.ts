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

/**
 * What a hyperlink or action does (§21.1.2.3.5 hlinkClick, §21.1.2.3.6 hlinkHover;
 * `ppaction://` URLs). `program`, `macro`, `file`, `presentation` and `unknown` are
 * kept in the file and never run (docs/security.md).
 */
export type SlideLinkAction =
  | { kind: 'none' }
  | { kind: 'customShow'; id: number; return?: boolean }
  | { kind: 'slide'; slide: number }
  | { kind: 'next' | 'prev' | 'first' | 'last' | 'lastViewed' | 'end' }
  /** A URL or mail address (external relationship). */
  | { kind: 'url'; url: string }
  /** Another presentation (`ppaction://hlinkpres`); `slide` is 1-based. */
  | { kind: 'presentation'; path: string; slide?: number | null }
  /** Another file (`ppaction://hlinkfile`). */
  | { kind: 'file'; path: string }
  | { kind: 'program'; path: string }
  | { kind: 'macro'; name: string }
  | { kind: 'unknown'; action: string }
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

/** What starts an action. */
export type SlideActionTrigger = 'click' | 'hover';

/**
 * What a show may do with an action (`bo_engine::slide_edit::action::Safety`):
 * follow it at once, open it after asking (http, https, mailto), or never.
 */
export type SlideActionSafety = 'internal' | 'external' | 'blocked';

/** The sound of an action (§20.1.2.2.32 snd): an embedded WAV part, or "stop the sound". */
export interface SlideActionSound {
  stop: boolean;
  /** Part name (bytes via `EngineHost.getMedia`). */
  part: string | null;
  name: string;
  loop: boolean;
}

/**
 * A hot area of the show: a shape or a piece of linked text, with its box
 * `[x, y, w, h]` (points). The engine sends every field; fixtures may leave out
 * the ones after `action` (a plain click link).
 */
export interface SlideShowLink {
  shapeId: number;
  rect: [number, number, number, number];
  action: SlideLinkAction;
  on?: SlideActionTrigger;
  /** A piece of text: its colour shows whether the link was followed. */
  text?: boolean;
  tooltip?: string;
  /** Flash the area when the action runs (`highlightClick`). */
  highlight?: boolean;
  sound?: SlideActionSound;
  safety?: SlideActionSafety;
  /** Identity of the target (followed state). */
  key?: string;
  history?: boolean;
}

/** The sound of an action being set. */
export type SlideActionSoundSpec =
  | { kind: 'keep' }
  | { kind: 'none' }
  | { kind: 'stop' }
  /** A WAV file, base64. */
  | { kind: 'file'; name: string; data: string };

/** An action being set on the selection. */
export interface SlideActionSpec {
  target: SlideLinkAction;
  tooltip?: string | null;
  highlight?: boolean;
  /** Default: keep the sound. */
  sound?: SlideActionSoundSpec;
}

/** One action of the selection, as the dialogs show it. */
export interface SlideActionOut {
  target: SlideLinkAction;
  tooltip?: string;
  highlight: boolean;
  sound?: SlideActionSound;
  safety: SlideActionSafety;
  /** The shape's own action (a media clip, an embedded object): not editable. */
  locked: boolean;
}

/** Answer of the `action*` commands (`SlideEditResult.action`). */
export interface SlideActionInfo {
  /** A text range, the selected shapes, or nothing to link. */
  scope: 'text' | 'shape' | 'none';
  click: SlideActionOut | null;
  hover: SlideActionOut | null;
  /** The selected text, or the text of the selected shape. */
  text: string;
  /** Title of every slide. */
  slides: string[];
}

/** Hyperlinks and actions (SLIDE-ACTION-001, `bo_engine::slide_edit::action`). */
export type SlideActionCommand =
  | { cmd: 'actionInfo' }
  | {
      cmd: 'actionSet';
      on?: SlideActionTrigger;
      spec: SlideActionSpec;
      /** `auto`: the selected text range if any, else the selected shapes. */
      scope?: 'auto' | 'shape';
    }
  /** An action button (`actionButton*` preset) with its usual action. */
  | {
      cmd: 'actionButton';
      preset: string;
      rect?: { x: number; y: number; w: number; h: number };
    }
  /** A link was followed in the show (its text takes the followed colour). */
  | { cmd: 'actionFollowed'; key: string }
  | { cmd: 'actionResetFollowed' };

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
  /** Hide slides from the show / show them again (absent `hidden`: toggle; default: current slide). */
  | { cmd: 'hideSlides'; slides?: number[]; hidden?: boolean }
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
