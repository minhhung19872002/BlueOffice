//! BlueEditor round 12: File › Properties as the reference editor shows it (black-box at 125 %):
//! a middle list — Description, Initial View, Security, Fonts, Advanced — and a page for each at
//! the right. Description lists the file's facts and its editable title / subject / author /
//! creator / producer / keywords (grey "add a …" when empty), Custom properties and dates;
//! Initial View sets the opening panel, layout, magnification, page and window / interface
//! options (written into the catalog, `bo_pdfread::InitialView`); Security shows the
//! protection and the restrictions summary; Fonts the pages' fonts as a tree; Advanced the
//! print dialog presets and reading options (`bo_pdfread::PrintPresets`). Measured: right page
//! from x 668 (Security 691), labels #6E6E6E 15 px, values from x 905, rows 35 px apart from
//! y 218, headings 19 px, the page title 23 px; drop-downs 248 × 26 px framed #999999 from x
//! 900, check boxes 17 px framed #626262 on #F3F3F3, links in the accent colour.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, edit, label, Form, On, Val};
use crate::editor_window::EditorWindow;
use crate::reader_icons::{self, Ink};
use crate::reader_window::rounded;
use crate::{Painter, TextStyle};
use bo_pdfread::{InitialView, PrintPresets, ZOOMS};
use bo_render::{PathSeg, Rgba};

