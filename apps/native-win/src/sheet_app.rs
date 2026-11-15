//! BlueSheet: grid view, GPU grid image and presentation (safe code only; ADR-0018, N6).
use crate::win32::{self, Handler};
use bo_engine::{SheetEngine, SystemFontIndex};
use bo_gpu_win::{Device, FontSpec, Frame, PageBitmap, TextRendering, WindowSurface};
use bo_render::{PageList, PathSeg, Rgba};
use bo_ui::{
    Font, GridBand, GridMove, GridRing, GridStore, Input, NcHit, PaintHash, Painter, Request,
    SheetView, SheetWindow, TextMeasure, TextStyle,
};
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

/// Interface text size factor (the Windows text size setting), as `f32` bits.
static TEXT_SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000);

fn refresh_text_scale() {
    let factor = std::env::var("BLUEOFFICE_TEXT_SCALE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|f| (0.5..=3.0).contains(f))
        .unwrap_or_else(win32::text_scale_factor);
    TEXT_SCALE.store(factor.to_bits(), Ordering::Relaxed);
}

/// Interface text size grown by the Windows text size setting (as BlueDoc does).
fn ui_size(size: f32) -> f32 {
    (size * f32::from_bits(TEXT_SCALE.load(Ordering::Relaxed)) - 0.01).round()
}

/// Open the workbook named on the command line (or a new one) and run the window.
pub fn run() -> Result<(), String> {
    let mut engine = SheetEngine::with_bundled_fonts();
    // Interface language: English by default; BLUEOFFICE_LOCALE selects another one.
    let locale = std::env::var("BLUEOFFICE_LOCALE").unwrap_or_else(|_| "en-US".into());
    // Numbers are typed and shown in the Vietnamese format (decimal comma) like the other
    // BlueSheet editions; BLUEOFFICE_SHEET_LOCALE=en-US selects a decimal point.
    let numbers = std::env::var("BLUEOFFICE_SHEET_LOCALE").unwrap_or_else(|_| "vi-VN".into());
    engine.set_locale(&numbers);
    // Dates as the Windows region shows them (BLUEOFFICE_SHEET_LOCALE keeps the locale's own).
    if std::env::var_os("BLUEOFFICE_SHEET_LOCALE").is_none() {
        engine.set_region_dates(
            win32::region_value("sShortDate").as_deref(),
            win32::region_value("sTimeFormat").as_deref(),
        );
    }
    // Formulas are calculated when a file opens, as the reference spreadsheet does with
    // files another application saved (BLUEOFFICE_SHEET_RECALC=0 keeps the saved results).
    engine.set_recalc_on_open(std::env::var("BLUEOFFICE_SHEET_RECALC").as_deref() != Ok("0"));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let offset = i64::from(win32::utc_offset_minutes()) * 60_000;
    engine.set_now_ms((now as i64 + offset).max(0) as u64);
    if std::env::var("BLUEOFFICE_SYSTEM_FONTS").as_deref() != Ok("0") {
        let cache = std::env::var_os("LOCALAPPDATA").map(|d| {
            std::path::PathBuf::from(d)
                .join("BlueOffice")
                .join("system-fonts.txt")
        });
        if let Some(dir) = cache.as_ref().and_then(|c| c.parent()) {
            let _ = std::fs::create_dir_all(dir);
        }
        engine.set_system_fonts(SystemFontIndex::scan_cached(
            &SystemFontIndex::default_dirs(),
            cache.as_deref(),
        ));
    }
    let mut file = None;
    let name = if let Some(path) = std::env::args_os().nth(1) {
        let path = std::path::PathBuf::from(path);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path
            .file_name()
            .map_or_else(|| "Book1.xlsx".into(), |n| n.to_string_lossy().into_owned());
        engine.open(&bytes, &name).map_err(|e| e.to_string())?;
        file = Some(path);
        name
    } else {
        engine
            .new_workbook("Book1.xlsx")
            .map_err(|e| e.to_string())?;
        "Book1.xlsx".into()
    };
    let mut view = SheetWindow::new(SheetView::new(engine), &locale, &name);
    // BLUEOFFICE_SHEET_BANDS=0 lays out the whole viewport on each scroll (to compare).
    view.view_mut()
        .set_band_lists(std::env::var("BLUEOFFICE_SHEET_BANDS").as_deref() != Ok("0"));
    let perf = std::env::var_os("BLUEOFFICE_NATIVE_PERF").and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok()
    });
    win32::run(
        &format!("{name} — BlueSheet"),
        Box::new(Shell {
            view,
            gpu: None,
            perf,
            file,
            pending: Vec::new(),
            dirty: true,
            pending_input: None,
            last_frame: [std::time::Duration::ZERO; 3],
            gpu_ms: 0.0,
            gpu_grid_ms: 0.0,
            grid_time: std::time::Duration::ZERO,
            blit: std::env::var("BLUEOFFICE_SHEET_BLIT").as_deref() != Ok("0"),
            ring: std::env::var("BLUEOFFICE_SHEET_RING").as_deref() != Ok("0"),
            layer: std::env::var("BLUEOFFICE_SHEET_LAYER").as_deref() != Ok("0"),
        }),
    )
}

