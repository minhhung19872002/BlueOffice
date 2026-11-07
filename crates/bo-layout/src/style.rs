//! Effective properties (ECMA-376 Part 1, §17.7.2 Style hierarchy):
//! document defaults → paragraph style chain → numbering level → direct formatting;
//! runs additionally get the character style chain.

use bo_model::{
    Color, Document, NumRef, ParaProps, Paragraph, RunProps, StyleKind, Underline, VertAlign,
};
use bo_text::{FaceId, FontManager};

/// Word-processor defaults when neither direct formatting nor styles specify a value
/// (ECMA-376 Part 1, §17.7.5.5: 10 pt; application default face).
pub const DEFAULT_FONT: &str = "Times New Roman";
pub const DEFAULT_SIZE_PT: f32 = 10.0;

/// Fully resolved run formatting used by layout.
#[derive(Debug, Clone, PartialEq)]
pub struct RunStyle {
    pub face: FaceId,
    /// Effective glyph size (after super/subscript and small caps scaling).
    pub size: f32,
    /// Baseline shift (points, positive = down).
    pub shift: f32,
    pub color: Color,
    pub underline: Underline,
    pub strike: bool,
    pub double_strike: bool,
    pub highlight: Option<Color>,
    pub caps: bool,
    /// Extra advance after each character (points).
    pub spacing: f32,
    pub hidden: bool,
    pub link: Option<String>,
}

/// Effective paragraph properties of `p`.
pub fn para_props(doc: &Document, p: &Paragraph) -> ParaProps {
    let style_id = p.props.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    });
    let mut eff = doc.default_para.clone();
    if let Some(id) = &style_id {
        let (sp, _) = doc.styles.resolve_paragraph(id);
        eff = sp.inherit(&eff);
    }
    let num = p.props.num.or(eff.num);
    if let Some(n) = num.filter(|n| n.num_id != 0) {
        if let Some(level) = doc.numbering.level(n) {
            eff = level.ppr.inherit(&eff);
        }
    }
    let mut out = p.props.inherit(&eff);
    out.style = style_id;
    out.num = num.filter(|n| n.num_id != 0);
    out
}

/// Run properties contributed by the paragraph's style chain.
pub fn para_style_run(doc: &Document, para: &ParaProps) -> RunProps {
    let mut r = doc.default_run.clone();
    if let Some(id) = &para.style {
        let (_, sr) = doc.styles.resolve_paragraph(id);
        r = sr.inherit(&r);
    }
    r
}

/// Effective run properties for a run inside a paragraph whose style run props are `base`.
pub fn run_props(doc: &Document, base: &RunProps, direct: &RunProps) -> RunProps {
    let mut r = base.clone();
    if let Some(id) = &direct.style {
        r = doc.styles.resolve_character(id).inherit(&r);
    }
    direct.inherit(&r)
}

/// Turn effective run properties into a layout style.
pub fn run_style(fonts: &mut FontManager, rp: &RunProps) -> Option<RunStyle> {
    let face = fonts.resolve(
        rp.font.as_deref().unwrap_or(DEFAULT_FONT),
        rp.bold.unwrap_or(false),
        rp.italic.unwrap_or(false),
    )?;
    let mut size = rp.size.map_or(DEFAULT_SIZE_PT, bo_model::HalfPoints::to_pt);
    let mut shift = 0.0;
    match rp.vert_align {
        // Super/subscript at ~2/3 size (common word-processor rendering).
        Some(VertAlign::Superscript) => {
            shift = -size * 0.33;
            size *= 0.65;
        }
        Some(VertAlign::Subscript) => {
            shift = size * 0.14;
            size *= 0.65;
        }
        _ => {}
    }
    let small_caps = rp.small_caps.unwrap_or(false);
    if small_caps && !rp.caps.unwrap_or(false) {
        size *= 0.8;
    }
    Some(RunStyle {
        face,
        size,
        shift,
        color: rp.color.unwrap_or(Color::BLACK),
        underline: rp.underline.unwrap_or(Underline::None),
        strike: rp.strike.unwrap_or(false),
        double_strike: rp.double_strike.unwrap_or(false),
        highlight: rp.highlight.or(rp.shading),
        caps: rp.caps.unwrap_or(false) || small_caps,
        spacing: rp.spacing.map_or(0.0, bo_model::Twips::to_pt),
        hidden: rp.hidden.unwrap_or(false),
        link: rp.link.clone(),
    })
}

/// List counters (ECMA-376 Part 1, §17.9 — numbering continues per abstract definition).
#[derive(Default, Debug, Clone)]
pub struct ListCounters {
    /// abstract id → counters per level.
    counters: std::collections::HashMap<i32, [i32; 9]>,
    /// num id that last used an abstract id (for startOverride restarts).
    seen_nums: std::collections::HashSet<i32>,
}

impl ListCounters {
    /// Advance the counter for `n` and return the label text and level run props.
    pub fn next(&mut self, doc: &Document, n: NumRef) -> Option<(String, RunProps, String)> {
        let level = doc.numbering.level(n)?;
        let abs = doc.numbering.abstract_of(n.num_id)?;
        let first_use = self.seen_nums.insert(n.num_id);
        let has_override = doc
            .numbering
            .nums
            .iter()
            .any(|x| x.id == n.num_id && !x.start_overrides.is_empty());
        let entry = self.counters.entry(abs).or_insert([0; 9]);
        let il = usize::from(n.ilvl.min(8));
        if first_use && has_override {
            *entry = [0; 9];
        }
        if entry[il] == 0 {
            entry[il] = doc.numbering.start(n);
        } else {
            entry[il] += 1;
        }
        // Deeper levels restart (§17.9.10 lvlRestart default behaviour).
        for v in entry.iter_mut().skip(il + 1) {
            *v = 0;
        }
        let counters = *entry;
        let mut label = level.text.clone();
        for (k, &count) in counters.iter().enumerate() {
            let pat = format!("%{}", k + 1);
            if label.contains(&pat) {
                let lv = doc.numbering.level(NumRef {
                    num_id: n.num_id,
                    ilvl: k as u8,
                });
                let value = if count == 0 {
                    lv.map_or(1, |l| l.start)
                } else {
                    count
                };
                let fmt = lv.map_or(bo_model::NumFormat::Decimal, |l| l.format.clone());
                label = label.replace(&pat, &bo_model::format_number(value, &fmt));
            }
        }
        Some((label, level.rpr.clone(), level.suffix.clone()))
    }
}
