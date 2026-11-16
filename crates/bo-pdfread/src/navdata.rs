//! Data of BlueReader's navigation panels (round 8): optional content groups as the Layers
//! panel lists them (ISO 32000-1 §8.11.4: `/OCProperties /D` with `/Order`, `/ON` / `/OFF`,
//! `/Locked`, `/RBGroups` and the `/Usage` print and view states), file attachments (embedded
//! files, §7.11.4, and file attachment annotations, §12.5.6.15) and the review / marked states of
//! comments (state replies, §12.5.6.3 Table 172 `/State` `/StateModel`).
use crate::annot::{text_obj, Annot};
use crate::{resolve, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId};
use std::collections::HashSet;

/// One row of the Layers panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerRow {
    /// Group name, or the label of a group of rows (`/Order` sub-array starting with a string).
    pub label: String,
    /// Nesting level (0 = top).
    pub depth: usize,
    /// Optional content group (index into [`PdfDocument::layer_count`]'s groups); `None` for a
    /// label row.
    pub group: Option<usize>,
    /// The group is shown.
    pub on: bool,
    /// The user cannot change it (`/Locked`).
    pub locked: bool,
    /// Print state from `/Usage /Print /PrintState` (`None`: not given).
    pub print: Option<bool>,
    /// View state from `/Usage /View /ViewState`.
    pub view: Option<bool>,
}

/// Where an attachment's bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// Entry `n` of the `/EmbeddedFiles` name tree (with a file stream).
    Embedded(usize),
    /// A file attachment annotation.
    Annot(ObjectId),
}

/// A file attached to the document or to a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    /// File name.
    pub name: String,
    /// Description (`/Desc` of a document-level attachment).
    pub description: String,
    /// Size in bytes (`/Params /Size`, else the stream's length).
    pub size: usize,
    /// Modification date `YYYY-MM-DD HH:MM` (empty when unknown).
    pub modified: String,
    /// Page of a file attachment annotation (`None`: attached to the document).
    pub page: Option<usize>,
    /// Rectangle of the annotation on its page (page points, y down).
    pub rect: Option<[f32; 4]>,
    source: Source,
}

/// Review and marked states of a comment (the latest state reply of each model).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentState {
    /// `Accepted`, `Rejected`, `Cancelled`, `Completed` (`None` or "None": no status).
    pub review: Option<String>,
    /// Marked with a checkmark.
    pub marked: bool,
}

fn refs(doc: &crate::store::Document, o: Option<&Object>) -> Vec<ObjectId> {
    o.map(|o| resolve(doc, o))
        .and_then(|o| o.as_array().ok())
        .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
        .unwrap_or_default()
}

fn usage_state(
    doc: &crate::store::Document,
    g: &Dictionary,
    cat: &[u8],
    key: &[u8],
) -> Option<bool> {
    let u = resolve(doc, g.get(b"Usage").ok()?).as_dict().ok()?;
    let c = resolve(doc, u.get(cat).ok()?).as_dict().ok()?;
    match c.get(key).and_then(Object::as_name).ok()? {
        b"ON" => Some(true),
        b"OFF" => Some(false),
        _ => None,
    }
}

impl PdfDocument {
    fn oc_properties(&self) -> Option<&Dictionary> {
        let c = self.doc.catalog().ok()?;
        resolve(&self.doc, c.get(b"OCProperties").ok()?)
            .as_dict()
            .ok()
    }

    /// Optional content groups of the document (`/OCProperties /OCGs`).
    fn layer_groups(&self) -> Vec<ObjectId> {
        self.oc_properties()
            .map(|p| refs(&self.doc, p.get(b"OCGs").ok()))
            .unwrap_or_default()
    }

    /// Number of optional content groups (0: no Layers panel).
    pub fn layer_count(&self) -> usize {
        self.layer_groups().len()
    }

