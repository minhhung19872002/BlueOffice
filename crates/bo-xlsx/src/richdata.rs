//! Pictures placed in cells: rich values of type `_localImage` ([MS-XLSX]
//! §2.3.6.1.10).
//!
//! A cell's `vm` (ECMA-376 Part 1, §18.3.1.4) is a 1-based index into
//! `metadata/valueMetadata` (§18.9.17); its `rc` (§18.9.13) names a metadata
//! type (`t`, 1-based into `metadataTypes`, §18.9.11) and a 0-based entry `v` of
//! the `futureMetadata` block of that name (§18.9.4). For `XLRICHVALUE` that
//! entry carries `extLst/ext {3E2802C4-…}/xlrd:rvb@i` ([MS-XLSX] §2.2.4.4,
//! §2.6.176): the index of a rich value (`rv`, §2.6.175) in the Rich Value Data
//! part (§2.1.10), whose keys come from its structure (`s`, §2.6.180) in the
//! Rich Value Structure part (§2.1.11). `_rvRel:LocalImageIdentifier` indexes a
//! `rel` of the Rich Value Rels part (§2.1.21, §2.6.241) whose relationship
//! targets the picture.
//!
//! Reading attaches the picture to the cell ([`bo_model::sheet::Cell::image`]);
//! the parts themselves stay verbatim. Writing keeps every existing rich value
//! (of any type) and its `vm`; pictures placed since the file was read are
//! appended to the parts (created when missing) and get new `vm` indices.

use crate::write::{relative_target, Names, RelIds};
use crate::xmlpatch::{children, index, El, Patch};
use bo_model::sheet::{CellImage, CellImageSource, OpaquePart, OpaqueRel, Workbook};
use bo_opc::xml::escape;
use bo_opc::{resolve_target, Relationship, TargetMode};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Metadata part relationship (ECMA-376 Part 1, §12.3.10).
pub const REL_METADATA: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata";
/// Metadata part content type (§12.3.10).
pub const CT_METADATA: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml";
/// Rich Value Data part ([MS-XLSX] §2.1.10).
pub const REL_RV: &str = "http://schemas.microsoft.com/office/2017/06/relationships/rdRichValue";
/// Rich Value Data content type ([MS-XLSX] §2.1.10).
pub const CT_RV: &str = "application/vnd.ms-excel.rdrichvalue+xml";
/// Rich Value Structure part ([MS-XLSX] §2.1.11).
pub const REL_RVS: &str =
    "http://schemas.microsoft.com/office/2017/06/relationships/rdRichValueStructure";
/// Rich Value Structure content type ([MS-XLSX] §2.1.11).
pub const CT_RVS: &str = "application/vnd.ms-excel.rdrichvaluestructure+xml";
/// Rich Value Rels part ([MS-XLSX] §2.1.21).
pub const REL_RVREL: &str =
    "http://schemas.microsoft.com/office/2022/10/relationships/richValueRel";
/// Rich Value Rels content type ([MS-XLSX] §2.1.21).
pub const CT_RVREL: &str = "application/vnd.ms-excel.richvaluerel+xml";
/// Picture relationship (ECMA-376 Part 1, §15.2.14).
pub const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
/// Namespace of `rvData`, `rvStructures` and `rvb` ([MS-XLSX] §2.4.76–78).
pub const NS_RD: &str = "http://schemas.microsoft.com/office/spreadsheetml/2017/richdata";
/// Namespace of `richValueRels` ([MS-XLSX] §2.4.100).
pub const NS_RVREL: &str = "http://schemas.microsoft.com/office/spreadsheetml/2022/richvaluerel";
/// Ext URI of `rvb` in `futureMetadata/bk/extLst` ([MS-XLSX] §2.2.4.4).
pub const EXT_RVB: &str = "{3E2802C4-A4D2-4D8B-9148-E3BE6C30E623}";
/// Metadata type name of rich values ([MS-XLSX] §2.2.4.4).
pub const XLRICHVALUE: &str = "XLRICHVALUE";
/// Key of the picture relationship index ([MS-XLSX] §2.3.6.1.10).
const KEY_IMAGE: &str = "_rvRel:LocalImageIdentifier";

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const SML_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// The rich-data parts of a workbook (absolute names).
#[derive(Debug, Default, Clone)]
struct Parts {
    metadata: Option<String>,
    rv: Option<String>,
    rvs: Option<String>,
    rvrel: Option<String>,
}

