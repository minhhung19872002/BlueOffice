//! Editing existing comments (ISO 32000-1, §12.5.6): moving one, changing its colour, reply
//! threads (`/IRT`, §12.5.6.2) and listing comments with their replies. Deleting and changing
//! the text are in `annot.rs`.
use crate::annot::{Annot, AnnotKind, NewAnnot};
use crate::interp::{apply, invert, M};
use crate::{resolve, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId};

/// A comment with its replies (oldest first).
#[derive(Debug, Clone, PartialEq)]
pub struct CommentThread {
    /// The comment.
    pub comment: Annot,
    /// Replies to it and to its replies, in modification order.
    pub replies: Vec<Annot>,
}

fn shifted(o: &Object, dx: f32, dy: f32) -> Option<Object> {
    // Arrays of x y pairs (Rect, QuadPoints, Vertices, L) move by (dx, dy).
    let a = o.as_array().ok()?;
    let mut out = Vec::with_capacity(a.len());
    for (i, v) in a.iter().enumerate() {
        let f = v.as_float().ok()?;
        let d = if i % 2 == 0 { dx } else { dy };
        out.push(Object::Real(((f + d) * 1000.0).round() / 1000.0));
    }
    Some(Object::Array(out))
}

impl PdfDocument {
    fn annot_page_base(&self, page: usize) -> Option<M> {
        let info = self.pages.get(page)?;
        Some(crate::page_matrix(info.crop, info.rotate))
    }

    /// Move a comment by (`dx`, `dy`) page points (y down, as [`Annot::rect`]); its appearance
    /// follows its rectangle.
    pub fn move_annotation(&mut self, a: &Annot, dx: f32, dy: f32) -> Result<(), String> {
        let id = a.id.ok_or("the comment cannot be changed")?;
        let base = self.annot_page_base(a.page).ok_or("no such page")?;
        let inv = invert(base).ok_or("page matrix")?;
        // The page-space offset in user space (linear part only).
        let (ox, oy) = apply(inv, 0.0, 0.0);
        let (px, py) = apply(inv, dx, dy);
        let (ux, uy) = (px - ox, py - oy);
        if !(ux.is_finite() && uy.is_finite()) {
            return Err("offset".into());
        }
        let popup = self
            .doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Popup").ok())
            .and_then(|p| p.as_reference().ok());
        self.begin("move comment");
        for target in std::iter::once(id).chain(popup) {
            self.touch(target);
            if let Ok(d) = self.doc.get_dictionary_mut(target) {
                for key in [&b"Rect"[..], b"QuadPoints", b"Vertices", b"L", b"CL"] {
                    let moved = d.get(key).ok().and_then(|o| shifted(o, ux, uy));
                    if let Some(m) = moved {
                        d.set(key, m);
                    }
                }
                let ink = d
                    .get(b"InkList")
                    .ok()
                    .and_then(|o| o.as_array().ok())
                    .map(|a| {
                        a.iter()
                            .map(|s| shifted(s, ux, uy).unwrap_or_else(|| s.clone()))
                            .collect::<Vec<_>>()
                    });
                if let Some(ink) = ink {
                    d.set("InkList", ink);
                }
                d.set("M", crate::annot::text_obj(&crate::annot::pdf_date()));
            }
        }
        self.commit();
        Ok(())
    }

