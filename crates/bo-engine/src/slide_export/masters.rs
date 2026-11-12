//! What the exports read from the handout master (ECMA-376 Part 1, §19.3.1.24
//! `handoutMaster`) and the notes master (§19.3.1.27 `notesMaster`), which the
//! reader keeps as raw parts: the header / footer / date / slide number
//! placeholders (§19.3.1.36 `ph`, §19.7.10 `ST_PlaceholderType` `hdr`, `ftr`,
//! `dt`, `sldNum`), their `p:hf` visibility flags (§19.3.1.25) and, for notes
//! pages, the slide image (`sldImg`) and notes body (`body`) boxes.

use super::handout::Frame;
use super::EMU_PER_PT;
use bo_model::slide::{Notes, Presentation, Shape, ShapeNode, TextRun, XmlElement};

/// A header / footer item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HfKind {
    /// `hdr`.
    Header,
    /// `ftr`.
    Footer,
    /// `dt`.
    Date,
    /// `sldNum`.
    SlideNumber,
}

/// Horizontal alignment of a header / footer line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Left.
    Left,
    /// Centre.
    Center,
    /// Right.
    Right,
}

impl HfKind {
    /// Every item, in drawing order.
    pub const ALL: [HfKind; 4] = [
        HfKind::Header,
        HfKind::Date,
        HfKind::Footer,
        HfKind::SlideNumber,
    ];

    /// `ST_PlaceholderType` value.
    fn ph(self) -> &'static str {
        match self {
            HfKind::Header => "hdr",
            HfKind::Footer => "ftr",
            HfKind::Date => "dt",
            HfKind::SlideNumber => "sldNum",
        }
    }

    /// Footer and slide number sit at the bottom of the page.
    pub fn at_bottom(self) -> bool {
        matches!(self, HfKind::Footer | HfKind::SlideNumber)
    }

    /// Our layout without a master: header / footer left, date / number right.
    pub fn default_align(self) -> Align {
        match self {
            HfKind::Header | HfKind::Footer => Align::Left,
            HfKind::Date | HfKind::SlideNumber => Align::Right,
        }
    }
}

/// A header / footer placeholder of a master.
#[derive(Debug, Clone, PartialEq)]
pub struct HfPlaceholder {
    kind: HfKind,
    /// Box in points of the master's page (`notesSz`).
    pub frame: Frame,
    /// Paragraph alignment (§21.1.2.2.7 `pPr@algn`), when set.
    pub align: Option<Align>,
    /// Font size of the first run (§21.1.2.3.9 `rPr@sz`), points.
    pub size_pt: Option<f32>,
    /// Text pieces: literal text, or `None` for a slide number field.
    parts: Vec<Option<String>>,
}

impl HfPlaceholder {
    /// The placeholder's text with slide number fields (§21.1.2.2.4 `fld`
    /// `type="slidenum"`) replaced by `number`; other fields show their cached text.
    pub fn text(&self, number: &str) -> String {
        let s: String = self
            .parts
            .iter()
            .map(|p| p.as_deref().unwrap_or(number))
            .collect();
        if s.trim().is_empty() && self.kind == HfKind::SlideNumber {
            return number.to_owned();
        }
        s
    }
}

/// Header / footer and layout boxes of a master.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MasterInfo {
    /// §19.3.1.25 `hf` flags (hdr, ftr, dt, sldNum), all on by default.
    flags: [bool; 4],
    placeholders: Vec<HfPlaceholder>,
    /// Slide image box (`sldImg`), notes master only.
    pub slide_image: Option<Frame>,
    /// Notes body box (`body`), notes master only.
    pub body: Option<Frame>,
    /// Level 1 size of the notes text style (§19.3.1.28 `notesStyle`), points.
    pub notes_size_pt: Option<f32>,
    /// The notes text style itself (§19.3.1.28 `notesStyle`), notes master only.
    pub notes_style: Option<bo_model::slide::ListStyle>,
    /// Colour map of the master (§19.3.1.6 `clrMap`).
    pub clr_map: Vec<(String, String)>,
}

impl MasterInfo {
    /// Is the item switched on by the master's `p:hf`?
    pub fn shows(&self, kind: HfKind) -> bool {
        let i = HfKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
        self.flags[i]
    }

    /// The master's placeholder of `kind`.
    pub fn placeholder(&self, kind: HfKind) -> Option<&HfPlaceholder> {
        self.placeholders.iter().find(|p| p.kind == kind)
    }
}

/// Boxes of a notes slide's own placeholders (they override the master's).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NotesBoxes {
    /// `sldImg` box (points of the notes page).
    pub slide_image: Option<Frame>,
    /// `body` box.
    pub body: Option<Frame>,
}

/// Box of a shape's `a:xfrm` in points (§20.1.7.6).
fn shape_frame(s: &Shape) -> Option<Frame> {
    let x = s.props.xfrm.as_ref()?;
    let (ox, oy) = x.off?;
    let (cx, cy) = x.ext?;
    Some(Frame {
        x: ox as f32 / EMU_PER_PT,
        y: oy as f32 / EMU_PER_PT,
        w: cx as f32 / EMU_PER_PT,
        h: cy as f32 / EMU_PER_PT,
    })
}

fn ph_kind(s: &Shape) -> Option<&str> {
    s.nv.placeholder
        .as_ref()
        .map(|p| p.kind.as_deref().unwrap_or("obj"))
}

fn shapes(nodes: &[ShapeNode], out: &mut Vec<Shape>) {
    for n in nodes {
        match n {
            ShapeNode::Shape(s) => out.push((**s).clone()),
            ShapeNode::Group(g) => shapes(&g.children, out),
            _ => {}
        }
    }
}