struct Gpu {
    device: Device,
    surface: WindowSurface,
    grid: Option<PageBitmap>,
    /// Second grid bitmap: scrolling copies the kept part of grid into it, then swaps.
    spare: Option<PageBitmap>,
    /// Where the grid image is stored in `grid` (scrolling turns it, BLUEOFFICE_SHEET_RING=0
    /// copies instead).
    ring: GridRing,
    /// Image of the title bar and ribbon, and the hash of what it shows.
    top: Option<(PageBitmap, u64)>,
}

struct Store<'a> {
    device: &'a Device,
    grid: &'a mut Option<PageBitmap>,
    spare: &'a mut Option<PageBitmap>,
    ring: &'a mut GridRing,
    blit: bool,
    use_ring: bool,
}

impl GridStore for Store<'_> {
    fn update(&mut self, list: &PageList, engine: &SheetEngine, scale: f32) -> Result<(), String> {
        let (w, h) = (
            (list.width * scale).ceil() as u32,
            (list.height * scale).ceil() as u32,
        );
        if self
            .grid
            .as_ref()
            .is_none_or(|b| (b.width, b.height) != (w, h))
        {
            *self.grid = Some(self.device.create_page_bitmap(list, scale)?);
        }
        let bitmap = self.grid.as_ref().ok_or("grid bitmap")?;
        *self.ring = GridRing::new(w, h);
        self.device
            .draw_page(list, engine.fonts(), scale, &|_| None, None, bitmap)
    }

    fn turn(
        &mut self,
        list: &PageList,
        engine: &SheetEngine,
        scale: f32,
        shift: [i64; 2],
        bands: &[[u32; 4]],
    ) -> Result<bool, String> {
        let (w, h) = (
            (list.width * scale).ceil() as u32,
            (list.height * scale).ceil() as u32,
        );
        let Some(bitmap) = self.grid.as_ref() else {
            return Ok(false);
        };
        if !(self.blit && self.use_ring) || (bitmap.width, bitmap.height) != (w, h) {
            return Ok(false);
        }
        let before = *self.ring;
        self.ring.turn(shift);
        for band in bands {
            for (r, at) in self.ring.split(*band) {
                let moved = [at[0] as i32 - r[0] as i32, at[1] as i32 - r[1] as i32];
                if self
                    .device
                    .draw_page_at(list, engine.fonts(), scale, &|_| None, r, moved, bitmap)
                    .is_err()
                {
                    // Pages the ring cannot draw shifted: draw the whole image again.
                    *self.ring = before;
                    self.update(list, engine, scale)?;
                    return Ok(true);
                }
            }
        }
        Ok(true)
    }

    fn turn_bands(
        &mut self,
        engine: &SheetEngine,
        scale: f32,
        shift: [i64; 2],
        bands: &[GridBand],
        size: (u32, u32),
    ) -> Result<bool, String> {
        let Some(bitmap) = self.grid.as_ref() else {
            return Ok(false);
        };
        if !(self.blit && self.use_ring) || (bitmap.width, bitmap.height) != size {
            return Ok(false);
        }
        let before = *self.ring;
        self.ring.turn(shift);
        for band in bands {
            for (r, at) in self.ring.split(band.rect) {
                // The band's list starts at its origin in the image.
                let local = [r[0] - band.origin[0], r[1] - band.origin[1], r[2], r[3]];
                let moved = [
                    at[0] as i32 - local[0] as i32,
                    at[1] as i32 - local[1] as i32,
                ];
                if self
                    .device
                    .draw_page_at(
                        &band.list,
                        engine.fonts(),
                        scale,
                        &|_| None,
                        local,
                        moved,
                        bitmap,
                    )
                    .is_err()
                {
                    // The caller lays out the whole viewport and turns the ring again.
                    *self.ring = before;
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn scroll(
        &mut self,
        list: &PageList,
        engine: &SheetEngine,
        scale: f32,
        moves: &[GridMove],
        bands: &[[u32; 4]],
    ) -> Result<(), String> {
        let (w, h) = (
            (list.width * scale).ceil() as u32,
            (list.height * scale).ceil() as u32,
        );
        if !self.blit
            || self.ring.phase() != [0, 0]
            || self
                .grid
                .as_ref()
                .is_none_or(|b| (b.width, b.height) != (w, h))
        {
            return self.update(list, engine, scale);
        }
        if self
            .spare
            .as_ref()
            .is_none_or(|b| (b.width, b.height) != (w, h))
        {
            *self.spare = Some(self.device.create_page_bitmap(list, scale)?);
        }
        let (Some(old), Some(new)) = (self.grid.as_ref(), self.spare.as_ref()) else {
            return self.update(list, engine, scale);
        };
        let rects: Vec<([u32; 4], [u32; 2])> = moves.iter().map(|m| (m.src, m.dst)).collect();
        if self.device.copy_page_rects(old, new, &rects).is_err() {
            return self.update(list, engine, scale);
        }
        std::mem::swap(self.grid, self.spare);
        let bitmap = self.grid.as_ref().ok_or("grid bitmap")?;
        for band in bands {
            self.device
                .draw_page(list, engine.fonts(), scale, &|_| None, Some(*band), bitmap)?;
        }
        Ok(())
    }
}

struct WinPainter<'a, 'f> {
    frame: &'a Frame<'f>,
    grid: Option<(&'a PageBitmap, GridRing)>,
}

impl Painter for WinPainter<'_, '_> {
    fn clear(&mut self, color: Rgba) {
        self.frame.clear(color);
    }
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        self.frame.fill_rect(x, y, w, h, color);
    }
    fn page(&mut self, index: usize, x: i32, y: i32) {
        if let (0, Some((b, ring))) = (index, self.grid) {
            if ring.phase() == [0, 0] {
                self.frame.draw_page(b, x, y);
            } else {
                for (r, at) in ring.pieces() {
                    let src = [at[0], at[1], r[2], r[3]];
                    self.frame
                        .draw_page_rect(b, src, x + r[0] as i32, y + r[1] as i32);
                }
            }
        }
    }
    fn text(&mut self, text: &str, x: f32, y: f32, style: TextStyle, color: Rgba) {
        self.frame
            .text(text, x, y, ui_size(style.size), style.bold, color);
    }
    fn text_font(&mut self, text: &str, x: f32, y: f32, font: &Font<'_>, color: Rgba) {
        self.frame.text_font(text, x, y, &spec(font), color);
    }
    fn path(&mut self, segs: &[PathSeg], fill: Option<Rgba>, stroke: Option<(Rgba, f32)>) {
        self.frame.path(segs, fill, stroke);
    }
    fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.frame.push_clip(x, y, w, h);
    }
    fn pop_clip(&mut self) {
        self.frame.pop_clip();
    }
}

struct Measure<'a>(Option<&'a Device>);

impl TextMeasure for Measure<'_> {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        match self.0 {
            Some(d) => d.measure_text(text, ui_size(style.size), style.bold),
            None => (
                text.chars().count() as f32 * style.size * 0.55,
                style.size * 1.33,
            ),
        }
    }
    fn measure_font(&self, text: &str, font: &Font<'_>) -> (f32, f32) {
        match self.0 {
            Some(d) => d.measure_font(text, &spec(font)),
            None => (
                text.chars().count() as f32 * font.size * 0.55,
                font.size * 1.33,
            ),
        }
    }
}

