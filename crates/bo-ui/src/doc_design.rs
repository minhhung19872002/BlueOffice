//! Design › Document Formatting of BlueDoc, laid out like a desktop word processor's: an
//! inline gallery of document style sets between Themes and Colors / Fonts (tiles of
//! 100 × 88 px at 125 % showing Title, Heading 1 and body text in the theme's faces and
//! colours), then Paragraph Spacing / Effects / Set as Default stacked. The style sets are
//! BlueOffice's own (`bo_edit::design::DOC_STYLE_SETS`, engine `applyDocumentStyleSet`).
use crate::ribbon::{Ribbon, RibbonHit};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Font, Painter};
use bo_edit::design::{DocStyleSetDef, DOC_STYLE_SETS, THEMES};
use bo_render::{PathSeg, Rgba};

/// Gallery tiles: command id and style set id, in gallery order.
pub(crate) const STYLE_TILES: [(&str, &str); 7] = [
    ("design.styleSet.standard", "standard"),
    ("design.styleSet.capital", "capital"),
    ("design.styleSet.lotus", "lotus"),
    ("design.styleSet.river", "river"),
    ("design.styleSet.harbor", "harbor"),
    ("design.styleSet.quiet", "quiet"),
    ("design.styleSet.bold", "bold"),
];
/// The gallery's ▾ column (opens the sets as a list): the right 18 px of the last tile.
const MORE_W: f32 = 18.0;

/// Commands shown greyed: not carried out by BlueDoc yet (theme effects, saving the
/// formatting as the default for new documents).
pub(crate) const UNAVAILABLE: [&str; 2] = ["design.effects", "design.setDefault"];

/// Fixed widths (logical px) of the gallery placeholders: tiles 80 (100 px at 125 %), the
/// ▾ column 18.
pub(crate) fn large_w(id: &str) -> Option<f32> {
    if id == STYLE_TILES[STYLE_TILES.len() - 1].0 {
        Some(80.0 + MORE_W)
    } else if STYLE_TILES.iter().any(|(t, _)| *t == id) {
        Some(80.0)
    } else {
        None
    }
}

/// The ▾ column of the gallery (device px), when the Design tab shows it.
pub(crate) fn more_rect(ribbon: &Ribbon, scale: f32) -> Option<[f32; 4]> {
    let last = ribbon.rect_of(RibbonHit::Command(STYLE_TILES[STYLE_TILES.len() - 1].0))?;
    let w = (MORE_W * scale).round();
    Some([last[0] + last[2] - w, last[1], w, last[3]])
}

/// The style set a tile command applies.
pub(crate) fn style_set_of(id: &str) -> Option<&'static str> {
    STYLE_TILES.iter().find(|(t, _)| *t == id).map(|(_, s)| *s)
}

