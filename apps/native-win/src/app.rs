//! The application: document view, GPU page store and presentation (safe code only).
use crate::win32::{self, Handler};
use bo_engine::{Engine, SystemFontIndex};
use bo_gpu_win::{Device, Frame, PageBitmap, TextRendering, WindowSurface};
use bo_render::{PageList, PathSeg, Rgba};
use bo_ui::{
    DocView, DocWindow, Input, NcHit, PageStore, Painter, Request, TextMeasure, TextStyle,
};
use std::collections::HashMap;
use std::io::Write;
use std::time::Instant;

/// Open the file named on the command line (or a new document) and run the window.
pub fn run() -> Result<(), String> {
    let mut engine = Engine::with_bundled_fonts();
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
            .new_document("Tài liệu mới.docx")
            .map_err(|e| e.to_string())?;
        "Tài liệu mới.docx".into()
    };
    let view = DocWindow::new(DocView::new(engine)?, "vi-VN", &name);
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
        self.frame.text(text, x, y, style.size, style.bold, color);
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
            Some(d) => d.measure_text(text, style.size, style.bold),
            None => (
                text.chars().count() as f32 * style.size * 0.55,
                style.size * 1.33,
            ),
        }
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

    fn render(&mut self) -> Result<(), String> {
        let Some(gpu) = self.gpu.as_mut() else {
            return Ok(());
        };
        self.view.prepare(&mut Store {
            device: &gpu.device,
            pages: &mut gpu.pages,
        })?;
        let (view, pages) = (&self.view, &gpu.pages.bitmaps);
        gpu.surface.frame(&gpu.device, |frame| {
            view.paint(&mut WinPainter { frame, pages });
        })
    }
}

impl Handler for Shell {
    fn created(&mut self, hwnd: isize, width: u32, height: u32, scale: f32) -> Result<(), String> {
        let device = Device::new()?;
        // BLUEOFFICE_TEXT=system|light|gray compares text rendering without a rebuild.
        device.set_text_rendering(match std::env::var("BLUEOFFICE_TEXT").as_deref() {
            Ok("system") => TextRendering::System,
            Ok("gray") => TextRendering::Grayscale,
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
        };
        let start = Instant::now();
        let m = Measure(self.gpu.as_ref().map(|g| &g.device));
        let mut redraw = self.view.handle(input, &m);
        for r in self.view.take_requests() {
            if r == Request::Save {
                redraw |= self.save();
            } else {
                self.pending.push(r);
            }
        }
        if !redraw {
            return;
        }
        let handled = start.elapsed();
        if let Err(e) = self.render() {
            eprintln!("render: {e}");
        }
        if let (Some(file), false) = (
            self.perf.as_mut(),
            matches!(kind, "blink" | "move" | "tick"),
        ) {
            let total = start.elapsed();
            let _ = writeln!(
                file,
                r#"{{"input":"{kind}","age_ms":{age_ms},"handle_ms":{:.3},"present_ms":{:.3},"total_ms":{:.3}}}"#,
                handled.as_secs_f64() * 1e3,
                total.saturating_sub(handled).as_secs_f64() * 1e3,
                total.as_secs_f64() * 1e3 + f64::from(age_ms),
            );
        }
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
}
