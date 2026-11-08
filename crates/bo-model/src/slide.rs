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
    /// Child element.
    Element(XmlElement),
    /// Text content (unescaped).
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
    /// Position key among the parent's modelled children (schema sequence order).
    pub key: u16,
    /// The element itself, verbatim.
    pub xml: XmlElement,
}

/// Unmodelled attributes, in document order.
pub type Attrs = Vec<(String, String)>;

/// A relationship of a part (ECMA-376 Part 2, §9.3).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rel {
    /// Relationship id (`Id`, e.g. `rId1`).
    pub id: String,
    /// Relationship type URI (`Type`).
    pub rel_type: String,
    /// Target as written in the file (relative, or absolute URI when external).
    pub target: String,
    /// `TargetMode="External"`.
    pub external: bool,
}

/// A package part BlueOffice does not model, kept byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPart {
    /// Absolute part name, e.g. `/ppt/media/image1.png`.
    pub name: String,
    /// Content type from `[Content_Types].xml`, if declared.
    pub content_type: Option<String>,
    /// Raw part bytes.
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
    System {
        /// System colour name (`val`, e.g. `windowText`).
        val: String,
        /// Last computed colour (`lastClr`), `0xRRGGBB`.
        last: Option<u32>,
    },
    /// §20.1.2.3.22 prstClr.
    Preset(String),
}

/// A colour transform child (`lumMod`, `alpha`, `tint`, …) with its `val`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorMod {
    /// Local element name, e.g. `lumMod`.
    pub name: String,
    /// `val` attribute (usually 1/1000 % or 60 000ths of a degree, by transform).
    pub val: Option<i32>,
}

/// DrawingML colour = base + ordered transforms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Color {
    /// Base colour.
    pub kind: ColorKind,
    /// Colour transforms in document order.
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
    /// Stop position (`pos`), 1/1000 %.
    pub pos: i32,
    /// Stop colour.
    pub color: Color,
}

/// Gradient shading (§20.1.8.41 lin / §20.1.8.46 path).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GradientShade {
    /// §20.1.8.41 lin: linear gradient.
    Linear {
        /// Gradient angle (`ang`), 60 000ths of a degree.
        ang: i32,
        /// `scaled` attribute.
        scaled: Option<bool>,
    },
    /// `path` = `circle` | `rect` | `shape`; fill-to rect l,t,r,b in 1/1000 %.
    Path {
        /// Path kind (`path` attribute).
        path: String,
        /// `fillToRect` l,t,r,b in 1/1000 %.
        fill_to: Option<[i32; 4]>,
    },
}

/// §20.1.8.33 gradFill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GradientFill {
    /// Gradient stops (`gsLst`).
    pub stops: Vec<GradientStop>,
    /// Linear or path shading.
    pub shade: Option<GradientShade>,
    /// `rotWithShape` attribute.
    pub rot_with_shape: Option<bool>,
    /// Tile flip (`flip`: `none` | `x` | `y` | `xy`).
    pub flip: Option<String>,
    /// Unmodelled children (tileRect, …).
    pub extra: Vec<Extra>,
}

/// §20.1.8.47 pattFill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PatternFill {
    /// Preset pattern (`prst`, e.g. `pct5`).
    pub preset: Option<String>,
    /// Foreground colour (`fgClr`).
    pub fg: Option<Color>,
    /// Background colour (`bgClr`).
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
    /// Unmodelled attributes of `a:blip` (e.g. `cstate`).
    pub blip_attrs: Attrs,
    /// §20.1.8.55 srcRect l,t,r,b in 1/1000 % (crop).
    pub src_rect: Option<[i32; 4]>,
    /// §20.1.8.56 stretch/fillRect present.
    pub stretch: bool,
    /// Raw `a:tile` when tiling.
    pub tile: Option<XmlElement>,
    /// `dpi` attribute.
    pub dpi: Option<String>,
    /// `rotWithShape` attribute.
    pub rot_with_shape: Option<bool>,
}

/// Fill (EG_FillProperties, §20.1.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Fill {
    /// §20.1.8.44 noFill.
    None,
    /// §20.1.8.54 solidFill.
    Solid(Color),
    /// §20.1.8.33 gradFill.
    Gradient(GradientFill),
    /// §20.1.8.47 pattFill.
    Pattern(PatternFill),
    /// §20.1.8.14 blipFill.
    Picture(BlipFill),
    /// §20.1.8.35 grpFill: inherit the group's fill.
    Group,
}

/// Arrow end (§20.1.8.38 headEnd / §20.1.8.57 tailEnd).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LineEnd {
    /// Arrow type (`type`: `triangle`, `arrow`, …).
    pub kind: Option<String>,
    /// Arrow width (`w`: `sm` | `med` | `lg`).
    pub w: Option<String>,
    /// Arrow length (`len`: `sm` | `med` | `lg`).
    pub len: Option<String>,
}

