//! BlueEditor round 8: Header & Footer, Watermark, Background, Bates Numbering, Password
//! Protection, Insert Blank Page / Insert Pages, Extract Pages and Split Document in the
//! reference editor's classic layout ([`crate::editor_form8::Form`]; positions are the
//! reference's at 125 %, measured on its dialogs, `docs/ui/REPORT-editor.md` round 8).
use crate::editor_form8::{
    button, check, combo, edit, group, label, label_on, link, radio, rlabel, spin, Ctl, Form,
    FormOut, IconKind, On, Val,
};
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use crate::Input;
use bo_pdfread::{PageMarks, SaveOptions};

/// What an open classic dialog is for.
#[derive(Debug, Clone)]
pub(crate) enum FormWhat {
    HeaderFooter,
    Watermark,
    Background,
    /// Bates: the file list, then the numbering options.
    BatesFiles,
    BatesOptions,
    Password,
    /// The permissions sub-dialog of Password Protection.
    Permissions,
    /// Insert Blank Page, or Insert Pages of a picked file (name, bytes).
    Insert(Option<(String, Vec<u8>)>),
    Extract,
    Split,
    /// A small dialog whose values go back into the dialog under it on OK (Appearance
    /// Options, Page number and date format, Page Range Options, text style).
    Sub,
    /// Name of a preset to save from the dialog under it (preset kind).
    SavePreset(&'static str),
}

/// Font sizes offered by the Size lists.
const SIZES: [&str; 16] = [
    "6", "7", "8", "9", "10", "11", "12", "14", "16", "18", "20", "24", "28", "36", "48", "72",
];

/// Units of the Position distances and their points.
const UNITS: [(&str, f32); 4] = [
    ("pdfEditor8.inches", 72.0),
    ("pdfEditor8.centimeters", 72.0 / 2.54),
    ("pdfEditor8.millimeters", 72.0 / 25.4),
    ("pdfEditor8.points", 1.0),
];

/// Pages of "1, 3-5, 8" (1-based, within `n`) as groups split at the commas.
pub(crate) fn page_groups(s: &str, n: usize) -> Vec<Vec<usize>> {
    s.split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let (a, b) = match part.split_once('-') {
                Some((a, b)) => (a.trim().parse::<usize>().ok()?, b.trim().parse().ok()?),
                None => {
                    let v = part.parse::<usize>().ok()?;
                    (v, v)
                }
            };
            let (a, b) = (a.min(b).max(1), a.max(b).min(n));
            (a <= b).then(|| (a - 1..b).collect())
        })
        .collect()
}

impl EditorWindow {
    fn ok_cancel(&self, y: f32, ok_x: f32) -> Vec<Ctl> {
        vec![
            button(
                "ok",
                [ok_x, y, 91.0, 28.0],
                self.t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [ok_x + 100.0, y, 91.0, 28.0],
                self.t("pdfReader.cancel"),
                On::Always,
            ),
        ]
    }

    fn page_total(&self) -> usize {
        self.r.view().map_or(0, crate::ReaderView::page_count)
    }

    fn open_form(&mut self, f: Form, what: FormWhat) {
        self.form8.push((f, what));
    }

