//! Report whether Direct2D device creation selects hardware or WARP.

#[cfg(windows)]
fn main() -> Result<(), String> {
    let device = bo_gpu_win::Device::new()?;
    println!("Direct2D device: {:?}", device.driver());
    let (background, rectangle) = device.verify_offscreen()?;
    if background != [255, 255, 255, 255] || rectangle != [0, 0, 255, 255] {
        return Err(format!(
            "unexpected BGRA pixels: {background:?}, {rectangle:?}"
        ));
    }
    println!("Offscreen rectangle and background verified");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The Direct2D device probe requires Windows.");
}
