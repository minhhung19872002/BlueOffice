//! Envelopes and label sheets for mail merge.
//!
//! Label sheet presets are BlueOffice's own, defined in millimetres (page size, rows ×
//! columns, top / left margins, label size and pitch); envelope sizes are the ISO 269
//! C4 / C5 / C6 / DL and ISO B5 envelope formats plus the North American No. 10
//! (4⅛ × 9½ in). Labels are a fixed-layout table (ECMA-376 Part 1, §17.4.53 tblLayout)
//! with exact row heights (§17.4.81 trHeight, `hRule="exact"`); an envelope places the
//! delivery address in a floating table positioned on the page (§17.4.58 tblpPr).

use bo_model::{
    Block, Cell, CellProps, Document, HeightRule, LineSpacing, PageSetup, ParaProps, Paragraph,
    Row, RowProps, Run, RunContent, RunProps, Section, Table, TableFloat, TableProps, TableWidth,
    Twips,
};

/// Millimetres to twips (1 in = 25.4 mm = 1440 twips).
pub fn mm(v: f64) -> Twips {
    #[allow(clippy::cast_possible_truncation)]
    Twips((v * 1440.0 / 25.4).round() as i32)
}

/// A label sheet; every length in millimetres.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelSheet {
    /// Preset id (`custom` for user geometry).
    pub id: String,
    /// Page width.
    pub page_width: f64,
    /// Page height.
    pub page_height: f64,
    /// Label rows per page.
    pub rows: u32,
    /// Label columns per page.
    pub cols: u32,
    /// Page top edge to the first label row.
    pub top: f64,
    /// Page left edge to the first label column.
    pub left: f64,
    /// Label width.
    pub label_width: f64,
    /// Label height.
    pub label_height: f64,
    /// Distance between the left edges of neighbouring labels.
    pub h_pitch: f64,
    /// Distance between the top edges of neighbouring labels.
    pub v_pitch: f64,
}

/// A label's rectangle on the page: (x, y, width, height) in millimetres.
pub type Rect = (f64, f64, f64, f64);

impl LabelSheet {
    /// Rectangle of the label in `row`, `col` (0-based).
    pub fn rect(&self, row: u32, col: u32) -> Rect {
        (
            self.left + f64::from(col) * self.h_pitch,
            self.top + f64::from(row) * self.v_pitch,
            self.label_width,
            self.label_height,
        )
    }

    /// Labels per page.
    pub fn per_page(&self) -> usize {
        (self.rows * self.cols) as usize
    }

