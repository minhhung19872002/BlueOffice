//! BlueDoc building blocks (DOC-BB-001): the entries of the glossary document
//! (ECMA-376 Part 1, §17.12) of the open document, of the user's own store (a
//! `.dotx` the host keeps) and of the library shipped with BlueDoc ([`library`]).
//!
//! Commands: list, save the selection as a block, insert (at the caret, as header /
//! footer / page number, as cover page — by the block's gallery), edit properties,
//! delete, AutoText expansion (F3) and a preview picture drawn by the layout engine.
//! Edits of the document are one undo step each (see `bo_edit::blocks`); results
//! carry the answer in `blocks`.

pub mod library;

use crate::edit::EditResult;
use crate::{EngineError, Result};
use bo_edit::blocks::{detached, glossary_document, import_blocks, BlockTarget};
use bo_model::glossary::{DocPart, DocPartProps, Glossary, GALLERIES};
use bo_model::{Block, Document, PageSetup, Paragraph, Section, Twips};
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Where a block is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockStore {
    /// The glossary document of the open document / template.
    Document,
    /// The user's building blocks file.
    User,
    /// The library shipped with BlueDoc (read-only).
    Builtin,
}

impl BlockStore {
    fn prefix(self) -> &'static str {
        match self {
            BlockStore::Document => "doc",
            BlockStore::User => "user",
            BlockStore::Builtin => "builtin",
        }
    }
}

/// Properties of a block as the dialogs edit them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSpec {
    /// Name (§17.12.13).
    pub name: String,
    /// Gallery (§17.18.15 `ST_DocPartGallery`).
    pub gallery: String,
    /// Category name (§17.12.12).
    #[serde(default)]
    pub category: String,
    /// Description (§17.12.4).
    #[serde(default)]
    pub description: String,
    /// Insert behaviour (§17.18.14: `content`, `p`, `pg`).
    #[serde(default)]
    pub behavior: String,
}

/// A building block command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BlockCommand {
    /// Every block of the document, the user's store and the library → `blocks.list`.
    BuildingBlocks,
    /// Save the selection as a block in `store` (`document` or `user`).
    SaveBuildingBlock {
        /// Properties.
        spec: BlockSpec,
        /// Where to keep it.
        store: BlockStore,
    },
    /// Insert block `key`: at the caret, or — by its gallery — as the header or
    /// footer of the page (`page`, default: the caret's page) or as a cover page.
    InsertBuildingBlock {
        /// Key from `blocks.list`.
        key: String,
        /// Page whose header / footer is replaced.
        #[serde(default)]
        page: Option<usize>,
    },
    /// Change the properties of block `key`.
    EditBuildingBlock {
        /// Key from `blocks.list`.
        key: String,
        /// New properties.
        spec: BlockSpec,
    },
    /// Delete block `key`.
    DeleteBuildingBlock {
        /// Key from `blocks.list`.
        key: String,
    },
    /// AutoText (F3): replace the block name typed before the caret by the block.
    ExpandAutoText,
    /// Picture of block `key` → `blocks.preview` (PNG, base64).
    BuildingBlockPreview {
        /// Key from `blocks.list`.
        key: String,
    },
    /// Load the user's building blocks file (`.dotx` bytes, base64; empty: none).
    LoadBlockStore {
        /// The file.
        file: String,
    },
}

const NAMES: &[&str] = &[
    "buildingBlocks",
    "saveBuildingBlock",
    "insertBuildingBlock",
    "editBuildingBlock",
    "deleteBuildingBlock",
    "expandAutoText",
    "buildingBlockPreview",
    "loadBlockStore",
];

impl BlockCommand {
    /// Parses a JSON command when it is a building block command.
    pub fn parse(v: &serde_json::Value) -> Option<BlockCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !NAMES.contains(&c) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    /// The command changes the open document.
    fn edits(&self) -> bool {
        match self {
            BlockCommand::SaveBuildingBlock { store, .. } => *store == BlockStore::Document,
            BlockCommand::EditBuildingBlock { key, .. }
            | BlockCommand::DeleteBuildingBlock { key } => key.starts_with("doc:"),
            BlockCommand::InsertBuildingBlock { .. } | BlockCommand::ExpandAutoText => true,
            _ => false,
        }
    }
}

