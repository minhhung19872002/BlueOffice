//! BlueReader / BlueEditor host: PDF documents, GPU page images and presentation (safe code
//! only; ADR-0018). Both products run the same host over a [`PdfWindow`].
use crate::win32::{self, Handler};
use bo_gpu_win::{Bitmap, Device, FontSpec, Frame, PageBitmap, TextRendering, WindowSurface};
use bo_pdfread::{PdfDocument, RenderOptions};
use bo_render::{PageList, PathSeg, Rgba};
use bo_ui::{
    EditorWindow, Font, Input, NcHit, Painter, PdfStore, PdfWindow, ReaderRequest, ReaderWindow,
    Request, TextMeasure, TextStyle,
};
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
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

/// Fonts for one document: bundled stand-ins for the standard 14 fonts, then installed fonts.
fn document_fonts() -> bo_text::FontManager {
    let mut fonts = bo_pdfread::standard_fonts();
    if std::env::var("BLUEOFFICE_SYSTEM_FONTS").as_deref() != Ok("0") {
        let cache = std::env::var_os("LOCALAPPDATA").map(|d| {
            std::path::PathBuf::from(d)
                .join("BlueOffice")
                .join("system-fonts.txt")
        });
        if let Some(dir) = cache.as_ref().and_then(|c| c.parent()) {
            let _ = std::fs::create_dir_all(dir);
        }
        fonts.set_system_fonts(bo_engine::SystemFontIndex::scan_cached(
            &bo_engine::SystemFontIndex::default_dirs(),
            cache.as_deref(),
        ));
    }
    fonts
}

fn open_pdf(path: &std::path::Path) -> Result<PdfDocument, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    PdfDocument::open(bytes, document_fonts())
}

fn recent_file(app: &str) -> Option<std::path::PathBuf> {
    let name = if app == "BlueEditor" {
        "editor-recent.txt"
    } else {
        "reader-recent.txt"
    };
    std::env::var_os("LOCALAPPDATA")
        .map(|d| std::path::PathBuf::from(d).join("BlueOffice").join(name))
}

fn load_recent(app: &str) -> Vec<std::path::PathBuf> {
    recent_file(app)
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .map(std::path::PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

fn save_recent(app: &str, list: &[std::path::PathBuf]) {
    if let Some(f) = recent_file(app) {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text: Vec<String> = list
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let _ = std::fs::write(f, text.join("\n"));
    }
}

/// Open the PDFs named on the command line and run the window (BlueEditor when `editor`).
pub fn run(editor: bool) -> Result<(), String> {
    let locale = std::env::var("BLUEOFFICE_LOCALE").unwrap_or_else(|_| "en-US".into());
    let mut window: Box<dyn PdfWindow> = if editor {
        Box::new(EditorWindow::new(&locale))
    } else {
        Box::new(ReaderWindow::new(&locale))
    };
    let app = window.app();
    window.set_recent(load_recent(app));
    for arg in std::env::args_os().skip(1) {
        let path = std::path::PathBuf::from(arg);
        let abs = std::fs::canonicalize(&path).unwrap_or(path.clone());
        let abs =
            std::path::PathBuf::from(abs.to_string_lossy().trim_start_matches(r"\\?\").to_owned());
        match open_pdf(&path) {
            Ok(doc) => {
                let name = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                window.open(doc, &name, Some(abs));
            }
            Err(e) if e.contains("password") => window.password_needed(&abs),
            Err(e) => window.set_status(&e),
        }
    }
    save_recent(app, window.recent());
    let title = window.title();
    let perf = std::env::var_os("BLUEOFFICE_NATIVE_PERF").and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok()
    });
    let initial = title.clone();
    win32::run(
        &initial,
        Box::new(Shell {
            window,
            gpu: None,
            hwnd: 0,
            title,
            perf,
            pending: Vec::new(),
            dirty: true,
            more: false,
            pending_input: None,
            last_frame: [std::time::Duration::ZERO; 3],
        }),
    )
}

struct Gpu {
    device: Device,
    surface: WindowSurface,
    pages: HashMap<usize, PageBitmap>,
}

struct Store<'a> {
    device: &'a Device,
    pages: &'a mut HashMap<usize, PageBitmap>,
}

fn pictures(
    images: &HashMap<String, Arc<bo_pdfread::Image>>,
) -> impl Fn(&str) -> Option<Bitmap> + '_ {
    move |id: &str| {
        images.get(id).map(|i| Bitmap {
            width: i.width,
            height: i.height,
            pixels: i.pixels.clone(),
        })
    }
}