/// §20.1.2.2.24 ln.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Line {
    /// Width in EMU.
    pub w: Option<i64>,
    /// Line cap (`cap`: `rnd` | `sq` | `flat`).
    pub cap: Option<String>,
    /// Compound line type (`cmpd`: `sng`, `dbl`, …).
    pub cmpd: Option<String>,
    /// Pen alignment (`algn`: `ctr` | `in`).
    pub algn: Option<String>,
    /// Line fill.
    pub fill: Option<Fill>,
    /// §20.1.8.48 prstDash.
    pub dash: Option<String>,
    /// `round` | `bevel` | `miter`.
    pub join: Option<String>,
    /// Head arrow (`headEnd`).
    pub head: Option<LineEnd>,
    /// Tail arrow (`tailEnd`).
    pub tail: Option<LineEnd>,
    /// Unmodelled children (custDash, extLst, …).
    pub extra: Vec<Extra>,
}

/// §20.1.8.49 outerShdw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OuterShadow {
    /// Blur radius (`blurRad`), EMU.
    pub blur_rad: Option<i64>,
    /// Shadow distance (`dist`), EMU.
    pub dist: Option<i64>,
    /// Shadow direction (`dir`), 60 000ths of a degree.
    pub dir: Option<i32>,
    /// Shadow colour.
    pub color: Option<Color>,
    /// Unmodelled attributes (`algn`, `sx`, `rotWithShape`, …).
    pub attrs: Attrs,
}

/// §20.1.8.32 glow: a soft colour band `rad` EMU wide around the shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Glow {
    /// Glow radius (`rad`), EMU.
    pub rad: Option<i64>,
    /// Glow colour.
    pub color: Option<Color>,
    /// Unmodelled attributes.
    pub attrs: Attrs,
}

/// Effect inside `a:effectLst` (§20.1.8.26).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    /// §20.1.8.49 outerShdw.
    OuterShadow(OuterShadow),
    /// §20.1.8.32 glow.
    Glow(Glow),
    /// Any other effect, kept opaque.
    Other(XmlElement),
}

// ---------------------------------------------------------------------------
// Geometry — §20.1.9
// ---------------------------------------------------------------------------

/// Guide (§20.1.9.11 gd): name + formula text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guide {
    /// Guide name.
    pub name: String,
    /// Guide formula (`fmla`, e.g. `val 50000`).
    pub fmla: String,
}

/// Path command of a custom geometry (§20.1.9.15 path children). Points are
/// guide names or integer literals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathCommand {
    /// moveTo: x, y.
    MoveTo(String, String),
    /// lnTo: x, y.
    LineTo(String, String),
    /// arcTo.
    ArcTo {
        /// Width radius (`wR`).
        w_r: String,
        /// Height radius (`hR`).
        h_r: String,
        /// Start angle (`stAng`).
        st_ang: String,
        /// Swing angle (`swAng`).
        sw_ang: String,
    },
    /// quadBezTo: control point x,y then end x,y.
    QuadTo([String; 4]),
    /// cubicBezTo: two control points then end point (x,y each).
    CubicTo([String; 6]),
    /// close.
    Close,
}

/// §20.1.9.15 path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GeomPath {
    /// Path coordinate space width (`w`), EMU.
    pub w: Option<i64>,
    /// Path coordinate space height (`h`), EMU.
    pub h: Option<i64>,
    /// `none` | `norm` | `lighten` | `lightenLess` | `darken` | `darkenLess`.
    pub fill: Option<String>,
    /// `stroke` attribute.
    pub stroke: Option<bool>,
    /// `extrusionOk` attribute.
    pub extrusion_ok: Option<bool>,
    /// Path commands in order.
    pub commands: Vec<PathCommand>,
}

/// §20.1.9.8 custGeom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomGeometry {
    /// Adjust values (`avLst`).
    pub av: Vec<Guide>,
    /// Guides (`gdLst`).
    pub gd: Vec<Guide>,
    /// Text rectangle l,t,r,b (guide refs).
    pub rect: Option<[String; 4]>,
    /// Paths (`pathLst`).
    pub paths: Vec<GeomPath>,
    /// ahLst / cxnLst kept opaque.
    pub extra: Vec<Extra>,
}

/// Shape geometry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Geometry {
    /// §20.1.9.18 prstGeom with adjust values (avLst).
    Preset {
        /// Preset shape name (`prst`, e.g. `rect`).
        name: String,
        /// Adjust values (`avLst`).
        av: Vec<Guide>,
    },
    /// §20.1.9.8 custGeom.
    Custom(CustomGeometry),
}

