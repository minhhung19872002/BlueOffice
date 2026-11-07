//! Presentation model (BlueSlide).
//!
//! Mirrors ECMA-376 Part 1, §19 (PresentationML) and the DrawingML subset of §20.1
//! used by slides. Values are stored in spec units: coordinates in EMU
//! (§20.1.10.16 `ST_Coordinate`), angles in 60 000ths of a degree
//! (§20.1.10.3 `ST_Angle`), percentages in 1/1000 % (§20.1.10.40 `ST_Percentage`),
//! font sizes in 1/100 pt (§20.1.10.72 `ST_TextFontSize`).
//!
//! Everything BlueOffice does not interpret is kept as opaque XML
//! ([`XmlElement`] inside [`Extra`]) together with a sort key that lets the
//! writer put it back at the same position among the modelled children
//! (prompt §5.1 "lossless round-trip").

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Opaque XML
// ---------------------------------------------------------------------------

/// A raw XML element kept verbatim (qualified names with their original prefixes).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct XmlElement {
    /// Qualified name, e.g. `p14:creationId`.
    pub name: String,
    /// Attributes in document order (qualified names, unescaped values),
    /// including any `xmlns` declarations.
    pub attrs: Vec<(String, String)>,
    /// Child nodes.
    pub children: Vec<XmlNode>,
}

/// Node of an opaque XML subtree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
}

impl XmlElement {
    /// New element without attributes or children.
    pub fn new(name: &str) -> Self {
        XmlElement {
            name: name.to_owned(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }
    /// Local part of the qualified name.
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }
    /// Attribute value by qualified name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    /// Child elements.
    pub fn elements(&self) -> impl Iterator<Item = &XmlElement> {
        self.children.iter().filter_map(|c| match c {
            XmlNode::Element(e) => Some(e),
            XmlNode::Text(_) => None,
        })
    }
    /// First child element with qualified name `name`.
    pub fn child(&self, name: &str) -> Option<&XmlElement> {
        self.elements().find(|e| e.name == name)
    }
    /// Concatenated text content of direct text children.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|c| match c {
                XmlNode::Text(t) => Some(t.as_str()),
                XmlNode::Element(_) => None,
            })
            .collect()
    }
}

/// An unmodelled child element plus its position key among the modelled
/// children of the parent (assigned by the reader from the schema sequence).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extra {
    pub key: u16,
    pub xml: XmlElement,
}

/// Unmodelled attributes, in document order.
pub type Attrs = Vec<(String, String)>;

/// A relationship of a part (ECMA-376 Part 2, §9.3).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rel {
    pub id: String,
    pub rel_type: String,
    /// Target as written in the file (relative, or absolute URI when external).
    pub target: String,
    pub external: bool,
}

/// A package part BlueOffice does not model, kept byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPart {
    pub name: String,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Colours — ECMA-376 Part 1, §20.1.2.3
// ---------------------------------------------------------------------------

/// Base colour model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColorKind {
    /// §20.1.2.3.32 srgbClr, `0xRRGGBB`.
    Srgb(u32),
    /// §20.1.2.3.30 scrgbClr, linear r,g,b in 1/1000 %.
    ScRgb(i32, i32, i32),
    /// §20.1.2.3.13 hslClr: hue (60000ths °), sat, lum (1/1000 %).
    Hsl(i32, i32, i32),
    /// §20.1.2.3.29 schemeClr (`accent1`, `tx1`, `phClr`, …).
    Scheme(String),
    /// §20.1.2.3.33 sysClr with optional `lastClr`.
    System { val: String, last: Option<u32> },
    /// §20.1.2.3.22 prstClr.
    Preset(String),
}

/// A colour transform child (`lumMod`, `alpha`, `tint`, …) with its `val`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorMod {
    /// Local element name, e.g. `lumMod`.
    pub name: String,
    pub val: Option<i32>,
}

/// DrawingML colour = base + ordered transforms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub kind: ColorKind,
    pub mods: Vec<ColorMod>,
}

