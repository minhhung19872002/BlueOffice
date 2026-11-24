/**
 * Picture tools shared by BlueDoc and BlueSlide (CORE-PIC-001): colour adjustments of a
 * picture's `a:blip` (ECMA-376 Part 1, §20.1.8), crop, compress, change, reset and SVG → shapes.
 * Mirrors `bo_model::picture::PictureFx` and `bo_engine::picture_tools`.
 */

/** §20.1.8.16 clrChange: pixels of `from` become `to` with opacity `alpha`. */
export interface ColorChange {
  /** `0xRRGGBB`. */
  from: number;
  /** `0xRRGGBB`. */
  to: number;
  /** Opacity of `to`, 1/1000 % (0 = transparent). */
  alpha: number;
}

/** Colour adjustments; percentages in 1/1000 % (100000 = 100 %). */
export interface PictureFx {
  /** §20.1.8.42 lum@bright. */
  bright?: number;
  /** §20.1.8.42 lum@contrast. */
  contrast?: number;
  /** Sharpen (> 0) or soften (< 0). */
  sharpen?: number;
  /** Saturation, 100000 = unchanged. */
  saturation?: number | null;
  /** §20.1.8.34 grayscl. */
  grayscale?: boolean;
  /** §20.1.8.11 biLevel threshold. */
  bilevel?: number | null;
  /** §20.1.8.23 duotone: dark and light colour (`0xRRGGBB`). */
  duotone?: [number, number] | null;
  /** §20.1.8.16 clrChange. */
  colorChange?: ColorChange | null;
  /** §20.1.8.4 alphaModFix opacity. */
  alpha?: number | null;
}

/** "Compress pictures" settings. */
export interface CompressSpec {
  /** Target pixels per inch (absent keeps the resolution). */
  ppi?: number | null;
  /** Remove cropped areas. */
  dropCropped?: boolean;
  /** JPEG quality 1–100 for opaque pictures. */
  jpegQuality?: number | null;
  /** Every picture of the document instead of the selected ones. */
  all?: boolean;
}

/** What "Compress pictures" did. */
export interface CompressReport {
  pictures: number;
  before: number;
  after: number;
}

/** The selected picture (mirrors `bo_engine::picture_tools::PictureInfo`). */
export interface PictureInfo {
  fx: PictureFx;
  /** srcRect l, t, r, b (1/1000 %). */
  crop?: [number, number, number, number];
  /** Preset shape the picture is cropped to (`rect` = none). */
  shape: string;
  media: string;
  pixelWidth: number;
  pixelHeight: number;
  bytes: number;
  /** Displayed size (points). */
  width: number;
  height: number;
  /** Resolution as shown (pixels per inch). */
  ppi: number;
  /** Has an SVG version that can become shapes. */
  svg: boolean;
  /** Gallery previews (PNG base64), in request order. */
  previews?: string[];
  compressed?: CompressReport;
  /** The background can be removed (a raster picture, CORE-PIC-002). */
  removable?: boolean;
  /** Settings of the removed background, when it was removed. */
  removal?: RemovalSpec;
  /** Preview of the "Remove background" mode. */
  background?: BackgroundPreview;
}

/** A stroke of the background removal mode: points as fractions (0..1) of the picture. */
export interface BackgroundMark {
  /** "Mark areas to keep" (else "to remove"). */
  keep: boolean;
  points: [number, number][];
}

/**
 * Settings of a background removal (mirrors `bo_engine::picture_bg::RemovalSpec`); stored as
 * [MS-ODRAWXML] `a14:backgroundRemoval` next to the original picture (`a14:imgLayer`).
 */
// A type alias (not an interface): intersected into commands that must fit an index signature.
export type RemovalSpec = {
  /** Rectangle holding the subject: left, top, right, bottom fractions. */
  rect?: [number, number, number, number];
  marks?: BackgroundMark[];
  /** Starting estimate of a host segmentation provider (one byte per pixel, base64). */
  prior?: { width: number; height: number; data: string };
  /** Previews also return the working picture (`BackgroundPreview.source`). */
  source?: boolean;
};

/** Preview of the background removal: the reduced picture with the removal tinted. */
export interface BackgroundPreview {
  /** PNG base64. */
  image: string;
  width: number;
  height: number;
  /** Share kept, 1/1000 %. */
  kept: number;
  /** Segmentation time (ms; 0 in the browser, where the host measures it). */
  ms: number;
  source?: string;
}

