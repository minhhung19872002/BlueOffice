/**
 * Advanced character formatting of BlueDoc (DOC-FMT-001), mirroring
 * `bo_edit::charfx`: effects (ECMA-376 Part 1, §17.3.2.13 emboss, §17.3.2.18 imprint,
 * §17.3.2.23 outline, §17.3.2.31 shadow), spacing / position / scale / kerning
 * (§17.3.2.35, §17.3.2.24, §17.3.2.43, §17.3.2.19), border and shading (§17.3.2.4,
 * §17.3.2.32), two lines in one (§17.3.2.10), the [MS-DOCX] w14 text effects and
 * OpenType typography, and phonetic guides (§17.3.3.25 ruby). Units: points,
 * degrees, percent, `RRGGBB`.
 */

/** Glow around the glyphs. */
export interface GlowSpec {
  color: string;
  /** Radius, points. */
  radius: number;
  /** Transparency, percent. */
  transparency: number;
}

/** Soft shadow behind the glyphs. */
export interface ShadowSpec {
  color: string;
  /** Blur radius, points. */
  blur: number;
  /** Distance, points. */
  distance: number;
  /** Direction, degrees clockwise from the right. */
  angle: number;
  transparency: number;
}

/** Fading mirror image under the text. */
export interface ReflectionSpec {
  /** Fade length, percent of the text height. */
  size: number;
  /** Transparency at the start, percent. */
  transparency: number;
  /** Gap, points. */
  distance: number;
}

/** Stroke of the glyph outlines. */
export interface OutlineSpec {
  color: string;
  /** Width, points. */
  width: number;
}

/** Two-colour linear gradient fill of the glyphs. */
export interface GradientSpec {
  from: string;
  to: string;
  /** Direction, degrees clockwise from left → right. */
  angle: number;
}

/** Border around the run (§17.18.2 `ST_Border` style, width in eighths of a point). */
export interface CharBorderSpec {
  style: string;
  size: number;
  color: string | null;
}

/** Advanced formatting at the selection (`FormatState.advanced`). */
export interface CharAdvanced {
  emboss: boolean;
  imprint: boolean;
  outline: boolean;
  shadow: boolean;
  /** Twips (0 = normal). */
  spacing: number;
  /** Half-points, positive = raised. */
  position: number;
  /** Percent. */
  scale: number;
  /** Kerning from this size (half-points); null = off. */
  kern: number | null;
  glow: GlowSpec | null;
  softShadow: ShadowSpec | null;
  reflection: ReflectionSpec | null;
  textOutline: OutlineSpec | null;
  gradient: GradientSpec | null;
  /** `''`, `none`, `standard`, `contextual`, `historical`, `discretional`, combinations, `all`. */
  ligatures: string;
  /** `''`, `default`, `lining`, `oldStyle`. */
  numForm: string;
  /** `''`, `default`, `proportional`, `tabular`. */
  numSpacing: string;
  /** 1–20, 0 = none. */
  stylisticSet: number;
  contextualAlternates: boolean;
  border: CharBorderSpec | null;
  shading: string | null;
  combine: boolean;
  /** `''`, `round`, `square`, `angle`, `curly`. */
  combineBrackets: string;
}

/** Changes sent by the Font dialog: absent = unchanged; `null` removes an effect. */
export type CharAdvancedPatch = Partial<CharAdvanced>;

/** A phonetic guide (§17.3.3.25 ruby). Sizes in half-points. */
export interface RubySpec {
  base: string;
  guide: string;
  /** `center`, `distributeLetter`, `distributeSpace`, `left`, `right`. */
  align: string;
  size: number;
  /** Guide baseline above the base baseline. */
  raise: number;
  baseSize: number;
  lang: string;
}
