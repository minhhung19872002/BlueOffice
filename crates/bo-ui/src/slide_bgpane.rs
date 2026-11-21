//! Format Background pane of BlueSlide (N6). Round 4: docked right of the editing area
//! like the reference presentation app's; round 20: Solid / Gradient / Pattern fill.
//! Round 22: laid out by measurement of the reference at 125 % (pane 357 px, title, Fill
//! tab button, "Fill" section, radio rows 36 px apart, controls right-aligned 40 px from
//! the edge, scroll bar, Apply to All / Reset Background fixed at the bottom), with
//! * Solid fill: Color drop-down (theme and standard colours) and Transparency;
//! * Gradient fill: Preset gradients (6 accents × 5 kinds), Type (linear, radial,
//!   rectangular, path), Direction, Angle, editable Gradient stops (add, remove, drag,
//!   Color, Position, Transparency, Brightness);
//! * Picture or texture fill: Insert… (a picture file), our own textures, Transparency,
//!   Tile picture as texture with Offset X / Y, Scale X / Y, Alignment and Mirror type, or
//!   stretch offsets;
//! * Pattern fill: 48 preset patterns (§20.1.10.51) with Foreground / Background colours.
//!
//! The pane keeps the fill's settings and builds the engine command
//! ([`BgPane::take_command`]); the window runs it.
use crate::slide_pickers::{hex, rgb, tint, STANDARD};
use crate::spin_box::{
    inside, paint_check, paint_combo, paint_spin, spin_hit, BoxLook, SpinEdit, SpinHit, SpinKey,
};
use crate::{Key, Painter, TextStyle};
use bo_engine::slide_edit::master::{BackgroundStop, BackgroundTile, MasterCommand};
use bo_render::{PathSeg, Rgba};

/// What a press in the pane asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BgHit {
    /// Close the pane.
    Close,
    /// Solid fill `RRGGBB` for the current slide.
    Color(String),
    /// Toggle "Hide background graphics".
    HideGraphics,
    /// Give every slide the current slide's background.
    ApplyAll,
    /// Back to the layout's background.
    Reset,
    /// Apply the pane's current fill to the slide ([`BgPane::take_command`]).
    Apply,
    /// Ask the host for a picture file (Insert…).
    PickPicture,
    /// Only the pane changed.
    Redraw,
}

/// Preset patterns offered (§20.1.10.51 ST_PresetPatternVal), 8 per row.
pub const PATTERNS: [&str; 48] = [
    "pct5",
    "pct10",
    "pct20",
    "pct25",
    "pct30",
    "pct40",
    "pct50",
    "pct60",
    "pct70",
    "pct75",
    "pct80",
    "pct90",
    "ltDnDiag",
    "ltUpDiag",
    "dkDnDiag",
    "dkUpDiag",
    "wdDnDiag",
    "wdUpDiag",
    "ltVert",
    "ltHorz",
    "narVert",
    "narHorz",
    "dkVert",
    "dkHorz",
    "dashDnDiag",
    "dashUpDiag",
    "dashHorz",
    "dashVert",
    "smConfetti",
    "lgConfetti",
    "zigZag",
    "wave",
    "diagBrick",
    "horzBrick",
    "weave",
    "plaid",
    "divot",
    "dotGrid",
    "dotDmnd",
    "shingle",
    "trellis",
    "sphere",
    "smGrid",
    "lgGrid",
    "smCheck",
    "lgCheck",
    "openDmnd",
    "solidDmnd",
];

/// Linear directions (degrees) in the Direction gallery.
pub const LINEAR_DIRS: [f32; 8] = [45.0, 90.0, 135.0, 0.0, 180.0, 315.0, 270.0, 225.0];

/// Path focus rectangles (l, t, r, b %) in the Direction gallery of radial / rectangular
/// gradients: from the bottom-right, bottom-left corner, the centre, the top-right,
/// top-left corner.
pub const FOCUS: [[f32; 4]; 5] = [
    [100.0, 100.0, 0.0, 0.0],
    [0.0, 100.0, 100.0, 0.0],
    [50.0, 50.0, 50.0, 50.0],
    [100.0, 0.0, 0.0, 100.0],
    [0.0, 0.0, 100.0, 100.0],
];

const PANE_BG: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const DIVIDER: Rgba = Rgba(0xbf, 0xbf, 0xbf, 255);
const RULE: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const TAB_FILL: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const TAB_EDGE: Rgba = Rgba(0x80, 0x80, 0x80, 255);
const RADIO_ON: Rgba = Rgba(0xa9, 0x2b, 0x1a, 255);
const RADIO_OFF: Rgba = Rgba(0x80, 0x80, 0x80, 255);
const ACCENT: Rgba = Rgba(0xd2, 0x47, 0x26, 255);
const BUTTON_EDGE: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const DIM: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);
const HOVER: Rgba = Rgba(0xd5, 0xe1, 0xf2, 255);
const SELECT: Rgba = Rgba(0x00, 0x78, 0xd7, 255);

/// One gradient stop as the pane edits it.
#[derive(Debug, Clone, PartialEq)]
pub struct Stop {
    /// Position, %.
    pub pos: f32,
    /// Colour before Brightness.
    pub base: Rgba,
    /// Brightness −100…100 % (towards black / white).
    pub bright: f32,
    /// Transparency %.
    pub transp: f32,
}

impl Stop {
    /// The stop's colour with Brightness applied.
    #[must_use]
    pub fn color(&self) -> Rgba {
        tint(self.base, self.bright / 100.0)
    }
}

/// Gradient settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Grad {
    /// Stops (2 to 10), kept in position order.
    pub stops: Vec<Stop>,
    /// Selected stop.
    pub sel: usize,
    /// 0 linear, 1 radial, 2 rectangular, 3 path.
    pub kind: u8,
    /// Linear angle, degrees.
    pub angle: f32,
    /// Radial / rectangular focus (index into [`FOCUS`]).
    pub focus: usize,
}

impl Default for Grad {
    fn default() -> Self {
        preset(Rgba(0x44, 0x72, 0xc4, 255), 0)
    }
}

/// Preset gradient of `accent` in `row`: light linear, top spotlight, medium linear, bottom
/// spotlight, radial (our own stop choices).
#[must_use]
pub fn preset(accent: Rgba, row: usize) -> Grad {
    let st = |pos: f32, t: f32| Stop {
        pos,
        base: tint(accent, t),
        bright: 0.0,
        transp: 0.0,
    };
    let (stops, kind, focus) = match row {
        0 => (
            vec![st(0.0, 0.92), st(74.0, 0.6), st(83.0, 0.6), st(100.0, 0.5)],
            0,
            2,
        ),
        1 => (vec![st(0.0, 0.85), st(100.0, 0.25)], 1, 4),
        2 => (vec![st(0.0, 0.55), st(55.0, 0.15), st(100.0, -0.15)], 0, 2),
        3 => (vec![st(0.0, 0.5), st(100.0, -0.25)], 1, 0),
        _ => (vec![st(0.0, 0.1), st(100.0, -0.35)], 1, 2),
    };
    Grad {
        stops,
        sel: 0,
        kind,
        angle: 90.0,
        focus,
    }
}

/// Picture / texture settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Pic {
    /// Tile picture as texture.
    pub tile: bool,
    /// Tile offsets, pt.
    pub tx: f32,
    /// Vertical tile offset, pt.
    pub ty: f32,
    /// Tile scale, %.
    pub sx: f32,
    /// Vertical tile scale, %.
    pub sy: f32,
    /// Alignment (index into [`ALIGNS`]).
    pub align: usize,
    /// Mirror type: 0 none, 1 horizontal, 2 vertical, 3 both.
    pub mirror: usize,
    /// Stretch offsets left, right, top, bottom, %.
    pub offsets: [f32; 4],
    /// Transparency %.
    pub transp: f32,
    /// Texture chosen last (shown on the Texture button).
    pub texture: Option<usize>,
    /// The slide has a picture background (options can change).
    pub has_picture: bool,
}

impl Default for Pic {
    fn default() -> Self {
        Pic {
            tile: true,
            tx: 0.0,
            ty: 0.0,
            sx: 100.0,
            sy: 100.0,
            align: 0,
            mirror: 0,
            offsets: [0.0; 4],
            transp: 0.0,
            texture: Some(0),
            has_picture: false,
        }
    }
}

/// Tile alignments (§20.1.10.53) in the Alignment box.
pub const ALIGNS: [&str; 9] = ["tl", "t", "tr", "l", "ctr", "r", "bl", "b", "br"];
const ALIGN_NAMES: [&str; 9] = [
    "Top left",
    "Top",
    "Top right",
    "Left",
    "Center",
    "Right",
    "Bottom left",
    "Bottom",
    "Bottom right",
];
const MIRRORS: [&str; 4] = ["none", "x", "y", "xy"];
const MIRROR_NAMES: [&str; 4] = ["None", "Horizontal", "Vertical", "Both"];
const TYPE_NAMES: [&str; 4] = ["Linear", "Radial", "Rectangular", "Path"];

/// Pattern fill settings: preset index ([`PATTERNS`]), foreground, background (a
/// transparent foreground: not chosen yet).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pat(pub usize, pub Rgba, pub Rgba);

impl Default for Pat {
    fn default() -> Self {
        Pat(0, Rgba(0, 0, 0, 0), WHITE)
    }
}

/// Which colour a Color drop-down sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTarget {
    /// The solid fill.
    Solid,
    /// The selected gradient stop.
    Stop,
    /// Pattern foreground.
    Fore,
    /// Pattern background.
    Back,
}

/// A drop-down gallery or list of the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopKind {
    /// Preset gradients.
    Presets,
    /// Gradient direction.
    Direction,
    /// Textures.
    Texture,
    /// Colours.
    Color(ColorTarget),
    /// Gradient type list.
    Type,
    /// Tile alignment list.
    Align,
    /// Mirror type list.
    Mirror,
}

/// An open drop-down: its frame, cells with their values, the cell under the pointer.
#[derive(Debug, Clone)]
struct Popup {
    kind: PopKind,
    rect: [f32; 4],
    cells: Vec<([f32; 4], usize)>,
    hover: Option<usize>,
    /// Colours of a colour popup's cells (same order as `cells`).
    colors: Vec<Rgba>,
    /// Header bars of a colour popup (rect, label key).
    heads: Vec<([f32; 4], &'static str)>,
}

/// A control placed in the pane (window px, scrolled).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ctl {
    /// A spin box by id.
    Spin(&'static str),
    Drop(PopKind),
    Combo(PopKind),
    StopTrack,
    AddStop,
    RemoveStop,
    Insert,
    Clipboard,
    TileCheck,
    Rotate,
    Pattern(usize),
}

/// A row of the scrolling content: its label and control.
#[derive(Debug, Clone, Copy)]
struct Placed {
    ctl: Ctl,
    rect: [f32; 4],
    /// Label (key, fallback) left of the control and its x.
    label: Option<(&'static str, &'static str, f32)>,
    /// Row centre y.
    cy: f32,
}

/// The pane's state and layout.
#[derive(Debug, Default)]
pub struct BgPane {
    /// Shown.
    pub shown: bool,
    /// Background graphics hidden on the current slide.
    pub hide_graphics: bool,
    /// Solid colour (`RRGGBB`).
    pub current: Option<String>,
    /// Solid transparency %.
    pub solid_transp: f32,
    /// Fill kind shown: 0 solid, 1 gradient, 2 picture or texture, 3 pattern.
    pub kind: u8,
    /// Gradient settings.
    pub grad: Grad,
    /// Picture settings.
    pub pic: Pic,
    /// Pattern preset index and colours.
    pub pattern: Pat,
    /// Picture bytes (base64) to send with the next picture command.
    pub pending: Option<String>,
    /// Insert… asked the host for a file: the next picked picture is the background.
    pub awaiting_file: bool,
    /// Slide the settings were read from.
    pub synced: Option<usize>,
    frame: [f32; 4],
    scale: f32,
    theme: Vec<Rgba>,
    close: [f32; 4],
    apply: [f32; 4],
    reset: [f32; 4],
    tab: [f32; 4],
    view: [f32; 4],
    scroll: f32,
    content_h: f32,
    placed: Vec<Placed>,
    radios: [[f32; 4]; 4],
    hide: [f32; 4],
    popup: Option<Popup>,
    edit: Option<SpinEdit>,
    drag_stop: bool,
    pointer: (f32, f32),
    hover: Option<Ctl>,
    scroll_drag: Option<(f32, f32)>,
    /// Access-key underlines by label key: x offset and width in the label (measured).
    marks: Vec<(&'static str, f32, f32)>,
    /// The title as it fits before the options chevron (measured; "..." when cut).
    title: Option<String>,
    /// Measured text widths of Apply to All and Reset Background.
    button_text: [f32; 2],
}

/// Access keys of the pane's labels (the reference's underlined letters in English).
const ACCESS: [(&str, char); 26] = [
    ("slideNative.solidFill", 'S'),
    ("slideR10.bg.gradient", 'G'),
    ("slideR22.bg.picture", 'P'),
    ("slideR10.bg.pattern", 'a'),
    ("slideR22.bg.hide", 'H'),
    ("slideR22.bg.presets", 'r'),
    ("slideR22.bg.type", 'y'),
    ("slideR22.bg.direction", 'D'),
    ("slideR22.bg.angle", 'e'),
    ("slideR22.bg.color", 'C'),
    ("slideR22.bg.position", 'o'),
    ("slideR22.bg.transparency", 'T'),
    ("slideR22.bg.brightness", 'i'),
    ("slideR22.bg.rotate", 'w'),
    ("slideR22.bg.texture", 'u'),
    ("slideR22.bg.tile", 'i'),
    ("slideR22.bg.offsetX", 'O'),
    ("slideR22.bg.offsetY", 'e'),
    ("slideR22.bg.scaleX", 'X'),
    ("slideR22.bg.scaleY", 'Y'),
    ("slideR22.bg.alignment", 'g'),
    ("slideR22.bg.mirror", 'M'),
    ("slideR22.bg.fore", 'F'),
    ("slideR22.bg.back", 'B'),
    ("slideNative.applyAll", 'A'),
    ("slideR22.bg.reset", 'B'),
];

/// Fallback text of an access-key label key.
fn fallback(key: &str) -> &'static str {
    match key {
        "slideNative.solidFill" => "Solid fill",
        "slideR10.bg.gradient" => "Gradient fill",
        "slideR22.bg.picture" => "Picture or texture fill",
        "slideR10.bg.pattern" => "Pattern fill",
        "slideR22.bg.hide" => "Hide background graphics",
        "slideR22.bg.presets" => "Preset gradients",
        "slideR22.bg.type" => "Type",
        "slideR22.bg.direction" => "Direction",
        "slideR22.bg.angle" => "Angle",
        "slideR22.bg.color" => "Color",
        "slideR22.bg.position" => "Position",
        "slideR22.bg.transparency" => "Transparency",
        "slideR22.bg.brightness" => "Brightness",
        "slideR22.bg.rotate" => "Rotate with shape",
        "slideR22.bg.texture" => "Texture",
        "slideR22.bg.tile" => "Tile picture as texture",
        "slideR22.bg.offsetX" => "Offset X",
        "slideR22.bg.offsetY" => "Offset Y",
        "slideR22.bg.scaleX" => "Scale X",
        "slideR22.bg.scaleY" => "Scale Y",
        "slideR22.bg.alignment" => "Alignment",
        "slideR22.bg.mirror" => "Mirror type",
        "slideR22.bg.fore" => "Foreground",
        "slideR22.bg.back" => "Background",
        "slideNative.applyAll" => "Apply to All",
        "slideR22.bg.reset" => "Reset Background",
        _ => "",
    }
}

impl BgPane {
    /// Reference device px at 125 % → this scale.
    fn d(&self, v: f32) -> f32 {
        (v * self.scale / 1.25).round()
    }

