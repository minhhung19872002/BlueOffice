//! New Table Style… of BlueSheet (round 38): the reference's dialog — a name, the list of table
//! elements (ECMA-376 Part 1, §18.18.83 ST_TableStyleType), a preview of the style and a
//! description of the chosen element's formatting, "Set as default table style for this
//! document". The element's format is chosen in the dialog itself (fill, text colour, bold,
//! a thin border of a colour) instead of a second Format Cells dialog; Clear removes it.
//! OK writes the style with engine `createTableStyle` (a §18.8.40 `tableStyle` with its
//! formats in §18.8.15 `dxfs`), and the Format as Table gallery lists it under Custom.

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::Painter;
use bo_model::sheet::{
    CellRange, CellRef, Table, TableStyleElementParts, TableStyleInfo, TableStyleParts,
};
use bo_render::Rgba;
use bo_sheet::table_style::{custom_look, element_dxf, TABLE_ELEMENTS};
use serde_json::json;

/// Label keys of [`TABLE_ELEMENTS`], in the reference's order.
const ELEMENT_LABELS: [(&str, &str); 13] = [
    ("sheetTStyle.wholeTable", "Whole Table"),
    ("sheetTStyle.firstColumnStripe", "First Column Stripe"),
    ("sheetTStyle.secondColumnStripe", "Second Column Stripe"),
    ("sheetTStyle.firstRowStripe", "First Row Stripe"),
    ("sheetTStyle.secondRowStripe", "Second Row Stripe"),
    ("sheetTStyle.lastColumn", "Last Column"),
    ("sheetTStyle.firstColumn", "First Column"),
    ("sheetTStyle.headerRow", "Header Row"),
    ("sheetTStyle.totalRow", "Total Row"),
    ("sheetTStyle.firstHeaderCell", "First Header Cell"),
    ("sheetTStyle.lastHeaderCell", "Last Header Cell"),
    ("sheetTStyle.firstTotalCell", "First Total Cell"),
    ("sheetTStyle.lastTotalCell", "Last Total Cell"),
];

/// The format of one element while the dialog is open (colours `RRGGBB`, `""` = none).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ElementFmt {
    pub fill: String,
    pub text: String,
    pub bold: bool,
    pub border: String,
}

impl ElementFmt {
    fn is_set(&self) -> bool {
        *self != Self::default()
    }
}

/// The style being made.
#[derive(Debug, Clone, Default)]
pub(super) struct TStyleDraft {
    pub formats: [ElementFmt; 13],
    /// Element shown in the format controls.
    pub current: usize,
}