    /// Rows of the Layers panel in `/Order` (every group in `/OCGs` order when there is none).
    pub fn layers(&self) -> Vec<LayerRow> {
        let groups = self.layer_groups();
        let Some(props) = self.oc_properties() else {
            return Vec::new();
        };
        let d = props
            .get(b"D")
            .ok()
            .and_then(|o| resolve(&self.doc, o).as_dict().ok());
        let locked: HashSet<ObjectId> = d
            .map(|d| refs(&self.doc, d.get(b"Locked").ok()))
            .unwrap_or_default()
            .into_iter()
            .collect();
        let row = |id: ObjectId, depth: usize| -> Option<LayerRow> {
            let index = groups.iter().position(|g| *g == id)?;
            let g = self.doc.get_dictionary(id).ok()?;
            let label = g
                .get(b"Name")
                .ok()
                .and_then(|o| resolve(&self.doc, o).as_str().ok())
                .map(crate::text_string)
                .unwrap_or_default();
            Some(LayerRow {
                label,
                depth,
                group: Some(index),
                on: !self.hidden.contains(&id),
                locked: locked.contains(&id),
                print: usage_state(&self.doc, g, b"Print", b"PrintState"),
                view: usage_state(&self.doc, g, b"View", b"ViewState"),
            })
        };
        let mut out = Vec::new();
        let order = d
            .and_then(|d| d.get(b"Order").ok())
            .and_then(|o| resolve(&self.doc, o).as_array().ok());
        match order {
            Some(order) => self.order_rows(order, 0, &row, &mut out, 0),
            None => out.extend(groups.iter().filter_map(|g| row(*g, 0))),
        }
        out
    }

    fn order_rows(
        &self,
        items: &[Object],
        depth: usize,
        row: &dyn Fn(ObjectId, usize) -> Option<LayerRow>,
        out: &mut Vec<LayerRow>,
        nest: usize,
    ) {
        // Nested arrays are bounded (hostile files can nest them deeply).
        if nest > 32 {
            return;
        }
        for o in items {
            match resolve(&self.doc, o) {
                Object::Array(sub) => {
                    // A sub-array starting with a string is a labelled set of rows; one after a
                    // group holds that group's children (§8.11.4.3 Table 101 `/Order`).
                    let (first, rest) = match sub.split_first() {
                        Some((Object::String(s, _), rest)) => (Some(crate::text_string(s)), rest),
                        _ => (None, sub.as_slice()),
                    };
                    if let Some(label) = first {
                        out.push(LayerRow {
                            label,
                            depth,
                            group: None,
                            on: true,
                            locked: false,
                            print: None,
                            view: None,
                        });
                    }
                    self.order_rows(rest, depth + 1, row, out, nest + 1);
                }
                _ => {
                    if let Some(r) = o.as_reference().ok().and_then(|id| row(id, depth)) {
                        out.push(r);
                    }
                }
            }
        }
    }

    /// Show or hide optional content group `group` (the Layers panel; not saved). Turning a
    /// group of a radio-button set on turns the others off (`/RBGroups`).
    pub fn set_layer_on(&mut self, group: usize, on: bool) {
        let groups = self.layer_groups();
        let Some(&id) = groups.get(group) else {
            return;
        };
        if on {
            let rb: Vec<Vec<ObjectId>> = self
                .oc_properties()
                .and_then(|p| p.get(b"D").ok())
                .and_then(|o| resolve(&self.doc, o).as_dict().ok())
                .and_then(|d| d.get(b"RBGroups").ok())
                .and_then(|o| resolve(&self.doc, o).as_array().ok())
                .map(|a| a.iter().map(|s| refs(&self.doc, Some(s))).collect())
                .unwrap_or_default();
            for set in rb.iter().filter(|s| s.contains(&id)) {
                self.hidden.extend(set.iter().filter(|g| **g != id));
            }
            self.hidden.remove(&id);
        } else {
            self.hidden.insert(id);
        }
    }

    /// Groups hidden when printing: those hidden on screen, then the `/Usage /Print` states
    /// (a group marked not to print is left out, one marked to print is printed).
    pub(crate) fn print_hidden(&self) -> HashSet<ObjectId> {
        let mut out = self.hidden.clone();
        for id in self.layer_groups() {
            let Ok(g) = self.doc.get_dictionary(id) else {
                continue;
            };
            match usage_state(&self.doc, g, b"Print", b"PrintState") {
                Some(false) => {
                    out.insert(id);
                }
                Some(true) => {
                    out.remove(&id);
                }
                None => {}
            }
        }
        out
    }