    /// Pane width at `scale` (357 device px at 125 %, divider included).
    #[must_use]
    pub fn width(scale: f32) -> f32 {
        (357.0 * scale / 1.25).round()
    }

    /// Lay out in `r` (window px) at `scale`, with the theme's colours (accents 1–6, dark 1,
    /// light 1).
    pub fn layout(&mut self, r: [f32; 4], scale: f32, theme: &[Rgba]) {
        self.frame = r;
        self.scale = scale;
        self.theme = theme.to_vec();
        let [x, y, w, h] = r;
        let d = |v: f32| (v * scale / 1.25).round();
        self.close = [x + d(318.0), y + d(14.0), d(22.0), d(26.0)];
        self.tab = [x + d(17.0), y + d(61.0), d(46.0), d(46.0)];
        // Buttons as wide as their text plus the reference's padding (Apply to All 114 px,
        // Reset Background 162 px at 125 %), 14 px apart.
        let bw = |i: usize, d0: f32| {
            let t = self.button_text[i];
            if t > 0.0 {
                (t + d(22.0)).round().max(d(70.0))
            } else {
                d(d0)
            }
        };
        let aw = bw(0, 114.0);
        self.apply = [x + d(15.0), y + h - d(56.0), aw, d(35.0)];
        self.reset = [
            x + d(15.0) + aw + d(14.0),
            y + h - d(56.0),
            bw(1, 162.0),
            d(35.0),
        ];
        self.view = [
            x + 1.0,
            y + d(132.0),
            w - 1.0,
            (h - d(132.0) - d(65.0)).max(0.0),
        ];
        let top = self.view[1] - self.scroll;
        // Fill header row centre 16 px below the viewport top; radios 59 px further, 36 apart.
        for (i, rr) in self.radios.iter_mut().enumerate() {
            let cy = top + d(59.0) + d(36.0) * i as f32;
            *rr = [x + d(46.0), cy - d(15.0), d(200.0), d(31.0)];
        }
        let cb = top + d(202.0);
        self.hide = [x + d(46.0), cb - d(15.0), d(240.0), d(31.0)];
        self.placed = self.rows(x, cb);
        let bottom = self
            .placed
            .iter()
            .map(|p| p.rect[1] + p.rect[3])
            .fold(cb + d(20.0), f32::max);
        self.content_h = bottom + d(16.0) - top;
        let max = (self.content_h - self.view[3]).max(0.0);
        if self.scroll > max {
            self.scroll = max;
            self.layout(r, scale, theme);
        }
    }

    /// The rows below "Hide background graphics" (centre `cb`) for the fill kind.
    fn rows(&self, x: f32, cb: f32) -> Vec<Placed> {
        let d = |v: f32| self.d(v);
        let right = x + d(316.0);
        let label = |k: &'static str, f: &'static str, indent: bool| {
            Some((k, f, x + d(if indent { 71.0 } else { 59.0 })))
        };
        let spin = |id: &'static str, k, f, indent, dy: f32| Placed {
            ctl: Ctl::Spin(id),
            rect: [right - d(115.0), cb + d(dy) - d(18.0), d(115.0), d(36.0)],
            label: label(k, f, indent),
            cy: cb + d(dy),
        };
        let drop = |kind: PopKind, k, f, indent, dy: f32| Placed {
            ctl: Ctl::Drop(kind),
            rect: [right - d(61.0), cb + d(dy) - d(19.0), d(61.0), d(39.0)],
            label: label(k, f, indent),
            cy: cb + d(dy),
        };
        let combo = |kind: PopKind, k, f, indent, dy: f32, w: f32| Placed {
            ctl: Ctl::Combo(kind),
            rect: [right - d(w), cb + d(dy) - d(15.0), d(w), d(31.0)],
            label: label(k, f, indent),
            cy: cb + d(dy),
        };
        let check = |ctl: Ctl, k, f, dy: f32| Placed {
            ctl,
            rect: [x + d(59.0), cb + d(dy) - d(15.0), d(220.0), d(31.0)],
            label: Some((k, f, x + d(84.0))),
            cy: cb + d(dy),
        };
        let mut v = Vec::new();
        match self.kind {
            0 => {
                v.push(drop(
                    PopKind::Color(ColorTarget::Solid),
                    "slideR22.bg.color",
                    "Color",
                    false,
                    54.0,
                ));
                v.push(spin(
                    "solidT",
                    "slideR22.bg.transparency",
                    "Transparency",
                    false,
                    97.0,
                ));
            }
            1 => {
                v.push(drop(
                    PopKind::Presets,
                    "slideR22.bg.presets",
                    "Preset gradients",
                    false,
                    50.0,
                ));
                v.push(combo(
                    PopKind::Type,
                    "slideR22.bg.type",
                    "Type",
                    false,
                    92.0,
                    162.0,
                ));
                v.push(drop(
                    PopKind::Direction,
                    "slideR22.bg.direction",
                    "Direction",
                    false,
                    134.0,
                ));
                v.push(spin("angle", "slideR22.bg.angle", "Angle", false, 178.5));
                v.push(Placed {
                    ctl: Ctl::StopTrack,
                    rect: [x + d(71.0), cb + d(235.5), d(145.0), d(44.0)],
                    label: label("slideR22.bg.stops", "Gradient stops", false),
                    cy: cb + d(216.0),
                });
                v.push(Placed {
                    ctl: Ctl::AddStop,
                    rect: [x + d(230.0), cb + d(239.5), d(36.0), d(36.0)],
                    label: None,
                    cy: cb + d(257.5),
                });
                v.push(Placed {
                    ctl: Ctl::RemoveStop,
                    rect: [x + d(280.0), cb + d(239.5), d(36.0), d(36.0)],
                    label: None,
                    cy: cb + d(257.5),
                });
                v.push(drop(
                    PopKind::Color(ColorTarget::Stop),
                    "slideR22.bg.color",
                    "Color",
                    true,
                    306.0,
                ));
                v.push(spin("pos", "slideR22.bg.position", "Position", true, 350.5));
                v.push(spin(
                    "stopT",
                    "slideR22.bg.transparency",
                    "Transparency",
                    true,
                    393.5,
                ));
                v.push(spin(
                    "bright",
                    "slideR22.bg.brightness",
                    "Brightness",
                    true,
                    436.5,
                ));
                v.push(check(
                    Ctl::Rotate,
                    "slideR22.bg.rotate",
                    "Rotate with shape",
                    474.5,
                ));
            }
            2 => {
                v.push(Placed {
                    ctl: Ctl::Insert,
                    rect: [x + d(59.0), cb + d(62.5), d(76.0), d(35.0)],
                    label: label("slideR22.bg.source", "Picture source", false),
                    cy: cb + d(43.0),
                });
                v.push(Placed {
                    ctl: Ctl::Clipboard,
                    rect: [x + d(149.0), cb + d(62.5), d(98.0), d(35.0)],
                    label: None,
                    cy: cb + d(80.0),
                });
                v.push(drop(
                    PopKind::Texture,
                    "slideR22.bg.texture",
                    "Texture",
                    false,
                    124.0,
                ));
                v.push(spin(
                    "picT",
                    "slideR22.bg.transparency",
                    "Transparency",
                    false,
                    168.5,
                ));
                v.push(check(
                    Ctl::TileCheck,
                    "slideR22.bg.tile",
                    "Tile picture as texture",
                    209.0,
                ));
                if self.pic.tile {
                    v.push(spin("tx", "slideR22.bg.offsetX", "Offset X", true, 249.5));
                    v.push(spin("ty", "slideR22.bg.offsetY", "Offset Y", true, 292.5));
                    v.push(spin("sx", "slideR22.bg.scaleX", "Scale X", true, 335.5));
                    v.push(spin("sy", "slideR22.bg.scaleY", "Scale Y", true, 378.5));
                    v.push(combo(
                        PopKind::Align,
                        "slideR22.bg.alignment",
                        "Alignment",
                        true,
                        419.0,
                        137.0,
                    ));
                    v.push(combo(
                        PopKind::Mirror,
                        "slideR22.bg.mirror",
                        "Mirror type",
                        true,
                        457.0,
                        117.0,
                    ));
                    v.push(check(
                        Ctl::Rotate,
                        "slideR22.bg.rotate",
                        "Rotate with shape",
                        495.0,
                    ));
                } else {
                    v.push(spin("ol", "slideR22.bg.left", "Left offset", true, 249.5));
                    v.push(spin("or", "slideR22.bg.right", "Right offset", true, 292.5));
                    v.push(spin("ot", "slideR22.bg.top", "Top offset", true, 335.5));
                    v.push(spin(
                        "ob",
                        "slideR22.bg.bottom",
                        "Bottom offset",
                        true,
                        378.5,
                    ));
                    v.push(check(
                        Ctl::Rotate,
                        "slideR22.bg.rotate",
                        "Rotate with shape",
                        417.0,
                    ));
                }
            }
            _ => {
                let cols = 8;
                let cw = ((d(257.0) - (cols - 1) as f32 * d(4.0)) / cols as f32).floor();
                let ch = d(28.0);
                let gy = cb + d(68.0);
                for (i, _) in PATTERNS.iter().enumerate() {
                    let (row, col) = (i / cols, i % cols);
                    v.push(Placed {
                        ctl: Ctl::Pattern(i),
                        rect: [
                            x + d(59.0) + col as f32 * (cw + d(4.0)),
                            gy + row as f32 * (ch + d(4.0)),
                            cw,
                            ch,
                        ],
                        label: if i == 0 {
                            label("slideR22.bg.pattern", "Pattern", false)
                        } else {
                            None
                        },
                        cy: cb + d(46.0),
                    });
                }
                let after = 68.0 + 6.0 * 32.0;
                v.push(drop(
                    PopKind::Color(ColorTarget::Fore),
                    "slideR22.bg.fore",
                    "Foreground",
                    false,
                    after + 30.0,
                ));
                v.push(drop(
                    PopKind::Color(ColorTarget::Back),
                    "slideR22.bg.back",
                    "Background",
                    false,
                    after + 74.0,
                ));
            }
        }
        v
    }

    /// Interface text style of the pane (12.4 px logical: the reference's pane text).
    fn style(&self) -> TextStyle {
        TextStyle {
            size: (12.4 * self.scale).round(),
            bold: false,
        }
    }

    /// Title style (20 px logical).
    fn title_style(&self) -> TextStyle {
        TextStyle {
            size: (20.0 * self.scale).round(),
            bold: false,
        }
    }

    /// Measure the access-key underlines, the title cut and the bottom buttons' text with
    /// the host's text measure (`t(key, fallback)` gives the strings); lays out again.
    pub fn measure(&mut self, text: &dyn crate::TextMeasure, t: &dyn Fn(&str, &str) -> String) {
        let style = self.style();
        self.marks.clear();
        for (key, c) in ACCESS {
            let label = t(key, fallback(key));
            let at = label.find(c).or_else(|| {
                label
                    .char_indices()
                    .find(|(_, ch)| ch.eq_ignore_ascii_case(&c))
                    .map(|(i, _)| i)
            });
            let Some(i) = at else { continue };
            let ch_len = label[i..].chars().next().map_or(1, char::len_utf8);
            let x0 = text.measure(&label[..i], style).0;
            let x1 = text.measure(&label[..i + ch_len], style).0;
            self.marks.push((key, x0, (x1 - x0).max(1.0)));
        }
        let head = t("slideNative.formatBackground", "Format Background");
        let ts = self.title_style();
        let room = self.d(250.0);
        self.title = Some(if text.measure(&head, ts).0 > room {
            let mut cut = head.clone();
            while !cut.is_empty() && text.measure(&format!("{}...", cut.trim_end()), ts).0 > room {
                cut.pop();
            }
            format!("{}...", cut.trim_end())
        } else {
            head
        });
        self.button_text = [
            text.measure(&t("slideNative.applyAll", "Apply to All"), style)
                .0,
            text.measure(&t("slideR22.bg.reset", "Reset Background"), style)
                .0,
        ];
        self.relayout();
    }

    /// Draw a label at `x` with its access-key underline when Windows shows access keys.
    #[allow(clippy::too_many_arguments)]
    fn label(
        &self,
        p: &mut dyn Painter,
        text: &str,
        key: &str,
        x: f32,
        top: f32,
        style: TextStyle,
        ink: Rgba,
    ) {
        p.text(text, x, top, style, ink);
        if !crate::popup::keyboard_cues() {
            return;
        }
        if let Some((_, mx, mw)) = self.marks.iter().find(|(k, _, _)| *k == key) {
            let uy = (top + style.size * 1.12).round();
            p.fill_rect((x + mx).round(), uy, mw.round().max(1.0), 1.0, ink);
        }
    }

