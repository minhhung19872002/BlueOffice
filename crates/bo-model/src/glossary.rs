//! Glossary document: the building blocks stored in a document or template
//! (ECMA-376 Part 1, §17.12 Glossary Document; part `word/glossary/document.xml`,
//! §11.3.8). A glossary has its own styles and numbering parts; its pictures live
//! in [`crate::Document::media`] with the other media of the package.

use crate::write::{fingerprint, Block, Numbering, ParaProps, Raw, RunProps, StyleSheet};
use serde::{Deserialize, Serialize};

/// §17.18.15 `ST_DocPartGallery` values BlueDoc offers when saving a block.
pub const GALLERIES: &[&str] = &[
    "autoTxt",
    "docParts",
    "hdrs",
    "ftrs",
    "pgNum",
    "coverPg",
    "txtBox",
    "tbls",
    "eq",
    "watermarks",
    "bib",
    "custom1",
    "custom2",
    "custom3",
    "custom4",
    "custom5",
];

/// §17.12.7 docPartPr: the properties of a building block.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocPartProps {
    /// §17.12.13 name.
    pub name: String,
    /// §17.12.9 gallery (`ST_DocPartGallery`: `autoTxt`, `docParts`, `hdrs`, …).
    pub gallery: String,
    /// §17.12.3 category / name.
    pub category: String,
    /// §17.12.4 description.
    #[serde(default)]
    pub description: String,
    /// §17.12.15 types (`ST_DocPartType`: `normal`, `autoExp`, `bbPlcHdr`, …).
    #[serde(default)]
    pub types: Vec<String>,
    /// §17.12.2 behaviors (`ST_DocPartBehavior`: `content`, `p`, `pg`).
    #[serde(default)]
    pub behaviors: Vec<String>,
    /// §17.12.11 guid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    /// §17.12.14 style: the style associated with the entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

impl DocPartProps {
    /// How the block is inserted: `content` (in line), `p` (in paragraphs of its
    /// own) or `pg` (on a page of its own); the first behaviour listed wins.
    pub fn behavior(&self) -> &str {
        self.behaviors.first().map_or("content", String::as_str)
    }

    /// A glossary document entry that only backs a placeholder of a content
    /// control (§17.18.16 `bbPlcHdr`), not offered in the galleries.
    pub fn is_placeholder(&self) -> bool {
        self.gallery == "placeholder" || self.types.iter().any(|t| t == "bbPlcHdr")
    }
}

/// §17.12.5 docPart: one building block.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DocPart {
    /// Properties (§17.12.7 docPartPr).
    pub props: DocPartProps,
    /// Content (§17.12.6 docPartBody).
    pub blocks: Vec<Block>,
    /// Original `w:docPart` XML.
    #[serde(skip)]
    pub src: Option<Raw>,
}

impl DocPart {
    /// Fingerprint of the modelled content (unchanged ⇒ the source XML is kept).
    pub fn content_fingerprint(&self) -> u64 {
        let blocks: Vec<u64> = self
            .blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph(p) => p.content_fingerprint(),
                Block::Table(t) => t.content_fingerprint(),
                Block::Opaque(r) => fingerprint(&r.xml),
            })
            .collect();
        fingerprint(&(&self.props, blocks))
    }

    /// The entry is exactly as it was read.
    pub fn unchanged(&self) -> bool {
        self.src
            .as_ref()
            .is_some_and(|r| r.fingerprint == self.content_fingerprint())
    }
}

/// Where a glossary was read from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GlossarySource {
    /// Part name (`/word/glossary/document.xml`).
    pub part: String,
    /// Text of the part up to and including `<w:docParts>` (empty: no such element).
    pub prefix: String,
    /// Text from `</w:docParts>` to the end.
    pub suffix: String,
    /// Fingerprint of the glossary's styles as read.
    pub styles_fingerprint: u64,
    /// Fingerprint of the glossary's numbering as read.
    pub numbering_fingerprint: u64,
    /// Number of entries read.
    pub count: usize,
}

/// §17.12.10 glossaryDocument with the parts it refers to.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Glossary {
    /// Entries in part order (§17.12.8 docParts).
    pub parts: Vec<DocPart>,
    /// Styles of the glossary document (its own `styles.xml`).
    #[serde(default)]
    pub styles: StyleSheet,
    /// `w:docDefaults` run properties of the glossary's styles part.
    #[serde(default)]
    pub default_run: RunProps,
    /// `w:docDefaults` paragraph properties of the glossary's styles part.
    #[serde(default)]
    pub default_para: ParaProps,
    /// Numbering of the glossary document (its own `numbering.xml`).
    #[serde(default)]
    pub numbering: Numbering,
    /// Source part (`None` for a glossary created here).
    #[serde(skip)]
    pub source: Option<GlossarySource>,
}

impl Glossary {
    /// Nothing changed since the glossary was read: every part of it is kept
    /// byte for byte.
    pub fn unchanged(&self) -> bool {
        self.source
            .as_ref()
            .is_some_and(|s| s.count == self.parts.len())
            && self.parts.iter().all(DocPart::unchanged)
    }

    /// Entries offered in the galleries (placeholders of content controls left out),
    /// with their index.
    pub fn listed(&self) -> impl Iterator<Item = (usize, &DocPart)> {
        self.parts
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.props.is_placeholder())
    }
}