/// One block of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockInfo {
    /// Key for the other commands (`doc:3`, `user:0`, `builtin:12`).
    pub key: String,
    /// Where the block is kept.
    pub store: BlockStore,
    /// Name.
    pub name: String,
    /// Gallery (`ST_DocPartGallery`).
    pub gallery: String,
    /// Category.
    pub category: String,
    /// Description.
    pub description: String,
    /// Insert behaviour (`content`, `p`, `pg`).
    pub behavior: String,
    /// Beginning of the block's text (AutoComplete tip, lists).
    pub text: String,
}

/// Answer of a building block command.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocksOut {
    /// Every block (document, user, library).
    pub list: Vec<BlockInfo>,
    /// The user's building blocks file after a change to it (`.dotx`, base64): the
    /// host stores it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<String>,
    /// Preview picture (`buildingBlockPreview`; PNG, base64).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// Name of the block an insert / F3 placed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inserted: Option<String>,
}

/// Building block state of the engine (kept across documents).
#[derive(Debug, Default)]
pub(crate) struct BlockState {
    /// The user's building blocks file.
    user: Option<Document>,
    /// The shipped library, built on first use.
    builtin: OnceCell<Document>,
    /// Preview pictures by block fingerprint.
    previews: BTreeMap<u64, String>,
}

/// Pixels per point of the previews.
const PREVIEW_SCALE: f32 = 0.5;
/// Pixels per point of a whole-page preview (cover pages).
const PAGE_PREVIEW_SCALE: f32 = 0.25;

fn summary(blocks: &[Block]) -> String {
    let mut out = String::new();
    let mut copy = blocks.to_vec();
    bo_edit::blocks::for_each_paragraph_mut(&mut copy, &mut |p| {
        if out.chars().count() < 120 {
            let t = p.plain_text().replace('\u{FFFC}', "");
            let t = t.trim();
            if !t.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(t);
            }
        }
    });
    out.chars().take(120).collect()
}

fn infos(store: BlockStore, g: Option<&Glossary>, out: &mut Vec<BlockInfo>) {
    let Some(g) = g else { return };
    for (i, p) in g.listed() {
        out.push(BlockInfo {
            key: format!("{}:{i}", store.prefix()),
            store,
            name: p.props.name.clone(),
            gallery: p.props.gallery.clone(),
            category: p.props.category.clone(),
            description: p.props.description.clone(),
            behavior: p.props.behavior().to_owned(),
            text: summary(&p.blocks),
        });
    }
}

fn parse_key(key: &str) -> Option<(BlockStore, usize)> {
    let (s, i) = key.split_once(':')?;
    let store = match s {
        "doc" => BlockStore::Document,
        "user" => BlockStore::User,
        "builtin" => BlockStore::Builtin,
        _ => return None,
    };
    Some((store, i.parse().ok()?))
}

/// Checked properties of a new or edited block (`badName`, `badGallery`).
fn props_of(
    spec: &BlockSpec,
    old: Option<&DocPartProps>,
) -> std::result::Result<DocPartProps, String> {
    let name = spec.name.trim();
    // §17.12.13 name: the schema allows no comma in a block name.
    if name.is_empty() || name.chars().count() > 255 || name.contains(',') {
        return Err("badName".into());
    }
    let known = GALLERIES.contains(&spec.gallery.as_str())
        || matches!(spec.gallery.as_str(), "pgNumT" | "pgNumB" | "pgNumMargins");
    if !known && old.is_none_or(|o| o.gallery != spec.gallery) {
        return Err("badGallery".into());
    }
    let behavior = match spec.behavior.as_str() {
        "p" | "pg" => spec.behavior.clone(),
        _ => "content".to_owned(),
    };
    let category = spec.category.trim();
    Ok(DocPartProps {
        name: name.to_owned(),
        gallery: spec.gallery.clone(),
        category: if category.is_empty() {
            "General".to_owned()
        } else {
            category.to_owned()
        },
        description: spec.description.trim().to_owned(),
        types: old.map_or_else(|| vec!["normal".to_owned()], |o| o.types.clone()),
        behaviors: vec![behavior],
        guid: old.and_then(|o| o.guid.clone()),
        style: old.and_then(|o| o.style.clone()),
    })
}

