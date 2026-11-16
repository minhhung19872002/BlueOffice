//! The application: document view, GPU page store and presentation (safe code only).
use crate::win32::{self, Handler};
use bo_engine::{Engine, SystemFontIndex};
use bo_gpu_win::{Device, FontSpec, Frame, PageBitmap, TextRendering, WindowSurface};
use bo_render::{PageList, PathSeg, Rgba};
use bo_ui::{
    DocView, DocWindow, Font, Input, NcHit, PageStore, Painter, Request, TextMeasure, TextStyle,
};
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

/// Interface text size factor (the Windows text size setting), as `f32` bits.
static TEXT_SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000);

/// Read the Windows text size setting; `BLUEOFFICE_TEXT_SCALE` (e.g. `1.1`) overrides it.
fn refresh_text_scale() {
    let factor = std::env::var("BLUEOFFICE_TEXT_SCALE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|f| (0.5..=3.0).contains(f))
        .unwrap_or_else(win32::text_scale_factor);
    TEXT_SCALE.store(factor.to_bits(), Ordering::Relaxed);
}

/// Interface text size in device pixels: the size the layout asks for, grown by the
/// Windows text size setting as a desktop word processor does. Whole pixels, halves
/// rounded down (15 px at 110 % is 16 px, 16 px is 18 px), which matched the reference.
fn ui_size(size: f32) -> f32 {
    (size * f32::from_bits(TEXT_SCALE.load(Ordering::Relaxed)) - 0.01).round()
}

/// Open the file named on the command line (or a new document) and run the window.
pub fn run() -> Result<(), String> {
    let mut engine = Engine::with_bundled_fonts();
    // DATE / TIME fields show the local time, as desktop word processors do.
    engine.set_utc_offset_minutes(win32::utc_offset_minutes());
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
        {
            let path = std::path::PathBuf::from(path);
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let name = path.file_name().map_or_else(
                || "document.docx".into(),
                |n| n.to_string_lossy().into_owned(),
            );
            engine.open(&bytes, &name).map_err(|e| e.to_string())?;
            file = Some(path);
            name
        }
    } else {
        engine
            .new_document("Document1.docx")
            .map_err(|e| e.to_string())?;
        "Document1.docx".into()
    };
    // Interface language: English by default (owner's choice, 2026-11-14); BLUEOFFICE_LOCALE
    // selects another locale of packages/i18n (e.g. vi-VN).
    let locale = std::env::var("BLUEOFFICE_LOCALE").unwrap_or_else(|_| "en-US".into());
    let new_document = file.is_none();
    let mut view = DocWindow::new(DocView::new(engine)?, &locale, &name);
    // A new document takes the theme, style set and effects set as the default.
    if new_document {
        if let Some(v) = defaults_path().and_then(|p| std::fs::read_to_string(p).ok()) {
            view.apply_design_defaults(&v);
        }
    }
    // Sizes in the region's unit and decimal separator, as the reference shows them
    // (iMeasure 1: US system, inches). BLUEOFFICE_UNITS=cm|in overrides.
    let inches = match std::env::var("BLUEOFFICE_UNITS").as_deref() {
        Ok("in") => true,
        Ok("cm") => false,
        _ => win32::region_value("iMeasure").as_deref() == Some("1"),
    };
    let decimal = win32::region_value("sDecimal")
        .and_then(|s| s.chars().next())
        .unwrap_or('.');
    view.set_units(inches, decimal);
    let perf = std::env::var_os("BLUEOFFICE_NATIVE_PERF").and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok()
    });
    win32::run(
        &format!("{name} — BlueDoc"),
        Box::new(Shell {
            view,
            gpu: None,
            perf,
            file,
            pending: Vec::new(),
            dirty: true,
            pending_input: None,
            last_frame: [std::time::Duration::ZERO; 3],
            clock: Instant::now(),
        }),
    )
}

/// GPU page images, one bitmap per page index.
struct Pages {
    bitmaps: HashMap<usize, PageBitmap>,
}

struct Gpu {
    device: Device,
    surface: WindowSurface,
    pages: Pages,
}

struct Store<'a> {
    device: &'a Device,
    pages: &'a mut Pages,
}