    /// Files attached to the document and to its pages, by name.
    pub fn attachments(&self) -> Vec<Attachment> {
        let mut out = Vec::new();
        let embedded: Vec<(String, Object)> = self
            .name_tree(b"EmbeddedFiles")
            .into_iter()
            .filter(|(_, v)| self.file_stream(v).is_some())
            .collect();
        for (n, (name, v)) in embedded.iter().enumerate() {
            let Some((s, spec)) = self.file_stream(v) else {
                continue;
            };
            out.push(Attachment {
                name: spec_name(&self.doc, spec).unwrap_or_else(|| name.clone()),
                description: spec_str(&self.doc, spec, b"Desc").unwrap_or_default(),
                size: stream_size(&self.doc, s),
                modified: stream_date(&self.doc, s),
                page: None,
                rect: None,
                source: Source::Embedded(n),
            });
        }
        for (index, info) in self.pages.iter().enumerate() {
            let Some(list) = self
                .doc
                .get_dictionary(info.id)
                .ok()
                .and_then(|p| p.get(b"Annots").ok())
                .and_then(|o| resolve(&self.doc, o).as_array().ok())
            else {
                continue;
            };
            let base = crate::page_matrix(info.crop, info.rotate);
            for a in list {
                let Ok(id) = a.as_reference() else { continue };
                let Ok(d) = self.doc.get_dictionary(id) else {
                    continue;
                };
                if d.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"FileAttachment") {
                    continue;
                }
                let Some(fs) = d.get(b"FS").ok() else {
                    continue;
                };
                let Some((s, spec)) = self.file_stream(fs) else {
                    continue;
                };
                out.push(Attachment {
                    name: spec_name(&self.doc, spec).unwrap_or_default(),
                    // The reference reader leaves the description of page attachments empty.
                    description: String::new(),
                    size: stream_size(&self.doc, s),
                    modified: stream_date(&self.doc, s),
                    page: Some(index),
                    rect: crate::annot_rect(&self.doc, d).map(|r| crate::device_rect(base, r)),
                    source: Source::Annot(id),
                });
            }
        }
        out.sort_by_key(|a| a.name.to_lowercase());
        out
    }

    /// Bytes of an attachment from [`PdfDocument::attachments`].
    pub fn attachment_bytes(&self, a: &Attachment) -> Result<Vec<u8>, String> {
        match a.source {
            Source::Embedded(n) => self.embedded_file_bytes(n),
            Source::Annot(id) => {
                let d = self.doc.get_dictionary(id).map_err(|e| e.to_string())?;
                let fs = d.get(b"FS").map_err(|e| e.to_string())?;
                let (s, _) = self.file_stream(fs).ok_or("no file")?;
                crate::filters::stream_bytes(&self.doc, s)
            }
        }
    }

    /// The comment's state replies: (model, state, modified, author), oldest first.
    fn state_replies(&self, a: &Annot) -> Vec<(String, String, String)> {
        let Some(parent) = a.id else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for info in &self.pages {
            let Some(list) = self
                .doc
                .get_dictionary(info.id)
                .ok()
                .and_then(|p| p.get(b"Annots").ok())
                .and_then(|o| resolve(&self.doc, o).as_array().ok())
            else {
                continue;
            };
            for o in list {
                let Ok(d) = resolve(&self.doc, o).as_dict() else {
                    continue;
                };
                if d.get(b"IRT").and_then(Object::as_reference).ok() != Some(parent) {
                    continue;
                }
                let Some(state) = spec_str(&self.doc, d, b"State") else {
                    continue;
                };
                // §12.5.6.3: the model defaults to Marked for Marked / Unmarked, else Review.
                let model = spec_str(&self.doc, d, b"StateModel")
                    .or_else(|| {
                        d.get(b"StateModel")
                            .and_then(Object::as_name)
                            .ok()
                            .map(|n| String::from_utf8_lossy(n).into_owned())
                    })
                    .unwrap_or_else(|| {
                        if state == "Marked" || state == "Unmarked" {
                            "Marked".into()
                        } else {
                            "Review".into()
                        }
                    });
                let m = spec_str(&self.doc, d, b"M").unwrap_or_default();
                out.push((model, state, m));
            }
        }
        out.sort_by(|a, b| a.2.cmp(&b.2));
        out
    }

    /// Review and marked states of a comment.
    pub fn comment_state(&self, a: &Annot) -> CommentState {
        let mut s = CommentState::default();
        for (model, state, _) in self.state_replies(a) {
            if model == "Marked" {
                s.marked = state == "Marked";
            } else {
                s.review = (state != "None").then_some(state);
            }
        }
        s
    }

    /// Whether `a` is a state reply (shown as its comment's status, not as a reply).
    pub fn is_state_reply(&self, a: &Annot) -> bool {
        a.id.and_then(|id| self.doc.get_dictionary(id).ok())
            .is_some_and(|d| d.has(b"State"))
    }

    /// Set a comment's review status (`model` "Review": Accepted / Rejected / Cancelled /
    /// Completed / None) or checkmark (`model` "Marked": Marked / Unmarked) with a state reply
    /// by `author`; one undo step, saved incrementally.
    pub fn set_comment_state(
        &mut self,
        a: &Annot,
        model: &str,
        state: &str,
        author: &str,
    ) -> Result<(), String> {
        self.begin_group("state");
        let r = self.state_reply(a, model, state, author);
        self.end_group();
        r
    }

    fn state_reply(
        &mut self,
        a: &Annot,
        model: &str,
        state: &str,
        author: &str,
    ) -> Result<(), String> {
        self.reply_to_annotation(a, author, "")?;
        let reply = self
            .doc
            .get_dictionary(self.pages.get(a.page).ok_or("no such page")?.id)
            .ok()
            .and_then(|p| p.get(b"Annots").ok())
            .and_then(|o| resolve(&self.doc, o).as_array().ok())
            .and_then(|l| l.last())
            .and_then(|o| o.as_reference().ok())
            .ok_or("reply")?;
        self.touch(reply);
        if let Ok(d) = self.doc.get_dictionary_mut(reply) {
            d.set("State", text_obj(state));
            d.set("StateModel", text_obj(model));
            d.set("Contents", text_obj(&format!("{state} set by {author}")));
            // Hidden, not printed: only the comment list shows it (bits 2 and 5 of /F).
            d.set("F", Object::Integer(2 | 32));
        }
        Ok(())
    }
}