impl Color {
    /// Plain sRGB colour.
    pub fn srgb(rgb: u32) -> Self {
        Color {
            kind: ColorKind::Srgb(rgb),
            mods: Vec::new(),
        }
    }
    /// Scheme colour without transforms.
    pub fn scheme(name: &str) -> Self {
        Color {
            kind: ColorKind::Scheme(name.to_owned()),
            mods: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Fills, lines, effects — §20.1.8
// ---------------------------------------------------------------------------

/// Gradient stop (§20.1.8.36 gs): `pos` in 1/1000 %.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub pos: i32,
    pub color: Color,
}

/// Gradient shading (§20.1.8.41 lin / §20.1.8.46 path).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GradientShade {
    Linear {
        ang: i32,
        scaled: Option<bool>,
    },
    /// `path` = `circle` | `rect` | `shape`; fill-to rect l,t,r,b in 1/1000 %.
    Path {
        path: String,
        fill_to: Option<[i32; 4]>,
    },
}

/// §20.1.8.33 gradFill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GradientFill {
    pub stops: Vec<GradientStop>,
    pub shade: Option<GradientShade>,
    pub rot_with_shape: Option<bool>,
    pub flip: Option<String>,
    pub extra: Vec<Extra>,
}

/// §20.1.8.47 pattFill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PatternFill {
    pub preset: Option<String>,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

/// §20.1.8.14 blipFill (pictures and picture fills).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BlipFill {
    /// Relationship id of the image (`r:embed`).
    pub embed: Option<String>,
    /// Relationship id of a linked image (`r:link`).
    pub link: Option<String>,
    /// Children of `a:blip` (alphaModFix, extLst, …) kept opaque.
    pub blip_children: Vec<XmlElement>,
    pub blip_attrs: Attrs,
    /// §20.1.8.55 srcRect l,t,r,b in 1/1000 % (crop).
    pub src_rect: Option<[i32; 4]>,
    /// §20.1.8.56 stretch/fillRect present.
    pub stretch: bool,
    /// Raw `a:tile` when tiling.
    pub tile: Option<XmlElement>,
    pub dpi: Option<String>,
    pub rot_with_shape: Option<bool>,
}

/// Fill (EG_FillProperties, §20.1.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
    None,
    Solid(Color),
    Gradient(GradientFill),
    Pattern(PatternFill),
    Picture(BlipFill),
    Group,
}

/// Arrow end (§20.1.8.38 headEnd / §20.1.8.57 tailEnd).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LineEnd {
    pub kind: Option<String>,
    pub w: Option<String>,
    pub len: Option<String>,
}

/// §20.1.2.2.24 ln.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Line {
    /// Width in EMU.
    pub w: Option<i64>,
    pub cap: Option<String>,
    pub cmpd: Option<String>,
    pub algn: Option<String>,
    pub fill: Option<Fill>,
    /// §20.1.8.48 prstDash.
    pub dash: Option<String>,
    /// `round` | `bevel` | `miter`.
    pub join: Option<String>,
    pub head: Option<LineEnd>,
    pub tail: Option<LineEnd>,
    pub extra: Vec<Extra>,
}

/// §20.1.8.49 outerShdw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OuterShadow {
    pub blur_rad: Option<i64>,
    pub dist: Option<i64>,
    pub dir: Option<i32>,
    pub color: Option<Color>,
    pub attrs: Attrs,
}

/// Effect inside `a:effectLst` (§20.1.8.26).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    OuterShadow(OuterShadow),
    Other(XmlElement),
}

// ---------------------------------------------------------------------------
// Geometry — §20.1.9
// ---------------------------------------------------------------------------

/// Guide (§20.1.9.11 gd): name + formula text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guide {
    pub name: String,
    pub fmla: String,
}

/// Path command of a custom geometry (§20.1.9.15 path children). Points are
/// guide names or integer literals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathCommand {
    MoveTo(String, String),
    LineTo(String, String),
    ArcTo {
        w_r: String,
        h_r: String,
        st_ang: String,
        sw_ang: String,
    },
    QuadTo([String; 4]),
    CubicTo([String; 6]),
    Close,
}

/// §20.1.9.15 path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GeomPath {
    pub w: Option<i64>,
    pub h: Option<i64>,
    /// `none` | `norm` | `lighten` | `lightenLess` | `darken` | `darkenLess`.
    pub fill: Option<String>,
    pub stroke: Option<bool>,
    pub extrusion_ok: Option<bool>,
    pub commands: Vec<PathCommand>,
}