/// A §17.12.11 guid for a new entry, derived from its content and position (the
/// engine has no random source in the browser).
fn new_guid(props: &DocPartProps, blocks: &[Block], salt: usize) -> String {
    let part = DocPart {
        props: props.clone(),
        blocks: blocks.to_vec(),
        src: None,
    };
    let a = part.content_fingerprint();
    let b = bo_model::fingerprint(&(a, salt, &props.name));
    format!(
        "{{{:08X}-{:04X}-4{:03X}-8{:03X}-{:012X}}}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        a & 0xFFF,
        (b >> 48) & 0xFFF,
        b & 0xFFFF_FFFF_FFFF
    )
}

/// An empty glossary for a document: its defaults and default paragraph style, so
/// unstyled block content keeps its look.
fn new_glossary(doc: &Document) -> Glossary {
    let mut scratch = Document::default();
    let ids: Vec<String> = doc
        .styles
        .default_of(bo_model::StyleKind::Paragraph)
        .map(|s| s.id.clone())
        .into_iter()
        .collect();
    bo_edit::styles::copy_styles(doc, &mut scratch, &ids);
    Glossary {
        styles: scratch.styles,
        numbering: scratch.numbering,
        default_run: doc.default_run.clone(),
        default_para: doc.default_para.clone(),
        ..Glossary::default()
    }
}

/// Add (or redefine: same name, gallery and category) an entry of `g` whose content
/// `blocks` comes from `from`; pictures `g`'s package lacks are returned.
fn add_part(
    g: &mut Glossary,
    from: &Document,
    media: &BTreeMap<String, Arc<Vec<u8>>>,
    mut props: DocPartProps,
    mut blocks: Vec<Block>,
) -> Vec<(String, Arc<Vec<u8>>)> {
    let mut scratch = glossary_document(g, media);
    let added = import_blocks(from, &mut scratch, media, &mut blocks);
    g.styles = scratch.styles;
    g.numbering = scratch.numbering;
    let same = g.parts.iter().position(|p| {
        p.props.name.to_lowercase() == props.name.to_lowercase()
            && p.props.gallery == props.gallery
            && p.props.category == props.category
    });
    if let Some(i) = same {
        props.guid.clone_from(&g.parts[i].props.guid);
        g.parts[i] = DocPart {
            props,
            blocks,
            src: None,
        };
    } else {
        props.guid = Some(new_guid(&props, &blocks, g.parts.len()));
        g.parts.push(DocPart {
            props,
            blocks,
            src: None,
        });
    }
    added
}

/// The store as `.dotx` bytes, base64.
fn store_file(store: &Document) -> std::result::Result<String, String> {
    let bytes = bo_docx::write_docx(store).map_err(|e| e.to_string())?;
    let bytes = crate::variant::convert(&bytes, crate::variant::Variant::Dotx)
        .map_err(|e| e.to_string())?
        .bytes;
    Ok(crate::ole::encode(&bytes))
}

fn empty_store() -> Document {
    Document {
        sections: vec![Section::new(vec![Block::Paragraph(Paragraph::default())])],
        ..Document::default()
    }
}

/// Where the blocks of a gallery go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GalleryTarget {
    /// Replaces the header of the page.
    Header,
    /// Replaces the footer of the page.
    Footer,
    /// At the start of the document, on a page of its own.
    Cover,
    /// At the caret.
    Caret,
}

/// Target of a block by its gallery (§17.18.15): headers and top page numbers
/// replace the header, footers and bottom page numbers the footer, cover pages go
/// to the start of the document, everything else to the caret.
fn gallery_target(gallery: &str) -> GalleryTarget {
    match gallery {
        "hdrs" | "custHdrs" | "pgNumT" | "custPgNumT" => GalleryTarget::Header,
        "ftrs" | "custFtrs" | "pgNumB" | "custPgNumB" => GalleryTarget::Footer,
        "coverPg" | "custCoverPg" => GalleryTarget::Cover,
        _ => GalleryTarget::Caret,
    }
}

