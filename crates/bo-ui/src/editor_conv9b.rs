//! BlueEditor round 9: the Convert formats the reference editor offers that BlueOffice can
//! write too — To XML Spreadsheet 2003 (the Office XML spreadsheet schema, the same cells as
//! To Excel), To XPS (ECMA-388 OpenXPS package, each page a picture: fixed pages in a fixed
//! document sequence) and To XML 1.0 (the pages' text lines with their boxes and sizes) — and
//! From Files › Convert to PDF Settings ("Converting To PDF", 857 × 678 px at 125 %: the kinds
//! of file at the left; for each, the settings, Add bookmarks and Tagged PDF, applied when an
//! office file is made into PDF).
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, check, combo, label, Ctl, Form, On, Val};
use crate::editor_window::EditorWindow;
use crate::reader_window::ReaderRequest;
use bo_pdfread::PdfDocument;

/// Office-to-PDF choices per kind of file (documents, workbooks, presentations).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ConvSettings {
    pub bookmarks: [bool; 3],
    pub tagged: [bool; 3],
}

/// The kinds listed in Converting To PDF (label keys) with their file extensions.
const KINDS: [(&str, &[&str]); 3] = [
    (
        "pdfEditor9.kindDocuments",
        &[".docx", ".docm", ".doc", ".odt", ".rtf", ".txt", ".html"],
    ),
    (
        "pdfEditor9.kindWorkbooks",
        &[".xlsx", ".xlsm", ".xls", ".xlsb", ".ods", ".csv"],
    ),
    (
        "pdfEditor9.kindPresentations",
        &[".pptx", ".pptm", ".ppt", ".odp"],
    ),
];

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// An XML Spreadsheet 2003 workbook of `cells` (row, column, text; 0-based) on one sheet.
pub(crate) fn xml_spreadsheet(cells: &[(u32, u32, String)], sheet: &str) -> Vec<u8> {
    let mut rows: std::collections::BTreeMap<u32, Vec<(u32, &str)>> =
        std::collections::BTreeMap::default();
    for (r, c, t) in cells {
        rows.entry(*r).or_default().push((*c, t.as_str()));
    }
    let mut out = String::from(
        "<?xml version=\"1.0\"?>\n<?mso-application progid=\"Excel.Sheet\"?>\n\
         <Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\" \
         xmlns:ss=\"urn:schemas-microsoft-com:office:spreadsheet\">\n",
    );
    out.push_str(&format!(
        " <Worksheet ss:Name=\"{}\">\n  <Table>\n",
        esc(sheet)
    ));
    for (r, mut cs) in rows {
        cs.sort_by_key(|c| c.0);
        out.push_str(&format!("   <Row ss:Index=\"{}\">\n", r + 1));
        for (c, t) in cs {
            out.push_str(&format!(
                "    <Cell ss:Index=\"{}\"><Data ss:Type=\"String\">{}</Data></Cell>\n",
                c + 1,
                esc(t)
            ));
        }
        out.push_str("   </Row>\n");
    }
    out.push_str("  </Table>\n </Worksheet>\n</Workbook>\n");
    out.into_bytes()
}

/// The pages' text lines as XML 1.0: page sizes, each line's box (points from the top left),
/// font size and colour.
pub(crate) fn xml_dump(doc: &mut PdfDocument) -> Result<Vec<u8>, String> {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<document pages=\"{}\">\n", doc.page_count()));
    for i in 0..doc.page_count() {
        let (w, h) = doc
            .page_info(i)
            .map_or((612.0, 792.0), |p| (p.width, p.height));
        out.push_str(&format!(
            " <page number=\"{}\" width=\"{w:.2}\" height=\"{h:.2}\">\n",
            i + 1
        ));
        let objs = doc.page_objects(i)?;
        let mut lines = bo_pdfread::text_lines(&objs);
        lines.retain(|l| !l.text.trim().is_empty());
        for l in lines {
            let [r, g, b] = l.color;
            out.push_str(&format!(
                "  <line x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" size=\"{:.1}\" \
                 color=\"#{r:02X}{g:02X}{b:02X}\">{}</line>\n",
                l.bbox[0],
                l.bbox[1],
                l.bbox[2] - l.bbox[0],
                l.bbox[3] - l.bbox[1],
                l.font_size,
                esc(l.text.trim())
            ));
        }
        out.push_str(" </page>\n");
    }
    out.push_str("</document>\n");
    Ok(out.into_bytes())
}