/// §20.1.9.8 custGeom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomGeometry {
    pub av: Vec<Guide>,
    pub gd: Vec<Guide>,
    /// Text rectangle l,t,r,b (guide refs).
    pub rect: Option<[String; 4]>,
    pub paths: Vec<GeomPath>,
    /// ahLst / cxnLst kept opaque.
    pub extra: Vec<Extra>,
}

/// Shape geometry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Geometry {
    /// §20.1.9.18 prstGeom with adjust values (avLst).
    Preset {
        name: String,
        av: Vec<Guide>,
    },
    Custom(CustomGeometry),
}

/// §20.1.7.6 xfrm (and §20.1.7.5 group xfrm with chOff/chExt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Xfrm {
    pub off: Option<(i64, i64)>,
    pub ext: Option<(i64, i64)>,
    pub ch_off: Option<(i64, i64)>,
    pub ch_ext: Option<(i64, i64)>,
    pub rot: Option<i32>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
}

/// §20.1.2.2.35 spPr (and grpSpPr).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShapeProps {
    pub xfrm: Option<Xfrm>,
    pub geometry: Option<Geometry>,
    pub fill: Option<Fill>,
    pub line: Option<Line>,
    /// `None` = no effectLst element.
    pub effects: Option<Vec<Effect>>,
    pub bw_mode: Option<String>,
    pub extra: Vec<Extra>,
}

/// Style matrix reference (§20.1.4.2.* lnRef/fillRef/effectRef).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleRef {
    pub idx: u32,
    pub color: Option<Color>,
}

/// §20.1.4.2.10 fontRef.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontRef {
    /// `major` | `minor` | `none`.
    pub idx: String,
    pub color: Option<Color>,
}

/// §19.3.1.46 style (shape style).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeStyle {
    pub ln_ref: StyleRef,
    pub fill_ref: StyleRef,
    pub effect_ref: StyleRef,
    pub font_ref: FontRef,
}

// ---------------------------------------------------------------------------
// Text — §21.1.2
// ---------------------------------------------------------------------------

/// Autofit (§21.1.2.1.1 / .2 / .3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Autofit {
    None,
    /// normAutofit: fontScale and lnSpcReduction in 1/1000 %.
    Normal {
        font_scale: Option<i32>,
        ln_spc_reduction: Option<i32>,
    },
    Shape,
}

/// §21.1.2.1.1 bodyPr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BodyProps {
    pub l_ins: Option<i64>,
    pub t_ins: Option<i64>,
    pub r_ins: Option<i64>,
    pub b_ins: Option<i64>,
    /// `t` | `ctr` | `b` | `just` | `dist`.
    pub anchor: Option<String>,
    pub anchor_ctr: Option<bool>,
    /// `square` | `none`.
    pub wrap: Option<String>,
    pub num_col: Option<i32>,
    pub spc_col: Option<i64>,
    pub rot: Option<i32>,
    pub vert: Option<String>,
    pub autofit: Option<Autofit>,
    pub attrs: Attrs,
    pub extra: Vec<Extra>,
}

/// Spacing (§21.1.2.2.10 lnSpc etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spacing {
    /// spcPct, 1/1000 %.
    Percent(i32),
    /// spcPts, 1/100 pt.
    Points(i32),
}

/// Bullet kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bullet {
    /// §21.1.2.4.8 buNone.
    None,
    /// §21.1.2.4.3 buChar.
    Char(String),
    /// §21.1.2.4.1 buAutoNum.
    AutoNum {
        scheme: String,
        start_at: Option<i32>,
    },
    /// §21.1.2.4.2 buBlip (kept opaque).
    Blip(XmlElement),
}

/// Text font (§21.1.2.3.7 latin etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TextFont {
    pub typeface: String,
    pub attrs: Attrs,
}

