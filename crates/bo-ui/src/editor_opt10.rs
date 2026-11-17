//! BlueEditor round 10: PDF Optimizer with every panel of the reference editor's Advanced
//! Optimization (black-box at 125 %, `docs/ui/REPORT-editor.md` round 10): the panel list at
//! the left (check box + name, the shown panel #0078D7) and the panel at the right —
//! Compress Images (colour / greyscale and monochrome rules), Unembed Fonts (embedded fonts and
//! fonts to unembed, Subset all embedded fonts), Transparency (Low / Medium / High Resolution
//! with their flattener settings), Discard Objects, Discard User Data, Clean Up; Predefined
//! Settings (Standard, Mobile and saved ones) with delete / save icons; Audit space usage; the
//! Reduce File Size dialog of File › PDF Optimizer. The work is `bo_pdfread::Optimize10`.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{
    button, combo, edit, label, label_on, rlabel, Ctl, Form, IconKind, On, Val,
};
use crate::editor_window::EditorWindow;
use crate::ReaderRequest;
use bo_pdfread::{
    CleanUp, ColorCompression, Discard, Downsample, MonoCompression, Optimize10, Resample,
};

/// JPEG quality of the Quality list (Minimum … Maximum).
const QUALITY: [u8; 5] = [10, 25, 50, 75, 90];
/// Versions of Make compatible with (after Retain existing).
const VERSIONS: [&str; 5] = ["1.3", "1.4", "1.5", "1.6", "1.7"];
/// Flattener settings of the Transparency presets, as the reference lists them: level, line
/// art and text ppi, gradients and meshes ppi, strokes to outlines, clip complex regions.
const FLATTEN: [(u32, u32, u32, bool, bool); 3] = [
    (75, 288, 144, true, false),
    (75, 300, 150, true, true),
    (100, 1200, 300, false, true),
];

/// State of the open PDF Optimizer that is not a control value.
#[derive(Debug, Default)]
pub(crate) struct Opt10 {
    /// Fonts left embedded / to unembed (Unembed Fonts panel).
    pub embedded: Vec<String>,
    pub unembed: Vec<String>,
    /// Settings waiting for the pages rendered to flatten transparency.
    pub pending: Option<(Optimize10, f32)>,
}

/// Values of the Standard settings.
fn standard() -> Vec<(&'static str, Val)> {
    let p = Val::Pick;
    let t = |s: &str| Val::Text(s.to_owned());
    vec![
        ("compat", p(0)),
        ("images", p(1)),
        ("fonts", p(1)),
        ("transp", p(0)),
        ("objects", p(1)),
        ("userData", p(1)),
        ("cleanUp", p(1)),
        ("ds", p(2)),
        ("ppi", t("150")),
        ("above", t("225")),
        ("comp", p(1)),
        ("quality", p(2)),
        ("monoDs", p(2)),
        ("monoPpi", t("300")),
        ("monoAbove", t("450")),
        ("monoComp", p(1)),
        ("monoQ", p(1)),
        ("smaller", p(1)),
        ("noUnembed", p(0)),
        ("subsetAll", p(1)),
        ("trLevel", p(1)),
        ("dActions", p(0)),
        ("dFlatten", p(0)),
        ("dJs", p(0)),
        ("dAlt", p(1)),
        ("dThumb", p(0)),
        ("dTags", p(0)),
        ("dSmooth", p(0)),
        ("dMerge", p(0)),
        ("dPrint", p(0)),
        ("dIndex", p(0)),
        ("dBookmarks", p(0)),
        ("uComments", p(0)),
        ("uExtRefs", p(0)),
        ("uMeta", p(0)),
        ("uObjData", p(0)),
        ("uAttach", p(0)),
        ("uPrivate", p(0)),
        ("uLayers", p(0)),
        ("objComp", p(0)),
        ("cFlate", p(1)),
        ("cLzw", p(1)),
        ("cBookmarks", p(1)),
        ("cLinks", p(1)),
        ("cDests", p(0)),
        ("cContent", p(1)),
        ("cWeb", p(1)),
    ]
}

