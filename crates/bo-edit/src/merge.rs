//! Mail merge (ECMA-376 Part 1, §17.16.5.35 MERGEFIELD; §17.14 mail merge):
//! filling merge fields from one data record, and producing one merged
//! document with a section per record.

use crate::fields::{field_args, field_kind};
use bo_model::{Block, Document, Paragraph, Run, RunContent, Section, SectionBreak};

/// Recipient data: column names and records (all values as display text).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeData {
    /// Data source name (file name).
    pub name: String,
    pub fields: Vec<String>,
    #[serde(skip)]
    pub rows: Vec<Vec<String>>,
}

impl MergeData {
    /// Value of column `field` (case-insensitive) in record `row`.
    pub fn value(&self, row: usize, field: &str) -> Option<&str> {
        let col = self.fields.iter().position(|f| {
            f.trim().eq_ignore_ascii_case(field.trim()) || f.trim() == field.trim()
        })?;
        self.rows.get(row)?.get(col).map(String::as_str)
    }
}

/// Column name of a MERGEFIELD instruction (`MERGEFIELD "Họ tên" \* MERGEFORMAT`).
pub fn merge_field_name(instr: &str) -> Option<String> {
    (field_kind(instr) == "MERGEFIELD")
        .then(|| field_args(instr).get(1).cloned())
        .flatten()
}

/// Display placeholder of an unmerged field: `«name»`.
pub fn placeholder(name: &str) -> String {
    format!("«{name}»")
}