fn find_parts(rels: &[OpaqueRel], parts: &[OpaquePart]) -> Parts {
    let by = |suffix: &str, rels: &[OpaqueRel]| {
        rels.iter()
            .find(|r| !r.external && r.rel_type.rsplit('/').next() == Some(suffix))
            .map(|r| r.target.clone())
    };
    let mut p = Parts {
        metadata: by("sheetMetadata", rels),
        rv: by("rdRichValue", rels),
        rvs: by("rdRichValueStructure", rels),
        rvrel: by("richValueRel", rels),
    };
    // The rich value parts may also hang off the metadata part ([MS-XLSX] §2.1.10).
    if let Some(meta) = &p.metadata {
        let extra = part_rels(parts, meta);
        p.rv = p.rv.or_else(|| by("rdRichValue", &extra));
        p.rvs = p.rvs.or_else(|| by("rdRichValueStructure", &extra));
        p.rvrel = p.rvrel.or_else(|| by("richValueRel", &extra));
    }
    p
}

/// Relationships of an opaque part (from its preserved `.rels` part).
fn part_rels(parts: &[OpaquePart], source: &str) -> Vec<OpaqueRel> {
    let name = bo_opc::rels_part_name(source);
    let Some(data) = parts.iter().find(|p| p.name == name).map(|p| &p.data) else {
        return Vec::new();
    };
    let Some(els) = index(data, &name) else {
        return Vec::new();
    };
    els.iter()
        .filter(|e| e.local == "Relationship")
        .filter_map(|e| {
            let external = e.attr("TargetMode").as_deref() == Some("External");
            let target = e.attr("Target")?;
            Some(OpaqueRel {
                id: e.attr("Id")?,
                rel_type: e.attr("Type").unwrap_or_default(),
                target: if external {
                    target
                } else {
                    resolve_target(source, &target)
                },
                external,
            })
        })
        .collect()
}

fn data_of<'a>(parts: &'a [OpaquePart], name: Option<&String>) -> Option<&'a [u8]> {
    let name = name?;
    parts
        .iter()
        .find(|p| &p.name == name)
        .map(|p| p.data.as_slice())
}

/// `vm` (1-based) → rich value index, for every `valueMetadata` block.
fn value_metadata(els: &[El]) -> Vec<Option<u32>> {
    let types: Vec<String> = els
        .iter()
        .filter(|e| e.local == "metadataType")
        .map(|e| e.attr("name").unwrap_or_default())
        .collect();
    // futureMetadata name → rvb index per bk.
    let mut future: HashMap<String, Vec<Option<u32>>> = HashMap::new();
    for (i, fm) in els
        .iter()
        .enumerate()
        .filter(|(_, e)| e.local == "futureMetadata")
    {
        let list = children(els, i)
            .filter(|(_, e)| e.local == "bk")
            .map(|(bk, _)| rvb_in(els, bk))
            .collect();
        future.insert(fm.attr("name").unwrap_or_default(), list);
    }
    let Some(vmi) = els.iter().position(|e| e.local == "valueMetadata") else {
        return Vec::new();
    };
    children(els, vmi)
        .filter(|(_, e)| e.local == "bk")
        .map(|(bk, _)| {
            let (_, rc) = children(els, bk).find(|(_, e)| e.local == "rc")?;
            let t: usize = rc.attr("t")?.parse().ok()?;
            let v: usize = rc.attr("v")?.parse().ok()?;
            let name = types.get(t.checked_sub(1)?)?;
            if name != XLRICHVALUE {
                return None;
            }
            *future.get(name)?.get(v)?
        })
        .collect()
}

/// `xlrd:rvb/@i` below element `bk`.
fn rvb_in(els: &[El], bk: usize) -> Option<u32> {
    let mut stack = vec![bk];
    while let Some(i) = stack.pop() {
        for (c, e) in children(els, i) {
            if e.local == "rvb" {
                return e.attr("i")?.parse().ok();
            }
            stack.push(c);
        }
    }
    None
}