fn spec<'a>(f: &Font<'a>) -> FontSpec<'a> {
    FontSpec {
        family: f.family,
        size: f.size,
        weight: f.weight,
        italic: f.italic,
        underline: f.underline,
    }
}

// Each bool is an independent switch (BLUEOFFICE_SHEET_* measurements) or the dirty flag.
#[allow(clippy::struct_excessive_bools)]
struct Shell {
    view: SheetWindow,
    gpu: Option<Gpu>,
    perf: Option<std::fs::File>,
    file: Option<std::path::PathBuf>,
    pending: Vec<Request>,
    dirty: bool,
    pending_input: Option<PendingInput>,
    /// Last frame: grid image, interface paint, end of drawing + present.
    last_frame: [std::time::Duration; 3],
    /// GPU time after the last logged frame's present (BLUEOFFICE_NATIVE_GPU=1 waits for it).
    gpu_ms: f64,
    /// GPU time of the grid image alone (same switch).
    gpu_grid_ms: f64,
    /// Grid image drawing since the last logged frame (also by paints the system asks for).
    grid_time: std::time::Duration,
    /// Scrolling moves the grid image instead of redrawing it (BLUEOFFICE_SHEET_BLIT=0
    /// turns it off to compare).
    blit: bool,
    /// The title bar and ribbon are cached as an image (BLUEOFFICE_SHEET_LAYER=0: off).
    layer: bool,
    /// Scrolling turns the grid image as a ring instead of copying it (BLUEOFFICE_SHEET_RING=0).
    ring: bool,
}

