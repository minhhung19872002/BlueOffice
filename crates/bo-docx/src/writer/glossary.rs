//! [`Glossary`] → glossary document part (ECMA-376 Part 1, §17.12; part §11.3.8).
//!
//! An untouched glossary keeps every one of its parts byte for byte. When entries
//! were added, removed or edited, `document.xml` of the glossary is rewritten —
//! unchanged `w:docPart`s verbatim — together with the glossary's own styles and
//! numbering parts when those changed, and its relationships.

use super::{
    find_element, with_namespace, Rels, Writer, A_NS, PIC_NS, REL_NUMBERING, R_NS, WP_NS, W_NS,
};
use crate::reader::glossary::REL_GLOSSARY;
use crate::{Result, CT_STYLES};
use bo_model::glossary::{DocPart, Glossary};
use bo_model::{fingerprint, Document, SourcePackage};
use bo_opc::xml::escape;
use bo_opc::{Package, REL_STYLES};
use std::fmt::Write as _;
use std::sync::Arc;

/// Content type of the glossary document part (ECMA-376 Part 1, §11.3.8).
pub const CT_GLOSSARY: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.glossary+xml";
const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const DEFAULT_PART: &str = "/word/glossary/document.xml";

/// §17.12.5 docPart with §17.12.7 docPartPr and §17.12.6 docPartBody.
fn doc_part_xml(w: &mut Writer<'_>, p: &DocPart) -> String {
    let pr = &p.props;
    // §17.12.13 name, §17.12.14 style, §17.12.3 category (name, gallery).
    let mut s = format!(
        "<w:docPart><w:docPartPr><w:name w:val=\"{}\"/>",
        escape(&pr.name)
    );
    if let Some(st) = &pr.style {
        let _ = write!(s, "<w:style w:val=\"{}\"/>", escape(st));
    }
    let _ = write!(
        s,
        "<w:category><w:name w:val=\"{}\"/><w:gallery w:val=\"{}\"/></w:category>",
        escape(if pr.category.is_empty() {
            "General"
        } else {
            &pr.category
        }),
        escape(if pr.gallery.is_empty() {
            "docParts"
        } else {
            &pr.gallery
        })
    );
    // §17.12.15 types, §17.12.2 behaviors.
    if !pr.types.is_empty() {
        s.push_str("<w:types>");
        for t in &pr.types {
            let _ = write!(s, "<w:type w:val=\"{}\"/>", escape(t));
        }
        s.push_str("</w:types>");
    }
    if !pr.behaviors.is_empty() {
        s.push_str("<w:behaviors>");
        for b in &pr.behaviors {
            let _ = write!(s, "<w:behavior w:val=\"{}\"/>", escape(b));
        }
        s.push_str("</w:behaviors>");
    }
    // §17.12.4 description, §17.12.11 guid.
    if !pr.description.is_empty() {
        let _ = write!(s, "<w:description w:val=\"{}\"/>", escape(&pr.description));
    }
    if let Some(g) = &pr.guid {
        let _ = write!(s, "<w:guid w:val=\"{}\"/>", escape(g));
    }
    s.push_str("</w:docPartPr><w:docPartBody>");
    s.push_str(&w.blocks(&p.blocks));
    s.push_str("</w:docPartBody></w:docPart>");
    s
}

/// Remove the glossary of the source package (the document has none any more).
fn remove(w: &mut Writer<'_>, part: &str) {
    let dir = part.rsplit_once('/').map_or("", |(d, _)| d).to_owned() + "/";
    let gone: Vec<String> = w
        .pkg
        .parts()
        .iter()
        .map(|p| p.name.clone())
        .filter(|n| n.starts_with(&dir))
        .collect();
    for n in gone {
        w.pkg.remove_part(&n);
    }
    w.rels.list.retain(|r| r.rel_type != REL_GLOSSARY);
    w.rels.changed = true;
}

