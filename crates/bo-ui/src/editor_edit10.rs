//! BlueEditor round 10: the Edit tab's drop-downs as the reference editor's (black box at
//! 125 %): Edit Object ▾ (All, Text, Image, Shape, Shading — Edit Object picks only that kind),
//! Add Images ▾ (Image From File; Draw New Image dimmed), Add Shapes ▾ (the shapes with our own
//! icons), Web Links ▾ (Create links from URLs, Remove all links, Append All Web Links on Current
//! Page, View Web Links), in the round-9 menu style.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, Ctl, Form, On};
use crate::editor_window::EditorWindow;
use crate::EditTool;
use bo_pdfread::ObjectKind;

impl EditorWindow {
    /// Round-10 Edit tab commands; false when not one of them.
    pub(crate) fn edit10_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "ed.editObjectMenu" => self.menu10(
                &[
                    (
                        "ed.obj.all",
                        "pdfEditor10.objAll",
                        Some("editObject"),
                        false,
                    ),
                    (
                        "ed.obj.text",
                        "pdfEditor10.objText",
                        Some("editText"),
                        false,
                    ),
                    (
                        "ed.obj.image",
                        "pdfEditor10.objImage",
                        Some("toImage"),
                        false,
                    ),
                    (
                        "ed.obj.shape",
                        "pdfEditor10.objShape",
                        Some("rectangle"),
                        false,
                    ),
                    (
                        "ed.obj.shading",
                        "pdfEditor10.objShading",
                        Some("areaHighlight"),
                        false,
                    ),
                ],
                Vec::new(),
                "ed.editObjectMenu",
            ),
            "ed.obj.all" | "ed.obj.text" | "ed.obj.image" | "ed.obj.shape" | "ed.obj.shading" => {
                self.edit.filter10 = match cmd {
                    "ed.obj.text" => Some(ObjectKind::Text),
                    "ed.obj.image" => Some(ObjectKind::Image),
                    "ed.obj.shape" => Some(ObjectKind::Path),
                    "ed.obj.shading" => Some(ObjectKind::Shading),
                    _ => None,
                };
                if self.edit_tool() != Some(EditTool::Object) {
                    self.toggle_tool(EditTool::Object);
                }
            }
            "ed.addImageMenu" => self.menu10(
                &[
                    (
                        "ed.addImage",
                        "pdfEditor10.imageFromFile",
                        Some("addImage"),
                        false,
                    ),
                    (
                        "ed.drawImage",
                        "pdfEditor10.drawNewImage",
                        Some("pencil"),
                        true,
                    ),
                ],
                Vec::new(),
                "ed.addImageMenu",
            ),
            "ed.addShapes" => self.menu10(
                &[
                    ("ed.shape.line", "pdfEditor.line", Some("line"), false),
                    (
                        "ed.shape.rect",
                        "pdfReader.rectangle",
                        Some("rectangle"),
                        false,
                    ),
                    (
                        "ed.shape.rounded",
                        "pdfEditor.roundedRect",
                        Some("rectangle"),
                        false,
                    ),
                    (
                        "ed.shape.ellipse",
                        "pdfEditor.ellipse",
                        Some("ellipse"),
                        false,
                    ),
                ],
                Vec::new(),
                "ed.addShapes",
            ),
            "ed.webLinks" => self.menu10(
                &[
                    (
                        "ed.webLinks.create",
                        "pdfEditor10.linksCreate",
                        Some("link"),
                        false,
                    ),
                    (
                        "ed.webLinks.remove",
                        "pdfEditor10.linksRemove",
                        Some("close"),
                        false,
                    ),
                    (
                        "ed.webLinks.append",
                        "pdfEditor10.linksAppend",
                        Some("plus"),
                        false,
                    ),
                    (
                        "ed.webLinks.view",
                        "pdfEditor10.linksView",
                        Some("search"),
                        false,
                    ),
                ],
                Vec::new(),
                "ed.webLinks",
            ),
            "ed.webLinks.append" => {
                let page = self
                    .view()
                    .map_or(0, crate::reader_view::ReaderView::current_page);
                let r = self.doc_edit(|d| d.create_web_links(&[page]));
                match r {
                    Ok(n) => {
                        self.after_doc_edit();
                        let s = self
                            .t("pdfEditor.webLinksCreated")
                            .replace("{count}", &n.to_string());
                        self.set_status(&s);
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            "ed.webLinks.view" => self.open_web_links(),
            _ => return false,
        }
        true
    }

    /// View Web Links: the web addresses of the document's link annotations and text.
    fn open_web_links(&mut self) {
        let n = self
            .view()
            .map_or(0, crate::reader_view::ReaderView::page_count);
        let mut rows: Vec<Vec<String>> = Vec::new();
        for page in 0..n {
            let found = self
                .with_doc(|d| {
                    let objs = d.page_objects(page)?;
                    Ok(bo_pdfread::text_lines(&objs)
                        .into_iter()
                        .flat_map(|l| bo_pdfread::find_web_addresses(&l.text))
                        .map(|(_, u)| u)
                        .collect::<Vec<_>>())
                })
                .unwrap_or_default();
            for url in found {
                rows.push(vec![(page + 1).to_string(), url]);
            }
        }
        let t = |k: &str| self.t(k);
        let c = vec![
            Ctl::Table {
                r: [17.0, 50.0, 610.0, 330.0],
                cols: vec![
                    (t("pdfEditor10.page"), 70.0),
                    (t("pdfEditor10.address"), 538.0),
                ],
                rows,
            },
            button(
                "ok",
                [536.0, 396.0, 91.0, 28.0],
                t("pdfReader.ok"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor10.linksView"), (644.0, 440.0), c);
        self.form8.push((f, FormWhat::Sub));
    }
}