/// §21.1.2.3.9 rPr / defRPr / endParaRPr.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RunProps {
    pub lang: Option<String>,
    /// Size in 1/100 pt.
    pub sz: Option<i32>,
    pub b: Option<bool>,
    pub i: Option<bool>,
    pub u: Option<String>,
    pub strike: Option<String>,
    /// Baseline shift, 1/1000 %.
    pub baseline: Option<i32>,
    /// Character spacing, 1/100 pt.
    pub spc: Option<i32>,
    pub cap: Option<String>,
    pub line: Option<Line>,
    pub fill: Option<Fill>,
    pub highlight: Option<Color>,
    pub latin: Option<TextFont>,
    pub ea: Option<TextFont>,
    pub cs: Option<TextFont>,
    pub sym: Option<TextFont>,
    pub attrs: Attrs,
    pub extra: Vec<Extra>,
}

/// Tab stop (§21.1.2.2.13 tab).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabStop {
    pub pos: Option<i64>,
    pub algn: Option<String>,
}

/// §21.1.2.2.7 pPr and list-style levels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ParaProps {
    pub lvl: Option<u8>,
    pub mar_l: Option<i64>,
    pub mar_r: Option<i64>,
    pub indent: Option<i64>,
    /// `l` | `ctr` | `r` | `just` | `dist` | …
    pub algn: Option<String>,
    pub def_tab_sz: Option<i64>,
    pub rtl: Option<bool>,
    pub ln_spc: Option<Spacing>,
    pub spc_bef: Option<Spacing>,
    pub spc_aft: Option<Spacing>,
    /// `None` = buClrTx; `Some` = buClr.
    pub bu_clr: Option<Option<Color>>,
    /// `None` = buSzTx; `Some(Percent|Points)`.
    pub bu_sz: Option<Option<Spacing>>,
    /// `None` = buFontTx; `Some` = buFont.
    pub bu_font: Option<Option<TextFont>>,
    pub bullet: Option<Bullet>,
    pub tabs: Option<Vec<TabStop>>,
    pub def_rpr: Option<RunProps>,
    pub attrs: Attrs,
    pub extra: Vec<Extra>,
}

/// Content of a paragraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextRun {
    /// §21.1.2.3.8 r.
    Run {
        props: RunProps,
        text: String,
    },
    /// §21.1.2.2.1 br.
    Break {
        props: RunProps,
    },
    /// §21.1.2.2.4 fld (slide number, date, …).
    Field {
        id: String,
        kind: Option<String>,
        props: RunProps,
        ppr: Option<Box<ParaProps>>,
        text: String,
    },
    Other(XmlElement),
}

/// §21.1.2.2.6 p.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Paragraph {
    pub props: Option<ParaProps>,
    pub runs: Vec<TextRun>,
    pub end_props: Option<RunProps>,
}

impl Paragraph {
    /// Plain text of the paragraph (breaks as `\n`).
    pub fn text(&self) -> String {
        let mut s = String::new();
        for r in &self.runs {
            match r {
                TextRun::Run { text, .. } | TextRun::Field { text, .. } => s.push_str(text),
                TextRun::Break { .. } => s.push('\n'),
                TextRun::Other(_) => {}
            }
        }
        s
    }
}

/// §21.1.2.4.12 lstStyle (and master text styles).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ListStyle {
    pub def: Option<ParaProps>,
    /// lvl1pPr … lvl9pPr.
    pub levels: [Option<ParaProps>; 9],
    pub extra: Vec<Extra>,
}

/// §21.1.2.1.2 txBody.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextBody {
    pub body_pr: BodyProps,
    pub list_style: Option<ListStyle>,
    pub paragraphs: Vec<Paragraph>,
}

impl TextBody {
    /// Plain text (paragraphs joined by `\n`).
    pub fn text(&self) -> String {
        self.paragraphs
            .iter()
            .map(Paragraph::text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---------------------------------------------------------------------------
// Tables — §21.1.3
// ---------------------------------------------------------------------------

/// §21.1.3.17 tcPr.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CellProps {
    pub mar_l: Option<i64>,
    pub mar_r: Option<i64>,
    pub mar_t: Option<i64>,
    pub mar_b: Option<i64>,
    pub anchor: Option<String>,
    pub ln_l: Option<Line>,
    pub ln_r: Option<Line>,
    pub ln_t: Option<Line>,
    pub ln_b: Option<Line>,
    pub fill: Option<Fill>,
    pub attrs: Attrs,
    pub extra: Vec<Extra>,
}

/// §21.1.3.16 tc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TableCell {
    pub grid_span: Option<u32>,
    pub row_span: Option<u32>,
    pub h_merge: Option<bool>,
    pub v_merge: Option<bool>,
    pub text: Option<TextBody>,
    pub props: Option<CellProps>,
    pub attrs: Attrs,
    pub extra: Vec<Extra>,
}

/// §21.1.3.18 tr.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TableRow {
    pub h: i64,
    pub cells: Vec<TableCell>,
    pub extra: Vec<Extra>,
}