struct PendingInput {
    first: Instant,
    kind: &'static str,
    age_ms: u32,
    handle: std::time::Duration,
    count: u32,
}

/// Start another BlueSheet window (with `file`, or a new workbook).
fn spawn_self(file: Option<&std::path::Path>) {
    if let Ok(exe) = std::env::current_exe() {
        let mut cmd = std::process::Command::new(exe);
        if let Some(f) = file {
            cmd.arg(f);
        }
        let _ = cmd.spawn();
    }
}

impl Shell {
    fn save(&mut self) -> bool {
        let Some(path) = self.file.clone() else {
            return self.save_as();
        };
        let sheet = self.view.view().sheet();
        let written = self
            .view
            .view_mut()
            .engine_mut()
            .save("xlsx", sheet)
            .map_err(|e| e.to_string())
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
        match written {
            Ok(()) => {
                self.view.view_mut().mark_saved();
                true
            }
            Err(e) => {
                eprintln!("save {}: {e}", path.display());
                false
            }
        }
    }

    fn save_as(&mut self) -> bool {
        let name = self.file.as_ref().and_then(|p| p.file_name()).map_or_else(
            || "Book1.xlsx".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let Some(path) = win32::file_dialog(true, &[("Excel workbook (*.xlsx)", "*.xlsx")], &name)
        else {
            return false;
        };
        self.file = Some(path.clone());
        if self.save() {
            let shown = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            self.view.set_name(&shown);
            return true;
        }
        false
    }

    fn handle_requests(&mut self) -> bool {
        let mut redraw = false;
        for r in self.view.take_requests() {
            match r {
                Request::Save => redraw |= self.save(),
                Request::SaveAs => redraw |= self.save_as(),
                Request::SetClipboard {
                    text,
                    html,
                    internal,
                } => win32::set_clipboard(&text, &html, &internal),
                Request::Paste => {
                    let (_, _, text) = win32::read_clipboard();
                    redraw |= self.view.paste(text);
                }
                Request::New => spawn_self(None),
                Request::Open => {
                    let filters = [
                        (
                            "Workbooks (*.xlsx;*.xlsm;*.xls;*.xlsb;*.ods;*.csv)",
                            "*.xlsx;*.xlsm;*.xls;*.xlsb;*.ods;*.csv",
                        ),
                        ("All files", "*.*"),
                    ];
                    if let Some(path) = win32::file_dialog(false, &filters, "") {
                        spawn_self(Some(&path));
                    }
                }
                Request::PickPicture => {
                    let filters = [(
                        "Pictures (*.png;*.jpg;*.jpeg;*.gif;*.bmp)",
                        "*.png;*.jpg;*.jpeg;*.gif;*.bmp",
                    )];
                    // BLUEOFFICE_PICTURE (a path) skips the dialog, for scripted comparisons.
                    let path = std::env::var_os("BLUEOFFICE_PICTURE")
                        .map(std::path::PathBuf::from)
                        .or_else(|| win32::file_dialog(false, &filters, ""));
                    if let Some(bytes) = path.and_then(|p| std::fs::read(p).ok()) {
                        redraw |= self.view.insert_picture(&bytes, "");
                    }
                }
                Request::PickMergeData => {
                    // A workbook (Merge Styles…) or a text file (Get Data from Text/CSV);
                    // BLUEOFFICE_PICK (a path) skips the dialog, for scripted comparisons.
                    let owned = self.view.pick_filters();
                    let filters: Vec<(&str, &str)> = owned
                        .iter()
                        .map(|(d, p)| (d.as_str(), p.as_str()))
                        .chain([("All files", "*.*")])
                        .collect();
                    let path = std::env::var_os("BLUEOFFICE_PICK")
                        .map(std::path::PathBuf::from)
                        .or_else(|| win32::file_dialog(false, &filters, ""));
                    if let Some(p) = path {
                        if let Ok(bytes) = std::fs::read(&p) {
                            let name = p
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            redraw |= self.view.picked_file(&bytes, &name);
                        }
                    }
                }
                other => self.pending.push(other),
            }
        }
        redraw
    }

    fn render(&mut self) -> Result<(), String> {
        let Some(gpu) = self.gpu.as_mut() else {
            return Ok(());
        };
        let t0 = Instant::now();
        self.view.prepare(&mut Store {
            device: &gpu.device,
            grid: &mut gpu.grid,
            spare: &mut gpu.spare,
            ring: &mut gpu.ring,
            blit: self.blit,
            use_ring: self.ring,
        })?;
        if self.perf.is_some() && std::env::var_os("BLUEOFFICE_NATIVE_GPU").is_some() {
            self.gpu_grid_ms = gpu.device.wait_gpu_ms();
        }
        let t1 = Instant::now();
        self.grid_time += t1 - t0;
        // The title bar and ribbon are drawn into an image only when they change (most frames
        // only change the grid; BLUEOFFICE_SHEET_LAYER=0 draws them every frame).
        let p0 = Instant::now();
        let mut top = None;
        if self.layer {
            let (w, _) = gpu.surface.size();
            let h = self.view.top_height().ceil().max(1.0) as u32;
            let mut hash = PaintHash::default();
            self.view.paint_top(&mut hash);
            let key = hash.finish();
            let fits = gpu
                .top
                .as_ref()
                .is_some_and(|(b, _)| (b.width, b.height) == (w, h));
            if !fits {
                gpu.top = gpu.device.create_layer(w, h).ok().map(|b| (b, !key));
            }
            if let Some((bitmap, cached)) = gpu.top.as_mut() {
                if *cached != key {
                    let view = &self.view;
                    gpu.device.draw_layer(bitmap, |frame| {
                        view.paint_top(&mut WinPainter { frame, grid: None });
                    })?;
                    *cached = key;
                }
            }
            top = gpu.top.as_ref().map(|(b, _)| b);
        }
        let mut painted = p0.elapsed();
        let (view, grid) = (&self.view, gpu.grid.as_ref().map(|g| (g, gpu.ring)));
        let r = gpu.surface.frame(&gpu.device, |frame| {
            let p0 = Instant::now();
            let mut wp = WinPainter { frame, grid };
            if let Some(top) = top {
                view.paint_base(&mut wp);
                frame.draw_page(top, 0, 0);
                view.paint_overlay(&mut wp);
            } else {
                view.paint(&mut wp);
            }
            painted += p0.elapsed();
        });
        self.last_frame = [t1 - t0, painted, t1.elapsed().saturating_sub(painted)];
        r
    }
}

impl Handler for Shell {
    fn created(&mut self, hwnd: isize, width: u32, height: u32, scale: f32) -> Result<(), String> {
        refresh_text_scale();
        let device = Device::new()?;
        if let Some(f) = self.perf.as_mut() {
            let _ = writeln!(f, r#"{{"input":"driver","name":"{:?}"}}"#, device.driver());
        }
        if let Err(e) = device.update_window_monitor(hwnd) {
            eprintln!("monitor text settings: {e}");
        }
        // Cell text like the reference spreadsheet's grid: the system's ClearType settings
        // (BLUEOFFICE_TEXT=gray|light compares other renderings).
        device.set_text_rendering(match std::env::var("BLUEOFFICE_TEXT").as_deref() {
            Ok("gray") => TextRendering::Grayscale,
            Ok("light") => TextRendering::light_cleartype(),
            _ => TextRendering::System,
        });
        let mut surface = WindowSurface::new(&device, hwnd, width, height)?;
        // BLUEOFFICE_SHEET_SYNC=0 hands frames to the compositor without waiting for the
        // vertical blank (the same on-screen latency here, and window captures with
        // PrintWindow can then read a frame still being drawn), 1 (default) as BlueDoc.
        surface.set_sync_interval(
            std::env::var("BLUEOFFICE_SHEET_SYNC")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
        );
        self.gpu = Some(Gpu {
            device,
            surface,
            grid: None,
            spare: None,
            ring: GridRing::default(),
            top: None,
        });
        let m = Measure(self.gpu.as_ref().map(|g| &g.device));
        self.view.handle(Input::Scale(scale), &m);
        self.view.handle(
            Input::Resize {
                width: width as f32,
                height: height as f32,
            },
            &m,
        );
        if let Some(z) = std::env::var("BLUEOFFICE_ZOOM")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            self.view.handle(Input::Zoom((z - 100.0) / 10.0), &m);
        }
        Ok(())
    }