/// An XPS package (ECMA-388) of page pictures: `pages` are (PNG, width, height in points).
pub(crate) fn xps_of(pages: &[(Vec<u8>, f32, f32)]) -> Result<Vec<u8>, String> {
    const NS: &str = "http://schemas.microsoft.com/xps/2005/06";
    let mut pkg = bo_opc::Package::new();
    let mut overrides = String::new();
    pkg.put_part(
        "/_rels/.rels",
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships \
             xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Id=\"R0\" Type=\"{NS}/fixedrepresentation\" \
             Target=\"/FixedDocSeq.fdseq\"/></Relationships>"
        )
        .into_bytes(),
    );
    pkg.put_part(
        "/FixedDocSeq.fdseq",
        format!(
            "<FixedDocumentSequence xmlns=\"{NS}\"><DocumentReference \
             Source=\"/Documents/1/FixedDoc.fdoc\"/></FixedDocumentSequence>"
        )
        .into_bytes(),
    );
    let mut fdoc = format!("<FixedDocument xmlns=\"{NS}\">");
    for (k, (png, w, h)) in pages.iter().enumerate() {
        let n = k + 1;
        // XPS units are 1/96 inch.
        let (pw, ph) = (w * 96.0 / 72.0, h * 96.0 / 72.0);
        let (iw, ih) = png_size(png).ok_or("bad page picture")?;
        fdoc.push_str(&format!(
            "<PageContent Source=\"/Documents/1/Pages/{n}.fpage\" Width=\"{pw:.2}\" \
             Height=\"{ph:.2}\"/>"
        ));
        pkg.put_part(
            &format!("/Documents/1/Pages/{n}.fpage"),
            format!(
                "<FixedPage xmlns=\"{NS}\" Width=\"{pw:.2}\" Height=\"{ph:.2}\" xml:lang=\"und\">\
                 <Path Data=\"M 0,0 L {pw:.2},0 {pw:.2},{ph:.2} 0,{ph:.2} Z\"><Path.Fill>\
                 <ImageBrush ImageSource=\"/Resources/Images/{n}.png\" Viewbox=\"0,0,{iw},{ih}\" \
                 ViewboxUnits=\"Absolute\" Viewport=\"0,0,{pw:.2},{ph:.2}\" \
                 ViewportUnits=\"Absolute\"/></Path.Fill></Path></FixedPage>"
            )
            .into_bytes(),
        );
        pkg.put_part(
            &format!("/Documents/1/Pages/_rels/{n}.fpage.rels"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships \
                 xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
                 <Relationship Id=\"R1\" Type=\"{NS}/required-resource\" \
                 Target=\"/Resources/Images/{n}.png\"/></Relationships>"
            )
            .into_bytes(),
        );
        pkg.put_part(&format!("/Resources/Images/{n}.png"), png.clone());
        overrides.push_str(&format!(
            "<Override PartName=\"/Documents/1/Pages/{n}.fpage\" \
             ContentType=\"application/vnd.ms-package.xps-fixedpage+xml\"/>"
        ));
    }
    fdoc.push_str("</FixedDocument>");
    pkg.put_part("/Documents/1/FixedDoc.fdoc", fdoc.into_bytes());
    pkg.put_part(
        "/[Content_Types].xml",
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types \
             xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" \
             ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"png\" ContentType=\"image/png\"/>\
             <Override PartName=\"/FixedDocSeq.fdseq\" \
             ContentType=\"application/vnd.ms-package.xps-fixeddocumentsequence+xml\"/>\
             <Override PartName=\"/Documents/1/FixedDoc.fdoc\" \
             ContentType=\"application/vnd.ms-package.xps-fixeddocument+xml\"/>{overrides}</Types>"
        )
        .into_bytes(),
    );
    pkg.write().map_err(|e| e.to_string())
}

/// Width and height of a PNG (IHDR, ISO/IEC 15948 §11.2.2).
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    let w = u32::from_be_bytes(png.get(16..20)?.try_into().ok()?);
    let h = u32::from_be_bytes(png.get(20..24)?.try_into().ok()?);
    Some((w, h))
}

/// Which kind of `name` (index into the Converting To PDF list), if any.
pub(crate) fn kind_of(name: &str) -> Option<usize> {
    let lower = name.to_ascii_lowercase();
    KINDS
        .iter()
        .position(|(_, exts)| exts.iter().any(|e| lower.ends_with(e)))
}

impl EditorWindow {
    /// Round 9 Convert formats; false when `cmd` is not one.
    pub(crate) fn conv9b_command(&mut self, cmd: &str) -> bool {
        let stem = self.doc_stem();
        match cmd {
            "conv9.xml2003" => {
                let r = self
                    .with_doc(crate::editor_convert::pdf_cells)
                    .map(|cells| xml_spreadsheet(&cells, "Sheet1"));
                self.save9(format!("{stem}.xml"), r);
            }
            "conv9.xml" => {
                let r = self.with_doc(xml_dump);
                self.save9(format!("{stem}.xml"), r);
            }
            "conv9.xps" => self.push(ReaderRequest::RenderPages {
                purpose: "xps".into(),
                scale: 150.0 / 72.0,
            }),
            "conv9.settings" => self.open_conv_settings9(0),
            _ => return false,
        }
        true
    }

