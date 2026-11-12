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

/// Font slot of a character (ECMA-376 Part 1, §17.3.2.26 rFonts): which of the four
/// fonts of a run displays it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Ascii = 0,
    HAnsi = 1,
    EastAsia = 2,
    Cs = 3,
}

/// Slot of `c` in a run (§17.3.2.26, by Unicode block): complex script runs
/// (§17.3.2.7 cs, §17.3.2.30 rtl) and complex script blocks use the cs font, East Asian
/// blocks the eastAsia font, U+0000–U+007F the ascii font, everything else hAnsi —
/// except that `w:hint="eastAsia"` sends the characters shared between Latin and East
/// Asian use (Latin-1 symbols, general punctuation, letterlike symbols…) to eastAsia.
pub fn slot_of(c: char, hint_east_asia: bool, complex_run: bool) -> Slot {
    let u = c as u32;
    if complex_run {
        return Slot::Cs;
    }
    match u {
        0x0590..=0x08FF
        | 0x0900..=0x0DFF
        | 0x0E00..=0x0EFF
        | 0x1780..=0x17FF
        | 0xFB1D..=0xFDFF
        | 0xFE70..=0xFEFF => Slot::Cs,
        0x1100..=0x11FF
        | 0x2E80..=0x2FFF
        | 0x3000..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7AF
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFFEF
        | 0x20000..=0x2FFFF => Slot::EastAsia,
        0..=0x7F => Slot::Ascii,
        0x00A1..=0x00FF | 0x2000..=0x2BFF if hint_east_asia => Slot::EastAsia,
        _ => Slot::HAnsi,
    }
}

/// Font family names of the four slots of effective run properties (§17.3.2.26),
/// theme fonts (§17.18.96) resolved with the document's font scheme.
pub fn slot_families(doc: &Document, rp: &RunProps) -> [String; 4] {
    let th = |t: &Option<String>| t.as_deref().and_then(|t| doc.theme_fonts.resolve(t));
    let ascii = th(&rp.theme_ascii)
        .or(rp.font.as_deref())
        .unwrap_or(DEFAULT_FONT)
        .to_owned();
    let h_ansi = if rp.font_h_ansi.is_none() && rp.theme_h_ansi.is_none() {
        ascii.clone()
    } else {
        th(&rp.theme_h_ansi)
            .or(rp.font_h_ansi.as_deref())
            .map_or_else(|| ascii.clone(), str::to_owned)
    };
    let east = th(&rp.theme_east_asia)
        .or(rp.font_east_asia.as_deref())
        .map_or_else(|| h_ansi.clone(), str::to_owned);
    let cs = th(&rp.theme_cs)
        .or(rp.font_cs.as_deref())
        .map_or_else(|| h_ansi.clone(), str::to_owned);
    [ascii, h_ansi, east, cs]
}