/// §20.1.7.6 xfrm (and §20.1.7.5 group xfrm with chOff/chExt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Xfrm {
    /// Offset (`a:off` x, y), EMU.
    pub off: Option<(i64, i64)>,
    /// Extents (`a:ext` cx, cy), EMU.
    pub ext: Option<(i64, i64)>,
    /// Child offset (`a:chOff`, group only), EMU.
    pub ch_off: Option<(i64, i64)>,
    /// Child extents (`a:chExt`, group only), EMU.
    pub ch_ext: Option<(i64, i64)>,
    /// Rotation (`rot`), 60 000ths of a degree.
    pub rot: Option<i32>,
    /// Horizontal flip (`flipH`).
    pub flip_h: Option<bool>,
    /// Vertical flip (`flipV`).
    pub flip_v: Option<bool>,
}

/// §20.1.2.2.35 spPr (and grpSpPr).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ShapeProps {
    /// Transform (`a:xfrm`).
    pub xfrm: Option<Xfrm>,
    /// Geometry (prstGeom / custGeom).
    pub geometry: Option<Geometry>,
    /// Fill.
    pub fill: Option<Fill>,
    /// Outline (`a:ln`).
    pub line: Option<Line>,
    /// `None` = no effectLst element.
    pub effects: Option<Vec<Effect>>,
    /// Black-and-white mode (`bwMode`).
    pub bw_mode: Option<String>,
    /// Unmodelled children (scene3d, sp3d, extLst, …).
    pub extra: Vec<Extra>,
}

/// Style matrix reference (§20.1.4.2.* lnRef/fillRef/effectRef).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleRef {
    /// Style matrix index (`idx`, 1-based; 0 = none).
    pub idx: u32,
    /// Colour override.
    pub color: Option<Color>,
}

/// §20.1.4.2.10 fontRef.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontRef {
    /// `major` | `minor` | `none`.
    pub idx: String,
    /// Colour override.
    pub color: Option<Color>,
}

/// §19.3.1.46 style (shape style).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeStyle {
    /// Line reference (`lnRef`).
    pub ln_ref: StyleRef,
    /// Fill reference (`fillRef`).
    pub fill_ref: StyleRef,
    /// Effect reference (`effectRef`).
    pub effect_ref: StyleRef,
    /// Font reference (`fontRef`).
    pub font_ref: FontRef,
}

// ---------------------------------------------------------------------------
// Text — §21.1.2
// ---------------------------------------------------------------------------

/// Autofit (§21.1.2.1.1 / .2 / .3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Autofit {
    /// No autofit (`noAutofit`).
    None,
    /// normAutofit: fontScale and lnSpcReduction in 1/1000 %.
    Normal {
        /// Font scale (`fontScale`), 1/1000 %.
        font_scale: Option<i32>,
        /// Line spacing reduction (`lnSpcReduction`), 1/1000 %.
        ln_spc_reduction: Option<i32>,
    },
    /// Resize shape to fit text (`spAutoFit`).
    Shape,
}

/// §21.1.2.1.1 bodyPr.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BodyProps {
    /// Left inset (`lIns`), EMU.
    pub l_ins: Option<i64>,
    /// Top inset (`tIns`), EMU.
    pub t_ins: Option<i64>,
    /// Right inset (`rIns`), EMU.
    pub r_ins: Option<i64>,
    /// Bottom inset (`bIns`), EMU.
    pub b_ins: Option<i64>,
    /// `t` | `ctr` | `b` | `just` | `dist`.
    pub anchor: Option<String>,
    /// `anchorCtr` attribute.
    pub anchor_ctr: Option<bool>,
    /// `square` | `none`.
    pub wrap: Option<String>,
    /// Number of columns (`numCol`).
    pub num_col: Option<i32>,
    /// Column spacing (`spcCol`), EMU.
    pub spc_col: Option<i64>,
    /// Text rotation (`rot`), 60 000ths of a degree.
    pub rot: Option<i32>,
    /// Vertical text type (`vert`: `horz`, `vert`, `vert270`, …).
    pub vert: Option<String>,
    /// Autofit child.
    pub autofit: Option<Autofit>,
    /// Unmodelled attributes.
    pub attrs: Attrs,
    /// Unmodelled children (prstTxWarp, scene3d, …).
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
        /// Numbering scheme (`type`, e.g. `arabicPeriod`).
        scheme: String,
        /// Starting number (`startAt`).
        start_at: Option<i32>,
    },
    /// §21.1.2.4.2 buBlip (kept opaque).
    Blip(XmlElement),
}

/// Text font (§21.1.2.3.7 latin etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TextFont {
    /// Typeface name (`typeface`).
    pub typeface: String,
    /// Other attributes (`panose`, `pitchFamily`, `charset`).
    pub attrs: Attrs,
}