/// A rich value: structure index and values.
struct Rv {
    s: usize,
    values: Vec<String>,
}

fn rich_values(els: &[El]) -> Vec<Rv> {
    children(els, 0)
        .filter(|(_, e)| e.local == "rv")
        .map(|(i, e)| Rv {
            s: e.attr("s")
                .and_then(|v| v.parse().ok())
                .unwrap_or(usize::MAX),
            values: children(els, i)
                .filter(|(_, v)| v.local == "v")
                .map(|(_, v)| v.text.clone())
                .collect(),
        })
        .collect()
}

/// Structures: type name and key names.
fn structures(els: &[El]) -> Vec<(String, Vec<String>)> {
    children(els, 0)
        .filter(|(_, e)| e.local == "s")
        .map(|(i, e)| {
            (
                e.attr("t").unwrap_or_default(),
                children(els, i)
                    .filter(|(_, k)| k.local == "k")
                    .map(|(_, k)| k.attr("n").unwrap_or_default())
                    .collect(),
            )
        })
        .collect()
}

/// Relationship ids listed by the Rich Value Rels part, in order.
fn rel_ids(els: &[El]) -> Vec<String> {
    children(els, 0)
        .filter(|(_, e)| e.local == "rel")
        .map(|(_, e)| e.attr("id").unwrap_or_default())
        .collect()
}

/// After reading: attach `_localImage` pictures to the cells that point at them.
pub(crate) fn resolve(wb: &mut Workbook) {
    let parts = find_parts(&wb.opaque.workbook_rels, &wb.opaque.parts);
    let op = &wb.opaque.parts;
    let (Some(meta), Some(rv), Some(rvs)) = (
        data_of(op, parts.metadata.as_ref()),
        data_of(op, parts.rv.as_ref()),
        data_of(op, parts.rvs.as_ref()),
    ) else {
        return;
    };
    let (Some(meta), Some(rv), Some(rvs)) = (
        index(meta, "/metadata"),
        index(rv, "/rdrichvalue"),
        index(rvs, "/rdrichvaluestructure"),
    ) else {
        return;
    };
    let vms = value_metadata(&meta);
    let values = rich_values(&rv);
    let structs = structures(&rvs);
    // Rich value rel index → picture bytes and extension.
    let pictures: Vec<Option<(Vec<u8>, String)>> = match &parts.rvrel {
        Some(name) => {
            let rels = part_rels(op, name);
            let ids = data_of(op, Some(name))
                .and_then(|d| index(d, name))
                .map(|e| rel_ids(&e))
                .unwrap_or_default();
            ids.iter()
                .map(|id| {
                    let r = rels.iter().find(|r| &r.id == id && !r.external)?;
                    let data = op.iter().find(|p| p.name == r.target)?.data.clone();
                    let ext = r
                        .target
                        .rsplit_once('.')
                        .map_or("png", |(_, e)| e)
                        .to_ascii_lowercase();
                    Some((data, ext))
                })
                .collect()
        }
        None => Vec::new(),
    };
    let image_of = |vm: u32| -> Option<CellImage> {
        let rvi = (*vms.get((vm as usize).checked_sub(1)?)?)? as usize;
        let rv = values.get(rvi)?;
        let (t, keys) = structs.get(rv.s)?;
        if t != "_localImage" {
            return None;
        }
        let val = |k: &str| {
            keys.iter()
                .position(|n| n.eq_ignore_ascii_case(k))
                .and_then(|i| rv.values.get(i))
        };
        let rel: usize = val(KEY_IMAGE)?.trim().parse().ok()?;
        let (data, ext) = pictures.get(rel)?.clone()?;
        let mut img = CellImage::local(data, &ext, val("Text").cloned());
        if let Some(o) = val("CalcOrigin").and_then(|v| v.trim().parse().ok()) {
            img.calc_origin = o;
        }
        Some(img)
    };
    let mut cache: HashMap<u32, Option<CellImage>> = HashMap::new();
    for ws in &mut wb.sheets {
        for cell in ws.cells.values_mut() {
            if let Some(vm) = cell.vm {
                let img = cache.entry(vm).or_insert_with(|| image_of(vm));
                if let Some(img) = img {
                    cell.image = Some(Box::new(img.clone()));
                }
            }
        }
    }
}