/// Boxes of the `sldImg` / `body` placeholders of a notes slide that set their own `xfrm`.
pub fn notes_boxes(n: &Notes) -> NotesBoxes {
    let mut list = Vec::new();
    shapes(&n.data.shapes, &mut list);
    let mut b = NotesBoxes::default();
    for s in &list {
        match ph_kind(s) {
            Some("sldImg") => b.slide_image = b.slide_image.or_else(|| shape_frame(s)),
            Some("body") => b.body = b.body.or_else(|| shape_frame(s)),
            _ => {}
        }
    }
    b
}

fn hf_placeholder(kind: HfKind, s: &Shape) -> Option<HfPlaceholder> {
    let frame = shape_frame(s)?;
    let mut parts = Vec::new();
    let mut align = None;
    let mut size_pt = None;
    if let Some(t) = &s.text {
        for (i, p) in t.paragraphs.iter().enumerate() {
            if i > 0 {
                parts.push(Some(" ".to_owned()));
            }
            if align.is_none() {
                align = p.props.as_ref().and_then(|pp| match pp.algn.as_deref() {
                    Some("ctr") => Some(Align::Center),
                    Some("r") => Some(Align::Right),
                    Some("l" | "just") => Some(Align::Left),
                    _ => None,
                });
            }
            for r in &p.runs {
                match r {
                    TextRun::Run { text, props } => {
                        if size_pt.is_none() {
                            size_pt = props.sz.map(|sz| sz as f32 / 100.0);
                        }
                        parts.push(Some(text.clone()));
                    }
                    TextRun::Field {
                        kind: k,
                        text,
                        props,
                        ..
                    } => {
                        if size_pt.is_none() {
                            size_pt = props.sz.map(|sz| sz as f32 / 100.0);
                        }
                        if k.as_deref() == Some("slidenum") {
                            parts.push(None);
                        } else {
                            parts.push(Some(text.clone()));
                        }
                    }
                    TextRun::Break { .. } => parts.push(Some(" ".to_owned())),
                    TextRun::Other(_) => {}
                }
            }
        }
    }
    Some(HfPlaceholder {
        kind,
        frame,
        align,
        size_pt,
        parts,
    })
}

/// Parse a master part's root element.
fn master_info(root: &XmlElement) -> MasterInfo {
    let mut info = MasterInfo {
        flags: [true; 4],
        ..MasterInfo::default()
    };
    // §19.3.1.25 hf: attributes hdr, ftr, dt, sldNum (xsd:boolean, default true).
    if let Some(hf) = root.elements().find(|e| e.local() == "hf") {
        for (i, k) in HfKind::ALL.iter().enumerate() {
            if let Some(v) = hf.attr(k.ph()) {
                info.flags[i] = !matches!(v, "0" | "false");
            }
        }
    }
    // §19.3.1.16 cSld / §19.3.1.45 spTree.
    let tree = root
        .elements()
        .find(|e| e.local() == "cSld")
        .and_then(|c| c.elements().find(|e| e.local() == "spTree"));
    if let Some(tree) = tree {
        let nodes: Vec<ShapeNode> = tree
            .elements()
            .filter(|e| matches!(e.local(), "sp" | "grpSp"))
            .map(bo_pptx::parse_node)
            .collect();
        let mut list = Vec::new();
        shapes(&nodes, &mut list);
        for s in &list {
            let kind = match ph_kind(s) {
                Some("hdr") => Some(HfKind::Header),
                Some("ftr") => Some(HfKind::Footer),
                Some("dt") => Some(HfKind::Date),
                Some("sldNum") => Some(HfKind::SlideNumber),
                Some("sldImg") => {
                    info.slide_image = info.slide_image.or_else(|| shape_frame(s));
                    None
                }
                Some("body") => {
                    info.body = info.body.or_else(|| shape_frame(s));
                    None
                }
                _ => None,
            };
            if let Some(p) = kind.and_then(|k| hf_placeholder(k, s)) {
                info.placeholders.push(p);
            }
        }
    }
    // §19.3.1.6 clrMap.
    info.clr_map = root
        .elements()
        .find(|e| e.local() == "clrMap")
        .map(|e| e.attrs.clone())
        .unwrap_or_default();
    info.notes_style = root
        .elements()
        .find(|e| e.local() == "notesStyle")
        .map(bo_drawingml::text::parse_list_style);
    // §19.3.1.28 notesStyle / §21.1.2.4.13 lvl1pPr / defRPr@sz.
    info.notes_size_pt = root
        .elements()
        .find(|e| e.local() == "notesStyle")
        .and_then(|s| s.elements().find(|e| e.local() == "lvl1pPr"))
        .and_then(|l| l.elements().find(|e| e.local() == "defRPr"))
        .and_then(|r| r.attr("sz"))
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| v / 100.0);
    info
}

/// First part whose name starts with `prefix` (e.g. `/ppt/handoutMasters/`), parsed.
fn master_part(pres: &Presentation, prefix: &str) -> Option<MasterInfo> {
    let part = pres.parts.iter().find(|p| {
        let n = p.name.to_ascii_lowercase();
        n.starts_with(prefix) && n.ends_with(".xml") && !n.contains("/_rels/")
    })?;
    let root = bo_drawingml::dom::parse(&part.data, &part.name).ok()?;
    Some(master_info(&root))
}

/// The handout master (§19.3.1.24), when the deck has one.
pub fn handout_master(pres: &Presentation) -> Option<MasterInfo> {
    master_part(pres, "/ppt/handoutmasters/")
}

/// The notes master (§19.3.1.27), when the deck has one.
pub fn notes_master(pres: &Presentation) -> Option<MasterInfo> {
    master_part(pres, "/ppt/notesmasters/")
}