/// Fully resolved run formatting used by layout.
#[derive(Debug, Clone, PartialEq)]
pub struct RunStyle {
    /// Face of the ascii slot (metrics of empty lines, labels, …).
    pub face: FaceId,
    /// Faces of the four font slots (see [`Slot`]).
    pub faces: [FaceId; 4],
    /// `w:hint="eastAsia"` (§17.18.42).
    pub hint_east_asia: bool,
    /// Complex script run (§17.3.2.7 cs / §17.3.2.30 rtl).
    pub complex: bool,
    /// Pair kerning applies (§17.3.2.19 kern: font size ≥ the threshold).
    pub kern: bool,
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
    /// Tracked insertion/deletion shown in markup: (author colour, deleted).
    pub revision: Option<(Color, bool)>,
    /// OpenType features for the shaper ([MS-DOCX] w14 typography).
    pub features: std::sync::Arc<[([u8; 4], u32)]>,
    /// Horizontal glyph scale (§17.3.2.43 w; 1 = 100 %).
    pub scale_x: f32,
    /// Border around the run (§17.3.2.4 bdr).
    pub border: Option<bo_model::Border>,
    /// Two lines in one (§17.3.2.10 eastAsianLayout `w:combine`).
    pub combine: bool,
    /// Brackets around combined text (§17.18.8 `ST_CombineBrackets`).
    pub combine_brackets: Option<(char, char)>,
    /// Glyph effects for the renderer.
    pub fx: Option<std::sync::Arc<crate::charfx::TextFx>>,
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
    // A style's numPr takes the level linked to the style (§17.9.23 pStyle).
    let num = p.props.num.or_else(|| {
        let n = eff.num?;
        Some(match &style_id {
            Some(id) => doc.numbering.level_for_style(n, id),
            None => n,
        })
    });
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
pub fn run_style(doc: &Document, fonts: &mut FontManager, rp: &RunProps) -> Option<RunStyle> {
    let (bold, italic) = (rp.bold.unwrap_or(false), rp.italic.unwrap_or(false));
    let fam = slot_families(doc, rp);
    let face = fonts.resolve(&fam[0], bold, italic)?;
    let mut faces = [face; 4];
    for i in 1..4 {
        if fam[i] != fam[0] {
            faces[i] = fonts.resolve(&fam[i], bold, italic).unwrap_or(face);
        }
    }
    // §17.3.2.19 kern: kerning from this size (half-points) up; absent = no kerning.
    let kern = match (rp.kern, rp.size) {
        (Some(k), Some(sz)) => k.0 > 0 && sz.0 >= k.0,
        (Some(k), None) => k.0 > 0 && DEFAULT_SIZE_PT * 2.0 >= k.0 as f32,
        _ => false,
    };
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
    // §17.3.2.24 position: raised (positive) or lowered text, in half-points.
    if let Some(pos) = rp.position {
        shift -= pos as f32 / 2.0;
    }
    let small_caps = rp.small_caps.unwrap_or(false);
    if small_caps && !rp.caps.unwrap_or(false) {
        size *= 0.8;
    }
    Some(RunStyle {
        face,
        faces,
        hint_east_asia: rp.font_hint.as_deref() == Some("eastAsia"),
        complex: rp.complex.unwrap_or(false) || rp.rtl.unwrap_or(false),
        kern,
        size,
        shift,
        // §17.3.2.6: a theme colour follows the document theme.
        color: doc.run_color(rp).unwrap_or(Color::BLACK),
        underline: rp.underline.unwrap_or(Underline::None),
        strike: rp.strike.unwrap_or(false),
        double_strike: rp.double_strike.unwrap_or(false),
        highlight: rp.highlight.or_else(|| crate::charfx::shading_color(rp)),
        caps: rp.caps.unwrap_or(false) || small_caps,
        spacing: rp.spacing.map_or(0.0, bo_model::Twips::to_pt),
        hidden: rp.hidden.unwrap_or(false),
        link: rp.link.clone(),
        revision: None,
        features: crate::charfx::features_of(rp).into(),
        scale_x: rp.scale.map_or(1.0, |w| w.clamp(1, 600) as f32 / 100.0),
        border: rp.border.clone().filter(bo_model::Border::is_visible),
        combine: rp
            .east_asian
            .as_ref()
            .is_some_and(|e| e.combine == Some(true)),
        combine_brackets: rp
            .east_asian
            .as_ref()
            .and_then(bo_model::run_fx::EastAsianLayout::brackets),
        fx: crate::charfx::TextFx::of(rp).map(std::sync::Arc::new),
    })
}

/// Colours used to tell revision authors apart (BlueDoc palette).
const AUTHOR_COLORS: [Color; 8] = [
    Color {
        r: 0xB3,
        g: 0x26,
        b: 0x1E,
    },
    Color {
        r: 0x1F,
        g: 0x5F,
        b: 0xA8,
    },
    Color {
        r: 0x2E,
        g: 0x7D,
        b: 0x32,
    },
    Color {
        r: 0x7B,
        g: 0x3F,
        b: 0xA0,
    },
    Color {
        r: 0xC0,
        g: 0x5A,
        b: 0x00,
    },
    Color {
        r: 0x00,
        g: 0x83,
        b: 0x8F,
    },
    Color {
        r: 0x8D,
        g: 0x6E,
        b: 0x00,
    },
    Color {
        r: 0xAD,
        g: 0x14,
        b: 0x57,
    },
];

/// Colour of tracked moves (§17.13.5.22 moveFrom, §17.13.5.25 moveTo), the same for
/// every author so that both halves of a move read as one change (BlueOffice's own
/// choice; the specification leaves the display to applications).
pub const MOVE_COLOR: Color = Color {
    r: 0x1B,
    g: 0x7A,
    b: 0x3E,
};

/// Stable colour of a revision author.
pub fn author_color(author: &str) -> Color {
    let h = bo_model::fingerprint(author);
    AUTHOR_COLORS[(h % AUTHOR_COLORS.len() as u64) as usize]
}

/// Apply the tracked-change display (ECMA-376 Part 1, §17.13.5) to a run style.
/// In the markup view, `marks` can leave the changes of hidden reviewers (or every
/// insertion/deletion) unmarked, shown as if accepted, and can move deleted text out
/// of the line into the markup area's balloons.
pub fn revision_style(
    mut st: RunStyle,
    rp: &RunProps,
    view: crate::RevisionView,
    marks: &crate::MarkFilter,
) -> RunStyle {
    use crate::RevisionView as V;
    // §17.3.2.41 vanish: shown with the formatting marks, dotted underline.
    if marks.show_hidden && rp.hidden == Some(true) {
        st.hidden = false;
        if st.underline == Underline::None {
            st.underline = Underline::Dotted;
        }
    }
    match view {
        V::Markup => {
            let plain = |author: &str| marks.hide_ins_del || marks.hides(author);
            // A deletion wins over the insertion it removes (inserted, then deleted).
            if let Some(d) = &rp.del {
                if plain(&d.info.author) || marks.deletions_in_balloons {
                    st.hidden = true;
                } else if d.moved {
                    // Moved away: double strikethrough in the move colour.
                    st.double_strike = true;
                    st.strike = false;
                    st.color = MOVE_COLOR;
                    st.revision = Some((MOVE_COLOR, true));
                } else {
                    st.strike = true;
                    st.color = author_color(&d.info.author);
                    st.revision = Some((st.color, true));
                }
            } else if let Some(i) = rp.ins.as_ref().filter(|i| !plain(&i.info.author)) {
                // Moved here: double underline in the move colour.
                let color = if i.moved {
                    MOVE_COLOR
                } else {
                    author_color(&i.info.author)
                };
                st.revision = Some((color, false));
                st.underline = if i.moved {
                    Underline::Double
                } else {
                    Underline::Single
                };
                st.color = color;
            }
        }
        V::Final => {
            if rp.del.is_some() {
                st.hidden = true;
            }
        }
        V::Original => {
            if rp.ins.is_some() {
                st.hidden = true;
            }
        }
    }
    st
}

/// List counters (ECMA-376 Part 1, §17.9 — numbering continues per abstract definition).
#[derive(Default, Debug, Clone, PartialEq)]
pub struct ListCounters {
    /// abstract id → counters per level.
    counters: std::collections::HashMap<i32, [i32; 9]>,
    /// num id that last used an abstract id (for startOverride restarts).
    seen_nums: std::collections::HashSet<i32>,
}

impl ListCounters {
    /// Advance the counter for `n` and return the label text and level run props.
    pub fn next(&mut self, doc: &Document, n: NumRef) -> Option<(String, RunProps, String)> {
        // One lookup of the instance and its abstract definition (§17.9.15 num,
        // §17.9.1 abstractNum): documents have hundreds of list instances.
        let num = doc.numbering.nums.iter().find(|x| x.id == n.num_id)?;
        let abs_def = doc
            .numbering
            .abstracts
            .iter()
            .find(|a| a.id == num.abstract_id)?;
        let level_of = |ilvl: u8| abs_def.levels.iter().find(|l| l.ilvl == ilvl);
        let level = level_of(n.ilvl)?;
        let abs = num.abstract_id;
        let first_use = self.seen_nums.insert(n.num_id);
        let has_override = !num.start_overrides.is_empty();
        let entry = self.counters.entry(abs).or_insert([0; 9]);
        let il = usize::from(n.ilvl.min(8));
        if first_use && has_override {
            // §17.9.9 lvlOverride / §17.9.26 startOverride: only the overridden levels (and
            // the levels below them) start again; the counters of the upper levels go on, so
            // restarting at level 2 keeps "3." above "3.1".
            for (l, _) in &num.start_overrides {
                for v in entry.iter_mut().skip(usize::from((*l).min(8))) {
                    *v = 0;
                }
            }
        }
        if entry[il] == 0 {
            // §17.9.26 startOverride, else the level's §17.9.25 start.
            entry[il] = num
                .start_overrides
                .iter()
                .find(|(l, _)| *l == n.ilvl)
                .map_or(level.start, |(_, s)| *s);
        } else {
            entry[il] += 1;
        }
        // ECMA-376 Part 1, §17.9.10 lvlRestart: a deeper level k starts again when a level
        // above the one-based level `val` is used (default: the level just above, i.e. any
        // upper level); `0` never restarts.
        for (k, v) in entry.iter_mut().enumerate().skip(il + 1) {
            let restart_after = level_of(k as u8)
                .and_then(|l| l.restart)
                .map_or(k, usize::from);
            if il < restart_after {
                *v = 0;
            }
        }
        let counters = *entry;
        // §17.9.4 isLgl: every number of the level text shows as a decimal (legal style).
        let legal = level.is_lgl;
        let mut label = level.text.clone();
        for (k, &count) in counters.iter().enumerate() {
            let pat = format!("%{}", k + 1);
            if label.contains(&pat) {
                let lv = level_of(k as u8);
                let value = if count == 0 {
                    lv.map_or(1, |l| l.start)
                } else {
                    count
                };
                let fmt = match lv {
                    Some(l)
                        if !legal
                            || matches!(
                                l.format,
                                bo_model::NumFormat::Bullet | bo_model::NumFormat::None
                            ) =>
                    {
                        l.format.clone()
                    }
                    _ => bo_model::NumFormat::Decimal,
                };
                label = label.replace(&pat, &bo_model::format_number(value, &fmt));
            }
        }
        Some((label, level.rpr.clone(), level.suffix.clone()))
    }
}

#[cfg(test)]
mod list_counter_tests {
    use super::ListCounters;
    use bo_model::{Document, Num, NumFormat, NumRef};