/// §21.1.3.2 gridCol: width in EMU plus opaque children (extLst).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GridCol {
    pub w: i64,
    pub extra: Vec<XmlElement>,
}

/// §21.1.3.13 tbl.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Table {
    /// tblPr kept opaque (style id, banding flags).
    pub props: Option<XmlElement>,
    /// Grid columns (§21.1.3.2 gridCol).
    pub grid: Vec<GridCol>,
    pub rows: Vec<TableRow>,
    pub extra: Vec<Extra>,
}

// ---------------------------------------------------------------------------
// Shape tree — §19.3.1
// ---------------------------------------------------------------------------

/// §19.3.1.36 ph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Placeholder {
    /// ST_PlaceholderType (`title`, `body`, `ctrTitle`, …); absent means `obj`.
    pub kind: Option<String>,
    pub idx: Option<u32>,
    pub orient: Option<String>,
    pub sz: Option<String>,
    pub has_custom_prompt: Option<bool>,
    pub extra: Vec<Extra>,
}

/// Non-visual properties (`nvSpPr` and siblings, §19.3.1.34 etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NonVisual {
    /// §20.1.2.2.8 cNvPr id.
    pub id: u32,
    pub name: String,
    pub descr: Option<String>,
    pub hidden: Option<bool>,
    pub cnv_attrs: Attrs,
    pub cnv_extra: Vec<Extra>,
    /// `cNvSpPr` / `cNvPicPr` / … kept opaque.
    pub drawing_props: Option<XmlElement>,
    pub placeholder: Option<Placeholder>,
    /// nvPr children other than ph (media, extLst, …).
    pub nv_extra: Vec<Extra>,
    pub nv_attrs: Attrs,
}

/// §19.3.1.43 sp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Shape {
    pub nv: NonVisual,
    pub props: ShapeProps,
    pub style: Option<ShapeStyle>,
    pub text: Option<TextBody>,
    pub attrs: Attrs,
    pub extra: Vec<Extra>,
}

/// §19.3.1.37 pic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Picture {
    pub nv: NonVisual,
    pub blip: BlipFill,
    pub props: ShapeProps,
    pub style: Option<ShapeStyle>,
    pub extra: Vec<Extra>,
}

/// §19.3.1.22 grpSp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GroupShape {
    pub nv: NonVisual,
    pub props: ShapeProps,
    pub children: Vec<ShapeNode>,
    pub extra: Vec<Extra>,
}

/// §19.3.1.19 cxnSp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Connector {
    pub nv: NonVisual,
    pub props: ShapeProps,
    pub style: Option<ShapeStyle>,
    pub extra: Vec<Extra>,
}

/// Content of a graphic frame (§20.1.2.2.17 graphicData).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GraphicContent {
    Table(Table),
    /// Charts, diagrams, OLE objects: `graphicData` kept opaque.
    Other(XmlElement),
}

/// §19.3.1.21 graphicFrame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphicFrame {
    pub nv: NonVisual,
    pub xfrm: Xfrm,
    /// `graphicData@uri`.
    pub uri: String,
    pub content: GraphicContent,
    pub extra: Vec<Extra>,
}

/// Element of a shape tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ShapeNode {
    Shape(Box<Shape>),
    Picture(Box<Picture>),
    Group(Box<GroupShape>),
    Connector(Box<Connector>),
    Frame(Box<GraphicFrame>),
    /// mc:AlternateContent, contentPart, …
    Other(XmlElement),
}

