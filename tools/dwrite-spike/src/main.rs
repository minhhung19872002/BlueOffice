//! Isolated experiment: rasterize an engine-shaped glyph run with DirectWrite.
//! This is not the BlueOffice renderer and does not alter the document layout.

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::mem::ManuallyDrop;
    use windows::core::{IUnknown, Interface};
    use windows::Win32::Graphics::DirectWrite::*;

    let path = std::env::args().nth(1).unwrap_or_else(|| {
        format!(
            "{}/../../assets/fonts/Tinos-Regular.ttf",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    let bytes = std::fs::read(path)?;
    let text = std::env::args().nth(2).unwrap_or_else(|| "Việt Nam".into());
    let mut fonts = bo_text::FontManager::new();
    let face_id = *fonts
        .add_font(bytes.clone())
        .first()
        .ok_or("invalid font")?;
    let shaped = fonts.shape(face_id, &text);
    if shaped.is_empty() || shaped.iter().any(|g| g.glyph_id == 0) {
        return Err("text is empty or the font lacks a glyph".into());
    }
    let em = 20.0_f32;
    let scale = em / f32::from(fonts.info(face_id).units_per_em);
    let indices: Vec<u16> = shaped.iter().map(|g| g.glyph_id).collect();
    let advances: Vec<f32> = shaped.iter().map(|g| g.x_advance as f32 * scale).collect();
    let offsets: Vec<DWRITE_GLYPH_OFFSET> = shaped
        .iter()
        .map(|g| DWRITE_GLYPH_OFFSET {
            advanceOffset: g.x_offset as f32 * scale,
            ascenderOffset: g.y_offset as f32 * scale,
        })
        .collect();
    let glyph_count = u32::try_from(indices.len())?;
    let length = u32::try_from(bytes.len())?;

    // This factory owns a registered in-memory loader until every font and analysis is gone.
    // The API copies the supplied bytes because ownerObject is null.
    unsafe {
        let factory5: IDWriteFactory5 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)?;
        let factory: IDWriteFactory = factory5.cast()?;
        let loader = factory5.CreateInMemoryFontFileLoader()?;
        factory.RegisterFontFileLoader(&loader)?;
        let result = (|| -> windows::core::Result<_> {
            let file = loader.CreateInMemoryFontFileReference(
                &factory,
                bytes.as_ptr().cast(),
                length,
                None::<&IUnknown>,
            )?;
            let face = factory.CreateFontFace(
                DWRITE_FONT_FACE_TYPE_TRUETYPE,
                &[Some(file)],
                0,
                DWRITE_FONT_SIMULATIONS_NONE,
            )?;
            let mut samples = Vec::new();
            for dpr in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
                let mut run = DWRITE_GLYPH_RUN {
                    fontFace: ManuallyDrop::new(Some(face.clone())),
                    fontEmSize: em,
                    glyphCount: glyph_count,
                    glyphIndices: indices.as_ptr(),
                    glyphAdvances: advances.as_ptr(),
                    glyphOffsets: offsets.as_ptr(),
                    ..Default::default()
                };
                let analysis = factory.CreateGlyphRunAnalysis(
                    &run,
                    dpr,
                    None,
                    DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
                    DWRITE_MEASURING_MODE_NATURAL,
                    0.0,
                    25.0,
                );
                ManuallyDrop::drop(&mut run.fontFace);
                let analysis = analysis?;
                let bounds = analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1)?;
                let width = (bounds.right - bounds.left).max(0) as usize;
                let height = (bounds.bottom - bounds.top).max(0) as usize;
                let mut alpha = vec![0; width * height * 3];
                analysis.CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &bounds, &mut alpha)?;
                samples.push((
                    dpr,
                    width,
                    height,
                    alpha.iter().filter(|&&x| x != 0).count(),
                    alpha.iter().map(|&x| u64::from(x)).sum::<u64>(),
                ));
            }
            Ok(samples)
        })();
        factory.UnregisterFontFileLoader(&loader)?;
        for (dpr, width, height, nonzero, alpha_sum) in result? {
            if nonzero == 0 {
                return Err("DirectWrite returned an empty glyph mask".into());
            }
            println!(
                "glyphs={glyph_count}, dpr={dpr}, bounds={width}x{height}, nonzero_samples={nonzero}, alpha_sum={alpha_sum}"
            );
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The DirectWrite experiment requires Windows.");
}