/// Everything [`write_new`] needs from the writer.
pub(crate) struct WriteCtx<'a> {
    pub wb_path: &'a str,
    pub names: &'a mut Names,
    pub wb_ids: &'a mut RelIds,
    pub wb_rels: &'a mut Vec<Relationship>,
    pub out_parts: &'a mut Vec<(String, Option<String>, Vec<u8>)>,
    pub overrides: &'a mut HashMap<String, Vec<u8>>,
}

/// Picture identity for de-duplication: media key, alt text, origin.
type ImageKey = (String, Option<String>, u32);

/// `vm` of every cell holding a picture placed since reading, keyed by
/// (sheet, row, col). Appends the pictures to the rich-data parts.
pub(crate) fn write_new(wb: &Workbook, ctx: &mut WriteCtx<'_>) -> HashMap<(usize, u32, u32), u32> {
    let mut pending: Vec<(ImageKey, &CellImage)> = Vec::new();
    let mut cells: Vec<((usize, u32, u32), usize)> = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        for (&(r, c), cell) in &ws.cells {
            let Some(img) = cell.image.as_deref() else {
                continue;
            };
            if cell.vm.is_some() || !matches!(img.source, CellImageSource::Local { .. }) {
                continue;
            }
            let key = (img.media_id(), img.alt_text.clone(), img.calc_origin);
            let i = if let Some(i) = pending.iter().position(|(k, _)| *k == key) {
                i
            } else {
                pending.push((key, img));
                pending.len() - 1
            };
            cells.push(((si, r, c), i));
        }
    }
    if pending.is_empty() {
        return HashMap::new();
    }
    let parts = find_parts(&wb.opaque.workbook_rels, &wb.opaque.parts);
    let current = |name: &Option<String>, ctx: &WriteCtx<'_>| -> Option<(String, Vec<u8>)> {
        let n = name.as_ref()?;
        let data = ctx
            .overrides
            .get(n)
            .cloned()
            .or_else(|| data_of(&wb.opaque.parts, Some(n)).map(<[u8]>::to_vec))?;
        Some((n.clone(), data))
    };

    // 1. Pictures → media parts and Rich Value Rels entries.
    let rvrel = current(&parts.rvrel, ctx);
    let rvrel_name = rvrel.as_ref().map_or_else(
        || {
            ctx.names
                .fresh(|n| numbered("/xl/richData/richValueRel", n, "xml"))
        },
        |(n, _)| n.clone(),
    );
    let rels_name = bo_opc::rels_part_name(&rvrel_name);
    let old_rels = data_of(&wb.opaque.parts, Some(&rels_name)).map(<[u8]>::to_vec);
    let mut used_ids: Vec<String> = old_rels
        .as_deref()
        .and_then(|d| index(d, &rels_name))
        .map(|els| els.iter().filter_map(|e| e.attr("Id")).collect())
        .unwrap_or_default();
    let first_rel = rvrel
        .as_ref()
        .and_then(|(n, d)| index(d, n))
        .map_or(0, |e| rel_ids(&e).len());
    let mut new_rel_xml = String::new();
    let mut new_rels = String::new();
    let mut media_seen: HashMap<String, usize> = HashMap::new();
    let mut rel_of: Vec<usize> = Vec::new();
    for (_, img) in &pending {
        let CellImageSource::Local { data, ext, key } = &img.source else {
            continue;
        };
        if let Some(&i) = media_seen.get(key) {
            rel_of.push(i);
            continue;
        }
        let ext = if ext.is_empty() { "png" } else { ext.as_str() };
        let media = ctx.names.fresh(|n| format!("/xl/media/image{n}.{ext}"));
        ctx.out_parts
            .push((media.clone(), Some(media_type(ext).into()), data.clone()));
        let mut n = used_ids.len() + 1;
        let id = loop {
            let id = format!("rId{n}");
            if !used_ids.contains(&id) {
                break id;
            }
            n += 1;
        };
        used_ids.push(id.clone());
        // [MS-XLSX] §2.6.241 CT_RichValueRelRelationship.
        new_rel_xml.push_str(&format!("<rel r:id=\"{id}\"/>"));
        new_rels.push_str(&format!(
            "<Relationship Id=\"{id}\" Type=\"{REL_IMAGE}\" Target=\"{}\"/>",
            escape(&relative_target(&rvrel_name, &media))
        ));
        let i = first_rel + media_seen.len();
        media_seen.insert(key.clone(), i);
        rel_of.push(i);
    }
    if let Some((name, data)) = &rvrel {
        if let Some(out) = append_root(data, name, |root| {
            // The `r` prefix may not be declared on an existing root.
            let needs_r = !root.attrs.iter().any(|(q, _)| q == "xmlns:r");
            let xml = if needs_r {
                new_rel_xml.replace("<rel ", &format!("<{} xmlns:r=\"{R_NS}\" ", root.q("rel")))
            } else {
                new_rel_xml.replace("<rel ", &format!("<{} ", root.q("rel")))
            };
            (xml, None)
        }) {
            ctx.overrides.insert(name.clone(), out);
        }
    } else {
        ctx.out_parts.push((
            rvrel_name.clone(),
            Some(CT_RVREL.into()),
            format!(
                "{XML_DECL}<richValueRels xmlns=\"{NS_RVREL}\" xmlns:r=\"{R_NS}\">{new_rel_xml}</richValueRels>"
            )
            .into_bytes(),
        ));
        add_wb_rel(ctx, REL_RVREL, &rvrel_name);
    }
    match old_rels {
        Some(data) => {
            if let Some(out) = append_root(&data, &rels_name, |_| (new_rels.clone(), None)) {
                ctx.overrides.insert(rels_name.clone(), out);
            }
        }
        None => ctx.out_parts.push((
            rels_name,
            None,
            format!("{XML_DECL}<Relationships xmlns=\"{PKG_REL_NS}\">{new_rels}</Relationships>")
                .into_bytes(),
        )),
    }

    // 2. Structures: `_localImage` with and without alt text ([MS-XLSX] §2.3.6.1.10).
    let rvs = current(&parts.rvs, ctx);
    let mut structs = rvs
        .as_ref()
        .and_then(|(n, d)| index(d, n))
        .map(|e| structures(&e))
        .unwrap_or_default();
    let old_struct_count = structs.len();
    let mut struct_of = |alt: bool| -> usize {
        let mut keys = vec![KEY_IMAGE.to_owned(), "CalcOrigin".to_owned()];
        if alt {
            keys.push("Text".to_owned());
        }
        if let Some(i) = structs
            .iter()
            .position(|(t, k)| t == "_localImage" && *k == keys)
        {
            return i;
        }
        structs.push(("_localImage".into(), keys));
        structs.len() - 1
    };
    let struct_ids: Vec<usize> = pending
        .iter()
        .map(|((_, alt, _), _)| struct_of(alt.is_some()))
        .collect();
    let mut new_s = String::new();
    for (t, keys) in &structs[old_struct_count..] {
        // §2.6.180 CT_RichValueStructure / §2.6.163 CT_Key.
        new_s.push_str(&format!("<s t=\"{}\">", escape(t)));
        for k in keys {
            let ty = if k == "Text" { "s" } else { "i" };
            new_s.push_str(&format!("<k n=\"{}\" t=\"{ty}\"/>", escape(k)));
        }
        new_s.push_str("</s>");
    }
    let total_structs = structs.len();
    write_container(
        ctx,
        rvs,
        "rdrichvaluestructure",
        "rvStructures",
        CT_RVS,
        REL_RVS,
        &new_s,
        total_structs,
    );

    // 3. Rich values (§2.6.175 CT_RichValue: one `v` per key, in key order).
    let rv = current(&parts.rv, ctx);
    let first_rv = rv
        .as_ref()
        .and_then(|(n, d)| index(d, n))
        .map_or(0, |e| rich_values(&e).len());
    let mut new_rv = String::new();
    for (i, ((_, alt, origin), _)) in pending.iter().enumerate() {
        new_rv.push_str(&format!(
            "<rv s=\"{}\"><v>{}</v><v>{origin}</v>",
            struct_ids[i], rel_of[i]
        ));
        if let Some(a) = alt {
            new_rv.push_str(&format!("<v>{}</v>", escape(a)));
        }
        new_rv.push_str("</rv>");
    }
    let total_rv = first_rv + pending.len();
    write_container(
        ctx,
        rv,
        "rdrichvalue",
        "rvData",
        CT_RV,
        REL_RV,
        &new_rv,
        total_rv,
    );

    // 4. Metadata: one `futureMetadata` block and one `valueMetadata` block per rich value.
    let meta = current(&parts.metadata, ctx);
    let rvbs: Vec<u32> = (first_rv..total_rv).map(|i| i as u32).collect();
    let first_vm = if let Some((name, data)) = meta {
        let (out, first) = patch_metadata(&data, &name, &rvbs).unwrap_or((data, 1));
        ctx.overrides.insert(name, out);
        first
    } else {
        let name = ctx.names.fresh(|n| numbered("/xl/metadata", n, "xml"));
        ctx.out_parts.push((
            name.clone(),
            Some(CT_METADATA.into()),
            new_metadata(&rvbs).into_bytes(),
        ));
        add_wb_rel(ctx, REL_METADATA, &name);
        1
    };
    cells
        .into_iter()
        .map(|(at, i)| (at, first_vm + i as u32))
        .collect()
}

