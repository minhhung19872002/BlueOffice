//! BlueEditor round 9: Organize › Crop Pages opens "Set Page Dimensions" laid out as the
//! reference editor's (black-box at 125 %: 843 × 673 px): Margin Controls (box, Constrain
//! Proportions, Top / Bottom / Left / Right, Remove White Margins, Set To Zero, Reset), Change
//! Page Size (shown, not offered: dimmed), Page Range (All Pages / Page: … of N, Crop: all,
//! even or odd pages of the range), Show All Boxes, Units, a preview of the page with the
//! crop box, the preview page "1 of N pages" and the cropped page size; OK writes `/CropBox`
//! (ISO 32000-1 §14.11.2) through `PdfDocument::crop_pages`.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{
    button, check, combo, group, label, radio, rlabel, spin, Ctl, Form, On, Val,
};
use crate::editor_window::EditorWindow;
use bo_render::Rgba;

/// Points per unit of the Units list (inch, centimetre, millimetre, point).
const UNITS: [(&str, f32); 4] = [
    ("inch", 72.0),
    ("cm", 28.346_457),
    ("mm", 2.834_645_7),
    ("pt", 1.0),
];

const SIDES: [&str; 4] = ["left", "top", "right", "bottom"];

fn dim_radio(id: &'static str, k: usize, x: f32, y: f32, label: String) -> Ctl {
    Ctl::Radio {
        id,
        k,
        x,
        y,
        label,
        on: On::Never,
    }
}

