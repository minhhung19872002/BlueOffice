//! Shapes and text boxes (ECMA-376 Part 1, §20.4 drawing objects with
//! [MS-ODRAWXML] `wps:wsp`; §17.17.1 txbxContent): insertion, move/resize of
//! anchored drawing objects, and replacing the content of a text box.
//!
//! A drawing object is addressed by its body paragraph id and the index-text offset
//! of its object character (the `at` of `bo_layout::LineSource::TextBox`).

use crate::{para, para_clone, path_of, EditError, Result, Selection, Session};
use bo_model::index::para_index;
use bo_model::{
    Anchor, Block, Color, Emu, Image, Paragraph, RunContent, Shape, ShapeLine, TextAnchor, Wrap,
};

/// Kind of shape inserted by [`Session::insert_shape`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeSpec {
    /// §20.1.10.56 `ST_ShapeType` preset (`rect`, `ellipse`, `roundRect`, …).
    pub preset: String,
    /// Insert a text box (white, thin black outline, empty paragraph).
    #[serde(default)]
    pub text_box: bool,
}

/// Fill of new shapes (BlueOffice palette, not a theme colour).
const SHAPE_FILL: Color = Color {
    r: 0x2E,
    g: 0x75,
    b: 0xB6,
};
const SHAPE_LINE: Color = Color {
    r: 0x1F,
    g: 0x4E,
    b: 0x79,
};

fn shape_of(spec: &ShapeSpec) -> Shape {
    if spec.text_box {
        Shape {
            preset: "rect".into(),
            fill: Some(Color::WHITE),
            line: Some(ShapeLine {
                width: Emu(9525),
                color: Color::BLACK,
                dash: None,
            }),
            text: Some(vec![Block::Paragraph(Paragraph::default())]),
            insets: Shape::INSETS,
            anchor: TextAnchor::Top,
            ..Shape::default()
        }
    } else {
        Shape {
            preset: if bo_drawingml_known(&spec.preset) {
                spec.preset.clone()
            } else {
                "rect".into()
            },
            fill: Some(SHAPE_FILL),
            line: Some(ShapeLine {
                width: Emu(12_700),
                color: SHAPE_LINE,
                dash: None,
            }),
            insets: Shape::INSETS,
            anchor: TextAnchor::Center,
            ..Shape::default()
        }
    }
}

