/**
 * BlueSlide masters, layouts, themes and slide size (SLIDE-MASTER-001): the
 * commands of `bo_engine::slide_edit::master` and their results. Sizes are in
 * EMU (ECMA-376 Part 1, §20.1.10.16), placeholder boxes in slide points.
 */

/** Placeholder kinds a layout can receive (§19.7.10 ST_PlaceholderType). */
export type SlidePlaceholderKind =
  'content' | 'text' | 'picture' | 'chart' | 'table' | 'diagram' | 'media' | 'onlineImage';

export const SLIDE_PLACEHOLDER_KINDS: SlidePlaceholderKind[] = [
  'content',
  'text',
  'picture',
  'chart',
  'table',
  'diagram',
  'media',
  'onlineImage',
];

/** `scale`: fill the new size (may extend past it) · `fit`: keep everything visible. */
export type SlideSizeMode = 'scale' | 'fit';

export type SlideMasterCommand =
  | { cmd: 'masterView'; on: boolean }
  | { cmd: 'insertLayout'; name: string }
  | { cmd: 'duplicateLayout'; name?: string }
  | { cmd: 'renameMasterItem'; name: string }
  | { cmd: 'deleteMasterItem' }
  | { cmd: 'preserveMaster'; on: boolean }
  | {
      cmd: 'insertPlaceholder';
      kind: SlidePlaceholderKind;
      rect?: { x: number; y: number; w: number; h: number };
    }
  | { cmd: 'layoutElements'; title?: boolean; footers?: boolean }
  | { cmd: 'hideBackgroundGraphics'; hide: boolean }
  | { cmd: 'setBackground'; color?: string; all?: boolean }
  | {
      cmd: 'masterTextStyle';
      style: 'title' | 'body' | 'other';
      level?: number;
      font?: string;
      size?: number;
      bold?: boolean;
      italic?: boolean;
      color?: string;
    }
  | { cmd: 'addMaster'; name: string }
  /** `file`: a `.thmx` / `.pptx` in base64. */
  | { cmd: 'applyTheme'; builtin?: string; file?: string; all?: boolean }
  | { cmd: 'themeVariant'; colors?: string; fonts?: string; all?: boolean }
  | { cmd: 'setSlideSize'; cx: number; cy: number; kind?: string; mode: SlideSizeMode }
  | { cmd: 'applyLayout'; layout: number; slides?: number[] };

/** A built-in theme or variant (preview colours `RRGGBB`, heading / body typefaces). */
export interface SlideDesignChoice {
  id: string;
  colors: string[];
  fonts: [string, string] | null;
}

export interface SlideDesignInfo {
  cx: number;
  cy: number;
  sizeKind: string | null;
  theme: string | null;
  colors: string;
  fonts: string;
  /** Masters and their layouts (normal view). */
  masters: {
    index: number;
    name: string;
    theme: string | null;
    layouts: { index: number; name: string }[];
  }[];
  themes: SlideDesignChoice[];
  colorVariants: SlideDesignChoice[];
  fontVariants: SlideDesignChoice[];
  /** `saveThemeFile`: the theme as a `.thmx` file, base64. */
  themeFile?: string;
}

/** One entry of the master view's slide pane. */
export interface SlideMasterItem {
  kind: 'master' | 'layout';
  name: string;
  master: number;
  uses: number;
  preserved: boolean;
  hideGraphics: boolean;
  title: boolean;
  footers: boolean;
}

export interface SlideMasterViewInfo {
  items: SlideMasterItem[];
}

/** Slide sizes in EMU (§19.2.1.39 sldSz; 914 400 EMU = 1 inch). */
export const SLIDE_SIZES = {
  standard: { cx: 9_144_000, cy: 6_858_000, kind: 'screen4x3' },
  widescreen: { cx: 12_192_000, cy: 6_858_000, kind: undefined },
} as const;

/** ST_SlideSizeCoordinate range (§19.2.1.39), EMU. */
export const SLIDE_SIZE_MIN = 914_400;
export const SLIDE_SIZE_MAX = 51_206_400;