    /// Round 8 commands; false when `cmd` is not one.
    pub(crate) fn dlg8_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "org.headerFooter" | "org.watermark" | "org.background" | "org.bates" => {
                let k = cmd.trim_start_matches("org.");
                let items = [
                    (format!("org8.add.{k}"), "pdfEditor8.add"),
                    (format!("org8.remove.{k}"), "pdfEditor8.removeAll"),
                ];
                let refs: Vec<(&str, &str)> = items.iter().map(|(c, l)| (c.as_str(), *l)).collect();
                let id: &'static str = match k {
                    "headerFooter" => "org.headerFooter",
                    "watermark" => "org.watermark",
                    "background" => "org.background",
                    _ => "org.bates",
                };
                self.menu(&refs, id);
            }
            "org8.add.headerFooter" => self.open_header_footer(),
            "org8.add.watermark" => self.open_watermark(),
            "org8.add.background" => self.open_background(),
            "org8.add.bates" => self.open_bates_files(),
            "org8.remove.headerFooter" | "org8.remove.bates" => {
                self.remove_marks(&["Header", "Footer"]);
            }
            "org8.remove.watermark" => self.remove_marks(&["Watermark"]),
            "org8.remove.background" => self.remove_marks(&["Background"]),
            "prot.password" => self.open_password(),
            "org.insertBlank" => self.open_insert(None),
            "org.extract" => self.open_extract(),
            "org.split" => self.open_split(),
            "obj8.extractImage" => self.extract_image8(),
            "fmt8.expand" => {
                self.format_open8 = true;
                self.sync_format();
            }
            "cmt8.props" => {
                self.comment_pane8 = !self.comment_pane8;
                self.format_open8 = true;
                self.format_closed = false;
                self.sync_format();
            }
            _ => return false,
        }
        true
    }

    fn remove_marks(&mut self, kinds: &[&str]) {
        match self.doc_edit(|d| d.remove_page_marks(kinds)) {
            Ok(_) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }

    /// A file picked for Insert › From File: ask where its pages go.
    pub(crate) fn dlg8_file_picked(&mut self, purpose: &str, name: &str, bytes: &[u8]) -> bool {
        if purpose == "wm8Image" {
            self.wm_image_picked(name, bytes);
            return true;
        }
        if purpose != "insert" || bo_pdfread::PdfDocument::needs_password(bytes) {
            return false;
        }
        self.open_insert(Some((name.to_owned(), bytes.to_vec())));
        true
    }

    // --- Header & Footer -------------------------------------------------------------

    fn open_header_footer(&mut self) {
        let t = |k: &str| self.t(k);
        let mut c = vec![
            label(16.0, 60.0, t("pdfEditor8.savedSettings")),
            combo(
                "saved",
                [123.0, 46.0, 148.0, 28.0],
                self.preset_names("hf"),
                On::Always,
            ),
            button(
                "delete",
                [279.0, 46.0, 91.0, 28.0],
                t("pdfEditor8.delete"),
                On::Always,
            ),
            button(
                "save",
                [378.0, 46.0, 115.0, 28.0],
                t("pdfEditor8.saveSettings"),
                On::Always,
            ),
            group(t("pdfEditor8.font"), [17.0, 92.0, 612.0, 95.0]),
            label(28.0, 147.0, t("pdfEditor8.name")),
            combo(
                "font",
                [77.0, 133.0, 197.0, 28.0],
                vec!["Helvetica".into()],
                On::Always,
            ),
            label(281.0, 147.0, t("pdfEditor8.size")),
            combo(
                "size",
                [317.0, 133.0, 93.0, 28.0],
                SIZES.iter().map(|s| (*s).to_owned()).collect(),
                On::Always,
            ),
            Ctl::Swatch {
                id: "color",
                r: [418.0, 132.0, 31.0, 31.0],
            },
            Ctl::Icon {
                id: "tstyle",
                r: [457.0, 132.0, 33.0, 31.0],
                icon: IconKind::TextStyle,
                on: On::Always,
            },
            link("appearance", 16.0, 204.0, t("pdfEditor8.appearanceOptions")),
            group(t("pdfEditor8.marginInches"), [637.0, 92.0, 351.0, 95.0]),
            label(649.0, 127.0, t("pdfEditor8.top")),
            spin(
                "top",
                [684.0, 113.0, 113.0, 28.0],
                0.1,
                (0.0, 20.0),
                On::Always,
            ),
            label(805.0, 127.0, t("pdfEditor8.bottom")),
            spin(
                "bottom",
                [864.0, 113.0, 113.0, 28.0],
                0.1,
                (0.0, 20.0),
                On::Always,
            ),
            label(649.0, 162.0, t("pdfEditor8.left")),
            spin(
                "left",
                [684.0, 148.0, 113.0, 28.0],
                0.1,
                (0.0, 20.0),
                On::Always,
            ),
            label(805.0, 162.0, t("pdfEditor8.right")),
            spin(
                "right",
                [864.0, 148.0, 113.0, 28.0],
                0.1,
                (0.0, 20.0),
                On::Always,
            ),
        ];
        let boxes = [
            ("hl", "pdfEditor8.leftHeader", 17.0, 230.0),
            ("hc", "pdfEditor8.centerHeader", 342.0, 230.0),
            ("hr", "pdfEditor8.rightHeader", 668.0, 230.0),
            ("fl", "pdfEditor8.leftFooter", 17.0, 324.0),
            ("fc", "pdfEditor8.centerFooter", 342.0, 324.0),
            ("fr", "pdfEditor8.rightFooter", 668.0, 324.0),
        ];
        for (id, key, x, y) in boxes {
            c.push(label(x - 1.0, y, t(key)));
            c.push(Ctl::Edit {
                id,
                r: [x, y + 17.0, 319.0, 60.0],
                multi: true,
                on: On::Always,
            });
        }
        c.extend([
            check("preview", 17.0, 422.0, t("pdfEditor8.showPreview")),
            label(151.0, 423.0, t("pdfEditor8.macros")),
            combo(
                "macro",
                [208.0, 409.0, 127.0, 28.0],
                vec![
                    t("pdfEditor8.macroPage"),
                    t("pdfEditor8.macroPageOf"),
                    t("pdfEditor8.macroDate"),
                ],
                On::Always,
            ),
            button(
                "insert",
                [343.0, 409.0, 111.0, 28.0],
                t("pdfEditor8.insert"),
                On::Always,
            ),
            link("numfmt", 462.0, 423.0, t("pdfEditor8.numberDateFormat")),
            link("range", 756.0, 423.0, t("pdfEditor8.pageRangeOptions")),
            group(t("pdfEditor8.preview"), [17.0, 454.0, 971.0, 312.0]),
            label(28.0, 490.0, t("pdfEditor8.previewPage")),
            spin(
                "ppage",
                [125.0, 476.0, 92.0, 28.0],
                1.0,
                (1.0, self.page_total().max(1) as f32),
                On::Always,
            ),
            Ctl::Frame {
                r: [28.0, 511.0, 946.0, 243.0],
            },
        ]);
        c.extend(self.ok_cancel(776.0, 796.0));
        let f = Form::new(t("pdfEditor8.hfTitle"), (1004.0, 820.0), c)
            .with("size", Val::Pick(3))
            .with("color", Val::Color([0, 0, 0]))
            .with("top", Val::Text("0.50".into()))
            .with("bottom", Val::Text("0.50".into()))
            .with("left", Val::Text("1.00".into()))
            .with("right", Val::Text("1.00".into()))
            .with("preview", Val::Pick(1))
            .with("ppage", Val::Text("1".into()));
        self.open_form(f, FormWhat::HeaderFooter);
    }

    fn header_footer_ok(&mut self, f: &Form) {
        let size: f32 = SIZES
            .get(f.pick("size"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(9.0);
        let one = |id: &str| f.text(id).replace('\n', " ");
        let m = PageMarks {
            header: [one("hl"), one("hc"), one("hr")],
            footer: [one("fl"), one("fc"), one("fr")],
            margin: f.num("top", 0.5) * 72.0 + size,
            bottom: f.num("bottom", 0.5) * 72.0 + size,
            side: f32::midpoint(f.num("left", 1.0), f.num("right", 1.0)) * 72.0,
            size,
            color: f.color("color"),
            date: crate::editor_dlg8b::date_text(f.pick("datefmt")),
            offset: (f.num("offx", 0.0), f.num("offy", 0.0)),
            page_start: f.num("start", 1.0) as i32,
            underline: f.pick("under") == 1,
            ..PageMarks::default()
        };
        if m.header.iter().chain(&m.footer).all(String::is_empty) {
            return;
        }
        let pages = self.range_pages(f);
        self.apply_marks_on(&pages, &m);
    }

    fn apply_page_marks(&mut self, m: &PageMarks) {
        self.apply_marks_on(&[], m);
    }

    /// Marks on `pages` (all when empty).
    fn apply_marks_on(&mut self, pages: &[usize], m: &PageMarks) {
        match self.doc_edit(|d| d.add_page_marks(pages, m)) {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }

    // --- Watermark / Background -------------------------------------------------------

    fn position_ctls(&self, top: f32, x: (f32, f32, f32, f32), c: &mut Vec<Ctl>) {
        let t = |k: &str| self.t(k);
        let units: Vec<String> = UNITS.iter().map(|u| t(u.0)).collect();
        let (lab_end, sp, un, from) = x;
        for (k, (id, uid, fid, key, places)) in [
            (
                "vdist",
                "vunit",
                "vfrom",
                "pdfEditor8.verticalDistance",
                ["pdfEditor8.top2", "pdfEditor8.center", "pdfEditor8.bottom2"],
            ),
            (
                "hdist",
                "hunit",
                "hfrom",
                "pdfEditor8.horizontalDistance",
                ["pdfEditor8.left2", "pdfEditor8.center", "pdfEditor8.right2"],
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let y = top + 36.0 * k as f32;
            c.push(rlabel(lab_end, y, t(key), On::Always));
            c.push(spin(
                id,
                [sp, y - 14.0, un - sp - 8.0, 28.0],
                0.1,
                (-100.0, 100.0),
                On::Always,
            ));
            c.push(combo(
                uid,
                [un, y - 14.0, 99.0, 28.0],
                units.clone(),
                On::Always,
            ));
            c.push(label(from, y, t("pdfEditor8.from")));
            c.push(combo(
                fid,
                [from + 43.0, y - 14.0, 82.0, 28.0],
                places.iter().map(|p| t(p)).collect(),
                On::Always,
            ));
        }
    }

    fn open_watermark(&mut self) {
        let t = |k: &str| self.t(k);
        let mut c = vec![
            label(16.0, 60.0, t("pdfEditor8.savedSettings")),
            combo(
                "saved",
                [123.0, 46.0, 272.0, 28.0],
                self.preset_names("wm"),
                On::Always,
            ),
            Ctl::Icon {
                id: "save",
                r: [403.0, 45.0, 32.0, 30.0],
                icon: IconKind::SavePreset,
                on: On::Always,
            },
            Ctl::Icon {
                id: "load",
                r: [441.0, 45.0, 32.0, 30.0],
                icon: IconKind::LoadPreset,
                on: On::Always,
            },
            Ctl::Icon {
                id: "delete",
                r: [479.0, 45.0, 32.0, 30.0],
                icon: IconKind::DeletePreset,
                on: On::Always,
            },
            button(
                "save",
                [517.0, 46.0, 115.0, 28.0],
                t("pdfEditor8.saveSettings"),
                On::Always,
            ),
            group(t("pdfEditor8.source"), [17.0, 93.0, 616.0, 228.0]),
            radio("src", 0, 28.0, 158.0, t("pdfEditor8.textColon")),
            Ctl::Edit {
                id: "text",
                r: [87.0, 115.0, 534.0, 87.0],
                multi: true,
                on: On::If("src", 0),
            },
            rlabel(120.0, 225.0, t("pdfEditor8.fontColon"), On::If("src", 0)),
            combo(
                "font",
                [142.0, 211.0, 199.0, 28.0],
                vec!["Helvetica".into()],
                On::If("src", 0),
            ),
            rlabel(398.0, 225.0, t("pdfEditor8.size"), On::If("src", 0)),
            combo(
                "size",
                [403.0, 211.0, 103.0, 28.0],
                std::iter::once(t("pdfEditor8.auto"))
                    .chain(SIZES.iter().map(|s| (*s).to_owned()))
                    .collect(),
                On::If("src", 0),
            ),
            Ctl::Swatch {
                id: "color",
                r: [515.0, 210.0, 30.0, 30.0],
            },
            Ctl::Icon {
                id: "tstyle",
                r: [552.0, 209.0, 32.0, 32.0],
                icon: IconKind::TextStyle,
                on: On::If("src", 0),
            },
            Ctl::Icon {
                id: "dynamic",
                r: [589.0, 209.0, 32.0, 32.0],
                icon: IconKind::Dynamic,
                on: On::Never,
            },
            radio("src", 1, 28.0, 262.0, t("pdfEditor8.file")),
            edit("file", [87.0, 248.0, 254.0, 28.0], On::If("src", 1)),
            button(
                "browse",
                [349.0, 248.0, 157.0, 28.0],
                t("pdfEditor8.browse"),
                On::If("src", 1),
            ),
            rlabel(180.0, 298.0, t("pdfEditor8.pageNumber"), On::Never),
            spin(
                "fpage",
                [197.0, 284.0, 143.0, 28.0],
                1.0,
                (1.0, 1.0),
                On::Never,
            ),
            rlabel(
                453.0,
                298.0,
                t("pdfEditor8.absoluteScale"),
                On::If("src", 1),
            ),
            spin(
                "fscale",
                [459.0, 284.0, 123.0, 28.0],
                10.0,
                (1.0, 1000.0),
                On::If("src", 1),
            ),
            group(t("pdfEditor8.appearance"), [17.0, 341.0, 616.0, 258.0]),
            label(28.0, 377.0, t("pdfEditor8.rotation")),
            radio("rot", 0, 147.0, 377.0, "-45°".into()),
            radio("rot", 1, 241.0, 377.0, t("pdfEditor8.none")),
            radio("rot", 2, 333.0, 377.0, "45°".into()),
            radio("rot", 3, 428.0, 377.0, t("pdfEditor8.custom")),
            spin(
                "angle",
                [521.0, 363.0, 99.0, 28.0],
                1.0,
                (-360.0, 360.0),
                On::If("rot", 3),
            ),
            label(28.0, 412.0, t("pdfEditor8.opacity")),
            Ctl::Slider {
                id: "opacity",
                r: [155.0, 400.0, 268.0, 24.0],
            },
            spin(
                "opacity",
                [439.0, 398.0, 74.0, 28.0],
                1.0,
                (0.0, 100.0),
                On::Always,
            ),
            check("scaleOn", 28.0, 447.0, t("pdfEditor8.scaleRelative")),
            spin(
                "scale",
                [357.0, 433.0, 74.0, 28.0],
                1.0,
                (1.0, 400.0),
                On::If("scaleOn", 1),
            ),
            check("tile", 28.0, 480.0, t("pdfEditor8.tiling")),
            label_on(28.0, 514.0, t("pdfEditor8.rowSpacing"), On::If("tile", 1)),
            spin(
                "rows",
                [197.0, 500.0, 101.0, 28.0],
                0.1,
                (0.0, 20.0),
                On::If("tile", 1),
            ),
            label_on(
                357.0,
                514.0,
                t("pdfEditor8.columnSpacing"),
                On::If("tile", 1),
            ),
            spin(
                "cols",
                [520.0, 500.0, 101.0, 28.0],
                0.1,
                (0.0, 20.0),
                On::If("tile", 1),
            ),
            label(28.0, 547.0, t("pdfEditor8.location")),
            radio("loc", 0, 147.0, 547.0, t("pdfEditor8.behind")),
            radio("loc", 1, 388.0, 547.0, t("pdfEditor8.onTop")),
            link("appearance", 28.0, 577.0, t("pdfEditor8.appearanceOptions")),
            group(t("pdfEditor8.position"), [17.0, 619.0, 616.0, 96.0]),
        ];
        self.position_ctls(654.0, (166.0, 173.0, 320.0, 482.0), &mut c);
        c.extend([
            link("range", 16.0, 737.0, t("pdfEditor8.pageRangeOptions")),
            check("preview", 17.0, 770.0, t("pdfEditor8.showPreview")),
            group(t("pdfEditor8.preview"), [640.0, 94.0, 437.0, 654.0]),
            label(652.0, 130.0, t("pdfEditor8.previewPage")),
            spin(
                "ppage",
                [750.0, 116.0, 92.0, 28.0],
                1.0,
                (1.0, self.page_total().max(1) as f32),
                On::Always,
            ),
            Ctl::Frame {
                r: [652.0, 152.0, 412.0, 582.0],
            },
        ]);
        c.extend(self.ok_cancel(758.0, 885.0));
        let f = Form::new(t("pdfEditor8.wmTitle"), (1093.0, 802.0), c)
            .with("rot", Val::Pick(1))
            .with("angle", Val::Text("0°".into()))
            .with("opacity", Val::Text("50%".into()))
            .with("scaleOn", Val::Pick(1))
            .with("scale", Val::Text("50%".into()))
            .with("rows", Val::Text("2.0".into()))
            .with("cols", Val::Text("2.0".into()))
            .with("fpage", Val::Text("1".into()))
            .with("fscale", Val::Text("100%".into()))
            .with("loc", Val::Pick(1))
            .with("vdist", Val::Text("0.00".into()))
            .with("hdist", Val::Text("0.00".into()))
            .with("vfrom", Val::Pick(1))
            .with("hfrom", Val::Pick(1))
            .with("preview", Val::Pick(1))
            .with("ppage", Val::Text("1".into()))
            .with("color", Val::Color([0, 0, 0]));
        self.open_form(f, FormWhat::Watermark);
    }

    /// Watermark / background centre offset from the page centre (points, right / up).
    fn offset(&self, f: &Form) -> (f32, f32) {
        let (pw, ph) = self
            .r
            .view()
            .and_then(|v| v.doc().page_info(0).map(|p| (p.width, p.height)))
            .unwrap_or((595.0, 842.0));
        let pt = |d: &str, u: &str| f.num(d, 0.0) * UNITS.get(f.pick(u)).map_or(72.0, |u| u.1);
        let dy = pt("vdist", "vunit");
        let dx = pt("hdist", "hunit");
        let y = match f.pick("vfrom") {
            0 => ph / 2.0 - dy,
            2 => -ph / 2.0 + dy,
            _ => -dy,
        };
        let x = match f.pick("hfrom") {
            0 => -pw / 2.0 + dx,
            2 => pw / 2.0 - dx,
            _ => dx,
        };
        // Top / Left / Right distances are to the mark's edge; keep it simple and centre it.
        (x, y)
    }

    fn watermark_ok(&mut self, f: &Form) {
        if f.pick("src") == 1 {
            self.image_watermark(f);
            return;
        }
        let text = f.text("text").replace('\n', " ");
        if text.trim().is_empty() {
            return;
        }
        let angle = match f.pick("rot") {
            0 => -45.0,
            2 => 45.0,
            3 => f.num("angle", 0.0),
            _ => 0.0,
        };
        let size = f
            .pick("size")
            .checked_sub(1)
            .and_then(|i| SIZES.get(i))
            .and_then(|s| s.parse().ok())
            .unwrap_or(72.0);
        let fit = if f.pick("scaleOn") == 1 || f.pick("size") == 0 {
            f.num("scale", 50.0) / 100.0
        } else {
            0.0
        };
        let m = PageMarks {
            watermark: text,
            watermark_size: size,
            watermark_fit: fit,
            watermark_color: f.color("color"),
            watermark_opacity: (f.num("opacity", 50.0) / 100.0).clamp(0.05, 1.0),
            watermark_angle: angle,
            watermark_behind: f.pick("loc") == 0,
            watermark_offset: self.offset(f),
            watermark_tile: (f.pick("tile") == 1)
                .then(|| (f.num("rows", 2.0) * 72.0, f.num("cols", 2.0) * 72.0)),
            underline: f.pick("under") == 1,
            date: today(),
            ..PageMarks::default()
        };
        let pages = self.range_pages(f);
        self.apply_marks_on(&pages, &m);
    }

    fn open_background(&mut self) {
        let t = |k: &str| self.t(k);
        let mut c = vec![
            label(16.0, 60.0, t("pdfEditor8.savedSettings")),
            combo(
                "saved",
                [123.0, 46.0, 148.0, 28.0],
                self.preset_names("bg"),
                On::Always,
            ),
            button(
                "delete",
                [281.0, 46.0, 91.0, 28.0],
                t("pdfEditor8.delete"),
                On::Always,
            ),
            button(
                "save",
                [381.0, 46.0, 115.0, 28.0],
                t("pdfEditor8.saveSettings"),
                On::Always,
            ),
            group(t("pdfEditor8.source"), [17.0, 92.0, 480.0, 145.0]),
            radio("src", 0, 28.0, 134.0, t("pdfEditor8.fromColor")),
            Ctl::Swatch {
                id: "color",
                r: [147.0, 120.0, 28.0, 28.0],
            },
            Ctl::Radio {
                id: "src",
                k: 1,
                x: 28.0,
                y: 170.0,
                label: t("pdfEditor8.file"),
                on: On::Never,
            },
            edit("file", [147.0, 156.0, 230.0, 28.0], On::Never),
            button(
                "browse",
                [385.0, 156.0, 99.0, 28.0],
                t("pdfEditor8.browse"),
                On::Never,
            ),
            label_on(28.0, 206.0, t("pdfEditor8.pageNumber"), On::Never),
            spin(
                "fpage",
                [147.0, 192.0, 100.0, 28.0],
                1.0,
                (1.0, 1.0),
                On::Never,
            ),
            label_on(277.0, 206.0, t("pdfEditor8.absoluteScale"), On::Never),
            spin(
                "fscale",
                [385.0, 192.0, 99.0, 28.0],
                10.0,
                (1.0, 1000.0),
                On::Never,
            ),
            group(t("pdfEditor8.appearance"), [17.0, 255.0, 480.0, 185.0]),
            label(28.0, 291.0, t("pdfEditor8.rotation")),
            spin(
                "angle",
                [147.0, 277.0, 99.0, 28.0],
                1.0,
                (-360.0, 360.0),
                On::Always,
            ),
            label(28.0, 326.0, t("pdfEditor8.opacity")),
            Ctl::Slider {
                id: "opacity",
                r: [155.0, 314.0, 208.0, 24.0],
            },
            spin(
                "opacity",
                [385.0, 312.0, 99.0, 28.0],
                1.0,
                (0.0, 100.0),
                On::Always,
            ),
            check("scaleOn", 28.0, 361.0, t("pdfEditor8.scaleRelative")),
            spin(
                "scale",
                [277.0, 347.0, 99.0, 28.0],
                1.0,
                (1.0, 400.0),
                On::If("scaleOn", 1),
            ),
            group(t("pdfEditor8.position"), [17.0, 458.0, 480.0, 105.0]),
        ];
        self.position_ctls(498.0, (162.0, 169.0, 252.0, 362.0), &mut c);
        c.extend([
            link("range", 16.0, 611.0, t("pdfEditor8.pageRangeOptions")),
            check("preview", 17.0, 671.0, t("pdfEditor8.showPreview")),
            group(t("pdfEditor8.preview"), [505.0, 92.0, 436.0, 555.0]),
            label(516.0, 128.0, t("pdfEditor8.previewPage")),
            spin(
                "ppage",
                [613.0, 114.0, 92.0, 28.0],
                1.0,
                (1.0, self.page_total().max(1) as f32),
                On::Always,
            ),
            Ctl::Frame {
                r: [549.0, 149.0, 345.0, 487.0],
            },
        ]);
        c.extend(self.ok_cancel(658.0, 749.0));
        let f = Form::new(t("pdfEditor8.bgTitle"), (957.0, 703.0), c)
            .with("color", Val::Color([0xFF, 0xFF, 0xFF]))
            .with("angle", Val::Text("0°".into()))
            .with("opacity", Val::Text("100%".into()))
            .with("scaleOn", Val::Pick(1))
            .with("scale", Val::Text("100%".into()))
            .with("fpage", Val::Text("1".into()))
            .with("fscale", Val::Text("100%".into()))
            .with("vdist", Val::Text("0.00".into()))
            .with("hdist", Val::Text("0.00".into()))
            .with("vfrom", Val::Pick(1))
            .with("hfrom", Val::Pick(1))
            .with("preview", Val::Pick(1))
            .with("ppage", Val::Text("1".into()));
        self.open_form(f, FormWhat::Background);
    }

    fn background_ok(&mut self, f: &Form) {
        let op = f.num("opacity", 100.0) / 100.0;
        let m = PageMarks {
            background: Some(f.color("color")),
            background_opacity: if op >= 0.999 { 0.0 } else { op.max(0.05) },
            ..PageMarks::default()
        };
        let pages = self.range_pages(f);
        self.apply_marks_on(&pages, &m);
    }

    // --- Bates Numbering ---------------------------------------------------------------

    fn open_bates_files(&mut self) {
        let t = |k: &str| self.t(k);
        let (size, path) = self.r.view().map_or((0, String::new()), |v| {
            (
                v.doc().bytes().len(),
                self.r
                    .path()
                    .and_then(|p| p.parent())
                    .map_or_else(String::new, |p| p.display().to_string()),
            )
        });
        let name = self
            .r
            .path()
            .and_then(|p| p.file_name())
            .map_or_else(|| self.doc_name(), |n| n.to_string_lossy().into_owned());
        let mut c = vec![
            button(
                "add",
                [17.0, 46.0, 91.0, 28.0],
                t("pdfEditor8.addFiles"),
                On::Never,
            ),
            rlabel(917.0, 60.0, t("pdfEditor8.totalOneFile"), On::Always),
            group(t("pdfEditor8.arrangeFiles"), [17.0, 92.0, 899.0, 384.0]),
            Ctl::Table {
                r: [28.0, 114.0, 877.0, 316.0],
                cols: vec![
                    (t("pdfEditor8.colName"), 218.0),
                    (t("pdfEditor8.colLocation"), 218.0),
                    (t("pdfEditor8.colSize"), 218.0),
                    (t("pdfEditor8.colModified"), 218.0),
                ],
                rows: vec![vec![
                    name,
                    path,
                    format!("{:.1} KB", size as f32 / 1024.0),
                    String::new(),
                ]],
            },
            button(
                "up",
                [28.0, 438.0, 91.0, 28.0],
                t("pdfEditor8.moveUp"),
                On::Never,
            ),
            button(
                "down",
                [128.0, 438.0, 100.0, 28.0],
                t("pdfEditor8.moveDown"),
                On::Never,
            ),
            button(
                "remove",
                [236.0, 438.0, 91.0, 28.0],
                t("pdfEditor8.remove"),
                On::Never,
            ),
            button(
                "output",
                [17.0, 487.0, 128.0, 28.0],
                t("pdfEditor8.outputOptions"),
                On::Never,
            ),
        ];
        c.extend(self.ok_cancel(487.0, 726.0));
        let f = Form::new(t("pdfEditor8.batesTitle"), (933.0, 532.0), c);
        self.open_form(f, FormWhat::BatesFiles);
    }

    /// The numbering of Bates Numbering (laid out in the same classic style).
    fn open_bates_options(&mut self) {
        let t = |k: &str| self.t(k);
        let mut c = vec![
            group(t("pdfEditor8.batesOptions"), [17.0, 55.0, 410.0, 175.0]),
            rlabel(190.0, 92.0, t("pdfEditor8.digits"), On::Always),
            spin(
                "digits",
                [200.0, 78.0, 110.0, 28.0],
                1.0,
                (3.0, 15.0),
                On::Always,
            ),
            rlabel(190.0, 127.0, t("pdfEditor8.startNumber"), On::Always),
            spin(
                "start",
                [200.0, 113.0, 110.0, 28.0],
                1.0,
                (1.0, 999_999.0),
                On::Always,
            ),
            rlabel(190.0, 162.0, t("pdfEditor8.prefix"), On::Always),
            edit("prefix", [200.0, 148.0, 210.0, 28.0], On::Always),
            rlabel(190.0, 197.0, t("pdfEditor8.suffix"), On::Always),
            edit("suffix", [200.0, 183.0, 210.0, 28.0], On::Always),
        ];
        c.extend(self.ok_cancel(246.0, 236.0));
        let f = Form::new(t("pdfEditor8.batesTitle"), (444.0, 290.0), c)
            .with("digits", Val::Text("6".into()))
            .with("start", Val::Text("1".into()));
        self.open_form(f, FormWhat::BatesOptions);
    }

    fn bates_ok(&mut self, f: &Form) {
        let m = PageMarks {
            bates: Some((
                f.text("prefix"),
                f.num("start", 1.0).max(0.0) as u32,
                f.num("digits", 6.0).clamp(1.0, 15.0) as usize,
                f.text("suffix"),
            )),
            margin: 28.0,
            size: 10.0,
            date: today(),
            ..PageMarks::default()
        };
        self.apply_page_marks(&m);
    }

    // --- Password Protection ---------------------------------------------------------

    fn open_password(&mut self) {
        let t = |k: &str| self.t(k);
        let (p, cp, m, a) = self.perm8;
        let yes = |b: bool| {
            t(if b {
                "pdfEditor8.allowed"
            } else {
                "pdfEditor8.notAllowed"
            })
        };
        let mut c = vec![
            group(t("pdfEditor8.openSettings"), [17.0, 55.0, 675.0, 95.0]),
            check("openOn", 28.0, 90.0, t("pdfEditor8.requireOpen")),
            label_on(65.0, 125.0, t("pdfEditor8.passwordInput"), On::Always),
            edit("open", [179.0, 111.0, 130.0, 28.0], On::Always),
            label_on(417.0, 125.0, t("pdfEditor8.passwordConfirm"), On::Always),
            edit("open2", [549.0, 111.0, 131.0, 28.0], On::Always),
            group(
                t("pdfEditor8.restrictSettings"),
                [17.0, 169.0, 675.0, 274.0],
            ),
            check("restrictOn", 28.0, 203.0, t("pdfEditor8.addRestriction")),
            label_on(65.0, 237.0, t("pdfEditor8.passwordInput"), On::Always),
            edit("owner", [179.0, 223.0, 130.0, 28.0], On::Always),
            label_on(417.0, 237.0, t("pdfEditor8.passwordConfirm"), On::Always),
            edit("owner2", [549.0, 223.0, 131.0, 28.0], On::Always),
            group(t("pdfEditor8.permissionSpec"), [28.0, 268.0, 544.0, 163.0]),
            rlabel(
                125.0,
                300.0,
                t("pdfEditor8.printing"),
                On::If("restrictOn", 1),
            ),
            label_on(
                131.0,
                300.0,
                t(if p {
                    "pdfEditor8.highResolution"
                } else {
                    "pdfEditor8.notAllowed"
                }),
                On::If("restrictOn", 1),
            ),
            rlabel(
                125.0,
                327.0,
                t("pdfEditor8.accessibility"),
                On::If("restrictOn", 1),
            ),
            label_on(
                131.0,
                327.0,
                t("pdfEditor8.allowed"),
                On::If("restrictOn", 1),
            ),
            rlabel(
                125.0,
                354.0,
                t("pdfEditor8.copying"),
                On::If("restrictOn", 1),
            ),
            label_on(131.0, 354.0, yes(cp), On::If("restrictOn", 1)),
            rlabel(
                125.0,
                381.0,
                t("pdfEditor8.changes"),
                On::If("restrictOn", 1),
            ),
            label_on(
                131.0,
                381.0,
                if m {
                    t("pdfEditor8.anyChanges")
                } else if a {
                    t("pdfEditor8.commentsOnly")
                } else {
                    t("pdfEditor8.none")
                },
                On::If("restrictOn", 1),
            ),
            button(
                "perm",
                [580.0, 331.0, 100.0, 28.0],
                t("pdfEditor8.permission"),
                On::If("restrictOn", 1),
            ),
            group(t("pdfEditor8.encryptSettings"), [17.0, 463.0, 675.0, 94.0]),
            label(28.0, 498.0, t("pdfEditor8.algorithm")),
            radio("alg", 0, 192.0, 498.0, t("pdfEditor8.aes128")),
            radio("alg", 1, 357.0, 498.0, t("pdfEditor8.aes256")),
            radio("alg", 2, 521.0, 498.0, t("pdfEditor8.rc4")),
            Ctl::Check {
                id: "meta",
                x: 28.0,
                y: 532.0,
                label: t("pdfEditor8.noMetadata"),
                on: On::IfNot("alg", 2),
            },
            check("policy", 17.0, 581.0, t("pdfEditor8.savePolicy")),
        ];
        c.push(button(
            "ok",
            [500.0, 567.0, 91.0, 28.0],
            t("pdfReader.ok"),
            On::Either("openOn", 1, "restrictOn", 1),
        ));
        c.push(button(
            "cancel",
            [600.0, 567.0, 91.0, 28.0],
            t("pdfReader.cancel"),
            On::Always,
        ));
        let mut f = Form::new(t("pdfEditor8.pwTitle"), (708.0, 612.0), c).with("alg", Val::Pick(0));
        f.ticks = vec![
            ("open", "openOn"),
            ("open2", "openOn"),
            ("owner", "restrictOn"),
            ("owner2", "restrictOn"),
        ];
        if let Some(v) = self.policy8.clone() {
            f.vals.extend(v);
        }
        self.open_form(f, FormWhat::Password);
    }

    fn open_permissions(&mut self) {
        let t = |k: &str| self.t(k);
        let (p, c0, m, a) = self.perm8;
        let mut c = vec![
            group(t("pdfEditor8.permissionSpec"), [17.0, 55.0, 410.0, 175.0]),
            check("print", 28.0, 90.0, t("pdfEditor.allowPrint")),
            check("copy", 28.0, 124.0, t("pdfEditor.allowCopy")),
            check("modify", 28.0, 158.0, t("pdfEditor.allowModify")),
            check("annot", 28.0, 192.0, t("pdfEditor.allowComments")),
        ];
        c.extend(self.ok_cancel(246.0, 236.0));
        let b = |v: bool| Val::Pick(usize::from(v));
        let f = Form::new(t("pdfEditor8.permTitle"), (444.0, 290.0), c)
            .with("print", b(p))
            .with("copy", b(c0))
            .with("modify", b(m))
            .with("annot", b(a));
        self.open_form(f, FormWhat::Permissions);
    }

    fn password_ok(&mut self, f: &Form) -> bool {
        let user = if f.pick("openOn") == 1 {
            f.text("open")
        } else {
            String::new()
        };
        let owner = if f.pick("restrictOn") == 1 {
            f.text("owner")
        } else {
            String::new()
        };
        if (f.pick("openOn") == 1 && (user.is_empty() || user != f.text("open2")))
            || (f.pick("restrictOn") == 1 && (owner.is_empty() || owner != f.text("owner2")))
        {
            let msg = self.t("pdfEditor8.passwordsDiffer");
            self.set_status(&msg);
            return false;
        }
        let (print, copy, modify, annotate) = if f.pick("restrictOn") == 1 {
            self.perm8
        } else {
            (true, true, true, true)
        };
        let owner = if owner.is_empty() {
            user.clone()
        } else {
            owner
        };
        let allow = bo_pdfread::Permissions {
            print,
            copy,
            modify,
            annotate,
            // "Any except extracting pages": page assembly stays reserved.
            assemble: false,
        };
        let cipher = match f.pick("alg") {
            1 => bo_pdfread::Cipher::Aes256,
            2 => bo_pdfread::Cipher::Rc4,
            _ => bo_pdfread::Cipher::Aes128,
        };
        let meta = f.pick("meta") != 1;
        if f.pick("policy") == 1 {
            self.keep_policy(f);
        }
        match self.doc_edit(|d| d.set_protection_with(&user, &owner, allow, cipher, meta)) {
            Ok(()) => {
                self.after_doc_edit();
                let msg = self.t("pdfEditor.securityAdded");
                self.set_status(&msg);
            }
            Err(e) => self.set_status(&e),
        }
        true
    }

    // --- Insert ------------------------------------------------------------------------

    fn open_insert(&mut self, file: Option<(String, Vec<u8>)>) {
        let t = |k: &str| self.t(k);
        let n = self.page_total().max(1);
        let cur = self.insert_at().max(1);
        let total = n.to_string();
        let mut c = vec![
            group(t("pdfEditor8.destination"), [17.0, 55.0, 445.0, 225.0]),
            radio("where", 0, 28.0, 107.0, t("pdfEditor8.beforePage")),
            spin(
                "before",
                [152.0, 93.0, 63.0, 28.0],
                1.0,
                (1.0, n as f32),
                On::If("where", 0),
            ),
            label(243.0, 107.0, t("pdfEditor8.of")),
            label(302.0, 107.0, total.clone()),
            radio("where", 1, 28.0, 153.0, t("pdfEditor8.afterPage")),
            spin(
                "after",
                [152.0, 139.0, 63.0, 28.0],
                1.0,
                (1.0, n as f32),
                On::If("where", 1),
            ),
            label(243.0, 153.0, t("pdfEditor8.of")),
            label(302.0, 153.0, total),
            radio("where", 2, 28.0, 197.0, t("pdfEditor8.beginning")),
            radio("where", 3, 28.0, 241.0, t("pdfEditor8.end")),
        ];
        let title = if file.is_some() {
            t("pdfEditor8.insertPagesTitle")
        } else {
            c.push(label(16.0, 302.0, t("pdfEditor8.pagesCount")));
            c.push(spin(
                "count",
                [147.0, 288.0, 98.0, 28.0],
                1.0,
                (1.0, 999.0),
                On::Always,
            ));
            t("pdfEditor8.insertBlankTitle")
        };
        c.extend(self.ok_cancel(326.0, 271.0));
        let f = Form::new(title, (479.0, 370.0), c)
            .with("where", Val::Pick(1))
            .with("before", Val::Text(cur.to_string()))
            .with("after", Val::Text(cur.to_string()))
            .with("count", Val::Text("1".into()));
        self.open_form(f, FormWhat::Insert(file));
    }

    fn insert_ok(&mut self, f: &Form, file: Option<(String, Vec<u8>)>) {
        let n = self.page_total();
        let at = match f.pick("where") {
            0 => (f.num("before", 1.0) as usize).saturating_sub(1),
            1 => f.num("after", 1.0) as usize,
            2 => 0,
            _ => n,
        }
        .min(n);
        let Some(v) = self.r.view_mut() else { return };
        if let Some((name, bytes)) = file {
            match v.doc_mut().insert_pdf(at, &bytes, None) {
                Ok(k) => {
                    self.after_edit();
                    self.org.select_range(at, at + k.saturating_sub(1));
                    self.r.set_status(&format!("{name}: +{k}"));
                }
                Err(e) => self.r.set_status(&e),
            }
            return;
        }
        let (w, h) = v
            .doc()
            .page_info(at.saturating_sub(1).min(n.saturating_sub(1)))
            .map_or((595.0, 842.0), |p| (p.width, p.height));
        let count = (f.num("count", 1.0) as usize).clamp(1, 999);
        let mut r = Ok(());
        for _ in 0..count {
            r = v.doc_mut().insert_blank_page(at, w, h);
            if r.is_err() {
                break;
            }
        }
        self.edit_result(r);
        self.org.select_range(at, at + count - 1);
    }

    // --- Extract -------------------------------------------------------------------------

    fn open_extract(&mut self) {
        let t = |k: &str| self.t(k);
        let n = self.page_total();
        let sel = self.target_pages();
        let range = if self.organize_on() && !self.org.selected.is_empty() {
            crate::editor_dlg8::ranges_label(&sel)
        } else {
            String::new()
        };
        let mut c = vec![
            group(t("pdfEditor8.pageRange"), [17.0, 55.0, 410.0, 180.0]),
            radio("range", 0, 28.0, 90.0, t("pdfEditor8.allPages")),
            radio("range", 1, 28.0, 125.0, t("pdfEditor8.pagesColon")),
            edit("pages", [115.0, 110.0, 185.0, 30.0], On::If("range", 1)),
            label(345.0, 125.0, t("pdfEditor8.of")),
            label(383.0, 125.0, n.to_string()),
            label(115.0, 158.0, t("pdfEditor8.sample")),
            Ctl::Info { x: 257.0, y: 157.0 },
            rlabel(107.0, 206.0, t("pdfEditor8.extractColon"), On::Always),
            combo(
                "which",
                [112.0, 192.0, 188.0, 28.0],
                vec![
                    t("pdfEditor8.allInRange"),
                    t("pdfEditor8.evenPages"),
                    t("pdfEditor8.oddPages"),
                ],
                On::Always,
            ),
            group(t("pdfEditor8.extractMode"), [17.0, 253.0, 410.0, 115.0]),
            radio("mode", 0, 28.0, 291.0, t("pdfEditor8.asOne")),
            radio("mode", 1, 28.0, 320.0, t("pdfEditor8.byComma")),
            radio("mode", 2, 28.0, 349.0, t("pdfEditor8.eachPage")),
            check("save", 17.0, 391.0, t("pdfEditor8.saveNow")),
            check("delete", 17.0, 425.0, t("pdfEditor8.deleteAfter")),
            Ctl::Check {
                id: "preview",
                x: 17.0,
                y: 459.0,
                label: t("pdfEditor8.showPreview"),
                on: On::Never,
            },
        ];
        c.extend(self.ok_cancel(446.0, 236.0));
        let f = Form::new(t("pdfEditor8.extractTitle"), (444.0, 491.0), c)
            .with("range", Val::Pick(0))
            .with("pages", Val::Text(range));
        self.open_form(f, FormWhat::Extract);
    }

    fn extract_ok(&mut self, f: &Form) {
        let n = self.page_total();
        let groups: Vec<Vec<usize>> = if f.pick("range") == 0 {
            vec![(0..n).collect()]
        } else {
            page_groups(&f.text("pages"), n)
        };
        let keep = |p: &usize| match f.pick("which") {
            1 => p % 2 == 1,
            2 => p % 2 == 0,
            _ => true,
        };
        let groups: Vec<Vec<usize>> = groups
            .into_iter()
            .map(|g| g.into_iter().filter(keep).collect::<Vec<_>>())
            .filter(|g| !g.is_empty())
            .collect();
        if groups.is_empty() {
            return;
        }
        let parts: Vec<Vec<usize>> = match f.pick("mode") {
            0 => vec![groups.concat()],
            1 => groups.clone(),
            _ => groups.concat().into_iter().map(|p| vec![p]).collect(),
        };
        let stem = self.doc_name();
        let Some(v) = self.r.view() else { return };
        let mut files = Vec::new();
        for pages in &parts {
            match v.doc().extract_pages(pages, SaveOptions::default()) {
                Ok(b) => files.push((
                    format!("{stem}-{}.pdf", crate::editor_dlg8::ranges_label(pages)),
                    b,
                )),
                Err(e) => {
                    self.r.set_status(&e);
                    return;
                }
            }
        }
        if f.pick("save") == 1 {
            if files.len() == 1 {
                if let Some((name, bytes)) = files.pop() {
                    self.push(ReaderRequest::SaveBytes { name, bytes });
                }
            } else {
                self.push(ReaderRequest::SaveMany(files));
            }
        } else {
            for (name, bytes) in files {
                self.push(ReaderRequest::OpenDocument { name, bytes });
            }
        }
        if f.pick("delete") == 1 {
            let mut all = groups.concat();
            all.sort_unstable();
            all.dedup();
            if all.len() < n {
                if let Some(v) = self.r.view_mut() {
                    let r = v.doc_mut().delete_pages(&all);
                    self.edit_result(r);
                }
            }
        }
    }

    // --- Split ---------------------------------------------------------------------------

    fn open_split(&mut self) {
        let t = |k: &str| self.t(k);
        let n = self.page_total();
        let mut c = vec![
            button(
                "more",
                [17.0, 46.0, 160.0, 28.0],
                t("pdfEditor8.selectMore"),
                On::Never,
            ),
            group(t("pdfEditor8.splitBy"), [17.0, 95.0, 460.0, 272.0]),
            radio("by", 0, 28.0, 128.0, t("pdfEditor8.numberOfPages")),
            label_on(229.0, 128.0, t("pdfEditor8.maxPages"), On::If("by", 0)),
            spin(
                "pages",
                [340.0, 114.0, 75.0, 28.0],
                1.0,
                (1.0, n.max(1) as f32),
                On::If("by", 0),
            ),
            radio("by", 1, 28.0, 172.0, t("pdfEditor8.fileSize")),
            label_on(229.0, 172.0, t("pdfEditor8.maxMb"), On::If("by", 1)),
            spin(
                "mb",
                [340.0, 158.0, 75.0, 28.0],
                0.1,
                (0.01, 10_000.0),
                On::If("by", 1),
            ),
            radio("by", 2, 28.0, 216.0, t("pdfEditor8.pageRangeSample")),
            Ctl::Info { x: 324.0, y: 216.0 },
            edit("range", [52.0, 245.0, 256.0, 30.0], On::If("by", 2)),
            label(327.0, 260.0, t("pdfEditor8.of")),
            label(384.0, 260.0, n.to_string()),
            radio("by", 3, 28.0, 304.0, t("pdfEditor8.topBookmarks")),
            radio("by", 4, 28.0, 348.0, t("pdfEditor8.blankPage")),
            button(
                "output",
                [17.0, 378.0, 128.0, 28.0],
                t("pdfEditor8.outputOptions"),
                On::Never,
            ),
        ];
        c.extend(self.ok_cancel(378.0, 286.0));
        let f = Form::new(t("pdfEditor8.splitTitle"), (494.0, 424.0), c)
            .with("pages", Val::Text("1".into()))
            .with("mb", Val::Text("1.00".into()))
            .focused("pages");
        self.open_form(f, FormWhat::Split);
    }

    /// Page groups of Split Document for the chosen way.
    pub(crate) fn split_groups(&mut self, f: &Form) -> Vec<Vec<usize>> {
        let n = self.page_total();
        match f.pick("by") {
            0 => {
                let every = (f.num("pages", 1.0) as usize).max(1);
                (0..n)
                    .step_by(every)
                    .map(|s| (s..(s + every).min(n)).collect())
                    .collect()
            }
            1 => {
                // Greedy: add pages while the part stays under the size (a page alone may not).
                let limit = (f.num("mb", 1.0) * 1_048_576.0) as usize;
                let Some(v) = self.r.view() else {
                    return Vec::new();
                };
                let mut out: Vec<Vec<usize>> = Vec::new();
                let mut cur: Vec<usize> = Vec::new();
                for p in 0..n {
                    cur.push(p);
                    let size = v
                        .doc()
                        .extract_pages(&cur, SaveOptions::default())
                        .map_or(0, |b| b.len());
                    if size > limit && cur.len() > 1 {
                        cur.pop();
                        out.push(std::mem::take(&mut cur));
                        cur.push(p);
                    }
                }
                if !cur.is_empty() {
                    out.push(cur);
                }
                out
            }
            2 => page_groups(&f.text("range"), n),
            3 => {
                let Some(v) = self.r.view() else {
                    return Vec::new();
                };
                let mut starts: Vec<usize> = v
                    .doc()
                    .outline()
                    .iter()
                    .filter_map(|o| match o.target {
                        Some(bo_pdfread::LinkTarget::Page { index, .. }) => Some(index),
                        _ => None,
                    })
                    .collect();
                starts.push(0);
                starts.sort_unstable();
                starts.dedup();
                starts
                    .iter()
                    .enumerate()
                    .map(|(k, s)| (*s..starts.get(k + 1).copied().unwrap_or(n)).collect())
                    .filter(|g: &Vec<usize>| !g.is_empty())
                    .collect()
            }
            _ => {
                // Blank pages separate the parts and are left out.
                let Some(v) = self.r.view_mut() else {
                    return Vec::new();
                };
                let mut out: Vec<Vec<usize>> = vec![Vec::new()];
                for p in 0..n {
                    let blank = v
                        .doc_mut()
                        .render(p, bo_pdfread::RenderOptions::default())
                        .is_ok_and(|pg| pg.list.commands.is_empty());
                    if blank {
                        out.push(Vec::new());
                    } else if let Some(last) = out.last_mut() {
                        last.push(p);
                    }
                }
                out.retain(|g| !g.is_empty());
                out
            }
        }
    }

    fn split_ok(&mut self, f: &Form) {
        let groups = self.split_groups(f);
        let stem = self.doc_name();
        let Some(v) = self.r.view() else { return };
        let mut files = Vec::new();
        for (i, g) in groups.iter().enumerate() {
            match v.doc().extract_pages(g, SaveOptions::default()) {
                Ok(b) => files.push((format!("{stem}-part{}.pdf", i + 1), b)),
                Err(e) => {
                    self.r.set_status(&e);
                    return;
                }
            }
        }
        if !files.is_empty() {
            self.push(ReaderRequest::SaveMany(files));
        }
    }

    // --- Edit Object: image ------------------------------------------------------------

    /// Image items of the Edit Object context menu (one picture selected): Replace and
    /// Extract Image, as the reference offers.
    pub(crate) fn image_menu8(&mut self) -> Vec<crate::popup::MenuItem> {
        let one = self
            .r
            .view_mut()
            .and_then(|v| self.edit.selected_image(v))
            .is_some();
        if !one {
            return Vec::new();
        }
        [
            ("obj.replace", "pdfEditor8.replaceImage"),
            ("obj8.extractImage", "pdfEditor8.extractImage"),
        ]
        .iter()
        .map(|(cmd, key)| crate::popup::MenuItem {
            label: self.t(key),
            choice: crate::popup::Choice::Command((*cmd).into()),
            checked: false,
            dim: false,
        })
        .collect()
    }

    /// Save the selected picture as a file (JPEG as stored, else PNG).
    fn extract_image8(&mut self) {
        let stem = self.doc_name();
        let Some(v) = self.r.view_mut() else { return };
        let Some((page, id)) = self.edit.selected_image(v) else {
            return;
        };
        match v.doc_mut().object_image(page, id) {
            Some((bytes, ext)) => self.push(ReaderRequest::SaveBytes {
                name: format!("{stem}-p{}-image.{ext}", page + 1),
                bytes,
            }),
            None => {
                let msg = self.t("pdfEditor8.noImage");
                self.set_status(&msg);
            }
        }
    }

    // --- Input, painting -----------------------------------------------------------------

    /// Input while a round-8 dialog is open; true when used (it keeps everything).
    pub(crate) fn form8_input(&mut self, input: &Input) -> bool {
        let Some((f, _)) = self.form8.last_mut() else {
            return false;
        };
        match input {
            Input::Resize { .. } | Input::Scale(_) | Input::Maximized(_) | Input::Tick => {
                return false;
            }
            Input::Blink => {
                self.caret_on = !self.caret_on;
                return true;
            }
            _ => {}
        }
        let saved = f.pick("saved");
        let out = f.input(input);
        if let FormOut::Button(id) = out {
            self.form8_button(id);
        } else if self
            .form8
            .last()
            .is_some_and(|f| f.0.pick("saved") != saved)
        {
            // A preset picked from Saved Settings fills the dialog.
            self.load_preset8();
        }
        true
    }

    fn form8_button(&mut self, id: &'static str) {
        let Some((f, what)) = self.form8.last_mut() else {
            return;
        };
        match (id, &*what) {
            ("cancel", _) => {
                self.form8.pop();
            }
            ("insert", FormWhat::HeaderFooter) => {
                let macro_text = match f.pick("macro") {
                    0 => crate::editor_dlg8b::NUMBER_FORMATS
                        .get(f.pick("numfmt"))
                        .map_or("<<page>>", |n| n.1),
                    1 => "<<page>> / <<pages>>",
                    _ => "<<date>>",
                };
                let target = f
                    .focus
                    .and_then(|i| match f.ctls.get(i) {
                        Some(Ctl::Edit {
                            id, multi: true, ..
                        }) => Some(*id),
                        _ => None,
                    })
                    .unwrap_or("fc");
                let mut s = f.text(target);
                s.push_str(macro_text);
                f.set(target, Val::Text(s));
            }
            ("perm", FormWhat::Password) => self.open_permissions(),
            ("ok", _) => {
                let Some((f, what)) = self.form8.pop() else {
                    return;
                };
                if !f
                    .ctls
                    .iter()
                    .any(|c| matches!(c, Ctl::Button { id: "ok", on, .. } if f.enabled(*on)))
                {
                    self.form8.push((f, what));
                    return;
                }
                match what {
                    FormWhat::HeaderFooter => self.header_footer_ok(&f),
                    FormWhat::Watermark => self.watermark_ok(&f),
                    FormWhat::Background => self.background_ok(&f),
                    FormWhat::BatesFiles => self.open_bates_options(),
                    FormWhat::BatesOptions => self.bates_ok(&f),
                    FormWhat::Password => {
                        if !self.password_ok(&f) {
                            self.form8.push((f, FormWhat::Password));
                        }
                    }
                    FormWhat::Permissions => {
                        self.perm8 = (
                            f.pick("print") == 1,
                            f.pick("copy") == 1,
                            f.pick("modify") == 1,
                            f.pick("annot") == 1,
                        );
                        // Show the new summary in the Password Protection dialog under it.
                        if let Some((parent, FormWhat::Password)) = self.form8.pop() {
                            let keep = parent.vals.clone();
                            self.open_password();
                            if let Some((p, _)) = self.form8.last_mut() {
                                p.vals = keep;
                            }
                        }
                    }
                    FormWhat::Insert(file) => self.insert_ok(&f, file),
                    FormWhat::Extract => self.extract_ok(&f),
                    FormWhat::Split => self.split_ok(&f),
                    FormWhat::Sub => self.sub_ok(&f),
                    FormWhat::SavePreset(kind) => self.save_preset_ok(kind, &f),
                }
            }
            _ => self.dlg8b_button(id),
        }
    }

    /// The open round-8 dialogs, the top one last.
    pub(crate) fn paint_form8(&mut self, p: &mut dyn crate::Painter) {
        let (win, scale, caret) = (self.window_size(), self.ui_scale(), self.caret_on);
        let img = self.preview_image8();
        let n = self.form8.len();
        for (i, (f, what)) in self.form8.iter_mut().enumerate() {
            f.paint(p, win, scale, caret && i + 1 == n);
            paint_preview(p, f, what, scale, img);
        }
    }
}

/// "1-3,5" label of 0-based pages.
pub(crate) fn ranges_label(pages: &[usize]) -> String {
    let mut v = pages.to_vec();
    v.sort_unstable();
    v.dedup();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1] == v[j] + 1 {
            j += 1;
        }
        out.push(if i == j {
            format!("{}", v[i] + 1)
        } else {
            format!("{}-{}", v[i] + 1, v[j] + 1)
        });
        i = j + 1;
    }
    out.join(",")
}

fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    bo_sign::iso_utc(secs).get(..10).unwrap_or("").to_owned()
}

/// The preview boxes: dashed margin guides and the header / footer texts (Header & Footer), a
/// page with the watermark text or the background colour (Watermark, Background).
fn paint_preview(
    p: &mut dyn crate::Painter,
    f: &Form,
    what: &FormWhat,
    scale: f32,
    img: Option<(usize, f32, f32)>,
) {
    use bo_render::Rgba;
    if f.pick("preview") != 1 {
        return;
    }
    let Some(r) = f.ctls.iter().find_map(|c| match c {
        Ctl::Frame { r } => Some(*r),
        _ => None,
    }) else {
        return;
    };
    let s = scale / 1.25;
    let q = |v: f32| (v * s).round();
    let b = [f.frame[0] + q(r[0]), f.frame[1] + q(r[1]), q(r[2]), q(r[3])];
    let small = crate::TextStyle {
        size: q(12.0),
        bold: false,
    };
    let blue = Rgba(0x03, 0x03, 0xFF, 255);
    match what {
        FormWhat::HeaderFooter => {
            // Margin guides: dashed blue lines; the page cut in the middle.
            let (mx, my) = (q(115.0), q(57.0));
            let dash_v = |p: &mut dyn crate::Painter, x: f32| {
                let mut y = b[1] + 1.0;
                while y < b[1] + b[3] - 1.0 {
                    p.fill_rect(x, y, 1.0, q(6.0).min(b[1] + b[3] - 1.0 - y), blue);
                    y += q(10.0);
                }
            };
            let dash_h = |p: &mut dyn crate::Painter, y: f32| {
                let mut x = b[0] + 1.0;
                while x < b[0] + b[2] - 1.0 {
                    p.fill_rect(x, y, q(6.0).min(b[0] + b[2] - 1.0 - x), 1.0, blue);
                    x += q(10.0);
                }
            };
            dash_v(p, b[0] + mx);
            dash_v(p, b[0] + b[2] - mx);
            dash_h(p, b[1] + my);
            dash_h(p, b[1] + b[3] - my);
            p.fill_rect(
                b[0] + 1.0,
                b[1] + b[3] / 2.0 - q(8.0),
                b[2] - 2.0,
                q(16.0),
                Rgba(0xF4, 0xF4, 0xF4, 255),
            );
            let c = f.color("color");
            let ink = Rgba(c[0], c[1], c[2], 255);
            let total = "1";
            for (row, ids) in [(0, ["hl", "hc", "hr"]), (1, ["fl", "fc", "fr"])] {
                for (k, id) in ids.iter().enumerate() {
                    let t = f
                        .text(id)
                        .replace('\n', " ")
                        .replace("<<page>>", &f.text("ppage"))
                        .replace("<<pages>>", total)
                        .replace(
                            "<<date>>",
                            &crate::editor_dlg8b::date_text(f.pick("datefmt")),
                        );
                    if t.is_empty() {
                        continue;
                    }
                    let tw = crate::editor_form8::text_w(&t, small.size);
                    let x = match k {
                        0 => b[0] + mx + q(3.0),
                        1 => b[0] + (b[2] - tw) / 2.0,
                        _ => b[0] + b[2] - mx - q(3.0) - tw,
                    };
                    let y = if row == 0 {
                        b[1] + my - small.size * 1.4
                    } else {
                        b[1] + b[3] - my + q(3.0)
                    };
                    p.text(&t, x.round(), y.round(), small, ink);
                }
            }
        }
        FormWhat::Watermark | FormWhat::Background => {
            // The page itself when its image is ready (else a page of its proportions),
            // centred in the box.
            let (ph, pw) = img.map_or_else(
                || {
                    let ph = b[3] - q(40.0);
                    (ph, (ph * 0.707).min(b[2] - q(40.0)))
                },
                |(_, w, h)| (h, w),
            );
            let (px, py) = (
                (b[0] + (b[2] - pw) / 2.0).round(),
                (b[1] + (b[3] - ph) / 2.0).round(),
            );
            let bg = if matches!(what, FormWhat::Background) {
                let c = f.color("color");
                let a = (f.num("opacity", 100.0) / 100.0).clamp(0.0, 1.0);
                let mix = |v: u8| (255.0 - (255.0 - f32::from(v)) * a).round() as u8;
                Rgba(mix(c[0]), mix(c[1]), mix(c[2]), 255)
            } else {
                Rgba(255, 255, 255, 255)
            };
            p.fill_rect(
                px - 1.0,
                py - 1.0,
                pw + 2.0,
                ph + 2.0,
                Rgba(0x40, 0x40, 0x40, 255),
            );
            if let Some((key, _, _)) = img {
                p.page(key, px as i32, py as i32);
                if matches!(what, FormWhat::Background) {
                    // The colour under the page's content, shown as a tint over it.
                    let a = (f.num("opacity", 100.0) / 100.0).clamp(0.0, 1.0);
                    let c = f.color("color");
                    p.fill_rect(px, py, pw, ph, Rgba(c[0], c[1], c[2], (a * 110.0) as u8));
                }
            } else {
                p.fill_rect(px, py, pw, ph, bg);
            }
            if matches!(what, FormWhat::Watermark) {
                let t = f.text("text").replace('\n', " ");
                if !t.is_empty() {
                    let c = f.color("color");
                    let a = (f.num("opacity", 50.0) / 100.0).clamp(0.05, 1.0);
                    // Watermark text at its share of the page width (Scale relative to target
                    // page), in its colour at its opacity.
                    let k = f.num("scale", 50.0) / 100.0;
                    let unit = crate::editor_form8::text_w(&t, 10.0).max(1.0);
                    let size = crate::TextStyle {
                        size: (pw * k * 10.0 / unit).clamp(q(8.0), ph / 3.0).round(),
                        bold: false,
                    };
                    let tw = crate::editor_form8::text_w(&t, size.size);

                    p.push_clip(px, py, pw, ph);
                    p.text(
                        &t,
                        (px + (pw - tw) / 2.0).round(),
                        (py + ph / 2.0 - size.size * 0.66).round(),
                        size,
                        Rgba(c[0], c[1], c[2], (a * 255.0) as u8),
                    );
                    p.pop_clip();
                }
            }
        }
        _ => {}
    }
}