/// §21.1.2.3.9 rPr / defRPr / endParaRPr.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RunProps {
    /// Language tag (`lang`, e.g. `vi-VN`).
    pub lang: Option<String>,
    /// Size in 1/100 pt.
    pub sz: Option<i32>,
    /// Bold (`b`).
    pub b: Option<bool>,
    /// Italic (`i`).
    pub i: Option<bool>,
    /// Underline type (`u`, e.g. `sng`).
    pub u: Option<String>,
    /// Strikethrough (`strike`: `noStrike` | `sngStrike` | `dblStrike`).
    pub strike: Option<String>,
    /// Baseline shift, 1/1000 %.
    pub baseline: Option<i32>,
    /// Character spacing, 1/100 pt.
    pub spc: Option<i32>,
    /// Capitalisation (`cap`: `none` | `small` | `all`).
    pub cap: Option<String>,
    /// Text outline (`a:ln`).
    pub line: Option<Line>,
    /// Text fill.
    pub fill: Option<Fill>,
    /// Highlight colour (`a:highlight`).
    pub highlight: Option<Color>,
    /// Latin font (`a:latin`).
    pub latin: Option<TextFont>,
    /// East Asian font (`a:ea`).
    pub ea: Option<TextFont>,
    /// Complex script font (`a:cs`).
    pub cs: Option<TextFont>,
    /// Symbol font (`a:sym`).
    pub sym: Option<TextFont>,
    /// Unmodelled attributes.
    pub attrs: Attrs,
    /// Unmodelled children (hlinkClick, effectLst, …).
    pub extra: Vec<Extra>,
}

/// Tab stop (§21.1.2.2.13 tab).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabStop {
    /// Tab position (`pos`), EMU.
    pub pos: Option<i64>,
    /// Tab alignment (`algn`: `l` | `ctr` | `r` | `dec`).
    pub algn: Option<String>,
}

/// §21.1.2.2.7 pPr and list-style levels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ParaProps {
    /// Outline level (`lvl`), 0-based.
    pub lvl: Option<u8>,
    /// Left margin (`marL`), EMU.
    pub mar_l: Option<i64>,
    /// Right margin (`marR`), EMU.
    pub mar_r: Option<i64>,
    /// First-line indent (`indent`), EMU; negative for hanging.
    pub indent: Option<i64>,
    /// `l` | `ctr` | `r` | `just` | `dist` | …
    pub algn: Option<String>,
    /// Default tab size (`defTabSz`), EMU.
    pub def_tab_sz: Option<i64>,
    /// Right-to-left (`rtl`).
    pub rtl: Option<bool>,
    /// Line spacing (`lnSpc`).
    pub ln_spc: Option<Spacing>,
    /// Space before (`spcBef`).
    pub spc_bef: Option<Spacing>,
    /// Space after (`spcAft`).
    pub spc_aft: Option<Spacing>,
    /// `None` = buClrTx; `Some` = buClr.
    pub bu_clr: Option<Option<Color>>,
    /// `None` = buSzTx; `Some(Percent|Points)`.
    pub bu_sz: Option<Option<Spacing>>,
    /// `None` = buFontTx; `Some` = buFont.
    pub bu_font: Option<Option<TextFont>>,
    /// Bullet kind.
    pub bullet: Option<Bullet>,
    /// Tab stops (`tabLst`).
    pub tabs: Option<Vec<TabStop>>,
    /// Default run properties (`defRPr`).
    pub def_rpr: Option<RunProps>,
    /// Unmodelled attributes.
    pub attrs: Attrs,
    /// Unmodelled children.
    pub extra: Vec<Extra>,
}

/// Content of a paragraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextRun {
    /// §21.1.2.3.8 r.
    Run {
        /// Run properties (`rPr`).
        props: RunProps,
        /// Run text (`a:t`).
        text: String,
    },
    /// §21.1.2.2.1 br.
    Break {
        /// Properties of the break (`rPr`).
        props: RunProps,
    },
    /// §21.1.2.2.4 fld (slide number, date, …).
    Field {
        /// Field id (`id`, a GUID).
        id: String,
        /// Field type (`type`, e.g. `slidenum`, `datetime1`).
        kind: Option<String>,
        /// Run properties (`rPr`).
        props: RunProps,
        /// Paragraph properties inside the field (`pPr`).
        ppr: Option<Box<ParaProps>>,
        /// Cached field text (`a:t`).
        text: String,
    },
    /// Any other paragraph child, kept opaque.
    Other(XmlElement),
}