    /// A decimal list with level texts `%1.`, `%1.%2.`, `%1.%2.%3.` and letter / roman
    /// formats on levels 2 and 3.
    fn doc() -> (Document, i32) {
        let mut d = Document::default();
        let id = d.numbering.add_simple_list(false);
        let a = &mut d.numbering.abstracts[0];
        a.levels[1].text = "%1.%2.".into();
        a.levels[2].text = "%1.%2.%3.".into();
        (d, id)
    }

    fn labels(d: &Document, seq: &[(i32, u8)]) -> Vec<String> {
        let mut c = ListCounters::default();
        seq.iter()
            .map(|&(num_id, ilvl)| {
                c.next(d, NumRef { num_id, ilvl })
                    .map(|l| l.0)
                    .unwrap_or_default()
            })
            .collect()
    }

    /// §17.9.26 startOverride at level 2: the new list instance restarts level 2 only,
    /// level 1 goes on (3. → 3.e. not 1.e.).
    #[test]
    fn restart_at_level_two_keeps_upper_counters() {
        let (mut d, id) = doc();
        d.numbering.nums.push(Num {
            id: id + 1,
            abstract_id: 0,
            start_overrides: vec![(1, 5)],
        });
        let got = labels(
            &d,
            &[
                (id, 0),
                (id, 0),
                (id, 0),
                (id, 1),
                (id + 1, 1),
                (id + 1, 1),
                (id + 1, 0),
            ],
        );
        assert_eq!(got, ["1.", "2.", "3.", "3.a.", "3.e.", "3.f.", "4."]);
    }