    fn input(&mut self, input: Input, age_ms: u32) {
        let kind = match &input {
            Input::Text(_) => "text",
            Input::Key(..) => "key",
            Input::Wheel(_) => "wheel",
            Input::Press { .. } => "press",
            Input::Drag { .. } => "drag",
            Input::Resize { .. } => "resize",
            Input::Scale(_) => "scale",
            Input::Blink => "blink",
            Input::Move { .. } => "move",
            Input::Release { .. } => "release",
            Input::Tick => "tick",
            Input::Maximized(_) => "maximized",
            Input::Leave => "leave",
            Input::Zoom(_) => "zoom",
            Input::Context { .. } => "context",
            Input::Preedit(_) => "preedit",
        };
        let start = Instant::now();
        let m = Measure(self.gpu.as_ref().map(|g| &g.device));
        let mut redraw = self.view.handle(input, &m);
        redraw |= self.handle_requests();
        if !redraw {
            return;
        }
        self.dirty = true;
        if !matches!(kind, "blink" | "move" | "tick") {
            let p = self.pending_input.get_or_insert(PendingInput {
                first: start,
                kind,
                age_ms,
                handle: std::time::Duration::ZERO,
                count: 0,
            });
            p.handle += start.elapsed();
            p.count += 1;
        }
    }