impl ShapeNode {
    /// Non-visual properties when the node is a known shape kind.
    pub fn nv(&self) -> Option<&NonVisual> {
        match self {
            ShapeNode::Shape(s) => Some(&s.nv),
            ShapeNode::Picture(p) => Some(&p.nv),
            ShapeNode::Group(g) => Some(&g.nv),
            ShapeNode::Connector(c) => Some(&c.nv),
            ShapeNode::Frame(f) => Some(&f.nv),
            ShapeNode::Other(_) => None,
        }
    }
}

/// §19.3.1.1 bg.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Background {
    /// bgPr: fill (+ opaque effects).
    Props {
        fill: Fill,
        shade_to_title: Option<bool>,
        extra: Vec<Extra>,
    },
    /// bgRef: index into the theme's background fill style list.
    Ref { idx: u32, color: Option<Color> },
}

/// §19.3.1.16 cSld.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CommonSlideData {
    pub name: Option<String>,
    pub background: Option<Background>,
    /// nvGrpSpPr + grpSpPr of the spTree.
    pub tree_nv: NonVisual,
    pub tree_props: ShapeProps,
    pub shapes: Vec<ShapeNode>,
    pub tree_extra: Vec<Extra>,
    pub extra: Vec<Extra>,
}

/// §19.3.1.6 clrMap / overrides: attribute pairs (`bg1`→`lt1`, …).
pub type ColorMap = Vec<(String, String)>;

/// §19.3.1.7 clrMapOvr content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMapOverride {
    /// §20.1.6.6 masterClrMapping: use the master's mapping.
    Master,
    /// §20.1.6.8 overrideClrMapping.
    Override(ColorMap),
}

/// Data common to every slide-like part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PartInfo {
    /// Absolute part name, e.g. `/ppt/slides/slide1.xml` (empty for new parts).
    pub part_name: String,
    /// Relationships of the part as read (images, hyperlinks, …).
    pub rels: Vec<Rel>,
    /// Root attributes (namespace declarations, `mc:Ignorable`, unmodelled attributes).
    pub root_attrs: Attrs,
}

/// §19.3.1.38 sld.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Slide {
    pub info: PartInfo,
    pub data: CommonSlideData,
    /// Index into [`Presentation::layouts`].
    pub layout: usize,
    /// `None` = no clrMapOvr element.
    pub clr_map_ovr: Option<ColorMapOverride>,
    pub show: Option<bool>,
    /// Transitions, timing, extLst… kept opaque.
    pub extra: Vec<Extra>,
    pub notes: Option<Notes>,
    /// `p:sldId@id` in presentation.xml.
    pub slide_id: u32,
    /// Relationship id from presentation.xml (kept stable on save when possible).
    pub pres_rel_id: String,
}

/// §19.3.1.26 notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Notes {
    pub info: PartInfo,
    pub data: CommonSlideData,
    pub clr_map_ovr: Option<ColorMapOverride>,
    pub extra: Vec<Extra>,
}

impl Notes {
    /// Text of the notes body placeholder(s).
    pub fn text(&self) -> String {
        let mut out = Vec::new();
        for n in &self.data.shapes {
            if let ShapeNode::Shape(s) = n {
                let is_body =
                    s.nv.placeholder
                        .as_ref()
                        .is_some_and(|p| p.kind.as_deref() == Some("body"));
                if is_body {
                    if let Some(t) = &s.text {
                        out.push(t.text());
                    }
                }
            }
        }
        out.join("\n")
    }
}

/// §19.3.1.39 sldLayout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SlideLayout {
    pub info: PartInfo,
    pub data: CommonSlideData,
    /// Index into [`Presentation::masters`].
    pub master: usize,
    /// ST_SlideLayoutType.
    pub kind: Option<String>,
    pub clr_map_ovr: Option<ColorMapOverride>,
    pub show_master_sp: Option<bool>,
    pub extra: Vec<Extra>,
}

/// §19.3.1.52 txStyles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextStyles {
    pub title: Option<ListStyle>,
    pub body: Option<ListStyle>,
    pub other: Option<ListStyle>,
    pub extra: Vec<Extra>,
}

