//! BlueDoc round 18 (docs/ui/REPORT.md "Vòng 108"): several windows of one document in this
//! process. View › New Window opens a second window on the same document ("name:1" /
//! "name:2" as the reference titles them): one [`Engine`] serves every window and moves to
//! the window being used, each window keeps its own selection, scroll, zoom and page
//! images, and an edit in one shows in the others. View Side by Side links two windows for
//! Synchronous Scrolling (also with a BlueDoc window of another process, by a registered
//! message); Insert › Screenshot shows thumbnails of the other programs' windows and
//! Screen Clipping. A child module of the application.
// Window indices and pixel loops read best with single letters.
#![allow(clippy::many_single_char_names)]
use super::{
    blocks_path, host6, keys_path, printers6, refresh_text_scale, watermarks_path, Gpu, Measure,
    Pages, Shell,
};
use crate::win32::{self, Handler};
use bo_engine::Engine;
use bo_gpu_win::{Device, TextRendering, WindowSurface};
use bo_ui::{DocView, DocWindow, Input, NcHit, Request};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// What a new window of the document is set up with.
pub(super) struct Setup {
    /// Interface language.
    pub locale: String,
    /// Name of the document as opened (titles while one window shows it).
    pub name: String,
}

/// The application's windows: every one shows the same document.
pub(super) struct Host {
    shells: Vec<Shell>,
    setup: Setup,
    /// The window whose view holds the document's engine (the others hold an empty one).
    owner: usize,
    /// Version of the document after the last edit, and the version each window last drew.
    version: u64,
    seen: Vec<u64>,
    /// When the document last changed (other windows catch up once typing pauses).
    last_edit: Instant,
    /// View Side by Side of each window: the linked window and whether Synchronous
    /// Scrolling is on.
    links: Vec<Option<(isize, bool)>>,
}

/// Inputs after which window sizes are not checked (typing stays as fast as before).
enum InputKind {
    Typing,
    Other,
}

/// Largest Available Windows thumbnail (pixels; the gallery scales it into its cell).
const THUMB_MAX: (u32, u32) = (139, 139);

/// Other windows catch up with an edit after this pause (typing goes on undisturbed).
const CATCH_UP: Duration = Duration::from_millis(300);

impl Host {
    pub(super) fn boxed(setup: Setup, first: Shell) -> Box<dyn Handler> {
        Box::new(Self {
            shells: vec![first],
            setup,
            owner: 0,
            version: 0,
            seen: vec![0],
            last_edit: Instant::now(),
            links: vec![None],
        })
    }

    /// The window of the message being handled.
    fn idx(&self) -> usize {
        let h = win32::current_window();
        self.shells.iter().position(|s| s.hwnd == h).unwrap_or(0)
    }

    /// Give the document's engine to window `i` (its selection comes back, its pages catch
    /// up with edits made in other windows).
    fn acquire(&mut self, i: usize) {
        if i == self.owner || i >= self.shells.len() {
            return;
        }
        let o = self.owner;
        let (a, b) = if i < o {
            let (l, r) = self.shells.split_at_mut(o);
            (&mut l[i], &mut r[0])
        } else {
            let (l, r) = self.shells.split_at_mut(i);
            (&mut r[0], &mut l[o])
        };
        a.view.view_mut().swap_engine(b.view.view_mut());
        let changed = self.seen[i] != self.version;
        a.view.view_mut().adopt_engine(changed);
        if changed {
            let m = Measure(a.gpu.as_ref().map(|g| &g.device));
            a.view.refresh(&m);
            a.dirty = true;
        }
        self.seen[i] = self.version;
        self.owner = i;
    }

    /// Note an edit made through window `i`'s view.
    fn note_version(&mut self) {
        let v = self.shells[self.owner].view.view().doc_version();
        if v != self.version {
            self.version = v;
            self.seen[self.owner] = v;
            self.last_edit = Instant::now();
        }
    }

    /// Titles "name:1", "name:2"… while several windows show the document, else "name".
    fn retitle(&mut self) {
        let n = self.shells.len();
        let base = self.shells[0]
            .file
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(
                || self.setup.name.clone(),
                |f| f.to_string_lossy().into_owned(),
            );
        for (k, s) in self.shells.iter_mut().enumerate() {
            let name = if n > 1 {
                format!("{base}:{}", k + 1)
            } else {
                base.clone()
            };
            s.view.set_name(&name);
            win32::set_title(s.hwnd, &format!("{name} \u{2014} BlueDoc"));
            s.view
                .set_window_count(win32::app_windows(" \u{2014} BlueDoc").len().max(n));
            let m = Measure(s.gpu.as_ref().map(|g| &g.device));
            s.view.refresh(&m);
            s.dirty = true;
        }
    }