/// Values of the Mobile settings (measured: 96 ppi above 144, JPEG High, monochrome 600 ppi
/// above 900 with CCITT Group 4, compatible with PDF 1.6).
fn mobile() -> Vec<(&'static str, Val)> {
    let mut v = standard();
    let p = Val::Pick;
    let t = |s: &str| Val::Text(s.to_owned());
    for (k, val) in [
        ("compat", p(4)),
        ("ppi", t("96")),
        ("above", t("144")),
        ("quality", p(3)),
        ("monoPpi", t("600")),
        ("monoAbove", t("900")),
        ("monoComp", p(3)),
    ] {
        if let Some(e) = v.iter_mut().find(|e| e.0 == k) {
            e.1 = val;
        }
    }
    v
}

/// Settings of the dialog's values.
pub(crate) fn settings(f: &Form, unembed: &[String]) -> Optimize10 {
    let on = |id: &str| f.pick(id) == 1;
    let method = |k: usize| match k {
        0 => Downsample::Off,
        1 => Downsample::Average,
        3 => Downsample::Subsample,
        _ => Downsample::Bicubic,
    };
    let ppi = f.num("ppi", 150.0).clamp(9.0, 2400.0);
    let mono_ppi = f.num("monoPpi", 300.0).clamp(9.0, 2400.0);
    let objects = on("objects");
    let user = on("userData");
    let clean = on("cleanUp");
    let comp = f.pick("objComp");
    Optimize10 {
        images: on("images"),
        color: Resample {
            method: method(f.pick("ds")),
            ppi,
            above: f.num("above", ppi * 1.5).max(ppi),
        },
        color_compression: match f.pick("comp") {
            0 => ColorCompression::Retain,
            2 => ColorCompression::Zip,
            _ => ColorCompression::Jpeg,
        },
        jpeg_quality: QUALITY.get(f.pick("quality")).copied().unwrap_or(50),
        mono: Resample {
            method: method(f.pick("monoDs")),
            ppi: mono_ppi,
            above: f.num("monoAbove", mono_ppi * 1.5).max(mono_ppi),
        },
        mono_compression: match f.pick("monoComp") {
            0 => MonoCompression::Retain,
            2 => MonoCompression::Ccitt3,
            3 => MonoCompression::Ccitt4,
            4 => MonoCompression::Zip,
            5 => MonoCompression::RunLength,
            _ => MonoCompression::Jbig2,
        },
        only_smaller: on("smaller"),
        fonts: on("fonts"),
        unembed: if on("fonts") && !on("noUnembed") {
            unembed.to_vec()
        } else {
            Vec::new()
        },
        subset: on("fonts") && on("subsetAll"),
        discard: Discard {
            form_actions: objects && on("dActions"),
            flatten_fields: objects && on("dFlatten"),
            javascript: objects && on("dJs"),
            alternate_images: objects && on("dAlt"),
            thumbnails: objects && on("dThumb"),
            tags: objects && on("dTags"),
            print_settings: objects && on("dPrint"),
            bookmarks: objects && on("dBookmarks"),
            comments_forms_media: user && on("uComments"),
            external_refs: user && on("uExtRefs"),
            metadata: user && on("uMeta"),
            attachments: user && on("uAttach"),
            private_data: user && on("uPrivate"),
            hidden_layers: user && on("uLayers"),
        },
        clean: CleanUp {
            object_streams: !clean || comp <= 1,
            flate_unencoded: !clean || (on("cFlate") && comp != 2),
            lzw_to_flate: clean && on("cLzw"),
            invalid_links: clean && on("cLinks"),
            optimize_content: clean && on("cContent"),
        },
        version: VERSIONS
            .get(f.pick("compat").wrapping_sub(1))
            .map(|v| (*v).to_owned()),
    }
}

fn row_check(id: &'static str, y: f32, label: String, on: On) -> Ctl {
    Ctl::Check {
        id,
        x: 259.0,
        y,
        label,
        on,
    }
}

impl EditorWindow {
    /// Items of Predefined Settings: Standard, Mobile and the saved settings.
    pub(crate) fn opt10_preset_names(&self) -> Vec<String> {
        [self.t("pdfEditor9.standard"), self.t("pdfEditor10.mobile")]
            .into_iter()
            .chain(
                self.presets8
                    .get("opt")
                    .into_iter()
                    .flatten()
                    .map(|(n, _)| n.clone()),
            )
            .collect()
    }

