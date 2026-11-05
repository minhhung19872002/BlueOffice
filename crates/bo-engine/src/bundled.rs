//! Fonts compiled into native builds (OFL / Apache-2.0, see THIRD_PARTY_NOTICES.md).

macro_rules! font {
    ($f:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/", $f)).as_slice()
    };
}

/// Bundled font files.
pub const FONTS: &[&[u8]] = &[
    font!("Tinos-Regular.ttf"),
    font!("Tinos-Bold.ttf"),
    font!("Tinos-Italic.ttf"),
    font!("Tinos-BoldItalic.ttf"),
    font!("LiberationSans-Regular.ttf"),
    font!("LiberationSans-Bold.ttf"),
    font!("LiberationSans-Italic.ttf"),
    font!("LiberationSans-BoldItalic.ttf"),
    font!("Carlito-Regular.ttf"),
    font!("Carlito-Bold.ttf"),
    font!("Carlito-Italic.ttf"),
    font!("Carlito-BoldItalic.ttf"),
];