fn numbered(stem: &str, n: u32, ext: &str) -> String {
    if n == 1 {
        format!("{stem}.{ext}")
    } else {
        format!("{stem}{n}.{ext}")
    }
}

/// Media type of a picture extension (IANA registrations).
fn media_type(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    }
}

fn add_wb_rel(ctx: &mut WriteCtx<'_>, rel_type: &str, part: &str) {
    let id = ctx.wb_ids.alloc();
    ctx.wb_rels.push(Relationship {
        id,
        rel_type: rel_type.into(),
        target: relative_target(ctx.wb_path, part),
        mode: TargetMode::Internal,
    });
}

/// Appends children to the root of `data` (before a trailing `extLst`); `f`
/// returns the XML to add and an optional new `count`.
fn append_root(
    data: &[u8],
    part: &str,
    f: impl FnOnce(&El) -> (String, Option<usize>),
) -> Option<Vec<u8>> {
    let els = index(data, part)?;
    let root = els.first()?;
    let (xml, count) = f(root);
    let mut p = Patch::default();
    match children(&els, 0).find(|(_, e)| e.local == "extLst") {
        Some((_, ext)) => p.insert_before(ext, &xml),
        None => p.append(0, &xml),
    }
    if let Some(c) = count {
        p.set_attr(0, "count", &c.to_string());
    }
    Some(p.apply(data, &els))
}

