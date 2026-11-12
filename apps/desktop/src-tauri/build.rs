//! Build script: generates the Tauri context (`tauri_build::build`).
//!
//! Windows: the app manifest declares Windows 10/11 support, which layered child windows need
//! (snap-layouts hit area over the custom maximise button, ADR-0016).
fn main() {
    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("windows-app-manifest.xml"));
    if let Err(e) = tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
    {
        panic!("tauri build script failed: {e:#}");
    }
}