    /// Change a comment's colour (`/C`) and draw its appearance again in that colour.
    pub fn set_annotation_color(&mut self, a: &Annot, color: [u8; 3]) -> Result<(), String> {
        let id = a.id.ok_or("the comment cannot be changed")?;
        let [r, g, b] = color.map(|c| f32::from(c) / 255.0);
        self.begin("comment colour");
        self.touch(id);
        let (rect, quads, kind_markup) = {
            let d = self.doc.get_dictionary_mut(id).map_err(|e| e.to_string())?;
            d.set("C", crate::annot::nums(&[r, g, b]));
            d.set("M", crate::annot::text_obj(&crate::annot::pdf_date()));
            let rect: Vec<f32> = d
                .get(b"Rect")
                .ok()
                .and_then(|o| o.as_array().ok())
                .map(|v| v.iter().filter_map(|o| o.as_float().ok()).collect())
                .unwrap_or_default();
            let quads: Vec<f32> = d
                .get(b"QuadPoints")
                .ok()
                .and_then(|o| o.as_array().ok())
                .map(|v| v.iter().filter_map(|o| o.as_float().ok()).collect())
                .unwrap_or_default();
            (
                rect,
                quads,
                matches!(
                    a.kind,
                    AnnotKind::Highlight
                        | AnnotKind::Underline
                        | AnnotKind::StrikeOut
                        | AnnotKind::Squiggly
                        | AnnotKind::Square
                        | AnnotKind::Circle
                        | AnnotKind::Note
                ),
            )
        };
        // A new appearance for the kinds this reader draws itself; others keep theirs (with
        // the new /C for viewers that draw from it).
        if kind_markup {
            if let Ok(rect) = <[f32; 4]>::try_from(rect.as_slice()) {
                let rect = [
                    rect[0].min(rect[2]),
                    rect[1].min(rect[3]),
                    rect[0].max(rect[2]),
                    rect[1].max(rect[3]),
                ];
                let mut probe = Dictionary::new();
                probe.set(
                    "Subtype",
                    Object::Name(a.kind.subtype().as_bytes().to_vec()),
                );
                probe.set("C", crate::annot::nums(&[r, g, b]));
                if !quads.is_empty() {
                    probe.set("QuadPoints", crate::annot::nums(&quads));
                }
                if let Some((content, alpha)) =
                    crate::annot::fallback_appearance(&self.doc, &probe, rect)
                {
                    let mut res = Dictionary::new();
                    let mut content = content;
                    if a.kind == AnnotKind::Highlight {
                        let mut gs = Dictionary::new();
                        gs.set("Type", Object::Name(b"ExtGState".to_vec()));
                        gs.set("BM", Object::Name(b"Multiply".to_vec()));
                        gs.set("ca", Object::Real(alpha.max(0.4)));
                        let mut ext = Dictionary::new();
                        ext.set("GS0", gs);
                        res.set("ExtGState", ext);
                        let mut c = b"/GS0 gs\n".to_vec();
                        c.extend_from_slice(&content);
                        content = c;
                    }
                    let mut ap_dict = Dictionary::new();
                    ap_dict.set("Type", Object::Name(b"XObject".to_vec()));
                    ap_dict.set("Subtype", Object::Name(b"Form".to_vec()));
                    ap_dict.set("BBox", crate::annot::nums(&rect));
                    ap_dict.set("Resources", res);
                    let ap_id = self.add(Object::Stream(lopdf::Stream::new(ap_dict, content)));
                    if let Ok(d) = self.doc.get_dictionary_mut(id) {
                        let mut ap = Dictionary::new();
                        ap.set("N", Object::Reference(ap_id));
                        d.set("AP", ap);
                    }
                }
            }
        }
        self.commit();
        Ok(())
    }

    /// Reply to a comment: a note in reply to it (`/IRT`), shown in the comment list only.
    pub fn reply_to_annotation(
        &mut self,
        a: &Annot,
        author: &str,
        text: &str,
    ) -> Result<(), String> {
        let parent = a.id.ok_or("the comment cannot be changed")?;
        // A note over the parent, then marked as a reply.
        self.add_annotation(&NewAnnot {
            kind: AnnotKind::Note,
            page: a.page,
            rects: vec![a.rect],
            color: a.color,
            author: author.to_owned(),
            contents: text.to_owned(),
        })?;
        let reply = self.last_annotation(a.page).ok_or("reply")?;
        self.begin("reply");
        self.touch(reply);
        if let Ok(d) = self.doc.get_dictionary_mut(reply) {
            d.set("IRT", Object::Reference(parent));
            d.set("RT", Object::Name(b"R".to_vec()));
            d.remove(b"AP");
        }
        self.commit();
        Ok(())
    }

