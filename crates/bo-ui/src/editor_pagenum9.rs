//! BlueEditor round 9: Organize › Format Page Numbers opens "Page Numbers" laid out as the
//! reference editor's (black-box at 125 %: 691 × 538 px): Pages (All Pages / Selected — only
//! with pages chosen in the grid — / Pages From … To … of N), Numbering (Begin new section:
//! Style, Prefix, Start; or Extend numbering used in preceding section to selected pages),
//! Sample, OK / Cancel. Labels go to `/PageLabels` (ISO 32000-1 §12.4.2).
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, combo, edit, group, label, radio, rlabel, Ctl, Form, On, Val};
use crate::editor_window::EditorWindow;
use bo_pdfread::LabelStyle;

/// The Style list in the reference's order.
pub(crate) const STYLES: [(LabelStyle, &str); 6] = [
    (LabelStyle::Decimal, "1, 2, 3, ..."),
    (LabelStyle::RomanLower, "i, ii, iii, ..."),
    (LabelStyle::RomanUpper, "I, II, III, ..."),
    (LabelStyle::LettersLower, "a, b, c, ..."),
    (LabelStyle::LettersUpper, "A, B, C, ..."),
    (LabelStyle::None, "None"),
];

/// The sample line: the first three labels of a section.
pub(crate) fn sample(style: LabelStyle, prefix: &str, start: u32) -> String {
    let one = |n: u32| -> String {
        let body = match style {
            LabelStyle::Decimal => n.to_string(),
            LabelStyle::RomanLower => roman(n).to_lowercase(),
            LabelStyle::RomanUpper => roman(n),
            LabelStyle::LettersLower => letters(n).to_lowercase(),
            LabelStyle::LettersUpper => letters(n),
            LabelStyle::None => String::new(),
        };
        format!("{prefix}{body}")
    };
    format!("{},{},{},...", one(start), one(start + 1), one(start + 2))
}

fn roman(mut n: u32) -> String {
    let mut s = String::new();
    for (v, r) in [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

/// A, B, … Z, AA, BB (§12.4.2 Table 159).
fn letters(n: u32) -> String {
    let n = n.max(1) - 1;
    let c = char::from(b'A' + u8::try_from(n % 26).unwrap_or(0));
    std::iter::repeat_n(c, (n / 26 + 1) as usize).collect()
}

impl EditorWindow {
    /// Open Page Numbers.
    pub(crate) fn open_pagenum9(&mut self) {
        let t = |k: &str| self.t(k);
        let n = self
            .r
            .view()
            .map_or(0, crate::ReaderView::page_count)
            .max(1);
        let sel = self.selected_pages();
        let (a, b) = (
            sel.first().copied().unwrap_or(0),
            sel.last().copied().unwrap_or(0),
        );
        let has_sel = sel.len() > 1;
        let c = vec![
            group(t("pdfEditor9.pagesGroup"), [17.0, 56.0, 657.0, 115.0]),
            radio("pages", 0, 29.0, 89.0, t("pdfEditor9.allPagesR")),
            Ctl::Radio {
                id: "pages",
                k: 1,
                x: 29.0,
                y: 118.0,
                label: t("pdfEditor9.selected"),
                on: if has_sel { On::Always } else { On::Never },
            },
            radio("pages", 2, 29.0, 148.0, t("pdfEditor9.pagesFrom")),
            edit("from", [145.0, 135.0, 98.0, 26.0], On::If("pages", 2)),
            label(256.0, 148.0, t("pdfEditor9.to")),
            edit("to", [285.0, 135.0, 98.0, 26.0], On::If("pages", 2)),
            label(
                396.0,
                148.0,
                t("pdfEditor9.of").replace("{{n}}", &n.to_string()),
            ),
            group(t("pdfEditor9.numbering"), [17.0, 189.0, 657.0, 199.0]),
            radio("mode", 0, 29.0, 223.0, t("pdfEditor9.beginSection")),
            rlabel(132.0, 255.0, t("pdfEditor9.style"), On::If("mode", 0)),
            combo(
                "style",
                [145.0, 241.0, 240.0, 27.0],
                STYLES.iter().map(|s| s.1.to_owned()).collect(),
                On::If("mode", 0),
            ),
            rlabel(132.0, 295.0, t("pdfEditor9.prefix"), On::If("mode", 0)),
            edit("prefix", [145.0, 282.0, 240.0, 27.0], On::If("mode", 0)),
            rlabel(132.0, 335.0, t("pdfEditor9.start"), On::If("mode", 0)),
            edit("start", [145.0, 322.0, 240.0, 27.0], On::If("mode", 0)),
            radio("mode", 1, 29.0, 368.0, t("pdfEditor9.extend")),
            label(17.0, 408.0, t("pdfEditor9.sampleLabel")),
            button(
                "ok",
                [483.0, 495.0, 91.0, 28.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [583.0, 495.0, 91.0, 28.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor9.pageNumbersTitle"), (691.0, 538.0), c)
            .with("pages", Val::Pick(if has_sel { 1 } else { 2 }))
            .with("from", Val::Text((a + 1).to_string()))
            .with(
                "to",
                Val::Text((if has_sel { b } else { a } + 1).to_string()),
            )
            .with("mode", Val::Pick(0))
            .with("style", Val::Pick(0))
            .with("start", Val::Text("1".into()));
        self.form8.push((f, FormWhat::PageNum9));
    }

    /// The sample text of the open Page Numbers dialog (painted after "Sample:").
    pub(crate) fn pagenum9_sample(f: &Form) -> String {
        let style = STYLES
            .get(f.pick("style"))
            .map_or(LabelStyle::Decimal, |s| s.0);
        let start = f.text("start").trim().parse::<u32>().unwrap_or(1).max(1);
        sample(style, &f.text("prefix"), start)
    }

    /// OK of Page Numbers.
    pub(crate) fn pagenum9_ok(&mut self, f: &Form) {
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        if n == 0 {
            return;
        }
        let sel = self.selected_pages();
        let (a, b) = match f.pick("pages") {
            0 => (0, n - 1),
            1 => (
                sel.first().copied().unwrap_or(0),
                sel.last().copied().unwrap_or(0),
            ),
            _ => {
                let num = |id: &str| f.text(id).trim().parse::<usize>().unwrap_or(1).max(1) - 1;
                let (x, y) = (num("from").min(n - 1), num("to").min(n - 1));
                (x.min(y), x.max(y))
            }
        };
        let (style, prefix, start) = if f.pick("mode") == 1 {
            // Extend: the section before the first page goes on through the pages.
            let ranges = self
                .with_doc(|d| Ok(d.page_label_ranges()))
                .unwrap_or_default();
            match ranges.iter().rev().find(|r| r.start < a) {
                Some(r) => {
                    let first = u32::try_from(a - r.start).unwrap_or(0) + r.first.max(1);
                    (r.style, r.prefix.clone(), first)
                }
                None => (
                    LabelStyle::Decimal,
                    String::new(),
                    u32::try_from(a + 1).unwrap_or(1),
                ),
            }
        } else {
            (
                STYLES
                    .get(f.pick("style"))
                    .map_or(LabelStyle::Decimal, |s| s.0),
                f.text("prefix"),
                f.text("start").trim().parse::<u32>().unwrap_or(1).max(1),
            )
        };
        match self.doc_edit(|d| d.set_page_labels(a, b, style, &prefix, start)) {
            Ok(()) => {
                if let Some(v) = self.r.view_mut() {
                    for i in 0..v.page_count() {
                        v.invalidate_page(i);
                    }
                }
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
    }
}
