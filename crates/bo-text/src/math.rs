//! OpenType MATH table access (OpenType 1.9, "MATH — The Mathematical Typesetting Table"): layout
//! constants, glyph extents and stretchy glyphs (size variants and glyph assemblies) of a math font.
//!
//! All values are returned in font units; the caller scales them by `size / units_per_em`.

use crate::{FaceId, FontManager};
use ttf_parser::GlyphId;

/// Layout constants of the MATH table (`MathConstants`), font units.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)] // names follow the OpenType MathConstants fields
pub struct MathConstants {
    pub units_per_em: u16,
    pub script_percent_scale_down: i16,
    pub script_script_percent_scale_down: i16,
    pub delimited_sub_formula_min_height: u16,
    pub display_operator_min_height: u16,
    pub axis_height: i16,
    pub accent_base_height: i16,
    pub subscript_shift_down: i16,
    pub subscript_top_max: i16,
    pub subscript_baseline_drop_min: i16,
    pub superscript_shift_up: i16,
    pub superscript_bottom_min: i16,
    pub superscript_baseline_drop_max: i16,
    pub sub_superscript_gap_min: i16,
    pub superscript_bottom_max_with_subscript: i16,
    pub space_after_script: i16,
    pub upper_limit_gap_min: i16,
    pub upper_limit_baseline_rise_min: i16,
    pub lower_limit_gap_min: i16,
    pub lower_limit_baseline_drop_min: i16,
    pub fraction_numerator_shift_up: i16,
    pub fraction_numerator_display_style_shift_up: i16,
    pub fraction_denominator_shift_down: i16,
    pub fraction_denominator_display_style_shift_down: i16,
    pub fraction_numerator_gap_min: i16,
    pub fraction_num_display_style_gap_min: i16,
    pub fraction_rule_thickness: i16,
    pub fraction_denominator_gap_min: i16,
    pub fraction_denom_display_style_gap_min: i16,
    pub overbar_vertical_gap: i16,
    pub overbar_rule_thickness: i16,
    pub overbar_extra_ascender: i16,
    pub underbar_vertical_gap: i16,
    pub underbar_rule_thickness: i16,
    pub underbar_extra_descender: i16,
    pub radical_vertical_gap: i16,
    pub radical_display_style_vertical_gap: i16,
    pub radical_rule_thickness: i16,
    pub radical_extra_ascender: i16,
    pub radical_kern_before_degree: i16,
    pub radical_kern_after_degree: i16,
    pub radical_degree_bottom_raise_percent: i16,
}

/// A glyph grown to a size: one glyph (the base or a size variant) or an assembly of parts.
#[derive(Debug, Clone, PartialEq)]
pub enum Stretched {
    /// A single glyph.
    Glyph(u16),
    /// Parts stacked along the stretch direction: (glyph, offset of its origin from the first part's
    /// origin, font units — upwards for vertical, rightwards for horizontal), and the total size.
    Assembly {
        /// (glyph id, offset).
        parts: Vec<(u16, i32)>,
        /// Total extent of the assembly along the stretch direction.
        size: i32,
    },
}

/// Ink extents of a glyph, font units: `y_max` above and `y_min` below the baseline (y up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlyphExtents {
    /// Horizontal advance.
    pub advance: u16,
    /// Lowest ink point (negative below the baseline).
    pub y_min: i16,
    /// Highest ink point.
    pub y_max: i16,
    /// Left ink edge.
    pub x_min: i16,
    /// Right ink edge.
    pub x_max: i16,
}

impl FontManager {
    /// The MATH constants of a face, if it is a math font.
    #[must_use]
    pub fn math_constants(&self, id: FaceId) -> Option<MathConstants> {
        let face = self.face(id)?;
        let c = face.tables().math?.constants?;
        Some(MathConstants {
            units_per_em: face.units_per_em(),
            script_percent_scale_down: c.script_percent_scale_down(),
            script_script_percent_scale_down: c.script_script_percent_scale_down(),
            delimited_sub_formula_min_height: c.delimited_sub_formula_min_height(),
            display_operator_min_height: c.display_operator_min_height(),
            axis_height: c.axis_height().value,
            accent_base_height: c.accent_base_height().value,
            subscript_shift_down: c.subscript_shift_down().value,
            subscript_top_max: c.subscript_top_max().value,
            subscript_baseline_drop_min: c.subscript_baseline_drop_min().value,
            superscript_shift_up: c.superscript_shift_up().value,
            superscript_bottom_min: c.superscript_bottom_min().value,
            superscript_baseline_drop_max: c.superscript_baseline_drop_max().value,
            sub_superscript_gap_min: c.sub_superscript_gap_min().value,
            superscript_bottom_max_with_subscript: c.superscript_bottom_max_with_subscript().value,
            space_after_script: c.space_after_script().value,
            upper_limit_gap_min: c.upper_limit_gap_min().value,
            upper_limit_baseline_rise_min: c.upper_limit_baseline_rise_min().value,
            lower_limit_gap_min: c.lower_limit_gap_min().value,
            lower_limit_baseline_drop_min: c.lower_limit_baseline_drop_min().value,
            fraction_numerator_shift_up: c.fraction_numerator_shift_up().value,
            fraction_numerator_display_style_shift_up: c
                .fraction_numerator_display_style_shift_up()
                .value,
            fraction_denominator_shift_down: c.fraction_denominator_shift_down().value,
            fraction_denominator_display_style_shift_down: c
                .fraction_denominator_display_style_shift_down()
                .value,
            fraction_numerator_gap_min: c.fraction_numerator_gap_min().value,
            fraction_num_display_style_gap_min: c.fraction_num_display_style_gap_min().value,
            fraction_rule_thickness: c.fraction_rule_thickness().value,
            fraction_denominator_gap_min: c.fraction_denominator_gap_min().value,
            fraction_denom_display_style_gap_min: c.fraction_denom_display_style_gap_min().value,
            overbar_vertical_gap: c.overbar_vertical_gap().value,
            overbar_rule_thickness: c.overbar_rule_thickness().value,
            overbar_extra_ascender: c.overbar_extra_ascender().value,
            underbar_vertical_gap: c.underbar_vertical_gap().value,
            underbar_rule_thickness: c.underbar_rule_thickness().value,
            underbar_extra_descender: c.underbar_extra_descender().value,
            radical_vertical_gap: c.radical_vertical_gap().value,
            radical_display_style_vertical_gap: c.radical_display_style_vertical_gap().value,
            radical_rule_thickness: c.radical_rule_thickness().value,
            radical_extra_ascender: c.radical_extra_ascender().value,
            radical_kern_before_degree: c.radical_kern_before_degree().value,
            radical_kern_after_degree: c.radical_kern_after_degree().value,
            radical_degree_bottom_raise_percent: c.radical_degree_bottom_raise_percent(),
        })
    }

