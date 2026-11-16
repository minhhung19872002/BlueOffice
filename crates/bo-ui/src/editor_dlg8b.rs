//! BlueEditor round 8 (second part, after the owner's review of `editor-60…67`): the links and
//! small dialogs of Header & Footer / Watermark / Background (Appearance Options, Page number
//! and date format, Page Range Options, text style), named presets (Saved Settings), a picture
//! as the watermark, the real page in the previews, the Password Protection policy.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{
    button, check, combo, edit, group, label, radio, rlabel, spin, Ctl, Form, On, Val,
};
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use std::collections::BTreeMap;

/// Values of a dialog kept as a preset.
pub(crate) type Vals = BTreeMap<&'static str, Val>;

/// Page number formats (label, macro text).
pub(crate) const NUMBER_FORMATS: [(&str, &str); 5] = [
    ("1", "<<page>>"),
    ("1 of n", "<<page>> of <<pages>>"),
    ("1/n", "<<page>>/<<pages>>"),
    ("Page 1", "Page <<page>>"),
    ("Page 1 of n", "Page <<page>> of <<pages>>"),
];

/// Date formats offered.
const DATE_FORMATS: [&str; 4] = ["m/d/yyyy", "mm/dd/yy", "dd/mm/yyyy", "yyyy-mm-dd"];

/// Today in date format `k` of [`DATE_FORMATS`].
pub(crate) fn date_text(k: usize) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let iso = bo_sign::iso_utc(secs);
    let part = |a: usize, b: usize| iso.get(a..b).unwrap_or("").to_owned();
    let (y, m, d) = (part(0, 4), part(5, 7), part(8, 10));
    let n = |s: &str| s.trim_start_matches('0').to_owned();
    match k {
        0 => format!("{}/{}/{y}", n(&m), n(&d)),
        1 => format!("{m}/{d}/{}", y.get(2..).unwrap_or("")),
        2 => format!("{d}/{m}/{y}"),
        _ => format!("{y}-{m}-{d}"),
    }
}

/// Preset kind of a dialog.
fn kind_of(w: &FormWhat) -> Option<&'static str> {
    match w {
        FormWhat::HeaderFooter => Some("hf"),
        FormWhat::Watermark => Some("wm"),
        FormWhat::Background => Some("bg"),
        _ => None,
    }
}

impl EditorWindow {
    /// Items of a Saved Settings list: "[None specified]" and the presets of `kind`.
    pub(crate) fn preset_names(&self, kind: &str) -> Vec<String> {
        std::iter::once(self.t("pdfEditor8.noneSpecified"))
            .chain(
                self.presets8
                    .get(kind)
                    .into_iter()
                    .flatten()
                    .map(|(n, _)| n.clone()),
            )
            .collect()
    }

    /// Pages chosen with Page Range Options (empty: all).
    pub(crate) fn range_pages(&self, f: &Form) -> Vec<usize> {
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        let base: Vec<usize> = if f.pick("prk") == 1 {
            let a = (f.num("prf", 1.0) as usize).max(1);
            let b = (f.num("prt", n as f32) as usize).min(n);
            (a - 1..b.max(a - 1)).collect()
        } else {
            (0..n).collect()
        };
        let keep = |p: &usize| match f.pick("prs") {
            1 => p % 2 == 1,
            2 => p % 2 == 0,
            _ => true,
        };
        let out: Vec<usize> = base.into_iter().filter(keep).collect();
        if out.len() == n {
            Vec::new()
        } else if out.is_empty() {
            vec![usize::MAX]
        } else {
            out
        }
    }