fn spec_str(doc: &crate::store::Document, d: &Dictionary, key: &[u8]) -> Option<String> {
    resolve(doc, d.get(key).ok()?)
        .as_str()
        .ok()
        .map(crate::text_string)
}

fn spec_name(doc: &crate::store::Document, spec: &Dictionary) -> Option<String> {
    spec_str(doc, spec, b"UF").or_else(|| spec_str(doc, spec, b"F"))
}

fn params(doc: &crate::store::Document, s: &lopdf::Stream) -> Option<Dictionary> {
    resolve(doc, s.dict.get(b"Params").ok()?)
        .as_dict()
        .ok()
        .cloned()
}

fn stream_size(doc: &crate::store::Document, s: &lopdf::Stream) -> usize {
    params(doc, s)
        .and_then(|p| {
            p.get(b"Size")
                .ok()
                .and_then(|v| resolve(doc, v).as_i64().ok())
        })
        .map_or(s.content.len(), |v| usize::try_from(v.max(0)).unwrap_or(0))
}

fn stream_date(doc: &crate::store::Document, s: &lopdf::Stream) -> String {
    params(doc, s)
        .and_then(|p| spec_str(doc, &p, b"ModDate"))
        .map(|d| crate::templates::date_text(&d))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::tests::build_pdf;
    use crate::{standard_fonts, AnnotKind, NewAnnot, PdfDocument, RenderOptions};

    fn layered() -> PdfDocument {
        let content = "/OC /a BDC 0 0 1 rg 10 10 20 20 re f EMC \
                       /OC /b BDC 1 0 0 rg 40 10 20 20 re f EMC \
                       /OC /c BDC 0 1 0 rg 70 10 20 20 re f EMC";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R 6 0 R 7 0 R] \
             /D << /OFF [6 0 R] /Locked [7 0 R] /RBGroups [[5 0 R 6 0 R]] \
             /Order [5 0 R [(Extras) 6 0 R 7 0 R]] >> >> >>"
                .to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 40] /Contents 4 0 R \
             /Resources << /Properties << /a 5 0 R /b 6 0 R /c 7 0 R >> >> >>"
                .to_owned(),
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
            "<< /Type /OCG /Name (Blue) >>".to_owned(),
            "<< /Type /OCG /Name (Red) >>".to_owned(),
            "<< /Type /OCG /Name (Green) /Usage << /Print << /PrintState /OFF >> >> >>".to_owned(),
        ];
        let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
        PdfDocument::open(build_pdf(&refs), standard_fonts()).expect("opens")
    }

    fn fills(d: &mut PdfDocument, printing: bool) -> usize {
        let extra = crate::RenderExtra {
            printing,
            ..crate::RenderExtra::default()
        };
        d.render_extra(0, RenderOptions::default(), extra)
            .expect("renders")
            .list
            .commands
            .iter()
            .filter(|c| matches!(c, bo_render::Command::Path { .. }))
            .count()
    }

    #[test]
    fn layers_follow_order_states_and_radio_groups() {
        let mut d = layered();
        assert_eq!(d.layer_count(), 3);
        let rows = d.layers();
        let shown: Vec<(&str, usize, Option<usize>, bool, bool)> = rows
            .iter()
            .map(|r| (r.label.as_str(), r.depth, r.group, r.on, r.locked))
            .collect();
        assert_eq!(
            shown,
            vec![
                ("Blue", 0, Some(0), true, false),
                ("Extras", 0, None, true, false),
                ("Red", 1, Some(1), false, false),
                ("Green", 1, Some(2), true, true),
            ]
        );
        assert_eq!(rows[3].print, Some(false));
        // Blue and green on screen; green is not printed.
        assert_eq!(fills(&mut d, false), 2);
        assert_eq!(fills(&mut d, true), 1);
        // Red on turns Blue (same radio-button set) off.
        d.set_layer_on(1, true);
        let on: Vec<bool> = d.layers().iter().map(|r| r.on).collect();
        assert_eq!(on, vec![false, true, true, true]);
        assert_eq!(fills(&mut d, false), 2);
        d.set_layer_on(2, false);
        assert_eq!(fills(&mut d, false), 1);
    }

    #[test]
    fn attachments_list_embedded_files_and_page_attachments() {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R /Names << /EmbeddedFiles << /Names \
             [(b.csv) 5 0 R] >> >> >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Annots [7 0 R] >>",
            "<< >>",
            "<< /Type /Filespec /F (b.csv) /Desc (Table) /EF << /F 6 0 R >> >>",
            "<< /Type /EmbeddedFile /Params << /Size 3 /ModDate (D:20261116093000) >> \
             /Length 3 >>\nstream\n1,2\nendstream",
            "<< /Type /Annot /Subtype /FileAttachment /Rect [10 70 26 86] /FS << /Type \
             /Filespec /UF (a.txt) /Desc (ignored) /EF << /F 8 0 R >> >> >>",
            "<< /Length 5 >>\nstream\nhello\nendstream",
        ];
        let d = PdfDocument::open(build_pdf(&objs), standard_fonts()).expect("opens");
        let list = d.attachments();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "a.txt");
        assert_eq!((list[0].page, list[0].size), (Some(0), 5));
        assert_eq!(list[0].description, "");
        assert_eq!(list[0].rect, Some([10.0, 14.0, 26.0, 30.0]));
        assert_eq!(d.attachment_bytes(&list[0]).expect("bytes"), b"hello");
        assert_eq!(list[1].name, "b.csv");
        assert_eq!(list[1].description, "Table");
        assert_eq!(list[1].modified, "2026-11-16 09:30");
        assert_eq!(d.attachment_bytes(&list[1]).expect("bytes"), b"1,2");
    }

    #[test]
    fn comment_states_are_state_replies() {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] >>",
        ];
        let mut d = PdfDocument::open(build_pdf(&objs), standard_fonts()).expect("opens");
        d.add_annotation(&NewAnnot {
            kind: AnnotKind::Note,
            page: 0,
            rects: vec![[20.0, 20.0, 40.0, 40.0]],
            color: [255, 214, 0],
            author: "A".into(),
            contents: "note".into(),
        })
        .expect("adds");
        let a = d.comment_threads()[0].comment.clone();
        assert_eq!(d.comment_state(&a), crate::CommentState::default());
        d.set_comment_state(&a, "Review", "Accepted", "B")
            .expect("state");
        d.set_comment_state(&a, "Marked", "Marked", "B")
            .expect("mark");
        let s = d.comment_state(&a);
        assert_eq!(s.review.as_deref(), Some("Accepted"));
        assert!(s.marked);
        // The state replies are replies of the thread, recognised as states.
        let t = &d.comment_threads()[0];
        assert_eq!(t.replies.len(), 2);
        assert!(t.replies.iter().all(|r| d.is_state_reply(r)));
        // One undo step each.
        d.undo();
        assert!(!d.comment_state(&a).marked);
        let saved = d.save().expect("saves");
        let again = PdfDocument::open(saved, standard_fonts()).expect("reopens");
        let a2 = again.comment_threads()[0].comment.clone();
        assert_eq!(again.comment_state(&a2).review.as_deref(), Some("Accepted"));
    }
}
