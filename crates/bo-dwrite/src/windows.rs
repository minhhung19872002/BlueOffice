use crate::{Glyph, Mask};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use windows::core::{IUnknown, Interface, BOOL};
use windows::Win32::Graphics::DirectWrite::*;

// A malformed run must not allocate an unbounded bitmap through the platform API.
const MAX_MASK_BYTES: usize = 64 * 1024 * 1024;

struct Registration {
    factory: IDWriteFactory,
    loader: IDWriteInMemoryFontFileLoader,
}

impl Drop for Registration {
    fn drop(&mut self) {
        // SAFETY: this guard owns both COM interfaces and outlives every font file and face
        // created by its callback. Unregister errors cannot be reported from Drop.
        unsafe {
            let _ = self.factory.UnregisterFontFileLoader(&self.loader);
        }
    }
}

struct OwnedRun(DWRITE_GLYPH_RUN);

impl Drop for OwnedRun {
    fn drop(&mut self) {
        // SAFETY: the face was initialized exactly once and is dropped exactly once here,
        // including during unwinding from the callback.
        unsafe {
            ManuallyDrop::drop(&mut self.0.fontFace);
        }
    }
}

/// Owned glyph data and font resources, valid until this value is dropped.
pub struct PreparedGlyphRun {
    // Fields drop in declaration order: the font face must be released before the loader.
    run: OwnedRun,
    _indices: Vec<u16>,
    _advances: Vec<f32>,
    _offsets: Vec<DWRITE_GLYPH_OFFSET>,
    _registration: Rc<Registration>,
}

fn valid_run(glyphs: &[Glyph], size: f32) -> bool {
    !glyphs.is_empty()
        && size.is_finite()
        && size > 0.0
        && glyphs.iter().all(|g| g.x.is_finite() && g.y.is_finite())
}

fn register() -> Result<Registration, String> {
    // SAFETY: the factory and loader are owned COM references; the guard unregisters on drop.
    unsafe {
        let factory5: IDWriteFactory5 =
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED).map_err(|e| e.to_string())?;
        let factory: IDWriteFactory = factory5.cast().map_err(|e| e.to_string())?;
        let loader = factory5
            .CreateInMemoryFontFileLoader()
            .map_err(|e| e.to_string())?;
        factory
            .RegisterFontFileLoader(&loader)
            .map_err(|e| e.to_string())?;
        Ok(Registration { factory, loader })
    }
}

fn load_face(reg: &Registration, font: &[u8], face_index: u32) -> Result<IDWriteFontFace, String> {
    if font.is_empty() {
        return Err("empty font data".into());
    }
    let length = u32::try_from(font.len()).map_err(|e| e.to_string())?;
    // SAFETY: the in-memory loader copies `font` because ownerObject is null; the returned face
    // is an owned COM reference and is released before `reg` unregisters its loader.
    unsafe {
        let file = reg
            .loader
            .CreateInMemoryFontFileReference(
                &reg.factory,
                font.as_ptr().cast(),
                length,
                None::<&IUnknown>,
            )
            .map_err(|e| e.to_string())?;
        let mut supported = BOOL(0);
        let mut file_type = DWRITE_FONT_FILE_TYPE::default();
        let mut face_type = DWRITE_FONT_FACE_TYPE::default();
        let mut faces = 0;
        file.Analyze(
            &mut supported,
            &mut file_type,
            Some(&mut face_type),
            &mut faces,
        )
        .map_err(|e| e.to_string())?;
        if !supported.as_bool() || face_index >= faces {
            return Err("unsupported font or invalid collection index".into());
        }
        reg.factory
            .CreateFontFace(
                face_type,
                &[Some(file)],
                face_index,
                DWRITE_FONT_SIMULATIONS_NONE,
            )
            .map_err(|e| e.to_string())
    }
}