fn opt(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

impl TStyleDraft {
    /// The style as the engine stores it (elements with a format only).
    pub(super) fn parts(&self, name: &str) -> TableStyleParts {
        TableStyleParts {
            name: name.to_owned(),
            pivot: false,
            elements: TABLE_ELEMENTS
                .iter()
                .zip(&self.formats)
                .filter(|(_, f)| f.is_set())
                .map(|(kind, f)| TableStyleElementParts {
                    kind: (*kind).to_owned(),
                    size: 1,
                    dxf: element_dxf(opt(&f.fill), opt(&f.text), f.bold, false, opt(&f.border)),
                })
                .collect(),
        }
    }
}

fn hex_rgba(h: &str) -> Option<Rgba> {
    let v = u32::from_str_radix(h.trim_start_matches('#'), 16).ok()?;
    (h.trim_start_matches('#').len() == 6).then_some(Rgba(
        (v >> 16) as u8,
        (v >> 8) as u8,
        v as u8,
        255,
    ))
}

impl SheetWindow {
    /// A name the workbook does not use yet ("Table Style 1", …).
    fn next_table_style_name(&self, base: &str) -> String {
        let taken: Vec<String> = self
            .view
            .engine()
            .workbook()
            .map(|w| {
                w.styles
                    .table_styles
                    .iter()
                    .map(|s| s.name.to_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        (1..=10_000u32)
            .map(|i| format!("{base} {i}"))
            .find(|n| !taken.contains(&n.to_lowercase()))
            .unwrap_or_else(|| base.to_owned())
    }

    /// Open New Table Style….
    pub(super) fn open_table_style_form(&mut self) -> bool {
        let t = |k: &str, d: &str| self.t(k, d);
        let palette = self.palette_hexes();
        let name = self.next_table_style_name(&t("sheetTStyle.defaultName", "Table Style"));
        let mut f = Form::new(
            &t("sheetTStyle.title", "New Table Style"),
            (560.0, 560.0),
            vec![],
        );
        f.add(
            "lname",
            None,
            [16.0, 50.0, 56.0, 20.0],
            Kind::Label(t("sheetTStyle.name", "Name:")),
        );
        f.add(
            "name",
            None,
            [76.0, 46.0, 300.0, 24.0],
            Kind::Edit {
                caret: name.chars().count(),
                value: name.chars().collect(),
            },
        );
        f.add(
            "lelem",
            None,
            [16.0, 84.0, 250.0, 20.0],
            Kind::Label(t("sheetTStyle.element", "Table Element:")),
        );
        f.add(
            "elements",
            None,
            [16.0, 106.0, 250.0, 230.0],
            Kind::List {
                items: ELEMENT_LABELS.iter().map(|(k, d)| t(k, d)).collect(),
                sel: Some(0),
                top: 0,
            },
        );
        f.add(
            "gformat",
            None,
            [16.0, 346.0, 250.0, 20.0],
            Kind::Group(t("sheetTStyle.format", "Format")),
        );
        for (i, (id, key, fallback)) in [
            ("fill", "sheetTStyle.fill", "Fill:"),
            ("text", "sheetTStyle.text", "Text color:"),
            ("border", "sheetTStyle.border", "Border:"),
        ]
        .into_iter()
        .enumerate()
        {
            let y = 372.0 + i as f32 * 32.0;
            let lid = match id {
                "fill" => "lfill",
                "text" => "ltext",
                _ => "lborder",
            };
            f.add(
                lid,
                None,
                [16.0, y + 4.0, 96.0, 20.0],
                Kind::Label(t(key, fallback)),
            );
            f.add(
                id,
                None,
                [116.0, y, 150.0, 26.0],
                Kind::ColorPick {
                    hex: String::new(),
                    palette: palette.clone(),
                },
            );
        }
        f.add(
            "bold",
            None,
            [16.0, 470.0, 120.0, 22.0],
            Kind::Check {
                label: t("sheetTStyle.bold", "Bold"),
                on: false,
            },
        );
        f.add(
            "clear",
            None,
            [166.0, 468.0, 100.0, 28.0],
            Kind::Button {
                label: t("sheetTStyle.clear", "Clear"),
                default: false,
            },
        );
        f.add(
            "gpreview",
            None,
            [290.0, 84.0, 254.0, 20.0],
            Kind::Group(t("sheetTStyle.preview", "Preview")),
        );
        f.add("preview", None, [300.0, 112.0, 234.0, 170.0], Kind::Canvas);
        f.add(
            "ldesc",
            None,
            [290.0, 294.0, 254.0, 20.0],
            Kind::Label(t("sheetTStyle.elementFormatting", "Element Formatting:")),
        );
        f.add(
            "desc",
            None,
            [290.0, 316.0, 254.0, 60.0],
            Kind::Note(String::new()),
        );
        f.add(
            "default",
            None,
            [16.0, 508.0, 330.0, 22.0],
            Kind::Check {
                label: t(
                    "sheetTStyle.setDefault",
                    "Set as default table style for this document",
                ),
                on: false,
            },
        );
        // OK / Cancel to the right of the default check, at the bottom.
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.tstyle = Some(TStyleDraft::default());
        self.form = Some(f);
        self.form_kind = FormKind::TableStyle;
        self.tstyle_sync_desc();
        true
    }

    /// Show the current element's format in the controls.
    fn tstyle_load(&mut self) {
        let Some(d) = self.tstyle.as_ref() else {
            return;
        };
        let fm = d.formats[d.current].clone();
        if let Some(f) = self.form.as_mut() {
            f.set_color("fill", &fm.fill);
            f.set_color("text", &fm.text);
            f.set_color("border", &fm.border);
            f.set_on("bold", fm.bold);
        }
        self.tstyle_sync_desc();
    }

    /// The controls changed: store them into the current element.
    fn tstyle_store(&mut self) {
        let (Some(f), Some(d)) = (self.form.as_ref(), self.tstyle.as_mut()) else {
            return;
        };
        d.formats[d.current] = ElementFmt {
            fill: f.color("fill"),
            text: f.color("text"),
            bold: f.on("bold"),
            border: f.color("border"),
        };
        self.tstyle_sync_desc();
    }

    /// "Element Formatting:" text of the current element, as the reference ("Shaded",
    /// "Bold", "Bordered"; nothing when the element has no format).
    fn tstyle_sync_desc(&mut self) {
        let Some(d) = self.tstyle.as_ref() else {
            return;
        };
        let fm = &d.formats[d.current];
        let mut parts = Vec::new();
        if !fm.fill.is_empty() {
            parts.push(self.t("sheetTStyle.shaded", "Shaded"));
        }
        if fm.bold {
            parts.push(self.t("sheetTStyle.boldDesc", "Bold"));
        }
        if !fm.text.is_empty() {
            parts.push(self.t("sheetTStyle.colored", "Font color"));
        }
        if !fm.border.is_empty() {
            parts.push(self.t("sheetTStyle.bordered", "Bordered"));
        }
        let s = parts.join("; ");
        if let Some(Kind::Note(n)) = self
            .form
            .as_mut()
            .and_then(|f| f.get_mut("desc"))
            .map(|c| &mut c.kind)
        {
            *n = s;
        }
    }

    /// Events of New Table Style….
    pub(super) fn tstyle_event(&mut self, ev: &Event) {
        match ev {
            Event::Changed("elements") | Event::Clicked("elements") => {
                let sel = self.form.as_ref().and_then(|f| f.sel("elements"));
                if let (Some(i), Some(d)) = (sel, self.tstyle.as_mut()) {
                    d.current = i.min(12);
                }
                self.tstyle_load();
            }
            Event::Changed("fill" | "text" | "border" | "bold") => self.tstyle_store(),
            Event::Clicked("clear") => {
                if let Some(d) = self.tstyle.as_mut() {
                    d.formats[d.current] = ElementFmt::default();
                }
                self.tstyle_load();
            }
            Event::Ok => {
                let Some(f) = self.form.as_ref() else { return };
                let name = f.text("name").trim().to_owned();
                let default = f.on("default");
                let Some(d) = self.tstyle.as_ref() else {
                    return;
                };
                let elements: serde_json::Map<String, serde_json::Value> = TABLE_ELEMENTS
                    .iter()
                    .zip(&d.formats)
                    .filter(|(_, f)| f.is_set())
                    .map(|(k, f)| {
                        (
                            (*k).to_owned(),
                            json!({
                                "fill": opt(&f.fill).map(|h| format!("#{h}")),
                                "text": opt(&f.text).map(|h| format!("#{h}")),
                                "bold": f.bold,
                                "border": opt(&f.border).map(|h| format!("#{h}")),
                            }),
                        )
                    })
                    .collect();
                if self.view.run(json!({
                    "cmd": "createTableStyle", "name": name, "elements": elements,
                    "setDefault": default,
                })) {
                    self.form = None;
                    self.tstyle = None;
                } else {
                    self.status = self.view.take_error().unwrap_or_else(|| {
                        self.t(
                            "sheetTStyle.nameTaken",
                            "This style name is already in use.",
                        )
                    });
                }
            }
            Event::Cancel | Event::Clicked("cancel") => {
                self.form = None;
                self.tstyle = None;
            }
            _ => {}
        }
    }

    /// The preview: a table of a header, four banded rows and a total row, five columns,
    /// drawn in the style being made (dashes in the text colour, as the gallery tiles).
    pub(super) fn paint_tstyle_preview(&self, r: [f32; 4], p: &mut dyn Painter) {
        let Some(d) = self.tstyle.as_ref() else {
            return;
        };
        let parts = d.parts("preview");
        let theme = self.view.engine().workbook().and_then(|w| w.theme.clone());
        let t = Table {
            range: CellRange::new(CellRef::new(0, 0), CellRef::new(5, 4)),
            header_row_count: Some(1),
            totals_row_count: Some(1),
            style: Some(TableStyleInfo {
                name: Some("preview".into()),
                show_row_stripes: true,
                show_column_stripes: false,
                show_first_column: false,
                show_last_column: false,
            }),
            ..Table::default()
        };
        let [x, y, w, h] = r;
        p.fill_rect(x, y, w, h, Rgba(255, 255, 255, 255));
        let inset = (w * 0.08).round();
        let (cw, rh) = ((w - 2.0 * inset) / 5.0, (h - 2.0 * inset) / 6.0);
        let grey = Rgba(0xD4, 0xD4, 0xD4, 255);
        for row in 0..6u32 {
            for col in 0..5u32 {
                let cx = (x + inset + col as f32 * cw).round();
                let cy = (y + inset + row as f32 * rh).round();
                let (ww, hh) = (cw.round(), rh.round());
                let look = custom_look(&parts, &t, row, col, theme.as_ref()).unwrap_or_default();
                if let Some(c) = look.fill.as_deref().and_then(hex_rgba) {
                    p.fill_rect(cx, cy, ww, hh, c);
                }
                let ink = look
                    .font
                    .as_deref()
                    .and_then(hex_rgba)
                    .unwrap_or(Rgba(0, 0, 0, 255));
                let dash_h = if look.bold { 2.0 } else { 1.0 };
                p.fill_rect(cx + ww * 0.3, cy + hh * 0.5, ww * 0.4, dash_h, ink);
                let edges = [
                    [cx, cy, ww, 1.0],
                    [cx, cy + hh - 1.0, ww, 1.0],
                    [cx, cy, 1.0, hh],
                    [cx + ww - 1.0, cy, 1.0, hh],
                ];
                for (e, rect) in look.borders.iter().zip(edges) {
                    if let Some(c) = e.as_ref().and_then(|e| hex_rgba(&e.color)) {
                        p.fill_rect(rect[0], rect[1], rect[2], rect[3], c);
                    }
                }
                // The reference's preview outlines empty cells faintly.
                if look == bo_sheet::table_style::Look::default() {
                    p.fill_rect(cx, cy + hh - 1.0, ww, 1.0, grey);
                }
            }
        }
    }
}