    /// Ink extents and advance of a glyph.
    #[must_use]
    pub fn glyph_extents(&self, id: FaceId, glyph: u16) -> GlyphExtents {
        let Some(face) = self.face(id) else {
            return GlyphExtents::default();
        };
        let g = GlyphId(glyph);
        let advance = face.glyph_hor_advance(g).unwrap_or(0);
        face.glyph_bounding_box(g).map_or(
            GlyphExtents {
                advance,
                ..GlyphExtents::default()
            },
            |b| GlyphExtents {
                advance,
                y_min: b.y_min,
                y_max: b.y_max,
                x_min: b.x_min,
                x_max: b.x_max,
            },
        )
    }

    /// Italic correction of a glyph (`MathItalicsCorrectionInfo`), font units.
    #[must_use]
    pub fn italic_correction(&self, id: FaceId, glyph: u16) -> i16 {
        self.face(id)
            .and_then(|f| {
                f.tables()
                    .math?
                    .glyph_info?
                    .italic_corrections?
                    .get(GlyphId(glyph))
            })
            .map_or(0, |v| v.value)
    }

    /// The glyph for `c` in a face (0 when missing).
    #[must_use]
    pub fn glyph_of(&self, id: FaceId, c: char) -> u16 {
        self.face(id)
            .and_then(|f| f.glyph_index(c))
            .map_or(0, |g| g.0)
    }

    /// `glyph` grown to at least `target` font units (vertically or horizontally): the smallest size
    /// variant that is large enough, else the glyph assembly, else the largest variant
    /// (`MathVariants`, `MathGlyphConstruction`, `GlyphAssembly`).
    #[must_use]
    pub fn stretch(&self, id: FaceId, glyph: u16, target: i32, vertical: bool) -> Stretched {
        let Some(face) = self.face(id) else {
            return Stretched::Glyph(glyph);
        };
        let Some(variants) = face.tables().math.and_then(|m| m.variants) else {
            return Stretched::Glyph(glyph);
        };
        let constructions = if vertical {
            variants.vertical_constructions
        } else {
            variants.horizontal_constructions
        };
        let Some(c) = constructions.get(GlyphId(glyph)) else {
            return Stretched::Glyph(glyph);
        };
        let mut largest = glyph;
        for v in c.variants {
            largest = v.variant_glyph.0;
            if i32::from(v.advance_measurement) >= target {
                return Stretched::Glyph(v.variant_glyph.0);
            }
        }
        match c.assembly {
            Some(a) => assemble(
                &a.parts.into_iter().collect::<Vec<_>>(),
                i32::from(variants.min_connector_overlap),
                target,
            )
            .unwrap_or(Stretched::Glyph(largest)),
            None => Stretched::Glyph(largest),
        }
    }
}

/// Builds an assembly of at least `target` units from `parts` (bottom-to-top / left-to-right), repeating
/// the extenders as few times as needed and spreading the connector overlap evenly (OpenType MATH,
/// "GlyphAssembly table").
fn assemble(
    parts: &[ttf_parser::math::GlyphPart],
    min_overlap: i32,
    target: i32,
) -> Option<Stretched> {
    if parts.is_empty() || !parts.iter().any(|p| p.part_flags.extender()) {
        return None;
    }
    for repeats in 0..=100usize {
        let list: Vec<&ttf_parser::math::GlyphPart> = parts
            .iter()
            .flat_map(|p| {
                let n = if p.part_flags.extender() { repeats } else { 1 };
                std::iter::repeat_n(p, n)
            })
            .collect();
        if list.is_empty() {
            continue;
        }
        let full: i32 = list.iter().map(|p| i32::from(p.full_advance)).sum();
        // Largest overlap allowed at each joint: the shorter of the two connectors.
        let joints: Vec<i32> = list
            .windows(2)
            .map(|w| {
                i32::from(w[0].end_connector_length.min(w[1].start_connector_length))
                    .max(min_overlap)
            })
            .collect();
        let max_size = full - min_overlap * joints.len() as i32;
        if max_size < target && repeats < 100 {
            continue;
        }
        // Even overlap that brings the size down to the target (never below the minimum).
        let n = joints.len().max(1) as i32;
        let wanted = ((full - target) / n).max(min_overlap);
        let mut parts_out = Vec::with_capacity(list.len());
        let mut pos = 0i32;
        for (i, p) in list.iter().enumerate() {
            parts_out.push((p.glyph_id.0, pos));
            pos += i32::from(p.full_advance);
            if let Some(&limit) = joints.get(i) {
                pos -= wanted.min(limit);
            }
        }
        return Some(Stretched::Assembly {
            parts: parts_out,
            size: pos,
        });
    }
    None
}