impl EditorWindow {
    /// Open Set Page Dimensions for the current page (or the pages chosen in the grid).
    pub(crate) fn open_crop9(&mut self) {
        let t = |k: &str| self.t(k);
        let n = self
            .r
            .view()
            .map_or(0, crate::ReaderView::page_count)
            .max(1);
        let pages = self.target_pages();
        let cur = pages.first().copied().unwrap_or(0) + 1;
        let sizes: Vec<String> = self.r.view().map_or_else(Vec::new, |v| {
            (0..v.page_count())
                .map(|i| {
                    let (w, h) = v.page_size(i);
                    format!("{w}x{h}")
                })
                .collect()
        });
        let unit_names: Vec<String> = UNITS.iter().map(|u| u.0.to_owned()).collect();
        let s4 = |id| spin(id, [0.0; 4], 0.01, (0.0, 1000.0), On::Always);
        let at = |c: Ctl, r: [f32; 4]| match c {
            Ctl::Spin {
                id,
                step,
                min,
                max,
                on,
                ..
            } => Ctl::Spin {
                id,
                r,
                step,
                min,
                max,
                on,
            },
            other => other,
        };
        let c = vec![
            group(t("pdfEditor9.marginControls"), [17.0, 56.0, 472.0, 232.0]),
            combo(
                "box",
                [27.0, 79.0, 170.0, 26.0],
                vec!["CropBox".into()],
                On::Always,
            ),
            check("constrain", 29.0, 126.0, t("pdfEditor9.constrain")),
            rlabel(91.0, 159.0, t("pdfEditor9.top"), On::Always),
            at(s4("top"), [96.0, 146.0, 104.0, 26.0]),
            rlabel(267.0, 159.0, t("pdfEditor9.bottom"), On::Always),
            at(s4("bottom"), [273.0, 146.0, 100.0, 26.0]),
            rlabel(91.0, 194.0, t("pdfEditor9.left"), On::Always),
            at(s4("left"), [96.0, 181.0, 104.0, 26.0]),
            rlabel(267.0, 194.0, t("pdfEditor9.right"), On::Always),
            at(s4("right"), [273.0, 181.0, 100.0, 26.0]),
            check("white", 29.0, 228.0, t("pdfEditor9.removeWhite")),
            button(
                "c9zero",
                [85.0, 249.0, 148.0, 27.0],
                t("pdfEditor9.setToZero"),
                On::Always,
            ),
            button(
                "c9reset",
                [297.0, 249.0, 123.0, 27.0],
                t("pdfEditor9.reset"),
                On::Always,
            ),
            group(t("pdfEditor9.changePageSize"), [17.0, 309.0, 472.0, 130.0]),
            dim_radio("size", 0, 29.0, 344.0, t("pdfEditor9.fixedSize")),
            combo(
                "fixed",
                [197.0, 331.0, 173.0, 26.0],
                vec![t("pdfEditor9.none")],
                On::Never,
            ),
            dim_radio("size", 1, 29.0, 380.0, t("pdfEditor9.custom")),
            rlabel(192.0, 380.0, t("pdfEditor9.width"), On::Never),
            spin(
                "cw",
                [197.0, 367.0, 98.0, 26.0],
                0.01,
                (0.0, 200.0),
                On::Never,
            ),
            rlabel(373.0, 380.0, t("pdfEditor9.height"), On::Never),
            spin(
                "ch",
                [378.0, 367.0, 98.0, 26.0],
                0.01,
                (0.0, 200.0),
                On::Never,
            ),
            Ctl::Check {
                id: "center",
                x: 29.0,
                y: 414.0,
                label: t("pdfEditor9.center"),
                on: On::Never,
            },
            rlabel(192.0, 414.0, t("pdfEditor9.xOffset"), On::Never),
            spin(
                "cx",
                [197.0, 401.0, 98.0, 26.0],
                0.01,
                (0.0, 200.0),
                On::Never,
            ),
            rlabel(373.0, 414.0, t("pdfEditor9.yOffset"), On::Never),
            spin(
                "cy",
                [378.0, 401.0, 98.0, 26.0],
                0.01,
                (0.0, 200.0),
                On::Never,
            ),
            group(t("pdfEditor9.pageRange2"), [17.0, 459.0, 472.0, 159.0]),
            radio("range", 0, 29.0, 494.0, t("pdfEditor9.allPagesR")),
            radio("range", 1, 29.0, 529.0, t("pdfEditor9.pageR")),
            crate::editor_form8::edit("pages", [109.0, 515.0, 205.0, 28.0], On::If("range", 1)),
            label(
                332.0,
                529.0,
                t("pdfEditor9.of").replace("{{n}}", &n.to_string()),
            ),
            label(110.0, 563.0, t("pdfEditor9.sample")),
            Ctl::Info { x: 253.0, y: 563.0 },
            rlabel(99.0, 597.0, t("pdfEditor9.cropWhich"), On::Always),
            combo(
                "subset",
                [109.0, 584.0, 205.0, 26.0],
                vec![
                    t("pdfEditor9.allInRange"),
                    t("pdfEditor9.evenOnly"),
                    t("pdfEditor9.oddOnly"),
                ],
                On::Always,
            ),
            check("showAll", 497.0, 59.0, t("pdfEditor9.showAllBoxes")),
            rlabel(740.0, 59.0, t("pdfEditor9.units"), On::Always),
            combo("units", [746.0, 45.0, 81.0, 26.0], unit_names, On::Always),
            Ctl::Frame {
                r: [497.0, 80.0, 330.0, 440.0],
            },
            spin(
                "ppage",
                [530.0, 546.0, 75.0, 26.0],
                1.0,
                (1.0, n as f32),
                On::Always,
            ),
            label(626.0, 560.0, t("pdfEditor9.ofShort")),
            label(688.0, 560.0, n.to_string()),
            label(735.0, 560.0, t("pdfEditor9.pagesWord")),
            button(
                "ok",
                [635.0, 629.0, 91.0, 28.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [735.0, 629.0, 91.0, 28.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let mut f = Form::new(t("pdfEditor9.setPageDimensions"), (843.0, 673.0), c)
            .with("range", Val::Pick(1))
            .with("pages", Val::Text(format!("{cur},")))
            .with("showAll", Val::Pick(1))
            .with("preview", Val::Pick(1))
            .with("ppage", Val::Text(cur.to_string()))
            .with("sizes", Val::Text(sizes.join(";")))
            .with("cropLabel", Val::Text(t("pdfEditor9.croppedSize")));
        for s in SIDES {
            f.set(s, Val::Text("0.00".into()));
        }
        self.form8.push((f, FormWhat::Crop9));
    }

    /// Set To Zero / Reset of Set Page Dimensions; true when it was one.
    pub(crate) fn crop9_button(&mut self, id: &str) -> bool {
        if !matches!(self.form8.last(), Some((_, FormWhat::Crop9)))
            || !matches!(id, "c9zero" | "c9reset")
        {
            return false;
        }
        if let Some((f, _)) = self.form8.last_mut() {
            for s in SIDES {
                f.set(s, Val::Text("0.00".into()));
            }
            f.set("white", Val::Pick(0));
        }
        true
    }

    /// OK: crop the pages of the range by the margins (or each page to its content when
    /// Remove White Margins is ticked).
    pub(crate) fn crop9_ok(&mut self, f: &Form) {
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        let mut pages: Vec<usize> = if f.pick("range") == 0 {
            (0..n).collect()
        } else {
            crate::editor_more::parse_pages(f.text("pages").trim_end_matches(','), "all", n)
        };
        match f.pick("subset") {
            1 => pages.retain(|p| (p + 1) % 2 == 0),
            2 => pages.retain(|p| (p + 1) % 2 == 1),
            _ => {}
        }
        if pages.is_empty() {
            return;
        }
        let r = if f.pick("white") == 1 {
            self.doc_edit(|d| {
                for &i in &pages {
                    let m = white_margins(d, i)?;
                    d.crop_pages(&[i], m)?;
                }
                Ok(())
            })
        } else {
            let m = margins_pt(f);
            if m.iter().all(|v| *v <= 0.0) {
                return;
            }
            self.doc_edit(|d| d.crop_pages(&pages, m))
        };
        match r {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }
}

/// Margins of the form in points: left, top, right, bottom.
pub(crate) fn margins_pt(f: &Form) -> [f32; 4] {
    let per = UNITS.get(f.pick("units")).map_or(72.0, |u| u.1);
    SIDES.map(|s| f.num(s, 0.0).max(0.0) * per)
}

/// Margins (left, top, right, bottom, points) that leave only the content of page `i`.
fn white_margins(d: &mut bo_pdfread::PdfDocument, i: usize) -> Result<[f32; 4], String> {
    let (w, h) = d
        .page_info(i)
        .map_or((612.0, 792.0), |p| (p.width, p.height));
    let objs = d.page_objects(i)?;
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for o in &objs {
        b = [
            b[0].min(o.bbox[0]),
            b[1].min(o.bbox[1]),
            b[2].max(o.bbox[2]),
            b[3].max(o.bbox[3]),
        ];
    }
    if b[0] > b[2] {
        return Ok([0.0; 4]);
    }
    Ok([
        b[0].max(0.0),
        b[1].max(0.0),
        (w - b[2]).max(0.0),
        (h - b[3]).max(0.0),
    ])
}

/// The preview of Set Page Dimensions: the page in the box, the parts cut away tinted, the
/// crop box framed, and the cropped page size under it.
pub(crate) fn paint_crop_preview(
    p: &mut dyn crate::Painter,
    f: &Form,
    b: [f32; 4],
    img: Option<(usize, f32, f32)>,
    s: f32,
) {
    let q = |v: f32| (v * s).round();
    let n: usize = f.num("ppage", 1.0).max(1.0) as usize;
    let sizes = f.text("sizes");
    let (pw, ph) = sizes
        .split(';')
        .nth(n - 1)
        .and_then(|t| t.split_once('x'))
        .and_then(|(a, c)| Some((a.parse::<f32>().ok()?, c.parse::<f32>().ok()?)))
        .unwrap_or((612.0, 792.0));
    let (iw, ih) = img.map_or_else(
        || {
            let k = ((b[2] - q(40.0)) / pw).min((b[3] - q(40.0)) / ph);
            (pw * k, ph * k)
        },
        |(_, w, h)| (w, h),
    );
    let k = iw / pw.max(1.0);
    let (px, py) = (
        (b[0] + (b[2] - iw) / 2.0).round(),
        (b[1] + (b[3] - ih) / 2.0).round(),
    );
    p.fill_rect(
        px - 1.0,
        py - 1.0,
        iw + 2.0,
        ih + 2.0,
        Rgba(0x40, 0x40, 0x40, 255),
    );
    if let Some((key, _, _)) = img {
        p.page(key, px as i32, py as i32);
    } else {
        p.fill_rect(px, py, iw, ih, Rgba(255, 255, 255, 255));
    }
    let m = margins_pt(f);
    let c = [
        px + m[0] * k,
        py + m[1] * k,
        (iw - (m[0] + m[2]) * k).max(1.0),
        (ih - (m[1] + m[3]) * k).max(1.0),
    ];
    let shade = Rgba(0, 0, 0, 70);
    p.fill_rect(px, py, iw, c[1] - py, shade);
    p.fill_rect(px, c[1] + c[3], iw, py + ih - c[1] - c[3], shade);
    p.fill_rect(px, c[1], c[0] - px, c[3], shade);
    p.fill_rect(c[0] + c[2], c[1], px + iw - c[0] - c[2], c[3], shade);
    if f.pick("showAll") == 1 {
        let blue = Rgba(0x00, 0x78, 0xD4, 255);
        p.fill_rect(c[0].round(), c[1].round(), c[2].round(), 1.0, blue);
        p.fill_rect(
            c[0].round(),
            (c[1] + c[3]).round() - 1.0,
            c[2].round(),
            1.0,
            blue,
        );
        p.fill_rect(c[0].round(), c[1].round(), 1.0, c[3].round(), blue);
        p.fill_rect(
            (c[0] + c[2]).round() - 1.0,
            c[1].round(),
            1.0,
            c[3].round(),
            blue,
        );
    }
    let (unit, per) = UNITS.get(f.pick("units")).copied().unwrap_or(UNITS[0]);
    let text = format!(
        "{:.2} × {:.2}  {unit}",
        (pw - m[0] - m[2]).max(0.0) / per,
        (ph - m[1] - m[3]).max(0.0) / per
    );
    let style = crate::TextStyle {
        size: q(12.0),
        bold: false,
    };
    let label = f.text("cropLabel");
    p.text(
        &format!("{label} {text}"),
        b[0] + q(43.0),
        b[1] + b[3] + q(70.0),
        style,
        Rgba(0, 0, 0, 255),
    );
}