fn build_run(
    registration: Rc<Registration>,
    face: IDWriteFontFace,
    glyphs: &[Glyph],
    size: f32,
) -> Result<PreparedGlyphRun, String> {
    let count = u32::try_from(glyphs.len()).map_err(|e| e.to_string())?;
    let indices: Vec<u16> = glyphs.iter().map(|g| g.id).collect();
    // Zero advances keep every engine-provided offset exact, including combining marks.
    let advances = vec![0.0; glyphs.len()];
    let offsets: Vec<DWRITE_GLYPH_OFFSET> = glyphs
        .iter()
        .map(|g| DWRITE_GLYPH_OFFSET {
            advanceOffset: g.x,
            ascenderOffset: -g.y,
        })
        .collect();
    let run = OwnedRun(DWRITE_GLYPH_RUN {
        fontFace: ManuallyDrop::new(Some(face)),
        fontEmSize: size,
        glyphCount: count,
        glyphIndices: indices.as_ptr(),
        glyphAdvances: advances.as_ptr(),
        glyphOffsets: offsets.as_ptr(),
        ..Default::default()
    });
    Ok(PreparedGlyphRun {
        run,
        _indices: indices,
        _advances: advances,
        _offsets: offsets,
        _registration: registration,
    })
}

/// DirectWrite font faces keyed by the caller's face id, sharing one factory and loader.
///
/// Loading a face copies its font file once; later runs reuse the face, so a page no longer
/// copies a font per glyph run. A key whose data pointer or length changes is reloaded.
pub struct FaceCache {
    // Faces drop before the shared registration that unregisters their loader.
    faces: HashMap<u32, (usize, usize, IDWriteFontFace)>,
    registration: Rc<Registration>,
}

impl FaceCache {
    /// Create an empty cache with its own isolated DirectWrite factory.
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            faces: HashMap::new(),
            registration: Rc::new(register()?),
        })
    }

    /// Total size of the cached font files (each was copied once by DirectWrite).
    pub fn font_bytes(&self) -> usize {
        self.faces.values().map(|(_, len, _)| *len).sum()
    }

    /// Release every cached face.
    pub fn clear(&mut self) {
        self.faces.clear();
    }

    /// Number of cached faces.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// Whether no face is cached.
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Prepare a positioned run, loading face `key` from `font` on first use.
    pub fn run(
        &mut self,
        key: u32,
        font: &[u8],
        face_index: u32,
        glyphs: &[Glyph],
        size: f32,
    ) -> Result<PreparedGlyphRun, String> {
        if !valid_run(glyphs, size) {
            return Err("invalid DirectWrite glyph run".into());
        }
        let id = (font.as_ptr() as usize, font.len());
        let face = match self.faces.get(&key) {
            Some((ptr, len, face)) if (*ptr, *len) == id => face.clone(),
            _ => {
                let face = load_face(&self.registration, font, face_index)?;
                self.faces.insert(key, (id.0, id.1, face.clone()));
                face
            }
        };
        build_run(Rc::clone(&self.registration), face, glyphs, size)
    }
}

impl PreparedGlyphRun {
    /// Load a font face and retain all buffers needed for the positioned run.
    pub fn new(font: &[u8], face_index: u32, glyphs: &[Glyph], size: f32) -> Result<Self, String> {
        if font.is_empty() || !valid_run(glyphs, size) {
            return Err("invalid DirectWrite glyph run".into());
        }
        let registration = Rc::new(register()?);
        let face = load_face(&registration, font, face_index)?;
        build_run(registration, face, glyphs, size)
    }

    /// Borrow the underlying run while its owner and all backing buffers remain alive.
    pub fn as_raw(&self) -> &DWRITE_GLYPH_RUN {
        &self.run.0
    }
}