/// Writes `rvStructures` / `rvData`: appended to the existing part, or new.
#[allow(clippy::too_many_arguments)]
fn write_container(
    ctx: &mut WriteCtx<'_>,
    existing: Option<(String, Vec<u8>)>,
    stem: &str,
    root: &str,
    ct: &str,
    rel: &str,
    xml: &str,
    count: usize,
) {
    if let Some((name, data)) = existing {
        if xml.is_empty() {
            return;
        }
        let out = append_root(&data, &name, |r| {
            let prefix = r.prefix();
            let xml = if prefix.is_empty() {
                xml.to_owned()
            } else {
                prefixed(xml, prefix)
            };
            (xml, Some(count))
        });
        if let Some(out) = out {
            ctx.overrides.insert(name, out);
        }
    } else {
        let name = ctx
            .names
            .fresh(|n| numbered(&format!("/xl/richData/{stem}"), n, "xml"));
        ctx.out_parts.push((
            name.clone(),
            Some(ct.into()),
            format!("{XML_DECL}<{root} xmlns=\"{NS_RD}\" count=\"{count}\">{xml}</{root}>")
                .into_bytes(),
        ));
        add_wb_rel(ctx, rel, &name);
    }
}

/// Qualifies the unprefixed element names of `xml` (our own output only).
fn prefixed(xml: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(xml.len() + 32);
    let mut chars = xml.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        out.push(ch);
        if ch == '<' {
            if let Some(&(_, '/')) = chars.peek() {
                out.push('/');
                chars.next();
            }
            out.push_str(prefix);
            out.push(':');
        }
    }
    out
}

