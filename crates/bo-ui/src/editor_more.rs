//! BlueEditor round 4, Organize and Convert tabs as in the reference editor: Move, Duplicate,
//! Replace, Swap, Interleaving, Resize Pages, Flatten, Format Page Numbers; Export All
//! Images, To HTML, From Clipboard, From Template (blank pages of a standard size), the
//! To Office / To Image / To Other menus. Commands the editor does not offer (From Web Page,
//! text recognition) stay grey with a tooltip.
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_actions::Ask2;
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use crate::{Painter, TextStyle};
use bo_pdfread::{LabelStyle, PdfDocument};
use bo_render::Rgba;

/// What a dialog of this module asks.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MoreAsk {
    /// Move: pages, before which page.
    Move,
    /// Swap: two page numbers.
    Swap,
    /// Interleave with the picked file (kept here): reverse its pages?
    Interleave(Vec<u8>),
    /// Resize: width, height (mm), pages, scale to fit.
    Resize,
    /// Flatten: pages.
    Flatten,
    /// Format Page Numbers: from, to, style, prefix, start.
    PageNumbers,
}

/// Standard page sizes of From Template (points).
pub(crate) const SIZES: [(&str, f32, f32); 6] = [
    ("A4", 595.28, 841.89),
    ("A3", 841.89, 1190.55),
    ("A5", 419.53, 595.28),
    ("Letter", 612.0, 792.0),
    ("Legal", 612.0, 1008.0),
    ("Tabloid", 792.0, 1224.0),
];

/// Commands shown but not offered, with the tooltip saying why.
pub(crate) const NOT_OFFERED: [(&str, &str); 6] = [
    ("conv.web", "pdfEditor.tipWeb"),
    ("conv.ocr", "pdfEditor.tipOcr"),
    ("conv.quickOcr", "pdfEditor.tipOcr"),
    ("conv.suspect", "pdfEditor.tipOcr"),
    ("conv.scanner", "pdfEditor.tipScanner"),
    ("org.rearrange", "pdfEditor.tipRearrange"),
];

const MM: f32 = 72.0 / 25.4;

/// "1-3, 5" for page indices (1-based, runs joined).
pub(crate) fn range_text(pages: &[usize]) -> String {
    let mut p: Vec<usize> = pages.to_vec();
    p.sort_unstable();
    p.dedup();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < p.len() {
        let mut j = i;
        while j + 1 < p.len() && p[j + 1] == p[j] + 1 {
            j += 1;
        }
        out.push(if i == j {
            (p[i] + 1).to_string()
        } else {
            format!("{}-{}", p[i] + 1, p[j] + 1)
        });
        i = j + 1;
    }
    out.join(", ")
}

/// Page indices of "1-3, 5" (1-based) among `n` pages; empty text or `all` is every page.
pub(crate) fn parse_pages(s: &str, all: &str, n: usize) -> Vec<usize> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case(all) || s.eq_ignore_ascii_case("all") {
        return (0..n).collect();
    }
    let mut out = Vec::new();
    for part in s.split([',', ';', ' ']).filter(|p| !p.is_empty()) {
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let a: usize = a.trim().parse().unwrap_or(0);
        let b: usize = if b.trim().is_empty() {
            n
        } else {
            b.trim().parse().unwrap_or(0)
        };
        if a == 0 || b == 0 {
            continue;
        }
        for k in a.min(b)..=a.max(b).min(n) {
            if !out.contains(&(k - 1)) {
                out.push(k - 1);
            }
        }
    }
    out.sort_unstable();
    out
}

/// A one-page PDF of `w` × `h` points.
pub(crate) fn blank_pdf_sized(w: f32, h: f32) -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] /Resources << >> /Contents 4 0 R >>"
        ),
        "<< /Length 0 >>\nstream\n\nendstream".to_owned(),
    ];
    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// A PDF with one page showing picture `png` (96 dpi: one pixel is ¾ point).
