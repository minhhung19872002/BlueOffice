//! BlueEditor round 9: the Convert drop-down menus and Organize › Rotate Pages as the reference
//! editor has them (black-box, 125 %). From Files ▾ — From File, From Multiple Files, Convert
//! to PDF Settings; To Office ▾ — Word, Word 97-2003, Excel, XML Spreadsheet 2003, PowerPoint,
//! XPS; To Image ▾ — PNG, JPEG, JPEG2000, TIFF, BMP; To Other ▾ — Plain Text, Rich Text,
//! Accessible Text, XML 1.0; Rotate Pages ▾ — Left, Right, Multi-page rotate (the "Rotate
//! Pages" dialog, 481 × 469 px). Formats the BlueOffice engines do not write are dimmed.
//! TIFF (TIFF 6.0 baseline, uncompressed RGB) and BMP (24-bit `BITMAPINFOHEADER`) are written
//! here; Rich Text is RTF 1.9 (public specification) of the pages' text.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, combo, edit, group, label, label_on, radio, Ctl, Form, On, Val};
use crate::editor_window::EditorWindow;
use crate::popup::{Choice, MenuItem};
use crate::reader_window::ReaderRequest;

/// One menu row: command, label key, icon, dimmed.
type Row = (&'static str, &'static str, Option<&'static str>, bool);

/// Rows of the Convert menus and the Rotate Pages menu, by the button's command.
pub(crate) fn menu_rows(id: &str) -> Option<Vec<Row>> {
    Some(match id {
        "conv.create" => vec![
            (
                "conv.create.file",
                "pdfEditor9.fromFile",
                Some("file"),
                false,
            ),
            (
                "conv9.fromMultiple",
                "pdfEditor9.fromMultiple",
                Some("createPdf"),
                false,
            ),
            (
                "conv9.settings",
                "pdfEditor9.convertSettings",
                Some("properties"),
                true,
            ),
        ],
        "conv.officeMenu" => vec![
            ("conv.word", "pdfEditor9.toWord", Some("toWord"), false),
            ("conv9.word97", "pdfEditor9.toWord97", Some("toWord"), true),
            ("conv.excel", "pdfEditor9.toExcel", Some("toExcel"), false),
            (
                "conv9.xml2003",
                "pdfEditor9.toXml2003",
                Some("toExcel"),
                true,
            ),
            (
                "conv.slides",
                "pdfEditor9.toPowerPoint",
                Some("toSlides"),
                false,
            ),
            ("conv9.xps", "pdfEditor9.toXps", Some("file"), true),
        ],
        "conv.imageMenu" => vec![
            ("conv.image", "pdfEditor9.png", Some("toImage"), false),
            ("conv.imageJpeg", "pdfEditor9.jpeg", Some("toImage"), false),
            (
                "conv9.jpeg2000",
                "pdfEditor9.jpeg2000",
                Some("toImage"),
                true,
            ),
            ("conv9.tiff", "pdfEditor9.tiff", Some("toImage"), false),
            ("conv9.bmp", "pdfEditor9.bmp", Some("toImage"), false),
        ],
        "conv.otherMenu" => vec![
            ("conv.text", "pdfEditor9.toPlainText", Some("file"), false),
            ("conv9.rtf", "pdfEditor9.toRichText", Some("file"), false),
            (
                "conv9.accessible",
                "pdfEditor9.toAccessibleText",
                Some("file"),
                false,
            ),
            ("conv9.xml", "pdfEditor9.toXml", Some("file"), true),
        ],
        "org.rotateMenu" => vec![
            (
                "org.rotateLeft",
                "pdfEditor9.rotateLeft",
                Some("rotateLeft"),
                false,
            ),
            (
                "org.rotateRight",
                "pdfEditor9.rotateRight",
                Some("rotateRight"),
                false,
            ),
            (
                "conv9.rotateMulti",
                "pdfEditor9.rotateMulti",
                Some("rotate"),
                false,
            ),
        ],
        _ => return None,
    })
}

/// A 24-bit bottom-up BMP of RGBA pixels (alpha dropped over white).
pub(crate) fn encode_bmp(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let row = (w * 3).div_ceil(4) * 4;
    let size = 54 + row * h;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(row * h).to_le_bytes());
    // 150 dpi in pixels per metre.
    out.extend_from_slice(&5906u32.to_le_bytes());
    out.extend_from_slice(&5906u32.to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    for y in (0..h as usize).rev() {
        let start = out.len();
        for x in 0..w as usize {
            let [r, g, b] = over_white(rgba, (y * w as usize + x) * 4);
            out.extend_from_slice(&[b, g, r]);
        }
        out.resize(start + row as usize, 0);
    }
    out
}

/// An uncompressed baseline RGB TIFF (TIFF 6.0 §8) of RGBA pixels (alpha over white).
pub(crate) fn encode_tiff(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let data_len = w * h * 3;
    // Header 8, pixels, then the bits-per-sample triple, resolution rationals and the IFD.
    let bps_at = 8 + data_len;
    let res_at = bps_at + 6;
    let ifd_at = (res_at + 16).div_ceil(2) * 2;
    let mut out = Vec::with_capacity(ifd_at as usize + 2 + 12 * 12 + 4);
    out.extend_from_slice(b"II*\0");
    out.extend_from_slice(&ifd_at.to_le_bytes());
    for i in 0..(w * h) as usize {
        out.extend_from_slice(&over_white(rgba, i * 4));
    }
    for _ in 0..3 {
        out.extend_from_slice(&8u16.to_le_bytes());
    }
    for _ in 0..2 {
        out.extend_from_slice(&150u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
    }
    out.resize(ifd_at as usize, 0);
    // (tag, type, count, value): type 3 SHORT, 4 LONG, 5 RATIONAL.
    let entries: [(u16, u16, u32, u32); 12] = [
        (256, 4, 1, w),
        (257, 4, 1, h),
        (258, 3, 3, bps_at),
        (259, 3, 1, 1),
        (262, 3, 1, 2),
        (273, 4, 1, 8),
        (277, 3, 1, 3),
        (278, 4, 1, h),
        (279, 4, 1, data_len),
        (282, 5, 1, res_at),
        (283, 5, 1, res_at + 8),
        (296, 3, 1, 2),
    ];
    out.extend_from_slice(&12u16.to_le_bytes());
    for (tag, ty, count, v) in entries {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if ty == 3 && count == 1 {
            out.extend_from_slice(&u16::try_from(v).unwrap_or(0).to_le_bytes());
            out.extend_from_slice(&[0, 0]);
        } else {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

fn over_white(rgba: &[u8], i: usize) -> [u8; 3] {
    let px = rgba.get(i..i + 4).unwrap_or(&[255, 255, 255, 255]);
    let a = u32::from(px[3]);
    let mix = |c: u8| u8::try_from((u32::from(c) * a + 255 * (255 - a)) / 255).unwrap_or(255);
    [mix(px[0]), mix(px[1]), mix(px[2])]
}

/// Rich Text of plain text: pages split by a page break (`\page`), lines by `\par`
/// (RTF 1.9.1; non-ASCII as `\uN?`).
pub(crate) fn to_rtf(text: &str) -> String {
    let mut out = String::from(
        "{\\rtf1\\ansi\\ansicpg1252\\deff0{\\fonttbl{\\f0\\fswiss Arial;}}\\f0\\fs22\n",
    );
    let pages: Vec<&str> = text.split('\u{c}').collect();
    for (k, page) in pages.iter().enumerate() {
        if k > 0 {
            out.push_str("\\page\n");
        }
        for line in page.trim_end_matches('\n').split('\n') {
            for c in line.chars() {
                match c {
                    '\\' | '{' | '}' => {
                        out.push('\\');
                        out.push(c);
                    }
                    ' '..='~' => out.push(c),
                    '\t' => out.push_str("\\tab "),
                    '\r' => {}
                    _ => {
                        let mut buf = [0u16; 2];
                        for u in c.encode_utf16(&mut buf) {
                            // RTF \u takes a signed 16-bit number.
                            out.push_str(&format!("\\u{}?", i16::from_ne_bytes(u.to_ne_bytes())));
                        }
                    }
                }
            }
            out.push_str("\\par\n");
        }
    }
    out.push('}');
    out
}

impl EditorWindow {
    fn menu_conv9(&mut self, rows: &[Row], id: &'static str) {
        let Some(r) = self.r.control_rect(id) else {
            return;
        };
        let items: Vec<MenuItem> = rows
            .iter()
            .map(|(cmd, key, _, dim)| MenuItem {
                label: self.t(key),
                choice: Choice::Command((*cmd).to_owned()),
                checked: false,
                dim: *dim,
            })
            .collect();
        let icons = rows.iter().map(|r| r.2).collect();
        let menu = self
            .r
            .reader_menu(items, r)
            .with_decor(icons, Vec::new())
            .with_icon_fn(crate::reader_icons::menu_icon);
        self.r.set_menu9(menu, id);
    }

    /// Round 9 Convert / Rotate commands; false when `cmd` is not one.
    pub(crate) fn conv9_command(&mut self, cmd: &str) -> bool {
        let id: Option<&'static str> = match cmd {
            "conv.create" => Some("conv.create"),
            "conv.officeMenu" => Some("conv.officeMenu"),
            "conv.imageMenu" => Some("conv.imageMenu"),
            "conv.otherMenu" => Some("conv.otherMenu"),
            "org.rotateMenu" => Some("org.rotateMenu"),
            _ => None,
        };
        if let Some(id) = id {
            if let Some(rows) = menu_rows(id) {
                if self.r.control_rect(id).is_some() {
                    self.menu_conv9(&rows, id);
                    return true;
                }
            }
            // Not on the ribbon (the File page): the command's own behaviour.
            return false;
        }
        match cmd {
            "conv.create.file" => self.push(ReaderRequest::PickFile {
                purpose: "create".into(),
                filters: crate::editor_actions::OFFICE_FILTERS
                    .iter()
                    .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                    .collect(),
            }),
            "conv9.fromMultiple" => self.open_combine9(),
            "conv9.tiff" | "conv9.bmp" => self.push(ReaderRequest::RenderPages {
                purpose: cmd.trim_start_matches("conv9.").into(),
                scale: 150.0 / 72.0,
            }),
            "conv9.rtf" | "conv9.accessible" => {
                let rtf = cmd == "conv9.rtf";
                let stem = self.doc_stem();
                let r = self.with_doc(|d| Ok(crate::editor_actions::document_text(d)));
                match r {
                    Ok(text) if rtf => self.push(ReaderRequest::SaveBytes {
                        name: format!("{stem}.rtf"),
                        bytes: to_rtf(&text).into_bytes(),
                    }),
                    // Accessible text: the reading order with blank lines between pages.
                    Ok(text) => self.push(ReaderRequest::SaveBytes {
                        name: format!("{stem}.txt"),
                        bytes: text.replace('\u{c}', "\n").into_bytes(),
                    }),
                    Err(e) => self.set_status(&e),
                }
            }
            "conv9.rotateMulti" => self.open_rotate9(),
            "file.optimize" => self.open_opt9(),
            "conv9.word97" | "conv9.xml2003" | "conv9.xps" | "conv9.jpeg2000" | "conv9.xml"
            | "conv9.settings" => {}
            _ => return false,
        }
        true
    }

    /// Pages rendered for TIFF / BMP; false for other purposes.
    pub(crate) fn conv9_rendered(&mut self, purpose: &str, pngs: &[Vec<u8>]) -> bool {
        let ext = match purpose {
            "tiff" => "tif",
            "bmp" => "bmp",
            _ => return false,
        };
        let stem = self.doc_stem();
        let files: Vec<(String, Vec<u8>)> = pngs
            .iter()
            .enumerate()
            .filter_map(|(i, png)| {
                let img = bo_raster::codecs::decode(png)?;
                let bytes = if ext == "tif" {
                    encode_tiff(&img.pixels, img.width, img.height)
                } else {
                    encode_bmp(&img.pixels, img.width, img.height)
                };
                Some((format!("{stem}-{}.{ext}", i + 1), bytes))
            })
            .collect();
        self.push(ReaderRequest::SaveMany(files));
        true
    }

    /// Open Rotate Pages (Multi-page rotate).
    pub(crate) fn open_rotate9(&mut self) {
        let t = |k: &str| self.t(k);
        let n = self
            .r
            .view()
            .map_or(0, crate::ReaderView::page_count)
            .max(1);
        let pages = self.target_pages();
        let c = vec![
            group(t("pdfEditor9.direction"), [16.0, 55.0, 449.0, 121.0]),
            radio("dir", 0, 28.0, 89.0, t("pdfEditor9.clockwise")),
            radio("dir", 1, 28.0, 121.0, t("pdfEditor9.counterclockwise")),
            radio("dir", 2, 28.0, 154.0, t("pdfEditor9.deg180")),
            group(t("pdfEditor9.pageRange2"), [16.0, 195.0, 449.0, 220.0]),
            radio("range", 0, 28.0, 229.0, t("pdfEditor9.allPagesR")),
            radio("range", 1, 28.0, 267.0, t("pdfEditor9.pageR")),
            edit("pages", [104.0, 253.0, 234.0, 29.0], On::If("range", 1)),
            label_on(358.0, 267.0, t("pdfEditor9.ofShort"), On::If("range", 1)),
            label_on(393.0, 267.0, n.to_string(), On::If("range", 1)),
            label(113.0, 303.0, t("pdfEditor9.sample")),
            Ctl::Info { x: 246.0, y: 303.0 },
            label(28.0, 349.0, t("pdfEditor9.rotateWhich")),
            combo(
                "subset",
                [104.0, 335.0, 234.0, 27.0],
                vec![
                    t("pdfEditor9.allInRange"),
                    t("pdfEditor9.evenOnly"),
                    t("pdfEditor9.oddOnly"),
                ],
                On::Always,
            ),
            combo(
                "shape",
                [104.0, 375.0, 234.0, 27.0],
                vec![
                    t("pdfEditor9.landscapeAndPortrait"),
                    t("pdfEditor9.landscapeOnly"),
                    t("pdfEditor9.portraitOnly"),
                ],
                On::Always,
            ),
            button(
                "ok",
                [275.0, 424.0, 87.0, 27.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [373.0, 425.0, 91.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let text = format!(
            "{},",
            crate::editor_more::range_text(&pages).replace(' ', "")
        );
        let f = Form::new(t("pdfEditor9.rotatePagesTitle"), (481.0, 469.0), c)
            .with("dir", Val::Pick(0))
            .with("range", Val::Pick(0))
            .with("pages", Val::Text(text))
            .with("subset", Val::Pick(0))
            .with("shape", Val::Pick(0));
        self.form8.push((f, FormWhat::Rotate9));
    }

    /// OK of Rotate Pages.
    pub(crate) fn rotate9_ok(&mut self, f: &Form) {
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        let mut pages: Vec<usize> = if f.pick("range") == 0 {
            (0..n).collect()
        } else {
            let text = f.text("pages");
            crate::editor_more::parse_pages(text.trim().trim_end_matches(','), "all", n)
        };
        match f.pick("subset") {
            1 => pages.retain(|p| (p + 1) % 2 == 0),
            2 => pages.retain(|p| (p + 1) % 2 == 1),
            _ => {}
        }
        let shape = f.pick("shape");
        if shape > 0 {
            if let Some(v) = self.r.view() {
                pages.retain(|&p| {
                    let (w, h) = v.page_size(p);
                    (w > h) == (shape == 1)
                });
            }
        }
        if pages.is_empty() {
            return;
        }
        let turns = match f.pick("dir") {
            0 => 1,
            1 => -1,
            _ => 2,
        };
        match self.doc_edit(|d| d.rotate_pages(&pages, turns)) {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }
}