/// §21.1.2.2.6 p.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Paragraph {
    /// Paragraph properties (`pPr`).
    pub props: Option<ParaProps>,
    /// Runs, breaks and fields in order.
    pub runs: Vec<TextRun>,
    /// End-of-paragraph run properties (`endParaRPr`).
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
    /// Default level (`defPPr`).
    pub def: Option<ParaProps>,
    /// lvl1pPr … lvl9pPr.
    pub levels: [Option<ParaProps>; 9],
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §21.1.2.1.2 txBody.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextBody {
    /// Body properties (`bodyPr`).
    pub body_pr: BodyProps,
    /// List style (`lstStyle`).
    pub list_style: Option<ListStyle>,
    /// Paragraphs.
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
    /// Left margin (`marL`), EMU.
    pub mar_l: Option<i64>,
    /// Right margin (`marR`), EMU.
    pub mar_r: Option<i64>,
    /// Top margin (`marT`), EMU.
    pub mar_t: Option<i64>,
    /// Bottom margin (`marB`), EMU.
    pub mar_b: Option<i64>,
    /// Vertical anchor (`anchor`: `t` | `ctr` | `b`).
    pub anchor: Option<String>,
    /// Left border (`lnL`).
    pub ln_l: Option<Line>,
    /// Right border (`lnR`).
    pub ln_r: Option<Line>,
    /// Top border (`lnT`).
    pub ln_t: Option<Line>,
    /// Bottom border (`lnB`).
    pub ln_b: Option<Line>,
    /// Cell fill.
    pub fill: Option<Fill>,
    /// Unmodelled attributes.
    pub attrs: Attrs,
    /// Unmodelled children (lnTlToBr, lnBlToTr, cell3D, …).
    pub extra: Vec<Extra>,
}

/// §21.1.3.16 tc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TableCell {
    /// Columns spanned (`gridSpan`).
    pub grid_span: Option<u32>,
    /// Rows spanned (`rowSpan`).
    pub row_span: Option<u32>,
    /// Horizontally merged into the cell on the left (`hMerge`).
    pub h_merge: Option<bool>,
    /// Vertically merged into the cell above (`vMerge`).
    pub v_merge: Option<bool>,
    /// Cell text (`txBody`).
    pub text: Option<TextBody>,
    /// Cell properties (`tcPr`).
    pub props: Option<CellProps>,
    /// Unmodelled attributes.
    pub attrs: Attrs,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §21.1.3.18 tr.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TableRow {
    /// Row height (`h`), EMU.
    pub h: i64,
    /// Cells.
    pub cells: Vec<TableCell>,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §21.1.3.2 gridCol: width in EMU plus opaque children (extLst).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GridCol {
    /// Column width (`w`), EMU.
    pub w: i64,
    /// Unmodelled children (extLst).
    pub extra: Vec<XmlElement>,
}

/// §21.1.3.13 tbl.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Table {
    /// tblPr kept opaque (style id, banding flags).
    pub props: Option<XmlElement>,
    /// Grid columns (§21.1.3.2 gridCol).
    pub grid: Vec<GridCol>,
    /// Rows.
    pub rows: Vec<TableRow>,
    /// Unmodelled children.
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
    /// Placeholder index (`idx`) used to match layout/master placeholders.
    pub idx: Option<u32>,
    /// Orientation (`orient`: `horz` | `vert`).
    pub orient: Option<String>,
    /// Size (`sz`: `full` | `half` | `quarter`).
    pub sz: Option<String>,
    /// `hasCustomPrompt` attribute.
    pub has_custom_prompt: Option<bool>,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// Non-visual properties (`nvSpPr` and siblings, §19.3.1.34 etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NonVisual {
    /// §20.1.2.2.8 cNvPr id.
    pub id: u32,
    /// Shape name (`cNvPr@name`).
    pub name: String,
    /// Alternative text (`cNvPr@descr`).
    pub descr: Option<String>,
    /// Hidden (`cNvPr@hidden`).
    pub hidden: Option<bool>,
    /// Unmodelled attributes of `cNvPr` (`title`, …).
    pub cnv_attrs: Attrs,
    /// Unmodelled children of `cNvPr` (hlinkClick, extLst, …).
    pub cnv_extra: Vec<Extra>,
    /// `cNvSpPr` / `cNvPicPr` / … kept opaque.
    pub drawing_props: Option<XmlElement>,
    /// Placeholder (`nvPr/ph`).
    pub placeholder: Option<Placeholder>,
    /// nvPr children other than ph (media, extLst, …).
    pub nv_extra: Vec<Extra>,
    /// Unmodelled attributes of `nvPr` (`isPhoto`, `userDrawn`).
    pub nv_attrs: Attrs,
}