/// A new metadata part (ECMA-376 Part 1, §18.9.12 `metadata`) for `rvbs`.
fn new_metadata(rvbs: &[u32]) -> String {
    let mut s = format!(
        "{XML_DECL}<metadata xmlns=\"{SML_NS}\" xmlns:xlrd=\"{NS_RD}\">\
         <metadataTypes count=\"1\">{}</metadataTypes>\
         <futureMetadata name=\"{XLRICHVALUE}\" count=\"{}\">",
        metadata_type(),
        rvbs.len()
    );
    for i in rvbs {
        s.push_str(&future_bk("", *i));
    }
    s.push_str(&format!(
        "</futureMetadata><valueMetadata count=\"{}\">",
        rvbs.len()
    ));
    for (v, _) in rvbs.iter().enumerate() {
        s.push_str(&format!("<bk><rc t=\"1\" v=\"{v}\"/></bk>"));
    }
    s.push_str("</valueMetadata></metadata>");
    s
}

/// §18.9.10 `metadataType` for rich values. Which flags are set is our own choice
/// (the value follows copies, pastes, merges and row/column shifts like a value).
fn metadata_type() -> String {
    format!(
        "<metadataType name=\"{XLRICHVALUE}\" minSupportedVersion=\"120000\" copy=\"1\" \
         pasteAll=\"1\" pasteValues=\"1\" merge=\"1\" splitFirst=\"1\" rowColShift=\"1\" \
         clearFormats=\"1\" clearComments=\"1\" assign=\"1\" coerce=\"1\"/>"
    )
}

/// `futureMetadata/bk` pointing at rich value `i` ([MS-XLSX] §2.2.4.4 rvb).
fn future_bk(prefix: &str, i: u32) -> String {
    let q = |l: &str| {
        if prefix.is_empty() {
            l.to_owned()
        } else {
            format!("{prefix}:{l}")
        }
    };
    format!(
        "<{bk}><{ext_lst}><{ext} uri=\"{EXT_RVB}\" xmlns:xlrd=\"{NS_RD}\"><xlrd:rvb i=\"{i}\"/></{ext}></{ext_lst}></{bk}>",
        bk = q("bk"),
        ext_lst = q("extLst"),
        ext = q("ext"),
    )
}

