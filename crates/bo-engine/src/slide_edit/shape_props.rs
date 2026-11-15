//! Shape properties edited from BlueSlide's Selection Pane and Alt Text pane, and the
//! table style gallery of Table Design (N6, round 5):
//!
//! - `setShapeProps`: name (`cNvPr@name`), alternative text (`cNvPr@descr`), visibility
//!   (`cNvPr@hidden`) — ECMA-376 Part 1, §19.3.1.12 cNvPr / §20.1.2.2.8 — and "decorative"
//!   ([MS-ODRAWXML] `adec:decorative` in the `cNvPr` extension list, §20.1.2.2.15 extLst) of
//!   any shape of the current slide, hidden ones and group members included.
//! - `shapeOrder`: one step up or down the stacking order of the shape's container (§19.3.1.45
//!   spTree / §19.3.1.22 grpSp children are in z-order), hidden shapes included.
//! - `setTableStyle`: a built-in style (§21.1.3.15 tableStyleId, the style written into the
//!   deck's §20.1.4.2.27 tblStyleLst part) for the selected table.
//! - `setTableFlags`: the §21.1.3.15 tblPr emphasis and banding flags.
//!
//! Each command is one undoable step.

use super::ops::Tx;
use super::{build, ensure_part, refuse, table, Outcome, SlideEditResult, SlideSel, R};
use crate::{EngineError, Result};
use bo_model::slide::{Extra, GraphicContent, NonVisual, ShapeNode, XmlElement, XmlNode};
use serde::Deserialize;

/// [MS-ODRAWXML] extension URI of `adec:decorative`.
pub const DECORATIVE_URI: &str = "{C183D7F6-B498-43B3-948B-1728B52AA6E4}";
/// [MS-ODRAWXML] namespace of `adec:decorative`.
pub const DECORATIVE_NS: &str = "http://schemas.microsoft.com/office/drawing/2017/decorative";

/// Position key of `a:extLst` among the children of `cNvPr` (hlinkClick, hlinkHover, extLst).
const CNV_EXT_KEY: u16 = 2;

/// A shape properties command (JSON bridge, ADR-0008).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ShapePropsCommand {
    /// Change shape `id` of the current slide (fields left out stay as they are).
    SetShapeProps {
        /// Shape id (`cNvPr@id`).
        id: u32,
        /// New name.
        #[serde(default)]
        name: Option<String>,
        /// New alternative text (empty removes it).
        #[serde(default)]
        descr: Option<String>,
        /// Hidden.
        #[serde(default)]
        hidden: Option<bool>,
        /// Decorative (no alternative text needed).
        #[serde(default)]
        decorative: Option<bool>,
    },
    /// Move shape `id` one step towards the front (`forward`) or the back.
    ShapeOrder {
        /// Shape id.
        id: u32,
        /// Towards the front.
        forward: bool,
    },
    /// Give the selected table built-in style `style` (0-based accent).
    SetTableStyle {
        /// Built-in style index (`bo_pptx::table_style::BUILTIN_IDS`).
        style: usize,
    },
    /// Set table style options of the selected table.
    SetTableFlags {
        /// `firstRow`.
        #[serde(default)]
        first_row: Option<bool>,
        /// `lastRow`.
        #[serde(default)]
        last_row: Option<bool>,
        /// `firstCol`.
        #[serde(default)]
        first_col: Option<bool>,
        /// `lastCol`.
        #[serde(default)]
        last_col: Option<bool>,
        /// `bandRow`.
        #[serde(default)]
        band_row: Option<bool>,
        /// `bandCol`.
        #[serde(default)]
        band_col: Option<bool>,
    },
}

/// Command names of [`ShapePropsCommand`].
pub const COMMANDS: &[&str] = &[
    "setShapeProps",
    "shapeOrder",
    "setTableStyle",
    "setTableFlags",
];