    /// View › New Window from window `i`.
    fn new_window(&mut self, i: usize) {
        self.acquire(i);
        let real = self.shells[i].view.view_mut().replace_engine(Engine::new());
        let view = match DocView::new(real) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("new window: {e}");
                return;
            }
        };
        let mut win = DocWindow::new(view, &self.setup.locale, &self.setup.name);
        configure(&mut win, self.shells[i].file.as_deref());
        let (place, max) = win32::multi18::window_place(self.shells[i].hwnd);
        // The new window opens a little lower and to the right, like the reference's.
        let place = [place[0] + 32, place[1] + 32, place[2], place[3]];
        let title = format!("{} \u{2014} BlueDoc", self.setup.name);
        let (hwnd, w, h, scale) = match win32::multi18::open_window(&title, Some(place), max) {
            Ok(r) => r,
            Err(e) => {
                // The engine goes back to the window it came from.
                let mut back = win;
                let real = back.view_mut().replace_engine(Engine::new());
                self.shells[i].view.view_mut().replace_engine(real);
                eprintln!("new window: {e}");
                return;
            }
        };
        let mut shell = Shell {
            view: win,
            gpu: None,
            perf: None,
            file: self.shells[i].file.clone(),
            pending: Vec::new(),
            dirty: true,
            pending_input: None,
            last_frame: [Duration::ZERO; 3],
            clock: Instant::now(),
            hwnd: 0,
        };
        if let Err(e) = init_window(&mut shell, hwnd, w, h, scale) {
            eprintln!("new window: {e}");
        }
        self.shells.push(shell);
        self.seen.push(self.version);
        self.links.push(None);
        // The new window holds the engine now; the first one gets it back when used.
        self.owner = self.shells.len() - 1;
        self.retitle();
    }

    /// Host requests of window `i` (those its own shell leaves to the host).
    fn host_requests(&mut self, i: usize) {
        let pending = std::mem::take(&mut self.shells[i].pending);
        let mut rest = Vec::new();
        for r in pending {
            match r {
                Request::NewWindow => self.new_window(i),
                Request::SideBySideLink { side, sync } => self.link(i, side, sync),
                Request::ScreenClip => self.screen_clip(i),
                other => rest.push(other),
            }
        }
        self.shells[i].pending.extend(rest);
    }

    /// View Side by Side from window `i`: on, it and the next BlueDoc window share the
    /// screen; Synchronous Scrolling scrolls the other with it.
    fn link(&mut self, i: usize, side: bool, sync: bool) {
        let own = self.shells[i].hwnd;
        let old = self.links[i].map(|l| l.0);
        if !side {
            self.links[i] = None;
            if let Some(j) = old.and_then(|h| self.shells.iter().position(|s| s.hwnd == h)) {
                self.links[j] = None;
                self.shells[j].view.set_side_by_side(false, false);
                self.shells[j].dirty = true;
            }
            return;
        }
        let partner = old.or_else(|| {
            // Another window of this document first, else another BlueDoc window.
            self.shells
                .iter()
                .map(|s| s.hwnd)
                .find(|h| *h != own)
                .or_else(|| {
                    win32::app_windows(" \u{2014} BlueDoc")
                        .into_iter()
                        .map(|w| w.0)
                        .find(|h| *h != own)
                })
        });
        let Some(partner) = partner else {
            self.shells[i].view.set_side_by_side(false, false);
            return;
        };
        if old.is_none() {
            win32::arrange_windows(&[own, partner], false);
        }
        self.links[i] = Some((partner, sync));
        if let Some(j) = self.shells.iter().position(|s| s.hwnd == partner) {
            self.links[j] = Some((own, sync));
            self.shells[j].view.set_side_by_side(true, sync);
            self.shells[j].dirty = true;
        }
    }

    /// Insert › Screenshot › Screen Clipping from window `i`.
    fn screen_clip(&mut self, i: usize) {
        let own: Vec<isize> = std::iter::once(self.shells[i].hwnd)
            .chain(
                self.shells
                    .iter()
                    .map(|s| s.hwnd)
                    .filter(|h| *h != self.shells[i].hwnd),
            )
            .collect();
        let clip = win32::multi18::screen_clip(&own);
        for s in &mut self.shells {
            s.dirty = true;
        }
        if std::env::var_os("BLUEOFFICE_NATIVE_DEBUG").is_some() {
            eprintln!("screen clip: {:?}", clip.as_ref().map(|c| (c.0, c.1)));
        }
        let Some((w, h, px)) = clip else { return };
        let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
            width: w,
            height: h,
            pixels: px,
        });
        if let Some(png) = png {
            self.acquire(i);
            let s = &mut self.shells[i];
            let ok = s.view.insert_picture(png, "Screen Clipping.png");
            if std::env::var_os("BLUEOFFICE_NATIVE_DEBUG").is_some() {
                eprintln!("screen clip inserted: {ok}");
            }
            let m = Measure(s.gpu.as_ref().map(|g| &g.device));
            s.view.refresh(&m);
            self.note_version();
        }
    }

    /// Windows resized while the host was busy (View Side by Side, Arrange All move them
    /// during a command, when their resize message cannot reach the host) catch up.
    fn fix_sizes(&mut self) {
        for k in 0..self.shells.len() {
            let (w, h) = win32::multi18::client_size(self.shells[k].hwnd);
            let known = self.shells[k].gpu.as_ref().map(|g| g.surface.size());
            if w > 0 && h > 0 && known.is_some_and(|s| s != (w, h)) {
                self.acquire(k);
                self.shells[k].resized(w, h);
            }
        }
    }

    /// Synchronous Scrolling: tell the linked window how far window `i` scrolled.
    fn sync_after(&self, i: usize, before: f32) {
        if let Some((partner, true)) = self.links[i] {
            let dy = self.shells[i].view.view().scroll_y() - before;
            if dy.abs() > 0.01 {
                win32::multi18::post_sync_scroll(partner, dy);
            }
        }
    }
}