/// §19.3.1.43 sp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Shape {
    /// Non-visual properties (`nvSpPr`).
    pub nv: NonVisual,
    /// Shape properties (`spPr`).
    pub props: ShapeProps,
    /// Shape style (`p:style`).
    pub style: Option<ShapeStyle>,
    /// Text body (`txBody`).
    pub text: Option<TextBody>,
    /// Unmodelled attributes (`useBgFill`, …).
    pub attrs: Attrs,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §19.3.1.37 pic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Picture {
    /// Non-visual properties (`nvPicPr`).
    pub nv: NonVisual,
    /// Picture fill (`blipFill`).
    pub blip: BlipFill,
    /// Shape properties (`spPr`).
    pub props: ShapeProps,
    /// Shape style (`p:style`).
    pub style: Option<ShapeStyle>,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §19.3.1.22 grpSp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GroupShape {
    /// Non-visual properties (`nvGrpSpPr`).
    pub nv: NonVisual,
    /// Group shape properties (`grpSpPr`).
    pub props: ShapeProps,
    /// Child shapes in z-order.
    pub children: Vec<ShapeNode>,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §19.3.1.19 cxnSp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Connector {
    /// Non-visual properties (`nvCxnSpPr`).
    pub nv: NonVisual,
    /// Shape properties (`spPr`).
    pub props: ShapeProps,
    /// Shape style (`p:style`).
    pub style: Option<ShapeStyle>,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// Content of a graphic frame (§20.1.2.2.17 graphicData).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GraphicContent {
    /// §21.1.3.13 tbl.
    Table(Table),
    /// Charts, diagrams, OLE objects: `graphicData` kept opaque.
    Other(XmlElement),
}

/// §19.3.1.21 graphicFrame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphicFrame {
    /// Non-visual properties (`nvGraphicFramePr`).
    pub nv: NonVisual,
    /// Frame transform (`p:xfrm`), EMU.
    pub xfrm: Xfrm,
    /// `graphicData@uri`.
    pub uri: String,
    /// Frame content.
    pub content: GraphicContent,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// Element of a shape tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ShapeNode {
    /// §19.3.1.43 sp.
    Shape(Box<Shape>),
    /// §19.3.1.37 pic.
    Picture(Box<Picture>),
    /// §19.3.1.22 grpSp.
    Group(Box<GroupShape>),
    /// §19.3.1.19 cxnSp.
    Connector(Box<Connector>),
    /// §19.3.1.21 graphicFrame.
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
        /// Background fill.
        fill: Fill,
        /// `shadeToTitle` attribute.
        shade_to_title: Option<bool>,
        /// Unmodelled children (effectLst, …).
        extra: Vec<Extra>,
    },
    /// bgRef: index into the theme's background fill style list.
    Ref {
        /// Theme style index (`idx`); 1001+ select background fill styles.
        idx: u32,
        /// Colour substituted for `phClr`.
        color: Option<Color>,
    },
}

/// §19.3.1.16 cSld.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CommonSlideData {
    /// Slide name (`cSld@name`).
    pub name: Option<String>,
    /// Background (`p:bg`).
    pub background: Option<Background>,
    /// nvGrpSpPr + grpSpPr of the spTree.
    pub tree_nv: NonVisual,
    /// Group shape properties of the spTree (`grpSpPr`).
    pub tree_props: ShapeProps,
    /// Shapes of the spTree in z-order.
    pub shapes: Vec<ShapeNode>,
    /// Unmodelled children of the spTree (extLst).
    pub tree_extra: Vec<Extra>,
    /// Unmodelled children of `cSld` (custDataLst, controls, extLst).
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
    /// Part name, relationships and root attributes.
    pub info: PartInfo,
    /// Common slide data (`cSld`).
    pub data: CommonSlideData,
    /// Index into [`Presentation::layouts`].
    pub layout: usize,
    /// `None` = no clrMapOvr element.
    pub clr_map_ovr: Option<ColorMapOverride>,
    /// Shown in the slide show (`show`); `None` = default (true).
    pub show: Option<bool>,
    /// §19.3.1.50 transition.
    pub transition: Option<Transition>,
    /// §19.3.1.48 timing (main animation sequence).
    pub timing: Option<Timing>,
    /// extLst and other unmodelled children.
    pub extra: Vec<Extra>,
    /// Notes slide, if any.
    pub notes: Option<Notes>,
    /// `p:sldId@id` in presentation.xml.
    pub slide_id: u32,
    /// Relationship id from presentation.xml (kept stable on save when possible).
    pub pres_rel_id: String,
}

/// §19.3.1.26 notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Notes {
    /// Part name, relationships and root attributes.
    pub info: PartInfo,
    /// Common slide data (`cSld`).
    pub data: CommonSlideData,
    /// Colour map override (`clrMapOvr`).
    pub clr_map_ovr: Option<ColorMapOverride>,
    /// Unmodelled children (extLst).
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
    /// Part name, relationships and root attributes.
    pub info: PartInfo,
    /// Common slide data (`cSld`).
    pub data: CommonSlideData,
    /// Index into [`Presentation::masters`].
    pub master: usize,
    /// ST_SlideLayoutType.
    pub kind: Option<String>,
    /// Colour map override (`clrMapOvr`).
    pub clr_map_ovr: Option<ColorMapOverride>,
    /// Show master shapes (`showMasterSp`).
    pub show_master_sp: Option<bool>,
    /// Unmodelled children (transition, timing, hf, extLst, …).
    pub extra: Vec<Extra>,
}