impl ShapePropsCommand {
    /// Parses a JSON command when it is one of these.
    pub fn parse(v: &serde_json::Value) -> Option<ShapePropsCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !COMMANDS.contains(&c) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// Is `nv` marked decorative ([MS-ODRAWXML] `adec:decorative@val`)?
pub fn is_decorative(nv: &NonVisual) -> bool {
    nv.cnv_extra
        .iter()
        .filter(|x| x.xml.local() == "extLst")
        .flat_map(|x| x.xml.elements())
        .filter(|e| e.local() == "ext" && e.attr("uri") == Some(DECORATIVE_URI))
        .flat_map(XmlElement::elements)
        .any(|d| d.local() == "decorative" && d.attr("val").is_none_or(|v| v == "1" || v == "true"))
}

/// Mark or unmark `nv` decorative; other extensions stay.
fn set_decorative(nv: &mut NonVisual, on: bool) {
    let pos = nv.cnv_extra.iter().position(|x| x.xml.local() == "extLst");
    if let Some(i) = pos {
        let lst = &mut nv.cnv_extra[i].xml;
        lst.children.retain(|c| {
            !matches!(c, XmlNode::Element(e) if e.local() == "ext" && e.attr("uri") == Some(DECORATIVE_URI))
        });
        if lst.elements().next().is_none() && !on {
            nv.cnv_extra.remove(i);
            return;
        }
    }
    if !on {
        return;
    }
    let mut dec = XmlElement::new("adec:decorative");
    dec.attrs = vec![
        ("xmlns:adec".into(), DECORATIVE_NS.into()),
        ("val".into(), "1".into()),
    ];
    let mut ext = XmlElement::new("a:ext");
    ext.attrs = vec![("uri".into(), DECORATIVE_URI.into())];
    ext.children.push(XmlNode::Element(dec));
    if let Some(x) = nv.cnv_extra.iter_mut().find(|x| x.xml.local() == "extLst") {
        x.xml.children.push(XmlNode::Element(ext));
    } else {
        let mut lst = XmlElement::new("a:extLst");
        lst.children.push(XmlNode::Element(ext));
        nv.cnv_extra.push(Extra {
            key: CNV_EXT_KEY,
            xml: lst,
        });
    }
}

fn nv_mut(n: &mut ShapeNode) -> Option<&mut NonVisual> {
    match n {
        ShapeNode::Shape(s) => Some(&mut s.nv),
        ShapeNode::Picture(p) => Some(&mut p.nv),
        ShapeNode::Group(g) => Some(&mut g.nv),
        ShapeNode::Connector(c) => Some(&mut c.nv),
        ShapeNode::Frame(f) => Some(&mut f.nv),
        ShapeNode::Other(_) => None,
    }
}

/// Index path of shape `id` in `nodes` (group children included, hidden shapes too).
pub fn path_of(nodes: &[ShapeNode], id: u32) -> Option<Vec<usize>> {
    for (i, n) in nodes.iter().enumerate() {
        if n.nv().is_some_and(|nv| nv.id == id) {
            return Some(vec![i]);
        }
        if let ShapeNode::Group(g) = n {
            if let Some(mut p) = path_of(&g.children, id) {
                p.insert(0, i);
                return Some(p);
            }
        }
    }
    None
}

fn shape_path(tx: &Tx<'_>, slide: usize, id: u32) -> R<Vec<usize>> {
    tx.pres
        .slides
        .get(slide)
        .and_then(|s| path_of(&s.data.shapes, id))
        .map_or_else(|| refuse("noShape"), Ok)
}

fn set_props(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    id: u32,
    (name, descr, hidden, decorative): (Option<String>, Option<String>, Option<bool>, Option<bool>),
) -> R<()> {
    let path = shape_path(tx, sel.slide, id)?;
    let Some(old) = super::ops::node_at(tx.pres, sel.slide, &path) else {
        return refuse("noShape");
    };
    let mut node = old.clone();
    let Some(nv) = nv_mut(&mut node) else {
        return refuse("noShape");
    };
    if let Some(n) = name {
        let n = n.trim();
        if n.is_empty() {
            return refuse("emptyName");
        }
        n.clone_into(&mut nv.name);
    }
    if let Some(d) = descr {
        let d = d.trim();
        nv.descr = (!d.is_empty()).then(|| d.to_owned());
    }
    if let Some(h) = hidden {
        // `hidden` defaults to false: leave the attribute out when shown.
        nv.hidden = h.then_some(true);
    }
    if let Some(d) = decorative {
        set_decorative(nv, d);
    }
    tx.replace_node(sel.slide, &path, node)?;
    Ok(())
}

fn order(tx: &mut Tx<'_>, sel: &SlideSel, id: u32, forward: bool) -> R<()> {
    let path = shape_path(tx, sel.slide, id)?;
    let Some((&last, parent)) = path.split_last() else {
        return refuse("noShape");
    };
    let Some(old) = super::ops::container(tx.pres, sel.slide, parent).cloned() else {
        return refuse("noShape");
    };
    let to = if forward {
        (last + 1 < old.len()).then_some(last + 1)
    } else {
        last.checked_sub(1)
    };
    let Some(to) = to else {
        return refuse("zOrderEnd");
    };
    let mut a = old[last].clone();
    let mut b = old[to].clone();
    std::mem::swap(&mut a, &mut b);
    let mut pa = parent.to_vec();
    pa.push(last);
    let mut pb = parent.to_vec();
    pb.push(to);
    tx.replace_node(sel.slide, &pa, a)?;
    tx.replace_node(sel.slide, &pb, b)?;
    Ok(())
}

/// The selected table's path (the only selected shape, a table frame).
fn table_path(tx: &Tx<'_>, sel: &SlideSel) -> R<Vec<usize>> {
    let [id] = sel.ids[..] else {
        return refuse("noTable");
    };
    let path = shape_path(tx, sel.slide, id)?;
    match super::ops::node_at(tx.pres, sel.slide, &path) {
        Some(ShapeNode::Frame(f)) if matches!(f.content, GraphicContent::Table(_)) => Ok(path),
        _ => refuse("noTable"),
    }
}

fn edit_table(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    f: impl FnOnce(&mut bo_model::slide::Table),
) -> R<()> {
    let path = table_path(tx, sel)?;
    let Some(ShapeNode::Frame(fr)) = super::ops::node_at(tx.pres, sel.slide, &path).cloned() else {
        return refuse("noTable");
    };
    let mut fr = fr;
    let GraphicContent::Table(t) = &mut fr.content else {
        return refuse("noTable");
    };
    f(t);
    tx.replace_node(sel.slide, &path, ShapeNode::Frame(fr))?;
    Ok(())
}

/// Set the `a:tableStyleId` child of `tblPr` (§21.1.3.15).
pub fn set_style_id(t: &mut bo_model::slide::Table, id: &str) {
    let p = t.props.get_or_insert_with(|| XmlElement::new("a:tblPr"));
    let mut el = XmlElement::new("a:tableStyleId");
    el.children.push(XmlNode::Text(id.to_owned()));
    if let Some(slot) = p
        .children
        .iter_mut()
        .find(|c| matches!(c, XmlNode::Element(e) if e.local() == "tableStyleId"))
    {
        *slot = XmlNode::Element(el);
    } else {
        // tableStyleId is the last child of the CT_TableProperties sequence (before extLst).
        let at = p
            .children
            .iter()
            .position(|c| matches!(c, XmlNode::Element(e) if e.local() == "extLst"))
            .unwrap_or(p.children.len());
        p.children.insert(at, XmlNode::Element(el));
    }
}

impl crate::Engine {
    /// Run a shape properties / table style command on the current slide.
    pub fn slide_props_command(&mut self, cmd: ShapePropsCommand) -> Result<SlideEditResult> {
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            match cmd {
                ShapePropsCommand::SetShapeProps {
                    id,
                    name,
                    descr,
                    hidden,
                    decorative,
                } => ed.mutate(pres, &mut out, |tx, sel| {
                    set_props(tx, sel, id, (name, descr, hidden, decorative))?;
                    // A shape hidden now can no longer stay selected.
                    if hidden == Some(true) {
                        sel.ids.retain(|i| *i != id);
                    }
                    Ok(())
                }),
                ShapePropsCommand::ShapeOrder { id, forward } => {
                    ed.mutate(pres, &mut out, |tx, sel| order(tx, sel, id, forward));
                }
                ShapePropsCommand::SetTableStyle { style } => {
                    ed.table = None;
                    ed.mutate(pres, &mut out, |tx, sel| {
                        let n = style % bo_pptx::table_style::BUILTIN_IDS.len();
                        table_path(tx, sel)?;
                        if let Some(part) = build::table_styles_with(tx.pres, n) {
                            ensure_part(tx, part)?;
                        }
                        let id = bo_pptx::table_style::BUILTIN_IDS[n];
                        edit_table(tx, sel, |t| set_style_id(t, id))
                    });
                }
                ShapePropsCommand::SetTableFlags {
                    first_row,
                    last_row,
                    first_col,
                    last_col,
                    band_row,
                    band_col,
                } => {
                    ed.table = None;
                    ed.mutate(pres, &mut out, |tx, sel| {
                        edit_table(tx, sel, |t| {
                            for (name, v) in [
                                ("firstRow", first_row),
                                ("lastRow", last_row),
                                ("firstCol", first_col),
                                ("lastCol", last_col),
                                ("bandRow", band_row),
                                ("bandCol", band_col),
                            ] {
                                if let Some(v) = v {
                                    table::set_flag(t, name, v);
                                }
                            }
                        })
                    });
                }
            }
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slide_edit::SlideCommand;
    use crate::Engine;

    fn blank() -> Engine {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide/slide-edit-arrange.pptx");
        let bytes = std::fs::read(&p).expect("corpus deck");
        let mut e = Engine::new();
        e.open_pptx(&bytes, "d.pptx").expect("open");
        e
    }

    fn deck() -> Engine {
        let mut e = blank();
        e.slide_command(SlideCommand::InsertShape {
            preset: "rect".into(),
            rect: None,
        })
        .expect("rect");
        e
    }

    fn shapes(e: &Engine) -> Vec<NonVisual> {
        e.presentation().expect("deck").slides[0]
            .data
            .shapes
            .iter()
            .filter_map(|n| n.nv().cloned())
            .collect()
    }

    #[test]
    fn name_alt_text_hidden_and_decorative_round_trip() {
        let mut e = deck();
        let id = shapes(&e).last().expect("shape").id;
        let r = e
            .slide_props_command(ShapePropsCommand::SetShapeProps {
                id,
                name: Some("Logo".into()),
                descr: Some("Company logo".into()),
                hidden: Some(true),
                decorative: Some(true),
            })
            .expect("props");
        assert!(r.error.is_none(), "{:?}", r.error);
        let nv = shapes(&e).into_iter().find(|n| n.id == id).expect("shape");
        assert_eq!(nv.name, "Logo");
        assert_eq!(nv.descr.as_deref(), Some("Company logo"));
        assert_eq!(nv.hidden, Some(true));
        assert!(is_decorative(&nv));
        // Saved and opened again: everything is still there.
        let bytes = e.save_pptx().expect("save");
        let mut back = Engine::new();
        back.open_pptx(&bytes, "b.pptx").expect("open");
        let nv = shapes(&back)
            .into_iter()
            .find(|n| n.id == id)
            .expect("shape");
        assert_eq!(nv.hidden, Some(true));
        assert!(is_decorative(&nv));
        // Shown again, not decorative: the attribute and the extension go.
        e.slide_props_command(ShapePropsCommand::SetShapeProps {
            id,
            name: None,
            descr: Some(String::new()),
            hidden: Some(false),
            decorative: Some(false),
        })
        .expect("props");
        let nv = shapes(&e).into_iter().find(|n| n.id == id).expect("shape");
        assert_eq!(nv.hidden, None);
        assert_eq!(nv.descr, None);
        assert_eq!(nv.cnv_extra.len(), 0);
        // One undo step each.
        e.slide_command(SlideCommand::Undo).expect("undo");
        let nv = shapes(&e).into_iter().find(|n| n.id == id).expect("shape");
        assert_eq!(nv.hidden, Some(true));
    }

    #[test]
    fn order_steps_hidden_shapes_too() {
        let mut e = deck();
        e.slide_command(SlideCommand::InsertShape {
            preset: "ellipse".into(),
            rect: None,
        })
        .expect("ellipse");
        let ids: Vec<u32> = shapes(&e).iter().map(|n| n.id).collect();
        let (a, b) = (ids[ids.len() - 2], ids[ids.len() - 1]);
        e.slide_props_command(ShapePropsCommand::SetShapeProps {
            id: a,
            name: None,
            descr: None,
            hidden: Some(true),
            decorative: None,
        })
        .expect("hide");
        e.slide_props_command(ShapePropsCommand::ShapeOrder {
            id: a,
            forward: true,
        })
        .expect("order");
        let now: Vec<u32> = shapes(&e).iter().map(|n| n.id).collect();
        assert_eq!(&now[now.len() - 2..], &[b, a]);
        let r = e
            .slide_props_command(ShapePropsCommand::ShapeOrder {
                id: a,
                forward: true,
            })
            .expect("order");
        assert!(r.error.is_some());
    }

    #[test]
    fn table_style_and_flags() {
        let mut e = blank();
        e.slide_command(SlideCommand::InsertTable {
            rows: 3,
            cols: 3,
            rect: None,
        })
        .expect("table");
        let r = e
            .slide_props_command(ShapePropsCommand::SetTableStyle { style: 2 })
            .expect("style");
        assert!(r.error.is_none(), "{:?}", r.error);
        r.selection.first().expect("selected");
        e.slide_props_command(ShapePropsCommand::SetTableFlags {
            first_row: None,
            last_row: Some(true),
            first_col: Some(true),
            last_col: None,
            band_row: Some(false),
            band_col: Some(true),
        })
        .expect("flags");
        let pres = e.presentation().expect("deck");
        let t = pres.slides[0]
            .data
            .shapes
            .iter()
            .find_map(|n| match n {
                ShapeNode::Frame(f) => match &f.content {
                    GraphicContent::Table(t) => Some(t.clone()),
                    GraphicContent::Other(_) => None,
                },
                _ => None,
            })
            .expect("table");
        let (flags, id) = bo_pptx::table_style::table_props(t.props.as_ref());
        assert_eq!(id.as_deref(), Some(bo_pptx::table_style::BUILTIN_IDS[2]));
        assert!(flags.first_row && flags.last_row && flags.first_col && flags.band_col);
        assert!(!flags.band_row);
        let styles = bo_pptx::table_style::deck_styles(pres);
        assert!(styles
            .iter()
            .any(|s| s.id == bo_pptx::table_style::BUILTIN_IDS[2]));
        let json = r#"{"cmd":"setTableStyle","style":4}"#;
        let out = e.slide_command_json(json).expect("json");
        assert!(!out.contains("\"error\""), "{out}");
    }
}