/// Preset names accepted for new shapes (all defined by §20.1.10.56; drawn by the
/// preset geometry engine).
fn bo_drawingml_known(name: &str) -> bool {
    !name.is_empty() && name.len() < 40 && name.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The drawing object at index offset `at` of `p`: (run index, item index).
fn locate(p: &Paragraph, at: usize) -> Option<(usize, usize)> {
    let (_, spans) = para_index(p);
    spans.into_iter().find_map(|s| {
        (s.start == at && matches!(p.runs[s.run].content[s.item], RunContent::Image(_)))
            .then_some((s.run, s.item))
    })
}

impl Session {
    /// The drawing object at index offset `at` of body paragraph `para`.
    pub fn drawing(&self, para: usize, at: usize) -> Option<&Image> {
        let paths = bo_model::index::body_paragraphs(&self.doc);
        let path = paths.get(para)?;
        let p = bo_model::index::para_at(&self.doc, path)?;
        let (r, i) = locate(p, at)?;
        match &p.runs[r].content[i] {
            RunContent::Image(img) => Some(img),
            _ => None,
        }
    }

    /// Change the drawing object at (`para`, `at`) with `f` (one undo step).
    pub fn edit_drawing(
        &mut self,
        para: usize,
        at: usize,
        f: impl FnOnce(&mut Image),
    ) -> Result<()> {
        let sel = self.selection;
        self.edit(|tx, _| {
            let paths = tx.paths();
            let path = path_of(tx.doc, &paths, para)?;
            let mut p = para_clone(tx.doc, &path)?;
            let (r, i) = locate(&p, at)
                .ok_or_else(|| EditError::Unsupported(format!("no drawing at {para}:{at}")))?;
            if let RunContent::Image(img) = &mut p.runs[r].content[i] {
                f(img);
            }
            tx.replace_para(&path, p)?;
            Ok(sel)
        })?;
        Ok(())
    }

    /// Show `blocks` as the content of the text box at (`para`, `at`) without history
    /// (live display while it is edited; [`Session::set_text_box`] records it).
    pub fn preview_text_box(&mut self, para: usize, at: usize, blocks: Vec<Block>) {
        let paths = bo_model::index::body_paragraphs(&self.doc);
        let Some(path) = paths.get(para) else { return };
        let Some(p) = bo_model::index::para_at_mut(&mut self.doc, path) else {
            return;
        };
        let Some((r, i)) = locate(p, at) else { return };
        if let RunContent::Image(img) = &mut p.runs[r].content[i] {
            if let Some(s) = img.shape.as_mut() {
                s.text = Some(blocks);
                self.version += 1;
                // Not an op: mark the top-level block for the incremental layout.
                if let [si, bi, ..] = path[..] {
                    self.changes.splice(si, bi, 1, 1);
                }
            }
        }
    }

    /// Replace the content of the text box at (`para`, `at`) (one undo step).
    pub fn set_text_box(&mut self, para: usize, at: usize, blocks: Vec<Block>) -> Result<()> {
        self.edit_drawing(para, at, |img| {
            if let Some(s) = img.shape.as_mut() {
                s.text = Some(blocks);
            }
        })
    }

    /// Move an anchored drawing object by (`dx`, `dy`) EMU and give it the size
    /// `width` × `height` EMU (`None`: unchanged). Inline objects are only resized.
    pub fn set_drawing_frame(
        &mut self,
        para: usize,
        at: usize,
        delta: (i64, i64),
        size: Option<(i64, i64)>,
    ) -> Result<()> {
        self.edit_drawing(para, at, |img| {
            if let Some(a) = img.anchor.as_mut() {
                // §20.4.2.12 posOffset replaces an alignment once the object is moved.
                if delta.0 != 0 {
                    a.h_align = None;
                    a.x = Emu(a.x.0 + delta.0);
                }
                if delta.1 != 0 {
                    a.v_align = None;
                    a.y = Emu(a.y.0 + delta.1);
                }
            }
            if let Some((w, h)) = size {
                img.width = Emu(w.max(12_700));
                img.height = Emu(h.max(12_700));
            }
        })
    }

    /// Insert a shape or text box anchored to the caret's paragraph (§20.4.2.3
    /// anchor: column/paragraph relative, in front of text, square wrapping for a
    /// text box), `width` × `height` EMU. Returns the paragraph id and offset of the
    /// new object.
    pub fn insert_shape(
        &mut self,
        spec: &ShapeSpec,
        width: i64,
        height: i64,
    ) -> Result<(usize, usize)> {
        let shape = shape_of(spec);
        let wrap = if spec.text_box {
            Wrap::Square
        } else {
            Wrap::None
        };
        let mut placed = (0, 0);
        self.edit(|tx, sel| {
            let (a, b) = sel.ordered();
            let at = tx.delete_range(a, b)?;
            let (_, _, atoms) = tx.atoms(at.para)?;
            let props = para::props_at(&atoms, at.offset);
            let img = Image {
                media: String::new(),
                width: Emu(width.max(12_700)),
                height: Emu(height.max(12_700)),
                anchor: Some(Anchor {
                    h_relative: "column".into(),
                    v_relative: "paragraph".into(),
                    wrap,
                    dist: [Emu(0), Emu(0), Emu(114_300), Emu(114_300)],
                    ..Anchor::default()
                }),
                crop: None,
                rotation: 0,
                description: String::new(),
                shape: Some(Box::new(shape)),
                format: None,
                src: None,
                ole: None,
            };
            placed = (at.para, at.offset);
            let caret = tx.insert_atoms(at, vec![(props, RunContent::Image(Box::new(img)))])?;
            Ok(Selection::caret(caret))
        })?;
        Ok(placed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pos;
    use bo_model::{Document, ParaProps, Run, RunProps, Section};

    fn session(text: &str) -> Session {
        let doc = Document {
            sections: vec![Section::new(vec![Block::Paragraph(Paragraph::new(
                ParaProps::default(),
                vec![Run::text(text, RunProps::default())],
            ))])],
            ..Document::default()
        };
        Session::new(doc)
    }

    #[test]
    fn insert_move_resize_and_fill_a_text_box() {
        let mut s = session("Xin chào");
        s.set_selection(Selection::caret(Pos::new(0, 3)));
        let spec = ShapeSpec {
            preset: "rect".into(),
            text_box: true,
        };
        let (para, at) = s.insert_shape(&spec, 1_270_000, 635_000).unwrap();
        assert_eq!((para, at), (0, 3));
        let img = s.drawing(0, 3).unwrap();
        assert!(img.shape.as_ref().unwrap().text.is_some());
        assert_eq!(img.anchor.as_ref().unwrap().wrap, Wrap::Square);

        s.set_drawing_frame(0, 3, (12_700, -12_700), Some((2_540_000, 1_270_000)))
            .unwrap();
        let img = s.drawing(0, 3).unwrap();
        let a = img.anchor.as_ref().unwrap();
        assert_eq!((a.x.0, a.y.0, img.width.0), (12_700, -12_700, 2_540_000));

        let blocks = vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run::text("Nội dung", RunProps::default())],
        ))];
        s.set_text_box(0, 3, blocks.clone()).unwrap();
        let t = s
            .drawing(0, 3)
            .unwrap()
            .shape
            .as_ref()
            .unwrap()
            .text
            .clone();
        assert_eq!(t, Some(blocks));
        // Text, frame and insertion are three undo steps.
        assert!(s.undo());
        assert!(s.undo());
        assert!(s.undo());
        assert!(s.drawing(0, 3).is_none());
        assert!(s.set_text_box(0, 3, Vec::new()).is_err());
    }
}