/** Picture tools command (BlueDoc `EditCommand`, BlueSlide `SlideCommand`). */
export type PictureCommand =
  | { cmd: 'pictureInfo' | 'pictureFill' | 'pictureFit' | 'pictureToShapes' }
  | { cmd: 'picturePreviews'; presets: PictureFx[]; size?: number }
  | { cmd: 'pictureFx'; fx: PictureFx }
  | { cmd: 'pictureCrop'; crop: [number, number, number, number] }
  | { cmd: 'pictureCropAspect'; ratio: string }
  | { cmd: 'pictureCropShape'; preset: string }
  | ({ cmd: 'pictureCompress' } & CompressSpec)
  | { cmd: 'pictureChange'; data: string; name?: string }
  | { cmd: 'pictureReset'; size?: boolean }
  | { cmd: 'pictureInsertSvg'; data: string; name?: string }
  | ({ cmd: 'pictureBackgroundPreview' | 'pictureRemoveBackground' } & RemovalSpec)
  /** Text wrapping (§20.4.2.8 inline, §20.4.2.15–20.4.2.20). */
  | { cmd: 'pictureWrap'; wrap: PictureWrap }
  /** Position gallery: in line, or square-wrapped at one of nine places of the margins. */
  | { cmd: 'picturePosition'; position: 'inline' | `${'t' | 'm' | 'b'}${'l' | 'c' | 'r'}` }
  /** Align a floating picture to the margin or (`page`) the page (§20.4.2.1 / §20.4.2.2). */
  | {
      cmd: 'pictureAlign';
      align: 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom';
      page?: boolean;
    }
  /** Turn the picture clockwise by `degrees` (§20.1.7.6 xfrm rot). */
  | { cmd: 'pictureRotate'; degrees: number }
  | { cmd: 'pictureOrder'; order: 'forward' | 'front' | 'backward' | 'back' }
  | { cmd: 'pictureFlip'; axis: 'horizontal' | 'vertical' }
  /** Shape Format › Shape Fill: `RRGGBB` (§20.1.8.54 solidFill) or `none` (BlueDoc). */
  | { cmd: 'pictureShapeFill'; color: string }
  /** Shape Format › Shape Outline (§20.1.2.2.24 ln); what is left out stays (BlueDoc). */
  | { cmd: 'pictureShapeOutline'; color?: string; weight?: number; dash?: string }
  /** Shape Format › Change Shape: preset geometry (§20.1.9.18 prstGeom) (BlueDoc). */
  | { cmd: 'pictureShapePreset'; preset: string }
  /** Picture Border: `RRGGBB` or `none`, weight in points, preset dash (BlueDoc). */
  | { cmd: 'pictureBorder'; color?: string; weight?: number; dash?: string }
  /** Picture Effects: one effect kind set to a preset, or `none` to remove it (BlueDoc). */
  | {
      cmd: 'pictureEffect';
      kind: 'shadow' | 'reflection' | 'glow' | 'softEdge' | 'bevel' | 'rotation' | 'preset';
      preset: string;
    }
  /** Artistic Effects (a14 imgEffect name, or `none`) (BlueDoc). */
  | { cmd: 'pictureArtistic'; name: string }
  /** Picture Quick Styles gallery item (BlueDoc). */
  | { cmd: 'pictureQuickStyle'; index: number }
  /**
   * Shape Fill › Gradient (round 20, BlueDoc): a gallery variation (`light` / `dark`, index
   * 0–8), `none`, or explicit stops (§20.1.8.33 gradFill).
   */
  | {
      cmd: 'shapeGradient';
      variation?: 'light' | 'dark' | 'none';
      index?: number;
      kind?: 'linear' | 'circle' | 'rect' | 'shape';
      angle?: number;
      focus?: [number, number, number, number];
      stops?: { pos: number; color: string; alpha?: number }[];
    }
  /** Shape Fill › Picture / Texture: base64 picture, tiled or stretched (§20.1.8.14 blipFill). */
  | { cmd: 'shapePictureFill'; data: string; tile?: boolean; scale?: number; alpha?: number }
  /** Transparency of the fill and the line in percent (§20.1.2.3.1 alpha). */
  | { cmd: 'shapeTransparency'; fill?: number; line?: number }
  /** Shape Effects: as `pictureEffect`, on a shape. */
  | {
      cmd: 'shapeEffect';
      kind: 'shadow' | 'reflection' | 'glow' | 'softEdge' | 'bevel' | 'rotation' | 'preset';
      preset: string;
    }
  /** WordArt Styles › Text Fill: colour, `none`, or a two-colour gradient [from, to, angle]. */
  | { cmd: 'shapeTextFill'; color?: string; gradient?: [string, string, number] }
  /** WordArt Styles › Text Outline: colour or `none`, weight in points ([MS-DOCX] w14). */
  | { cmd: 'shapeTextOutline'; color?: string; weight?: number }
  /** WordArt Styles › Text Effects: shadow / reflection / glow preset, or `none`. */
  | { cmd: 'shapeTextEffect'; kind: 'shadow' | 'reflection' | 'glow' | 'none'; preset?: string }
  /** WordArt Styles gallery item. */
  | { cmd: 'wordArtStyle'; index: number }
  /** Text › Align Text: top, middle or bottom (§21.1.2.1.1 bodyPr anchor). */
  | { cmd: 'shapeTextAnchor'; anchor: 't' | 'ctr' | 'b' }
  /**
   * Round 21, Format Shape › Text Box: text insets left, top, right, bottom in points
   * (§21.1.2.1.1 bodyPr lIns / tIns / rIns / bIns). Effect presets of `pictureEffect` /
   * `shapeEffect` may also be `custom:key=value;…` settings from the Format panes (shadow:
   * inner, color, transparency, size, blur, angle, distance; reflection: transparency, size,
   * blur, distance; glow: size, color, transparency; bevel: type, width, height).
   */
  | { cmd: 'shapeTextInsets'; insets: [number, number, number, number] }
  | { cmd: 'shapeRotateWithShape'; on: boolean }
  | { cmd: 'shapeTextLayout'; vert?: 'horz' | 'vert' | 'vert270'; autoFit?: boolean; wrap?: boolean }
  | { cmd: 'shapeTextWarp'; preset: string };

/** Text wrapping of a picture (Wrap Text menu). */
export type PictureWrap =
  | 'inline'
  | 'square'
  | 'tight'
  | 'through'
  | 'topAndBottom'
  | 'behind'
  | 'inFront';