/// Appends `rvbs` to an existing metadata part; returns it and the first new `vm`.
fn patch_metadata(data: &[u8], part: &str, rvbs: &[u32]) -> Option<(Vec<u8>, u32)> {
    let els = index(data, part)?;
    let root = els.first()?;
    let q = |l: &str| root.q(l);
    let mut p = Patch::default();
    // CT_Metadata sequence (§18.9.12): metadataTypes, metadataStrings, mdxMetadata,
    // futureMetadata*, cellMetadata, valueMetadata, extLst.
    let kids: Vec<(usize, &El)> = children(&els, 0).collect();
    let find = |l: &str| kids.iter().find(|(_, e)| e.local == l).copied();
    // Metadata type (1-based index t).
    let types: Vec<(usize, &El)> = match find("metadataTypes") {
        Some((i, _)) => children(&els, i)
            .filter(|(_, e)| e.local == "metadataType")
            .collect(),
        None => Vec::new(),
    };
    let t = if let Some(i) = types
        .iter()
        .position(|(_, e)| e.attr("name").as_deref() == Some(XLRICHVALUE))
    {
        i + 1
    } else {
        let ty = if root.prefix().is_empty() {
            metadata_type()
        } else {
            prefixed(&metadata_type(), root.prefix())
        };
        if let Some((i, _)) = find("metadataTypes") {
            p.append(i, &ty);
            p.set_attr(i, "count", &(types.len() + 1).to_string());
        } else {
            // First child of the root.
            let open = format!("<{0} count=\"1\">{ty}</{0}>", q("metadataTypes"));
            match kids.first() {
                Some((_, first)) => p.insert_before(first, &open),
                None => p.append(0, &open),
            }
        }
        types.len() + 1
    };
    // futureMetadata XLRICHVALUE.
    let future = kids
        .iter()
        .find(|(_, e)| {
            e.local == "futureMetadata" && e.attr("name").as_deref() == Some(XLRICHVALUE)
        })
        .copied();
    let bks: String = rvbs.iter().map(|i| future_bk(root.prefix(), *i)).collect();
    let first_v = if let Some((i, _)) = future {
        let n = children(&els, i).filter(|(_, e)| e.local == "bk").count();
        p.append(i, &bks);
        p.set_attr(i, "count", &(n + rvbs.len()).to_string());
        n
    } else {
        let block = format!(
            "<{0} name=\"{XLRICHVALUE}\" count=\"{1}\">{bks}</{0}>",
            q("futureMetadata"),
            rvbs.len()
        );
        // After the last futureMetadata / mdxMetadata / metadataStrings / metadataTypes.
        let before = kids
            .iter()
            .find(|(_, e)| {
                matches!(
                    e.local.as_str(),
                    "cellMetadata" | "valueMetadata" | "extLst"
                )
            })
            .copied();
        match before {
            Some((_, el)) => p.insert_before(el, &block),
            None => p.append(0, &block),
        }
        0
    };
    let mut rcs = String::new();
    for k in 0..rvbs.len() {
        let _ = write!(
            rcs,
            "<{0}><{1} t=\"{t}\" v=\"{2}\"/></{0}>",
            q("bk"),
            q("rc"),
            first_v + k
        );
    }
    let first_vm = if let Some((i, _)) = find("valueMetadata") {
        let n = children(&els, i).filter(|(_, e)| e.local == "bk").count();
        p.append(i, &rcs);
        p.set_attr(i, "count", &(n + rvbs.len()).to_string());
        n + 1
    } else {
        let block = format!(
            "<{0} count=\"{1}\">{rcs}</{0}>",
            q("valueMetadata"),
            rvbs.len()
        );
        match find("extLst") {
            Some((_, el)) => p.insert_before(el, &block),
            None => p.append(0, &block),
        }
        1
    };
    Some((p.apply(data, &els), u32::try_from(first_vm).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_metadata_maps_vm_to_rich_values() {
        let xml = new_metadata(&[4, 5]);
        let els = index(xml.as_bytes(), "/m.xml").unwrap();
        assert_eq!(value_metadata(&els), vec![Some(4), Some(5)]);
    }

    #[test]
    fn patching_keeps_dynamic_array_metadata() {
        // A part with only the dynamic array type (cellMetadata, [MS-XLSX] XLDAPR).
        let src = format!(
            "<metadata xmlns=\"{SML_NS}\" xmlns:xda=\"x\"><metadataTypes count=\"1\">\
             <metadataType name=\"XLDAPR\" minSupportedVersion=\"120000\"/></metadataTypes>\
             <futureMetadata name=\"XLDAPR\" count=\"1\"><bk/></futureMetadata>\
             <cellMetadata count=\"1\"><bk><rc t=\"1\" v=\"0\"/></bk></cellMetadata></metadata>"
        );
        let (out, first) = patch_metadata(src.as_bytes(), "/m.xml", &[0, 1]).unwrap();
        assert_eq!(first, 1);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("<metadataTypes count=\"2\">"), "{s}");
        assert!(
            s.contains("<cellMetadata count=\"1\"><bk><rc t=\"1\" v=\"0\"/></bk></cellMetadata>")
        );
        let els = index(s.as_bytes(), "/m.xml").unwrap();
        assert_eq!(value_metadata(&els), vec![Some(0), Some(1)]);
        // futureMetadata XLRICHVALUE comes before cellMetadata (§18.9.12 sequence).
        assert!(s.find("XLRICHVALUE\" count").unwrap() < s.find("<cellMetadata").unwrap());
    }

    #[test]
    fn prefixes_elements() {
        assert_eq!(
            prefixed("<s t=\"a\"><k n=\"b\"/></s>", "x"),
            "<x:s t=\"a\"><x:k n=\"b\"/></x:s>"
        );
    }
}