    /// An override of level 1 still restarts the whole list.
    #[test]
    fn restart_at_level_one_restarts_everything() {
        let (mut d, id) = doc();
        d.numbering.nums.push(Num {
            id: id + 1,
            abstract_id: 0,
            start_overrides: vec![(0, 1)],
        });
        let got = labels(&d, &[(id, 0), (id, 1), (id + 1, 0), (id + 1, 1)]);
        assert_eq!(got, ["1.", "1.a.", "1.", "1.a."]);
    }

    /// §17.9.10 lvlRestart: level 3 restarting only after level 1 keeps counting across
    /// level-2 items; `0` never restarts; the default restarts under each upper item.
    #[test]
    fn restart_after_level() {
        let (mut d, id) = doc();
        let seq = [
            (id, 0),
            (id, 1),
            (id, 2),
            (id, 2),
            (id, 1),
            (id, 2),
            (id, 0),
            (id, 1),
            (id, 2),
        ];
        assert_eq!(
            labels(&d, &seq),
            ["1.", "1.a.", "1.a.i.", "1.a.ii.", "1.b.", "1.b.i.", "2.", "2.a.", "2.a.i."]
        );
        d.numbering.abstracts[0].levels[2].restart = Some(1);
        assert_eq!(
            labels(&d, &seq),
            ["1.", "1.a.", "1.a.i.", "1.a.ii.", "1.b.", "1.b.iii.", "2.", "2.a.", "2.a.i."]
        );
        d.numbering.abstracts[0].levels[2].restart = Some(0);
        assert_eq!(
            labels(&d, &seq),
            ["1.", "1.a.", "1.a.i.", "1.a.ii.", "1.b.", "1.b.iii.", "2.", "2.a.", "2.a.iv."]
        );
    }

    /// §17.9.4 isLgl: the level's text shows every number as a decimal; other levels keep
    /// their own formats.
    #[test]
    fn legal_style_numbers_are_decimal() {
        let (mut d, id) = doc();
        d.numbering.abstracts[0].levels[0].format = NumFormat::UpperRoman;
        d.numbering.abstracts[0].levels[2].is_lgl = true;
        let got = labels(&d, &[(id, 0), (id, 0), (id, 1), (id, 2), (id, 2)]);
        assert_eq!(got, ["I.", "II.", "II.a.", "2.1.1.", "2.1.2."]);
    }
}
