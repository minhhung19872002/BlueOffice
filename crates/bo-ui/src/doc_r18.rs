//! BlueDoc parity round 18 (docs/ui/REPORT.md "Vòng 108"): the window commands of the View
//! tab that need several windows in one process — New Window, View Side by Side with
//! Synchronous Scrolling, Reset Window Position — and Insert › Screenshot's gallery of
//! Available Windows (thumbnails the host draws) with Screen Clipping. A child module of the
//! shell.
use super::{DocWindow, Request};
use crate::input::Key;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::{Input, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

#[cfg(test)]
#[path = "doc_r18_tests.rs"]
mod tests;

/// Round-18 state of the window.
#[derive(Default)]
pub(super) struct R18 {
    /// View › View Side by Side is on.
    side: bool,
    /// Synchronous Scrolling is on (turned on with View Side by Side, as the reference).
    sync: bool,
    /// Insert › Screenshot's gallery while it is open.
    shots: Option<Shots>,
    /// Online Pictures › Insert: the download in progress (a worker thread).
    pub(super) download: Option<Download>,
}

/// A picture fetcher: the bytes and a file name of an address, None when it fails.
pub(super) type Fetch = fn(&str) -> Option<(Vec<u8>, String)>;

/// An Online Pictures download: the address, when it started, the worker's answer
/// (picture bytes and name, or `None` when it failed).
pub(super) struct Download {
    url: String,
    started: std::time::Instant,
    rx: std::sync::mpsc::Receiver<Option<(Vec<u8>, String)>>,
    /// The download failed: the panel says so until closed.
    failed: bool,
}

/// Fetch `url` (HTTPS through the transport of time-stamping, 16 MB / 30 s at most); the
/// picture bytes and a file name when it is a picture.
fn fetch_picture(url: &str) -> Option<(Vec<u8>, String)> {
    use bo_sign::http::HttpTransport as _;
    let t = bo_sign::http::UreqTransport::new(std::time::Duration::from_secs(30));
    let r = t.get(url).ok()?;
    if !(200..300).contains(&r.status) || bo_raster::transcode_to_png(&r.body).is_none() {
        return None;
    }
    let name = url
        .rsplit('/')
        .next()
        .filter(|n| n.contains('.'))
        .unwrap_or("Online picture.png")
        .to_owned();
    Some((r.body, name))
}

/// Insert › Screenshot: the Available Windows gallery.
pub(super) struct Shots {
    /// Window handle, title, thumbnail size in pixels (the host's picture `id` is the handle).
    items: Vec<(isize, String, (u32, u32))>,
    /// Panel rectangle (x, y, width, height) in device pixels.
    rect: [f32; 4],
    /// Item under the pointer: thumbnail index, or `items.len()` for Screen Clipping.
    hover: Option<usize>,
}

/// Shape Styles gallery entries: (fill, outline) — BlueOffice's own blue, orange and grey
/// with a darker outline of the same hue.
const SHAPE_STYLES: [(&str, &str); 3] = [
    ("4472C4", "2F528F"),
    ("ED7D31", "AE5A21"),
    ("A5A5A5", "787878"),
];

/// Colours of Shape Fill / Shape Outline (standard colours): (hex, key, English).
const SHAPE_COLORS: [(&str, &str, &str); 10] = [
    ("C00000", "doc18.cDarkRed", "Dark Red"),
    ("FF0000", "doc18.cRed", "Red"),
    ("FFC000", "doc18.cOrange", "Orange"),
    ("FFFF00", "doc18.cYellow", "Yellow"),
    ("92D050", "doc18.cLightGreen", "Light Green"),
    ("00B050", "doc18.cGreen", "Green"),
    ("00B0F0", "doc18.cLightBlue", "Light Blue"),
    ("0070C0", "doc18.cBlue", "Blue"),
    ("002060", "doc18.cDarkBlue", "Dark Blue"),
    ("7030A0", "doc18.cPurple", "Purple"),
];

/// Outline dashes (§20.1.10.48): (value, key, English).
const DASHES: [(&str, &str, &str); 6] = [
    ("solid", "doc18.dSolid", "Solid"),
    ("sysDot", "doc18.dRoundDot", "Round Dot"),
    ("sysDash", "doc18.dSquareDot", "Square Dot"),
    ("dash", "doc18.dDash", "Dash"),
    ("dashDot", "doc18.dDashDot", "Dash Dot"),
    ("lgDash", "doc18.dLongDash", "Long Dash"),
];

/// Edit Shape › Change Shape presets (§20.1.9.18): (preset, key, English).
const PRESETS: [(&str, &str, &str); 8] = [
    ("rect", "doc18.pRect", "Rectangle"),
    ("roundRect", "doc18.pRoundRect", "Rounded Rectangle"),
    ("ellipse", "doc18.pOval", "Oval"),
    ("triangle", "doc18.pTriangle", "Isosceles Triangle"),
    ("rightArrow", "doc18.pRightArrow", "Right Arrow"),
    ("hexagon", "doc18.pHexagon", "Hexagon"),
    ("star5", "doc18.pStar", "Star: 5 Points"),
    (
        "wedgeRectCallout",
        "doc18.pCallout",
        "Speech Bubble: Rectangle",
    ),
];

/// Commands of round 18 that are not dimmed.
pub(crate) const IDS: [&str; 3] = ["view.newWindow", "view.syncScroll", "r18.screenClip"];

// Sizes in device pixels at 125 %, measured on the reference's gallery (653 × 246 with
// four windows): square cells of 157 px edge to edge under a 35 px heading band, the
// Screen Clipping row below them, room for the size grip at the bottom.
const CELL: f32 = 157.0;
const PANEL_W: f32 = 653.0;
const HEADER_H: f32 = 35.0;
const CLIP_TOP: f32 = 3.0;
const CLIP_H: f32 = 36.0;
const BOTTOM: f32 = 14.0;
const PER_ROW: usize = 4;
/// Margin between a cell's edge and its thumbnail.
const INSET: f32 = 9.0;

const PANEL_BG: Rgba = Rgba(0xFF, 0xFF, 0xFF, 255);
const PANEL_EDGE: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const HEADER_BG: Rgba = Rgba(0xEB, 0xEB, 0xEB, 255);
const TEXT: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const HOVER_BG: Rgba = Rgba(0xE1, 0xDF, 0xDD, 255);
const HOVER_EDGE: Rgba = Rgba(0x00, 0x00, 0x00, 255);

impl DocWindow {
    fn r18_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// A round-18 command; `None` when `id` is not one.
    pub(super) fn r18_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "view.newWindow" => {
                self.requests.push(Request::NewWindow);
                false
            }
            "view.sideBySide" => {
                self.r18.side = !self.r18.side;
                self.r18.sync = self.r18.side;
                self.r18_link();
                true
            }
            "view.syncScroll" => {
                if !self.r18.side {
                    return Some(false);
                }
                self.r18.sync = !self.r18.sync;
                self.r18_link();
                true
            }
            // Reset Window Position: the two windows share the screen equally again.
            "view.resetWindow" => {
                self.requests.push(Request::SideBySide);
                false
            }
            "r18.screenClip" => {
                self.menu = None;
                self.r18.shots = None;
                self.requests.push(Request::ScreenClip);
                true
            }
            _ if crate::doc_scale18::is_group(id) => self.r18_open_group(id),
            "shape.fill" => self.r18_shape_menu("shape.fill"),
            "shape.outline" => self.r18_shape_menu("shape.outline"),
            "shape.edit" => self.r18_shape_menu("shape.edit"),
            _ if id.starts_with("shape.style.") => {
                let i: usize = id.trim_start_matches("shape.style.").parse().unwrap_or(0);
                let (fill, line) = SHAPE_STYLES[i.min(SHAPE_STYLES.len() - 1)];
                self.r18_shape(&serde_json::json!({ "cmd": "pictureShapeFill", "color": fill }))
                    && self.r18_shape(&serde_json::json!({
                        "cmd": "pictureShapeOutline", "color": line, "weight": 1.0
                    }))
            }
            _ if id.starts_with("r18.fill:") => self.r18_shape(&serde_json::json!({
                "cmd": "pictureShapeFill", "color": &id["r18.fill:".len()..]
            })),
            _ if id.starts_with("r18.olc:") => self.r18_shape(&serde_json::json!({
                "cmd": "pictureShapeOutline", "color": &id["r18.olc:".len()..]
            })),
            _ if id.starts_with("r18.olw:") => {
                let w: f32 = id["r18.olw:".len()..].parse().unwrap_or(1.0);
                self.r18_shape(&serde_json::json!({ "cmd": "pictureShapeOutline", "weight": w }))
            }
            _ if id.starts_with("r18.old:") => self.r18_shape(&serde_json::json!({
                "cmd": "pictureShapeOutline", "dash": &id["r18.old:".len()..]
            })),
            _ if id.starts_with("r18.preset:") => self.r18_shape(&serde_json::json!({
                "cmd": "pictureShapePreset", "preset": &id["r18.preset:".len()..]
            })),
            // Online Pictures › Insert: the download runs on a worker thread while a panel
            // shows its progress (Cancel stops waiting for it).
            _ if id.starts_with("r17.picUrl:") => {
                let url = id.trim_start_matches("r17.picUrl:").to_owned();
                self.r18_start_download(url, fetch_picture)
            }
            _ => return None,
        })
    }

    /// Open (or close) the drop-down panel of a group collapsed by round 18's scaling.
    fn r18_open_group(&mut self, id: &str) -> bool {
        let Some((sid, tab, label)) = crate::doc_scale18::group(id) else {
            return false;
        };
        if self.r16.pop.as_ref().is_some_and(|p| p.id == sid) {
            self.r16.pop = None;
            return true;
        }
        let Some(group) = crate::ribbon::doc_group(tab, label) else {
            return false;
        };
        let anchor = self.ribbon.rect_of(RibbonHit::Command(sid)).unwrap_or([
            0.0,
            self.ribbon.bottom(),
            1.0,
            0.0,
        ]);
        self.menu = None;
        self.r16.pop = Some(crate::doc_grouppop16::GroupPopup::new(
            sid, tab, group, anchor,
        ));
        true
    }

    /// Run a Shape Format command on the selected shape.
    fn r18_shape(&mut self, v: &serde_json::Value) -> bool {
        self.menu = None;
        let ok = self.view.picture_command(v);
        self.sync_context();
        ok
    }

    /// Shape Fill ▾, Shape Outline ▾, Edit Shape ▾ (Change Shape) of the Shape Format tab.
    fn r18_shape_menu(&mut self, id: &'static str) -> bool {
        let item = |label: String, cmd: String| MenuItem {
            label,
            choice: Choice::Command(cmd),
            checked: false,
            dim: false,
        };
        let mut items = Vec::new();
        match id {
            "shape.fill" => {
                items.push(item(
                    self.r18_t("doc18.noFill", "No Fill"),
                    "r18.fill:none".into(),
                ));
                for (hex, key, en) in SHAPE_COLORS {
                    items.push(item(self.r18_t(key, en), format!("r18.fill:{hex}")));
                }
            }
            "shape.outline" => {
                items.push(item(
                    self.r18_t("doc18.noOutline", "No Outline"),
                    "r18.olc:none".into(),
                ));
                for (hex, key, en) in SHAPE_COLORS {
                    items.push(item(self.r18_t(key, en), format!("r18.olc:{hex}")));
                }
                for w in ["0.25", "0.5", "0.75", "1", "1.5", "2.25", "3", "4.5", "6"] {
                    items.push(item(format!("{w} pt"), format!("r18.olw:{w}")));
                }
                for (dash, key, en) in DASHES {
                    items.push(item(self.r18_t(key, en), format!("r18.old:{dash}")));
                }
            }
            _ => {
                for (preset, key, en) in PRESETS {
                    items.push(item(self.r18_t(key, en), format!("r18.preset:{preset}")));
                }
            }
        }
        let anchor = self.ribbon.rect_of(RibbonHit::Command(id)).unwrap_or([
            self.size.0 / 2.0,
            120.0,
            60.0,
            30.0,
        ]);
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    fn r18_link(&mut self) {
        self.requests.push(Request::SideBySideLink {
            side: self.r18.side,
            sync: self.r18.sync,
        });
    }

    /// The host ended View Side by Side (the other window closed, or the user turned it off
    /// there): the buttons come up again.
    pub fn set_side_by_side(&mut self, side: bool, sync: bool) {
        self.r18.side = side;
        self.r18.sync = side && sync;
    }

    /// Whether View Side by Side and Synchronous Scrolling are on.
    pub fn side_by_side(&self) -> (bool, bool) {
        (self.r18.side, self.r18.sync)
    }

    pub(super) fn r18_on_ids(&self) -> Vec<&'static str> {
        let mut on = Vec::new();
        if self.r18.side {
            on.push("view.sideBySide");
        }
        if self.r18.sync {
            on.push("view.syncScroll");
        }
        on
    }

    /// View tab: View Side by Side needs two windows; Synchronous Scrolling and Reset Window
    /// Position only work while it is on (as the reference).
    pub(super) fn r18_layout(&mut self) {
        if self.ribbon.active_label() != "ribbon.tabs.view" {
            return;
        }
        let dim = if self.r17.count < 2 {
            vec!["view.sideBySide", "view.resetWindow", "view.syncScroll"]
        } else if self.r18.side {
            Vec::new()
        } else {
            vec!["view.resetWindow", "view.syncScroll"]
        };
        self.ribbon.set_extra_dim(dim);
    }

    /// Insert › Screenshot: show the Available Windows the host listed as a gallery of
    /// thumbnails (pictures `handle` the host registered for the painter) with Screen
    /// Clipping below.
    pub fn show_screenshot_gallery(&mut self, items: Vec<(isize, String, (u32, u32))>) -> bool {
        let s = self.scale / 1.25;
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("insert.screenshot"))
            .unwrap_or([self.size.0 / 2.0, 120.0, 60.0, 90.0]);
        let rows = items.len().div_ceil(PER_ROW).max(1) as f32;
        let w = (PANEL_W * s).round();
        let h = ((HEADER_H + 1.0 + rows * CELL + CLIP_TOP + CLIP_H + BOTTOM) * s).round();
        let x = anchor[0].min(self.size.0 - w).max(0.0);
        let y = (anchor[1] + anchor[3]).min(self.size.1 - h).max(0.0);
        self.menu = None;
        self.r18.shots = Some(Shots {
            items,
            rect: [x, y, w, h],
            hover: None,
        });
        true
    }

    /// Whether the Screenshot gallery is open.
    pub fn screenshot_gallery_open(&self) -> bool {
        self.r18.shots.is_some()
    }

    /// Rectangles of the gallery's thumbnail cells and of the Screen Clipping row.
    fn r18_cells(&self, g: &Shots) -> (Vec<[f32; 4]>, [f32; 4]) {
        let s = self.scale / 1.25;
        let [x, y, w, h] = g.rect;
        let cells = (0..g.items.len())
            .map(|i| {
                let (c, r) = ((i % PER_ROW) as f32, (i / PER_ROW) as f32);
                [
                    x + (2.0 + c * CELL) * s,
                    y + (HEADER_H + 1.0 + r * CELL) * s,
                    CELL * s,
                    CELL * s,
                ]
            })
            .collect();
        let clip = [
            x + s,
            y + h - (CLIP_H + BOTTOM) * s,
            w - 2.0 * s,
            CLIP_H * s,
        ];
        (cells, clip)
    }

    fn r18_hit(&self, g: &Shots, px: f32, py: f32) -> Option<usize> {
        let inside =
            |r: &[f32; 4]| px >= r[0] && px < r[0] + r[2] && py >= r[1] && py < r[1] + r[3];
        let (cells, clip) = self.r18_cells(g);
        if inside(&clip) {
            return Some(g.items.len());
        }
        cells.iter().position(inside)
    }

    /// Pointer and keys while the Screenshot gallery is open.
    pub(super) fn r18_input(&mut self, input: &Input) -> Option<bool> {
        if self.r18.download.is_some() {
            return self.r18_download_input(input);
        }
        let g = self.r18.shots.as_ref()?;
        match *input {
            Input::Move { x, y } => {
                let hover = self.r18_hit(g, x, y);
                let g = self.r18.shots.as_mut()?;
                let changed = g.hover != hover;
                g.hover = hover;
                Some(changed)
            }
            Input::Press { x, y, .. } => {
                let [gx, gy, gw, gh] = g.rect;
                let hit = self.r18_hit(g, x, y);
                let n = g.items.len();
                let chosen = hit.map(|i| g.items.get(i).map(|w| w.0));
                if x < gx || x >= gx + gw || y < gy || y >= gy + gh {
                    // A click outside closes the gallery (and does nothing else).
                    self.r18.shots = None;
                    return Some(true);
                }
                match (hit, chosen) {
                    (Some(i), _) if i == n => {
                        self.r18_action("r18.screenClip");
                    }
                    (Some(_), Some(Some(h))) => {
                        self.r18.shots = None;
                        self.requests.push(Request::CaptureWindow(h));
                    }
                    _ => {}
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.r18.shots = None;
                Some(true)
            }
            Input::Release { .. }
            | Input::Drag { .. }
            | Input::Wheel(_)
            | Input::Context { .. }
            | Input::Key(..)
            | Input::Text(_) => Some(false),
            _ => None,
        }
    }

    /// Start downloading `url` with `fetch` on a worker thread.
    pub(super) fn r18_start_download(&mut self, url: String, fetch: Fetch) -> bool {
        let (tx, rx) = std::sync::mpsc::channel();
        let job = url.clone();
        let spawned = std::thread::Builder::new()
            .name("online-picture".into())
            .spawn(move || {
                let _ = tx.send(fetch(&job));
            });
        if spawned.is_err() {
            return false;
        }
        self.r18.download = Some(Download {
            url,
            started: std::time::Instant::now(),
            rx,
            failed: false,
        });
        true
    }

    /// The download panel's rectangle and its Cancel (or Close) button's.
    fn r18_download_rects(&self) -> ([f32; 4], [f32; 4]) {
        let s = self.scale / 1.25;
        let (w, h) = (420.0 * s, 150.0 * s);
        let x = ((self.size.0 - w) / 2.0).round();
        let y = ((self.size.1 - h) / 2.0).round();
        let b = [x + w - 112.0 * s, y + h - 46.0 * s, 100.0 * s, 32.0 * s];
        ([x, y, w, h], b)
    }

    /// Input while an Online Pictures download runs: ticks look for its end, Cancel / Esc
    /// stop waiting, everything else waits (the panel is modal like the reference's).
    fn r18_download_input(&mut self, input: &Input) -> Option<bool> {
        let (_, b) = self.r18_download_rects();
        match *input {
            Input::Tick | Input::Blink => {
                let d = self.r18.download.as_mut()?;
                if d.failed {
                    return None;
                }
                match d.rx.try_recv() {
                    Ok(Some((bytes, name))) => {
                        self.r18.download = None;
                        self.view.insert_picture(bytes, &name);
                        self.sync_context();
                        Some(true)
                    }
                    Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        d.failed = true;
                        Some(true)
                    }
                    // Still downloading: the bar moves.
                    Err(std::sync::mpsc::TryRecvError::Empty) => Some(true),
                }
            }
            Input::Press { x, y, .. } => {
                if x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3] {
                    self.r18.download = None;
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.r18.download = None;
                Some(true)
            }
            Input::Key(..)
            | Input::Text(_)
            | Input::Release { .. }
            | Input::Drag { .. }
            | Input::Wheel(_)
            | Input::Context { .. } => Some(false),
            _ => None,
        }
    }

    /// Whether an Online Pictures download is in progress (or its failure is shown).
    pub fn downloading(&self) -> bool {
        self.r18.download.is_some()
    }

    fn r18_paint_download(&self, p: &mut dyn Painter) {
        let Some(d) = &self.r18.download else { return };
        let s = self.scale / 1.25;
        let ([x, y, w, h], b) = self.r18_download_rects();
        let px = s.max(1.0).round();
        p.fill_rect(x, y, w, h, PANEL_EDGE);
        p.fill_rect(x + px, y + px, w - 2.0 * px, h - 2.0 * px, PANEL_BG);
        let bold = TextStyle {
            size: (15.0 * s).round(),
            bold: true,
        };
        let plain = TextStyle {
            size: (14.0 * s).round(),
            bold: false,
        };
        let title = if d.failed {
            self.r18_t(
                "doc18.downloadFailed",
                "The picture could not be downloaded",
            )
        } else {
            self.r18_t("doc18.downloading", "Downloading picture...")
        };
        p.text(&title, x + 16.0 * s, y + 14.0 * s, bold, TEXT);
        let url: String = d.url.chars().take(56).collect();
        p.text(&url, x + 16.0 * s, y + 42.0 * s, plain, TEXT);
        if !d.failed {
            // An indeterminate bar: a block sliding along the track.
            let (tx, ty, tw, th) = (x + 16.0 * s, y + 70.0 * s, w - 32.0 * s, 8.0 * s);
            p.fill_rect(tx, ty, tw, th, Rgba(0xE6, 0xE6, 0xE6, 255));
            let t = d.started.elapsed().as_secs_f32();
            let k = (t * 0.6).fract();
            let bw = tw * 0.25;
            let bx = tx + (tw + bw) * k - bw;
            let (l, r) = (bx.max(tx), (bx + bw).min(tx + tw));
            if r > l {
                p.fill_rect(l, ty, r - l, th, Rgba(0x00, 0x5F, 0xB8, 255));
            }
        }
        p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0xC6, 0xC6, 0xC6, 255));
        p.fill_rect(
            b[0] + px,
            b[1] + px,
            b[2] - 2.0 * px,
            b[3] - 2.0 * px,
            Rgba(0xFD, 0xFD, 0xFD, 255),
        );
        let label = if d.failed {
            self.r18_t("doc18.close", "Close")
        } else {
            self.r18_t("doc18.cancel", "Cancel")
        };
        p.text(&label, b[0] + 26.0 * s, b[1] + 7.0 * s, plain, TEXT);
    }

    /// Draw the Screenshot gallery over the window.
    pub(super) fn r18_paint(&self, p: &mut dyn Painter) {
        self.r18_paint_download(p);
        let Some(g) = &self.r18.shots else { return };
        let s = self.scale / 1.25;
        let [x, y, w, h] = g.rect;
        let px = s.max(1.0).round();
        p.fill_rect(x, y, w, h, PANEL_EDGE);
        p.fill_rect(x + px, y + px, w - 2.0 * px, h - 2.0 * px, PANEL_BG);
        // Heading band.
        p.fill_rect(x + px, y + px, w - 2.0 * px, HEADER_H * s - px, HEADER_BG);
        let style = TextStyle {
            size: (15.0 * s).round(),
            bold: true,
        };
        let head = self.r18_t("doc18.availableWindows", "Available Windows");
        p.text(&head, x + 13.0 * s, y + 8.0 * s, style, TEXT);
        let (cells, clip) = self.r18_cells(g);
        let plain = TextStyle {
            size: (15.0 * s).round(),
            bold: false,
        };
        if g.items.is_empty() {
            let none = self.r18_t("doc18.noWindows", "No windows are available");
            p.text(&none, x + 10.0 * s, y + (HEADER_H + 10.0) * s, plain, TEXT);
        }
        for (i, (cell, (h, _, (tw, th)))) in cells.iter().zip(&g.items).enumerate() {
            if g.hover == Some(i) {
                let b = 2.0 * px;
                p.fill_rect(cell[0], cell[1], cell[2], cell[3], HOVER_EDGE);
                p.fill_rect(
                    cell[0] + b,
                    cell[1] + b,
                    cell[2] - 2.0 * b,
                    cell[3] - 2.0 * b,
                    HOVER_BG,
                );
            }
            // The thumbnail fits the cell inside the margin, keeping its shape.
            let (aw, ah) = (cell[2] - 2.0 * INSET * s, cell[3] - 2.0 * INSET * s);
            let k = (aw / *tw as f32).min(ah / *th as f32);
            let (dw, dh) = ((*tw as f32 * k).round(), (*th as f32 * k).round());
            let dx = (cell[0] + (cell[2] - dw) / 2.0).round();
            let dy = (cell[1] + (cell[3] - dh) / 2.0).round();
            p.image(*h as u64, [dx, dy, dw, dh]);
        }
        // Screen Clipping with its icon, then the size grip's dots.
        if g.hover == Some(g.items.len()) {
            p.fill_rect(clip[0], clip[1], clip[2], clip[3], HOVER_BG);
        }
        paint_clip_icon(p, clip[0] + 7.0 * s, clip[1] + 9.0 * s, 20.0 * s);
        let label = self.r18_t("doc18.screenClipping", "Screen Clipping");
        p.text(&label, clip[0] + 45.0 * s, clip[1] + 7.0 * s, plain, TEXT);
        let grip = Rgba(0xA0, 0xA0, 0xA0, 255);
        for (gx, gy) in [(1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            p.fill_rect(
                x + w - (10.0 - 4.0 * gx) * s,
                y + h - (10.0 - 4.0 * gy) * s,
                2.0 * s,
                2.0 * s,
                grip,
            );
        }
    }
}

/// Screen Clipping's icon (BlueOffice's own drawing): a dashed frame with a cross-hair at
/// its corner, `size` px square.
fn paint_clip_icon(p: &mut dyn Painter, x: f32, y: f32, size: f32) {
    let k = size / 16.0;
    let dash = Rgba(0x44, 0x44, 0x44, 255);
    let blue = Rgba(0x2B, 0x57, 0x9A, 255);
    for i in 0..4 {
        let t = i as f32 * 3.5 * k;
        p.fill_rect(x + t, y + k, 2.0 * k, k, dash);
        p.fill_rect(x + t, y + 12.0 * k, 2.0 * k, k, dash);
        p.fill_rect(x, y + k + t, k, 2.0 * k, dash);
        p.fill_rect(x + 13.0 * k, y + k + t, k, 2.0 * k, dash);
    }
    let (cx, cy) = (x + 13.5 * k, y + 12.5 * k);
    p.path(
        &[
            PathSeg::M(cx - 4.0 * k, cy),
            PathSeg::L(cx + 2.5 * k, cy),
            PathSeg::M(cx, cy - 4.0 * k),
            PathSeg::L(cx, cy + 2.5 * k),
        ],
        None,
        Some((blue, 1.5 * k)),
    );
}