impl PageStore for Store<'_> {
    fn update(
        &mut self,
        index: usize,
        list: &PageList,
        engine: &Engine,
        scale: f32,
        band: Option<[u32; 4]>,
    ) -> Result<(), String> {
        let (w, h) = (
            (list.width * scale).ceil() as u32,
            (list.height * scale).ceil() as u32,
        );
        let fits = self
            .pages
            .bitmaps
            .get(&index)
            .is_some_and(|b| (b.width, b.height) == (w, h));
        // A band can only be drawn over an existing image of the same size.
        let band = if fits { band } else { None };
        if !fits {
            let bitmap = self.device.create_page_bitmap(list, scale)?;
            self.pages.bitmaps.insert(index, bitmap);
        }
        let bitmap = self.pages.bitmaps.get(&index).ok_or("page bitmap")?;
        if std::env::var_os("BLUEOFFICE_NATIVE_DEBUG").is_some() {
            eprintln!("update page {index} band {band:?} new {}", !fits);
        }
        let pictures = |id: &str| {
            let bytes = engine.media_resolved(id)?;
            let (width, height, pixels) = bo_raster::decode_premultiplied(&bytes)?;
            Some(bo_gpu_win::Bitmap {
                width,
                height,
                pixels,
            })
        };
        self.device
            .draw_page(list, engine.fonts(), scale, &pictures, band, bitmap)
    }

    fn remove(&mut self, index: usize) {
        self.pages.bitmaps.remove(&index);
    }
}

struct WinPainter<'a, 'f> {
    frame: &'a Frame<'f>,
    pages: &'a HashMap<usize, PageBitmap>,
}