/// §19.3.1.52 txStyles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TextStyles {
    /// Title text style (`titleStyle`).
    pub title: Option<ListStyle>,
    /// Body text style (`bodyStyle`).
    pub body: Option<ListStyle>,
    /// Other text style (`otherStyle`).
    pub other: Option<ListStyle>,
    /// Unmodelled children (extLst).
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
    /// Part name, relationships and root attributes.
    pub info: PartInfo,
    /// Common slide data (`cSld`).
    pub data: CommonSlideData,
    /// Colour map (`clrMap`).
    pub clr_map: ColorMap,
    /// Layouts of this master in sldLayoutIdLst order.
    pub layouts: Vec<LayoutRef>,
    /// Index into [`Presentation::themes`].
    pub theme: usize,
    /// Master text styles (`txStyles`).
    pub text_styles: TextStyles,
    /// Unmodelled children (transition, timing, hf, extLst, …).
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
    /// Scheme name (`name`).
    pub name: String,
    /// Colour slots `(slot name, colour)` in schema order.
    pub colors: Vec<(String, Color)>,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §20.1.4.1.24 majorFont / minorFont.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FontCollection {
    /// Latin font (`a:latin`).
    pub latin: TextFont,
    /// East Asian font (`a:ea`).
    pub ea: TextFont,
    /// Complex script font (`a:cs`).
    pub cs: TextFont,
    /// Script-specific fonts (`a:font script="Jpan" typeface="…"`).
    pub scripts: Vec<(String, String)>,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §20.1.4.1.18 fontScheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FontScheme {
    /// Font scheme name (`name`).
    pub name: String,
    /// Major (heading) fonts (`majorFont`).
    pub major: FontCollection,
    /// Minor (body) fonts (`minorFont`).
    pub minor: FontCollection,
    /// Unmodelled children (extLst).
    pub extra: Vec<Extra>,
}

/// §20.1.4.1.14 fmtScheme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FormatScheme {
    /// Format scheme name (`name`).
    pub name: Option<String>,
    /// Fill styles (`fillStyleLst`).
    pub fill_styles: Vec<Fill>,
    /// Line styles (`lnStyleLst`).
    pub line_styles: Vec<Line>,
    /// Effect styles: outer shadow (if any) of each `effectStyle`, plus the raw element.
    pub effect_styles: Vec<XmlElement>,
    /// Background fill styles (`bgFillStyleLst`).
    pub bg_fill_styles: Vec<Fill>,
}

/// §20.1.6.9 theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Theme {
    /// Part name, relationships and root attributes.
    pub info: PartInfo,
    /// Theme name (`name`).
    pub name: Option<String>,
    /// Colour scheme (`clrScheme`).
    pub colors: ColorScheme,
    /// Font scheme (`fontScheme`).
    pub fonts: FontScheme,
    /// Format scheme (`fmtScheme`).
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
// Transitions and animations — §19.3.1.50 transition, §19.5 (timing)
// ---------------------------------------------------------------------------

/// Slide transition effect (children of §19.3.1.50 transition).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionKind {
    /// §19.3.1.15 cut.
    Cut,
    /// §19.3.1.20 fade.
    Fade,
    /// §19.3.1.39 push.
    Push,
    /// §19.3.1.53 wipe.
    Wipe,
    /// §19.3.1.17 cover.
    Cover,
    /// §19.3.1.40 pull.
    Pull,
    /// §19.3.1.47 split.
    Split,
    /// §19.3.1.41 random.
    Random,
    /// Any other effect, by local element name (`dissolve`, `p14:vortex`, …).
    Other(String),
}

/// §19.3.1.50 transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Transition {
    /// `None` = transition element without an effect child.
    pub kind: Option<TransitionKind>,
    /// Effect direction (`l`/`r`/`u`/`d`, `horz`/`vert`, `in`/`out`).
    pub dir: Option<String>,
    /// ST_TransitionSpeed: `fast` | `med` | `slow`.
    pub speed: Option<String>,
    /// Explicit duration in ms ([MS-PPTX] p14:dur).
    pub dur_ms: Option<u32>,
    /// `advClick` (default true).
    pub advance_on_click: Option<bool>,
    /// `advTm` in ms.
    pub advance_after_ms: Option<u32>,
    /// Attributes of the effect element (`dir`, `orient`, `thruBlk`, `spokes`, [MS-PPTX] `pattern`,
    /// `prst`, …) as read or as BlueSlide writes them; empty: only [`Transition::dir`] is written.
    #[serde(default)]
    pub attrs: Vec<(String, String)>,
    /// §19.3.1.42 sndAc: the sound played with the transition, or stopping the one playing.
    #[serde(default)]
    pub sound: Option<TransitionSound>,
    /// Element as read (`p:transition` or the `mc:AlternateContent` wrapping it).
    /// Written back verbatim while the modelled fields are unchanged.
    pub source: Option<Extra>,
}

/// §19.3.1.42 sndAc of a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionSound {
    /// §19.3.1.45 stSnd: §19.3.1.44 snd (embedded WAV audio, relationship id and name), `loop`.
    Start {
        /// `r:embed` of the audio relationship.
        embed: String,
        /// `name` of the sound.
        name: String,
        /// `stSnd@loop`: play until the next sound.
        looped: bool,
    },
    /// §19.3.1.19 endSnd: stop the sound playing.
    Stop,
}