    /// Whether a window position is over the pane or its open drop-down.    /// Whether a window position is over the pane or its open drop-down.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.shown
            && (inside(self.frame, x, y)
                || self.popup.as_ref().is_some_and(|p| inside(p.rect, x, y)))
    }

    /// A drop-down is open (presses anywhere go to the pane first).
    pub fn popup_open(&self) -> bool {
        self.shown && self.popup.is_some()
    }

    /// A spin box is being typed in.
    pub fn editing(&self) -> bool {
        self.shown && self.edit.is_some()
    }

    fn scroll_max(&self) -> f32 {
        (self.content_h - self.view[3]).max(0.0)
    }

    /// The scroll bar's track and thumb (window px) when the content overflows.
    fn scrollbar(&self) -> Option<([f32; 4], [f32; 4])> {
        let max = self.scroll_max();
        if max <= 0.0 {
            return None;
        }
        let [x, y, _, h] = self.view;
        let bw = self.d(21.0);
        let track = [x - 1.0 + self.d(320.0), y, bw, h];
        let inner = h - 2.0 * bw;
        let th = (inner * self.view[3] / self.content_h).max(self.d(20.0));
        let ty = y + bw + (inner - th) * self.scroll / max;
        Some((track, [track[0], ty, bw, th]))
    }

    /// Scroll the content by `dy` device px.
    fn scroll_by(&mut self, dy: f32) -> bool {
        let to = (self.scroll + dy).clamp(0.0, self.scroll_max());
        if to == self.scroll {
            return false;
        }
        self.scroll = to;
        let (r, s, t) = (self.frame, self.scale, self.theme.clone());
        self.layout(r, s, &t);
        true
    }

    /// The pointer was last over the pane.
    pub fn pointer_inside(&self) -> bool {
        self.shown && inside(self.frame, self.pointer.0, self.pointer.1)
    }

    /// Mouse wheel (`dy` device px, positive = down) while the pointer is over the pane.
    pub fn wheel(&mut self, dy: f32) -> bool {
        if !self.pointer_inside() {
            return false;
        }
        self.popup = None;
        self.scroll_by(dy)
    }

    /// Pointer moved (hover, stop and scroll-bar dragging).
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        self.pointer = (x, y);
        if let Some(p) = self.popup.as_mut() {
            let h = p.cells.iter().position(|(r, _)| inside(*r, x, y));
            if h != p.hover {
                p.hover = h;
                return true;
            }
            return false;
        }
        let h = self
            .placed
            .iter()
            .find(|p| inside(p.rect, x, y))
            .map(|p| p.ctl);
        if h != self.hover {
            self.hover = h;
            return true;
        }
        false
    }

    /// A drag (button held) to (`x`, `y`): moves the selected gradient stop or the scroll
    /// thumb. `Some(true)`: handled and the pane changed.
    pub fn drag(&mut self, x: f32, y: f32) -> Option<bool> {
        self.pointer = (x, y);
        if let Some((y0, s0)) = self.scroll_drag {
            let (track, thumb) = self.scrollbar()?;
            let inner = track[3] - 2.0 * track[2] - thumb[3];
            if inner > 0.0 {
                let to = s0 + (y - y0) * self.scroll_max() / inner;
                return Some(self.scroll_by(to - self.scroll));
            }
            return Some(false);
        }
        if !self.drag_stop {
            return None;
        }
        let pos = self.track_pos(x)?;
        let g = &mut self.grad;
        if let Some(s) = g.stops.get_mut(g.sel) {
            if (s.pos - pos).abs() < 0.5 {
                return Some(false);
            }
            s.pos = pos;
        }
        Some(true)
    }

    /// Button released: ends a stop drag (the gradient is applied) or a thumb drag.
    pub fn release(&mut self) -> Option<BgHit> {
        if self.scroll_drag.take().is_some() {
            return Some(BgHit::Redraw);
        }
        if !std::mem::take(&mut self.drag_stop) {
            return None;
        }
        self.sort_stops();
        Some(BgHit::Apply)
    }

    /// Position % of a window x on the stop track.
    fn track_pos(&self, x: f32) -> Option<f32> {
        let r = self.placed.iter().find(|p| p.ctl == Ctl::StopTrack)?.rect;
        let m = self.d(12.0);
        let w = (r[2] - 2.0 * m).max(1.0);
        Some(((x - r[0] - m) / w * 100.0).clamp(0.0, 100.0).round())
    }

    /// The stop marker rects (window px), in stop order.
    fn markers(&self) -> Vec<[f32; 4]> {
        let Some(r) = self
            .placed
            .iter()
            .find(|p| p.ctl == Ctl::StopTrack)
            .map(|p| p.rect)
        else {
            return Vec::new();
        };
        let m = self.d(12.0);
        let w = r[2] - 2.0 * m;
        self.grad
            .stops
            .iter()
            .map(|s| {
                let cx = r[0] + m + w * s.pos / 100.0;
                [cx - m, r[1], 2.0 * m, r[3] - self.d(2.0)]
            })
            .collect()
    }

    fn sort_stops(&mut self) {
        let g = &mut self.grad;
        let sel = g.stops.get(g.sel).cloned();
        g.stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        if let Some(s) = sel {
            g.sel = g.stops.iter().position(|t| *t == s).unwrap_or(0);
        }
    }

    /// A press at (`x`, `y`) (already known to be over the pane or its drop-down, or any
    /// press while a drop-down is open).
    pub fn press(&mut self, x: f32, y: f32) -> Option<BgHit> {
        self.pointer = (x, y);
        if let Some(p) = self.popup.take() {
            if !inside(p.rect, x, y) {
                return Some(BgHit::Redraw);
            }
            let v = p.cells.iter().find(|(r, _)| inside(*r, x, y)).map(|c| c.1);
            let Some(v) = v else {
                self.popup = Some(p);
                return Some(BgHit::Redraw);
            };
            let color = p
                .cells
                .iter()
                .position(|c| c.1 == v)
                .and_then(|i| p.colors.get(i).copied());
            return Some(self.choose(p.kind, v, color));
        }
        let commit = self.commit_edit();
        if inside(self.close, x, y) {
            return Some(BgHit::Close);
        }
        if inside(self.apply, x, y) {
            return Some(BgHit::ApplyAll);
        }
        if inside(self.reset, x, y) {
            return Some(BgHit::Reset);
        }
        if let Some((track, thumb)) = self.scrollbar() {
            if inside(track, x, y) {
                let bw = track[2];
                if y < track[1] + bw {
                    self.scroll_by(-self.d(30.0));
                } else if y >= track[1] + track[3] - bw {
                    self.scroll_by(self.d(30.0));
                } else if inside(thumb, x, y) {
                    self.scroll_drag = Some((y, self.scroll));
                } else if y < thumb[1] {
                    self.scroll_by(-self.view[3]);
                } else {
                    self.scroll_by(self.view[3]);
                }
                return Some(BgHit::Redraw);
            }
        }
        if !inside(self.view, x, y) {
            return commit
                .or(Some(BgHit::Redraw))
                .filter(|_| inside(self.frame, x, y));
        }
        if let Some(i) = self.radios.iter().position(|r| inside(*r, x, y)) {
            return Some(self.set_kind(i as u8));
        }
        if inside(self.hide, x, y) {
            return Some(BgHit::HideGraphics);
        }
        // Stop markers first (they overhang the track's ends).
        if let Some(i) = self.markers().iter().position(|r| inside(*r, x, y)) {
            self.grad.sel = i;
            self.drag_stop = true;
            self.relayout();
            return Some(BgHit::Redraw);
        }
        let Some(pl) = self.placed.iter().find(|p| inside(p.rect, x, y)).copied() else {
            return commit.or(Some(BgHit::Redraw));
        };
        Some(match pl.ctl {
            Ctl::Spin(id) => match spin_hit(pl.rect, self.scale, x, y) {
                Some(SpinHit::Up) => self.step(id, 1.0),
                Some(SpinHit::Down) => self.step(id, -1.0),
                _ => {
                    self.edit = Some(SpinEdit {
                        id,
                        text: String::new(),
                    });
                    BgHit::Redraw
                }
            },
            Ctl::Drop(kind) | Ctl::Combo(kind) => {
                if kind == PopKind::Direction && self.grad.kind == 3 {
                    return Some(BgHit::Redraw);
                }
                self.open_popup(kind, pl.rect);
                BgHit::Redraw
            }
            Ctl::StopTrack => {
                // A press on the bar adds a stop there in the blended colour.
                let pos = self.track_pos(x)?;
                self.add_stop(Some(pos))
            }
            Ctl::AddStop => self.add_stop(None),
            Ctl::RemoveStop => {
                if self.grad.stops.len() <= 2 {
                    return Some(BgHit::Redraw);
                }
                let i = self.grad.sel.min(self.grad.stops.len() - 1);
                self.grad.stops.remove(i);
                self.grad.sel = i.min(self.grad.stops.len() - 1);
                BgHit::Apply
            }
            Ctl::Insert => BgHit::PickPicture,
            Ctl::Clipboard | Ctl::Rotate => BgHit::Redraw,
            Ctl::TileCheck => {
                self.pic.tile = !self.pic.tile;
                self.relayout();
                self.picture_apply()
            }
            Ctl::Pattern(i) => {
                self.pattern.0 = i;
                BgHit::Apply
            }
        })
    }

    fn relayout(&mut self) {
        let (r, s, t) = (self.frame, self.scale, self.theme.clone());
        self.layout(r, s, &t);
    }

    /// Choose fill kind `k`; the slide takes that kind's current settings.
    fn set_kind(&mut self, k: u8) -> BgHit {
        if self.kind == k {
            return BgHit::Redraw;
        }
        self.kind = k;
        if k == 2 && !self.pic.has_picture {
            // Picture or texture fill starts with the first texture, tiled.
            self.pic = Pic::default();
            self.pending = crate::slide_textures22::png(0)
                .map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b));
        }
        if k == 0 && self.current.is_none() {
            self.current = Some("FFFFFF".into());
        }
        if k == 1 && self.grad == Grad::default() {
            // Gradient fill starts with the light preset of the theme's first accent.
            let accent = self
                .theme
                .first()
                .copied()
                .unwrap_or(Rgba(0x44, 0x72, 0xc4, 255));
            self.grad = preset(accent, 0);
        }
        if k == 3 && self.pattern.1 == Rgba(0, 0, 0, 0) {
            self.pattern = Pat(
                0,
                self.theme
                    .first()
                    .copied()
                    .unwrap_or(Rgba(0x44, 0x72, 0xc4, 255)),
                WHITE,
            );
        }
        self.relayout();
        BgHit::Apply
    }

    fn picture_apply(&self) -> BgHit {
        if self.pic.has_picture || self.pending.is_some() {
            BgHit::Apply
        } else {
            BgHit::Redraw
        }
    }

    /// Add a stop: at `pos`, or halfway between the selected stop and its neighbour.
    fn add_stop(&mut self, pos: Option<f32>) -> BgHit {
        let g = &mut self.grad;
        if g.stops.len() >= 10 {
            return BgHit::Redraw;
        }
        let sel = g.sel.min(g.stops.len().saturating_sub(1));
        let pos = pos.unwrap_or_else(|| {
            let a = g.stops.get(sel).map_or(0.0, |s| s.pos);
            let b = g.stops.get(sel + 1).map_or(100.0, |s| s.pos);
            if sel + 1 < g.stops.len() {
                f32::midpoint(a, b).round()
            } else {
                let p = g.stops.get(sel.wrapping_sub(1)).map_or(0.0, |s| s.pos);
                f32::midpoint(a, p).round()
            }
        });
        let c = color_at(&g.stops, pos);
        g.stops.push(Stop {
            pos,
            base: Rgba(c.0, c.1, c.2, 255),
            bright: 0.0,
            transp: 0.0,
        });
        g.sel = g.stops.len() - 1;
        self.sort_stops();
        BgHit::Apply
    }

    /// Value of spin box `id`.
    fn value(&self, id: &str) -> f32 {
        let s = self.grad.stops.get(self.grad.sel);
        match id {
            "solidT" => self.solid_transp,
            "angle" => self.grad.angle,
            "pos" => s.map_or(0.0, |s| s.pos),
            "stopT" => s.map_or(0.0, |s| s.transp),
            "bright" => s.map_or(0.0, |s| s.bright),
            "picT" => self.pic.transp,
            "tx" => self.pic.tx,
            "ty" => self.pic.ty,
            "sx" => self.pic.sx,
            "sy" => self.pic.sy,
            "ol" => self.pic.offsets[0],
            "or" => self.pic.offsets[1],
            "ot" => self.pic.offsets[2],
            "ob" => self.pic.offsets[3],
            _ => 0.0,
        }
    }

    fn set_value(&mut self, id: &str, v: f32) {
        let pct = v.clamp(0.0, 100.0);
        let sel = self.grad.sel;
        let stop = self.grad.stops.get_mut(sel);
        match id {
            "solidT" => self.solid_transp = pct,
            "angle" => self.grad.angle = v.rem_euclid(360.0),
            "pos" => {
                if let Some(s) = stop {
                    s.pos = pct;
                }
                self.sort_stops();
            }
            "stopT" => {
                if let Some(s) = stop {
                    s.transp = pct;
                }
            }
            "bright" => {
                if let Some(s) = stop {
                    s.bright = v.clamp(-100.0, 100.0);
                }
            }
            "picT" => self.pic.transp = pct,
            "tx" => self.pic.tx = v.clamp(-4000.0, 4000.0),
            "ty" => self.pic.ty = v.clamp(-4000.0, 4000.0),
            "sx" => self.pic.sx = v.clamp(1.0, 1000.0),
            "sy" => self.pic.sy = v.clamp(1.0, 1000.0),
            "ol" => self.pic.offsets[0] = v.clamp(-500.0, 99.0),
            "or" => self.pic.offsets[1] = v.clamp(-500.0, 99.0),
            "ot" => self.pic.offsets[2] = v.clamp(-500.0, 99.0),
            "ob" => self.pic.offsets[3] = v.clamp(-500.0, 99.0),
            _ => {}
        }
    }

    /// Text a spin box shows.
    fn shown_value(&self, id: &str) -> String {
        let v = self.value(id);
        let n = if (v - v.round()).abs() < 0.05 {
            format!("{}", v.round() as i64)
        } else {
            format!("{v:.1}")
        };
        match id {
            "angle" => format!("{n}\u{b0}"),
            "tx" | "ty" => format!("{n} pt"),
            _ => format!("{n}%"),
        }
    }

    fn step(&mut self, id: &'static str, dir: f32) -> BgHit {
        let by = match id {
            "angle" => 10.0,
            _ => 1.0,
        };
        let v = self.value(id) + by * dir;
        self.set_value(id, v);
        self.after_change(id)
    }

    /// What a changed value does: the slide follows.
    fn after_change(&self, id: &str) -> BgHit {
        match id {
            "picT" | "tx" | "ty" | "sx" | "sy" | "ol" | "or" | "ot" | "ob" => self.picture_apply(),
            "solidT" if self.current.is_none() => BgHit::Redraw,
            _ => BgHit::Apply,
        }
    }

    /// Commit a typed value (on Enter or a press elsewhere).
    fn commit_edit(&mut self) -> Option<BgHit> {
        let e = self.edit.take()?;
        let v = e.text.replace(',', ".").parse::<f32>().ok()?;
        self.set_value(e.id, v);
        Some(self.after_change(e.id))
    }

    /// Typed text while a spin box is being edited.
    pub fn text(&mut self, s: &str) -> bool {
        self.edit.as_mut().is_some_and(|e| e.type_text(s))
    }

    /// A key while a spin box is being edited.
    pub fn key(&mut self, k: Key) -> Option<BgHit> {
        let e = self.edit.as_mut()?;
        match e.key(k) {
            SpinKey::Edited => Some(BgHit::Redraw),
            SpinKey::Commit => Some(self.commit_edit().unwrap_or(BgHit::Redraw)),
            SpinKey::Cancel => {
                self.edit = None;
                Some(BgHit::Redraw)
            }
            SpinKey::Up | SpinKey::Down => {
                let id = e.id;
                self.edit = None;
                Some(self.step(id, if k == Key::Up { 1.0 } else { -1.0 }))
            }
            SpinKey::Ignored => None,
        }
    }

    /// Open drop-down `kind` under (or above) the control at `r`.
    fn open_popup(&mut self, kind: PopKind, r: [f32; 4]) {
        let d = |v: f32| self.d(v);
        let mut cells = Vec::new();
        let mut colors = Vec::new();
        let mut heads = Vec::new();
        let grid = |n: usize, cols: usize, cell: f32, gap: f32, pad: f32| {
            let rows = n.div_ceil(cols);
            let w = cols as f32 * cell + (cols - 1) as f32 * gap + 2.0 * pad;
            let h = rows as f32 * cell + (rows.saturating_sub(1)) as f32 * gap + 2.0 * pad;
            (w, h)
        };
        let (w, h) = match kind {
            PopKind::Presets => grid(30, 6, d(84.0), d(10.0), d(6.0)),
            PopKind::Direction => grid(self.dir_count(), 5, d(84.0), d(10.0), d(6.0)),
            PopKind::Texture => grid(
                crate::slide_textures22::NAMES.len(),
                5,
                d(96.0),
                d(8.0),
                d(6.0),
            ),
            PopKind::Color(_) => (d(402.0), d(352.0)),
            PopKind::Type | PopKind::Mirror => (r[2], d(36.0) * 4.0 + 2.0),
            PopKind::Align => (r[2], d(36.0) * 9.0 + 2.0),
        };
        // Right-aligned with the control, kept inside the window (pane's right edge).
        let win_right = self.frame[0] + self.frame[2];
        let x = (r[0] + r[2] - w).min(win_right - w - 1.0).max(1.0);
        let below = r[1] + r[3];
        let win_bottom = self.frame[1] + self.frame[3];
        let y = if below + h <= win_bottom {
            below
        } else {
            (r[1] - h).max(self.frame[1] - d(100.0)).max(1.0)
        };
        let rect = [x, y, w, h];
        let place = |n: usize, cols: usize, cell: f32, gap: f32, pad: f32| {
            (0..n)
                .map(|i| {
                    let (row, col) = (i / cols, i % cols);
                    (
                        [
                            x + pad + col as f32 * (cell + gap),
                            y + pad + row as f32 * (cell + gap),
                            cell,
                            cell,
                        ],
                        i,
                    )
                })
                .collect::<Vec<_>>()
        };
        match kind {
            PopKind::Presets => cells = place(30, 6, d(84.0), d(10.0), d(6.0)),
            PopKind::Direction => cells = place(self.dir_count(), 5, d(84.0), d(10.0), d(6.0)),
            PopKind::Texture => {
                cells = place(
                    crate::slide_textures22::NAMES.len(),
                    5,
                    d(96.0),
                    d(8.0),
                    d(6.0),
                );
            }
            PopKind::Color(_) => {
                let cell = d(34.0);
                let step = d(40.0);
                let x0 = x + d(4.0);
                let mut yy = y + d(2.0);
                heads.push(([x + 1.0, yy, w - 2.0, d(34.0)], "theme"));
                yy += d(38.0);
                let base = self.palette_base();
                for (c, col) in base.iter().enumerate() {
                    cells.push(([x0 + c as f32 * step, yy, cell, cell], cells.len()));
                    colors.push(*col);
                }
                yy += cell + d(6.0);
                for row in 0..5 {
                    for (c, col) in base.iter().enumerate() {
                        cells.push(([x0 + c as f32 * step, yy, cell, cell], cells.len()));
                        colors.push(palette_tint(*col, row));
                    }
                    yy += cell;
                }
                yy += d(8.0);
                heads.push(([x + 1.0, yy, w - 2.0, d(34.0)], "standard"));
                yy += d(38.0);
                for (c, v) in STANDARD.iter().enumerate() {
                    cells.push(([x0 + c as f32 * step, yy, cell, cell], cells.len()));
                    colors.push(rgb(*v));
                }
            }
            PopKind::Type | PopKind::Align | PopKind::Mirror => {
                let n = match kind {
                    PopKind::Type | PopKind::Mirror => 4,
                    _ => 9,
                };
                for i in 0..n {
                    cells.push(([x + 1.0, y + 1.0 + i as f32 * d(36.0), w - 2.0, d(36.0)], i));
                }
            }
        }
        self.popup = Some(Popup {
            kind,
            rect,
            cells,
            hover: None,
            colors,
            heads,
        });
    }

    /// Direction gallery size for the gradient type.
    fn dir_count(&self) -> usize {
        match self.grad.kind {
            0 => LINEAR_DIRS.len(),
            1 | 2 => FOCUS.len(),
            _ => 0,
        }
    }

    /// Top row of the colour drop-down: light 1, dark 1, light 2, dark 2, accents 1–6.
    fn palette_base(&self) -> Vec<Rgba> {
        let t = |i: usize, d: u32| self.theme.get(i).copied().unwrap_or_else(|| rgb(d));
        let mut v = vec![
            t(7, 0xFF_FFFF),
            t(6, 0x00_0000),
            rgb(0xE7_E6E6),
            rgb(0x44_546A),
        ];
        for (i, d) in [
            0x44_72C4, 0xED_7D31, 0xA5_A5A5, 0xFF_C000, 0x5B_9BD5, 0x70_AD47,
        ]
        .iter()
        .enumerate()
        {
            v.push(t(i, *d));
        }
        v
    }

    /// A drop-down cell chosen.
    fn choose(&mut self, kind: PopKind, v: usize, color: Option<Rgba>) -> BgHit {
        match kind {
            PopKind::Presets => {
                let accent = self
                    .theme
                    .get(v % 6)
                    .copied()
                    .unwrap_or(Rgba(0x44, 0x72, 0xc4, 255));
                let angle = self.grad.angle;
                self.grad = preset(accent, v / 6);
                if self.grad.kind == 0 {
                    self.grad.angle = angle;
                }
                self.relayout();
                BgHit::Apply
            }
            PopKind::Direction => {
                match self.grad.kind {
                    0 => self.grad.angle = LINEAR_DIRS.get(v).copied().unwrap_or(90.0),
                    _ => self.grad.focus = v.min(FOCUS.len() - 1),
                }
                BgHit::Apply
            }
            PopKind::Texture => {
                self.pic.texture = Some(v);
                self.pic.tile = true;
                self.pending = crate::slide_textures22::png(v)
                    .map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b));
                self.relayout();
                BgHit::Apply
            }
            PopKind::Color(t) => {
                let Some(c) = color else {
                    return BgHit::Redraw;
                };
                match t {
                    ColorTarget::Solid => {
                        self.current = Some(hex(c));
                        return BgHit::Color(hex(c));
                    }
                    ColorTarget::Stop => {
                        if let Some(s) = self.grad.stops.get_mut(self.grad.sel) {
                            s.base = c;
                            s.bright = 0.0;
                        }
                    }
                    ColorTarget::Fore => self.pattern.1 = c,
                    ColorTarget::Back => self.pattern.2 = c,
                }
                BgHit::Apply
            }
            PopKind::Type => {
                self.grad.kind = v.min(3) as u8;
                self.relayout();
                BgHit::Apply
            }
            PopKind::Align => {
                self.pic.align = v.min(ALIGNS.len() - 1);
                self.picture_apply()
            }
            PopKind::Mirror => {
                self.pic.mirror = v.min(MIRRORS.len() - 1);
                self.picture_apply()
            }
        }
    }

    /// A picture file was picked for Insert….
    pub fn set_picture(&mut self, bytes: &[u8]) {
        self.pending = Some(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        ));
        self.pic.texture = None;
        self.pic.tile = false;
        self.pic.offsets = [0.0; 4];
        self.kind = 2;
        self.relayout();
    }

    /// The engine command for the pane's fill (picture bytes are sent once).
    pub fn take_command(&mut self, all: bool) -> Option<MasterCommand> {
        Some(match self.kind {
            0 => {
                let color = self.current.clone()?;
                if self.solid_transp > 0.0 {
                    let stop = |pos: f64| BackgroundStop {
                        pos,
                        color: color.clone(),
                        transparency: f64::from(self.solid_transp),
                    };
                    // A transparent solid colour: an even two-stop gradient keeps the alpha.
                    MasterCommand::SetBackgroundGradient {
                        stops: vec![stop(0.0), stop(100.0)],
                        path: None,
                        angle: Some(90.0),
                        focus: None,
                        rotate: None,
                        all,
                    }
                } else {
                    MasterCommand::SetBackground {
                        color: Some(color),
                        all,
                    }
                }
            }
            1 => {
                let g = &self.grad;
                let stops = g
                    .stops
                    .iter()
                    .map(|s| BackgroundStop {
                        pos: f64::from(s.pos),
                        color: hex(s.color()),
                        transparency: f64::from(s.transp),
                    })
                    .collect();
                let (path, focus) = match g.kind {
                    0 => (None, None),
                    1 | 2 => (
                        Some(if g.kind == 1 { "circle" } else { "rect" }.to_owned()),
                        FOCUS.get(g.focus).map(|f| f.map(f64::from)),
                    ),
                    _ => (Some("shape".to_owned()), Some([50.0; 4])),
                };
                MasterCommand::SetBackgroundGradient {
                    stops,
                    path,
                    angle: Some(f64::from(g.angle)),
                    focus,
                    rotate: None,
                    all,
                }
            }
            2 => {
                let p = &self.pic;
                let data = self.pending.take();
                if data.is_none() && !p.has_picture {
                    return None;
                }
                let o = p.offsets;
                MasterCommand::SetBackgroundPicture {
                    data,
                    tile: p.tile.then(|| BackgroundTile {
                        tx: f64::from(p.tx),
                        ty: f64::from(p.ty),
                        sx: f64::from(p.sx),
                        sy: f64::from(p.sy),
                        algn: ALIGNS[p.align.min(8)].into(),
                        flip: MIRRORS[p.mirror.min(3)].into(),
                    }),
                    // Pane order left, right, top, bottom → l, t, r, b.
                    offsets: (!p.tile).then(|| [o[0], o[2], o[1], o[3]].map(f64::from)),
                    transparency: f64::from(p.transp),
                    rotate: None,
                    all,
                }
            }
            _ => {
                let Pat(i, fg, bg) = self.pattern;
                MasterCommand::SetBackgroundFill {
                    kind: "pattern".into(),
                    colors: vec![hex(fg), hex(bg)],
                    angle: None,
                    preset: Some(PATTERNS[i.min(PATTERNS.len() - 1)].into()),
                    all,
                }
            }
        })
    }

    /// Read the settings from the slide's own background (`None`: it inherits).
    pub fn sync(&mut self, slide: usize, fill: Option<&bo_model::slide::Fill>) {
        use bo_model::slide::{ColorKind, Fill, GradientShade};
        self.synced = Some(slide);
        self.edit = None;
        self.popup = None;
        let srgb = |c: &bo_model::slide::Color| match &c.kind {
            ColorKind::Srgb(v) => Some(rgb(*v)),
            _ => None,
        };
        let alpha = |c: &bo_model::slide::Color| {
            c.mods
                .iter()
                .find(|m| m.name == "alpha")
                .and_then(|m| m.val)
                .map_or(0.0, |a| (100.0 - a as f32 / 1000.0).clamp(0.0, 100.0))
        };
        self.pic.has_picture = false;
        match fill {
            Some(Fill::Solid(c)) => {
                self.kind = 0;
                self.current = srgb(c).map(hex);
                self.solid_transp = alpha(c);
            }
            Some(Fill::Gradient(g)) => {
                let stops: Vec<Stop> = g
                    .stops
                    .iter()
                    .map(|s| Stop {
                        pos: s.pos as f32 / 1000.0,
                        base: srgb(&s.color).unwrap_or(Rgba(0x80, 0x80, 0x80, 255)),
                        bright: 0.0,
                        transp: alpha(&s.color),
                    })
                    .collect();
                // An even two-stop gradient of one colour is a transparent solid colour.
                if stops.len() == 2 && stops[0].base == stops[1].base && stops[0].transp > 0.0 {
                    self.kind = 0;
                    self.current = Some(hex(stops[0].base));
                    self.solid_transp = stops[0].transp;
                } else if stops.len() >= 2 {
                    self.kind = 1;
                    let (kind, angle, focus) = match &g.shade {
                        Some(GradientShade::Path { path, fill_to }) => {
                            let f = fill_to.map(|r| r.map(|v| v as f32 / 1000.0));
                            let focus = f
                                .and_then(|f| FOCUS.iter().position(|c| *c == f))
                                .unwrap_or(2);
                            let k = match path.as_str() {
                                "circle" => 1,
                                "rect" => 2,
                                _ => 3,
                            };
                            (k, self.grad.angle, focus)
                        }
                        Some(GradientShade::Linear { ang, .. }) => (0, *ang as f32 / 60_000.0, 2),
                        None => (0, 90.0, 2),
                    };
                    self.grad = Grad {
                        sel: self.grad.sel.min(stops.len() - 1),
                        stops,
                        kind,
                        angle,
                        focus,
                    };
                }
            }
            Some(Fill::Picture(b)) => {
                self.kind = 2;
                self.pic.has_picture = true;
                self.pic.transp = b
                    .blip_children
                    .iter()
                    .find(|e| e.local() == "alphaModFix")
                    .and_then(|e| e.attr("amt"))
                    .and_then(|v| v.parse::<f32>().ok())
                    .map_or(0.0, |a| (100.0 - a / 1000.0).clamp(0.0, 100.0));
                if let Some(t) = &b.tile {
                    {
                        let n = |k: &str, d: f32| {
                            t.attr(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d)
                        };
                        self.pic.tile = true;
                        self.pic.tx = (n("tx", 0.0) / 12_700.0 * 10.0).round() / 10.0;
                        self.pic.ty = (n("ty", 0.0) / 12_700.0 * 10.0).round() / 10.0;
                        self.pic.sx = n("sx", 100_000.0) / 1000.0;
                        self.pic.sy = n("sy", 100_000.0) / 1000.0;
                        let a = t.attr("algn").unwrap_or("tl");
                        self.pic.align = ALIGNS.iter().position(|v| *v == a).unwrap_or(0);
                        let f = t.attr("flip").unwrap_or("none");
                        self.pic.mirror = MIRRORS.iter().position(|v| *v == f).unwrap_or(0);
                    }
                } else {
                    {
                        self.pic.tile = false;
                        let [l, t, r, bb] = b.fill_rect.as_deref().copied().unwrap_or([0; 4]);
                        self.pic.offsets = [l, r, t, bb].map(|v| v as f32 / 1000.0);
                    }
                }
            }
            Some(Fill::Pattern(pf)) => {
                self.kind = 3;
                let name = pf.preset.as_deref().unwrap_or("pct5");
                self.pattern.0 = PATTERNS.iter().position(|p| *p == name).unwrap_or(0);
                if let Some(c) = pf.fg.as_ref().and_then(srgb) {
                    self.pattern.1 = c;
                }
                if let Some(c) = pf.bg.as_ref().and_then(srgb) {
                    self.pattern.2 = c;
                }
            }
            _ => {
                self.kind = 0;
                self.current = None;
                self.solid_transp = 0.0;
            }
        }
        if self.pattern.1 == Rgba(0, 0, 0, 0) {
            self.pattern.1 = self
                .theme
                .first()
                .copied()
                .unwrap_or(Rgba(0x44, 0x72, 0xc4, 255));
            self.pattern.2 = WHITE;
        }
        self.relayout();
    }

    /// Paint with `t(key, fallback)` for the interface strings.
    pub fn paint(&self, p: &mut dyn Painter, t: &dyn Fn(&str, &str) -> String, ink: Rgba) {
        if !self.shown {
            return;
        }
        let d = |v: f32| self.d(v);
        let [x, y, w, h] = self.frame;
        p.fill_rect(x, y, w, h, PANE_BG);
        p.fill_rect(x, y, 1.0, h, DIVIDER);
        // Measured on the reference: the title 20 px, cut with "..." before the options
        // chevron ([`BgPane::measure`]).
        let title = self.title_style();
        let style = self.style();
        let bold = TextStyle {
            bold: true,
            ..style
        };
        let head = self
            .title
            .clone()
            .unwrap_or_else(|| t("slideNative.formatBackground", "Format Background"));
        p.text(&head, x + d(16.0), y + d(10.0), title, ink);
        // Close cross.
        let [cx, cy, cw, ch] = self.close;
        let k = d(7.0);
        let (mx, my) = ((cx + cw / 2.0).round() + 0.5, (cy + ch / 2.0).round() + 0.5);
        p.path(
            &[
                PathSeg::M(mx - k, my - k),
                PathSeg::L(mx + k, my + k),
                PathSeg::M(mx + k, my - k),
                PathSeg::L(mx - k, my + k),
            ],
            None,
            Some((ink, 1.0)),
        );
        // Task pane options chevron left of the cross.
        let (vx, vy) = (x + d(277.0) + 0.5, my - d(3.0));
        p.path(
            &[
                PathSeg::M(vx - d(7.0), vy),
                PathSeg::L(vx, vy + d(7.0)),
                PathSeg::L(vx + d(7.0), vy),
            ],
            None,
            Some((ink, 1.0)),
        );
        // The Fill tab button (selected) with its pointer in the rule below.
        let [tx, ty, tw, th] = self.tab;
        p.fill_rect(tx, ty, tw, th, TAB_EDGE);
        p.fill_rect(tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0, TAB_FILL);
        paint_bucket(p, [tx, ty, tw, th], d(1.0));
        let ry = y + d(126.0);
        p.fill_rect(x + d(16.0), ry, d(324.0), 1.0, RULE);
        let nx = tx + tw / 2.0;
        p.path(
            &[
                PathSeg::M(nx - d(9.0), ry + 0.5),
                PathSeg::L(nx, ry - d(9.0)),
                PathSeg::L(nx + d(9.0), ry + 0.5),
            ],
            Some(PANE_BG),
            Some((RULE, 1.0)),
        );
        // Scrolling content.
        let [vx0, vy0, vw, vh] = self.view;
        p.push_clip(vx0, vy0, vw, vh);
        let top = vy0 - self.scroll;
        let hy = top + d(16.0);
        chevron(p, x + d(32.0), hy, d(5.0), ink);
        p.text(
            &t("slideNative.fill", "Fill"),
            x + d(48.0),
            text_y(hy, bold),
            bold,
            ink,
        );
        let kinds = [
            "slideNative.solidFill",
            "slideR10.bg.gradient",
            "slideR22.bg.picture",
            "slideR10.bg.pattern",
        ];
        for (i, key) in kinds.iter().enumerate() {
            let r = self.radios[i];
            let cyy = r[1] + r[3] / 2.0;
            paint_radio(p, x + d(56.0), cyy, d(9.0), usize::from(self.kind) == i);
            let label = t(key, fallback(key));
            self.label(p, &label, key, x + d(72.0), text_y(cyy, style), style, ink);
        }
        let hb = d(18.0);
        let hcy = self.hide[1] + self.hide[3] / 2.0;
        paint_check(
            p,
            [x + d(47.0), (hcy - hb / 2.0).round(), hb, hb],
            self.hide_graphics,
            true,
            ink,
        );
        self.label(
            p,
            &t("slideR22.bg.hide", "Hide background graphics"),
            "slideR22.bg.hide",
            x + d(72.0),
            text_y(hcy, style),
            style,
            ink,
        );
        p.fill_rect(x + d(46.0), hcy + d(25.0), d(270.0), 1.0, RULE);
        for pl in &self.placed {
            if let Some((k, f, lx)) = pl.label {
                if !matches!(pl.ctl, Ctl::TileCheck | Ctl::Rotate) {
                    self.label(p, &t(k, f), k, lx, text_y(pl.cy, style), style, ink);
                }
            }
            self.paint_control(p, pl, t, style, ink);
        }
        p.pop_clip();
        if let Some((track, thumb)) = self.scrollbar() {
            paint_scrollbar(p, track, thumb, self.scale);
        }
        // Bottom buttons.
        let reset_on = self.current.is_some() || self.kind != 0;
        for (i, (r, key, on)) in [
            (self.apply, "slideNative.applyAll", true),
            (self.reset, "slideR22.bg.reset", reset_on),
        ]
        .into_iter()
        .enumerate()
        {
            let label = t(key, fallback(key));
            p.fill_rect(r[0], r[1], r[2], r[3], if on { BUTTON_EDGE } else { RULE });
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(0xfd, 0xfd, 0xfd, 255),
            );
            let tw = if self.button_text[i] > 0.0 {
                self.button_text[i]
            } else {
                crate::slide_pickers::estimate(&label, style) * 0.9
            };
            self.label(
                p,
                &label,
                key,
                (r[0] + (r[2] - tw) / 2.0).round(),
                text_y(r[1] + r[3] / 2.0, style),
                style,
                if on { ink } else { DIM },
            );
        }
        if let Some(pop) = &self.popup {
            self.paint_popup(p, pop, t, style, ink);
        }
    }

    fn paint_control(
        &self,
        p: &mut dyn Painter,
        pl: &Placed,
        t: &dyn Fn(&str, &str) -> String,
        style: TextStyle,
        ink: Rgba,
    ) {
        let d = |v: f32| self.d(v);
        let r = pl.rect;
        let hover = self.hover == Some(pl.ctl);
        match pl.ctl {
            Ctl::Spin(id) => {
                let typed = self.edit.as_ref().filter(|e| e.id == id);
                let text = typed.map_or_else(|| self.shown_value(id), |e| e.text.clone());
                let caret = typed.map(|e| crate::slide_pickers::estimate(&e.text, style));
                let enabled = true;
                paint_spin(
                    p,
                    r,
                    self.scale,
                    &BoxLook {
                        text: &text,
                        style,
                        ink,
                        enabled,
                        caret,
                        hover: if hover {
                            spin_hit(r, self.scale, self.pointer.0, self.pointer.1)
                        } else {
                            None
                        },
                    },
                );
                if matches!(id, "solidT" | "picT" | "stopT" | "bright") {
                    // The small slider between label and box.
                    let v = self.value(id);
                    let f = if id == "bright" {
                        (v + 100.0) / 200.0
                    } else {
                        v / 100.0
                    };
                    let cy = (pl.cy).round();
                    if id == "bright" || id == "stopT" {
                        // The reference's stop sliders show only a 2 px bar left of the box,
                        // with a gap after the label.
                        p.fill_rect(
                            r[0] - d(16.0),
                            cy - d(8.0),
                            d(2.5).max(2.0),
                            d(16.0),
                            Rgba(0x40, 0x40, 0x40, 255),
                        );
                    } else {
                        let (sx, sw) = (r[0] - d(32.0), d(28.0));
                        p.fill_rect(sx, cy, sw, 1.0, Rgba(0x60, 0x60, 0x60, 255));
                        let kx = (sx + (sw - d(4.0)) * f.clamp(0.0, 1.0)).round();
                        p.fill_rect(
                            kx,
                            cy - d(9.0),
                            d(4.0),
                            d(18.0),
                            Rgba(0x40, 0x40, 0x40, 255),
                        );
                    }
                }
            }
            Ctl::Drop(kind) => {
                let enabled = !(kind == PopKind::Direction && self.grad.kind == 3);
                let open = self.popup.as_ref().is_some_and(|q| q.kind == kind);
                let edge = if open || hover {
                    Rgba(0x7a, 0x7a, 0x7a, 255)
                } else {
                    BUTTON_EDGE
                };
                p.fill_rect(r[0], r[1], r[2], r[3], edge);
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    if open {
                        Rgba(0xcc, 0xe4, 0xf7, 255)
                    } else {
                        WHITE
                    },
                );
                let icon = [r[0] + d(10.0), r[1] + d(8.0), d(22.0), d(22.0)];
                match kind {
                    PopKind::Color(target) => {
                        let c = match target {
                            ColorTarget::Solid => self
                                .current
                                .as_deref()
                                .and_then(|h| u32::from_str_radix(h, 16).ok())
                                .map_or(WHITE, rgb),
                            ColorTarget::Stop => self
                                .grad
                                .stops
                                .get(self.grad.sel)
                                .map_or(WHITE, Stop::color),
                            ColorTarget::Fore => self.pattern.1,
                            ColorTarget::Back => self.pattern.2,
                        };
                        paint_small_bucket(p, icon, c, ink);
                    }
                    PopKind::Presets | PopKind::Direction => {
                        p.fill_rect(
                            icon[0],
                            icon[1],
                            icon[2],
                            icon[3],
                            Rgba(0x90, 0x90, 0x90, 255),
                        );
                        let inner = [icon[0] + 1.0, icon[1] + 1.0, icon[2] - 2.0, icon[3] - 2.0];
                        if enabled {
                            paint_gradient(p, inner, &self.stop_colors(), &self.shape());
                        } else {
                            p.fill_rect(inner[0], inner[1], inner[2], inner[3], WHITE);
                        }
                    }
                    PopKind::Texture => {
                        p.fill_rect(
                            icon[0],
                            icon[1],
                            icon[2],
                            icon[3],
                            Rgba(0x90, 0x90, 0x90, 255),
                        );
                        if let Some(i) = self.pic.texture {
                            paint_texture(
                                p,
                                [icon[0] + 1.0, icon[1] + 1.0, icon[2] - 2.0, icon[3] - 2.0],
                                i,
                                5,
                            );
                        } else {
                            p.fill_rect(
                                icon[0] + 1.0,
                                icon[1] + 1.0,
                                icon[2] - 2.0,
                                icon[3] - 2.0,
                                WHITE,
                            );
                            // A dotted square: no texture.
                            for k in 0..5 {
                                for j in 0..5 {
                                    p.fill_rect(
                                        icon[0] + d(3.0) + k as f32 * d(4.0),
                                        icon[1] + d(3.0) + j as f32 * d(4.0),
                                        1.0,
                                        1.0,
                                        Rgba(0x40, 0x70, 0xc0, 255),
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
                // Drop arrow.
                let (ax, ay) = (r[0] + r[2] - d(12.0), r[1] + r[3] / 2.0);
                let k = d(5.0);
                p.path(
                    &[
                        PathSeg::M(ax - k, ay - k / 2.0),
                        PathSeg::L(ax + k, ay - k / 2.0),
                        PathSeg::L(ax, ay + k / 2.0),
                        PathSeg::Z,
                    ],
                    Some(if enabled {
                        Rgba(0x20, 0x20, 0x20, 255)
                    } else {
                        DIM
                    }),
                    None,
                );
            }
            Ctl::Combo(kind) => {
                let text = match kind {
                    PopKind::Type => t(
                        &format!("slideR22.bg.type{}", self.grad.kind),
                        TYPE_NAMES[usize::from(self.grad.kind.min(3))],
                    ),
                    PopKind::Align => t(
                        &format!("slideR22.bg.align{}", self.pic.align),
                        ALIGN_NAMES[self.pic.align.min(8)],
                    ),
                    _ => t(
                        &format!("slideR22.bg.mirror{}", self.pic.mirror),
                        MIRROR_NAMES[self.pic.mirror.min(3)],
                    ),
                };
                paint_combo(
                    p,
                    r,
                    self.scale,
                    &BoxLook {
                        text: &text,
                        style,
                        ink,
                        enabled: true,
                        caret: None,
                        hover: hover.then_some(SpinHit::Field),
                    },
                );
            }
            Ctl::StopTrack => {
                let [bx, by, bw, _] = r;
                let m = d(12.0);
                let bar = [bx + m, by + d(3.0), bw - 2.0 * m, d(20.0)];
                p.fill_rect(
                    bar[0] - 1.0,
                    bar[1] - 1.0,
                    bar[2] + 2.0,
                    bar[3] + 2.0,
                    Rgba(0x40, 0x40, 0x40, 255),
                );
                paint_gradient(p, bar, &self.stop_colors(), &GradShape::Linear(0.0));
                for (i, (mr, s)) in self.markers().iter().zip(&self.grad.stops).enumerate() {
                    paint_marker(p, *mr, s.color(), i == self.grad.sel, self.scale);
                }
            }
            Ctl::AddStop | Ctl::RemoveStop => {
                let add = pl.ctl == Ctl::AddStop;
                let enabled = if add {
                    self.grad.stops.len() < 10
                } else {
                    self.grad.stops.len() > 2
                };
                p.fill_rect(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    if hover && enabled {
                        Rgba(0x7a, 0x7a, 0x7a, 255)
                    } else {
                        BUTTON_EDGE
                    },
                );
                p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
                paint_stop_button(p, r, add, enabled, self.scale);
            }
            Ctl::Insert | Ctl::Clipboard => {
                let on = pl.ctl == Ctl::Insert;
                let label = if on {
                    t("slideR22.bg.insert", "Insert...")
                } else {
                    t("slideR22.bg.clipboard", "Clipboard")
                };
                p.fill_rect(r[0], r[1], r[2], r[3], if on { BUTTON_EDGE } else { RULE });
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(0xfd, 0xfd, 0xfd, 255),
                );
                let tw = crate::slide_pickers::estimate(&label, style) * 0.9;
                p.text(
                    &label,
                    (r[0] + (r[2] - tw) / 2.0).round(),
                    text_y(r[1] + r[3] / 2.0, style),
                    style,
                    if on { ink } else { DIM },
                );
            }
            Ctl::TileCheck | Ctl::Rotate => {
                let on = if pl.ctl == Ctl::TileCheck {
                    self.pic.tile
                } else {
                    false
                };
                let enabled = pl.ctl == Ctl::TileCheck;
                let b = d(18.0);
                let box_r = [r[0] + 1.0, (pl.cy - b / 2.0).round(), b, b];
                if on {
                    // Checked boxes are filled with the accent and a white tick.
                    p.fill_rect(box_r[0], box_r[1], b, b, RADIO_ON);
                    p.path(
                        &[
                            PathSeg::M(box_r[0] + b * 0.22, box_r[1] + b * 0.52),
                            PathSeg::L(box_r[0] + b * 0.42, box_r[1] + b * 0.72),
                            PathSeg::L(box_r[0] + b * 0.8, box_r[1] + b * 0.3),
                        ],
                        None,
                        Some((WHITE, (b / 9.0).max(1.0))),
                    );
                } else {
                    paint_check(p, box_r, false, enabled, ink);
                }
                if let Some((k, f, lx)) = pl.label {
                    self.label(
                        p,
                        &t(k, f),
                        k,
                        lx,
                        text_y(pl.cy, style),
                        style,
                        if enabled { ink } else { DIM },
                    );
                }
            }
            Ctl::Pattern(i) => {
                let chosen = self.pattern.0 == i;
                p.fill_rect(
                    r[0] - 1.0,
                    r[1] - 1.0,
                    r[2] + 2.0,
                    r[3] + 2.0,
                    if chosen {
                        Rgba(0x60, 0x60, 0x60, 255)
                    } else if hover {
                        Rgba(0xa0, 0xa0, 0xa0, 255)
                    } else {
                        PANE_BG
                    },
                );
                paint_pattern(p, r, PATTERNS[i], self.pattern.1, self.pattern.2);
            }
        }
    }

    fn paint_popup(
        &self,
        p: &mut dyn Painter,
        pop: &Popup,
        t: &dyn Fn(&str, &str) -> String,
        style: TextStyle,
        ink: Rgba,
    ) {
        let d = |v: f32| self.d(v);
        let [x, y, w, h] = pop.rect;
        p.fill_rect(
            x - 1.0,
            y - 1.0,
            w + 2.0,
            h + 2.0,
            Rgba(0xc6, 0xc6, 0xc6, 255),
        );
        p.fill_rect(x, y, w, h, WHITE);
        let bold = TextStyle {
            bold: true,
            ..style
        };
        for (r, k) in &pop.heads {
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xee, 0xee, 0xee, 255));
            let label = if *k == "theme" {
                t("slideR22.bg.themeColors", "Theme Colors")
            } else {
                t("slideR22.bg.standardColors", "Standard Colors")
            };
            p.text(
                &label,
                r[0] + d(10.0),
                text_y(r[1] + r[3] / 2.0, bold),
                bold,
                Rgba(0x44, 0x44, 0x44, 255),
            );
        }
        for (n, (r, v)) in pop.cells.iter().enumerate() {
            let hover = pop.hover == Some(n);
            match pop.kind {
                PopKind::Presets => {
                    let accent = self
                        .theme
                        .get(v % 6)
                        .copied()
                        .unwrap_or(Rgba(0x44, 0x72, 0xc4, 255));
                    let g = preset(accent, v / 6);
                    let stops: Vec<(f32, Rgba)> =
                        g.stops.iter().map(|s| (s.pos, s.color())).collect();
                    let shape = shape_of(&g);
                    paint_gradient(p, *r, &stops, &shape);
                    rounded_frame(
                        p,
                        *r,
                        d(7.0),
                        WHITE,
                        if hover {
                            ACCENT
                        } else {
                            Rgba(0x9a, 0x9a, 0x9a, 255)
                        },
                    );
                }
                PopKind::Direction => {
                    let mut g = self.grad.clone();
                    match g.kind {
                        0 => g.angle = LINEAR_DIRS[*v % 8],
                        _ => g.focus = *v,
                    }
                    paint_gradient(p, *r, &self.stop_colors(), &shape_of(&g));
                    rounded_frame(
                        p,
                        *r,
                        d(7.0),
                        WHITE,
                        if hover {
                            ACCENT
                        } else {
                            Rgba(0x9a, 0x9a, 0x9a, 255)
                        },
                    );
                }
                PopKind::Texture => {
                    paint_texture(p, *r, *v, 24);
                    if hover || self.pic.texture == Some(*v) {
                        let c = if hover {
                            ACCENT
                        } else {
                            Rgba(0x60, 0x60, 0x60, 255)
                        };
                        outline(p, [r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0], c, 2.0);
                    }
                }
                PopKind::Color(_) => {
                    let c = pop.colors.get(n).copied().unwrap_or(WHITE);
                    p.fill_rect(r[0], r[1], r[2], r[3], c);
                    if hover {
                        outline(p, *r, ACCENT, 2.0);
                        outline(
                            p,
                            [r[0] + 2.0, r[1] + 2.0, r[2] - 4.0, r[3] - 4.0],
                            WHITE,
                            1.0,
                        );
                    } else if c.0 > 240 && c.1 > 240 && c.2 > 240 {
                        outline(p, *r, Rgba(0xd0, 0xd0, 0xd0, 255), 1.0);
                    }
                }
                PopKind::Type | PopKind::Align | PopKind::Mirror => {
                    let (current, name) = match pop.kind {
                        PopKind::Type => (
                            usize::from(self.grad.kind),
                            (format!("slideR22.bg.type{v}"), TYPE_NAMES[(*v).min(3)]),
                        ),
                        PopKind::Align => (
                            self.pic.align,
                            (format!("slideR22.bg.align{v}"), ALIGN_NAMES[(*v).min(8)]),
                        ),
                        _ => (
                            self.pic.mirror,
                            (format!("slideR22.bg.mirror{v}"), MIRROR_NAMES[(*v).min(3)]),
                        ),
                    };
                    let on = hover || (pop.hover.is_none() && current == *v);
                    if on {
                        p.fill_rect(r[0], r[1], r[2], r[3], if hover { HOVER } else { SELECT });
                    }
                    let c = if on && !hover { WHITE } else { ink };
                    p.text(
                        &t(&name.0, name.1),
                        r[0] + d(3.0),
                        text_y(r[1] + r[3] / 2.0, style),
                        style,
                        c,
                    );
                }
            }
        }
    }

    /// The gradient's stops as (position, colour with transparency over white).
    fn stop_colors(&self) -> Vec<(f32, Rgba)> {
        self.grad
            .stops
            .iter()
            .map(|s| {
                let c = s.color();
                let k = 1.0 - s.transp / 100.0;
                let m = |v: u8| (255.0 + (f32::from(v) - 255.0) * k).round() as u8;
                (s.pos, Rgba(m(c.0), m(c.1), m(c.2), 255))
            })
            .collect()
    }

    fn shape(&self) -> GradShape {
        shape_of(&self.grad)
    }
}

/// Shape of a gradient for previews.
#[derive(Debug, Clone, Copy)]
enum GradShape {
    /// Linear at an angle (degrees, clockwise from the x axis).
    Linear(f32),
    /// Circle or rectangle about a focus (fractions of the box), outward.
    Path { focus: (f32, f32), rect: bool },
}

fn shape_of(g: &Grad) -> GradShape {
    match g.kind {
        0 => GradShape::Linear(g.angle),
        3 => GradShape::Path {
            focus: (0.5, 0.5),
            rect: true,
        },
        k => {
            let [l, t, r, b] = FOCUS.get(g.focus).copied().unwrap_or([50.0; 4]);
            GradShape::Path {
                focus: ((l + 100.0 - r) / 200.0, (t + 100.0 - b) / 200.0),
                rect: k == 2,
            }
        }
    }
}

/// Colour at `pos` % between sorted stops.
fn color_at(stops: &[Stop], pos: f32) -> Rgba {
    let v: Vec<(f32, Rgba)> = stops.iter().map(|s| (s.pos, s.color())).collect();
    blend_at(&v, pos)
}

fn blend_at(stops: &[(f32, Rgba)], pos: f32) -> Rgba {
    let Some(first) = stops.first() else {
        return WHITE;
    };
    if pos <= first.0 {
        return first.1;
    }
    for w in stops.windows(2) {
        let (a, b) = (w[0], w[1]);
        if pos <= b.0 {
            let t = if b.0 > a.0 {
                (pos - a.0) / (b.0 - a.0)
            } else {
                1.0
            };
            let m = |u: u8, v: u8| (f32::from(u) + (f32::from(v) - f32::from(u)) * t).round() as u8;
            return Rgba(m(a.1 .0, b.1 .0), m(a.1 .1, b.1 .1), m(a.1 .2, b.1 .2), 255);
        }
    }
    stops.last().map_or(WHITE, |s| s.1)
}

/// Row `row` (0–4) of the colour drop-down's shades under `c`.
fn palette_tint(c: Rgba, row: usize) -> Rgba {
    let lum = (u32::from(c.0) * 3 + u32::from(c.1) * 6 + u32::from(c.2)) / 10;
    let t = if lum > 235 {
        [-0.05, -0.15, -0.25, -0.35, -0.5][row]
    } else if lum < 30 {
        [0.5, 0.35, 0.25, 0.15, 0.05][row]
    } else {
        [0.8, 0.6, 0.4, -0.25, -0.5][row]
    };
    tint(c, t)
}

/// Text top for a row centred at `cy`.
fn text_y(cy: f32, style: TextStyle) -> f32 {
    (cy - style.size * 1.33 / 2.0).round()
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = r * 0.552;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

/// Radio button: white disc in a grey ring, or a thick accent ring when chosen.
fn paint_radio(p: &mut dyn Painter, cx: f32, cy: f32, r: f32, on: bool) {
    if on {
        p.path(&circle(cx, cy, r), Some(RADIO_ON), None);
        p.path(&circle(cx, cy, r * 0.48), Some(WHITE), None);
    } else {
        p.path(
            &circle(cx, cy, r - 0.5),
            Some(WHITE),
            Some((RADIO_OFF, 1.0)),
        );
    }
}

fn chevron(p: &mut dyn Painter, cx: f32, cy: f32, k: f32, ink: Rgba) {
    p.path(
        &[
            PathSeg::M(cx - k, cy - k / 2.0),
            PathSeg::L(cx, cy + k / 2.0),
            PathSeg::L(cx + k, cy - k / 2.0),
        ],
        None,
        Some((ink, 1.5)),
    );
}

fn outline(p: &mut dyn Painter, r: [f32; 4], c: Rgba, w: f32) {
    let [x, y, ww, h] = r;
    p.fill_rect(x, y, ww, w, c);
    p.fill_rect(x, y + h - w, ww, w, c);
    p.fill_rect(x, y, w, h, c);
    p.fill_rect(x + ww - w, y, w, h, c);
}

/// Mask the corners of `r` with `bg` and stroke a rounded frame.
fn rounded_frame(p: &mut dyn Painter, r: [f32; 4], rad: f32, bg: Rgba, edge: Rgba) {
    let [x, y, w, h] = r;
    let k = rad * 0.448;
    let corners = [
        (x, y, 1.0, 1.0),
        (x + w, y, -1.0, 1.0),
        (x, y + h, 1.0, -1.0),
        (x + w, y + h, -1.0, -1.0),
    ];
    for (cx, cy, sx, sy) in corners {
        p.path(
            &[
                PathSeg::M(cx, cy),
                PathSeg::L(cx + sx * rad, cy),
                PathSeg::C(cx + sx * k, cy, cx, cy + sy * k, cx, cy + sy * rad),
                PathSeg::Z,
            ],
            Some(bg),
            None,
        );
    }
    let (x0, y0, x1, y1) = (x + 0.5, y + 0.5, x + w - 0.5, y + h - 0.5);
    p.path(
        &[
            PathSeg::M(x0 + rad, y0),
            PathSeg::L(x1 - rad, y0),
            PathSeg::C(x1 - k, y0, x1, y0 + k, x1, y0 + rad),
            PathSeg::L(x1, y1 - rad),
            PathSeg::C(x1, y1 - k, x1 - k, y1, x1 - rad, y1),
            PathSeg::L(x0 + rad, y1),
            PathSeg::C(x0 + k, y1, x0, y1 - k, x0, y1 - rad),
            PathSeg::L(x0, y0 + rad),
            PathSeg::C(x0, y0 + k, x0 + k, y0, x0 + rad, y0),
            PathSeg::Z,
        ],
        None,
        Some((edge, 1.0)),
    );
}

/// Paint a gradient into `r` as bands (linear) or rings (path) of blended colour.
fn paint_gradient(p: &mut dyn Painter, r: [f32; 4], stops: &[(f32, Rgba)], shape: &GradShape) {
    let [x, y, w, h] = r;
    if w < 1.0 || h < 1.0 {
        return;
    }
    p.push_clip(x, y, w, h);
    let n = 40;
    match *shape {
        GradShape::Linear(angle) => {
            let a = angle.to_radians();
            let (dx, dy) = (a.cos(), a.sin());
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            let half = f32::midpoint(w * dx.abs(), h * dy.abs());
            let e = w + h;
            for k in 0..n {
                let t0 = k as f32 / n as f32;
                let t1 = (k + 1) as f32 / n as f32;
                let s0 = -half + 2.0 * half * t0 - 0.5;
                let s1 = -half + 2.0 * half * t1 + 0.5;
                let c = blend_at(stops, (t0 + t1) * 50.0);
                let pt = |s: f32, q: f32| (cx + dx * s - dy * q, cy + dy * s + dx * q);
                let (a0, b0) = pt(s0, e);
                let (a1, b1) = pt(s1, e);
                let (a2, b2) = pt(s1, -e);
                let (a3, b3) = pt(s0, -e);
                p.path(
                    &[
                        PathSeg::M(a0, b0),
                        PathSeg::L(a1, b1),
                        PathSeg::L(a2, b2),
                        PathSeg::L(a3, b3),
                        PathSeg::Z,
                    ],
                    Some(c),
                    None,
                );
            }
        }
        GradShape::Path { focus, rect } => {
            let (fx, fy) = (x + w * focus.0, y + h * focus.1);
            let far_x = (fx - x).max(x + w - fx);
            let far_y = (fy - y).max(y + h - fy);
            let rad = if rect {
                far_x.max(far_y)
            } else {
                far_x.hypot(far_y)
            };
            p.fill_rect(x, y, w, h, blend_at(stops, 100.0));
            for k in (0..n).rev() {
                let f = (k + 1) as f32 / n as f32;
                let c = blend_at(stops, (k as f32 + 0.5) / n as f32 * 100.0);
                if rect {
                    let (rx, ry) = (far_x * f, far_y * f);
                    p.fill_rect(fx - rx, fy - ry, 2.0 * rx, 2.0 * ry, c);
                } else {
                    p.path(&circle(fx, fy, rad * f), Some(c), None);
                }
            }
        }
    }
    p.pop_clip();
}

/// A texture preview from `n` × `n` blocks.
fn paint_texture(p: &mut dyn Painter, r: [f32; 4], i: usize, n: usize) {
    let cells = crate::slide_textures22::preview(i, n);
    let (cw, ch) = (r[2] / n as f32, r[3] / n as f32);
    for (k, c) in cells.iter().enumerate() {
        let (cx, cy) = ((k % n) as f32, (k / n) as f32);
        let x0 = (r[0] + cx * cw).floor();
        let y0 = (r[1] + cy * ch).floor();
        let x1 = (r[0] + (cx + 1.0) * cw).floor();
        let y1 = (r[1] + (cy + 1.0) * ch).floor();
        p.fill_rect(x0, y0, x1 - x0, y1 - y0, *c);
    }
}

/// Our own 8 × 8 cell of preset pattern `name` (bit set = foreground).
#[must_use]
pub fn pattern_bits(name: &str) -> [u8; 8] {
    let every = |n: u32| {
        let mut b = [0u8; 8];
        let mut k = 0u32;
        for (row, v) in b.iter_mut().enumerate() {
            for col in 0..8 {
                let idx = row as u32 * 8 + col;
                // Spread dots on a skewed lattice for even coverage.
                if (idx * 5 + row as u32 * 3).is_multiple_of(n) {
                    *v |= 1 << col;
                    k += 1;
                }
            }
        }
        let _ = k;
        b
    };
    let diag = |up: bool, width: u32, gap: u32| {
        let mut b = [0u8; 8];
        for (row, v) in b.iter_mut().enumerate() {
            for col in 0..8u32 {
                let s = if up {
                    (col + row as u32) % gap
                } else {
                    (col + 8 - row as u32) % gap
                };
                if s < width {
                    *v |= 1 << col;
                }
            }
        }
        b
    };
    let lines = |vert: bool, width: u32, gap: u32| {
        let mut b = [0u8; 8];
        for (row, v) in b.iter_mut().enumerate() {
            for col in 0..8u32 {
                let at = if vert { col } else { row as u32 };
                if at % gap < width {
                    *v |= 1 << col;
                }
            }
        }
        b
    };
    match name {
        "pct5" => every(20),
        "pct10" => every(10),
        "pct20" => every(5),
        "pct25" => every(4),
        "pct30" => [0x55, 0x88, 0x55, 0x22, 0x55, 0x88, 0x55, 0x22],
        "pct40" => [0x55, 0xaa, 0x55, 0x8a, 0x55, 0xaa, 0x55, 0xa8],
        "pct50" => [0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa],
        "pct60" => [0x77, 0xaa, 0xdd, 0xaa, 0x77, 0xaa, 0xdd, 0xaa],
        "pct70" => [0x77, 0xdd, 0x77, 0xff, 0x77, 0xdd, 0x77, 0xff],
        "pct75" => [0x77, 0xff, 0xdd, 0xff, 0x77, 0xff, 0xdd, 0xff],
        "pct80" => [0x7f, 0xff, 0xf7, 0xff, 0x7f, 0xff, 0xf7, 0xff],
        "pct90" => [0x7f, 0xff, 0xff, 0xff, 0xf7, 0xff, 0xff, 0xff],
        "ltDnDiag" => diag(false, 1, 4),
        "ltUpDiag" => diag(true, 1, 4),
        "dkDnDiag" => diag(false, 2, 4),
        "dkUpDiag" => diag(true, 2, 4),
        "wdDnDiag" => diag(false, 3, 8),
        "wdUpDiag" => diag(true, 3, 8),
        "ltVert" => lines(true, 1, 4),
        "ltHorz" => lines(false, 1, 4),
        "narVert" => lines(true, 1, 2),
        "narHorz" => lines(false, 1, 2),
        "dkVert" => lines(true, 2, 4),
        "dkHorz" => lines(false, 2, 4),
        "dashDnDiag" => [0x11, 0x22, 0x44, 0x00, 0x11, 0x22, 0x44, 0x00],
        "dashUpDiag" => [0x88, 0x44, 0x22, 0x00, 0x88, 0x44, 0x22, 0x00],
        "dashHorz" => [0x0f, 0x00, 0x00, 0x00, 0xf0, 0x00, 0x00, 0x00],
        "dashVert" => [0x01, 0x01, 0x01, 0x01, 0x10, 0x10, 0x10, 0x10],
        "smConfetti" => [0x01, 0x10, 0x02, 0x40, 0x08, 0x80, 0x04, 0x20],
        "lgConfetti" => [0x03, 0x33, 0x0c, 0xc0, 0x30, 0x36, 0x0c, 0xc0],
        "zigZag" => [0x81, 0x42, 0x24, 0x18, 0x81, 0x42, 0x24, 0x18],
        "wave" => [0x00, 0x18, 0xa4, 0x03, 0x00, 0x18, 0xa4, 0x03],
        "diagBrick" => [0x01, 0x02, 0x04, 0x08, 0x18, 0x24, 0x42, 0x81],
        "horzBrick" => [0xff, 0x01, 0x01, 0x01, 0xff, 0x10, 0x10, 0x10],
        "weave" => [0x11, 0x2a, 0x44, 0xa2, 0x11, 0x28, 0x44, 0x8a],
        "plaid" => [0x55, 0xaa, 0x55, 0xaa, 0x0f, 0x0f, 0x0f, 0x0f],
        "divot" => [0x00, 0x08, 0x10, 0x08, 0x00, 0x80, 0x01, 0x80],
        "dotGrid" => [0x55, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00],
        "dotDmnd" => [0x01, 0x00, 0x44, 0x00, 0x10, 0x00, 0x44, 0x00],
        "shingle" => [0x03, 0x84, 0x48, 0x30, 0x0c, 0x02, 0x01, 0x01],
        "trellis" => [0xff, 0x66, 0xff, 0x99, 0xff, 0x66, 0xff, 0x99],
        "sphere" => [0x3c, 0x7e, 0xc3, 0x81, 0x81, 0xc3, 0x7e, 0x3c],
        "smGrid" => [0xff, 0x11, 0x11, 0x11, 0xff, 0x11, 0x11, 0x11],
        "lgGrid" => [0xff, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
        "smCheck" => [0x33, 0x33, 0xcc, 0xcc, 0x33, 0x33, 0xcc, 0xcc],
        "lgCheck" => [0x0f, 0x0f, 0x0f, 0x0f, 0xf0, 0xf0, 0xf0, 0xf0],
        "openDmnd" => [0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81],
        "solidDmnd" => [0x18, 0x3c, 0x7e, 0xff, 0x7e, 0x3c, 0x18, 0x00],
        _ => every(2),
    }
}

/// A pattern swatch: its 8 × 8 cell repeated at 2 device px per bit.
fn paint_pattern(p: &mut dyn Painter, r: [f32; 4], name: &str, fg: Rgba, bg: Rgba) {
    let bits = pattern_bits(name);
    p.fill_rect(r[0], r[1], r[2], r[3], bg);
    p.push_clip(r[0], r[1], r[2], r[3]);
    let s = (r[3] / 28.0).round().max(1.0);
    let nx = (r[2] / s).ceil() as usize;
    let ny = (r[3] / s).ceil() as usize;
    for j in 0..ny {
        let row = bits[j % 8];
        let mut i = 0;
        while i < nx {
            if row & (1 << (i % 8)) != 0 {
                // Merge runs of set bits into one rectangle.
                let start = i;
                while i < nx && row & (1 << (i % 8)) != 0 {
                    i += 1;
                }
                p.fill_rect(
                    r[0] + start as f32 * s,
                    r[1] + j as f32 * s,
                    (i - start) as f32 * s,
                    s,
                    fg,
                );
            } else {
                i += 1;
            }
        }
    }
    p.pop_clip();
}

/// Gradient stop marker: a tab with a point below, the stop colour inside, an accent edge
/// when selected.
fn paint_marker(p: &mut dyn Painter, r: [f32; 4], c: Rgba, sel: bool, scale: f32) {
    let [x, y, w, h] = r;
    let tip = (9.6 * scale).round();
    let (x0, x1, y0, y1) = (x + 1.5, x + w - 1.5, y + 0.5, y + h - tip);
    let shape = [
        PathSeg::M(x0, y0),
        PathSeg::L(x1, y0),
        PathSeg::L(x1, y1),
        PathSeg::L(x + w / 2.0, y + h - 0.5),
        PathSeg::L(x0, y1),
        PathSeg::Z,
    ];
    p.path(&shape, Some(WHITE), None);
    p.fill_rect(x0 + 1.5, y0 + 1.5, x1 - x0 - 3.0, y1 - y0 - 3.0, c);
    let edge = if sel {
        ACCENT
    } else {
        Rgba(0x40, 0x40, 0x40, 255)
    };
    p.path(&shape, None, Some((edge, if sel { 1.5 } else { 1.0 })));
}

/// Add / remove gradient stop buttons: a small stop marker with a plus or a cross (own
/// drawing).
fn paint_stop_button(p: &mut dyn Painter, r: [f32; 4], add: bool, enabled: bool, scale: f32) {
    let u = scale / 1.25;
    let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    let grey = if enabled {
        Rgba(0x8a, 0x8a, 0x8a, 255)
    } else {
        Rgba(0xc8, 0xc8, 0xc8, 255)
    };
    let mk = [cx - 2.0 * u, cy - 6.0 * u, 9.0 * u, 18.0 * u];
    paint_marker(p, mk, grey, false, scale * 0.5);
    let s = 4.0 * u;
    let (mx, my) = (cx - 6.0 * u, cy - 7.0 * u);
    if add {
        let c = if enabled {
            Rgba(0x3f, 0x9c, 0x6a, 255)
        } else {
            DIM
        };
        p.fill_rect(mx - s, my - 1.0 * u, 2.0 * s, 2.0 * u, c);
        p.fill_rect(mx - 1.0 * u, my - s, 2.0 * u, 2.0 * s, c);
    } else {
        let c = if enabled {
            Rgba(0xd2, 0x47, 0x26, 255)
        } else {
            DIM
        };
        p.path(
            &[
                PathSeg::M(mx - s, my - s),
                PathSeg::L(mx + s, my + s),
                PathSeg::M(mx + s, my - s),
                PathSeg::L(mx - s, my + s),
            ],
            None,
            Some((c, 2.0 * u)),
        );
    }
}

/// The Fill tab's icon: a paint pail tipped over with a drop (own drawing).
fn paint_bucket(p: &mut dyn Painter, r: [f32; 4], u: f32) {
    let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0 + 2.0 * u);
    let s = 11.0 * u;
    // The pail: a square turned 45°.
    let body = [
        PathSeg::M(cx, cy - s),
        PathSeg::L(cx + s, cy),
        PathSeg::L(cx, cy + s),
        PathSeg::L(cx - s, cy),
        PathSeg::Z,
    ];
    p.path(&body, Some(WHITE), Some((ACCENT, 2.0 * u)));
    // Handle over the top and the paint running off the right corner.
    p.path(
        &[
            PathSeg::M(cx - 4.0 * u, cy - s + 4.0 * u),
            PathSeg::C(
                cx - 4.0 * u,
                cy - s - 6.0 * u,
                cx + 4.0 * u,
                cy - s - 6.0 * u,
                cx + 4.0 * u,
                cy - s + 4.0 * u,
            ),
        ],
        None,
        Some((ACCENT, 1.5 * u)),
    );
    p.path(
        &[
            PathSeg::M(cx + s, cy),
            PathSeg::C(
                cx + s + 5.0 * u,
                cy + 2.0 * u,
                cx + s + 4.0 * u,
                cy + 9.0 * u,
                cx + s + 1.0 * u,
                cy + 12.0 * u,
            ),
            PathSeg::C(
                cx + s - 1.0 * u,
                cy + 8.0 * u,
                cx + s - 2.0 * u,
                cy + 4.0 * u,
                cx + s,
                cy,
            ),
            PathSeg::Z,
        ],
        Some(ACCENT),
        None,
    );
    p.path(&circle(cx, cy - 3.0 * u, 1.6 * u), Some(ACCENT), None);
}

/// Colour buttons' icon: a small pail over a bar of the colour.
fn paint_small_bucket(p: &mut dyn Painter, r: [f32; 4], c: Rgba, ink: Rgba) {
    let [x, y, w, h] = r;
    let u = w / 22.0;
    let (cx, cy) = (x + w / 2.0 - u, y + h / 2.0 - 3.0 * u);
    let s = 6.5 * u;
    p.path(
        &[
            PathSeg::M(cx, cy - s),
            PathSeg::L(cx + s, cy),
            PathSeg::L(cx, cy + s),
            PathSeg::L(cx - s, cy),
            PathSeg::Z,
        ],
        Some(WHITE),
        Some((ink, 1.0)),
    );
    p.path(
        &[
            PathSeg::M(cx + s, cy),
            PathSeg::C(
                cx + s + 3.0 * u,
                cy + u,
                cx + s + 2.0 * u,
                cy + 5.0 * u,
                cx + s + 0.5 * u,
                cy + 6.0 * u,
            ),
            PathSeg::Z,
        ],
        Some(Rgba(0x2b, 0x79, 0xc8, 255)),
        None,
    );
    p.fill_rect(x + u, y + h - 5.0 * u, w - 2.0 * u, 4.0 * u, c);
    if c.0 > 240 && c.1 > 240 && c.2 > 240 {
        outline(
            p,
            [x + u, y + h - 5.0 * u, w - 2.0 * u, 4.0 * u],
            Rgba(0xb0, 0xb0, 0xb0, 255),
            1.0,
        );
    }
}

fn paint_scrollbar(p: &mut dyn Painter, track: [f32; 4], thumb: [f32; 4], scale: f32) {
    let [x, y, w, h] = track;
    p.fill_rect(x, y, w, h, Rgba(0x80, 0x80, 0x80, 255));
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
    let k = (3.2 * scale).round();
    let ax = (x + w / 2.0).round();
    for (cy, up) in [(y + w / 2.0, true), (y + h - w / 2.0, false)] {
        let dy = if up { -k / 2.0 } else { k / 2.0 };
        p.path(
            &[
                PathSeg::M(ax - k, cy - dy),
                PathSeg::L(ax + k, cy - dy),
                PathSeg::L(ax, cy + dy),
                PathSeg::Z,
            ],
            Some(Rgba(0x20, 0x20, 0x20, 255)),
            None,
        );
    }
    p.fill_rect(
        x + 1.0,
        y + w - 1.0,
        w - 2.0,
        1.0,
        Rgba(0x80, 0x80, 0x80, 255),
    );
    p.fill_rect(
        x + 1.0,
        y + h - w,
        w - 2.0,
        1.0,
        Rgba(0x80, 0x80, 0x80, 255),
    );
    p.fill_rect(
        thumb[0] + 1.0,
        thumb[1],
        thumb[2] - 2.0,
        thumb[3],
        Rgba(0x80, 0x80, 0x80, 255),
    );
    p.fill_rect(
        thumb[0] + 2.0,
        thumb[1] + 1.0,
        thumb[2] - 4.0,
        thumb[3] - 2.0,
        Rgba(0xf0, 0xf0, 0xf0, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(kind: u8) -> BgPane {
        let mut pane = BgPane {
            shown: true,
            kind,
            ..BgPane::default()
        };
        pane.layout(
            [1135.0, 236.0, 357.0, 824.0],
            1.25,
            &[Rgba(0x44, 0x72, 0xc4, 255); 8],
        );
        pane
    }

    fn at(pane: &BgPane, ctl: Ctl) -> [f32; 4] {
        pane.placed
            .iter()
            .find(|p| p.ctl == ctl)
            .map(|p| p.rect)
            .expect("control")
    }

    #[test]
    fn layout_follows_the_reference_measurements() {
        let pane = pane(2);
        // Radios 36 px apart from 191 below the pane top (unscrolled), Insert… at 59.
        assert_eq!(pane.radios[0][1] + 15.0, 236.0 + 191.0);
        assert_eq!(pane.radios[1][1] - pane.radios[0][1], 36.0);
        let ins = at(&pane, Ctl::Insert);
        assert_eq!((ins[0], ins[2], ins[3]), (1135.0 + 59.0, 76.0, 35.0));
        // Spin boxes end 316 px from the pane's left edge.
        let s = at(&pane, Ctl::Spin("picT"));
        assert_eq!(s[0] + s[2], 1135.0 + 316.0);
        assert_eq!(pane.apply, [1150.0, 236.0 + 824.0 - 56.0, 114.0, 35.0]);
        // Tiled pictures overflow the viewport: the pane scrolls.
        assert!(pane.scrollbar().is_some());
    }

    #[test]
    fn kinds_build_engine_commands() {
        let mut p = pane(0);
        let r = p.radios[1];
        assert_eq!(p.press(r[0] + 5.0, r[1] + 5.0), Some(BgHit::Apply));
        assert_eq!(p.kind, 1);
        match p.take_command(false) {
            Some(MasterCommand::SetBackgroundGradient {
                stops, path: None, ..
            }) => {
                assert_eq!(stops.len(), 4);
            }
            other => panic!("{other:?}"),
        }
        // Picture or texture: the first texture is sent once, then only options.
        let r = p.radios[2];
        assert_eq!(p.press(r[0] + 5.0, r[1] + 5.0), Some(BgHit::Apply));
        match p.take_command(false) {
            Some(MasterCommand::SetBackgroundPicture {
                data: Some(_),
                tile: Some(t),
                ..
            }) => {
                assert_eq!((t.algn.as_str(), t.flip.as_str()), ("tl", "none"));
            }
            other => panic!("{other:?}"),
        }
        assert!(p.take_command(false).is_none());
        p.pic.has_picture = true;
        assert!(matches!(
            p.take_command(true),
            Some(MasterCommand::SetBackgroundPicture {
                data: None,
                all: true,
                ..
            })
        ));
        // Pattern.
        let r = p.radios[3];
        p.press(r[0] + 5.0, r[1] + 5.0);
        let c = at(&p, Ctl::Pattern(12));
        assert_eq!(p.press(c[0] + 2.0, c[1] + 2.0), Some(BgHit::Apply));
        assert!(matches!(
            p.take_command(false),
            Some(MasterCommand::SetBackgroundFill { preset: Some(ref n), .. }) if n == "ltDnDiag"
        ));
    }

    #[test]
    fn gradient_stops_add_remove_drag_and_type() {
        let mut p = pane(1);
        assert_eq!(p.grad.stops.len(), 4);
        let add = at(&p, Ctl::AddStop);
        assert_eq!(p.press(add[0] + 3.0, add[1] + 3.0), Some(BgHit::Apply));
        assert_eq!(p.grad.stops.len(), 5);
        // The new stop lies between the selected stop and the next one.
        assert_eq!(p.grad.stops[p.grad.sel].pos, 37.0);
        let rem = at(&p, Ctl::RemoveStop);
        p.press(rem[0] + 3.0, rem[1] + 3.0);
        assert_eq!(p.grad.stops.len(), 4);
        // Drag the first stop to the middle of the track.
        let m = p.markers()[0];
        assert_eq!(p.press(m[0] + m[2] / 2.0, m[1] + 4.0), Some(BgHit::Redraw));
        let track = at(&p, Ctl::StopTrack);
        assert_eq!(p.drag(track[0] + track[2] / 2.0, m[1]), Some(true));
        assert_eq!(p.release(), Some(BgHit::Apply));
        assert_eq!(p.grad.stops[p.grad.sel].pos, 50.0);
        assert!(p.grad.stops.windows(2).all(|w| w[0].pos <= w[1].pos));
        // Type a position.
        let pos = at(&p, Ctl::Spin("pos"));
        p.press(pos[0] + 5.0, pos[1] + 5.0);
        assert!(p.editing());
        assert!(p.text("2"));
        assert!(p.text("5"));
        assert_eq!(p.key(Key::Enter), Some(BgHit::Apply));
        assert_eq!(p.grad.stops[p.grad.sel].pos, 25.0);
        // Brightness and transparency arrows.
        let b = at(&p, Ctl::Spin("bright"));
        p.press(b[0] + b[2] - 3.0, b[1] + 3.0);
        assert_eq!(p.grad.stops[p.grad.sel].bright, 1.0);
        // Type: Radial; Direction offers five focus points.
        let ty = at(&p, Ctl::Combo(PopKind::Type));
        p.press(ty[0] + 5.0, ty[1] + 5.0);
        let cell = p.popup.as_ref().map(|q| q.cells[1].0).expect("list");
        assert_eq!(p.press(cell[0] + 3.0, cell[1] + 3.0), Some(BgHit::Apply));
        assert_eq!(p.grad.kind, 1);
        assert_eq!(p.dir_count(), 5);
        // Preset gradients: 30 tiles.
        let pg = at(&p, Ctl::Drop(PopKind::Presets));
        p.press(pg[0] + 5.0, pg[1] + 5.0);
        assert_eq!(p.popup.as_ref().map(|q| q.cells.len()), Some(30));
        // A press outside closes it.
        assert_eq!(p.press(10.0, 10.0), Some(BgHit::Redraw));
        assert!(p.popup.is_none());
    }

    #[test]
    fn sync_reads_the_slide_background() {
        use bo_model::slide::{Color, Fill, GradientFill, GradientShade, GradientStop};
        let mut p = pane(0);
        let g = Fill::Gradient(GradientFill {
            stops: vec![
                GradientStop {
                    pos: 0,
                    color: Color::srgb(0xFF_FFFF),
                },
                GradientStop {
                    pos: 100_000,
                    color: Color::srgb(0x00_0000),
                },
            ],
            shade: Some(GradientShade::Path {
                path: "rect".into(),
                fill_to: Some([0, 0, 100_000, 100_000]),
            }),
            ..GradientFill::default()
        });
        p.sync(3, Some(&g));
        assert_eq!((p.kind, p.grad.kind, p.grad.focus), (1, 2, 4));
        assert_eq!(p.synced, Some(3));
        p.sync(4, None);
        assert_eq!((p.kind, p.current.as_deref()), (0, None));
    }

    struct Mono;
    impl crate::TextMeasure for Mono {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (text.chars().count() as f32 * style.size * 0.5, style.size)
        }
    }

    #[test]
    fn measured_title_buttons_and_access_keys() {
        let mut p = pane(1);
        let t = |k: &str, f: &str| {
            if k == "slideNative.formatBackground" {
                "Format the Background of Slides".to_owned()
            } else {
                f.to_owned()
            }
        };
        p.measure(&Mono, &t);
        // 31 characters at 25 px × 0.5 overflow 250 px: cut with "...", fitting.
        let title = p.title.clone().expect("title");
        assert!(title.ends_with("...") && title.chars().count() as f32 * 12.5 <= 250.0);
        // Buttons follow their text: 12 + 22 px padding, 14 px apart.
        let s = (12.4f32 * 1.25).round();
        let tw = 12.0 * s * 0.5;
        assert_eq!(p.apply[2], tw + 22.0);
        assert_eq!(p.reset[0], p.apply[0] + p.apply[2] + 14.0);
        // "Gradient fill": G underlined at 0; "Transparency": T.
        let g = p
            .marks
            .iter()
            .find(|m| m.0 == "slideR10.bg.gradient")
            .expect("mark");
        assert_eq!((g.1, g.2), (0.0, s * 0.5));
        assert!(p.marks.iter().any(|m| m.0 == "slideR22.bg.reset"));
    }

    #[test]
    fn pattern_cells_and_palette_rows() {
        for name in PATTERNS {
            let bits = pattern_bits(name);
            assert!(bits.iter().any(|b| *b != 0), "{name}");
        }
        assert_eq!(
            pattern_bits("pct50")
                .iter()
                .map(|b| b.count_ones())
                .sum::<u32>(),
            32
        );
        // Shades darken white, lighten black.
        assert!(palette_tint(WHITE, 4).0 < 200);
        assert!(palette_tint(Rgba(0, 0, 0, 255), 0).0 > 100);
    }
}