/// Set up a new window's interface like the first one (`app::run`): preferences, recent
/// files, file details, printers, units, the user's building blocks, watermarks and keys.
fn configure(view: &mut DocWindow, file: Option<&std::path::Path>) {
    view.set_utc_offset_minutes(win32::utc_offset_minutes());
    let prefs = host6::read_prefs();
    view.set_prefs(&prefs);
    view.set_recent(host6::read_recent());
    if let Some(p) = file {
        view.set_file_info(Some(&p.to_string_lossy()), host6::file_size(p));
        view.set_file_details(host6::file_details(p));
    }
    let (printers, default_printer) = host6::printers();
    view.set_printer_states(printers.iter().map(|n| printers6::state(n)).collect());
    view.set_printers(printers, default_printer);
    if let Some(v) = super::defaults_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        view.apply_interface_defaults(&v);
    }
    if let Some(bytes) = blocks_path().and_then(|p| std::fs::read(p).ok()) {
        use base64::Engine as _;
        view.load_block_store(&base64::engine::general_purpose::STANDARD.encode(bytes));
    }
    if let Some(text) = watermarks_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        view.load_watermarks(&text);
    }
    if let Some(text) = keys_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        view.load_key_bindings(&text);
    }
    let inches = match std::env::var("BLUEOFFICE_UNITS").as_deref() {
        Ok("in") => true,
        Ok("cm") => false,
        _ => win32::region_value("iMeasure").as_deref() == Some("1"),
    };
    let decimal = win32::region_value("sDecimal")
        .and_then(|s| s.chars().next())
        .unwrap_or('.');
    view.set_units(inches, decimal);
}