pub(crate) fn rasterize(
    font: &[u8],
    face_index: u32,
    glyphs: &[Glyph],
    size: f32,
    pixels_per_unit: f32,
    origin: (f32, f32),
) -> Result<Mask, String> {
    if font.is_empty()
        || glyphs.is_empty()
        || !size.is_finite()
        || size <= 0.0
        || !pixels_per_unit.is_finite()
        || pixels_per_unit <= 0.0
        || !origin.0.is_finite()
        || !origin.1.is_finite()
        || glyphs.iter().any(|g| !g.x.is_finite() || !g.y.is_finite())
    {
        return Err("invalid DirectWrite glyph run".into());
    }
    let length = u32::try_from(font.len()).map_err(|e| e.to_string())?;
    let count = u32::try_from(glyphs.len()).map_err(|e| e.to_string())?;
    let indices: Vec<u16> = glyphs.iter().map(|g| g.id).collect();
    // Each glyph carries its complete engine-provided offset; zero advances prevent
    // DirectWrite from re-spacing the run, including zero-advance combining marks.
    let advances = vec![0.0; glyphs.len()];
    let offsets: Vec<DWRITE_GLYPH_OFFSET> = glyphs
        .iter()
        .map(|g| DWRITE_GLYPH_OFFSET {
            advanceOffset: g.x,
            ascenderOffset: -g.y,
        })
        .collect();

    // SAFETY: the DirectWrite interfaces own their COM references. The in-memory loader
    // copies `font` because ownerObject is null. The glyph arrays stay alive through analysis,
    // and the loader is unregistered only after the file, face and analysis are dropped.
    unsafe {
        let factory5: IDWriteFactory5 =
            DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED).map_err(|e| e.to_string())?;
        let factory: IDWriteFactory = factory5.cast().map_err(|e| e.to_string())?;
        let loader = factory5
            .CreateInMemoryFontFileLoader()
            .map_err(|e| e.to_string())?;
        factory
            .RegisterFontFileLoader(&loader)
            .map_err(|e| e.to_string())?;
        let result = (|| -> Result<Mask, String> {
            let file = loader
                .CreateInMemoryFontFileReference(
                    &factory,
                    font.as_ptr().cast(),
                    length,
                    None::<&IUnknown>,
                )
                .map_err(|e| e.to_string())?;
            let mut supported = BOOL(0);
            let mut file_type = DWRITE_FONT_FILE_TYPE::default();
            let mut face_type = DWRITE_FONT_FACE_TYPE::default();
            let mut faces = 0;
            file.Analyze(
                &mut supported,
                &mut file_type,
                Some(&mut face_type),
                &mut faces,
            )
            .map_err(|e| e.to_string())?;
            if !supported.as_bool() || face_index >= faces {
                return Err("unsupported font or invalid collection index".into());
            }
            let face = factory
                .CreateFontFace(
                    face_type,
                    &[Some(file)],
                    face_index,
                    DWRITE_FONT_SIMULATIONS_NONE,
                )
                .map_err(|e| e.to_string())?;
            let mut run = DWRITE_GLYPH_RUN {
                fontFace: ManuallyDrop::new(Some(face)),
                fontEmSize: size,
                glyphCount: count,
                glyphIndices: indices.as_ptr(),
                glyphAdvances: advances.as_ptr(),
                glyphOffsets: offsets.as_ptr(),
                ..Default::default()
            };
            let analysis = factory.CreateGlyphRunAnalysis(
                &run,
                pixels_per_unit,
                None,
                DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
                DWRITE_MEASURING_MODE_NATURAL,
                origin.0,
                origin.1,
            );
            ManuallyDrop::drop(&mut run.fontFace);
            let analysis = analysis.map_err(|e| e.to_string())?;
            let bounds = analysis
                .GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1)
                .map_err(|e| e.to_string())?;
            let width = usize::try_from(i64::from(bounds.right) - i64::from(bounds.left))
                .map_err(|e| e.to_string())?;
            let height = usize::try_from(i64::from(bounds.bottom) - i64::from(bounds.top))
                .map_err(|e| e.to_string())?;
            let bytes = width
                .checked_mul(height)
                .and_then(|n| n.checked_mul(3))
                .filter(|n| *n <= MAX_MASK_BYTES)
                .ok_or("DirectWrite mask too large")?;
            let mut coverage = vec![0; bytes];
            if bytes != 0 {
                analysis
                    .CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &bounds, &mut coverage)
                    .map_err(|e| e.to_string())?;
            }
            Ok(Mask {
                left: bounds.left,
                top: bounds.top,
                right: bounds.right,
                bottom: bounds.bottom,
                coverage,
            })
        })();
        factory
            .UnregisterFontFileLoader(&loader)
            .map_err(|e| e.to_string())?;
        result
    }
}