    /// Open PDF Optimizer (Advanced Optimization).
    pub(crate) fn open_opt10(&mut self) {
        self.ensure_presets8();
        let fonts = self
            .with_doc(|d| Ok(d.embedded_fonts()))
            .unwrap_or_default();
        self.opt10.embedded = fonts.into_iter().map(|f| f.name).collect();
        self.opt10.unembed.clear();
        let mut f = Form::new(
            self.t("pdfEditor9.optimizerTitle"),
            (1158.0, 795.0),
            Vec::new(),
        )
        .with("saved", Val::Pick(0))
        .with("panel", Val::Pick(0));
        for (k, v) in standard() {
            f.set(k, v);
        }
        f.ctls = self.opt10_ctls(&f);
        self.form8.push((f, FormWhat::Optimize9));
    }

    /// The controls for the dialog's values (the shown panel, lists of fonts).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn opt10_ctls(&self, f: &Form) -> Vec<Ctl> {
        let t = |k: &str| self.t(k);
        let version = self
            .r
            .view()
            .map_or_else(|| "1.7".to_owned(), |v| v.doc().version());
        let saved = f.pick("saved");
        let mut compat = vec![t("pdfEditor9.retainExisting")];
        compat.extend(
            VERSIONS
                .iter()
                .map(|v| t("pdfEditor10.pdfVersion").replace("{{v}}", v)),
        );
        let mut c = vec![
            label(17.0, 60.0, t("pdfEditor9.predefined")),
            combo(
                "saved",
                [159.0, 47.0, 124.0, 26.0],
                self.opt10_preset_names(),
                On::Always,
            ),
            Ctl::Icon {
                id: "optDelete",
                r: [290.0, 49.0, 22.0, 22.0],
                icon: IconKind::DeletePreset,
                on: if saved >= 2 { On::Always } else { On::Never },
            },
            Ctl::Icon {
                id: "optSave",
                r: [318.0, 49.0, 22.0, 22.0],
                icon: IconKind::SavePreset,
                on: On::Always,
            },
            button(
                "audit",
                [1002.0, 47.0, 139.0, 27.0],
                t("pdfEditor9.audit"),
                On::Always,
            ),
            label(
                17.0,
                97.0,
                t("pdfEditor9.currentVersion").replace("{{v}}", &version),
            ),
            label(247.0, 97.0, t("pdfEditor9.compatible")),
            combo("compat", [408.0, 83.0, 181.0, 27.0], compat, On::Always),
            Ctl::Box {
                r: [16.0, 130.0, 226.0, 612.0],
            },
            Ctl::Box {
                r: [248.0, 130.0, 894.0, 612.0],
            },
        ];
        for (k, (id, key)) in [
            ("images", "pdfEditor9.compressImages"),
            ("fonts", "pdfEditor9.unembedFonts"),
            ("transp", "pdfEditor9.transparency"),
            ("objects", "pdfEditor9.discardObjects"),
            ("userData", "pdfEditor9.discardUserData"),
            ("cleanUp", "pdfEditor9.cleanUp"),
        ]
        .into_iter()
        .enumerate()
        {
            c.push(Ctl::PanelRow {
                check: id,
                k,
                y: 143.0 + 24.0 * k as f32,
                label: t(key),
            });
        }
        match f.pick("panel") {
            1 => {
                let on = On::If("fonts", 1);
                let lists = if f.pick("fonts") == 1 && f.pick("noUnembed") == 0 {
                    On::Always
                } else {
                    On::Never
                };
                c.extend([
                    Ctl::Warn { x: 279.0, y: 160.0 },
                    label_on(306.0, 161.0, t("pdfEditor10.unembedWarning"), on),
                    row_check("noUnembed", 200.0, t("pdfEditor10.noUnembed"), on),
                    label_on(259.0, 231.0, t("pdfEditor10.embeddedFonts"), on),
                    Ctl::ListBox {
                        id: "fEmb",
                        r: [259.0, 247.0, 320.0, 451.0],
                        items: self.opt10.embedded.clone(),
                        on: lists,
                    },
                    label_on(811.0, 231.0, t("pdfEditor10.fontsToUnembed"), on),
                    Ctl::ListBox {
                        id: "fUn",
                        r: [811.0, 247.0, 320.0, 451.0],
                        items: self.opt10.unembed.clone(),
                        on: lists,
                    },
                    button(
                        "unembedBtn",
                        [586.0, 358.0, 217.0, 28.0],
                        t("pdfEditor10.unembed"),
                        if lists == On::Always && f.pick("fEmb") > 0 {
                            On::Always
                        } else {
                            On::Never
                        },
                    ),
                    button(
                        "retainBtn",
                        [586.0, 533.0, 217.0, 28.0],
                        t("pdfEditor10.retain"),
                        if lists == On::Always && f.pick("fUn") > 0 {
                            On::Always
                        } else {
                            On::Never
                        },
                    ),
                    row_check("subsetAll", 718.0, t("pdfEditor10.subsetAll"), on),
                ]);
            }
            2 => {
                let on = On::If("transp", 1);
                let level = f.pick("trLevel").min(2);
                let (lv, art, grad, strokes, clip) = FLATTEN[level];
                let yes = |b: bool| if b { "True" } else { "False" };
                c.extend([
                    label_on(259.0, 159.0, t("pdfEditor10.presetSettings"), on),
                    Ctl::Box {
                        r: [259.0, 185.0, 225.0, 546.0],
                    },
                    Ctl::Box {
                        r: [491.0, 185.0, 640.0, 546.0],
                    },
                ]);
                for (k, key) in [
                    "pdfEditor10.lowRes",
                    "pdfEditor10.mediumRes",
                    "pdfEditor10.highRes",
                ]
                .into_iter()
                .enumerate()
                {
                    c.push(Ctl::Radio {
                        id: "trLevel",
                        k,
                        x: 270.0,
                        y: 211.0 + 48.0 * k as f32,
                        label: t(key),
                        on,
                    });
                }
                let lines = [
                    format!("{} {lv}", t("pdfEditor10.trLevel")),
                    format!("{} {art}", t("pdfEditor10.lineArt")),
                    format!("{} {grad}", t("pdfEditor10.gradients")),
                    format!("{} False", t("pdfEditor10.textOutlines")),
                    format!("{} {}", t("pdfEditor10.strokeOutlines"), yes(strokes)),
                    format!("{} {}", t("pdfEditor10.clipComplex"), yes(clip)),
                    format!("{} True", t("pdfEditor10.overprint")),
                ];
                for (k, l) in lines.into_iter().enumerate() {
                    c.push(label_on(502.0, 211.0 + 37.0 * k as f32, l, on));
                }
            }
            3 => {
                let on = On::If("objects", 1);
                for (k, (id, key, never)) in [
                    ("dActions", "pdfEditor10.dActions", false),
                    ("dFlatten", "pdfEditor10.dFlatten", false),
                    ("dJs", "pdfEditor10.dJs", false),
                    ("dAlt", "pdfEditor10.dAlt", false),
                    ("dThumb", "pdfEditor10.dThumb", false),
                    ("dTags", "pdfEditor10.dTags", false),
                    ("dSmooth", "pdfEditor10.dSmooth", true),
                    ("dMerge", "pdfEditor10.dMerge", true),
                    ("dPrint", "pdfEditor10.dPrint", false),
                    ("dIndex", "pdfEditor10.dIndex", true),
                    ("dBookmarks", "pdfEditor10.dBookmarks", false),
                ]
                .into_iter()
                .enumerate()
                {
                    let o = if never { On::Never } else { on };
                    c.push(row_check(id, 153.0 + 32.0 * k as f32, t(key), o));
                }
            }
            4 => {
                let on = On::If("userData", 1);
                for (k, (id, key, never)) in [
                    ("uComments", "pdfEditor10.uComments", false),
                    ("uExtRefs", "pdfEditor10.uExtRefs", false),
                    ("uMeta", "pdfEditor10.uMeta", false),
                    ("uObjData", "pdfEditor10.uObjData", true),
                    ("uAttach", "pdfEditor10.uAttach", false),
                    ("uPrivate", "pdfEditor10.uPrivate", false),
                    ("uLayers", "pdfEditor10.uLayers", false),
                ]
                .into_iter()
                .enumerate()
                {
                    let o = if never { On::Never } else { on };
                    c.push(row_check(id, 153.0 + 32.0 * k as f32, t(key), o));
                }
            }
            5 => {
                let on = On::If("cleanUp", 1);
                c.push(label_on(259.0, 155.0, t("pdfEditor10.objComp"), on));
                c.push(combo(
                    "objComp",
                    [554.0, 142.0, 281.0, 27.0],
                    vec![
                        t("pdfEditor10.compressAll"),
                        t("pdfEditor10.compressStructure"),
                        t("pdfEditor10.removeCompression"),
                        t("pdfEditor10.leaveCompression"),
                    ],
                    on,
                ));
                for (k, (id, key, never)) in [
                    ("cFlate", "pdfEditor10.cFlate", false),
                    ("cLzw", "pdfEditor10.cLzw", false),
                    ("cBookmarks", "pdfEditor10.cBookmarks", true),
                    ("cLinks", "pdfEditor10.cLinks", false),
                    ("cDests", "pdfEditor10.cDests", true),
                    ("cContent", "pdfEditor10.cContent", false),
                    ("cWeb", "pdfEditor10.cWeb", true),
                ]
                .into_iter()
                .enumerate()
                {
                    let o = if never { On::Never } else { on };
                    c.push(row_check(id, 189.0 + 32.0 * k as f32, t(key), o));
                }
            }
            _ => {
                let on = On::If("images", 1);
                let downsample = vec![
                    t("pdfEditor9.dsOff"),
                    t("pdfEditor9.dsAverage"),
                    t("pdfEditor9.dsBicubic"),
                    t("pdfEditor9.dsSubsampling"),
                ];
                let quality = vec![
                    t("pdfEditor9.qMinimum"),
                    t("pdfEditor9.qLow"),
                    t("pdfEditor9.qMedium"),
                    t("pdfEditor9.qHigh"),
                    t("pdfEditor9.qMaximum"),
                ];
                let jpeg = f.pick("comp") == 1;
                c.extend([
                    label_on(260.0, 166.0, t("pdfEditor9.colorImages"), on),
                    label_on(261.0, 196.0, t("pdfEditor9.downsample"), on),
                    combo("ds", [356.0, 183.0, 231.0, 27.0], downsample.clone(), on),
                    label_on(594.0, 196.0, t("pdfEditor9.toLabelOpt"), on),
                    edit("ppi", [615.0, 183.0, 50.0, 27.0], on),
                    label_on(673.0, 196.0, t("pdfEditor9.ppiAbove"), on),
                    edit("above", [825.0, 183.0, 49.0, 27.0], on),
                    label_on(882.0, 196.0, t("pdfEditor9.ppiDot"), on),
                    label_on(260.0, 232.0, t("pdfEditor9.compression"), on),
                    combo(
                        "comp",
                        [355.0, 219.0, 231.0, 27.0],
                        vec![
                            t("pdfEditor9.retainExisting"),
                            "JPEG".to_owned(),
                            "ZIP".to_owned(),
                        ],
                        on,
                    ),
                ]);
                if jpeg {
                    c.extend([
                        label_on(594.0, 232.0, t("pdfEditor9.quality"), on),
                        combo("quality", [650.0, 219.0, 125.0, 27.0], quality, on),
                    ]);
                }
                c.extend([
                    label_on(260.0, 290.0, t("pdfEditor9.monoImages"), on),
                    label_on(261.0, 320.0, t("pdfEditor9.downsample"), on),
                    combo("monoDs", [356.0, 307.0, 231.0, 27.0], downsample, on),
                    label_on(594.0, 320.0, t("pdfEditor9.toLabelOpt"), on),
                    edit("monoPpi", [615.0, 307.0, 50.0, 27.0], on),
                    label_on(673.0, 320.0, t("pdfEditor9.ppiAbove"), on),
                    edit("monoAbove", [825.0, 307.0, 49.0, 27.0], on),
                    label_on(882.0, 320.0, t("pdfEditor9.ppiDot"), on),
                    label_on(260.0, 356.0, t("pdfEditor9.compression"), on),
                    combo(
                        "monoComp",
                        [355.0, 343.0, 231.0, 27.0],
                        vec![
                            t("pdfEditor9.retainExisting"),
                            "JBIG2".to_owned(),
                            "CCITT Group 3".to_owned(),
                            "CCITT Group 4".to_owned(),
                            "ZIP".to_owned(),
                            t("pdfEditor10.runLength"),
                        ],
                        on,
                    ),
                ]);
                if f.pick("monoComp") == 1 {
                    c.extend([
                        label_on(594.0, 356.0, t("pdfEditor9.quality"), on),
                        combo(
                            "monoQ",
                            [650.0, 343.0, 125.0, 27.0],
                            vec![t("pdfEditor10.lossless"), t("pdfEditor9.lossy")],
                            on,
                        ),
                    ]);
                }
                c.push(Ctl::Check {
                    id: "smaller",
                    x: 259.0,
                    y: 415.0,
                    label: t("pdfEditor9.onlySmaller"),
                    on,
                });
            }
        }
        c.extend([
            button(
                "ok",
                [950.0, 751.0, 91.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [1050.0, 751.0, 91.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ]);
        c
    }

    /// Rebuild the open optimizer's controls after a value changed (panel shown, lists,
    /// quality rows), and fill it from Predefined Settings when that changed.
    pub(crate) fn opt10_after_input(&mut self, saved_before: usize) {
        let Some((f, FormWhat::Optimize9)) = self.form8.last() else {
            return;
        };
        let saved = f.pick("saved");
        if saved != saved_before {
            let vals = match saved {
                0 => Some(standard()),
                1 => Some(mobile()),
                k => self
                    .presets8
                    .get("opt")
                    .and_then(|l| l.get(k - 2))
                    .map(|p| p.1.iter().map(|(k, v)| (*k, v.clone())).collect()),
            };
            if let (Some(vals), Some((f, _))) = (vals, self.form8.last_mut()) {
                for (k, v) in vals {
                    if !matches!(k, "saved" | "panel") {
                        f.set(k, v);
                    }
                }
            }
        }
        self.opt10_refresh();
    }

    fn opt10_refresh(&mut self) {
        let Some(ctls) = self.form8.last().map(|(f, _)| self.opt10_ctls(f)) else {
            return;
        };
        if let Some((f, _)) = self.form8.last_mut() {
            if f.ctls.len() != ctls.len() {
                f.focus = None;
                f.list = None;
            }
            f.ctls = ctls;
        }
    }

    /// Buttons of the optimizer and its small dialogs; true when handled.
    pub(crate) fn opt10_button(&mut self, id: &'static str) -> bool {
        let Some((f, what)) = self.form8.last() else {
            return false;
        };
        if !matches!(what, FormWhat::Optimize9) {
            return false;
        }
        match id {
            "panel" => self.opt10_refresh(),
            "unembedBtn" | "retainBtn" => {
                let (from_id, unembed) = if id == "unembedBtn" {
                    ("fEmb", true)
                } else {
                    ("fUn", false)
                };
                let k = f.pick(from_id);
                let (from, to) = if unembed {
                    (&mut self.opt10.embedded, &mut self.opt10.unembed)
                } else {
                    (&mut self.opt10.unembed, &mut self.opt10.embedded)
                };
                if k > 0 && k <= from.len() {
                    let name = from.remove(k - 1);
                    to.push(name);
                    to.sort();
                }
                if let Some((f, _)) = self.form8.last_mut() {
                    f.set("fEmb", Val::Pick(0));
                    f.set("fUn", Val::Pick(0));
                }
                self.opt10_refresh();
            }
            "audit" => self.open_audit10(),
            "optSave" => {
                let mut c = vec![
                    label(17.0, 72.0, self.t("pdfEditor10.saveAs")),
                    edit("pname", [17.0, 93.0, 342.0, 28.0], On::Always),
                ];
                c.push(button(
                    "ok",
                    [168.0, 132.0, 91.0, 28.0],
                    self.t("pdfReader.ok"),
                    On::Always,
                ));
                c.push(button(
                    "cancel",
                    [268.0, 132.0, 91.0, 28.0],
                    self.t("pdfReader.cancel"),
                    On::Always,
                ));
                let f = Form::new(self.t("pdfEditor10.createSettings"), (376.0, 175.0), c)
                    .focused("pname");
                self.form8.push((f, FormWhat::SavePreset("opt")));
            }
            "optDelete" => {
                let k = f.pick("saved");
                if k >= 2 {
                    if let Some(list) = self.presets8.get_mut("opt") {
                        if k - 2 < list.len() {
                            list.remove(k - 2);
                        }
                    }
                    self.store_presets8();
                    let names = self.opt10_preset_names();
                    if let Some((f, _)) = self.form8.last_mut() {
                        f.set_items("saved", &names);
                        f.set("saved", Val::Pick(0));
                    }
                    self.opt10_after_input(usize::MAX);
                }
            }
            _ => return false,
        }
        true
    }

    /// Audit Space Usage: bytes and share of the file by kind (rows with bytes, then Total).
    pub(crate) fn open_audit10(&mut self) {
        let Ok(bytes) = self.with_doc(|d| Ok(d.audit_space())) else {
            return;
        };
        let total: usize = bytes.iter().sum::<usize>().max(1);
        let keys = [
            "pdfEditor10.aImages",
            "pdfEditor10.aBookmarks",
            "pdfEditor10.aThumbnails",
            "pdfEditor10.aComments",
            "pdfEditor10.aForms",
            "pdfEditor10.aFonts",
            "pdfEditor10.aContent",
            "pdfEditor10.aStructure",
            "pdfEditor10.aDests",
            "pdfEditor10.aOverhead",
            "pdfEditor10.aColorSpaces",
            "pdfEditor10.aXref",
        ];
        let mut rows: Vec<(String, usize)> = bytes
            .iter()
            .zip(keys)
            .filter(|(b, _)| **b > 0)
            .map(|(b, k)| (self.t(k), *b))
            .collect();
        rows.push((self.t("pdfEditor10.aTotal"), total));
        let extra = 35.0 * (rows.len() as f32 - 7.0);
        let h = 407.0 + extra;
        let mut c = vec![
            crate::editor_form8::group(
                self.t("pdfEditor10.results"),
                [16.0, 57.0, 317.0, 301.0 + extra],
            ),
            label(29.0, 88.0, self.t("pdfEditor10.description")),
            label(180.0, 88.0, self.t("pdfEditor10.bytes")),
            label(248.0, 88.0, self.t("pdfEditor10.percentage")),
        ];
        for (k, (name, b)) in rows.iter().enumerate() {
            let y = 124.0 + 35.0 * k as f32;
            c.push(label(29.0, y, name.clone()));
            c.push(label(180.0, y, thousands(*b)));
            let pct = if k + 1 == rows.len() {
                "100 %".to_owned()
            } else {
                format!("{:.2} %", *b as f64 * 100.0 / total as f64)
            };
            c.push(label(248.0, y, pct));
        }
        c.push(button(
            "ok",
            [129.0, h - 44.0, 91.0, 27.0],
            self.t("pdfReader.ok"),
            On::Always,
        ));
        let f = Form::new(self.t("pdfEditor10.auditTitle"), (349.0, h), c);
        self.form8.push((f, FormWhat::Sub));
    }

    /// File › PDF Optimizer › Reduce File Size: the reference's small dialog (544 × 262).
    pub(crate) fn open_reduce10(&mut self) {
        let t = |k: &str| self.t(k);
        let mut compat = vec![t("pdfEditor9.retainExisting")];
        compat.extend(
            VERSIONS
                .iter()
                .map(|v| t("pdfEditor10.pdfVersion").replace("{{v}}", v)),
        );
        let c = vec![
            crate::editor_form8::group(t("pdfEditor10.versionCompat"), [16.0, 57.0, 512.0, 142.0]),
            rlabel(194.0, 133.0, t("pdfEditor9.compatible"), On::Always),
            combo("compat", [200.0, 119.0, 271.0, 27.0], compat, On::Always),
            button(
                "ok",
                [337.0, 220.0, 91.0, 27.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [437.0, 220.0, 91.0, 27.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f =
            Form::new(t("pdfEditor10.reduceTitle"), (544.0, 262.0), c).with("compat", Val::Pick(0));
        self.form8.push((f, FormWhat::Reduce10));
    }

    /// OK of Reduce File Size: the Standard settings made compatible with the chosen version.
    pub(crate) fn reduce10_ok(&mut self, f: &Form) {
        let mut base = Form::new(String::new(), (1.0, 1.0), Vec::new());
        for (k, v) in standard() {
            base.set(k, v);
        }
        base.set("compat", Val::Pick(f.pick("compat")));
        let o = settings(&base, &[]);
        self.run_opt10(&o);
    }

    /// OK of PDF Optimizer.
    pub(crate) fn opt10_ok(&mut self, f: &Form) {
        let o = settings(f, &self.opt10.unembed);
        if f.pick("transp") == 1 {
            let pages = self
                .with_doc(|d| Ok(d.transparency_pages()))
                .unwrap_or_default();
            if !pages.is_empty() {
                let ppi = FLATTEN[f.pick("trLevel").min(2)].2 as f32;
                self.opt10.pending = Some((o, ppi));
                self.push(ReaderRequest::RenderPages {
                    purpose: "opt10flatten".into(),
                    scale: ppi / 72.0,
                });
                return;
            }
        }
        self.run_opt10(&o);
    }

    /// Pages rendered to flatten transparency: draw the pages that use it as pictures, then
    /// optimize. False for other purposes.
    pub(crate) fn opt10_rendered(&mut self, purpose: &str, pngs: &[Vec<u8>]) -> bool {
        if purpose != "opt10flatten" {
            return false;
        }
        let Some((o, ppi)) = self.opt10.pending.take() else {
            return true;
        };
        let images: Vec<(usize, u32, u32, Vec<u8>)> = pngs
            .iter()
            .enumerate()
            .filter_map(|(i, png)| {
                let img = png_rgba(png)?;
                Some((i, img.width, img.height, img.pixels))
            })
            .collect();
        let r = self.doc_edit(|d| d.flatten_transparency(&images, ppi));
        if let Err(e) = r {
            self.set_status(&e);
        }
        self.run_opt10(&o);
        true
    }

    /// Optimize with `o` and save the optimized copy.
    pub(crate) fn run_opt10(&mut self, o: &Optimize10) {
        let enc = |rgba: &[u8], w: u32, h: u32, q: u8| bo_raster::jpeg::encode_jpeg(rgba, w, h, q);
        let name = format!("{}-optimized.pdf", self.doc_stem());
        match self.doc_edit(|d| d.optimize10(o, &enc)) {
            Ok((bytes, rep)) => {
                if let Some(v) = self.r.view_mut() {
                    for i in 0..v.page_count() {
                        v.invalidate_page(i);
                    }
                }
                self.after_doc_edit();
                let s = self
                    .t("pdfEditor10.optimized")
                    .replace("{before}", &thousands(rep.before))
                    .replace("{after}", &thousands(rep.after))
                    .replace("{images}", &(rep.images + rep.mono).to_string())
                    .replace("{fonts}", &(rep.subset + rep.unembedded).to_string());
                self.set_status(&s);
                self.push(ReaderRequest::SaveBytes { name, bytes });
            }
            Err(e) => self.set_status(&e),
        }
    }
}

/// A page the host rendered (PNG) as straight RGBA. `bo_raster::codecs::decode` reads only
/// GIF / BMP / TIFF: before round 10 the JPEG, TIFF and BMP conversions got nothing from it.
pub(crate) fn png_rgba(png: &[u8]) -> Option<bo_raster::codecs::Rgba> {
    let (width, height, mut pixels) = bo_raster::decode_premultiplied(png)?;
    for p in pixels.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = (u32::from(*c) * 255 / a).min(255) as u8;
            }
        }
    }
    Some(bo_raster::codecs::Rgba {
        width,
        height,
        pixels,
    })
}

/// `1234567` → `1,234,567` (the reference's audit numbers).
pub(crate) fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