/// §19.5.29 cTn@presetClass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimClass {
    /// Entrance effect (`entr`).
    Entrance,
    /// Exit effect (`exit`).
    Exit,
    /// Emphasis effect (`emph`).
    Emphasis,
    /// Motion path (`path`).
    Path,
    /// Any other preset class, by attribute value.
    Other(String),
}

/// Recognised animation effect (from the time-node behaviours).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimEffect {
    /// Visibility set only (§19.5.66 set of `style.visibility`).
    Appear,
    /// §19.5.3 animEffect filter `fade`.
    Fade,
    /// §19.5.1 anim of `ppt_x` / `ppt_y` from off-slide.
    FlyIn,
    /// §19.5.3 animEffect filter `wipe(…)`.
    Wipe,
    /// Unrecognised effect.
    Other,
}

/// §19.5.29 cTn@nodeType of an effect node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimTrigger {
    /// Starts on click (`clickEffect`).
    OnClick,
    /// Starts with the previous effect (`withEffect`).
    WithPrevious,
    /// Starts after the previous effect (`afterEffect`).
    AfterPrevious,
}

/// One effect of the main sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animation {
    /// Target shape (`p:spTgt@spid` = cNvPr id).
    pub shape_id: u32,
    /// Preset class.
    pub class: AnimClass,
    /// Recognised effect.
    pub effect: AnimEffect,
    /// `l` | `r` | `t` | `b` (where the shape comes from / the wipe starts).
    pub direction: Option<String>,
    /// How the effect starts.
    pub trigger: AnimTrigger,
    /// Delay before start, ms.
    pub delay_ms: u32,
    /// Duration, ms.
    pub dur_ms: u32,
    /// Preset effect id (`presetID`).
    pub preset_id: Option<u32>,
    /// Preset subtype (`presetSubtype`).
    pub preset_subtype: Option<u32>,
    /// The effect's `p:par` as read, kept for effects BlueOffice does not model
    /// (emphasis, motion paths, other filters): regenerated timing reuses it, so
    /// those effects survive edits of the sequence losslessly.
    #[serde(default)]
    pub source: Option<XmlElement>,
}

/// §19.3.1.48 timing reduced to the main sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Timing {
    /// Main-sequence effects in order.
    pub animations: Vec<Animation>,
    /// `p:timing` as read; written back verbatim while `animations` is unchanged.
    pub source: Option<Extra>,
}

/// An animation placed on the slide's timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    /// Index into [`Timing::animations`].
    pub animation: usize,
    /// 0 = starts with the slide, n = after the n-th click.
    pub click: usize,
    /// Start / end in ms, relative to the click.
    pub start_ms: u32,
    /// End in ms, relative to the click.
    pub end_ms: u32,
}

impl Timing {
    /// Simple timeline: `OnClick` opens a new click step, `WithPrevious`
    /// starts with the previous effect, `AfterPrevious` when it ends.
    pub fn timeline(&self) -> Vec<TimelineEntry> {
        let mut out: Vec<TimelineEntry> = Vec::new();
        let mut click = 0;
        let mut prev_start = 0;
        let mut prev_end = 0;
        for (i, a) in self.animations.iter().enumerate() {
            let base = match a.trigger {
                AnimTrigger::OnClick => {
                    click += 1;
                    prev_end = 0;
                    0
                }
                AnimTrigger::WithPrevious => prev_start,
                AnimTrigger::AfterPrevious => prev_end,
            };
            let start = base + a.delay_ms;
            let end = start + a.dur_ms;
            out.push(TimelineEntry {
                animation: i,
                click,
                start_ms: start,
                end_ms: end,
            });
            prev_start = start;
            prev_end = prev_end.max(end);
        }
        out
    }

    /// Number of clicks needed to play every effect.
    pub fn click_count(&self) -> usize {
        self.timeline().last().map_or(0, |e| e.click)
    }
}

// ---------------------------------------------------------------------------
// Presentation — §19.2.1.26
// ---------------------------------------------------------------------------

/// Slide size (§19.2.1.39 sldSz) in EMU.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SlideSize {
    /// Width (`cx`), EMU.
    pub cx: i64,
    /// Height (`cy`), EMU.
    pub cy: i64,
    /// Preset size type (`type`, e.g. `screen16x9`).
    pub kind: Option<String>,
}

/// A whole presentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Presentation {
    /// Slide size (`sldSz`).
    pub slide_size: SlideSize,
    /// §19.2.1.23 notesSz.
    pub notes_size: SlideSize,
    /// Slides in presentation order.
    pub slides: Vec<Slide>,
    /// Slide layouts.
    pub layouts: Vec<SlideLayout>,
    /// Slide masters.
    pub masters: Vec<SlideMaster>,
    /// Themes.
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