impl Painter for WinPainter<'_, '_> {
    fn clear(&mut self, color: Rgba) {
        self.frame.clear(color);
    }
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        self.frame.fill_rect(x, y, w, h, color);
    }
    fn page(&mut self, index: usize, x: i32, y: i32) {
        if let Some(bitmap) = self.pages.get(&index) {
            self.frame.draw_page(bitmap, x, y);
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
    fn layer_begin(&mut self, id: usize, rect: [f32; 4], key: u64) -> bool {
        let (x, y) = (rect[0].round(), rect[1].round());
        let (r, b) = ((rect[0] + rect[2]).round(), (rect[1] + rect[3]).round());
        self.frame.layer_begin(
            id,
            [x as i32, y as i32, (r - x) as i32, (b - y) as i32],
            key,
        )
    }
    fn layer_end(&mut self, id: usize) {
        self.frame.layer_end(id);
    }
}

/// Interface text measured by DirectWrite (before a device exists: an estimate).
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

struct Shell {
    view: DocWindow,
    gpu: Option<Gpu>,
    perf: Option<std::fs::File>,
    /// File the document was opened from (Save writes it back).
    file: Option<std::path::PathBuf>,
    /// Window operations for the window layer.
    pending: Vec<Request>,
    /// A frame is needed (drawn when the message queue is empty).
    dirty: bool,
    pending_input: Option<PendingInput>,
    /// Last frame: page preparation, interface paint, end-of-draw + present.
    last_frame: [std::time::Duration; 3],
    /// Clock of animations.
    clock: Instant,
}

impl Shell {
    /// Write the document to the file it came from (`.docx`); `true` when the state changed.
    fn save(&mut self) -> bool {
        let Some(path) = self.file.clone() else {
            return false;
        };
        let written = self
            .view
            .view()
            .save_docx()
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

    /// Let the user pick a picture file and insert it at the selection.
    fn pick_picture(&mut self) -> bool {
        let filters = [(
            "Pictures (*.png;*.jpg;*.jpeg;*.gif;*.bmp;*.tif;*.tiff;*.svg)",
            "*.png;*.jpg;*.jpeg;*.gif;*.bmp;*.tif;*.tiff;*.svg",
        )];
        // BLUEOFFICE_PICTURE (a path) skips the dialog, for scripted comparisons.
        let path = std::env::var_os("BLUEOFFICE_PICTURE")
            .map(std::path::PathBuf::from)
            .or_else(|| win32::file_dialog(false, &filters, ""));
        let Some(path) = path else {
            return false;
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return false;
        };
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.view.insert_picture(bytes, &name)
    }

    /// Carry out the interface's requests (files, clipboard, printing); `true` to redraw.
    fn handle_requests(&mut self) -> bool {
        let mut redraw = false;
        for r in self.view.take_requests() {
            match r {
                Request::Save => redraw |= self.save(),
                Request::SetClipboard {
                    text,
                    html,
                    internal,
                } => win32::set_clipboard(&text, &html, &internal),
                Request::Paste => {
                    let (internal, html, text) = win32::read_clipboard();
                    redraw |= self.view.paste(internal, html, text);
                }
                Request::New => spawn_self(None),
                Request::Open => {
                    let filters = [
                        (
                            "Documents (*.docx;*.doc;*.odt;*.rtf;*.dotx;*.docm)",
                            "*.docx;*.doc;*.odt;*.rtf;*.dotx;*.docm",
                        ),
                        ("All files", "*.*"),
                    ];
                    if let Some(path) = win32::file_dialog(false, &filters, "") {
                        spawn_self(Some(&path));
                    }
                }
                Request::SaveAs => {
                    let name = self.file.as_ref().and_then(|p| p.file_name()).map_or_else(
                        || "Document1.docx".to_owned(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    if let Some(path) =
                        win32::file_dialog(true, &[("Word document (*.docx)", "*.docx")], &name)
                    {
                        self.file = Some(path.clone());
                        if self.save() {
                            let shown = path
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            self.view.set_name(&shown);
                            redraw = true;
                        }
                    }
                }
                Request::ExportPdf => {
                    let name = self.file.as_ref().and_then(|p| p.file_stem()).map_or_else(
                        || "Document1.pdf".to_owned(),
                        |n| format!("{}.pdf", n.to_string_lossy()),
                    );
                    if let Some(path) = win32::file_dialog(true, &[("PDF (*.pdf)", "*.pdf")], &name)
                    {
                        match self.view.view().export_pdf() {
                            Ok(bytes) => {
                                if let Err(e) = std::fs::write(&path, bytes) {
                                    eprintln!("export {}: {e}", path.display());
                                }
                            }
                            Err(e) => eprintln!("export: {e}"),
                        }
                    }
                }
                Request::Print => {
                    // Printed through the PDF application's print verb (opened when it has none).
                    if let Ok(bytes) = self.view.view().export_pdf() {
                        let path = std::env::temp_dir()
                            .join(format!("bluedoc-print-{}.pdf", std::process::id()));
                        if std::fs::write(&path, bytes).is_ok()
                            && !win32::shell_open(&path, "print")
                        {
                            win32::shell_open(&path, "open");
                        }
                    }
                }
                Request::PickMergeData => {
                    let filters = [("Recipient lists (*.csv;*.xlsx)", "*.csv;*.xlsx")];
                    if let Some(path) = win32::file_dialog(false, &filters, "") {
                        if let Ok(bytes) = std::fs::read(&path) {
                            use base64::Engine as _;
                            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                            let name = path
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            redraw |= self.view.merge_data(&name, &b64);
                        }
                    }
                }
                Request::PickPicture => redraw |= self.pick_picture(),
                // Review › Compare: the document to compare with (BLUEOFFICE_COMPARE, a
                // path, skips the dialog for scripted comparisons).
                Request::PickCompareFile => {
                    let filters = [("Documents (*.docx;*.doc;*.odt)", "*.docx;*.doc;*.odt")];
                    let path = std::env::var_os("BLUEOFFICE_COMPARE")
                        .map(std::path::PathBuf::from)
                        .or_else(|| win32::file_dialog(false, &filters, ""));
                    if let Some(bytes) = path.as_ref().and_then(|p| std::fs::read(p).ok()) {
                        use base64::Engine as _;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                        let name = path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        redraw |= self.view.compare_file(&name, &b64);
                    }
                }
                // Envelopes and Labels: the new document opens in a window of its own.
                Request::OpenNewDocument => {
                    if let Some(bytes) = self.view.take_new_document() {
                        let stamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis());
                        let path = std::env::temp_dir().join(format!("BlueDoc-{stamp}.docx"));
                        if std::fs::write(&path, bytes).is_ok() {
                            spawn_self(Some(&path));
                        }
                    }
                }
                // Design › Set as Default: kept for new documents.
                Request::SaveDefaults => {
                    if let (Some(path), Some(v)) = (defaults_path(), self.view.design_defaults()) {
                        if let Some(dir) = path.parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        let _ = std::fs::write(&path, v);
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
            pages: &mut gpu.pages,
        })?;
        let t1 = Instant::now();
        let (view, pages) = (&self.view, &gpu.pages.bitmaps);
        let mut painted = std::time::Duration::ZERO;
        let r = gpu.surface.frame(&gpu.device, |frame| {
            let p0 = Instant::now();
            view.paint(&mut WinPainter { frame, pages });
            painted = p0.elapsed();
        });
        // Performance log: page re-render, interface paint, the rest of the frame (end of
        // drawing and present).
        self.last_frame = [t1 - t0, painted, t1.elapsed().saturating_sub(painted)];
        r
    }
}

/// Where Design › Set as Default keeps the defaults of new documents.
fn defaults_path() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|d| {
        std::path::PathBuf::from(d)
            .join("BlueOffice")
            .join("bluedoc-defaults.json")
    })
}

/// Start another BlueDoc window (with `file`, or a new document).
fn spawn_self(file: Option<&std::path::Path>) {
    if let Ok(exe) = std::env::current_exe() {
        let mut cmd = std::process::Command::new(exe);
        if let Some(f) = file {
            cmd.arg(f);
        }
        let _ = cmd.spawn();
    }
}

/// Inputs handled since the last frame, for the performance log.
struct PendingInput {
    first: Instant,
    kind: &'static str,
    age_ms: u32,
    handle: std::time::Duration,
    count: u32,
}

impl Handler for Shell {
    fn created(&mut self, hwnd: isize, width: u32, height: u32, scale: f32) -> Result<(), String> {
        refresh_text_scale();
        let device = Device::new()?;
        if let Err(e) = device.update_window_monitor(hwnd) {
            eprintln!("monitor text settings: {e}");
        }
        // BLUEOFFICE_TEXT=system|light|gray compares text rendering without a rebuild.
        device.set_text_rendering(match std::env::var("BLUEOFFICE_TEXT").as_deref() {
            Ok("system") => TextRendering::System,
            Ok("light") => TextRendering::light_cleartype(),
            _ => TextRendering::document_default(),
        });
        let surface = WindowSurface::new(&device, hwnd, width, height)?;
        self.gpu = Some(Gpu {
            device,
            surface,
            pages: Pages {
                bitmaps: HashMap::new(),
            },
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
        // BLUEOFFICE_ZOOM=140 opens at that zoom (10 % steps), for screenshot comparisons.
        if let Some(z) = std::env::var("BLUEOFFICE_ZOOM")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            self.view.handle(Input::Zoom((z - 100.0) / 10.0), &m);
        }
        // BLUEOFFICE_ACTIONS="insert.table.3x3;table.insertRowBelow" runs interface commands
        // at start, for screenshot comparisons without sending input to the desktop.
        if let Ok(ids) = std::env::var("BLUEOFFICE_ACTIONS") {
            for id in ids.split(';').filter(|s| !s.is_empty()) {
                self.view.run_interface_command(id, &m);
            }
            self.handle_requests();
            let m = Measure(self.gpu.as_ref().map(|g| &g.device));
            self.view.refresh(&m);
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
        let m = Measure(self.gpu.as_ref().map(|g| &g.device));
        redraw |= self.view.refresh(&m);
        if !redraw {
            return;
        }
        // The frame is drawn once the message queue is empty (win32::run).
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
                self.view.invalidate_page_images();
                self.dirty = true;
            }
            Ok(false) => {}
            Err(e) => eprintln!("monitor text settings: {e}"),
        }
    }

    fn paint(&mut self) {
        if let Err(e) = self.render() {
            eprintln!("render: {e}");
        }
    }

    fn caret(&self) -> Option<[f32; 4]> {
        self.view.caret_rect()
    }

    fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        self.view.nc_hit(x, y)
    }

    fn window_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.pending)
    }

    fn wants_frame(&self) -> bool {
        self.dirty || self.view.animating()
    }

    fn frame(&mut self) {
        self.dirty = false;
        let now = self.clock.elapsed().as_secs_f64() * 1e3;
        self.view.advance(now);
        if let Err(e) = self.render() {
            eprintln!("render: {e}");
        }
        if let (Some(file), Some(p)) = (self.perf.as_mut(), self.pending_input.take()) {
            let total = p.first.elapsed();
            let _ = writeln!(
                file,
                r#"{{"input":"{}","inputs":{},"age_ms":{},"handle_ms":{:.3},"prepare_ms":{:.3},"paint_ms":{:.3},"submit_ms":{:.3},"present_ms":{:.3},"total_ms":{:.3}}}"#,
                p.kind,
                p.count,
                p.age_ms,
                p.handle.as_secs_f64() * 1e3,
                self.last_frame[0].as_secs_f64() * 1e3,
                self.last_frame[1].as_secs_f64() * 1e3,
                self.last_frame[2].as_secs_f64() * 1e3,
                total.saturating_sub(p.handle).as_secs_f64() * 1e3,
                total.as_secs_f64() * 1e3 + f64::from(p.age_ms),
            );
        }
    }
}