pub(crate) fn picture_pdf(png: &[u8]) -> Result<Vec<u8>, String> {
    let (w, h) = bo_pdfread::image_size(png).ok_or("not a picture")?;
    let (pw, ph) = (w as f32 * 0.75, h as f32 * 0.75);
    let mut doc = PdfDocument::open(blank_pdf_sized(pw, ph), bo_pdfread::standard_fonts())?;
    doc.add_image(0, [0.0, 0.0, pw, ph], bo_pdfread::NewImage(png))?;
    doc.save_full(bo_pdfread::SaveOptions::default())
}

fn style_of(s: &str) -> LabelStyle {
    match s.trim() {
        "1" | "1, 2, 3" => LabelStyle::Decimal,
        "i" | "i, ii, iii" => LabelStyle::RomanLower,
        "I" | "I, II, III" => LabelStyle::RomanUpper,
        "a" | "a, b, c" => LabelStyle::LettersLower,
        "A" | "A, B, C" => LabelStyle::LettersUpper,
        _ => LabelStyle::None,
    }
}

fn style_text(s: LabelStyle) -> &'static str {
    match s {
        LabelStyle::Decimal => "1, 2, 3",
        LabelStyle::RomanLower => "i, ii, iii",
        LabelStyle::RomanUpper => "I, II, III",
        LabelStyle::LettersLower => "a, b, c",
        LabelStyle::LettersUpper => "A, B, C",
        LabelStyle::None => "",
    }
}

impl EditorWindow {
    fn more_dialog(
        &mut self,
        what: MoreAsk,
        title: &str,
        fields: Vec<(String, String)>,
        checks: Vec<(String, bool)>,
    ) {
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t(title),
            fields,
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        if !checks.is_empty() {
            d = d.with_checks(checks);
        }
        d.layout(
            &crate::editor_window::Estimate,
            self.window_size(),
            self.ui_scale(),
        );
        self.dialog2 = Some((d, Ask2::More(what)));
    }

    fn page_count(&self) -> usize {
        self.view().map_or(0, crate::ReaderView::page_count)
    }