/// The GPU part of `Shell::created` for a window opened by the host (no start-up actions).
fn init_window(
    s: &mut Shell,
    hwnd: isize,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<(), String> {
    s.hwnd = hwnd;
    refresh_text_scale();
    let device = Device::new()?;
    let _ = device.update_window_monitor(hwnd);
    device.set_text_rendering(match std::env::var("BLUEOFFICE_TEXT").as_deref() {
        Ok("system") => TextRendering::System,
        Ok("light") => TextRendering::light_cleartype(),
        _ => TextRendering::document_default(),
    });
    let surface = WindowSurface::new(&device, hwnd, width, height)?;
    s.gpu = Some(Gpu {
        device,
        surface,
        pages: Pages {
            bitmaps: HashMap::new(),
            images: HashMap::new(),
        },
    });
    s.view.set_ole_drag(true);
    let m = Measure(s.gpu.as_ref().map(|g| &g.device));
    s.view.handle(Input::Scale(scale), &m);
    s.view.handle(
        Input::Resize {
            width: width as f32,
            height: height as f32,
        },
        &m,
    );
    s.view.refresh(&m);
    Ok(())
}

impl Shell {
    /// Insert › Screenshot: thumbnails of the other programs' windows as a gallery.
    pub(super) fn screenshot_gallery(&mut self) -> bool {
        let list = win32::screenshot_windows(self.hwnd);
        let mut items = Vec::new();
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.pages.images.clear();
            for (h, title) in list {
                let Some((w, hh, px)) = win32::capture_window(h) else {
                    continue;
                };
                // Windows the system draws elsewhere (cloaked, another desktop) come back black.
                if px
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[0] < 8 && p[1] < 8 && p[2] < 8)
                {
                    continue;
                }
                let (tw, th, thumb) = shrink(w, hh, &px, THUMB_MAX);
                if let Ok(bmp) = gpu.device.create_image(tw, th, &thumb) {
                    gpu.pages.images.insert(h as u64, bmp);
                    items.push((h, title, (tw, th)));
                }
            }
        }
        self.view.show_screenshot_gallery(items)
    }
}

/// Shrink an RGBA picture to fit `max` (box filter; never enlarged).
fn shrink(w: u32, h: u32, px: &[u8], max: (u32, u32)) -> (u32, u32, Vec<u8>) {
    let k = (max.0 as f32 / w as f32)
        .min(max.1 as f32 / h as f32)
        .min(1.0);
    let (tw, th) = (
        ((w as f32 * k).round() as u32).max(1),
        ((h as f32 * k).round() as u32).max(1),
    );
    let mut out = vec![0u8; (tw * th * 4) as usize];
    for y in 0..th {
        let (y0, y1) = (y * h / th, ((y + 1) * h / th).max(y * h / th + 1).min(h));
        for x in 0..tw {
            let (x0, x1) = (x * w / tw, ((x + 1) * w / tw).max(x * w / tw + 1).min(w));
            let mut sum = [0u32; 3];
            let mut n = 0u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * w + sx) * 4) as usize;
                    if let Some(p) = px.get(i..i + 3) {
                        sum[0] += u32::from(p[0]);
                        sum[1] += u32::from(p[1]);
                        sum[2] += u32::from(p[2]);
                        n += 1;
                    }
                }
            }
            let o = ((y * tw + x) * 4) as usize;
            let n = n.max(1);
            out[o] = (sum[0] / n) as u8;
            out[o + 1] = (sum[1] / n) as u8;
            out[o + 2] = (sum[2] / n) as u8;
            out[o + 3] = 255;
        }
    }
    (tw, th, out)
}

impl Handler for Host {
    fn created(&mut self, hwnd: isize, width: u32, height: u32, scale: f32) -> Result<(), String> {
        let r = self.shells[0].created(hwnd, width, height, scale);
        self.note_version();
        self.seen[0] = self.version;
        // Start-up actions may have asked for host work (New Window in a script).
        self.host_requests(0);
        self.fix_sizes();
        r
    }

    fn input(&mut self, input: Input, age_ms: u32) {
        let i = self.idx();
        if i != self.owner {
            match input {
                // The caret blinks in the window being used only.
                Input::Blink => return,
                // A window catches up with edits made in another once typing pauses.
                Input::Tick
                    if self.seen[i] == self.version || self.last_edit.elapsed() < CATCH_UP =>
                {
                    return;
                }
                _ => {}
            }
        }
        self.acquire(i);
        let kind = if matches!(input, Input::Text(_) | Input::Key(..) | Input::Blink) {
            InputKind::Typing
        } else {
            InputKind::Other
        };
        let before = self.shells[i].view.view().scroll_y();
        self.shells[i].input(input, age_ms);
        self.note_version();
        self.host_requests(i);
        self.sync_after(i, before);
        if !matches!(kind, InputKind::Typing) {
            self.fix_sizes();
        }
    }

    fn resized(&mut self, width: u32, height: u32) {
        let i = self.idx();
        self.acquire(i);
        self.shells[i].resized(width, height);
    }

    fn moved(&mut self, hwnd: isize) {
        if let Some(s) = self.shells.iter_mut().find(|s| s.hwnd == hwnd) {
            s.moved(hwnd);
        }
    }