const TEXT: Rgba = Rgba(0x6E, 0x6E, 0x6E, 255);
const DARK: Rgba = Rgba(0x22, 0x22, 0x22, 255);
const PLACEHOLDER: Rgba = Rgba(0x96, 0x96, 0x96, 255);
const CHOSEN: Rgba = Rgba(0xD9, 0xD9, 0xD9, 255);
const RULE: Rgba = Rgba(0xE0, 0xE0, 0xE0, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const COMBO_FRAME: Rgba = Rgba(0x99, 0x99, 0x99, 255);
const CHECK_FRAME: Rgba = Rgba(0x62, 0x62, 0x62, 255);
const CHECK_FILL: Rgba = Rgba(0xF3, 0xF3, 0xF3, 255);

/// Pages of the middle list: label key and icon.
const SUBS: [(&str, &str); 5] = [
    ("pdfEditor12.pDescription", "info"),
    ("pdfEditor12.pInitialView", "viewSetting"),
    ("pdfEditor12.pSecurity", "secure"),
    ("pdfEditor12.pFonts", "addText"),
    ("pdfEditor12.pAdvanced", "properties"),
];

/// Editable entries of the information dictionary: key, label key, placeholder key.
const INFO_ROWS: [(&str, &str, &str); 6] = [
    ("Title", "pdfEditor12.title", "pdfEditor12.addTitle"),
    ("Subject", "pdfEditor12.subject", "pdfEditor12.addSubject"),
    ("Creator", "pdfEditor12.creator", "pdfEditor12.addCreator"),
    ("Author", "pdfEditor12.author", "pdfEditor12.addAuthor"),
    (
        "Producer",
        "pdfEditor12.producer",
        "pdfEditor12.addProducer",
    ),
    (
        "Keywords",
        "pdfEditor12.keywords",
        "pdfEditor12.addKeywords",
    ),
];

/// Language tags of Advanced › Language (empty: not set).
const LANGS: [&str; 9] = [
    "", "en-US", "vi-VN", "fr-FR", "de-DE", "ja-JP", "zh-CN", "ko-KR", "es-ES",
];

/// State of the Properties page.
#[derive(Debug, Clone, Default)]
pub(crate) struct BsProps12 {
    /// Chosen page of the middle list.
    pub sub: usize,
    /// Description: "Show More Information" on.
    pub more: bool,
    /// The open drop-down: its id and box (window pixels).
    pub open: Option<(&'static str, [f32; 4])>,
    /// Fonts: collapsed font rows.
    pub collapsed: Vec<usize>,
}

/// One drop-down of the right page: id, label keys of its items, chosen index.
type Combo = (&'static str, Vec<String>, usize);

fn date_text(raw: &str) -> String {
    // D:YYYYMMDDHHmmSS (§7.9.4).
    let d = raw.trim_start_matches("D:");
    let digits: String = d.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() < 8 {
        return raw.to_owned();
    }
    let g = |a: usize, b: usize| digits.get(a..b).unwrap_or("00");
    format!(
        "{}-{}-{} {}:{}:{}",
        g(0, 4),
        g(4, 6),
        g(6, 8),
        g(8, 10),
        g(10, 12),
        g(12, 14)
    )
}

fn style(sc: f32, px125: f32) -> TextStyle {
    TextStyle {
        size: px125 / 1.375 * sc,
        bold: false,
    }
}

impl EditorWindow {
    /// Paint the Properties page: the middle list from `mx` and the chosen page at the right;
    /// `top` is the page's top edge (the tab row's bottom).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_props12(
        &mut self,
        p: &mut dyn Painter,
        mx: f32,
        top: f32,
        hits: &mut Vec<([f32; 4], String)>,
    ) {
        let sc = self.ui_scale();
        let px = move |v: f32| (v * sc).round();
        // Reference pixels (125 %, window y from the tab row's bottom at 80).
        let q = move |v125: f32| (v125 / 1.25 * sc).round();
        let qy = move |v125: f32| top + q(v125 - 80.0);
        let (w, h) = self.window_size();
        let accent = self.accent();
        let item = style(sc, 17.0);
        let title = style(sc, 26.0);
        let head = style(sc, 23.0);
        let section = style(sc, 19.0);
        let row = style(sc, 15.5);
        let small = style(sc, 14.0);
        let mid = |st: TextStyle, yc: f32| (qy(yc) - st.size * 1.33 / 2.0).round();
        // Middle list.
        let label = self.t("pdfEditor.bsProperties");
        p.text(
            label.trim_end_matches(['.', '…']),
            mx - px(0.8),
            top + px(14.8),
            title,
            Rgba(0x66, 0x66, 0x66, 255),
        );
        p.fill_rect(px(517.6), top, 1.0, h - top, RULE);
        let ink = Ink {
            ink: Rgba(0x33, 0x33, 0x33, 255),
            accent,
        };
        let mut ry = top + px(85.6);
        for (k, (key, icon)) in SUBS.iter().enumerate() {
            let r = [mx - px(9.6), ry, px(300.0) - px(30.4), px(34.4)];
            let chosen = k == self.bsp12.sub;
            if chosen {
                p.path(
                    &rounded(r[0], r[1], r[2], r[3], r[3] / 2.0),
                    Some(CHOSEN),
                    None,
                );
            }
            let is = px(20.8);
            reader_icons::draw(p, icon, r[0] + px(16.0), r[1] + (r[3] - is) / 2.0, is, ink);
            let lt = r[1] + ((r[3] - item.size * 1.33) / 2.0).round();
            p.text(
                &self.t(key),
                r[0] + px(43.2),
                lt,
                item,
                if chosen { accent } else { DARK },
            );
            hits.push((r, format!("bsp12.sub{k}")));
            ry += px(45.6);
        }
        // The page at the right.
        let x0 = q(if self.bsp12.sub == 2 { 691.0 } else { 668.0 });
        let xv = q(905.0);
        p.push_clip(px(518.0), top, w - px(518.0), h - top);
        let sub_title = self.t(SUBS[self.bsp12.sub.min(4)].0);
        p.text(&sub_title, x0, mid(head, 119.0), head, TEXT);
        let mut combos: Vec<(Combo, [f32; 4])> = Vec::new();
        match self.bsp12.sub {
            1 => {
                let v = self
                    .view()
                    .map(|v| v.doc().initial_view())
                    .unwrap_or_default();
                let n = self.view().map_or(1, crate::ReaderView::page_count);
                p.text(
                    &self.t("pdfEditor12.layoutDest"),
                    x0,
                    mid(section, 165.0),
                    section,
                    TEXT,
                );
                let navs: Vec<String> = (0..5)
                    .map(|i| self.t(&format!("pdfEditor12.nav{i}")))
                    .collect();
                let lays: Vec<String> = (0..7)
                    .map(|i| self.t(&format!("pdfEditor12.lay{i}")))
                    .collect();
                let mut mags: Vec<String> = (0..6)
                    .map(|i| self.t(&format!("pdfEditor12.mag{i}")))
                    .collect();
                mags.extend(ZOOMS.iter().map(|z| format!("{z}%")));
                for (k, (lk, combo)) in [
                    ("pdfEditor12.navigation", ("iv.nav", navs, v.navigation)),
                    ("pdfEditor12.pageLayout", ("iv.layout", lays, v.layout)),
                    (
                        "pdfEditor12.magnification",
                        ("iv.mag", mags, v.magnification),
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let yc = 217.0 + 36.0 * k as f32;
                    p.text(&self.t(lk), x0, mid(row, yc), row, TEXT);
                    let r = [q(900.0), qy(yc - 13.0), q(248.0), q(26.0)];
                    combos.push((combo, r));
                }
                p.text(
                    &self.t("pdfEditor12.openToPage"),
                    x0,
                    mid(row, 326.0),
                    row,
                    TEXT,
                );
                let r = [q(900.0), qy(312.0), q(132.0), q(27.0)];
                self.edit_box12(p, r, &(v.page + 1).to_string(), row);
                hits.push((r, "bsp12.openPage".into()));
                p.text(&format!("/  {n}"), q(1041.0), mid(row, 326.0), row, TEXT);
                p.text(
                    &self.t("pdfEditor12.windowOptions"),
                    x0,
                    mid(section, 408.0),
                    section,
                    TEXT,
                );
                let checks = [
                    (
                        "iv.title",
                        "pdfEditor12.displayTitle",
                        v.display_title,
                        449.0,
                    ),
                    ("iv.full", "pdfEditor12.fullScreen", v.full_screen, 486.0),
                    (
                        "iv.center",
                        "pdfEditor12.centerWindow",
                        v.center_window,
                        523.0,
                    ),
                    (
                        "iv.resize",
                        "pdfEditor12.resizeWindow",
                        v.resize_window,
                        560.0,
                    ),
                    (
                        "iv.menubar",
                        "pdfEditor12.hideMenubar",
                        v.hide_menubar,
                        695.0,
                    ),
                    (
                        "iv.toolbars",
                        "pdfEditor12.hideToolbars",
                        v.hide_toolbars,
                        732.0,
                    ),
                    (
                        "iv.controls",
                        "pdfEditor12.hideControls",
                        v.hide_window_controls,
                        769.0,
                    ),
                ];
                p.text(
                    &self.t("pdfEditor12.uiOptions"),
                    x0,
                    mid(section, 654.0),
                    section,
                    TEXT,
                );
                for (id, key, on, yc) in checks {
                    let r = self.check12(p, x0, qy(yc), on, &self.t(key), row, accent);
                    hits.push((r, format!("bsp12.check.{id}")));
                }
            }
            2 => {
                let info = self.view().map(|v| v.doc().doc_info()).unwrap_or_default();
                let enc = !info.security.is_empty();
                let r = [q(691.0), qy(165.0), q(408.0), q(59.0)];
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xDC, 0xDC, 0xDC, 255));
                p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                // The lock glyph 32 px tall as the reference's (its shape spans 18 of 24 units).
                let is = q(42.0);
                let ink2 = Ink {
                    ink: Rgba(0x33, 0x33, 0x33, 255),
                    accent,
                };
                reader_icons::draw(
                    p,
                    if enc { "secure" } else { "unsecure" },
                    q(687.0),
                    r[1] + (r[3] - is) / 2.0,
                    is,
                    ink2,
                );
                let what = if enc {
                    self.t("pdfEditor12.passwordProtection")
                } else {
                    self.t("pdfEditor12.noProtection")
                };
                p.text(&what, q(739.0), mid(row, 195.0), row, DARK);
                arrow_down(p, r[0] + r[2] - q(6.0), qy(195.0), sc, DARK);
                hits.push((r, "bsp12.secMenu".into()));
                // Change Settings: available when the document has a password.
                let b = [q(691.0), qy(242.0), q(141.0), q(32.0)];
                p.fill_rect(
                    b[0],
                    b[1],
                    b[2],
                    b[3],
                    if enc {
                        Rgba(0xAD, 0xAD, 0xAD, 255)
                    } else {
                        Rgba(0xE9, 0xE9, 0xE9, 255)
                    },
                );
                p.fill_rect(
                    b[0] + 1.0,
                    b[1] + 1.0,
                    b[2] - 2.0,
                    b[3] - 2.0,
                    Rgba(0xFB, 0xFB, 0xFB, 255),
                );
                let bl = self.t("pdfEditor12.changeSettings");
                let bw = crate::editor_form8::text_w(&bl, small.size * 1.1);
                p.text(
                    &bl,
                    (b[0] + (b[2] - bw) / 2.0).round(),
                    (b[1] + (b[3] - small.size * 1.33) / 2.0).round(),
                    small,
                    if enc {
                        DARK
                    } else {
                        Rgba(0xC1, 0xC1, 0xC1, 255)
                    },
                );
                if enc {
                    hits.push((b, "bsp12.changeSecurity".into()));
                }
                p.text(
                    &self.t("pdfEditor12.restrictions"),
                    x0,
                    mid(section, 302.0),
                    section,
                    TEXT,
                );
                let bits = info.permissions.unwrap_or(-1);
                let allowed = |bit: u32| !enc || bits & (1 << (bit - 1)) != 0;
                for (k, (key, bit)) in [
                    ("pdfEditor12.rPrinting", 3),
                    ("pdfEditor12.rChanging", 4),
                    ("pdfEditor12.rAssembly", 11),
                    ("pdfEditor12.rCopying", 5),
                    ("pdfEditor12.rAccessibility", 10),
                    ("pdfEditor12.rExtraction", 5),
                    ("pdfEditor12.rCommenting", 6),
                    ("pdfEditor12.rFilling", 9),
                    ("pdfEditor12.rSigning", 9),
                ]
                .into_iter()
                .enumerate()
                {
                    let yc = 337.0 + 45.0 * k as f32;
                    p.text(&self.t(key), x0, mid(row, yc), row, TEXT);
                    let v = if allowed(bit) {
                        self.t("pdfEditor12.allowed")
                    } else {
                        self.t("pdfEditor12.notAllowed")
                    };
                    p.text(&v, q(1061.0), mid(row, yc), row, TEXT);
                }
            }
            3 => {
                let mut fonts = self.view().map(|v| v.doc().font_list()).unwrap_or_default();
                // The reference lists the fonts by name (subset tag ignored).
                fonts.sort_by(|a, b| base_font(&a.name).cmp(base_font(&b.name)));
                let mut yc = 155.0;
                // A classic tree as the reference's (measured at 125 %): 11 px expander box with
                // a #919191 frame and a light gradient, navy 7 px minus, black dotted lines.
                let frame = Rgba(0x91, 0x91, 0x91, 255);
                let navy = Rgba(75, 99, 167, 255);
                let dot = Rgba(0, 0, 0, 255);
                let vdots = |p: &mut dyn Painter, x: f32, y0: f32, y1: f32| {
                    let mut y = y0;
                    while y < y1 {
                        p.fill_rect(x, y, 1.0, 1.0, dot);
                        y += 2.0;
                    }
                };
                let hdots = |p: &mut dyn Painter, x0: f32, x1: f32, y: f32| {
                    let mut x = x0;
                    while x <= x1 {
                        p.fill_rect(x, y, 1.0, 1.0, dot);
                        x += 2.0;
                    }
                };
                let n_fonts = fonts.len();
                for (k, f) in fonts.iter().enumerate() {
                    let open = !self.bsp12.collapsed.contains(&k);
                    let bx = [q(674.0), qy(yc - 4.0), q(11.0), q(11.0)];
                    let box_y = bx[1];
                    p.fill_rect(bx[0] + 1.0, bx[1], bx[2] - 2.0, bx[3], frame);
                    p.fill_rect(bx[0], bx[1] + 1.0, bx[2], bx[3] - 2.0, frame);
                    p.fill_rect(bx[0] + 1.0, bx[1] + 1.0, bx[2] - 2.0, bx[3] - 2.0, WHITE);
                    let half = (bx[3] / 2.0).floor();
                    p.fill_rect(
                        bx[0] + 1.0,
                        bx[1] + half + 1.0,
                        bx[2] - 2.0,
                        bx[3] - half - 2.0,
                        Rgba(0xE3, 0xE3, 0xE3, 255),
                    );
                    let cy = bx[1] + half;
                    p.fill_rect(bx[0] + 2.0, cy, bx[2] - 4.0, 1.0, navy);
                    let cx = bx[0] + half;
                    if !open {
                        p.fill_rect(cx, bx[1] + 2.0, 1.0, bx[3] - 4.0, navy);
                    }
                    hdots(p, bx[0] + bx[2] + 1.0, q(688.0), cy);
                    hits.push((
                        [bx[0] - 2.0, bx[1] - 2.0, q(260.0), bx[3] + 4.0],
                        format!("bsp12.font{k}"),
                    ));
                    p.text(base_font(&f.name), q(691.0), mid(small, yc), small, TEXT);
                    yc += 22.0;
                    if open {
                        let mut kids = vec![
                            format!("{}{}", self.t("pdfEditor12.fType"), f.kind),
                            format!(
                                "{}{}",
                                self.t("pdfEditor12.fEncoding"),
                                encoding_name(&f.encoding)
                            ),
                        ];
                        // A font the file does not embed: the face we draw it with, as the
                        // reference names its substitute ("Actual Font:Arial").
                        let actual = (!f.embedded)
                            .then(|| self.view().and_then(|v| v.doc().substitute_font(&f.name)))
                            .flatten();
                        if let Some(actual) = actual {
                            kids.push(format!("{}{actual}", self.t("pdfEditor12.fActual")));
                            kids.push(format!("{}TrueType", self.t("pdfEditor12.fActualType")));
                        } else {
                            kids.push(format!(
                                "{}{}",
                                self.t("pdfEditor12.fEmbedded"),
                                if f.embedded && f.name.contains('+') {
                                    self.t("pdfEditor12.embeddedSubset")
                                } else if f.embedded {
                                    self.t("pdfEditor12.yes")
                                } else {
                                    self.t("pdfEditor12.no")
                                }
                            ));
                        }
                        let lx = q(698.0);
                        let top = qy(yc - 22.0 + 8.0);
                        for kid in &kids {
                            let ky = qy(yc);
                            vdots(p, lx, top, ky);
                            hdots(p, lx, q(706.0), ky);
                            p.text(kid, q(710.0), mid(small, yc), small, TEXT);
                            yc += 22.0;
                        }
                    }
                    // The root line runs down to the next font's box.
                    if k + 1 < n_fonts {
                        vdots(p, cx, box_y + bx[3] + 1.0, qy(yc - 4.0));
                    }
                }
                if fonts.is_empty() {
                    p.text(
                        &self.t("pdfEditor12.noFonts"),
                        x0,
                        mid(row, 160.0),
                        row,
                        PLACEHOLDER,
                    );
                }
            }
            4 => {
                let pp = self
                    .view()
                    .map(|v| v.doc().print_presets())
                    .unwrap_or_default();
                let n = self.view().map_or(1, crate::ReaderView::page_count);
                p.text(
                    &self.t("pdfEditor12.printPresets"),
                    x0,
                    mid(section, 165.0),
                    section,
                    TEXT,
                );
                let scal = vec![self.t("pdfEditor12.default"), self.t("pdfEditor12.none")];
                let dup = vec![
                    self.t("pdfEditor12.simplex"),
                    self.t("pdfEditor12.duplexShort"),
                    self.t("pdfEditor12.duplexLong"),
                ];
                let mut copies = vec![self.t("pdfEditor12.default")];
                copies.extend((1..=5).map(|k| k.to_string()));
                let bind = vec![
                    self.t("pdfEditor12.leftEdge"),
                    self.t("pdfEditor12.rightEdge"),
                ];
                let langs: Vec<String> = LANGS
                    .iter()
                    .enumerate()
                    .map(|(k, _)| self.t(&format!("pdfEditor12.lang{k}")))
                    .collect();
                let lang_k = LANGS.iter().position(|l| *l == pp.language).unwrap_or(0);
                let rows = [
                    (
                        "pdfEditor12.pageScaling",
                        ("adv.scaling", scal, pp.scaling),
                        217.0,
                    ),
                    (
                        "pdfEditor12.duplexMode",
                        ("adv.duplex", dup, pp.duplex),
                        253.0,
                    ),
                    (
                        "pdfEditor12.numCopies",
                        ("adv.copies", copies, pp.copies.min(5) as usize),
                        289.0,
                    ),
                    (
                        "pdfEditor12.binding",
                        ("adv.binding", bind, pp.binding),
                        528.0,
                    ),
                    ("pdfEditor12.language", ("adv.lang", langs, lang_k), 564.0),
                ];
                for (lk, combo, yc) in rows {
                    p.text(&self.t(lk), x0, mid(row, yc), row, TEXT);
                    let r = [q(904.0), qy(yc - 13.0), q(249.0), q(26.0)];
                    combos.push((combo, r));
                }
                p.text(&self.t("pdfEditor12.pages"), x0, mid(row, 330.0), row, TEXT);
                let r = [q(905.0), qy(316.0), q(60.0), q(27.0)];
                self.edit_box12(p, r, &pp.pages, row);
                hits.push((r, "bsp12.printPages".into()));
                p.text(&format!("/  {n}"), q(982.0), mid(row, 327.0), row, TEXT);
                p.text(
                    &self.t("pdfEditor12.sample"),
                    q(905.0),
                    mid(small, 360.0),
                    small,
                    TEXT,
                );
                p.text(
                    &self.t("pdfEditor12.paperSource"),
                    x0,
                    mid(row, 391.0),
                    row,
                    TEXT,
                );
                let r = self.check12(p, q(904.0), qy(391.0), pp.tray_by_size, "", row, accent);
                hits.push((r, "bsp12.check.adv.tray".into()));
                p.text(
                    &self.t("pdfEditor12.readingOptions"),
                    x0,
                    mid(section, 494.0),
                    section,
                    TEXT,
                );
            }
            _ => {
                let rows = self.description12();
                p.text(
                    &self.t("pdfEditor12.general"),
                    x0,
                    mid(section, 164.0),
                    section,
                    TEXT,
                );
                let mut yc = 218.0;
                for (i, (label, value, cmd)) in rows.into_iter().enumerate() {
                    p.text(&label, x0, mid(row, yc), row, TEXT);
                    let (text, col) = match value {
                        Ok(v) => (v, TEXT),
                        Err(placeholder) => (placeholder, PLACEHOLDER),
                    };
                    // Values sit 3 px above their labels, as the reference's; a long file
                    // path keeps its end in a 413 px box (clipped on the left).
                    let vy = mid(row, yc - 3.0);
                    // The dialog-font estimate runs ~10 % wide on paths (measured: 306 px drawn
                    // for 341 px estimated).
                    let tw = crate::editor_form8::text_w(&text, row.size * 1.1) * 0.9;
                    let bw = q(413.0);
                    if i == 0 && tw > bw {
                        p.push_clip(xv, qy(yc - 16.0), bw, q(32.0));
                        p.text(&text, xv + bw - tw, vy, row, col);
                        p.pop_clip();
                    } else {
                        p.text(&text, xv, vy, row, col);
                    }
                    if let Some(cmd) = cmd {
                        hits.push(([xv - q(4.0), qy(yc - 15.0), q(420.0), q(30.0)], cmd));
                    }
                    yc += 35.0;
                }
                // Custom properties, dates and "Show More Information".
                // "Custom" 74 px below the last row (measured at 125 %).
                yc += 39.0;
                let custom = self.t("pdfEditor12.custom");
                p.text(&custom, x0, mid(row, yc), row, accent);
                hits.push((
                    [x0, qy(yc - 12.0), q(120.0), q(24.0)],
                    "bsp12.custom".into(),
                ));
                yc += 15.0;
                let customs = self
                    .view()
                    .map(|v| v.doc().custom_info())
                    .unwrap_or_default();
                for (k, v) in customs {
                    yc += 31.0;
                    p.text(&k, x0, mid(row, yc), row, TEXT);
                    p.text(&v, xv, mid(row, yc - 3.0), row, TEXT);
                }
                yc += 35.0;
                let info = self.view().map(|v| v.doc().doc_info()).unwrap_or_default();
                p.text(
                    &self.t("pdfEditor12.relatedDates"),
                    x0,
                    mid(section, yc),
                    section,
                    TEXT,
                );
                yc += 38.0;
                for (k, v) in [
                    ("pdfEditor12.lastModified", info.modified),
                    ("pdfEditor12.created", info.created),
                ] {
                    p.text(&self.t(k), x0, mid(row, yc), row, TEXT);
                    let v = if v.is_empty() {
                        "—".to_owned()
                    } else {
                        date_text(&v)
                    };
                    p.text(&v, xv, mid(row, yc - 3.0), row, TEXT);
                    yc += 31.0;
                }
                if self.bsp12.more {
                    for (k, v) in self.more_rows12() {
                        p.text(&k, x0, mid(row, yc), row, TEXT);
                        p.text(&v, xv, mid(row, yc - 3.0), row, TEXT);
                        yc += 31.0;
                    }
                }
                yc += 14.0;
                let more = if self.bsp12.more {
                    self.t("pdfEditor12.showLess")
                } else {
                    self.t("pdfEditor12.showMore")
                };
                p.text(&more, x0, mid(row, yc), row, accent);
                hits.push(([x0, qy(yc - 12.0), q(200.0), q(24.0)], "bsp12.more".into()));
            }
        }
        // Drop-downs: the box, then its list when open (last, over the rest).
        let mut list = None;
        for ((id, items, k), r) in combos {
            p.fill_rect(r[0], r[1], r[2], r[3], COMBO_FRAME);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
            let text = items.get(k).cloned().unwrap_or_default();
            p.text(
                &text,
                r[0] + q(3.0),
                (r[1] + (r[3] - row.size * 1.33) / 2.0).round(),
                row,
                DARK,
            );
            chevron_down(p, r[0] + r[2] - q(12.4), r[1] + r[3] / 2.0, sc, DARK);
            hits.push((r, format!("bsp12.combo.{id}")));
            if self.bsp12.open.is_some_and(|o| o.0 == id) {
                list = Some((id, items, k, r));
            }
        }
        p.pop_clip();
        if let Some((id, items, k, r)) = list {
            let ih = q(26.0);
            let lh = ih * items.len() as f32 + 2.0;
            let ly = if r[1] + r[3] + lh > h {
                r[1] - lh
            } else {
                r[1] + r[3]
            };
            p.fill_rect(r[0], ly, r[2], lh, COMBO_FRAME);
            p.fill_rect(r[0] + 1.0, ly + 1.0, r[2] - 2.0, lh - 2.0, WHITE);
            let (pxp, pyp) = self.pointer();
            for (j, it) in items.iter().enumerate() {
                let rr = [r[0] + 1.0, ly + 1.0 + ih * j as f32, r[2] - 2.0, ih];
                let over = crate::editor_window::inside(rr, pxp, pyp);
                if over {
                    p.fill_rect(rr[0], rr[1], rr[2], rr[3], Rgba(0xE5, 0xF1, 0xFB, 255));
                }
                if j == k {
                    p.fill_rect(rr[0], rr[1], 2.0, rr[3], accent);
                }
                p.text(
                    it,
                    rr[0] + q(4.0),
                    (rr[1] + (ih - row.size * 1.33) / 2.0).round(),
                    row,
                    DARK,
                );
                // List rows go first so they win over what lies under them.
                hits.insert(0, (rr, format!("bsp12.pick.{id}.{j}")));
            }
        }
    }

    /// A text box (as the reference's: top #ECECEC, bottom line #838383) showing `text`.
    fn edit_box12(&self, p: &mut dyn Painter, r: [f32; 4], text: &str, st: TextStyle) {
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xEC, 0xEC, 0xEC, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 1.0,
            Rgba(0xFE, 0xFE, 0xFE, 255),
        );
        p.fill_rect(
            r[0],
            r[1] + r[3] - 1.0,
            r[2],
            1.0,
            Rgba(0x83, 0x83, 0x83, 255),
        );
        let sc = self.ui_scale();
        p.text(
            text,
            r[0] + (6.0 / 1.25 * sc).round(),
            (r[1] + (r[3] - st.size * 1.33) / 2.0).round(),
            st,
            DARK,
        );
    }

    /// A 17 px check box at `x` centred on `yc` with its label; the hit box.
    #[allow(clippy::too_many_arguments)]
    fn check12(
        &self,
        p: &mut dyn Painter,
        x: f32,
        yc: f32,
        on: bool,
        text: &str,
        st: TextStyle,
        accent: Rgba,
    ) -> [f32; 4] {
        let sc = self.ui_scale();
        let s = (17.0 / 1.25 * sc).round();
        let y = (yc - s / 2.0).round();
        if on {
            p.fill_rect(x, y, s, s, accent);
            let k = s / 17.0;
            p.path(
                &[
                    PathSeg::M(x + 4.0 * k, y + 8.5 * k),
                    PathSeg::L(x + 7.0 * k, y + 11.5 * k),
                    PathSeg::L(x + 13.0 * k, y + 5.0 * k),
                ],
                None,
                Some((WHITE, (1.6 * k).max(1.2))),
            );
        } else {
            p.fill_rect(x, y, s, s, CHECK_FRAME);
            p.fill_rect(x + 1.0, y + 1.0, s - 2.0, s - 2.0, CHECK_FILL);
        }
        let lx = x + s + (3.0 / 1.25 * sc).round();
        if !text.is_empty() {
            p.text(text, lx, (yc - st.size * 1.33 / 2.0).round(), st, TEXT);
        }
        let tw = crate::editor_form8::text_w(text, st.size * 1.1);
        [x, y - 2.0, lx - x + tw + 4.0, s + 4.0]
    }

    /// Rows of Description › General: label, value (or the grey placeholder), the command a
    /// click runs.
    fn description12(&mut self) -> Vec<(String, Result<String, String>, Option<String>)> {
        let path = self.path().map(|p| p.to_string_lossy().into_owned());
        let Some(v) = self.view() else {
            return Vec::new();
        };
        let d = v.doc();
        let info = d.doc_info();
        let pages = d.page_count();
        let size = d.page_info(0).map(|p| (p.width, p.height));
        let yes_no = |b: bool| {
            if b {
                "pdfEditor12.yes"
            } else {
                "pdfEditor12.no"
            }
        };
        let mut rows = vec![
            (
                self.t("pdfEditor12.file"),
                Ok(path.unwrap_or_else(|| format!("{}.pdf", self.doc_stem()))),
                None,
            ),
            (
                self.t("pdfEditor12.pdfVersion"),
                Ok(format!("PDF-{}", info.version)),
                None,
            ),
            (
                self.t("pdfEditor12.fastWebView"),
                Ok(self.t(yes_no(info.linearized))),
                None,
            ),
            (
                self.t("pdfEditor12.taggedPdf"),
                Ok(self.t(yes_no(info.tagged))),
                None,
            ),
            (
                self.t("pdfEditor12.pageSize"),
                Ok(size.map_or_else(String::new, |(w, h)| {
                    format!("[{:.2} * {:.2} inch]", w / 72.0, h / 72.0)
                })),
                None,
            ),
            (self.t("pdfEditor12.pagesRow"), Ok(pages.to_string()), None),
        ];
        for (key, lk, ph) in INFO_ROWS {
            let v = match key {
                "Title" => info.title.clone(),
                "Subject" => info.subject.clone(),
                "Creator" => info.creator.clone(),
                "Author" => info.author.clone(),
                "Producer" => info.producer.clone(),
                _ => info.keywords.clone(),
            };
            let value = if v.is_empty() { Err(self.t(ph)) } else { Ok(v) };
            rows.push((self.t(lk), value, Some(format!("bsp12.info.{key}"))));
        }
        rows
    }

    /// "Show More Information": location and file size.
    fn more_rows12(&mut self) -> Vec<(String, String)> {
        let dir = self
            .path()
            .and_then(|p| p.parent())
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = self.view().map_or(0, |v| v.doc().bytes().len());
        vec![
            (self.t("pdfEditor12.location"), dir),
            (
                self.t("pdfEditor12.fileSize"),
                format!("{:.1} KB ({bytes} B)", bytes as f32 / 1024.0),
            ),
        ]
    }

    /// A click on the Properties page (`bsp12.*`); false when `cmd` is not one.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn props12_command(&mut self, cmd: &str) -> bool {
        let Some(rest) = cmd.strip_prefix("bsp12.") else {
            return false;
        };
        let was_open = self.bsp12.open.take();
        if let Some(k) = rest.strip_prefix("sub") {
            self.bsp12.sub = k.parse().unwrap_or(0).min(4);
            return true;
        }
        if let Some(k) = rest.strip_prefix("font") {
            if let Ok(k) = k.parse::<usize>() {
                if let Some(i) = self.bsp12.collapsed.iter().position(|&c| c == k) {
                    self.bsp12.collapsed.remove(i);
                } else {
                    self.bsp12.collapsed.push(k);
                }
            }
            return true;
        }
        if let Some(id) = rest.strip_prefix("combo.") {
            if was_open.is_none_or(|o| o.0 != id) {
                let id: &'static str = match id {
                    "iv.nav" => "iv.nav",
                    "iv.layout" => "iv.layout",
                    "iv.mag" => "iv.mag",
                    "adv.scaling" => "adv.scaling",
                    "adv.duplex" => "adv.duplex",
                    "adv.copies" => "adv.copies",
                    "adv.binding" => "adv.binding",
                    _ => "adv.lang",
                };
                self.bsp12.open = Some((id, [0.0; 4]));
            }
            return true;
        }
        if let Some(pick) = rest.strip_prefix("pick.") {
            let Some((id, k)) = pick.rsplit_once('.') else {
                return true;
            };
            let k: usize = k.parse().unwrap_or(0);
            self.set_combo12(id, k);
            return true;
        }
        if let Some(id) = rest.strip_prefix("check.") {
            if let Some(f) = id.strip_prefix("adv.") {
                if f == "tray" {
                    self.edit_presets12(|p| p.tray_by_size = !p.tray_by_size);
                }
                return true;
            }
            self.edit_view12(|v| {
                let b = match id {
                    "iv.title" => &mut v.display_title,
                    "iv.full" => &mut v.full_screen,
                    "iv.center" => &mut v.center_window,
                    "iv.resize" => &mut v.resize_window,
                    "iv.menubar" => &mut v.hide_menubar,
                    "iv.toolbars" => &mut v.hide_toolbars,
                    _ => &mut v.hide_window_controls,
                };
                *b = !*b;
            });
            return true;
        }
        if let Some(key) = rest.strip_prefix("info.") {
            let key = INFO_ROWS
                .iter()
                .find(|r| r.0 == key)
                .map_or("Title", |r| r.0);
            let cur = self
                .view()
                .map(|v| {
                    let i = v.doc().doc_info();
                    match key {
                        "Title" => i.title,
                        "Subject" => i.subject,
                        "Creator" => i.creator,
                        "Author" => i.author,
                        "Producer" => i.producer,
                        _ => i.keywords,
                    }
                })
                .unwrap_or_default();
            let lk = INFO_ROWS.iter().find(|r| r.0 == key).map_or("", |r| r.1);
            self.open_value12(Value12::Info(key), &self.t(lk), cur);
            return true;
        }
        match rest {
            "more" => self.bsp12.more = !self.bsp12.more,
            "custom" => self.open_custom12(),
            "openPage" => {
                let cur = self.view().map_or(1, |v| v.doc().initial_view().page + 1);
                let l = self.t("pdfEditor12.openToPage");
                self.open_value12(Value12::OpenPage, &l, cur.to_string());
            }
            "printPages" => {
                let cur = self
                    .view()
                    .map(|v| v.doc().print_presets().pages)
                    .unwrap_or_default();
                let l = self.t("pdfEditor12.pages");
                self.open_value12(Value12::PrintPages, &l, cur);
            }
            "secMenu" => {
                let enc = self.view().is_some_and(|v| v.doc().is_encrypted());
                self.push_command(if enc {
                    "prot.removeSecurity"
                } else {
                    "prot.password"
                });
            }
            "changeSecurity" => self.push_command("prot.password"),
            _ => {}
        }
        true
    }

    fn set_combo12(&mut self, id: &str, k: usize) {
        match id {
            "iv.nav" => self.edit_view12(|v| v.navigation = k.min(4)),
            "iv.layout" => self.edit_view12(|v| v.layout = k.min(6)),
            "iv.mag" => self.edit_view12(|v| v.magnification = k.min(5 + ZOOMS.len())),
            "adv.scaling" => self.edit_presets12(|p| p.scaling = k.min(1)),
            "adv.duplex" => self.edit_presets12(|p| p.duplex = k.min(2)),
            "adv.copies" => self.edit_presets12(|p| p.copies = k.min(5) as u32),
            "adv.binding" => self.edit_presets12(|p| p.binding = k.min(1)),
            _ => self.edit_presets12(|p| p.language = LANGS.get(k).copied().unwrap_or("").into()),
        }
    }

    /// Change the initial view (applied at once, one undo step, as the reference applies it).
    fn edit_view12(&mut self, f: impl FnOnce(&mut InitialView)) {
        let Some(mut v) = self.view().map(|v| v.doc().initial_view()) else {
            return;
        };
        f(&mut v);
        if let Err(e) = self.doc_edit(|d| d.set_initial_view(&v)) {
            self.set_status(&e);
        }
        self.after_doc_edit();
    }

    fn edit_presets12(&mut self, f: impl FnOnce(&mut PrintPresets)) {
        let Some(mut p) = self.view().map(|v| v.doc().print_presets()) else {
            return;
        };
        f(&mut p);
        if let Err(e) = self.doc_edit(|d| d.set_print_presets(&p)) {
            self.set_status(&e);
        }
        self.after_doc_edit();
    }

    /// A one-box dialog for a value of the Properties page.
    fn open_value12(&mut self, what: Value12, label_text: &str, cur: String) {
        let t = |k: &str| self.t(k);
        let c = vec![
            label(24.0, 70.0, label_text.trim_end_matches(':').to_owned()),
            edit("value", [24.0, 90.0, 405.0, 29.0], On::Always),
            button(
                "ok",
                [243.0, 146.0, 91.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [343.0, 146.0, 91.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor12.propsTitle"), (453.0, 192.0), c)
            .with("value", Val::Text(cur))
            .focused("value");
        self.form8
            .push((f, FormWhat::R12(crate::editor_fields12::Dlg12::Value(what))));
    }

    /// Custom: a name and a value to add to the information dictionary.
    fn open_custom12(&mut self) {
        let t = |k: &str| self.t(k);
        let c = vec![
            label(24.0, 70.0, t("pdfEditor12.customName")),
            edit("name", [24.0, 90.0, 405.0, 29.0], On::Always),
            label(24.0, 140.0, t("pdfEditor12.customValue")),
            edit("value", [24.0, 160.0, 405.0, 29.0], On::Always),
            button(
                "ok",
                [243.0, 216.0, 91.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [343.0, 216.0, 91.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor12.customTitle"), (453.0, 262.0), c).focused("name");
        self.form8.push((
            f,
            FormWhat::R12(crate::editor_fields12::Dlg12::Value(Value12::Custom)),
        ));
    }

    /// OK of a Properties value dialog.
    pub(crate) fn value12_ok(&mut self, f: &Form, what: Value12) {
        let v = f.text("value");
        let r = match what {
            Value12::Info(key) => self.doc_edit(|d| d.set_info(key, v.trim())),
            Value12::Custom => {
                let name = f.text("name");
                let name = name.trim().replace(' ', "");
                self.doc_edit(|d| d.set_info(&name, v.trim()))
            }
            Value12::OpenPage => {
                let n = self.view().map_or(1, crate::ReaderView::page_count);
                match v.trim().parse::<usize>() {
                    Ok(k) if k >= 1 && k <= n => {
                        let Some(mut iv) = self.view().map(|v| v.doc().initial_view()) else {
                            return;
                        };
                        iv.page = k - 1;
                        self.doc_edit(|d| d.set_initial_view(&iv))
                    }
                    _ => Err(self.t("pdfEditor.badPage")),
                }
            }
            Value12::PrintPages => {
                let Some(mut pp) = self.view().map(|v| v.doc().print_presets()) else {
                    return;
                };
                pp.pages = v.trim().to_owned();
                self.doc_edit(|d| d.set_print_presets(&pp))
            }
        };
        match r {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }
}

/// What a Properties value dialog sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value12 {
    Info(&'static str),
    Custom,
    OpenPage,
    PrintPages,
}

/// A small solid triangle pointing down, centred at (`cx`, `cy`) (9 × 5 px at 125 %).
fn arrow_down(p: &mut dyn Painter, cx: f32, cy: f32, sc: f32, c: Rgba) {
    let w = (7.2 * sc).round();
    let h = (4.0 * sc).round();
    p.path(
        &[
            PathSeg::M(cx - w / 2.0, cy - h / 2.0),
            PathSeg::L(cx + w / 2.0, cy - h / 2.0),
            PathSeg::L(cx, cy + h / 2.0),
            PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

/// A font name without its subset tag ("ABCDEF+Name" → "Name").
fn base_font(name: &str) -> &str {
    name.split_once('+').map_or(name, |s| s.1)
}

/// Short encoding names as the reference shows them ("WinAnsiEncoding" → "Ansi").
fn encoding_name(e: &str) -> &str {
    match e {
        "WinAnsiEncoding" => "Ansi",
        "MacRomanEncoding" => "Roman",
        "StandardEncoding" => "Standard",
        "MacExpertEncoding" => "Expert",
        "PDFDocEncoding" => "PDFDoc",
        other => other,
    }
}

/// A thin chevron pointing down, centred at (`cx`, `cy`) (11 × 6 px at 125 %, 1.5 px grey
/// stroke, as the reference's combo boxes).
fn chevron_down(p: &mut dyn Painter, cx: f32, cy: f32, sc: f32, _c: Rgba) {
    let w = (8.8 * sc).round();
    let h = (4.4 * sc).round();
    p.path(
        &[
            PathSeg::M(cx - w / 2.0, cy - h / 2.0),
            PathSeg::L(cx, cy + h / 2.0),
            PathSeg::L(cx + w / 2.0, cy - h / 2.0),
        ],
        None,
        Some((Rgba(0x80, 0x80, 0x80, 255), (1.2 * sc).max(1.0))),
    );
}

#[cfg(test)]
#[path = "editor_bsprops12_tests.rs"]
mod tests;