    /// Commands of this module; false when `cmd` is not one.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn more_command(&mut self, cmd: &str) -> bool {
        if NOT_OFFERED.iter().any(|(c, _)| *c == cmd) {
            return true;
        }
        if let Some(name) = cmd.strip_prefix("conv.template.") {
            if let Some((n, w, h)) = SIZES.iter().find(|s| s.0 == name) {
                let name = format!("{} ({n}).pdf", self.t("pdfEditor.untitled"));
                self.push(ReaderRequest::OpenDocument {
                    name,
                    bytes: blank_pdf_sized(*w, *h),
                });
            }
            return true;
        }
        let f = |w: &Self, k: &str, v: String| (w.t(k), v);
        match cmd {
            "org.move" => {
                let pages = self.target_pages();
                let to = pages.last().map_or(1, |p| p + 2);
                let fields = vec![
                    f(self, "pdfEditor.pagesField", range_text(&pages)),
                    f(self, "pdfEditor.beforePage", to.to_string()),
                ];
                self.more_dialog(MoreAsk::Move, "pdfEditor.movePages", fields, Vec::new());
            }
            "org.duplicate" => {
                let pages = self.target_pages();
                match self.doc_edit(|d| d.duplicate_pages(&pages)) {
                    Ok(copies) => {
                        self.after_doc_edit();
                        if let (Some(a), Some(b)) = (copies.first(), copies.last()) {
                            self.org.select_range(*a, *b);
                        }
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            "org.replace" => self.push(ReaderRequest::PickFile {
                purpose: "replacePages".into(),
                filters: vec![("PDF (*.pdf)".into(), "*.pdf".into())],
            }),
            "org.interleave" => self.push(ReaderRequest::PickFile {
                purpose: "interleave".into(),
                filters: vec![("PDF (*.pdf)".into(), "*.pdf".into())],
            }),
            "org.swap" => {
                let pages = self.target_pages();
                let a = pages.first().copied().unwrap_or(0);
                let b = pages
                    .get(1)
                    .copied()
                    .unwrap_or((a + 1).min(self.page_count().saturating_sub(1)));
                let fields = vec![
                    f(self, "pdfEditor.swapPage", (a + 1).to_string()),
                    f(self, "pdfEditor.swapWith", (b + 1).to_string()),
                ];
                self.more_dialog(MoreAsk::Swap, "pdfEditor.swapPages", fields, Vec::new());
            }
            "org.resize" => {
                let cur = self
                    .view()
                    .map_or((595.28, 841.89), |v| v.page_size(v.current_page()));
                let mm = |v: f32| format!("{:.0}", v / MM);
                let fields = vec![
                    f(self, "pdfEditor.widthMm", mm(cur.0)),
                    f(self, "pdfEditor.heightMm", mm(cur.1)),
                    f(self, "pdfEditor.pagesField", self.t("pdfEditor.cropAll")),
                ];
                let checks = vec![(self.t("pdfEditor.scaleToFit"), true)];
                self.more_dialog(MoreAsk::Resize, "pdfEditor.resizePages", fields, checks);
            }
            "org.flatten" => {
                let fields = vec![f(self, "pdfEditor.pagesField", self.t("pdfEditor.cropAll"))];
                self.more_dialog(MoreAsk::Flatten, "pdfEditor.flatten", fields, Vec::new());
            }
            "org.pageNumbers" => {
                let pages = self.target_pages();
                let (a, b) = (
                    pages.first().copied().unwrap_or(0),
                    pages.last().copied().unwrap_or(0),
                );
                let (a, b) = if a == b {
                    (0, self.page_count().saturating_sub(1))
                } else {
                    (a, b)
                };
                let cur = self
                    .with_doc(|d| Ok(d.page_label_ranges()))
                    .unwrap_or_default();
                let range = cur.iter().rev().find(|r| r.start <= a);
                let style = range.map_or(LabelStyle::Decimal, |r| r.style);
                let prefix = range.map(|r| r.prefix.clone()).unwrap_or_default();
                let fields = vec![
                    f(self, "pdfEditor.fromPage", (a + 1).to_string()),
                    f(self, "pdfEditor.toPage", (b + 1).to_string()),
                    f(self, "pdfEditor.numberStyle", style_text(style).into()),
                    f(self, "pdfEditor.numberPrefix", prefix),
                    f(self, "pdfEditor.numberStart", "1".into()),
                ];
                self.more_dialog(
                    MoreAsk::PageNumbers,
                    "pdfEditor.pageNumbers",
                    fields,
                    Vec::new(),
                );
            }
            "conv.clipboard" => self.push(ReaderRequest::Command("host.clipboardImage".into())),
            "conv.templateMenu" => {
                let items: Vec<(&str, &str)> = vec![
                    ("conv.template.A4", "pdfEditor.sizeA4"),
                    ("conv.template.A3", "pdfEditor.sizeA3"),
                    ("conv.template.A5", "pdfEditor.sizeA5"),
                    ("conv.template.Letter", "pdfEditor.sizeLetter"),
                    ("conv.template.Legal", "pdfEditor.sizeLegal"),
                    ("conv.template.Tabloid", "pdfEditor.sizeTabloid"),
                ];
                self.menu(&items, "conv.templateMenu");
            }
            "conv.exportImages" => {
                let stem = self.doc_stem();
                match self.with_doc(|d| Ok(d.extract_images())) {
                    Ok(imgs) if imgs.is_empty() => {
                        let s = self.t("pdfEditor.noImages");
                        self.set_status(&s);
                    }
                    Ok(imgs) => {
                        let files = imgs
                            .into_iter()
                            .enumerate()
                            .map(|(k, i)| {
                                (
                                    format!("{stem}-p{}-{}.{}", i.page + 1, k + 1, i.ext),
                                    i.data,
                                )
                            })
                            .collect();
                        self.push(ReaderRequest::SaveMany(files));
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            "conv.html" => {
                let stem = self.doc_stem();
                match self.with_doc(|d| Ok(d.to_html(&stem))) {
                    Ok(h) => self.push(ReaderRequest::SaveBytes {
                        name: format!("{stem}.html"),
                        bytes: h.into_bytes(),
                    }),
                    Err(e) => self.set_status(&e),
                }
            }
            "conv.officeMenu" => {
                let items = [
                    ("conv.word", "pdfEditor.toWordFile"),
                    ("conv.excel", "pdfEditor.toExcelFile"),
                    ("conv.slides", "pdfEditor.toSlidesFile"),
                ];
                self.menu(&items, "conv.officeMenu");
            }
            "conv.imageMenu" => {
                let items = [
                    ("conv.image", "pdfEditor.imagePng"),
                    ("conv.imageJpeg", "pdfEditor.imageJpeg"),
                ];
                self.menu(&items, "conv.imageMenu");
            }
            "conv.imageJpeg" => self.push(ReaderRequest::RenderPages {
                purpose: "jpeg".into(),
                scale: 150.0 / 72.0,
            }),
            "conv.otherMenu" => {
                let items = [
                    ("conv.text", "pdfEditor.toTextFile"),
                    ("conv.html", "pdfEditor.toHtmlFile"),
                ];
                self.menu(&items, "conv.otherMenu");
            }
            _ => return false,
        }
        true
    }

    /// Pages rendered as PNG to save as JPEG files.
    pub(crate) fn jpeg_rendered(&mut self, pngs: &[Vec<u8>]) {
        let stem = self.doc_stem();
        let files: Vec<(String, Vec<u8>)> = pngs
            .iter()
            .enumerate()
            .filter_map(|(i, png)| {
                let img = bo_raster::codecs::decode(png)?;
                let jpg = bo_raster::jpeg::encode(&img, 90)?;
                Some((format!("{stem}-{}.jpg", i + 1), jpg))
            })
            .collect();
        self.push(ReaderRequest::SaveMany(files));
    }

    /// A file picked for this module; false when `purpose` is not one of its own.
    pub(crate) fn more_file_picked(&mut self, purpose: &str, bytes: &[u8]) -> bool {
        match purpose {
            "replacePages" => {
                let pages = self.target_pages();
                let pick: Vec<usize> = (0..pages.len()).collect();
                let r = self.doc_edit(|d| d.replace_pages(&pages, bytes, Some(&pick)));
                match r {
                    Ok(n) => {
                        self.after_doc_edit();
                        if let Some(&a) = pages.first() {
                            self.org.select_range(a, a + n.saturating_sub(1));
                        }
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            "interleave" => {
                let checks = vec![(self.t("pdfEditor.interleaveReverse"), false)];
                self.more_dialog(
                    MoreAsk::Interleave(bytes.to_vec()),
                    "pdfEditor.interleave",
                    Vec::new(),
                    checks,
                );
            }
            // A device-independent bitmap from the clipboard: a new PDF of it.
            "clipboardImage" => {
                let png = bo_raster::codecs::decode_dib(bytes).and_then(bo_raster::rgba_to_png);
                let r = png.ok_or_else(|| self.t("pdfEditor.noClipboardImage"));
                match r.and_then(|p| picture_pdf(&p)) {
                    Ok(pdf) => {
                        let name = format!("{}.pdf", self.t("pdfEditor.clipboardName"));
                        self.push(ReaderRequest::OpenDocument { name, bytes: pdf });
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            _ => return false,
        }
        true
    }

    /// The answer of a dialog of this module.
    pub(crate) fn more_answer(&mut self, what: MoreAsk, v: &[String]) {
        let n = self.page_count();
        let all = self.t("pdfEditor.cropAll");
        let get = |i: usize| v.get(i).map_or("", String::as_str).trim().to_owned();
        let num = |i: usize| get(i).parse::<usize>().unwrap_or(0);
        let on = |i: usize| v.get(i).is_some_and(|s| s == "1");
        let r: Result<(), String> = match what {
            MoreAsk::Move => {
                let pages = parse_pages(&get(0), &all, n);
                let to = num(1).clamp(1, n + 1) - 1;
                let r = self.doc_edit(|d| d.move_pages(&pages, to));
                if r.is_ok() {
                    let before = pages.iter().filter(|&&p| p < to).count();
                    let start = to - before;
                    self.org
                        .select_range(start, start + pages.len().saturating_sub(1));
                }
                r
            }
            MoreAsk::Swap => {
                let (a, b) = (num(0), num(1));
                if a == 0 || b == 0 {
                    Err(self.t("pdfEditor.badPage"))
                } else {
                    self.doc_edit(|d| d.swap_pages(a - 1, b - 1))
                }
            }
            MoreAsk::Interleave(bytes) => {
                let rev = on(0);
                self.doc_edit(|d| d.interleave_pdf(&bytes, rev).map(|_| ()))
            }
            MoreAsk::Resize => {
                let mm = |i: usize| get(i).replace(',', ".").parse::<f32>().unwrap_or(0.0);
                let (w, h) = (mm(0) * MM, mm(1) * MM);
                let pages = parse_pages(&get(2), &all, n);
                let fit = on(3);
                self.doc_edit(|d| d.resize_pages(&pages, w, h, fit))
            }
            MoreAsk::Flatten => {
                let pages = parse_pages(&get(0), &all, n);
                match self.doc_edit(|d| d.flatten_pages(&pages)) {
                    Ok(k) => {
                        let s = self
                            .t("pdfEditor.pagesFlattened")
                            .replace("{count}", &k.to_string());
                        self.set_status(&s);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            MoreAsk::PageNumbers => {
                let (a, b) = (num(0).max(1) - 1, num(1).max(1) - 1);
                let style = style_of(&get(2));
                let prefix = v.get(3).cloned().unwrap_or_default();
                let start = num(4).max(1) as u32;
                self.doc_edit(|d| {
                    d.set_page_labels(a, b.min(n.saturating_sub(1)), style, &prefix, start)
                })
            }
        };
        match r {
            Ok(()) => {
                if let Some(v) = self.view_mut() {
                    for i in 0..v.page_count() {
                        v.invalidate_page(i);
                    }
                }
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// Tooltip of a grey command under the pointer.
    pub(crate) fn paint_more_tip(&mut self, p: &mut dyn Painter) {
        let (x, y) = self.pointer();
        let Some((r, key)) = NOT_OFFERED
            .iter()
            .chain(crate::editor_more2::NOT_OFFERED2.iter())
            .find_map(|(id, key)| {
                self.r
                    .control_rect(id)
                    .filter(|r| crate::editor_window::inside(*r, x, y))
                    .map(|r| (r, *key))
            })
        else {
            return;
        };
        let s = self.ui_scale();
        let text = self.t(key);
        let style = TextStyle {
            size: 12.0 * s,
            bold: false,
        };
        let w = text.chars().count() as f32 * style.size * 0.5 + 16.0 * s;
        let h = 24.0 * s;
        let (ww, _) = self.window_size();
        let tx = (r[0] + 4.0 * s).min(ww - w - 4.0).max(0.0).round();
        let ty = (r[1] + r[3] + 4.0 * s).round();
        p.fill_rect(tx, ty, w, h, Rgba(118, 118, 118, 255));
        p.fill_rect(
            tx + 1.0,
            ty + 1.0,
            w - 2.0,
            h - 2.0,
            Rgba(255, 255, 255, 255),
        );
        p.text(
            &text,
            tx + 8.0 * s,
            ty + ((h - style.size * 1.33) / 2.0).round(),
            style,
            Rgba(51, 51, 51, 255),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges() {
        assert_eq!(range_text(&[0, 1, 2, 4]), "1-3, 5");
        assert_eq!(parse_pages("1-3, 5", "All", 6), vec![0, 1, 2, 4]);
        assert_eq!(parse_pages("All", "All", 3), vec![0, 1, 2]);
        assert_eq!(parse_pages("", "Tất cả", 2), vec![0, 1]);
        assert_eq!(parse_pages("5-", "All", 6), vec![4, 5]);
        assert_eq!(parse_pages("9", "All", 3), Vec::<usize>::new());
    }

    #[test]
    fn blank_and_picture_pdfs_open() {
        let doc = PdfDocument::open(blank_pdf_sized(612.0, 792.0), bo_pdfread::standard_fonts())
            .expect("opens");
        assert_eq!(doc.page_info(0).map(|p| p.width), Some(612.0));
        let img = bo_raster::codecs::Rgba {
            width: 8,
            height: 4,
            pixels: vec![200; 8 * 4 * 4],
        };
        let png = bo_raster::rgba_to_png(img).expect("png");
        let pdf = picture_pdf(&png).expect("pdf");
        let doc = PdfDocument::open(pdf, bo_pdfread::standard_fonts()).expect("opens");
        assert_eq!(
            doc.page_info(0).map(|p| (p.width, p.height)),
            Some((6.0, 3.0))
        );
    }
}