impl Writer<'_> {
    /// Write the glossary document of the document being written (`main` is its
    /// main part, whose relationships gain or lose the glossary relationship).
    pub(super) fn write_glossary(&mut self, main: &str) -> Result<()> {
        let doc = self.doc;
        let existing = self.pkg.related_part(main, REL_GLOSSARY)?;
        let g: &Glossary = match doc.glossary.as_deref() {
            Some(g) if !g.parts.is_empty() => g,
            _ => {
                if let Some(part) = existing {
                    remove(self, &part);
                }
                return Ok(());
            }
        };
        // Entries read from this very part can be kept verbatim.
        let same_source = match (&existing, &g.source) {
            (Some(part), Some(src)) => *part == src.part && self.pkg.part(part).is_some(),
            _ => false,
        };
        if same_source && g.unchanged() {
            return Ok(());
        }
        let part = existing.unwrap_or_else(|| DEFAULT_PART.to_owned());
        let dir = part.rsplit_once('/').map_or("/word", |(d, _)| d).to_owned();
        let src = g.source.as_ref().filter(|_| same_source);
        // The glossary is a document of its own: its styles, numbering and
        // relationships are written by a writer over a scratch document.
        let gdoc = Document {
            styles: g.styles.clone(),
            default_run: g.default_run.clone(),
            default_para: g.default_para.clone(),
            numbering: g.numbering.clone(),
            media: doc.media.clone(),
            source: src.map(|s| {
                Arc::new(SourcePackage {
                    main_part: part.clone(),
                    styles_fingerprint: s.styles_fingerprint,
                    numbering_fingerprint: s.numbering_fingerprint,
                    ..SourcePackage::default()
                })
            }),
            ..Document::default()
        };
        let list = self.pkg.relationships(&part)?;
        let mut sub = Writer {
            doc: &gdoc,
            pkg: std::mem::replace(&mut self.pkg, Package::new()),
            rels: Rels {
                part: part.clone(),
                list,
                changed: false,
            },
            hf_counter: self.hf_counter,
            drawing_id: self.drawing_id,
            in_endnote: false,
            sdt_depth: 0,
            form_depth: 0,
            diagram_parts: std::collections::BTreeMap::new(),
            picture_media: std::collections::BTreeSet::new(),
        };
        let res = sub.write_glossary_parts(g, &part, &dir, src.is_some());
        // Hand the package back whatever happened.
        self.pkg = std::mem::replace(&mut sub.pkg, Package::new());
        self.hf_counter = sub.hf_counter;
        self.drawing_id = sub.drawing_id;
        self.diagram_parts.append(&mut sub.diagram_parts);
        self.picture_media.append(&mut sub.picture_media);
        res?;
        self.rels.id_for(REL_GLOSSARY, &part, false);
        Ok(())
    }

    /// `self` writes the scratch document of glossary `g` into part `part`.
    fn write_glossary_parts(
        &mut self,
        g: &Glossary,
        part: &str,
        dir: &str,
        from_source: bool,
    ) -> Result<()> {
        let default_prefix = || {
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:glossaryDocument xmlns:w=\"{W_NS}\"><w:docParts>"
            )
        };
        let (mut prefix, suffix) = match g.source.as_ref().filter(|_| from_source) {
            Some(s) if !s.prefix.is_empty() => (s.prefix.clone(), s.suffix.clone()),
            _ => (
                default_prefix(),
                "</w:docParts></w:glossaryDocument>".to_owned(),
            ),
        };
        // Regenerated entries use these prefixes (drawings, relationships).
        if let Some(at) = find_element(&prefix, "w:glossaryDocument") {
            if let Some(len) = prefix[at..].find('>') {
                let mut root = prefix[at..=at + len].to_owned();
                for (p, ns) in [("r", R_NS), ("wp", WP_NS), ("a", A_NS), ("pic", PIC_NS)] {
                    root = with_namespace(&root, p, ns);
                }
                prefix.replace_range(at..=at + len, &root);
            }
        }
        let mut body = String::new();
        for p in &g.parts {
            match &p.src {
                Some(raw) if from_source && p.unchanged() => body.push_str(&raw.xml),
                _ => body.push_str(&doc_part_xml(self, p)),
            }
        }
        let xml = format!("{prefix}{body}{suffix}").into_bytes();
        if self.pkg.part(part).is_some() {
            self.pkg.put_part(part, xml);
        } else {
            self.pkg.add_part(part, CT_GLOSSARY, xml)?;
            self.rels.changed = true;
        }
        // The glossary's own styles part (§17.7.4.18) and numbering part (§17.9.17).
        let styles_changed =
            g.source.as_ref().filter(|_| from_source).is_none_or(|s| {
                s.styles_fingerprint != crate::reader::styles_fingerprint(self.doc)
            });
        if styles_changed && self.pkg.related_part(part, REL_STYLES)?.is_none() {
            let name = format!("{dir}/styles.xml");
            self.pkg.add_part(&name, CT_STYLES, Vec::new())?;
            self.rels.id_for(REL_STYLES, &name, false);
            let rels = self.rels.list.clone();
            self.pkg.set_relationships(part, &rels);
        }
        self.write_styles(part)?;
        let numbering_changed = g
            .source
            .as_ref()
            .filter(|_| from_source)
            .is_none_or(|s| s.numbering_fingerprint != fingerprint(&g.numbering));
        let has_numbering = !g.numbering.abstracts.is_empty() || !g.numbering.nums.is_empty();
        if numbering_changed
            && has_numbering
            && self.pkg.related_part(part, REL_NUMBERING)?.is_none()
        {
            let name = format!("{dir}/numbering.xml");
            self.pkg.add_part(&name, CT_NUMBERING, Vec::new())?;
            self.rels.id_for(REL_NUMBERING, &name, false);
            let rels = self.rels.list.clone();
            self.pkg.set_relationships(part, &rels);
        }
        self.write_numbering(part)?;
        if self.rels.changed {
            let rels = self.rels.list.clone();
            self.pkg.set_relationships(part, &rels);
        }
        Ok(())
    }
}