    fn text_scale_changed(&mut self) {
        refresh_text_scale();
    }

    fn resized(&mut self, width: u32, height: u32) {
        if let Some(gpu) = self.gpu.as_mut() {
            if let Err(e) = gpu.surface.resize(&gpu.device, width, height) {
                eprintln!("resize: {e}");
            }
        }
        let m = Measure(self.gpu.as_ref().map(|g| &g.device));
        self.view.handle(
            Input::Resize {
                width: width as f32,
                height: height as f32,
            },
            &m,
        );
        let _ = self.render();
    }

    fn moved(&mut self, hwnd: isize) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        match gpu.device.update_window_monitor(hwnd) {
            Ok(true) => {
                self.view.view_mut().invalidate();
                self.dirty = true;
            }
            Ok(false) => {}
            Err(e) => eprintln!("monitor text settings: {e}"),
        }
    }

    fn paint(&mut self) {
        // A paint the system asks for (a capture, an uncovered window) draws the pending
        // input's frame at once (logged like a frame) so that it is presented only once.
        if self.dirty {
            self.frame();
        } else if let Err(e) = self.render() {
            eprintln!("render: {e}");
        }
    }
    fn caret(&self) -> Option<[f32; 4]> {
        self.view.caret_rect()
    }

    fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        self.view.nc_hit(x, y)
    }

    fn cursor(&self) -> Option<u8> {
        Some(self.view.cursor())
    }

    fn window_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.pending)
    }

    fn wants_frame(&self) -> bool {
        self.dirty || self.view.animating()
    }

    fn frame(&mut self) {
        self.dirty = false;
        if let Err(e) = self.render() {
            eprintln!("render: {e}");
        }
        if self.perf.is_some() && std::env::var_os("BLUEOFFICE_NATIVE_GPU").is_some() {
            self.gpu_ms = self.gpu.as_ref().map_or(0.0, |g| g.device.wait_gpu_ms());
        }
        if let (Some(file), Some(p)) = (self.perf.as_mut(), self.pending_input.take()) {
            let total = p.first.elapsed();
            let _ = writeln!(
                file,
                r#"{{"input":"{}","inputs":{},"age_ms":{},"handle_ms":{:.3},"grid_ms":{:.3},"prepare_ms":{:.3},"paint_ms":{:.3},"submit_ms":{:.3},"present_ms":{:.3},"total_ms":{:.3},"gpu_ms":{:.3},"gpu_grid_ms":{:.3}}}"#,
                p.kind,
                p.count,
                p.age_ms,
                p.handle.as_secs_f64() * 1e3,
                std::mem::take(&mut self.grid_time).as_secs_f64() * 1e3,
                self.last_frame[0].as_secs_f64() * 1e3,
                self.last_frame[1].as_secs_f64() * 1e3,
                self.last_frame[2].as_secs_f64() * 1e3,
                total.saturating_sub(p.handle).as_secs_f64() * 1e3,
                total.as_secs_f64() * 1e3 + f64::from(p.age_ms),
                self.gpu_ms,
                self.gpu_grid_ms,
            );
        }
    }
}
