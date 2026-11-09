/**
 * Playback options of BlueSlide video / audio clips (SLIDE-MEDIA-001). Mirrors
 * `bo_engine::slide_edit::media_playback`: trim, fades and bookmarks ([MS-PPTX] p14:media),
 * WebVTT caption tracks ([MS-PPTX] p1730:tracksInfo), the media time node (ECMA-376 Part 1,
 * §19.5 p:video / p:audio → p:cMediaNode) and the clip's start effect.
 */

/** A bookmark of a clip ([MS-PPTX] p14:bmk). */
export interface SlideMediaBookmark {
  name: string;
  /** From the start of the media, ms. */
  timeMs: number;
}

/** A caption track ([MS-PPTX] CT_Track). */
export interface SlideMediaCaption {
  id: string;
  label: string;
  lang: string | null;
  /** Embedded WebVTT part (bytes via `EngineHost.getMedia`). */
  part: string | null;
}

/** How a clip starts: with the slide / after the previous effect, on a click of the click
 * sequence, on a click on the clip. */
export type SlideMediaStart = 'auto' | 'seq' | 'click';

/** How a clip plays. */
export interface SlideMediaPlayback {
  trimStartMs: number;
  trimEndMs: number;
  fadeInMs: number;
  fadeOutMs: number;
  bookmarks: SlideMediaBookmark[];
  /** `null` = only through its controls. */
  start: SlideMediaStart | null;
  /** Click step of the main sequence on which the clip starts (0 = with the slide). */
  playClick: number | null;
  /** Percent. */
  volume: number;
  mute: boolean;
  /** Slides the clip plays across (1 = this slide only). */
  slides: number;
  loop: boolean;
  rewind: boolean;
  hideWhenStopped: boolean;
  fullScreen: boolean;
  captions: SlideMediaCaption[];
}

/** `numSld` written for "play across slides" (BlueOffice: until the show ends). */
export const MEDIA_ACROSS_SLIDES = 999;

/** Media playback commands (clip `id`, default: the selected clip). */
export type SlideMediaCommand =
  | {
      cmd: 'mediaPlayback';
      id?: number;
      trimStartMs?: number;
      trimEndMs?: number;
      fadeInMs?: number;
      fadeOutMs?: number;
      start?: SlideMediaStart | 'none';
      volume?: number;
      mute?: boolean;
      slides?: number;
      loop?: boolean;
      rewind?: boolean;
      hideWhenStopped?: boolean;
      fullScreen?: boolean;
    }
  | { cmd: 'mediaBookmarkAdd'; id?: number; name?: string; timeMs: number }
  | { cmd: 'mediaBookmarkRemove'; id?: number; name: string }
  | { cmd: 'mediaCaptionAdd'; id?: number; text: string; label?: string; lang?: string }
  | { cmd: 'mediaCaptionRemove'; id?: number; track: string }
  | { cmd: 'mediaBookmarkTrigger'; index: number; media: number; bookmark?: string | null };
