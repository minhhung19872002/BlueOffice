//! Design › Colors › Customize Colors… and Fonts › Customize Fonts… (BlueDoc round 15): the
//! document's theme part with new §20.1.6.2 clrScheme slots and / or §20.1.4.1.24 majorFont /
//! §20.1.4.1.25 minorFont Latin typefaces (the first BlueOffice theme when the document has
//! none). Applied like a theme file (`Session::apply_theme_xml`, one undo step).
use bo_edit::design::THEMES;
use bo_model::slide::Color;

/// §20.1.6.2 clrScheme slots in schema order.
const SLOTS: [&str; 12] = [
    "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

/// The theme XML with these parts changed; `None` when there is no theme to start from or a
/// colour is not `RRGGBB`.
pub(crate) fn parts_theme(
    doc: &bo_model::Document,
    colors: Option<&[String]>,
    major: Option<&str>,
    minor: Option<&str>,
    name: Option<&str>,
) -> Option<String> {
    let mut theme = crate::doc_chart::doc_theme(doc).or_else(|| {
        let xml = bo_edit::design::theme_xml(THEMES.first()?);
        bo_drawingml::theme::parse_theme(xml.as_bytes(), "/word/theme/theme1.xml").ok()
    })?;
    if let Some(colors) = colors {
        let mut slots = Vec::with_capacity(SLOTS.len());
        for (slot, hex) in SLOTS.iter().zip(colors) {
            let v = u32::from_str_radix(hex.trim().trim_start_matches('#'), 16).ok()?;
            slots.push(((*slot).to_owned(), Color::srgb(v & 0x00ff_ffff)));
        }
        if slots.len() != SLOTS.len() {
            return None;
        }
        theme.colors.colors = slots;
        if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
            n.trim().clone_into(&mut theme.colors.name);
        }
    }
    if let Some(f) = major.filter(|f| !f.trim().is_empty()) {
        f.trim().clone_into(&mut theme.fonts.major.latin.typeface);
    }
    if let Some(f) = minor.filter(|f| !f.trim().is_empty()) {
        f.trim().clone_into(&mut theme.fonts.minor.latin.typeface);
    }
    if colors.is_none() && (major.is_some() || minor.is_some()) {
        if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
            n.trim().clone_into(&mut theme.fonts.name);
        }
    }
    String::from_utf8(bo_drawingml::theme::write_theme(&theme)).ok()
}

#[cfg(test)]
mod tests {
    use crate::edit::Command;
    use crate::Engine;

    #[test]
    fn custom_colours_and_fonts_become_the_theme() {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("a.docx").expect("new");
        let colors: Vec<String> = [
            "000000", "FFFFFF", "1F3864", "E7E6E6", "C00000", "ED7D31", "A5A5A5", "FFC000",
            "5B9BD5", "70AD47", "0563C1", "954F72",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        e.command(Command::ApplyThemeParts {
            colors: Some(colors),
            major: None,
            minor: None,
            name: Some("Mine".into()),
        })
        .expect("colors");
        let d = e.document().expect("doc");
        let accent1 = d
            .design
            .theme_colors
            .iter()
            .find(|(k, _)| k == "accent1")
            .map(|(_, c)| c.to_hex());
        assert_eq!(accent1.as_deref(), Some("C00000"));
        assert!(d
            .design
            .theme_xml
            .as_deref()
            .is_some_and(|x| x.contains("name=\"Mine\"")));
        e.command(Command::ApplyThemeParts {
            colors: None,
            major: Some("Cambria".into()),
            minor: Some("Georgia".into()),
            name: Some("Pair".into()),
        })
        .expect("fonts");
        let x = e
            .document()
            .and_then(|d| d.design.theme_xml.clone())
            .expect("theme");
        assert!(x.contains("typeface=\"Cambria\"") && x.contains("typeface=\"Georgia\""));
        // One undo step each.
        e.command(Command::Undo).expect("undo");
        let x = e
            .document()
            .and_then(|d| d.design.theme_xml.clone())
            .expect("theme");
        assert!(!x.contains("typeface=\"Georgia\""));
    }
}