impl crate::Engine {
    fn builtin_blocks(&self) -> &Document {
        self.blocks.builtin.get_or_init(library::library)
    }

    /// Every building block: the document's, the user's, the library's.
    pub fn building_blocks(&self) -> Vec<BlockInfo> {
        let mut out = Vec::new();
        infos(
            BlockStore::Document,
            self.document().and_then(|d| d.glossary.as_deref()),
            &mut out,
        );
        infos(
            BlockStore::User,
            self.blocks
                .user
                .as_ref()
                .and_then(|d| d.glossary.as_deref()),
            &mut out,
        );
        infos(
            BlockStore::Builtin,
            self.builtin_blocks().glossary.as_deref(),
            &mut out,
        );
        out
    }

    /// Block `key` with the scratch document that defines its styles, numbering
    /// and pictures.
    fn block(&self, key: &str) -> Option<(Document, DocPart)> {
        let (store, i) = parse_key(key)?;
        let doc = match store {
            BlockStore::Document => self.document()?,
            BlockStore::User => self.blocks.user.as_ref()?,
            BlockStore::Builtin => self.builtin_blocks(),
        };
        let g = doc.glossary.as_deref()?;
        let part = g.parts.get(i)?.clone();
        Some((glossary_document(g, &doc.media), part))
    }

    /// The user's building blocks file as it is now (`.dotx`), if any.
    pub fn block_store(&self) -> Option<Vec<u8>> {
        let store = self.blocks.user.as_ref()?;
        let bytes = bo_docx::write_docx(store).ok()?;
        crate::variant::convert(&bytes, crate::variant::Variant::Dotx)
            .ok()
            .map(|c| c.bytes)
    }

    /// Run a building block command. Failures are reported in `error` (keys
    /// `noBlock`, `badName`, `badGallery`, `emptySelection`, `builtinBlock`,
    /// `noAutoText`, `bodyOnly`, `badFile`, `readOnly`, `protected`).
    pub fn doc_block_command(&mut self, cmd: BlockCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let mut out = BlocksOut::default();
        // The user's store is engine state: it loads before any document is open
        // (the answer then is the "no document" error, the store is kept).
        let cmd = match cmd {
            BlockCommand::LoadBlockStore { file } if self.session.is_none() => {
                let _ = self.run_block_command(BlockCommand::LoadBlockStore { file }, &mut out);
                return Err(EngineError::NoDocument);
            }
            _ if self.session.is_none() => return Err(EngineError::NoDocument),
            other => other,
        };
        let res = if self.sec.read_only && cmd.edits() {
            Err(crate::security::READ_ONLY.to_owned())
        } else if cmd.edits()
            && self
                .session
                .as_ref()
                .is_some_and(bo_edit::Session::forms_locked)
        {
            Err("protected".to_owned())
        } else {
            self.run_block_command(cmd, &mut out)
        };
        let dirty = self.relayout()?;
        let mut r = self.story_result(dirty, res.err())?;
        out.list = self.building_blocks();
        r.blocks = Some(Box::new(out));
        r.collab = self.collab_after_edit();
        Ok(r)
    }

