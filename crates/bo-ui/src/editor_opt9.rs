//! BlueEditor round 9: File › PDF Optimizer opens "PDF Optimizer" laid out as the reference
//! editor's Advanced Optimization (black-box at 125 %: 1158 × 795 px): Predefined Settings,
//! Current PDF version, Make compatible with, the list of panels with check boxes (Compress
//! Images, Unembed Fonts, Transparency, Discard Objects, Discard User Data, Clean Up) and the
//! Compress Images panel (colour / greyscale downsampling to N ppi for images above M ppi,
//! compression and quality; monochrome rows; compress only when smaller). What the engine does
//! not do (unembedding fonts, flattening transparency, JBIG2) is dimmed. OK writes the
//! optimized copy as before (`bo_pdfread::OptimizeOptions`).
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, check, combo, edit, label, label_on, Ctl, Form, On, Val};
use crate::editor_window::EditorWindow;

/// JPEG quality of the Quality list (Minimum … Maximum).
pub(crate) const QUALITY: [u8; 5] = [10, 25, 50, 75, 90];

fn dim_check(id: &'static str, x: f32, y: f32, label: String) -> Ctl {
    Ctl::Check {
        id,
        x,
        y,
        label,
        on: On::Never,
    }
}

impl EditorWindow {
    /// Open PDF Optimizer.
    pub(crate) fn open_opt9(&mut self) {
        let t = |k: &str| self.t(k);
        let version = self
            .r
            .view()
            .map_or_else(|| "1.7".to_owned(), |v| v.doc().version());
        let quality = vec![
            t("pdfEditor9.qMinimum"),
            t("pdfEditor9.qLow"),
            t("pdfEditor9.qMedium"),
            t("pdfEditor9.qHigh"),
            t("pdfEditor9.qMaximum"),
        ];
        let downsample = vec![
            t("pdfEditor9.dsOff"),
            t("pdfEditor9.dsAverage"),
            t("pdfEditor9.dsBicubic"),
            t("pdfEditor9.dsSubsampling"),
        ];
        let c = vec![
            label(17.0, 60.0, t("pdfEditor9.predefined")),
            combo(
                "preset",
                [159.0, 47.0, 124.0, 26.0],
                vec![t("pdfEditor9.standard")],
                On::Always,
            ),
            button(
                "audit",
                [1002.0, 47.0, 139.0, 27.0],
                t("pdfEditor9.audit"),
                On::Never,
            ),
            label(
                17.0,
                97.0,
                t("pdfEditor9.currentVersion").replace("{{v}}", &version),
            ),
            label(247.0, 97.0, t("pdfEditor9.compatible")),
            combo(
                "compat",
                [408.0, 83.0, 181.0, 27.0],
                vec![t("pdfEditor9.retainExisting")],
                On::Always,
            ),
            Ctl::Frame {
                r: [16.0, 130.0, 226.0, 612.0],
            },
            check("images", 21.0, 143.0, t("pdfEditor9.compressImages")),
            dim_check("fonts", 21.0, 167.0, t("pdfEditor9.unembedFonts")),
            dim_check("transp", 21.0, 191.0, t("pdfEditor9.transparency")),
            check("objects", 21.0, 215.0, t("pdfEditor9.discardObjects")),
            check("userData", 21.0, 239.0, t("pdfEditor9.discardUserData")),
            check("cleanUp", 21.0, 263.0, t("pdfEditor9.cleanUp")),
            Ctl::Frame {
                r: [248.0, 130.0, 894.0, 612.0],
            },
            label(260.0, 166.0, t("pdfEditor9.colorImages")),
            label_on(
                261.0,
                196.0,
                t("pdfEditor9.downsample"),
                On::If("images", 1),
            ),
            combo(
                "ds",
                [356.0, 183.0, 231.0, 27.0],
                downsample.clone(),
                On::If("images", 1),
            ),
            label_on(
                594.0,
                196.0,
                t("pdfEditor9.toLabelOpt"),
                On::If("images", 1),
            ),
            edit("ppi", [615.0, 183.0, 50.0, 27.0], On::If("images", 1)),
            label_on(673.0, 196.0, t("pdfEditor9.ppiAbove"), On::If("images", 1)),
            edit("above", [825.0, 183.0, 49.0, 27.0], On::If("images", 1)),
            label_on(882.0, 196.0, t("pdfEditor9.ppiDot"), On::If("images", 1)),
            label_on(
                260.0,
                232.0,
                t("pdfEditor9.compression"),
                On::If("images", 1),
            ),
            combo(
                "comp",
                [355.0, 219.0, 231.0, 27.0],
                vec!["JPEG".to_owned()],
                On::If("images", 1),
            ),
            label_on(594.0, 232.0, t("pdfEditor9.quality"), On::If("images", 1)),
            combo(
                "quality",
                [650.0, 219.0, 125.0, 27.0],
                quality,
                On::If("images", 1),
            ),
            label(260.0, 290.0, t("pdfEditor9.monoImages")),
            label_on(261.0, 320.0, t("pdfEditor9.downsample"), On::Never),
            combo("monoDs", [356.0, 307.0, 231.0, 27.0], downsample, On::Never),
            label_on(594.0, 320.0, t("pdfEditor9.toLabelOpt"), On::Never),
            edit("monoPpi", [615.0, 307.0, 50.0, 27.0], On::Never),
            label_on(673.0, 320.0, t("pdfEditor9.ppiAbove"), On::Never),
            edit("monoAbove", [825.0, 307.0, 49.0, 27.0], On::Never),
            label_on(882.0, 320.0, t("pdfEditor9.ppiDot"), On::Never),
            label_on(260.0, 356.0, t("pdfEditor9.compression"), On::Never),
            combo(
                "monoComp",
                [355.0, 343.0, 231.0, 27.0],
                vec!["JBIG2".to_owned()],
                On::Never,
            ),
            label_on(594.0, 356.0, t("pdfEditor9.quality"), On::Never),
            combo(
                "monoQ",
                [650.0, 343.0, 125.0, 27.0],
                vec![t("pdfEditor9.lossy")],
                On::Never,
            ),
            Ctl::Check {
                id: "smaller",
                x: 259.0,
                y: 415.0,
                label: t("pdfEditor9.onlySmaller"),
                on: On::If("images", 1),
            },
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
        ];
        let f = Form::new(t("pdfEditor9.optimizerTitle"), (1158.0, 795.0), c)
            .with("preset", Val::Pick(0))
            .with("compat", Val::Pick(0))
            .with("images", Val::Pick(1))
            .with("objects", Val::Pick(1))
            .with("userData", Val::Pick(1))
            .with("cleanUp", Val::Pick(1))
            .with("ds", Val::Pick(2))
            .with("ppi", Val::Text("150".into()))
            .with("above", Val::Text("225".into()))
            .with("comp", Val::Pick(0))
            .with("quality", Val::Pick(2))
            .with("monoDs", Val::Pick(2))
            .with("monoPpi", Val::Text("300".into()))
            .with("monoAbove", Val::Text("450".into()))
            .with("monoComp", Val::Pick(0))
            .with("monoQ", Val::Pick(0))
            .with("smaller", Val::Pick(1));
        self.form8.push((f, FormWhat::Optimize9));
    }

    /// OK of PDF Optimizer: resolution, threshold and quality from the Compress Images panel.
    pub(crate) fn opt9_ok(&mut self, f: &Form) {
        let images = f.pick("images") == 1 && f.pick("ds") != 0;
        let ppi = if images {
            f.num("ppi", 150.0).clamp(9.0, 2400.0)
        } else {
            0.0
        };
        let above = f.num("above", ppi * 1.5).max(ppi);
        let threshold = if ppi > 0.0 { above / ppi } else { 1.5 };
        let quality = QUALITY.get(f.pick("quality")).copied().unwrap_or(50);
        self.optimize9(ppi, threshold, quality, f.pick("cleanUp") == 1);
    }
}
