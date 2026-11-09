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
  | { cmd: 'pictureInsertSvg'; data: string; name?: string };