    fn run_block_command(
        &mut self,
        cmd: BlockCommand,
        out: &mut BlocksOut,
    ) -> std::result::Result<(), String> {
        let key_err = |e: bo_edit::EditError| match e {
            bo_edit::EditError::Unsupported(k) => k,
            other => other.to_string(),
        };
        match cmd {
            BlockCommand::BuildingBlocks => Ok(()),
            BlockCommand::LoadBlockStore { file } => {
                if file.is_empty() {
                    self.blocks.user = None;
                    return Ok(());
                }
                let bytes =
                    crate::merge::base64_decode(&file).ok_or_else(|| "badFile".to_owned())?;
                let doc = bo_docx::read_docx(&bytes).map_err(|_| "badFile".to_owned())?;
                self.blocks.user = Some(doc);
                Ok(())
            }
            BlockCommand::SaveBuildingBlock { spec, store } => {
                let props = props_of(&spec, None)?;
                if self.hf.is_some() || self.note.is_some() || self.textbox.is_some() {
                    return Err("bodyOnly".into());
                }
                let s = self.session.as_mut().ok_or("noDocument")?;
                if s.selection().is_collapsed() {
                    return Err("emptySelection".into());
                }
                let blocks = s.selection_block_content();
                if blocks.is_empty() {
                    return Err("emptySelection".into());
                }
                match store {
                    BlockStore::Document => {
                        let doc = s.doc();
                        let mut g = doc
                            .glossary
                            .as_deref()
                            .cloned()
                            .unwrap_or_else(|| new_glossary(doc));
                        // The glossary shares the package's media: nothing to add.
                        let _ = add_part(&mut g, doc, &doc.media, props, blocks);
                        s.set_glossary(Some(Arc::new(g))).map_err(key_err)
                    }
                    BlockStore::User => {
                        let doc = s.doc();
                        let user = self.blocks.user.get_or_insert_with(empty_store);
                        let mut g = user
                            .glossary
                            .as_deref()
                            .cloned()
                            .unwrap_or_else(|| new_glossary(doc));
                        let added = add_part(&mut g, doc, &user.media, props, blocks);
                        user.media.extend(added);
                        user.glossary = Some(Arc::new(g));
                        out.store = Some(store_file(user)?);
                        Ok(())
                    }
                    BlockStore::Builtin => Err("builtinBlock".into()),
                }
            }
            BlockCommand::EditBuildingBlock { key, spec } => {
                self.change_block(&key, out, |g, i| {
                    let props = props_of(&spec, Some(&g.parts[i].props))?;
                    g.parts[i].props = props;
                    Ok(())
                })
            }
            BlockCommand::DeleteBuildingBlock { key } => self.change_block(&key, out, |g, i| {
                g.parts.remove(i);
                Ok(())
            }),
            BlockCommand::InsertBuildingBlock { key, page } => {
                let (from, part) = self.block(&key).ok_or("noBlock")?;
                let target = match gallery_target(&part.props.gallery) {
                    t @ (GalleryTarget::Header | GalleryTarget::Footer) => {
                        self.close_header_footer().map_err(|e| e.to_string())?;
                        let pi = page.or_else(|| self.caret_page()).unwrap_or(0);
                        let (section, variant) = self
                            .variant_of(pi)
                            .unwrap_or((0, bo_edit::section::HfVariant::Default));
                        BlockTarget::HeaderFooter {
                            section,
                            header: t == GalleryTarget::Header,
                            variant,
                        }
                    }
                    GalleryTarget::Cover => {
                        self.close_header_footer().map_err(|e| e.to_string())?;
                        BlockTarget::DocStart
                    }
                    GalleryTarget::Caret => BlockTarget::Caret,
                };
                if self.hf.is_some() || self.note.is_some() || self.textbox.is_some() {
                    return Err("bodyOnly".into());
                }
                let s = self.session.as_mut().ok_or("noDocument")?;
                s.insert_building_block(&from, &part.blocks, part.props.behavior(), target)
                    .map_err(key_err)?;
                out.inserted = Some(part.props.name);
                Ok(())
            }
            BlockCommand::ExpandAutoText => {
                if self.hf.is_some() || self.note.is_some() || self.textbox.is_some() {
                    return Err("bodyOnly".into());
                }
                let list = self.building_blocks();
                let names: Vec<&str> = list.iter().map(|b| b.name.as_str()).collect();
                let s = self.session.as_ref().ok_or("noDocument")?;
                let (i, start) = s.autotext_match(&names).ok_or("noAutoText")?;
                let (from, part) = self.block(&list[i].key).ok_or("noBlock")?;
                let s = self.session.as_mut().ok_or("noDocument")?;
                s.expand_autotext(start, &from, &part.blocks, part.props.behavior())
                    .map_err(key_err)?;
                out.inserted = Some(part.props.name);
                Ok(())
            }
            BlockCommand::BuildingBlockPreview { key } => {
                out.preview = Some(self.block_preview(&key).ok_or("noBlock")?);
                Ok(())
            }
        }
    }

