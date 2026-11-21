//! BlueEditor round 12: the face that draws a font that is not embedded, for File ›
//! Properties › Fonts ("Actual Font"), as the renderer substitutes it (`font::FontInfo::load`).

use crate::PdfDocument;

impl PdfDocument {
    /// Name of the face that draws `/BaseFont` `base` when the file does not embed it: the
    /// family the font manager resolves for the substitute names, with "-Bold", "-Italic" or
    /// "-BoldItalic" as the name asks ("Helvetica-Bold" → "Arial-Bold" when Arial is installed,
    /// else the bundled "Liberation Sans-Bold").
    #[must_use]
    pub fn substitute_font(&self, base: &str) -> Option<String> {
        let (families, bold, italic) =
            crate::font::substitute_names(crate::font::base_name(base), false, false);
        let family = self.fonts.display_family(families.first()?)?;
        let style = match (bold, italic) {
            (true, true) => "-BoldItalic",
            (true, false) => "-Bold",
            (false, true) => "-Italic",
            (false, false) => "",
        };
        Some(format!("{family}{style}"))
    }
}

#[cfg(test)]
mod tests {
    /// The standard sans font is drawn by a sans face, keeping the bold style in the name.
    #[test]
    fn helvetica_bold_names_a_bold_sans_substitute() {
        let bytes = crate::tests::build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] >>",
        ]);
        let doc = crate::PdfDocument::open(bytes, crate::standard_fonts()).expect("open");
        let s = doc
            .substitute_font("ABCDEF+Helvetica-Bold")
            .expect("substitute");
        assert!(s.ends_with("-Bold"), "{s}");
        assert!(!s.to_ascii_lowercase().contains("times"), "{s}");
    }
}
