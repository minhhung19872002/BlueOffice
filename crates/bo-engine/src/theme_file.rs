//! Theme files (`.thmx`, CORE-FMT-001) for BlueDoc and BlueSlide: read the theme
//! of a theme file (or of another document, workbook or presentation) and save
//! the open file's theme as a theme file. BlueSheet: `sheet::theme_file`.
//!
//! A theme is the DrawingML `a:theme` element (ECMA-376 Part 1, §20.1.6.9: colour
//! scheme §20.1.4.1.14, font scheme §20.1.4.1.18, format scheme §20.1.4.1.14 fmtScheme)
//! stored in a theme part (§14.2.7). The package layout of a theme file is in
//! [`crate::variant::thmx_package`].

use crate::variant::{theme_of_package, theme_of_thmx, thmx_package};
use crate::{Engine, EngineError, Result};
use base64::Engine as _;
use bo_model::ThemeFonts;

/// Theme part XML of a theme file, or of the theme of a `.docx` / `.xlsx` / `.pptx`
/// (any variant).
pub fn theme_xml_of(bytes: &[u8]) -> Option<Vec<u8>> {
    theme_of_thmx(bytes).or_else(|| theme_of_package(bytes))
}

/// Base64 (as sent by hosts) → theme part XML.
pub fn theme_xml_of_b64(b64: &str) -> Option<Vec<u8>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    theme_xml_of(&bytes)
}

/// Font scheme of a theme (§20.1.4.1.18 fontScheme: §20.1.4.1.24 majorFont,
/// §20.1.4.1.25 minorFont) as document theme fonts; `None` when the theme does not parse.
pub fn theme_fonts(xml: &[u8]) -> Option<ThemeFonts> {
    let t = bo_drawingml::theme::parse_theme(xml, "/word/theme/theme1.xml").ok()?;
    let f = &t.fonts;
    Some(ThemeFonts {
        major_latin: f.major.latin.typeface.clone(),
        major_east_asia: f.major.ea.typeface.clone(),
        major_cs: f.major.cs.typeface.clone(),
        minor_latin: f.minor.latin.typeface.clone(),
        minor_east_asia: f.minor.ea.typeface.clone(),
        minor_cs: f.minor.cs.typeface.clone(),
    })
}

/// sRGB of the colour scheme slots of a theme part (§20.1.6.2 clrScheme), which resolve
/// the theme colours of runs (§17.3.2.6).
pub fn theme_colors(xml: &[u8]) -> Vec<(String, bo_model::Color)> {
    bo_drawingml::theme::parse_theme(xml, "/word/theme/theme1.xml")
        .map(|t| bo_drawingml::theme::scheme_rgb(&t))
        .unwrap_or_default()
}

impl Engine {
    /// The open document's or presentation's theme as a `.thmx` package (for a
    /// presentation, the theme of its first slide master). Errors with
    /// [`EngineError::Unsupported`] when the file has no theme part.
    pub fn theme_file(&self) -> Result<Vec<u8>> {
        let xml = if let Some(doc) = self.document() {
            match &doc.design.theme_xml {
                Some(x) => Some(x.as_bytes().to_vec()),
                None => theme_of_package(&bo_docx::write_docx(doc)?),
            }
        } else {
            let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
            // While the master view is open the deck's slides are set aside.
            let assembled = crate::slide_edit::master::assemble(pres, &self.slides)
                .or_else(|| crate::slide_edit::page_master::assemble(pres, &self.slides));
            theme_of_package(&bo_pptx::write_pptx(assembled.as_ref().unwrap_or(pres))?)
        };
        let xml = xml.ok_or_else(|| EngineError::Unsupported("noTheme".into()))?;
        thmx_package(&xml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fonts() {
        let xml = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="T"><a:themeElements><a:clrScheme name="c"><a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="111111"/></a:dk2><a:lt2><a:srgbClr val="EEEEEE"/></a:lt2><a:accent1><a:srgbClr val="123456"/></a:accent1><a:accent2><a:srgbClr val="123456"/></a:accent2><a:accent3><a:srgbClr val="123456"/></a:accent3><a:accent4><a:srgbClr val="123456"/></a:accent4><a:accent5><a:srgbClr val="123456"/></a:accent5><a:accent6><a:srgbClr val="123456"/></a:accent6><a:hlink><a:srgbClr val="0000FF"/></a:hlink><a:folHlink><a:srgbClr val="800080"/></a:folHlink></a:clrScheme><a:fontScheme name="f"><a:majorFont><a:latin typeface="Tinos"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Carlito"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="m"/></a:themeElements></a:theme>"#;
        let f = theme_fonts(xml.as_bytes()).unwrap();
        assert_eq!(
            (f.major_latin.as_str(), f.minor_latin.as_str()),
            ("Tinos", "Carlito")
        );
        let pkg = thmx_package(xml.as_bytes()).unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(pkg);
        assert_eq!(theme_xml_of_b64(&b64).unwrap(), xml.as_bytes());
    }
}