    /// Apply `f` to the glossary holding block `key` (document: one undo step;
    /// user store: the changed file is returned in `out.store`).
    fn change_block(
        &mut self,
        key: &str,
        out: &mut BlocksOut,
        f: impl FnOnce(&mut Glossary, usize) -> std::result::Result<(), String>,
    ) -> std::result::Result<(), String> {
        let (store, i) = parse_key(key).ok_or("noBlock")?;
        let doc = match store {
            BlockStore::Document => self.document(),
            BlockStore::User => self.blocks.user.as_ref(),
            BlockStore::Builtin => return Err("builtinBlock".into()),
        };
        let mut g = doc
            .and_then(|d| d.glossary.as_deref())
            .filter(|g| i < g.parts.len())
            .cloned()
            .ok_or("noBlock")?;
        f(&mut g, i)?;
        let g = (!g.parts.is_empty()).then(|| Arc::new(g));
        if store == BlockStore::Document {
            let s = self.session.as_mut().ok_or("noDocument")?;
            s.set_glossary(g).map_err(|e| e.to_string())
        } else {
            let user = self.blocks.user.as_mut().ok_or("noBlock")?;
            user.glossary = g;
            out.store = Some(store_file(user)?);
            Ok(())
        }
    }

    /// Page of the caret in the main text.
    fn caret_page(&self) -> Option<usize> {
        let sel = self.session.as_ref()?.selection();
        crate::nav::caret_rect(self.layout.document(), sel.focus).map(|c| c.page)
    }

    /// PNG (base64) of block `key` laid out by the engine: flow content on an A4
    /// text column cropped to its height, a cover page as the whole page.
    fn block_preview(&mut self, key: &str) -> Option<String> {
        let (from, part) = self.block(key)?;
        let fp = bo_model::fingerprint(&(
            part.content_fingerprint(),
            &from.styles,
            &from.default_run,
            &from.default_para,
        ));
        if let Some(p) = self.blocks.previews.get(&fp) {
            return Some(p.clone());
        }
        let whole_page = part.props.behavior() == "pg";
        let margin = if whole_page { 1_418 } else { 170 };
        let page = PageSetup {
            width: Twips(11_906),
            height: Twips(16_838),
            margin_top: Twips(margin),
            margin_bottom: Twips(margin),
            margin_left: Twips(1_418),
            margin_right: Twips(1_418),
            header: Twips(0),
            footer: Twips(0),
        };
        let mut blocks = detached(&part.blocks);
        // Tallest floating object of the block (text boxes are not text lines).
        let mut floating = 0.0_f32;
        bo_edit::blocks::for_each_paragraph_mut(&mut blocks, &mut |p| {
            for r in &p.runs {
                for c in &r.content {
                    if let bo_model::RunContent::Image(img) = c {
                        floating = floating.max(img.height.0 as f32 / 12_700.0);
                    }
                }
            }
        });
        if !matches!(blocks.last(), Some(Block::Paragraph(_))) {
            blocks.push(Block::Paragraph(Paragraph::default()));
        }
        let doc = Document {
            sections: vec![Section {
                page,
                ..Section::new(blocks)
            }],
            ..from
        };
        let layout = bo_layout::layout_document_with(
            &doc,
            &mut self.fonts,
            &mut self.cache,
            bo_layout::LayoutOptions::default(),
        )
        .ok()?;
        let first = layout.pages.first()?;
        let mut list = bo_render::build_page_list(first, &self.fonts);
        let scale = if whole_page {
            PAGE_PREVIEW_SCALE
        } else {
            let top = page.margin_top.to_pt();
            let text = first
                .lines
                .iter()
                .map(|l| l.top + l.height)
                .fold(top, f32::max);
            let bottom = text.max(top + floating + 8.0) + top;
            list.height = bottom.clamp(24.0, list.height);
            PREVIEW_SCALE
        };
        let images = bo_render::image::Chain(&doc.media, &self.images);
        let png = bo_raster::render_page_png_with(&list, &self.fonts, scale, &images).ok()?;
        let b64 = crate::ole::encode(&png);
        if self.blocks.previews.len() > 256 {
            self.blocks.previews.clear();
        }
        self.blocks.previews.insert(fp, b64.clone());
        Some(b64)
    }
}
