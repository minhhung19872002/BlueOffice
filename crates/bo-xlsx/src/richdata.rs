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
use bo_model::sheet::{CellImage, CellImageSource, ImageSizing, OpaquePart, OpaqueRel, Workbook};
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
/// Key of the web picture index ([MS-XLSX] §2.3.6.1.11 Web Image Type): the
/// 0-based `webImageSrd` entry of the Rich Value Web Image part.
const KEY_WEB_IMAGE: &str = "WebImageIdentifier";
/// Rich Value Web Image part relationship ([MS-XLSX] rich data web image part).
pub const REL_WEBIMAGE: &str =
    "http://schemas.microsoft.com/office/2020/07/relationships/rdrichvaluewebimage";
/// Rich Value Web Image part content type.
pub const CT_WEBIMAGE: &str = "application/vnd.ms-excel.rdrichvaluewebimage+xml";
/// Namespace of `webImagesSrd` ([MS-XLSX] CT_WebImagesSupportingRichData).
pub const NS_WEBIMAGE: &str =
    "http://schemas.microsoft.com/office/spreadsheetml/2020/richdatawebimage";
/// Hyperlink relationship (ECMA-376 Part 1, §15.3): the `address` of a web picture.
pub const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

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
    webimage: Option<String>,
}

fn find_parts(rels: &[OpaqueRel], parts: &[OpaquePart]) -> Parts {
    let by = |suffix: &str, rels: &[OpaqueRel]| {
        rels.iter()
            .find(|r| {
                !r.external
                    && r.rel_type
                        .rsplit('/')
                        .next()
                        .is_some_and(|t| t.eq_ignore_ascii_case(suffix))
            })
            .map(|r| r.target.clone())
    };
    let mut p = Parts {
        metadata: by("sheetMetadata", rels),
        rv: by("rdRichValue", rels),
        rvs: by("rdRichValueStructure", rels),
        rvrel: by("richValueRel", rels),
        webimage: by("rdRichValueWebImage", rels),
    };
    // The rich value parts may also hang off the metadata part ([MS-XLSX] §2.1.10).
    if let Some(meta) = &p.metadata {
        let extra = part_rels(parts, meta);
        p.rv = p.rv.or_else(|| by("rdRichValue", &extra));
        p.rvs = p.rvs.or_else(|| by("rdRichValueStructure", &extra));
        p.rvrel = p.rvrel.or_else(|| by("richValueRel", &extra));
        p.webimage = p.webimage.or_else(|| by("rdRichValueWebImage", &extra));
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

/// External URLs of the [MS-XLSX] Rich Value Web Image part (CT_WebImagesSupportingRichData
/// `webImagesSrd`): the `webImageSrd/address/@r:id` target of every entry, in order.
fn web_addresses(op: &[OpaquePart], name: Option<&String>) -> Vec<Option<String>> {
    let Some(name) = name else {
        return Vec::new();
    };
    let rels = part_rels(op, name);
    let Some(els) = data_of(op, Some(name)).and_then(|d| index(d, name)) else {
        return Vec::new();
    };
    children(&els, 0)
        .filter(|(_, e)| e.local == "webImageSrd")
        .map(|(i, _)| {
            let (_, a) = children(&els, i).find(|(_, e)| e.local == "address")?;
            let id = a.attr("id")?;
            rels.iter()
                .find(|r| r.id == id && r.external)
                .map(|r| r.target.clone())
        })
        .collect()
}

/// The pictures the rich values of the file describe, by `vm` (1-based): local
/// pictures (`_localImage`, [MS-XLSX] §2.3.6.1.10) and web pictures (`_webimage`,
/// §2.3.6.1.11). Only `vm`s used by a cell are resolved.
fn images_by_vm(wb: &Workbook) -> HashMap<u32, CellImage> {
    let mut out = HashMap::new();
    let parts = find_parts(&wb.opaque.workbook_rels, &wb.opaque.parts);
    let op = &wb.opaque.parts;
    let (Some(meta), Some(rv), Some(rvs)) = (
        data_of(op, parts.metadata.as_ref()),
        data_of(op, parts.rv.as_ref()),
        data_of(op, parts.rvs.as_ref()),
    ) else {
        return out;
    };
    let (Some(meta), Some(rv), Some(rvs)) = (
        index(meta, "/metadata"),
        index(rv, "/rdrichvalue"),
        index(rvs, "/rdrichvaluestructure"),
    ) else {
        return out;
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
    let web = web_addresses(op, parts.webimage.as_ref());
    let image_of = |vm: u32| -> Option<CellImage> {
        let rvi = (*vms.get((vm as usize).checked_sub(1)?)?)? as usize;
        let rv = values.get(rvi)?;
        let (t, keys) = structs.get(rv.s)?;
        let val = |k: &str| {
            keys.iter()
                .position(|n| n.eq_ignore_ascii_case(k))
                .and_then(|i| rv.values.get(i))
        };
        let num = |k: &str| val(k).and_then(|v| v.trim().parse::<f64>().ok());
        let mut img = match t.as_str() {
            "_localImage" => {
                let rel: usize = val(KEY_IMAGE)?.trim().parse().ok()?;
                let (data, ext) = pictures.get(rel)?.clone()?;
                CellImage::local(data, &ext, val("Text").cloned())
            }
            "_webimage" => {
                let w: usize = val(KEY_WEB_IMAGE)?.trim().parse().ok()?;
                let url = web.get(w)?.clone()?;
                let sizing = num("ImageSizing")
                    .and_then(|s| ImageSizing::from_code(s as u32))
                    .unwrap_or_default();
                CellImage {
                    source: CellImageSource::Web {
                        url,
                        sizing,
                        height: num("ImageHeight"),
                        width: num("ImageWidth"),
                    },
                    alt_text: val("Text").cloned(),
                    calc_origin: 1,
                }
            }
            _ => return None,
        };
        if let Some(o) = val("CalcOrigin").and_then(|v| v.trim().parse().ok()) {
            img.calc_origin = o;
        }
        Some(img)
    };
    for ws in &wb.sheets {
        for cell in ws.cells.values() {
            if let Some(vm) = cell.vm {
                if let std::collections::hash_map::Entry::Vacant(e) = out.entry(vm) {
                    if let Some(img) = image_of(vm) {
                        e.insert(img);
                    }
                }
            }
        }
    }
    out
}

/// After reading: attach `_localImage` and `_webimage` pictures to the cells that
/// point at them (an `IMAGE` result read from a file shows before it is recalculated).
pub(crate) fn resolve(wb: &mut Workbook) {
    let images = images_by_vm(wb);
    if images.is_empty() {
        return;
    }
    for ws in &mut wb.sheets {
        for cell in ws.cells.values_mut() {
            if let Some(img) = cell.vm.and_then(|vm| images.get(&vm)) {
                cell.image = Some(Box::new(img.clone()));
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

/// Whether the rich value a cell's `vm` points at no longer describes its picture
/// (always for pictures without a `vm`). Local pictures read from the file keep
/// their `vm`; an `IMAGE` result is compared with the web picture of its `vm`.
fn needs_new_value(img: &CellImage, vm: Option<u32>, old: &HashMap<u32, CellImage>) -> bool {
    match (vm, &img.source) {
        (None, _) => true,
        (Some(_), CellImageSource::Local { .. }) => false,
        (Some(vm), CellImageSource::Web { .. }) => old.get(&vm) != Some(img),
    }
}

/// Picture in a cell to write; `ComputedImage` and the key order of
/// [MS-XLSX] §2.3.6.1.11 for web pictures.
fn web_keys(img: &CellImage) -> Vec<(String, &'static str)> {
    let mut keys = vec![
        (KEY_WEB_IMAGE.to_owned(), "i"),
        ("CalcOrigin".to_owned(), "i"),
        ("ComputedImage".to_owned(), "b"),
        ("ImageSizing".to_owned(), "i"),
    ];
    if let CellImageSource::Web {
        height: Some(_), ..
    } = &img.source
    {
        keys.push(("ImageHeight".to_owned(), "d"));
    }
    if let CellImageSource::Web { width: Some(_), .. } = &img.source {
        keys.push(("ImageWidth".to_owned(), "d"));
    }
    if img.alt_text.is_some() {
        keys.push(("Text".to_owned(), "s"));
    }
    keys
}

/// A number as written in a rich value `v` (shortest round-trip form).
fn num_text(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// `vm` of every cell holding a picture placed since reading (or an `IMAGE`
/// result whose rich value changed), keyed by (sheet, row, col). Appends the
/// pictures to the rich-data parts: `_localImage` ([MS-XLSX] §2.3.6.1.10) and
/// `_webimage` (§2.3.6.1.11, the `IMAGE` function: its URL in the Rich Value Web
/// Image part as an external hyperlink relationship, no picture bytes).
#[allow(clippy::too_many_lines)]
pub(crate) fn write_new(wb: &Workbook, ctx: &mut WriteCtx<'_>) -> HashMap<(usize, u32, u32), u32> {
    let old_images = images_by_vm(wb);
    let mut pending: Vec<&CellImage> = Vec::new();
    let mut cells: Vec<((usize, u32, u32), usize)> = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        for (&(r, c), cell) in &ws.cells {
            let Some(img) = cell.image.as_deref() else {
                continue;
            };
            if !needs_new_value(img, cell.vm, &old_images) {
                continue;
            }
            let i = if let Some(i) = pending.iter().position(|p| *p == img) {
                i
            } else {
                pending.push(img);
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
    let fresh_id = |used: &mut Vec<String>| -> String {
        let mut n = used.len() + 1;
        let id = loop {
            let id = format!("rId{n}");
            if !used.contains(&id) {
                break id;
            }
            n += 1;
        };
        used.push(id.clone());
        id
    };
    let has = |local: bool| {
        pending
            .iter()
            .any(|p| matches!(p.source, CellImageSource::Local { .. }) == local)
    };

    // 1. Pictures → media parts and Rich Value Rels entries.
    let mut rel_of: Vec<usize> = vec![0; pending.len()];
    if has(true) {
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
        for (pi, img) in pending.iter().enumerate() {
            let CellImageSource::Local { data, ext, key } = &img.source else {
                continue;
            };
            if let Some(&i) = media_seen.get(key) {
                rel_of[pi] = i;
                continue;
            }
            let ext = if ext.is_empty() { "png" } else { ext.as_str() };
            let media = ctx.names.fresh(|n| format!("/xl/media/image{n}.{ext}"));
            ctx.out_parts
                .push((media.clone(), Some(media_type(ext).into()), data.clone()));
            let id = fresh_id(&mut used_ids);
            // [MS-XLSX] §2.6.241 CT_RichValueRelRelationship.
            new_rel_xml.push_str(&format!("<rel r:id=\"{id}\"/>"));
            new_rels.push_str(&format!(
                "<Relationship Id=\"{id}\" Type=\"{REL_IMAGE}\" Target=\"{}\"/>",
                escape(&relative_target(&rvrel_name, &media))
            ));
            let i = first_rel + media_seen.len();
            media_seen.insert(key.clone(), i);
            rel_of[pi] = i;
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
        append_rels(ctx, &rels_name, old_rels, &new_rels);
    }

    // 1b. Web pictures → `webImageSrd` entries of the Rich Value Web Image part, the
    // URL as an external hyperlink relationship (`address`).
    let mut web_of: Vec<usize> = vec![0; pending.len()];
    if has(false) {
        let part = current(&parts.webimage, ctx);
        let part_name = part.as_ref().map_or_else(
            || {
                ctx.names
                    .fresh(|n| numbered("/xl/richData/rdRichValueWebImage", n, "xml"))
            },
            |(n, _)| n.clone(),
        );
        let rels_name = bo_opc::rels_part_name(&part_name);
        let old_rels = data_of(&wb.opaque.parts, Some(&rels_name)).map(<[u8]>::to_vec);
        let mut used_ids: Vec<String> = old_rels
            .as_deref()
            .and_then(|d| index(d, &rels_name))
            .map(|els| els.iter().filter_map(|e| e.attr("Id")).collect())
            .unwrap_or_default();
        let first = part
            .as_ref()
            .and_then(|(n, d)| index(d, n))
            .map_or(0, |els| {
                children(&els, 0)
                    .filter(|(_, e)| e.local == "webImageSrd")
                    .count()
            });
        let mut xml = String::new();
        let mut new_rels = String::new();
        let mut url_seen: HashMap<String, usize> = HashMap::new();
        for (pi, img) in pending.iter().enumerate() {
            let CellImageSource::Web { url, .. } = &img.source else {
                continue;
            };
            if let Some(&i) = url_seen.get(url) {
                web_of[pi] = i;
                continue;
            }
            let id = fresh_id(&mut used_ids);
            xml.push_str(&format!(
                "<webImageSrd><address r:id=\"{id}\"/></webImageSrd>"
            ));
            new_rels.push_str(&format!(
                "<Relationship Id=\"{id}\" Type=\"{REL_HYPERLINK}\" Target=\"{}\" TargetMode=\"External\"/>",
                escape(url)
            ));
            let i = first + url_seen.len();
            url_seen.insert(url.clone(), i);
            web_of[pi] = i;
        }
        if let Some((name, data)) = &part {
            if let Some(out) = append_root(data, name, |root| {
                let needs_r = !root.attrs.iter().any(|(q, _)| q == "xmlns:r");
                let p = root.prefix();
                let mut x = if p.is_empty() {
                    xml.clone()
                } else {
                    prefixed(&xml, p)
                };
                if needs_r {
                    // The `r` prefix may not be declared on an existing root.
                    let tag = root.q("webImageSrd");
                    x = x.replace(&format!("<{tag}>"), &format!("<{tag} xmlns:r=\"{R_NS}\">"));
                }
                (x, None)
            }) {
                ctx.overrides.insert(name.clone(), out);
            }
        } else {
            ctx.out_parts.push((
                part_name.clone(),
                Some(CT_WEBIMAGE.into()),
                format!(
                    "{XML_DECL}<webImagesSrd xmlns=\"{NS_WEBIMAGE}\" xmlns:r=\"{R_NS}\">{xml}</webImagesSrd>"
                )
                .into_bytes(),
            ));
            add_wb_rel(ctx, REL_WEBIMAGE, &part_name);
        }
        append_rels(ctx, &rels_name, old_rels, &new_rels);
    }

    // 2. Structures: `_localImage` with and without alt text ([MS-XLSX] §2.3.6.1.10),
    // `_webimage` with the keys its picture uses (§2.3.6.1.11).
    let rvs = current(&parts.rvs, ctx);
    let mut structs = rvs
        .as_ref()
        .and_then(|(n, d)| index(d, n))
        .map(|e| structures(&e))
        .unwrap_or_default();
    let old_struct_count = structs.len();
    let mut types: Vec<Vec<(String, &'static str)>> = vec![Vec::new(); old_struct_count];
    let mut struct_of = |img: &CellImage| -> usize {
        let (t, keys): (&str, Vec<(String, &'static str)>) = match img.source {
            CellImageSource::Local { .. } => {
                let mut keys = vec![(KEY_IMAGE.to_owned(), "i"), ("CalcOrigin".to_owned(), "i")];
                if img.alt_text.is_some() {
                    keys.push(("Text".to_owned(), "s"));
                }
                ("_localImage", keys)
            }
            CellImageSource::Web { .. } => ("_webimage", web_keys(img)),
        };
        let names: Vec<String> = keys.iter().map(|(k, _)| k.clone()).collect();
        if let Some(i) = structs.iter().position(|(st, k)| st == t && *k == names) {
            return i;
        }
        structs.push((t.into(), names));
        types.push(keys);
        structs.len() - 1
    };
    let struct_ids: Vec<usize> = pending.iter().map(|img| struct_of(img)).collect();
    let mut new_s = String::new();
    for ((t, _), keys) in structs[old_struct_count..]
        .iter()
        .zip(&types[old_struct_count..])
    {
        // §2.6.180 CT_RichValueStructure / §2.6.163 CT_Key.
        new_s.push_str(&format!("<s t=\"{}\">", escape(t)));
        for (k, ty) in keys {
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
    for (i, img) in pending.iter().enumerate() {
        let origin = img.calc_origin;
        new_rv.push_str(&format!("<rv s=\"{}\">", struct_ids[i]));
        match &img.source {
            CellImageSource::Local { .. } => {
                new_rv.push_str(&format!("<v>{}</v><v>{origin}</v>", rel_of[i]));
            }
            CellImageSource::Web {
                sizing,
                height,
                width,
                ..
            } => {
                // ComputedImage: the picture is the result of a formula.
                new_rv.push_str(&format!(
                    "<v>{}</v><v>{origin}</v><v>1</v><v>{}</v>",
                    web_of[i],
                    sizing.code()
                ));
                for v in [height, width].into_iter().flatten() {
                    new_rv.push_str(&format!("<v>{}</v>", num_text(*v)));
                }
            }
        }
        if let Some(a) = &img.alt_text {
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

// ------------------------------------------------------------------ dynamic arrays

/// Metadata type name of dynamic array properties ([MS-XLSX] `CT_DynamicArrayProperties`,
/// cell metadata of a dynamic array formula).
pub const XLDAPR: &str = "XLDAPR";
/// Ext URI of `xda:dynamicArrayProperties` in `futureMetadata/bk/extLst` ([MS-XLSX]).
const EXT_DA: &str = "{bdbb8cdc-fa1e-496e-a857-3c3f30c029c3}";
/// Namespace of `dynamicArrayProperties` ([MS-XLSX]).
const NS_DA: &str = "http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray";

/// §18.9.10 `metadataType` for dynamic array properties. The flags are our own choice:
/// the property follows the cell like its formula does.
fn da_metadata_type() -> String {
    format!(
        "<metadataType name=\"{XLDAPR}\" minSupportedVersion=\"120000\" copy=\"1\" \
         pasteAll=\"1\" pasteValues=\"1\" merge=\"1\" splitFirst=\"1\" rowColShift=\"1\" \
         clearFormats=\"1\" clearComments=\"1\" assign=\"1\" coerce=\"1\" cellMeta=\"1\"/>"
    )
}

/// `futureMetadata/bk` of a dynamic array formula (`fDynamic="1"`).
fn da_future_bk(prefix: &str) -> String {
    let q = |l: &str| {
        if prefix.is_empty() {
            l.to_owned()
        } else {
            format!("{prefix}:{l}")
        }
    };
    format!(
        "<{bk}><{ext_lst}><{ext} uri=\"{EXT_DA}\" xmlns:xda=\"{NS_DA}\">\
         <xda:dynamicArrayProperties fDynamic=\"1\" fCollapsed=\"0\"/></{ext}></{ext_lst}></{bk}>",
        bk = q("bk"),
        ext_lst = q("extLst"),
        ext = q("ext"),
    )
}

/// `cm` values (1-based into `cellMetadata`, §18.9.3) whose block marks a dynamic array
/// formula: an `rc` of type `XLDAPR` naming a `futureMetadata` entry with `fDynamic="1"`.
fn dynamic_cms(els: &[El]) -> Vec<u32> {
    let types: Vec<String> = els
        .iter()
        .filter(|e| e.local == "metadataType")
        .map(|e| e.attr("name").unwrap_or_default())
        .collect();
    let dynamic: Vec<bool> = els
        .iter()
        .enumerate()
        .find(|(_, e)| e.local == "futureMetadata" && e.attr("name").as_deref() == Some(XLDAPR))
        .map(|(i, _)| {
            children(els, i)
                .filter(|(_, e)| e.local == "bk")
                .map(|(bk, _)| {
                    let mut stack = vec![bk];
                    while let Some(n) = stack.pop() {
                        for (c, e) in children(els, n) {
                            if e.local == "dynamicArrayProperties" {
                                return matches!(e.attr("fDynamic").as_deref(), Some("1" | "true"));
                            }
                            stack.push(c);
                        }
                    }
                    false
                })
                .collect()
        })
        .unwrap_or_default();
    let Some(cmi) = els.iter().position(|e| e.local == "cellMetadata") else {
        return Vec::new();
    };
    children(els, cmi)
        .filter(|(_, e)| e.local == "bk")
        .enumerate()
        .filter_map(|(k, (bk, _))| {
            let is_dynamic = children(els, bk)
                .filter(|(_, e)| e.local == "rc")
                .any(|(_, rc)| {
                    let t: Option<usize> = rc.attr("t").and_then(|t| t.parse().ok());
                    let v: Option<usize> = rc.attr("v").and_then(|v| v.parse().ok());
                    match (t, v) {
                        (Some(t), Some(v)) => {
                            t.checked_sub(1)
                                .and_then(|t| types.get(t))
                                .map(String::as_str)
                                == Some(XLDAPR)
                                && dynamic.get(v).copied().unwrap_or(false)
                        }
                        _ => false,
                    }
                });
            is_dynamic.then(|| u32::try_from(k + 1).ok()).flatten()
        })
        .collect()
}

/// After reading: an array formula whose cell metadata marks a dynamic array spills
/// ([`bo_model::sheet::Formula::dynamic`]); the others are legacy array formulas.
pub(crate) fn resolve_dynamic(wb: &mut Workbook) {
    let parts = find_parts(&wb.opaque.workbook_rels, &wb.opaque.parts);
    let Some(name) = parts.metadata.as_ref() else {
        return;
    };
    let Some(els) = data_of(&wb.opaque.parts, Some(name)).and_then(|d| index(d, name)) else {
        return;
    };
    let cms = dynamic_cms(&els);
    if cms.is_empty() {
        return;
    }
    for ws in &mut wb.sheets {
        for cell in ws.cells.values_mut() {
            let Some(cm) = cell.cm else { continue };
            if let Some(f) = cell.formula.as_mut() {
                if matches!(f.kind, bo_model::sheet::FormulaKind::Array { .. }) && cms.contains(&cm)
                {
                    f.dynamic = true;
                }
            }
        }
    }
}

/// Adds the dynamic array cell metadata to an existing metadata part; returns it and the
/// `cm` of the new block.
fn patch_dynamic(data: &[u8], part: &str) -> Option<(Vec<u8>, u32)> {
    let els = index(data, part)?;
    let root = els.first()?;
    let q = |l: &str| root.q(l);
    let pre = root.prefix();
    let mut p = Patch::default();
    let kids: Vec<(usize, &El)> = children(&els, 0).collect();
    let find = |l: &str| kids.iter().find(|(_, e)| e.local == l).copied();
    let types: Vec<(usize, &El)> = match find("metadataTypes") {
        Some((i, _)) => children(&els, i)
            .filter(|(_, e)| e.local == "metadataType")
            .collect(),
        None => Vec::new(),
    };
    let t = if let Some(i) = types
        .iter()
        .position(|(_, e)| e.attr("name").as_deref() == Some(XLDAPR))
    {
        i + 1
    } else {
        let ty = if pre.is_empty() {
            da_metadata_type()
        } else {
            prefixed(&da_metadata_type(), pre)
        };
        if let Some((i, _)) = find("metadataTypes") {
            p.append(i, &ty);
            p.set_attr(i, "count", &(types.len() + 1).to_string());
        } else {
            let open = format!("<{0} count=\"1\">{ty}</{0}>", q("metadataTypes"));
            match kids.first() {
                Some((_, first)) => p.insert_before(first, &open),
                None => p.append(0, &open),
            }
        }
        types.len() + 1
    };
    let future = kids
        .iter()
        .find(|(_, e)| e.local == "futureMetadata" && e.attr("name").as_deref() == Some(XLDAPR))
        .copied();
    let v = if let Some((i, _)) = future {
        let n = children(&els, i).filter(|(_, e)| e.local == "bk").count();
        p.append(i, &da_future_bk(pre));
        p.set_attr(i, "count", &(n + 1).to_string());
        n
    } else {
        let block = format!(
            "<{0} name=\"{XLDAPR}\" count=\"1\">{1}</{0}>",
            q("futureMetadata"),
            da_future_bk(pre)
        );
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
    let rc = format!("<{0}><{1} t=\"{t}\" v=\"{v}\"/></{0}>", q("bk"), q("rc"));
    let cm = if let Some((i, _)) = find("cellMetadata") {
        let n = children(&els, i).filter(|(_, e)| e.local == "bk").count();
        p.append(i, &rc);
        p.set_attr(i, "count", &(n + 1).to_string());
        n + 1
    } else {
        let block = format!("<{0} count=\"1\">{rc}</{0}>", q("cellMetadata"));
        let before = kids
            .iter()
            .find(|(_, e)| matches!(e.local.as_str(), "valueMetadata" | "extLst"))
            .copied();
        match before {
            Some((_, el)) => p.insert_before(el, &block),
            None => p.append(0, &block),
        }
        1
    };
    Some((p.apply(data, &els), u32::try_from(cm).ok()?))
}

/// `cm` of every dynamic array formula cell without one (entered since reading), keyed by
/// (sheet, row, col): the metadata part gets (or already has) a dynamic array block.
pub(crate) fn write_dynamic(
    wb: &Workbook,
    ctx: &mut WriteCtx<'_>,
) -> HashMap<(usize, u32, u32), u32> {
    let mut cells: Vec<(usize, u32, u32)> = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        for (&(r, c), cell) in &ws.cells {
            if cell.cm.is_none()
                && cell.formula.as_ref().is_some_and(|f| {
                    f.dynamic && matches!(f.kind, bo_model::sheet::FormulaKind::Array { .. })
                })
            {
                cells.push((si, r, c));
            }
        }
    }
    if cells.is_empty() {
        return HashMap::new();
    }
    // The metadata part: created by `write_new` in this write, or the one read.
    let fresh = ctx
        .out_parts
        .iter()
        .position(|(_, ct, _)| ct.as_deref() == Some(CT_METADATA));
    let cm = if let Some(i) = fresh {
        let (name, _, data) = &ctx.out_parts[i];
        let existing = index(data, name)
            .map(|els| dynamic_cms(&els))
            .unwrap_or_default();
        if let Some(&cm) = existing.first() {
            cm
        } else {
            let Some((out, cm)) = patch_dynamic(data, name) else {
                return HashMap::new();
            };
            ctx.out_parts[i].2 = out;
            cm
        }
    } else {
        let parts = find_parts(&wb.opaque.workbook_rels, &wb.opaque.parts);
        let current = parts.metadata.as_ref().and_then(|n| {
            let data = ctx
                .overrides
                .get(n)
                .cloned()
                .or_else(|| data_of(&wb.opaque.parts, Some(n)).map(<[u8]>::to_vec))?;
            Some((n.clone(), data))
        });
        if let Some((name, data)) = current {
            let existing = index(&data, &name)
                .map(|els| dynamic_cms(&els))
                .unwrap_or_default();
            if let Some(&cm) = existing.first() {
                cm
            } else {
                let Some((out, cm)) = patch_dynamic(&data, &name) else {
                    return HashMap::new();
                };
                ctx.overrides.insert(name, out);
                cm
            }
        } else {
            let name = ctx.names.fresh(|n| numbered("/xl/metadata", n, "xml"));
            let xml = format!(
                "{XML_DECL}<metadata xmlns=\"{SML_NS}\" xmlns:xda=\"{NS_DA}\">\
                 <metadataTypes count=\"1\">{}</metadataTypes>\
                 <futureMetadata name=\"{XLDAPR}\" count=\"1\">{}</futureMetadata>\
                 <cellMetadata count=\"1\"><bk><rc t=\"1\" v=\"0\"/></bk></cellMetadata></metadata>",
                da_metadata_type(),
                da_future_bk("")
            );
            ctx.out_parts
                .push((name.clone(), Some(CT_METADATA.into()), xml.into_bytes()));
            add_wb_rel(ctx, REL_METADATA, &name);
            1
        }
    };
    cells.into_iter().map(|k| (k, cm)).collect()
}

/// Appends `new_rels` to the relationships part `rels_name` (created when missing).
fn append_rels(ctx: &mut WriteCtx<'_>, rels_name: &str, old: Option<Vec<u8>>, new_rels: &str) {
    match ctx.overrides.get(rels_name).cloned().or(old) {
        Some(data) => {
            if let Some(out) = append_root(&data, rels_name, |_| (new_rels.to_owned(), None)) {
                ctx.overrides.insert(rels_name.to_owned(), out);
            }
        }
        None => ctx.out_parts.push((
            rels_name.to_owned(),
            None,
            format!("{XML_DECL}<Relationships xmlns=\"{PKG_REL_NS}\">{new_rels}</Relationships>")
                .into_bytes(),
        )),
    }
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