/// Result text of a MERGEFIELD for a value, applying `\b` / `\f` (text before /
/// after a non-empty value) and the case switches of §17.16.4.3.
pub fn merge_text(instr: &str, value: &str) -> String {
    let args = field_args(instr);
    let switch = |s: &str| {
        args.iter()
            .position(|a| a.eq_ignore_ascii_case(s))
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let mut v = match switch("\\*").as_deref() {
        Some(f) if f.eq_ignore_ascii_case("Upper") => value.to_uppercase(),
        Some(f) if f.eq_ignore_ascii_case("Lower") => value.to_lowercase(),
        Some(f) if f.eq_ignore_ascii_case("Caps") => value
            .split(' ')
            .map(|w| {
                let mut c = w.chars();
                c.next().map_or_else(String::new, |f| {
                    f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
                })
            })
            .collect::<Vec<_>>()
            .join(" "),
        Some(f) if f.eq_ignore_ascii_case("FirstCap") => {
            let mut c = value.chars();
            c.next().map_or_else(String::new, |f| {
                f.to_uppercase().collect::<String>() + c.as_str()
            })
        }
        _ => value.to_owned(),
    };
    if !v.is_empty() {
        if let Some(b) = switch("\\b") {
            v = b + &v;
        }
        if let Some(f) = switch("\\f") {
            v.push_str(&f);
        }
    }
    v
}

fn map_blocks(blocks: &mut [Block], f: &mut dyn FnMut(&mut Paragraph)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => f(p),
            Block::Table(t) => {
                for row in &mut t.rows {
                    for c in &mut row.cells {
                        map_blocks(&mut c.blocks, f);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

fn map_doc(doc: &mut Document, f: &mut dyn FnMut(&mut Paragraph)) {
    for s in &mut doc.sections {
        map_blocks(&mut s.blocks, f);
        for hf in [&mut s.headers, &mut s.footers] {
            for b in [&mut hf.default, &mut hf.first, &mut hf.even]
                .into_iter()
                .flatten()
            {
                map_blocks(b, f);
            }
        }
    }
}

/// Copy of `doc` showing record `row` in its merge fields (fields are kept; used
/// for the preview). `None` shows the `«name»` placeholders.
pub fn preview(doc: &Document, data: &MergeData, row: Option<usize>) -> Document {
    let mut d = doc.clone();
    map_doc(&mut d, &mut |p| {
        for r in &mut p.runs {
            for c in &mut r.content {
                if let RunContent::Field { instr, cached } = c {
                    if let Some(name) = merge_field_name(instr) {
                        *cached = match row {
                            Some(i) => merge_text(instr, data.value(i, &name).unwrap_or("")),
                            None => placeholder(&name),
                        };
                    }
                }
            }
        }
    });
    d
}

/// Replace merge fields of a paragraph with plain text of record `row`.
fn fill(p: &mut Paragraph, data: &MergeData, row: usize) {
    let mut changed = false;
    let runs: Vec<Run> = std::mem::take(&mut p.runs)
        .into_iter()
        .map(|mut r| {
            r.content = r
                .content
                .into_iter()
                .filter_map(|c| match c {
                    RunContent::Field { instr, cached } => match merge_field_name(&instr) {
                        Some(name) => {
                            changed = true;
                            let v = merge_text(&instr, data.value(row, &name).unwrap_or(""));
                            (!v.is_empty()).then_some(RunContent::Text(v))
                        }
                        None => Some(RunContent::Field { instr, cached }),
                    },
                    // Comments of the template are not copied into the letters.
                    RunContent::CommentStart(_)
                    | RunContent::CommentEnd(_)
                    | RunContent::CommentRef(_) => {
                        changed = true;
                        None
                    }
                    other => Some(other),
                })
                .collect();
            r
        })
        .collect();
    p.runs = runs;
    if changed {
        p.src = None;
    }
}

/// One document with the template repeated for every record (each record starts
/// on a new page, §17.6.22 nextPage).
pub fn merge_all(doc: &Document, data: &MergeData) -> Document {
    let mut out = doc.clone();
    out.sections.clear();
    out.comments.clear();
    for row in 0..data.rows.len() {
        let mut copy = doc.clone();
        map_doc(&mut copy, &mut |p| fill(p, data, row));
        for (i, mut s) in copy.sections.into_iter().enumerate() {
            if row > 0 && i == 0 && s.break_type == SectionBreak::Continuous {
                s.break_type = SectionBreak::NextPage;
            }
            // Filled headers/footers need their own parts: regenerate the sectPr.
            if let Some(orig) = doc.sections.get(i) {
                if orig.headers != s.headers || orig.footers != s.footers {
                    s.src = None;
                }
            }
            // The section's last paragraph carries its sectPr: regenerate it.
            if let Some(Block::Paragraph(p)) = s.blocks.last_mut() {
                p.src = None;
            }
            out.sections.push(Section { ..s });
        }
    }
    if out.sections.is_empty() {
        out.sections = doc.sections.clone();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{ParaProps, RunProps};

    fn template() -> Document {
        let f = |name: &str, extra: &str| RunContent::Field {
            instr: format!("MERGEFIELD {name}{extra} \\* MERGEFORMAT"),
            cached: placeholder(name),
        };
        Document {
            sections: vec![Section::new(vec![Block::Paragraph(Paragraph::new(
                ParaProps::default(),
                vec![Run {
                    props: RunProps::default(),
                    content: vec![
                        RunContent::Text("Kính gửi: ".into()),
                        f("\"Họ tên\"", ""),
                        f("ChucVu", " \\b \", \""),
                    ],
                }],
            ))])],
            ..Document::default()
        }
    }

    fn data() -> MergeData {
        MergeData {
            name: "ds.csv".into(),
            fields: vec!["Họ tên".into(), "ChucVu".into()],
            rows: vec![
                vec!["Nguyễn Văn An".into(), "Giám đốc".into()],
                vec!["Trần Thị Bình".into(), String::new()],
            ],
        }
    }

    #[test]
    fn names_switches_preview_and_merge() {
        assert_eq!(
            merge_field_name("MERGEFIELD \"Họ tên\" \\* MERGEFORMAT").as_deref(),
            Some("Họ tên")
        );
        assert_eq!(merge_text("MERGEFIELD x \\* Upper", "abc"), "ABC");
        assert_eq!(
            merge_text("MERGEFIELD x \\* Caps", "nguyễn văn"),
            "Nguyễn Văn"
        );
        assert_eq!(merge_text("MERGEFIELD x \\b \"(\" \\f \")\"", "a"), "(a)");
        assert_eq!(merge_text("MERGEFIELD x \\b \"(\"", ""), "");
        let t = template();
        let d = data();
        assert_eq!(
            preview(&t, &d, Some(0)).plain_text(),
            "Kính gửi: Nguyễn Văn An, Giám đốc"
        );
        assert_eq!(
            preview(&t, &d, None).plain_text(),
            "Kính gửi: «Họ tên»«ChucVu»"
        );
        let m = merge_all(&t, &d);
        assert_eq!(m.sections.len(), 2);
        assert_eq!(
            m.plain_text(),
            "Kính gửi: Nguyễn Văn An, Giám đốc\nKính gửi: Trần Thị Bình"
        );
        assert_eq!(m.sections[1].break_type, SectionBreak::NextPage);
    }
}