    /// Check the geometry: positive sizes, pitch at least the label size, every
    /// label on the page.
    pub fn validate(&self) -> Result<(), String> {
        let pos = [
            self.page_width,
            self.page_height,
            self.label_width,
            self.label_height,
        ];
        if pos.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || self.rows == 0
            || self.cols == 0
            || self.rows * self.cols > 400
            || self.top < 0.0
            || self.left < 0.0
        {
            return Err("label sheet: invalid size".into());
        }
        if (self.cols > 1 && self.h_pitch < self.label_width)
            || (self.rows > 1 && self.v_pitch < self.label_height)
        {
            return Err("label sheet: pitch smaller than the label".into());
        }
        let (x, y, w, h) = self.rect(self.rows - 1, self.cols - 1);
        if x + w > self.page_width + 1e-6 || y + h > self.page_height + 1e-6 {
            return Err("label sheet: labels do not fit on the page".into());
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn sheet(
    id: &str,
    page: (f64, f64),
    rows: u32,
    cols: u32,
    top: f64,
    left: f64,
    label: (f64, f64),
    pitch: (f64, f64),
) -> LabelSheet {
    LabelSheet {
        id: id.into(),
        page_width: page.0,
        page_height: page.1,
        rows,
        cols,
        top,
        left,
        label_width: label.0,
        label_height: label.1,
        h_pitch: pitch.0,
        v_pitch: pitch.1,
    }
}

/// BlueOffice label sheet presets (our own geometry).
pub fn label_presets() -> Vec<LabelSheet> {
    const A4: (f64, f64) = (210.0, 297.0);
    vec![
        sheet("a4-3x8", A4, 8, 3, 12.9, 7.25, (63.5, 33.9), (66.0, 33.9)),
        sheet("a4-2x7", A4, 7, 2, 15.5, 5.0, (99.0, 38.0), (101.0, 38.0)),
        sheet("a4-3x7", A4, 7, 3, 1.5, 0.0, (70.0, 42.0), (70.0, 42.0)),
        sheet("a4-2x4", A4, 4, 2, 14.5, 5.0, (99.0, 67.0), (101.0, 67.0)),
        sheet("a4-4x10", A4, 10, 4, 21.5, 8.0, (48.5, 25.4), (48.5, 25.4)),
        sheet(
            "letter-3x10",
            (215.9, 279.4),
            10,
            3,
            12.7,
            4.75,
            (66.7, 25.4),
            (69.85, 25.4),
        ),
    ]
}

/// An envelope size in millimetres (width ≥ height; printed landscape).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeSize {
    /// Preset id (`custom` for user sizes).
    pub id: String,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// Envelope sizes: ISO 269 DL, C6, C5, C4, ISO B5 and North American No. 10.
pub fn envelope_presets() -> Vec<EnvelopeSize> {
    [
        ("dl", 220.0, 110.0),
        ("c6", 162.0, 114.0),
        ("c5", 229.0, 162.0),
        ("c4", 324.0, 229.0),
        ("b5", 250.0, 176.0),
        ("no10", 241.3, 104.8),
    ]
    .into_iter()
    .map(|(id, width, height)| EnvelopeSize {
        id: id.into(),
        width,
        height,
    })
    .collect()
}

/// Atoms of a template line: `«Field»` becomes `MERGEFIELD Field` (§17.16.5.35).
pub fn template_runs(line: &str, props: &RunProps) -> Vec<Run> {
    let mut atoms = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find('«') {
        let after = &rest[i + '«'.len_utf8()..];
        let Some(j) = after.find('»') else { break };
        if i > 0 {
            atoms.push((props.clone(), RunContent::Text(rest[..i].to_owned())));
        }
        let name = after[..j].trim();
        if !name.is_empty() {
            let quoted = if name.contains(' ') {
                format!("\"{name}\"")
            } else {
                name.to_owned()
            };
            atoms.push((
                props.clone(),
                RunContent::Field {
                    instr: format!("MERGEFIELD {quoted} \\* MERGEFORMAT"),
                    cached: super::placeholder(name),
                    locked: false,
                    codes: false,
                },
            ));
        }
        rest = &after[j + '»'.len_utf8()..];
    }
    if !rest.is_empty() {
        atoms.push((props.clone(), RunContent::Text(rest.to_owned())));
    }
    crate::para::rebuild(atoms)
}

fn line_paragraphs(text: &str, props: &RunProps) -> Vec<Paragraph> {
    let pp = ParaProps {
        space_before: Some(Twips(0)),
        space_after: Some(Twips(0)),
        line_spacing: Some(LineSpacing::Auto(240)),
        ..ParaProps::default()
    };
    let lines: Vec<&str> = text.lines().collect();
    let lines = if lines.is_empty() { vec![""] } else { lines };
    lines
        .into_iter()
        .map(|l| Paragraph::new(pp.clone(), template_runs(l, props)))
        .collect()
}

/// A tiny paragraph after a table (a section must end with a paragraph): 1 pt exact.
fn end_paragraph() -> Paragraph {
    Paragraph::new(
        ParaProps {
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            line_spacing: Some(LineSpacing::Exact(Twips(20))),
            ..ParaProps::default()
        },
        Vec::new(),
    )
}

fn cell(width: Twips, blocks: Vec<Block>) -> Cell {
    Cell {
        props: CellProps {
            width: Some(TableWidth::Dxa(width)),
            ..CellProps::default()
        },
        blocks,
    }
}

/// How a label sheet is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LabelMode {
    /// One record per label: every label after the first starts with NEXT (§17.16.5.38).
    PerRecord,
    /// Every label of a page shows the same record (or the same fixed text).
    Same,
}

/// A label sheet document: one page of labels showing `text` (lines; `«Field»` is a
/// merge field).
pub fn labels_document(s: &LabelSheet, text: &str, mode: LabelMode) -> Result<Document, String> {
    s.validate()?;
    let props = RunProps::default();
    let hgap = (s.h_pitch - s.label_width).max(0.0);
    let vgap = (s.v_pitch - s.label_height).max(0.0);
    let (lw, gw) = (mm(s.label_width), mm(hgap));
    let mut grid = Vec::new();
    for c in 0..s.cols {
        if c > 0 && gw.0 > 0 {
            grid.push(gw);
        }
        grid.push(lw);
    }
    let table_w = grid.iter().map(|t| t.0).sum::<i32>();
    let mut rows = Vec::new();
    let mut first = true;
    for r in 0..s.rows {
        if r > 0 && vgap > 0.0 {
            rows.push(Row {
                props: RowProps {
                    height: Some((mm(vgap), HeightRule::Exact)),
                    cant_split: true,
                    ..RowProps::default()
                },
                cells: grid
                    .iter()
                    .map(|w| cell(*w, vec![Block::Paragraph(end_paragraph())]))
                    .collect(),
            });
        }
        let mut cells = Vec::new();
        for c in 0..s.cols {
            if c > 0 && gw.0 > 0 {
                cells.push(cell(gw, vec![Block::Paragraph(end_paragraph())]));
            }
            let mut paras = line_paragraphs(text, &props);
            if mode == LabelMode::PerRecord && !first {
                if let Some(p) = paras.first_mut() {
                    let field = Run {
                        props: props.clone(),
                        content: vec![RunContent::Field {
                            instr: "NEXT".into(),
                            cached: String::new(),
                            locked: false,
                            codes: false,
                        }],
                    };
                    p.runs.insert(0, field);
                }
            }
            first = false;
            cells.push(cell(lw, paras.into_iter().map(Block::Paragraph).collect()));
        }
        rows.push(Row {
            props: RowProps {
                height: Some((mm(s.label_height), HeightRule::Exact)),
                cant_split: true,
                ..RowProps::default()
            },
            cells,
        });
    }
    let table = Table {
        props: TableProps {
            width: Some(TableWidth::Dxa(Twips(table_w))),
            fixed_layout: true,
            // Text inside a label keeps 2 mm from its left and right edges.
            cell_margin: Some((mm(2.0), mm(2.0))),
            ..TableProps::default()
        },
        grid,
        rows,
        ..Table::default()
    };
    let (pw, ph) = (mm(s.page_width), mm(s.page_height));
    let (top, left) = (mm(s.top), mm(s.left));
    let page = PageSetup {
        width: pw,
        height: ph,
        margin_top: top,
        margin_bottom: Twips(0),
        margin_left: left,
        margin_right: Twips((pw.0 - left.0 - table_w).max(0)),
        header: Twips(0),
        footer: Twips(0),
    };
    Ok(Document {
        sections: vec![Section {
            page,
            ..Section::new(vec![Block::Table(table), Block::Paragraph(end_paragraph())])
        }],
        ..Document::default()
    })
}

/// Envelope margins (return address distance from the edges), mm.
pub const ENVELOPE_MARGIN: f64 = 10.0;

/// Default delivery address position of an envelope (our choice): 40 % across,
/// 45 % down, mm.
pub fn delivery_default(size: &EnvelopeSize) -> (f64, f64) {
    let (w, h) = (size.width.max(size.height), size.width.min(size.height));
    (
        (w * 0.4 * 2.0).round() / 2.0,
        (h * 0.45 * 2.0).round() / 2.0,
    )
}

/// An envelope document: landscape page of `size`, the return address at the top-left
/// margins and the delivery address (lines; `«Field»` is a merge field) in a floating
/// table at `at` (mm from the page's top-left corner; default [`delivery_default`]).
pub fn envelope_document(
    size: &EnvelopeSize,
    delivery: &str,
    return_address: &str,
    at: Option<(f64, f64)>,
) -> Result<Document, String> {
    let (w, h) = (size.width.max(size.height), size.width.min(size.height));
    if !(w.is_finite() && h > 20.0 && w < 1000.0) {
        return Err("envelope: invalid size".into());
    }
    let (x, y) = at.unwrap_or_else(|| delivery_default(size));
    let width = w - x - ENVELOPE_MARGIN;
    if x < 0.0 || y < 0.0 || width < 20.0 || y >= h - ENVELOPE_MARGIN {
        return Err("envelope: delivery address outside the envelope".into());
    }
    let props = RunProps::default();
    let mut blocks: Vec<Block> = if return_address.trim().is_empty() {
        Vec::new()
    } else {
        line_paragraphs(return_address, &props)
            .into_iter()
            .map(Block::Paragraph)
            .collect()
    };
    let address = Table {
        props: TableProps {
            width: Some(TableWidth::Dxa(mm(width))),
            fixed_layout: true,
            cell_margin: Some((Twips(0), Twips(0))),
            // §17.4.58 tblpPr: positioned from the page's top-left corner.
            float: Some(TableFloat {
                horz_anchor: Some("page".into()),
                vert_anchor: Some("page".into()),
                x: mm(x),
                y: mm(y),
                ..TableFloat::default()
            }),
            ..TableProps::default()
        },
        grid: vec![mm(width)],
        rows: vec![Row {
            props: RowProps::default(),
            cells: vec![cell(
                mm(width),
                line_paragraphs(delivery, &props)
                    .into_iter()
                    .map(Block::Paragraph)
                    .collect(),
            )],
        }],
        ..Table::default()
    };
    blocks.push(Block::Table(address));
    blocks.push(Block::Paragraph(end_paragraph()));
    let m = mm(ENVELOPE_MARGIN);
    Ok(Document {
        sections: vec![Section {
            page: PageSetup {
                width: mm(w),
                height: mm(h),
                margin_top: m,
                margin_bottom: m,
                margin_left: m,
                margin_right: m,
                header: Twips(0),
                footer: Twips(0),
            },
            ..Section::new(blocks)
        }],
        ..Document::default()
    })
}
