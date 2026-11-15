//! BlueSlide application: editing area, GPU slide images and presentation (safe code only).
//! Same structure as BlueDoc's `app.rs`, with idle-time drawing of neighbouring slides so
//! switching slides shows a kept image at once.
use crate::win32::{self, Handler};
use bo_engine::{Engine, SystemFontIndex};
use bo_gpu_win::{Device, FontSpec, Frame, PageBitmap, TextRendering, WindowSurface};
use bo_render::{PageList, PathSeg, Rgba};
use bo_ui::{
    Font, Input, NcHit, PageStore, Painter, Request, SlideView, SlideWindow, TextMeasure, TextStyle,
};
use std::collections::HashMap;
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

/// Interface text size in device pixels grown by the Windows text size setting (whole
/// pixels, halves rounded down), as BlueDoc's window does.
fn ui_size(size: f32) -> f32 {
    (size * f32::from_bits(TEXT_SCALE.load(Ordering::Relaxed)) - 0.01).round()
}

/// Open the presentation named on the command line (or a new one) and run the window.
pub fn run() -> Result<(), String> {
    let mut engine = Engine::with_bundled_fonts();
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
        let path = std::path::PathBuf::from(path);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path.file_name().map_or_else(
            || "presentation.pptx".into(),
            |n| n.to_string_lossy().into_owned(),
        );
        engine.open(&bytes, &name).map_err(|e| e.to_string())?;
        if engine.slide_count() == 0 {
            return Err(format!("{name}: not a presentation"));
        }
        file = Some(path);
        name
    } else {
        let bytes = bo_engine::slide_edit::album::new_deck("").map_err(|e| e.to_string())?;
        engine
            .open(&bytes, "Presentation1.pptx")
            .map_err(|e| e.to_string())?;
        engine.mark_saved();
        "Presentation1.pptx".into()
    };
    let locale = std::env::var("BLUEOFFICE_LOCALE").unwrap_or_else(|_| "en-US".into());
    let view = SlideWindow::new(SlideView::new(engine)?, &locale, &name);
    let perf = std::env::var_os("BLUEOFFICE_NATIVE_PERF").and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok()
    });
    win32::run(
        &format!("{name} — BlueSlide"),
        Box::new(Shell {
            view,
            gpu: None,
            perf,
            file,
            pending: Vec::new(),
            dirty: true,
            prefetching: true,
            pending_input: None,
            last_frame: [std::time::Duration::ZERO; 3],
            epoch: Instant::now(),
            anim_frame: None,
        }),
    )
}

struct Gpu {
    device: Device,
    surface: WindowSurface,
    bitmaps: HashMap<usize, PageBitmap>,
}

struct Store<'a> {
    device: &'a Device,
    bitmaps: &'a mut HashMap<usize, PageBitmap>,
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
            .bitmaps
            .get(&index)
            .is_some_and(|b| (b.width, b.height) == (w, h));
        let band = if fits { band } else { None };
        if !fits {
            let bitmap = self.device.create_page_bitmap(list, scale)?;
            self.bitmaps.insert(index, bitmap);
        }
        let bitmap = self.bitmaps.get(&index).ok_or("slide bitmap")?;
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
        self.bitmaps.remove(&index);
    }

    fn update_layer(
        &mut self,
        index: usize,
        list: &PageList,
        engine: &Engine,
        scale: f32,
    ) -> Result<(), String> {
        let bitmap = self.device.create_layer_bitmap(list, scale)?;
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
            .draw_page(list, engine.fonts(), scale, &pictures, None, &bitmap)?;
        self.bitmaps.insert(index, bitmap);
        Ok(())
    }
}

struct WinPainter<'a, 'f> {
    frame: &'a Frame<'f>,
    bitmaps: &'a HashMap<usize, PageBitmap>,
}

