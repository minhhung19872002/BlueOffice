//! Report whether Direct2D device creation selects hardware or WARP.

#[cfg(windows)]
fn main() -> Result<(), String> {
    let device = bo_gpu_win::Device::new()?;
    println!("Direct2D device: {:?}", device.driver());
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The Direct2D device probe requires Windows.");
}