impl PdfStore for Store<'_> {
    fn update(
        &mut self,
        key: usize,
        list: &PageList,
        fonts: &bo_text::FontManager,
        images: &HashMap<String, Arc<bo_pdfread::Image>>,
        scale: f32,
    ) -> Result<(), String> {
        let (w, h) = (
            (list.width * scale).ceil() as u32,
            (list.height * scale).ceil() as u32,
        );
        if self
            .pages
            .get(&key)
            .is_none_or(|b| (b.width, b.height) != (w, h))
        {
            self.pages.remove(&key);
            let b = self.device.create_page_bitmap(list, scale)?;
            self.pages.insert(key, b);
        }
        let bitmap = self.pages.get(&key).ok_or("page bitmap")?;
        let pics = pictures(images);
        self.device
            .draw_page(list, fonts, scale, &pics, None, bitmap)
    }

    fn remove(&mut self, key: usize) {
        self.pages.remove(&key);
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
        if let Some(b) = self.pages.get(&index) {
            self.frame.draw_page(b, x, y);
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

struct Shell {
    window: Box<dyn PdfWindow>,
    gpu: Option<Gpu>,
    hwnd: isize,
    title: String,
    perf: Option<std::fs::File>,
    pending: Vec<Request>,
    dirty: bool,
    /// Page images still to draw (prefetch): another frame is wanted.
    more: bool,
    pending_input: Option<PendingInput>,
    /// Last frame: page images, interface paint, end of drawing + present.
    last_frame: [std::time::Duration; 3],
}

struct PendingInput {
    first: Instant,
    kind: &'static str,
    age_ms: u32,
    handle: std::time::Duration,
    count: u32,
}

impl Shell {
    fn open_path(&mut self, path: &std::path::Path) {
        match open_pdf(path) {
            Ok(doc) => {
                let name = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                self.window.open(doc, &name, Some(path.to_path_buf()));
                save_recent(self.window.app(), self.window.recent());
            }
            Err(e) if e.contains("password") => self.window.password_needed(path),
            Err(e) => self.window.set_status(&e),
        }
    }

    /// Ask where to write `bytes` (suggested `name`) and write them.
    fn save_bytes(&mut self, name: &str, bytes: &[u8]) -> Option<std::path::PathBuf> {
        let ext = std::path::Path::new(name)
            .extension()
            .map_or_else(|| "pdf".to_owned(), |e| e.to_string_lossy().to_lowercase());
        let label = format!("{} (*.{ext})", ext.to_uppercase());
        let pattern = format!("*.{ext}");
        let path = win32::file_dialog(true, &[(label.as_str(), pattern.as_str())], name)?;
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                self.window.set_status(&format!("{}", path.display()));
                Some(path)
            }
            Err(e) => {
                self.window.set_status(&format!("{}: {e}", path.display()));
                None
            }
        }
    }

    /// Render every page of the active document as PNG at `scale` pixels per point.
    fn render_pngs(&mut self, scale: f32) -> Result<Vec<Vec<u8>>, String> {
        let gpu = self.gpu.as_ref().ok_or("no device")?;
        let view = self.window.view_mut().ok_or("no document")?;
        let doc = view.doc_mut();
        let mut out = Vec::new();
        for i in 0..doc.page_count() {
            let opts = RenderOptions {
                min_line: 1.0 / scale,
                ..RenderOptions::default()
            };
            let p = doc.render(i, opts)?;
            let pics = pictures(&p.images);
            let img = if let Ok(img) = gpu.device.render_page(&p.list, doc.fonts(), scale, &pics) {
                img
            } else {
                let p = doc.render(
                    i,
                    RenderOptions {
                        text_as_paths: true,
                        ..opts
                    },
                )?;
                let pics = pictures(&p.images);
                gpu.device.render_page(&p.list, doc.fonts(), scale, &pics)?
            };
            let mut png_bytes = Vec::new();
            {
                let mut enc = png::Encoder::new(&mut png_bytes, img.width, img.height);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                let mut w = enc.write_header().map_err(|e| e.to_string())?;
                w.write_image_data(&img.pixels).map_err(|e| e.to_string())?;
            }
            out.push(png_bytes);
        }
        Ok(out)
    }

    fn save_to(&mut self, path: &std::path::Path) -> bool {
        let Some(view) = self.window.view_mut() else {
            return false;
        };
        let bytes = view.doc_mut().save();
        match bytes.and_then(|b| std::fs::write(path, b).map_err(|e| e.to_string())) {
            Ok(()) => true,
            Err(e) => {
                self.window.set_status(&e);
                false
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle_requests(&mut self) -> bool {
        let mut redraw = false;
        for r in self.window.take_requests() {
            redraw = true;
            match r {
                ReaderRequest::Open(Some(path)) => self.open_path(&path),
                ReaderRequest::Open(None) => {
                    let filters = [("PDF (*.pdf)", "*.pdf"), ("All files", "*.*")];
                    if let Some(path) = win32::file_dialog(false, &filters, "") {
                        self.open_path(&path);
                    }
                }
                ReaderRequest::Save => {
                    if let Some(path) = self.window.path().map(std::path::Path::to_path_buf) {
                        self.save_to(&path);
                    }
                }
                ReaderRequest::SaveAs => {
                    let name = self.window.path().and_then(|p| p.file_name()).map_or_else(
                        || "document.pdf".to_owned(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    if let Some(path) = win32::file_dialog(true, &[("PDF (*.pdf)", "*.pdf")], &name)
                    {
                        if self.save_to(&path) {
                            self.window.set_path(path);
                        }
                    }
                }
                ReaderRequest::Print => {
                    // Printed through the system's PDF print verb, like BlueDoc's print path.
                    if let Some(view) = self.window.view_mut() {
                        if let Ok(bytes) = view.doc_mut().save() {
                            let path = std::env::temp_dir()
                                .join(format!("bluereader-print-{}.pdf", std::process::id()));
                            if std::fs::write(&path, bytes).is_ok()
                                && !win32::shell_open(&path, "print")
                            {
                                win32::shell_open(&path, "open");
                            }
                        }
                    }
                }
                ReaderRequest::Copy(text) => win32::set_clipboard(&text, "", ""),
                ReaderRequest::OpenUrl(url) => {
                    if url.starts_with("http://")
                        || url.starts_with("https://")
                        || url.starts_with("mailto:")
                    {
                        win32::shell_open(std::path::Path::new(&url), "open");
                    }
                }
                ReaderRequest::Window(w) => self.pending.push(w),
                ReaderRequest::Command(_) => {}
                ReaderRequest::PickFile { purpose, filters } => {
                    let f: Vec<(&str, &str)> = filters
                        .iter()
                        .map(|(a, b)| (a.as_str(), b.as_str()))
                        .collect();
                    if let Some(path) = win32::file_dialog(false, &f, "") {
                        match std::fs::read(&path) {
                            Ok(bytes) => self.window.file_picked(&purpose, &path, &bytes),
                            Err(e) => self.window.set_status(&format!("{}: {e}", path.display())),
                        }
                    }
                }
                ReaderRequest::SaveBytes { name, bytes } => {
                    self.save_bytes(&name, &bytes);
                }
                ReaderRequest::SaveMany(files) => {
                    let Some((first, bytes)) = files.first() else {
                        continue;
                    };
                    if let Some(path) = self.save_bytes(first, bytes) {
                        let dir = path.parent().map(std::path::Path::to_path_buf);
                        for (name, bytes) in files.iter().skip(1) {
                            let target = dir
                                .as_ref()
                                .map_or_else(|| std::path::PathBuf::from(name), |d| d.join(name));
                            if let Err(e) = std::fs::write(&target, bytes) {
                                self.window
                                    .set_status(&format!("{}: {e}", target.display()));
                            }
                        }
                        if let Some(d) = dir {
                            self.window
                                .set_status(&format!("{} × {}", files.len(), d.display()));
                        }
                    }
                }
                ReaderRequest::RenderPages { purpose, scale } => match self.render_pngs(scale) {
                    Ok(pngs) => self.window.pages_rendered(&purpose, pngs),
                    Err(e) => self.window.set_status(&e),
                },
                ReaderRequest::OpenWithPassword { path, password } => {
                    let r = std::fs::read(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|b| {
                            PdfDocument::open_with_password(b, document_fonts(), &password)
                        });
                    match r {
                        Ok(doc) => {
                            let name = path
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            self.window.open(doc, &name, Some(path));
                            save_recent(self.window.app(), self.window.recent());
                        }
                        Err(e) => self.window.set_status(&e),
                    }
                }
                ReaderRequest::OpenDocument { name, bytes } => {
                    match PdfDocument::open(bytes, document_fonts()) {
                        Ok(doc) => self.window.open(doc, &name, None),
                        Err(e) => self.window.set_status(&e),
                    }
                }
            }
        }
        redraw
    }

    fn render(&mut self) -> Result<(), String> {
        let Some(gpu) = self.gpu.as_mut() else {
            return Ok(());
        };
        let t0 = Instant::now();
        self.more = self.window.prepare(&mut Store {
            device: &gpu.device,
            pages: &mut gpu.pages,
        });
        let t1 = Instant::now();
        let (window, pages) = (&mut self.window, &gpu.pages);
        let mut painted = std::time::Duration::ZERO;
        let r = gpu.surface.frame(&gpu.device, |frame| {
            let p0 = Instant::now();
            window.paint(&mut WinPainter { frame, pages });
            painted = p0.elapsed();
        });
        self.last_frame = [t1 - t0, painted, t1.elapsed().saturating_sub(painted)];
        let title = self.window.title();
        if title != self.title && self.hwnd != 0 {
            win32::set_title(self.hwnd, &title);
            self.title = title;
        }
        r
    }
}

impl Handler for Shell {
    fn created(&mut self, hwnd: isize, width: u32, height: u32, scale: f32) -> Result<(), String> {
        refresh_text_scale();
        self.hwnd = hwnd;
        let device = Device::new()?;
        if let Err(e) = device.update_window_monitor(hwnd) {
            eprintln!("monitor text settings: {e}");
        }
        device.set_text_rendering(page_text());
        let surface = WindowSurface::new(&device, hwnd, width, height)?;
        self.gpu = Some(Gpu {
            device,
            surface,
            pages: HashMap::new(),
        });
        let m = Measure(self.gpu.as_ref().map(|g| &g.device));
        self.window.handle(Input::Scale(scale), &m);
        self.window.handle(
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
        let mut redraw = self.window.handle(input, &m);
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
        self.window.handle(
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
        if let Err(e) = gpu.device.update_window_monitor(hwnd) {
            eprintln!("monitor text settings: {e}");
        }
    }

    fn paint(&mut self) {
        if let Err(e) = self.render() {
            eprintln!("render: {e}");
        }
    }

    fn caret(&self) -> Option<[f32; 4]> {
        self.window.caret()
    }

    fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        self.window.nc_hit(x, y)
    }

    fn cursor(&self) -> Option<u8> {
        Some(self.window.cursor())
    }

    fn window_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.pending)
    }

    fn wants_frame(&self) -> bool {
        self.dirty || self.more
    }

    fn frame(&mut self) {
        self.dirty = false;
        if let Err(e) = self.render() {
            eprintln!("render: {e}");
        }
        if let (Some(file), Some(p)) = (self.perf.as_mut(), self.pending_input.take()) {
            let total = p.first.elapsed();
            let _ = writeln!(
                file,
                r#"{{"input":"{}","inputs":{},"age_ms":{},"handle_ms":{:.3},"prepare_ms":{:.3},"paint_ms":{:.3},"submit_ms":{:.3},"total_ms":{:.3}}}"#,
                p.kind,
                p.count,
                p.age_ms,
                p.handle.as_secs_f64() * 1e3,
                self.last_frame[0].as_secs_f64() * 1e3,
                self.last_frame[1].as_secs_f64() * 1e3,
                self.last_frame[2].as_secs_f64() * 1e3,
                total.as_secs_f64() * 1e3 + f64::from(p.age_ms),
            );
        }
    }
}

/// Gamma, enhanced contrast and ClearType level of PDF page text: the same ink coverage on body
/// text as the reference reader (docs/ui/REPORT-reader.md round 2).
const PDF_TEXT: (f32, f32, f32) = (1.8, 1.0, 1.0);

/// Page text rendering: [PDF_TEXT] unless `BLUEOFFICE_TEXT=system|gray|light|doc` asks for
/// another; `BO_PDF_TEXT=gamma,contrast,level[,gray][,nosym][,nofit]` tunes it for comparisons.
fn page_text() -> TextRendering {
    match std::env::var("BLUEOFFICE_TEXT").as_deref() {
        Ok("system") => return TextRendering::System,
        Ok("gray") => return TextRendering::Grayscale,
        Ok("light") => return TextRendering::light_cleartype(),
        Ok("doc") => return TextRendering::document_default(),
        _ => {}
    }
    let tuned = std::env::var("BO_PDF_TEXT").ok();
    let mut it = tuned.as_deref().unwrap_or("").split(',');
    let mut num = |d: f32| it.next().and_then(|v| v.trim().parse().ok()).unwrap_or(d);
    let (gamma, contrast, level) = (num(PDF_TEXT.0), num(PDF_TEXT.1), num(PDF_TEXT.2));
    let has = |s: &str| tuned.as_deref().is_some_and(|v| v.contains(s));
    let grid_fit = !has("nofit");
    // The reference draws page text with ClearType when the system uses it, grayscale else.
    if has("gray") || !bo_gpu_win::system_cleartype() {
        return TextRendering::GrayscaleTuned {
            gamma,
            contrast,
            grid_fit,
        };
    }
    TextRendering::Tuned {
        gamma,
        contrast,
        level,
        symmetric: !has("nosym"),
        grid_fit,
    }
}

/// Command-line tools: `--render in.pdf <page> out.png [zoom%]` and `--bench in.pdf [pages]`.
pub fn tool(args: &[String]) -> Result<(), String> {
    let path = args.get(1).ok_or("missing PDF path")?;
    let t0 = Instant::now();
    let mut doc = open_pdf(std::path::Path::new(path))?;
    let open_ms = t0.elapsed().as_secs_f64() * 1e3;
    let device = Device::new()?;
    device.set_text_rendering(page_text());
    // 125 % display scale at 100 % zoom unless a zoom is given.
    if args[0] == "--render" {
        let page: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(1);
        let out = args.get(3).ok_or("missing output path")?;
        let zoom: f32 = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(100.0);
        let scale = zoom / 100.0 * 96.0 / 72.0 * 1.25;
        let opts = RenderOptions {
            min_line: 1.0 / scale,
            ..RenderOptions::default()
        };
        let p = doc.render(page.saturating_sub(1), opts)?;
        let pics = pictures(&p.images);
        let img = if let Ok(i) = device.render_page(&p.list, doc.fonts(), scale, &pics) {
            i
        } else {
            let p = doc.render(
                page.saturating_sub(1),
                RenderOptions {
                    text_as_paths: true,
                    ..opts
                },
            )?;
            let pics = pictures(&p.images);
            device.render_page(&p.list, doc.fonts(), scale, &pics)?
        };
        let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&img.pixels).map_err(|e| e.to_string())?;
        println!(
            "{out}: {}x{} ({} commands)",
            img.width,
            img.height,
            p.list.commands.len()
        );
        return Ok(());
    }
    let n = doc.page_count();
    let limit: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(n).min(n);
    let scale = 96.0 / 72.0 * 1.25;
    let mut list_ms = Vec::with_capacity(limit);
    let mut gpu_ms = Vec::with_capacity(limit);
    let mut fallbacks = 0;
    let t_first = Instant::now();
    let mut first_ms = 0.0;
    for i in 0..limit {
        let a = Instant::now();
        let p = doc.render(i, RenderOptions::default())?;
        list_ms.push(a.elapsed().as_secs_f64() * 1e3);
        let b = Instant::now();
        let pics = pictures(&p.images);
        let bitmap = device.create_page_bitmap(&p.list, scale)?;
        if device
            .draw_page(&p.list, doc.fonts(), scale, &pics, None, &bitmap)
            .is_err()
        {
            fallbacks += 1;
            let p = doc.render(
                i,
                RenderOptions {
                    text_as_paths: true,
                    ..RenderOptions::default()
                },
            )?;
            let pics = pictures(&p.images);
            device.draw_page(&p.list, doc.fonts(), scale, &pics, None, &bitmap)?;
        }
        gpu_ms.push(b.elapsed().as_secs_f64() * 1e3);
        if i == 0 {
            first_ms = open_ms + t_first.elapsed().as_secs_f64() * 1e3;
        }
    }
    let stat = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let p = |q: f64| {
            v.get(((v.len() as f64 - 1.0) * q).round() as usize)
                .copied()
                .unwrap_or(0.0)
        };
        (p(0.5), p(0.95), v.last().copied().unwrap_or(0.0))
    };
    let (l50, l95, lmax) = stat(&mut list_ms);
    let (g50, g95, gmax) = stat(&mut gpu_ms);
    println!(
        r#"{{"pages":{n},"rendered":{limit},"open_ms":{open_ms:.1},"first_page_ms":{first_ms:.1},"list_p50":{l50:.2},"list_p95":{l95:.2},"list_max":{lmax:.1},"gpu_p50":{g50:.2},"gpu_p95":{g95:.2},"gpu_max":{gmax:.1},"path_fallbacks":{fallbacks},"faces":{}}}"#,
        doc.fonts().len()
    );
    Ok(())
}