impl Painter for WinPainter<'_, '_> {
    fn clear(&mut self, color: Rgba) {
        self.frame.clear(color);
    }
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        self.frame.fill_rect(x, y, w, h, color);
    }
    fn page(&mut self, index: usize, x: i32, y: i32) {
        if let Some(bitmap) = self.bitmaps.get(&index) {
            self.frame.draw_page(bitmap, x, y);
        }
    }
    fn page_part(&mut self, index: usize, src: [f32; 4], dest: [f32; 4], opacity: f32) {
        if let Some(bitmap) = self.bitmaps.get(&index) {
            self.frame.draw_page_part(bitmap, src, dest, opacity);
        }
    }
    fn page_part_rotated(
        &mut self,
        index: usize,
        src: [f32; 4],
        dest: [f32; 4],
        opacity: f32,
        degrees: f32,
    ) {
        if let Some(bitmap) = self.bitmaps.get(&index) {
            self.frame
                .draw_page_part_rotated(bitmap, src, dest, opacity, degrees);
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

/// Inputs handled since the last frame, for the performance log.
struct PendingInput {
    first: Instant,
    kind: &'static str,
    age_ms: u32,
    handle: std::time::Duration,
    count: u32,
}

struct Shell {
    view: SlideWindow,
    gpu: Option<Gpu>,
    perf: Option<std::fs::File>,
    file: Option<std::path::PathBuf>,
    pending: Vec<Request>,
    dirty: bool,
    /// Neighbouring slides may still be drawn ahead (when idle).
    prefetching: bool,
    pending_input: Option<PendingInput>,
    last_frame: [std::time::Duration; 3],
    /// Clock origin of slide show animations.
    epoch: Instant,
    /// Previous animation frame (frame pacing in the performance log).
    anim_frame: Option<Instant>,
}

impl Shell {
    fn save_to(&mut self, path: &std::path::Path) -> bool {
        let written = self
            .view
            .view()
            .save_pptx()
            .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()));
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
            || "Presentation1.pptx".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let Some(path) = win32::file_dialog(true, &[("Presentation (*.pptx)", "*.pptx")], &name)
        else {
            return false;
        };
        self.file = Some(path.clone());
        if self.save_to(&path) {
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
                Request::Save => {
                    redraw |= match self.file.clone() {
                        Some(path) => self.save_to(&path),
                        None => self.save_as(),
                    };
                }
                Request::SaveAs => redraw |= self.save_as(),
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
                            "Presentations (*.pptx;*.ppt;*.odp;*.potx;*.ppsx;*.pptm)",
                            "*.pptx;*.ppt;*.odp;*.potx;*.ppsx;*.pptm",
                        ),
                        ("All files", "*.*"),
                    ];
                    if let Some(path) = win32::file_dialog(false, &filters, "") {
                        spawn_self(Some(&path));
                    }
                }
                Request::ExportPdf | Request::Print => {
                    let print = r == Request::Print;
                    let bytes = match self.view.view().engine().export_pdf() {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("export: {e}");
                            continue;
                        }
                    };
                    if print {
                        let path = std::env::temp_dir()
                            .join(format!("blueslide-print-{}.pdf", std::process::id()));
                        if std::fs::write(&path, bytes).is_ok()
                            && !win32::shell_open(&path, "print")
                        {
                            win32::shell_open(&path, "open");
                        }
                    } else {
                        let name = self.file.as_ref().and_then(|p| p.file_stem()).map_or_else(
                            || "Presentation1.pdf".to_owned(),
                            |n| format!("{}.pdf", n.to_string_lossy()),
                        );
                        if let Some(path) =
                            win32::file_dialog(true, &[("PDF (*.pdf)", "*.pdf")], &name)
                        {
                            if let Err(e) = std::fs::write(&path, bytes) {
                                eprintln!("export {}: {e}", path.display());
                            }
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
        self.view
            .set_clock(self.epoch.elapsed().as_secs_f64() * 1e3);
        self.view.prepare(&mut Store {
            device: &gpu.device,
            bitmaps: &mut gpu.bitmaps,
        })?;
        let t1 = Instant::now();
        let (view, bitmaps) = (&self.view, &gpu.bitmaps);
        let mut painted = std::time::Duration::ZERO;
        let r = gpu.surface.frame(&gpu.device, |frame| {
            let p0 = Instant::now();
            view.paint(&mut WinPainter { frame, bitmaps });
            painted = p0.elapsed();
        });
        self.last_frame = [t1 - t0, painted, t1.elapsed().saturating_sub(painted)];
        r
    }
}

/// Start another BlueSlide window (with `file`, or a new presentation).
fn spawn_self(file: Option<&std::path::Path>) {
    if let Ok(exe) = std::env::current_exe() {
        let mut cmd = std::process::Command::new(exe);
        if let Some(f) = file {
            cmd.arg(f);
        }
        let _ = cmd.spawn();
    }
}

impl Handler for Shell {
    fn created(&mut self, hwnd: isize, width: u32, height: u32, scale: f32) -> Result<(), String> {
        refresh_text_scale();
        let device = Device::new()?;
        if let Err(e) = device.update_window_monitor(hwnd) {
            eprintln!("monitor text settings: {e}");
        }
        device.set_text_rendering(match std::env::var("BLUEOFFICE_TEXT").as_deref() {
            Ok("system") => TextRendering::System,
            Ok("light") => TextRendering::light_cleartype(),
            _ => TextRendering::document_default(),
        });
        let surface = WindowSurface::new(&device, hwnd, width, height)?;
        self.gpu = Some(Gpu {
            device,
            surface,
            bitmaps: HashMap::new(),
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
        self.prefetching = true;
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
        self.prefetching = true;
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
        self.dirty || self.prefetching || self.view.animating()
    }

    fn frame(&mut self) {
        if self.view.animating() {
            // Transitions and effects: one frame per display refresh.
            self.dirty = true;
            let now = Instant::now();
            if let (Some(file), Some(prev)) = (self.perf.as_mut(), self.anim_frame) {
                let _ = writeln!(
                    file,
                    r#"{{"anim_frame_ms":{:.3}}}"#,
                    (now - prev).as_secs_f64() * 1e3
                );
            }
            self.anim_frame = Some(now);
        } else {
            self.anim_frame = None;
        }
        if !self.dirty {
            // Idle: draw one slide ahead of need, without presenting.
            self.prefetching = match self.gpu.as_mut() {
                Some(gpu) => self
                    .view
                    .prefetch(&mut Store {
                        device: &gpu.device,
                        bitmaps: &mut gpu.bitmaps,
                    })
                    .unwrap_or(false),
                None => false,
            };
            return;
        }
        self.dirty = false;
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