    fn save9(&mut self, name: String, r: Result<Vec<u8>, String>) {
        match r {
            Ok(bytes) => self.push(ReaderRequest::SaveBytes { name, bytes }),
            Err(e) => self.set_status(&e),
        }
    }

    /// Pages rendered for XPS; false for other purposes.
    pub(crate) fn conv9b_rendered(&mut self, purpose: &str, pngs: &[Vec<u8>]) -> bool {
        if purpose != "xps" {
            return false;
        }
        let sizes: Vec<(f32, f32)> = self.r.view().map_or_else(Vec::new, |v| {
            (0..v.page_count()).map(|i| v.page_size(i)).collect()
        });
        let pages: Vec<(Vec<u8>, f32, f32)> = pngs
            .iter()
            .zip(sizes)
            .map(|(p, (w, h))| (p.clone(), w, h))
            .collect();
        let stem = self.doc_stem();
        let r = xps_of(&pages);
        self.save9(format!("{stem}.xps"), r);
        true
    }

    /// Open Converting To PDF on kind `row`.
    pub(crate) fn open_conv_settings9(&mut self, row: usize) {
        let t = |k: &str| self.t(k);
        let s = self.conv9_settings;
        let kind = t(KINDS[row.min(2)].0);
        let c = vec![
            Ctl::Table {
                r: [24.0, 42.0, 240.0, 563.0],
                cols: vec![(String::new(), 240.0)],
                rows: KINDS.iter().map(|k| vec![t(k.0)]).collect(),
            },
            Ctl::Frame {
                r: [279.0, 42.0, 550.0, 563.0],
            },
            label(
                307.0,
                82.0,
                t("pdfEditor9.convSettingsFor").replace("{{kind}}", &kind),
            ),
            label(307.0, 147.0, t("pdfEditor9.convSettingsLabel")),
            combo(
                "preset",
                [307.0, 177.0, 340.0, 27.0],
                vec![t("pdfEditor9.standard")],
                On::Always,
            ),
            button(
                "c9edit",
                [663.0, 174.0, 108.0, 33.0],
                t("pdfEditor9.editDots"),
                On::Never,
            ),
            check("bm", 307.0, 245.0, t("pdfEditor9.addBookmarks")),
            check("tag", 307.0, 280.0, t("pdfEditor9.taggedPdf")),
            button(
                "ok",
                [577.0, 622.0, 123.0, 37.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [715.0, 622.0, 123.0, 37.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor9.convSettingsTitle"), (857.0, 678.0), c)
            .with("row", Val::Pick(row))
            .with("preset", Val::Pick(0))
            .with("bm", Val::Pick(usize::from(s.bookmarks[row])))
            .with("tag", Val::Pick(usize::from(s.tagged[row])));
        self.form8.push((f, FormWhat::ConvSettings9(row)));
    }

    /// Keep the shown kind's choices (on OK, or before showing another kind).
    pub(crate) fn conv_settings9_keep(&mut self, f: &Form, row: usize) {
        if let Some(b) = self.conv9_settings.bookmarks.get_mut(row) {
            *b = f.pick("bm") == 1;
        }
        if let Some(b) = self.conv9_settings.tagged.get_mut(row) {
            *b = f.pick("tag") == 1;
        }
    }

    /// A row of the kinds list was picked: keep the shown kind's values, show the other.
    pub(crate) fn conv_settings9_row(&mut self) -> bool {
        let Some((f, FormWhat::ConvSettings9(row))) = self.form8.last() else {
            return false;
        };
        let (f, row) = (f.clone(), *row);
        let new = f.pick("row");
        if new == row {
            return true;
        }
        self.conv_settings9_keep(&f, row);
        self.form8.pop();
        self.open_conv_settings9(new);
        true
    }

    /// PDF of an office file with the Converting To PDF choices of its kind.
    pub(crate) fn office_to_pdf9(&self, name: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let Some(k) = kind_of(name) else {
            return crate::editor_actions::office_to_pdf(name, bytes);
        };
        let s = self.conv9_settings;
        let choices = bo_pdf::ExportChoices {
            pdfa: None,
            tagged: s.tagged[k],
            bookmarks: s.bookmarks[k],
        };
        if choices == bo_pdf::ExportChoices::default() {
            return crate::editor_actions::office_to_pdf(name, bytes);
        }
        if k == 1 {
            let mut e = bo_engine::SheetEngine::with_bundled_fonts();
            e.open(bytes, name).map_err(|e| e.to_string())?;
            e.export_pdf_with(&choices).map_err(|e| e.to_string())
        } else {
            let mut e = bo_engine::Engine::with_bundled_fonts();
            e.open(bytes, name).map_err(|e| e.to_string())?;
            e.export_pdf_with(&choices).map_err(|e| e.to_string())
        }
    }
}