    fn last_annotation(&self, page: usize) -> Option<ObjectId> {
        let info = self.pages.get(page)?;
        let d = self.doc.get_dictionary(info.id).ok()?;
        resolve(&self.doc, d.get(b"Annots").ok()?)
            .as_array()
            .ok()?
            .last()?
            .as_reference()
            .ok()
    }

    /// Comments with their reply threads (replies are not listed as comments of their own).
    pub fn comment_threads(&self) -> Vec<CommentThread> {
        let all = self.annotations();
        let parent_of = |a: &Annot| -> Option<ObjectId> {
            let d = self.doc.get_dictionary(a.id?).ok()?;
            if d.get(b"RT").and_then(Object::as_name).ok() == Some(b"Group") {
                return None;
            }
            d.get(b"IRT").ok()?.as_reference().ok()
        };
        let parents: Vec<Option<ObjectId>> = all.iter().map(parent_of).collect();
        // Root of each reply chain (bounded walk against cycles).
        let root = |i: usize| -> Option<usize> {
            let mut cur = i;
            for _ in 0..64 {
                match parents[cur] {
                    Some(p) => cur = all.iter().position(|a| a.id == Some(p))?,
                    None => return (cur != i).then_some(cur),
                }
            }
            None
        };
        let mut threads: Vec<CommentThread> = Vec::new();
        let mut index_of = std::collections::HashMap::new();
        for (i, a) in all.iter().enumerate() {
            if parents[i].is_none() {
                index_of.insert(i, threads.len());
                threads.push(CommentThread {
                    comment: a.clone(),
                    replies: Vec::new(),
                });
            }
        }
        for (i, a) in all.iter().enumerate() {
            if parents[i].is_some() {
                match root(i).and_then(|r| index_of.get(&r)) {
                    Some(&t) => threads[t].replies.push(a.clone()),
                    // A reply whose comment is gone shows as a comment.
                    None => threads.push(CommentThread {
                        comment: a.clone(),
                        replies: Vec::new(),
                    }),
                }
            }
        }
        for t in &mut threads {
            t.replies.sort_by(|a, b| a.modified.cmp(&b.modified));
        }
        threads
    }
}

#[cfg(test)]
mod tests {
    use crate::{standard_fonts, AnnotKind, NewAnnot, PdfDocument};

    fn doc() -> PdfDocument {
        let content = "BT /F1 12 Tf 20 70 Td (Hello) Tj ET";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>"
                .to_owned(),
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        ];
        let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
        PdfDocument::open(crate::tests::build_pdf(&refs), standard_fonts()).expect("opens")
    }

    #[test]
    fn move_recolour_reply_and_threads() {
        let mut d = doc();
        d.add_annotation(&NewAnnot {
            kind: AnnotKind::Highlight,
            page: 0,
            rects: vec![[20.0, 20.0, 60.0, 34.0]],
            color: [255, 230, 0],
            author: "A".into(),
            contents: "first".into(),
        })
        .expect("adds");
        let a = d.annotations().remove(0);
        d.move_annotation(&a, 10.0, 5.0).expect("moves");
        let moved = d.annotations().remove(0);
        assert!((moved.rect[0] - a.rect[0] - 10.0).abs() < 0.01, "{moved:?}");
        assert!((moved.rect[1] - a.rect[1] - 5.0).abs() < 0.01);
        d.set_annotation_color(&moved, [0, 128, 255])
            .expect("colour");
        assert_eq!(d.annotations()[0].color, [0, 128, 255]);
        let page = d
            .render(0, crate::RenderOptions::default())
            .expect("renders");
        assert!(page
            .list
            .commands
            .iter()
            .any(|c| matches!(c, bo_render::Command::PushGroup { .. })));
        let parent = d.annotations().remove(0);
        d.reply_to_annotation(&parent, "B", "a reply")
            .expect("reply");
        let threads = d.comment_threads();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].replies.len(), 1);
        assert_eq!(threads[0].replies[0].contents, "a reply");
        // Survives a save and reopen.
        let bytes = d.save().expect("saves");
        let again = PdfDocument::open(bytes, standard_fonts()).expect("reopens");
        let threads = again.comment_threads();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].replies[0].author, "B");
    }
}