fn hex(s: &str) -> Rgba {
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

/// Faces and colours the previews use: heading and body faces, title (dk2) and heading
/// (accent 1) colours.
#[derive(Debug, Clone)]
pub(crate) struct PreviewLook {
    pub(crate) major: String,
    pub(crate) minor: String,
    pub(crate) title: Rgba,
    pub(crate) heading: Rgba,
}

impl PreviewLook {
    /// From the BlueOffice theme `theme_id` when known, else the document's theme fonts and
    /// the default heading colours.
    pub(crate) fn new(theme_id: Option<&str>, major: &str, minor: &str) -> Self {
        if let Some(t) = theme_id.and_then(|id| THEMES.iter().find(|t| t.id == id)) {
            return Self {
                major: t.major.to_owned(),
                minor: t.minor.to_owned(),
                title: hex(t.colors[2]),
                heading: hex(t.colors[4]),
            };
        }
        let face = |f: &str, d: &str| {
            if f.is_empty() {
                d.to_owned()
            } else {
                f.to_owned()
            }
        };
        Self {
            major: face(major, "Calibri Light"),
            minor: face(minor, "Calibri"),
            title: Rgba(0x1f, 0x1f, 0x1f, 255),
            heading: Rgba(0x2f, 0x54, 0x96, 255),
        }
    }
}

fn caps(s: &str, on: bool) -> String {
    if on {
        s.to_uppercase()
    } else {
        s.to_owned()
    }
}

/// Draw the style set tiles and the ▾ column over their ribbon placeholders.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint(
    p: &mut dyn Painter,
    ribbon: &Ribbon,
    current: Option<&str>,
    look: &PreviewLook,
    t: &Theme,
    strings: &Strings,
    scale: f32,
) {
    let rect = |id: &'static str| ribbon.rect_of(RibbonHit::Command(id));
    let (Some(first), Some(more)) = (rect(STYLE_TILES[0].0), more_rect(ribbon, scale)) else {
        return;
    };
    let px = |v: f32| (v * scale).round();
    let grey = |v: u8| Rgba(v, v, v, 255);
    // Gallery box: frame 198, white inside, as the reference's in-ribbon gallery.
    let top = first[1] + px(2.0);
    let h = first[3] - px(4.0);
    let (left, right) = (first[0] - px(2.0), more[0] + more[2]);
    p.fill_rect(left - 1.0, first[1], right - left + 2.0, first[3], t.ribbon);
    p.fill_rect(left, top, right - left, h, grey(198));
    p.fill_rect(
        left + 1.0,
        top + 1.0,
        right - left - 2.0,
        h - 2.0,
        grey(255),
    );
    let hovered = ribbon.hovered();
    let sample = strings.get("docTable.styleSample");
    let sample = if sample == "docTable.styleSample" {
        "BlueDoc keeps the look of every heading and paragraph in its styles, so a whole document changes at once."
    } else {
        sample
    };
    let title_word = {
        let v = strings.get("docTable.styleTitle");
        if v == "docTable.styleTitle" {
            "Title"
        } else {
            v
        }
    };
    let heading_word = {
        let v = strings.get("docTable.styleHeading");
        if v == "docTable.styleHeading" {
            "Heading 1"
        } else {
            v
        }
    };
    for (id, set_id) in STYLE_TILES {
        let Some(r) = rect(id) else { continue };
        let Some(set) = DOC_STYLE_SETS.iter().find(|s| s.id == set_id) else {
            continue;
        };
        // The last tile leaves its right edge to the ▾ column.
        let rw = if id == STYLE_TILES[STYLE_TILES.len() - 1].0 {
            r[2] - more[2]
        } else {
            r[2]
        };
        let (tx, ty, tw, th) = (r[0] + px(2.0), top + px(3.0), rw - px(4.0), h - px(6.0));
        if current == Some(set_id) {
            p.fill_rect(tx, ty, tw, th, grey(136));
            p.fill_rect(tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0, grey(255));
        } else if hovered == Some(RibbonHit::Command(id)) {
            p.fill_rect(tx, ty, tw, th, grey(210));
            p.fill_rect(tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0, grey(255));
        }
        p.push_clip(tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0);
        paint_tile(
            p,
            set,
            look,
            [tx + px(4.0), ty + px(3.0), tw - px(8.0)],
            (title_word, heading_word, sample),
            scale,
        );
        p.pop_clip();
    }
    // ▾ column: up / down / more marks, as the reference's gallery scroller.
    let col = [more[0], top + 1.0, more[2] - 1.0, h - 2.0];
    p.fill_rect(col[0], col[1], 1.0, col[3], grey(225));
    let cx = col[0] + col[2] / 2.0;
    let k = px(3.5);
    let ink = grey(96);
    let my = col[1] + col[3] - px(12.0);
    p.fill_rect(cx - k, my - px(4.0), 2.0 * k, 1.0, ink);
    p.path(
        &[
            PathSeg::M(cx - k, my - k / 2.0 + px(1.0)),
            PathSeg::L(cx, my + k / 2.0 + px(1.0)),
            PathSeg::L(cx + k, my - k / 2.0 + px(1.0)),
        ],
        None,
        Some((ink, scale.max(1.0))),
    );
}

/// One tile: Title, Heading 1 and three lines of body text, sizes in proportion to the set.
fn paint_tile(
    p: &mut dyn Painter,
    set: &DocStyleSetDef,
    look: &PreviewLook,
    at: [f32; 3],
    words: (&str, &str, &str),
    scale: f32,
) {
    fn face(family: &str, size: f32, bold: bool) -> Font<'_> {
        Font {
            family,
            size,
            weight: if bold { 700 } else { 400 },
            italic: false,
            underline: false,
        }
    }
    let [x, mut y, w] = at;
    // Tiles show the sizes at about a quarter (Title 28 pt → 14 px at 100 %).
    let k = scale * 0.5;
    let ts = (set.title.size as f32 * k * 0.5).round();
    let hs = (set.heading1.size as f32 * k * 0.42).round();
    p.text_font(
        &caps(words.0, set.title.caps),
        x,
        y,
        &face(&look.major, ts, set.title.bold),
        look.title,
    );
    y += (ts * 1.25).round();
    p.text_font(
        &caps(words.1, set.heading1.caps),
        x,
        y,
        &face(&look.major, hs, set.heading1.bold),
        look.heading,
    );
    y += (hs * 1.35).round();
    // Body: three short lines of the sample text in the body face.
    let bs = (5.0 * scale).round().max(4.0);
    let chars_per_line = ((w / (bs * 0.5)).floor() as usize).max(8);
    let text: Vec<char> = words.2.chars().collect();
    for line in text.chunks(chars_per_line).take(3) {
        let s: String = line.iter().collect();
        p.text_font(
            s.trim(),
            x,
            y,
            &face(&look.minor, bs, false),
            Rgba(64, 64, 64, 255),
        );
        y += (bs * 1.3).round();
    }
}