    /// A small dialog over the top one, its values taken from it (`keys`) or `defaults`.
    fn open_sub(
        &mut self,
        title: &str,
        size: (f32, f32),
        ctls: Vec<Ctl>,
        defaults: &[(&'static str, Val)],
    ) {
        let parent = self
            .form8
            .last()
            .map(|f| f.0.vals.clone())
            .unwrap_or_default();
        let mut f = Form::new(self.t(title), size, ctls);
        for (k, v) in defaults {
            f.set(k, parent.get(k).cloned().unwrap_or_else(|| v.clone()));
        }
        self.form8.push((f, FormWhat::Sub));
    }

    fn ok_cancel_at(&self, y: f32, x: f32) -> Vec<Ctl> {
        vec![
            button("ok", [x, y, 91.0, 28.0], self.t("pdfReader.ok"), On::Always),
            button(
                "cancel",
                [x + 100.0, y, 91.0, 28.0],
                self.t("pdfReader.cancel"),
                On::Always,
            ),
        ]
    }

    /// Buttons and links of the round-8 dialogs not handled by `editor_dlg8`.
    pub(crate) fn dlg8b_button(&mut self, id: &'static str) {
        let Some(what) = self.form8.last().map(|f| f.1.clone()) else {
            return;
        };
        let t = |w: &Self, k: &str| w.t(k);
        match id {
            "appearance" => {
                let mut c = vec![
                    group(
                        t(self, "pdfEditor8.appearanceOptions2"),
                        [17.0, 55.0, 410.0, 125.0],
                    ),
                    rlabel(225.0, 92.0, t(self, "pdfEditor8.offsetX"), On::Always),
                    spin(
                        "offx",
                        [232.0, 78.0, 110.0, 28.0],
                        1.0,
                        (-500.0, 500.0),
                        On::Always,
                    ),
                    rlabel(225.0, 127.0, t(self, "pdfEditor8.offsetY"), On::Always),
                    spin(
                        "offy",
                        [232.0, 113.0, 110.0, 28.0],
                        1.0,
                        (-500.0, 500.0),
                        On::Always,
                    ),
                    Ctl::Check {
                        id: "shrink",
                        x: 28.0,
                        y: 160.0,
                        label: t(self, "pdfEditor8.shrink"),
                        on: On::Never,
                    },
                ];
                c.extend(self.ok_cancel_at(196.0, 236.0));
                self.open_sub(
                    "pdfEditor8.appearanceOptions2",
                    (444.0, 240.0),
                    c,
                    &[
                        ("offx", Val::Text("0".into())),
                        ("offy", Val::Text("0".into())),
                    ],
                );
            }
            "numfmt" => {
                let mut c = vec![
                    group(
                        t(self, "pdfEditor8.numberDateFormat"),
                        [17.0, 55.0, 410.0, 140.0],
                    ),
                    rlabel(175.0, 92.0, t(self, "pdfEditor8.numberFormat"), On::Always),
                    combo(
                        "numfmt",
                        [182.0, 78.0, 230.0, 28.0],
                        NUMBER_FORMATS.iter().map(|n| n.0.to_owned()).collect(),
                        On::Always,
                    ),
                    rlabel(175.0, 127.0, t(self, "pdfEditor8.dateFormat"), On::Always),
                    combo(
                        "datefmt",
                        [182.0, 113.0, 230.0, 28.0],
                        DATE_FORMATS.iter().map(|d| (*d).to_owned()).collect(),
                        On::Always,
                    ),
                    rlabel(175.0, 162.0, t(self, "pdfEditor8.startNumber"), On::Always),
                    spin(
                        "start",
                        [182.0, 148.0, 110.0, 28.0],
                        1.0,
                        (-9999.0, 99_999.0),
                        On::Always,
                    ),
                ];
                c.extend(self.ok_cancel_at(211.0, 236.0));
                self.open_sub(
                    "pdfEditor8.numberDateFormat",
                    (444.0, 255.0),
                    c,
                    &[
                        ("numfmt", Val::Pick(0)),
                        ("datefmt", Val::Pick(0)),
                        ("start", Val::Text("1".into())),
                    ],
                );
            }
            "range" => {
                let n = self
                    .r
                    .view()
                    .map_or(1, crate::ReaderView::page_count)
                    .max(1);
                let mut c = vec![
                    group(t(self, "pdfEditor8.pageRange"), [17.0, 55.0, 410.0, 160.0]),
                    radio("prk", 0, 28.0, 90.0, t(self, "pdfEditor8.allPages")),
                    radio("prk", 1, 28.0, 125.0, t(self, "pdfEditor8.pagesFrom")),
                    spin(
                        "prf",
                        [150.0, 111.0, 80.0, 28.0],
                        1.0,
                        (1.0, n as f32),
                        On::If("prk", 1),
                    ),
                    label(240.0, 125.0, t(self, "pdfEditor8.to")),
                    spin(
                        "prt",
                        [270.0, 111.0, 80.0, 28.0],
                        1.0,
                        (1.0, n as f32),
                        On::If("prk", 1),
                    ),
                    rlabel(100.0, 175.0, t(self, "pdfEditor8.subset"), On::Always),
                    combo(
                        "prs",
                        [107.0, 161.0, 220.0, 28.0],
                        vec![
                            t(self, "pdfEditor8.allInRange"),
                            t(self, "pdfEditor8.evenPages"),
                            t(self, "pdfEditor8.oddPages"),
                        ],
                        On::Always,
                    ),
                ];
                c.extend(self.ok_cancel_at(231.0, 236.0));
                self.open_sub(
                    "pdfEditor8.pageRangeOptions",
                    (444.0, 275.0),
                    c,
                    &[
                        ("prk", Val::Pick(0)),
                        ("prf", Val::Text("1".into())),
                        ("prt", Val::Text(n.to_string())),
                        ("prs", Val::Pick(0)),
                    ],
                );
            }
            "tstyle" => {
                let mut c = vec![
                    group(t(self, "pdfEditor8.textStyle"), [17.0, 55.0, 410.0, 70.0]),
                    check("under", 28.0, 90.0, t(self, "pdfEditor8.underline")),
                ];
                c.extend(self.ok_cancel_at(141.0, 236.0));
                self.open_sub(
                    "pdfEditor8.textStyle",
                    (444.0, 185.0),
                    c,
                    &[("under", Val::Pick(0))],
                );
            }
            "save" => {
                if let Some(kind) = kind_of(&what) {
                    let mut c = vec![
                        label(17.0, 72.0, t(self, "pdfEditor8.presetName")),
                        edit("pname", [17.0, 88.0, 410.0, 28.0], On::Always),
                    ];
                    c.extend(self.ok_cancel_at(136.0, 236.0));
                    let f = Form::new(t(self, "pdfEditor8.saveSettingsTitle"), (444.0, 180.0), c)
                        .focused("pname");
                    self.form8.push((f, FormWhat::SavePreset(kind)));
                }
            }
            "load" => self.load_preset8(),
            "delete" => {
                let Some(kind) = kind_of(&what) else { return };
                let Some(k) = self.form8.last().map(|f| f.0.pick("saved")) else {
                    return;
                };
                if k > 0 {
                    if let Some(list) = self.presets8.get_mut(kind) {
                        if k - 1 < list.len() {
                            list.remove(k - 1);
                        }
                    }
                    let names = self.preset_names(kind);
                    if let Some((f, _)) = self.form8.last_mut() {
                        f.set_items("saved", names);
                        f.set("saved", Val::Pick(0));
                    }
                }
            }
            "browse" => self.push(ReaderRequest::PickFile {
                purpose: "wm8Image".into(),
                filters: crate::editor_window::image_filters(),
            }),
            _ => {}
        }
    }

    /// OK of a small dialog: its values go into the dialog under it.
    pub(crate) fn sub_ok(&mut self, f: &Form) {
        if let Some((parent, _)) = self.form8.last_mut() {
            for (k, v) in &f.vals {
                parent.set(k, v.clone());
            }
        }
    }

    /// OK of the preset name: keep the values of the dialog under it.
    pub(crate) fn save_preset_ok(&mut self, kind: &'static str, f: &Form) {
        let name = f.text("pname").trim().to_owned();
        if name.is_empty() {
            return;
        }
        let Some(vals) = self.form8.last().map(|p| p.0.vals.clone()) else {
            return;
        };
        let list = self.presets8.entry(kind).or_default();
        list.retain(|(n, _)| *n != name);
        list.push((name.clone(), vals));
        let at = list.len();
        let names = self.preset_names(kind);
        if let Some((p, _)) = self.form8.last_mut() {
            p.set_items("saved", names);
            p.set("saved", Val::Pick(at));
        }
    }

    /// The chosen preset's values into the dialog.
    pub(crate) fn load_preset8(&mut self) {
        let Some((f, what)) = self.form8.last() else {
            return;
        };
        let Some(kind) = kind_of(what) else { return };
        let k = f.pick("saved");
        let Some(vals) = k
            .checked_sub(1)
            .and_then(|i| self.presets8.get(kind)?.get(i))
            .map(|p| p.1.clone())
        else {
            return;
        };
        if let Some((f, _)) = self.form8.last_mut() {
            for (key, v) in vals {
                if key != "saved" {
                    f.set(key, v);
                }
            }
        }
    }

    /// Password Protection: keep the choices (not the passwords) for the next time.
    pub(crate) fn keep_policy(&mut self, f: &Form) {
        let keep: Vals = f
            .vals
            .iter()
            .filter(|(k, _)| matches!(**k, "openOn" | "restrictOn" | "alg" | "meta" | "policy"))
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        self.policy8 = Some(keep);
    }

    /// A picture picked for the watermark's File source.
    pub(crate) fn wm_image_picked(&mut self, name: &str, bytes: &[u8]) {
        self.wm_image8 = Some(bytes.to_vec());
        if let Some((f, FormWhat::Watermark)) = self.form8.last_mut() {
            f.set("file", Val::Text(name.to_owned()));
        }
    }

    /// The picked picture over every page of the range: centred, Absolute Scale of its size
    /// (1 px = 1 pt) or Scale relative to the page width, rotation, opacity.
    pub(crate) fn image_watermark(&mut self, f: &Form) {
        let Some(bytes) = self.wm_image8.clone() else {
            let msg = self.t("pdfEditor8.pickPicture");
            self.set_status(&msg);
            return;
        };
        let Some((iw, ih)) = bo_pdfread::image_size(&bytes) else {
            let msg = self.t("pdfEditor8.noImage");
            self.set_status(&msg);
            return;
        };
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        let mut pages = self.range_pages(f);
        if pages.is_empty() {
            pages = (0..n).collect();
        }
        let (dx, dy) = self.offset_of(f);
        let alpha = (f.num("opacity", 50.0) / 100.0).clamp(0.05, 1.0);
        let rel = f.pick("scaleOn") == 1;
        let r = self.doc_edit(|d| {
            for &p in pages.iter().filter(|p| **p < n) {
                let Some((pw, ph)) = d.page_info(p).map(|i| (i.width, i.height)) else {
                    continue;
                };
                let (w, h) = if rel {
                    let k = f.num("scale", 50.0) / 100.0 * pw / iw as f32;
                    (iw as f32 * k, ih as f32 * k)
                } else {
                    let k = f.num("fscale", 100.0) / 100.0;
                    (iw as f32 * k, ih as f32 * k)
                };
                let (cx, cy) = (pw / 2.0 + dx, ph / 2.0 - dy);
                let rect = [cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0];
                d.add_image(p, rect, bo_pdfread::NewImage(&bytes))?;
                let last = d.page_objects(p)?.len().saturating_sub(1);
                d.set_object_opacity(p, &[last], alpha)?;
            }
            Ok(())
        });
        match r {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }

    /// Distances of the Position group as an offset from the page centre (right / up, pt).
    fn offset_of(&self, f: &Form) -> (f32, f32) {
        let pt = |d: &str| f.num(d, 0.0) * 72.0;
        (pt("hdist"), pt("vdist"))
    }

    /// The page and scale (device pixels per point) of the open Watermark / Background
    /// preview.
    pub(crate) fn preview_want8(&self) -> Option<(usize, f32)> {
        let (f, what) = self
            .form8
            .iter()
            .rev()
            .find(|(_, w)| matches!(w, FormWhat::Watermark | FormWhat::Background))?;
        let _ = what;
        if f.pick("preview") != 1 {
            return None;
        }
        let r = f.ctls.iter().find_map(|c| match c {
            Ctl::Frame { r } => Some(*r),
            _ => None,
        })?;
        let v = self.r.view()?;
        let n = v.page_count();
        let i = (f.num("ppage", 1.0) as usize).clamp(1, n.max(1)) - 1;
        let (pw, ph) = v.page_size(i);
        let s = self.ui_scale() / 1.25;
        let scale = ((r[2] - 40.0) * s / pw.max(1.0)).min((r[3] - 40.0) * s / ph.max(1.0));
        Some((i, scale))
    }

    /// The preview's page image: key, width and height in device pixels, when drawn.
    pub(crate) fn preview_image8(&self) -> Option<(usize, f32, f32)> {
        let (i, s) = self.preview_want8()?;
        let v = self.r.view()?;
        if !v.has_preview(i, s) {
            return None;
        }
        let (pw, ph) = v.page_size(i);
        let base = self.r.key_base()?;
        Some((
            base + crate::reader_view::PREVIEW_KEY,
            (pw * s).round(),
            (ph * s).round(),
        ))
    }
}
