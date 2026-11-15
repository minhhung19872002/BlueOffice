//! Form field recognition (BlueEditor Form › Recognize): fields a printed form asks for,
//! found from the page itself — runs of underscores or dot leaders become text fields
//! sitting on them, small empty squares become check boxes, and empty boxes of text height
//! become text fields. Places already covered by a widget are skipped. Our own rules.
use crate::objects::ObjectKind;
use crate::{NewField, NewFieldKind, PdfDocument};

/// Name for a field from the label before it: letters and digits, at most 40 characters.
fn label_name(label: &str) -> String {
    let t = label
        .trim()
        .trim_end_matches([':', '.', '…', '_', ' '])
        .trim();
    // The words just before the blank.
    let words: Vec<&str> = t.split_whitespace().collect();
    let tail = words[words.len().saturating_sub(4)..].join(" ");
    tail.chars().take(40).collect()
}

fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    let w = a[2].min(b[2]) - a[0].max(b[0]);
    let h = a[3].min(b[3]) - a[1].max(b[1]);
    w > 0.0
        && h > 0.0
        && w * h > 0.3 * ((a[2] - a[0]) * (a[3] - a[1])).min((b[2] - b[0]) * (b[3] - b[1]))
}

impl PdfDocument {
    /// Fields recognised on page `page` (not added yet; names are not unique yet).
    pub fn recognize_fields(&mut self, page: usize) -> Result<Vec<NewField>, String> {
        let objs = self.page_objects(page)?;
        let taken: Vec<[f32; 4]> = self
            .form_fields()
            .into_iter()
            .filter(|f| f.page == page)
            .map(|f| f.rect)
            .collect();
        let mut out: Vec<NewField> = Vec::new();
        let field = |kind, rect, name: String| NewField {
            kind,
            page,
            rect,
            name,
            tooltip: String::new(),
            options: Vec::new(),
            required: false,
            read_only: false,
        };
        // Blanks drawn with underscores or dot leaders.
        for line in crate::text_lines(&objs) {
            let chars: Vec<char> = line.text.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let c = chars[i];
                if c != '_' && c != '.' && c != '…' {
                    i += 1;
                    continue;
                }
                let mut j = i;
                while j < chars.len() && chars[j] == c {
                    j += 1;
                }
                let enough = if c == '_' {
                    j - i >= 3
                } else {
                    j - i >= 5 || (c == '…' && j - i >= 2)
                };
                if enough {
                    let boxes = line.char_boxes.get(i..j).unwrap_or(&[]);
                    if let (Some(a), Some(b)) = (boxes.first(), boxes.last()) {
                        let bottom = a[3].max(b[3]);
                        let h = (line.font_size * 1.3).clamp(10.0, 28.0);
                        let label: String = chars[..i].iter().collect();
                        let rect = [a[0], bottom - h, b[2], bottom + 1.0];
                        out.push(field(NewFieldKind::Text, rect, label_name(&label)));
                    }
                }
                i = j;
            }
        }
        // Empty boxes drawn as paths.
        let text_boxes: Vec<[f32; 4]> = objs
            .iter()
            .filter(|o| o.kind == ObjectKind::Text && !o.text.trim().is_empty())
            .map(|o| o.bbox)
            .collect();
        for o in objs.iter().filter(|o| o.kind == ObjectKind::Path) {
            let b = o.bbox;
            let (w, h) = (b[2] - b[0], b[3] - b[1]);
            let inner = [b[0] + 1.0, b[1] + 1.0, b[2] - 1.0, b[3] - 1.0];
            let empty = !text_boxes.iter().any(|t| overlaps(*t, inner));
            if !empty {
                continue;
            }
            let kind =
                if (6.0..=20.0).contains(&w) && (6.0..=20.0).contains(&h) && (w - h).abs() < 2.5 {
                    NewFieldKind::Check
                } else if w >= 40.0 && (12.0..=40.0).contains(&h) && w < 500.0 {
                    NewFieldKind::Text
                } else {
                    continue;
                };
            // The label: the nearest text on the left on the same line.
            let label = objs
                .iter()
                .filter(|t| {
                    t.kind == ObjectKind::Text
                        && t.bbox[2] <= b[0] + 1.0
                        && t.bbox[3] > b[1]
                        && t.bbox[1] < b[3]
                })
                .max_by(|x, y| x.bbox[2].total_cmp(&y.bbox[2]))
                .map(|t| t.text.clone())
                .unwrap_or_default();
            out.push(field(
                kind,
                [b[0] + 1.0, b[1] + 1.0, b[2] - 1.0, b[3] - 1.0],
                label_name(&label),
            ));
        }
        // Drop what an existing widget covers and repeats (a box drawn twice).
        let mut kept: Vec<NewField> = Vec::new();
        for f in out {
            if taken.iter().any(|t| overlaps(*t, f.rect))
                || kept.iter().any(|k| overlaps(k.rect, f.rect))
            {
                continue;
            }
            kept.push(f);
        }
        Ok(kept)
    }

    /// Recognise and add the fields of `pages` (all when empty) as one undo step; returns
    /// how many were added.
    pub fn add_recognized_fields(&mut self, pages: &[usize]) -> Result<usize, String> {
        let list: Vec<usize> = if pages.is_empty() {
            (0..self.pages.len()).collect()
        } else {
            pages.to_vec()
        };
        let mut found = Vec::new();
        for p in list {
            found.extend(self.recognize_fields(p)?);
        }
        if found.is_empty() {
            return Ok(0);
        }
        self.begin_group("recognize fields");
        let mut n = 0;
        let mut err = None;
        for mut f in found {
            let prefix = match f.kind {
                NewFieldKind::Check => "Check Box",
                _ => "Text",
            };
            let base = if f.name.is_empty() {
                prefix.to_owned()
            } else {
                f.name.clone()
            };
            f.name = if self.form_fields().iter().any(|x| x.name == base) {
                self.free_field_name(&format!("{base} "))
            } else {
                base
            };
            match self.add_field(&f) {
                Ok(()) => n += 1,
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        self.end_group();
        match err {
            Some(e) if n == 0 => Err(e),
            _ => Ok(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standard_fonts;

    #[test]
    fn blanks_and_boxes_become_fields() {
        let content = "BT /F1 12 Tf 40 250 Td (Name: ______________) Tj ET \
                       BT /F1 12 Tf 40 200 Td (Agree) Tj ET 100 198 10 10 re S \
                       40 140 200 20 re S \
                       BT /F1 12 Tf 40 100 Td (Boxed text) Tj ET 35 95 120 20 re S";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        ];
        let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
        let mut d =
            PdfDocument::open(crate::tests::build_pdf(&refs), standard_fonts()).expect("opens");
        let found = d.recognize_fields(0).expect("recognizes");
        let kinds: Vec<NewFieldKind> = found.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            [NewFieldKind::Text, NewFieldKind::Check, NewFieldKind::Text],
            "{found:?}"
        );
        assert_eq!(found[0].name, "Name");
        assert_eq!(found[1].name, "Agree");
        assert_eq!(d.add_recognized_fields(&[]).expect("adds"), 3);
        assert_eq!(d.form_fields().len(), 3);
        // Already covered places are not found again; one undo removes them all.
        assert_eq!(d.recognize_fields(0).expect("again").len(), 0);
        assert!(d.undo());
        assert_eq!(d.form_fields().len(), 0);
    }
}