/// §19.3.1.41 sldLayoutId: a layout owned by a master.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LayoutRef {
    /// Index into [`Presentation::layouts`].
    pub layout: usize,
    /// `p:sldLayoutId@id`.
    pub id: u32,
    /// Relationship id from the master part.
    pub rel_id: String,
}

/// §19.3.1.42 sldMaster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SlideMaster {
    pub info: PartInfo,
    pub data: CommonSlideData,
    pub clr_map: ColorMap,
    /// Layouts of this master in sldLayoutIdLst order.
    pub layouts: Vec<LayoutRef>,
    /// Index into [`Presentation::themes`].
    pub theme: usize,
    pub text_styles: TextStyles,
    pub extra: Vec<Extra>,
    /// `p:sldMasterId@id` in presentation.xml.
    pub master_id: u32,
    /// Relationship id from presentation.xml.
    pub pres_rel_id: String,
}

// ---------------------------------------------------------------------------
// Theme — §20.1.6
// ---------------------------------------------------------------------------

/// §20.1.6.2 clrScheme: (dk1, lt1, dk2, lt2, accent1..6, hlink, folHlink) in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ColorScheme {
    pub name: String,
    pub colors: Vec<(String, Color)>,
    pub extra: Vec<Extra>,
}

/// §20.1.4.1.24 majorFont / minorFont.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FontCollection {
    pub latin: TextFont,
    pub ea: TextFont,
    pub cs: TextFont,
    /// Script-specific fonts (`a:font script="Jpan" typeface="…"`).
    pub scripts: Vec<(String, String)>,
    pub extra: Vec<Extra>,
}

/// §20.1.4.1.18 fontScheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FontScheme {
    pub name: String,
    pub major: FontCollection,
    pub minor: FontCollection,
    pub extra: Vec<Extra>,
}

/// §20.1.4.1.14 fmtScheme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FormatScheme {
    pub name: Option<String>,
    pub fill_styles: Vec<Fill>,
    pub line_styles: Vec<Line>,
    /// Effect styles: outer shadow (if any) of each `effectStyle`, plus the raw element.
    pub effect_styles: Vec<XmlElement>,
    pub bg_fill_styles: Vec<Fill>,
}

/// §20.1.6.9 theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Theme {
    pub info: PartInfo,
    pub name: Option<String>,
    pub colors: ColorScheme,
    pub fonts: FontScheme,
    pub formats: FormatScheme,
    /// themeElements extras (extLst) — keys relative to themeElements.
    pub elements_extra: Vec<Extra>,
    /// objectDefaults, extraClrSchemeLst, custClrLst, extLst.
    pub extra: Vec<Extra>,
}

impl Theme {
    /// Look up a scheme colour slot (`dk1`, `accent1`, …).
    pub fn scheme_color(&self, slot: &str) -> Option<&Color> {
        self.colors
            .colors
            .iter()
            .find(|(k, _)| k == slot)
            .map(|(_, c)| c)
    }
}

// ---------------------------------------------------------------------------
// Presentation — §19.2.1.26
// ---------------------------------------------------------------------------

/// Slide size (§19.2.1.39 sldSz) in EMU.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SlideSize {
    pub cx: i64,
    pub cy: i64,
    pub kind: Option<String>,
}

/// A whole presentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Presentation {
    pub slide_size: SlideSize,
    /// §19.2.1.23 notesSz.
    pub notes_size: SlideSize,
    pub slides: Vec<Slide>,
    pub layouts: Vec<SlideLayout>,
    pub masters: Vec<SlideMaster>,
    pub themes: Vec<Theme>,
    /// §19.2.1.8 defaultTextStyle.
    pub default_text_style: Option<ListStyle>,
    /// presentation.xml part info (rels other than masters/slides are kept here).
    pub info: PartInfo,
    /// presentation.xml unmodelled children (notesMasterIdLst, extLst, …).
    pub extra: Vec<Extra>,
    /// Every other package part (media, notes master, docProps, presProps, …), verbatim.
    pub parts: Vec<RawPart>,
}

impl Presentation {
    /// Raw bytes of a preserved part (e.g. an image) by absolute part name.
    pub fn part_data(&self, name: &str) -> Option<&[u8]> {
        self.parts
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .map(|p| p.data.as_slice())
    }
}