    fn paint(&mut self) {
        let i = self.idx();
        self.acquire(i);
        self.shells[i].paint();
    }

    fn text_scale_changed(&mut self) {
        let i = self.idx();
        self.shells[i].text_scale_changed();
    }

    fn caret(&self) -> Option<[f32; 4]> {
        self.shells[self.idx()].caret()
    }

    fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        self.shells[self.idx()].nc_hit(x, y)
    }

    fn window_requests(&mut self) -> Vec<Request> {
        let i = self.idx();
        self.shells[i].window_requests()
    }

    fn wants_frame(&self) -> bool {
        self.shells.iter().any(Handler::wants_frame)
    }

    fn frame(&mut self) {
        // The window being used first, then the others that need a frame.
        let mut order: Vec<usize> = (0..self.shells.len()).collect();
        order.sort_by_key(|k| usize::from(*k != self.owner));
        for k in order {
            if self.shells[k].wants_frame() {
                self.acquire(k);
                self.shells[k].frame();
            }
        }
    }

    fn cursor(&self) -> Option<u8> {
        self.shells[self.idx()].cursor()
    }

    fn close_requested(&mut self) -> bool {
        // Closing one of several windows of the document closes only that window.
        if self.shells.len() > 1 {
            return true;
        }
        self.shells[0].close_requested()
    }

    fn accepts_drops(&self) -> bool {
        true
    }

    fn drag_over(&mut self, x: f32, y: f32) -> bool {
        let i = self.idx();
        self.acquire(i);
        self.shells[i].drag_over(x, y)
    }

    fn drag_leave(&mut self) {
        let i = self.idx();
        self.shells[i].drag_leave();
    }

    fn drop_data(&mut self, x: f32, y: f32, copy: bool, data: win32::ole_drag::DropData) -> bool {
        let i = self.idx();
        self.acquire(i);
        let ok = self.shells[i].drop_data(x, y, copy, data);
        self.note_version();
        ok
    }

    fn drag_done(&mut self, effect: u32) {
        let i = self.idx();
        self.acquire(i);
        self.shells[i].drag_done(effect);
        self.note_version();
    }

    fn frame_waitable(&self) -> Option<isize> {
        let k = (0..self.shells.len())
            .find(|k| *k == self.owner && self.shells[*k].wants_frame())
            .or_else(|| self.shells.iter().position(Handler::wants_frame))?;
        self.shells[k].frame_waitable()
    }

    fn frame_waited(&mut self) {
        let k = (0..self.shells.len())
            .find(|k| *k == self.owner && self.shells[*k].wants_frame())
            .or_else(|| self.shells.iter().position(Handler::wants_frame));
        if let Some(k) = k {
            self.shells[k].frame_waited();
        }
    }

    fn destroyed(&mut self, hwnd: isize) {
        let Some(i) = self.shells.iter().position(|s| s.hwnd == hwnd) else {
            return;
        };
        if self.shells.len() == 1 {
            return;
        }
        // The engine stays with a window that remains.
        if self.owner == i {
            let j = usize::from(i == 0);
            self.acquire(j);
        }
        self.shells.remove(i);
        self.seen.remove(i);
        self.links.remove(i);
        if self.owner > i {
            self.owner -= 1;
        }
        for (k, l) in self.links.iter_mut().enumerate() {
            if l.is_some_and(|l| l.0 == hwnd) {
                *l = None;
                self.shells[k].view.set_side_by_side(false, false);
            }
        }
        self.retitle();
    }

    fn sync_scroll(&mut self, dy: f32) {
        let i = self.idx();
        if self.shells[i].view.view_mut().scroll_by(dy) {
            self.shells[i].dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::shrink;

    #[test]
    fn thumbnails_keep_their_shape_and_average_pixels() {
        // 4 × 2: left half black, right half white.
        let px: Vec<u8> = (0..8)
            .flat_map(|i| {
                let v = if i % 4 < 2 { 0 } else { 255 };
                [v, v, v, 255]
            })
            .collect();
        let (w, h, out) = shrink(4, 2, &px, (2, 2));
        assert_eq!((w, h), (2, 1));
        assert_eq!(&out[..4], &[0, 0, 0, 255]);
        assert_eq!(&out[4..8], &[255, 255, 255, 255]);
        // Never enlarged.
        assert_eq!(shrink(4, 2, &px, (100, 100)).0, 4);
    }
}
