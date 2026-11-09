//! Package variants of the three OOXML document types (CORE-FMT-001): documents,
//! templates, macro-enabled documents / templates and slide shows, and theme
//! files (`.thmx`).
//!
//! A variant is fixed by the content type of the package's main part (the target of
//! the package `officeDocument` relationship, ECMA-376 Part 2 §9.3 / Part 1 §15.2.12.1),
//! never by the file extension:
//!
//! * WordprocessingML main document part, ECMA-376 Part 1 §11.3.10: `…wordprocessingml.document.main+xml`
//!   (document) and `…wordprocessingml.template.main+xml` (template);
//! * SpreadsheetML workbook part, §12.3.23: `…spreadsheetml.sheet.main+xml` / `…spreadsheetml.template.main+xml`;
//! * PresentationML presentation part, §13.3.6: `…presentationml.presentation.main+xml`,
//!   `…presentationml.slideshow.main+xml`, `…presentationml.template.main+xml`;
//! * the macro-enabled main parts of [MS-OFFMACRO2] (`application/vnd.ms-word.document.macroEnabled.main+xml`,
//!   `…ms-word.template.macroEnabledTemplate.main+xml`, `…ms-excel.sheet.macroEnabled.main+xml`,
//!   `…ms-excel.template.macroEnabled.main+xml`, `…ms-powerpoint.presentation|slideshow|template.macroEnabled.main+xml`).
//!   All fourteen strings match the content-type table of the Open XML SDK (MIT,
//!   DocumentFormat.OpenXml 3.1.1); `bo-validate` checks the saved variants with it
//!   (it does not reject an unknown main content type, hence the test of the strings).
//!
//! Converting between variants of one document type only switches the main part's
//! content type override (Part 2 §10.1.2.2.3). Saving to a macro-free variant drops
//! the VBA project ([MS-OFFMACRO2] `vbaProject` relationship and the parts only it
//! reaches, e.g. `vbaData.xml`), Word key map customizations and ribbon
//! customizations that call macros (see [`crate::inspect::package::scrub`]); saving to
//! a macro-enabled variant keeps those parts byte for byte. Macros are never run.

use crate::inspect::package::{scrub, Scrub};
use crate::{EngineError, Result};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

const WML: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.";
const SML: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.";
const PML: &str = "application/vnd.openxmlformats-officedocument.presentationml.";

/// One package variant (file type) of a document, workbook or presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// `.docx` — §11.3.10 document.
    Docx,
    /// `.docm` — [MS-OFFMACRO2] macro-enabled document.
    Docm,
    /// `.dotx` — §11.3.10 template.
    Dotx,
    /// `.dotm` — [MS-OFFMACRO2] macro-enabled template.
    Dotm,
    /// `.xlsx` — §12.3.23 workbook.
    Xlsx,
    /// `.xlsm` — macro-enabled workbook.
    Xlsm,
    /// `.xltx` — §12.3.23 template.
    Xltx,
    /// `.xltm` — macro-enabled template.
    Xltm,
    /// `.pptx` — §13.3.6 presentation.
    Pptx,
    /// `.pptm` — macro-enabled presentation.
    Pptm,
    /// `.potx` — §13.3.6 template.
    Potx,
    /// `.potm` — macro-enabled template.
    Potm,
    /// `.ppsx` — §13.3.6 slide show.
    Ppsx,
    /// `.ppsm` — macro-enabled slide show.
    Ppsm,
}

impl Variant {
    /// Every variant.
    pub const ALL: [Variant; 14] = [
        Variant::Docx,
        Variant::Docm,
        Variant::Dotx,
        Variant::Dotm,
        Variant::Xlsx,
        Variant::Xlsm,
        Variant::Xltx,
        Variant::Xltm,
        Variant::Pptx,
        Variant::Pptm,
        Variant::Potx,
        Variant::Potm,
        Variant::Ppsx,
        Variant::Ppsm,
    ];

    /// File extension (lower case, no dot).
    pub fn ext(self) -> &'static str {
        match self {
            Variant::Docx => "docx",
            Variant::Docm => "docm",
            Variant::Dotx => "dotx",
            Variant::Dotm => "dotm",
            Variant::Xlsx => "xlsx",
            Variant::Xlsm => "xlsm",
            Variant::Xltx => "xltx",
            Variant::Xltm => "xltm",
            Variant::Pptx => "pptx",
            Variant::Pptm => "pptm",
            Variant::Potx => "potx",
            Variant::Potm => "potm",
            Variant::Ppsx => "ppsx",
            Variant::Ppsm => "ppsm",
        }
    }

    /// The variant of a file extension (`"dotx"`, `".DOTX"`, `"a.dotx"`).
    pub fn from_ext(ext: &str) -> Option<Variant> {
        let e = ext.rsplit('.').next().unwrap_or(ext).to_ascii_lowercase();
        Variant::ALL.into_iter().find(|v| v.ext() == e)
    }

    /// Content type of the main part.
    pub fn content_type(self) -> String {
        match self {
            // ECMA-376 Part 1, §11.3.10 Main Document Part.
            Variant::Docx => format!("{WML}document.main+xml"),
            Variant::Dotx => format!("{WML}template.main+xml"),
            // ECMA-376 Part 1, §12.3.23 Workbook Part.
            Variant::Xlsx => format!("{SML}sheet.main+xml"),
            Variant::Xltx => format!("{SML}template.main+xml"),
            // ECMA-376 Part 1, §13.3.6 Presentation Part.
            Variant::Pptx => format!("{PML}presentation.main+xml"),
            Variant::Potx => format!("{PML}template.main+xml"),
            Variant::Ppsx => format!("{PML}slideshow.main+xml"),
            // [MS-OFFMACRO2] macro-enabled main parts.
            Variant::Docm => "application/vnd.ms-word.document.macroEnabled.main+xml".into(),
            Variant::Dotm => {
                "application/vnd.ms-word.template.macroEnabledTemplate.main+xml".into()
            }
            Variant::Xlsm => "application/vnd.ms-excel.sheet.macroEnabled.main+xml".into(),
            Variant::Xltm => "application/vnd.ms-excel.template.macroEnabled.main+xml".into(),
            Variant::Pptm => {
                "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml".into()
            }
            Variant::Potm => "application/vnd.ms-powerpoint.template.macroEnabled.main+xml".into(),
            Variant::Ppsm => "application/vnd.ms-powerpoint.slideshow.macroEnabled.main+xml".into(),
        }
    }

    /// The variant whose main part has content type `ct` (ASCII case-insensitive).
    pub fn from_content_type(ct: &str) -> Option<Variant> {
        let ct = ct.trim();
        Variant::ALL
            .into_iter()
            .find(|v| v.content_type().eq_ignore_ascii_case(ct))
    }

    /// App id: `doc`, `sheet` or `slide`.
    pub fn app(self) -> &'static str {
        match self {
            Variant::Docx | Variant::Docm | Variant::Dotx | Variant::Dotm => "doc",
            Variant::Xlsx | Variant::Xlsm | Variant::Xltx | Variant::Xltm => "sheet",
            _ => "slide",
        }
    }

    /// The variant can carry a VBA project.
    pub fn macros(self) -> bool {
        matches!(
            self,
            Variant::Docm
                | Variant::Dotm
                | Variant::Xlsm
                | Variant::Xltm
                | Variant::Pptm
                | Variant::Potm
                | Variant::Ppsm
        )
    }

    /// The variant is a template.
    pub fn template(self) -> bool {
        matches!(
            self,
            Variant::Dotx
                | Variant::Dotm
                | Variant::Xltx
                | Variant::Xltm
                | Variant::Potx
                | Variant::Potm
        )
    }

    /// The variant opens as a slide show.
    pub fn show(self) -> bool {
        matches!(self, Variant::Ppsx | Variant::Ppsm)
    }

    /// The same kind of file with (`true`) or without macros.
    #[must_use]
    pub fn with_macros(self, on: bool) -> Variant {
        use Variant as V;
        let pairs = [
            (V::Docx, V::Docm),
            (V::Dotx, V::Dotm),
            (V::Xlsx, V::Xlsm),
            (V::Xltx, V::Xltm),
            (V::Pptx, V::Pptm),
            (V::Potx, V::Potm),
            (V::Ppsx, V::Ppsm),
        ];
        for (free, enabled) in pairs {
            if self == free || self == enabled {
                return if on { enabled } else { free };
            }
        }
        self
    }

    /// The editable document a template (or slide show) creates: `.dotx` → `.docx`,
    /// `.dotm` → `.docm`, `.ppsx` → `.pptx`, … (documents map to themselves).
    /// A document made from a macro-enabled template keeps its macros (BlueOffice's
    /// choice: nothing is lost; saving it macro-free later warns).
    #[must_use]
    pub fn document(self) -> Variant {
        let base = match self.app() {
            "doc" => Variant::Docx,
            "sheet" => Variant::Xlsx,
            _ => Variant::Pptx,
        };
        base.with_macros(self.macros())
    }

    /// Variants of the same document type (`doc`, `sheet`, `slide`).
    pub fn family(app: &str) -> Vec<Variant> {
        Variant::ALL
            .into_iter()
            .filter(|v| v.app() == app)
            .collect()
    }
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(opc)`
fn opc(e: bo_opc::OpcError) -> EngineError {
    EngineError::Format(e.to_string())
}

/// The main part of a package: name and content type.
fn main_part(pkg: &Package) -> Result<Option<(String, String)>> {
    let Some(main) = pkg.related_part("/", REL_OFFICE_DOCUMENT).map_err(opc)? else {
        return Ok(None);
    };
    let ct = pkg.content_types().map_err(opc)?;
    Ok(ct.get(&main).map(|t| (main.clone(), t.to_owned())))
}

/// The variant of an OOXML package (by its main part's content type), `None` for
/// anything else (other containers, encrypted packages, unknown main parts).
pub fn detect(bytes: &[u8]) -> Option<Variant> {
    if !bytes.starts_with(b"PK") {
        return None;
    }
    let pkg = Package::read(bytes, &Limits::default()).ok()?;
    let (_, ct) = main_part(&pkg).ok()??;
    Variant::from_content_type(&ct)
}

/// The package holds a VBA project ([MS-OFFMACRO2] `vbaProject` relationship target).
fn has_vba(pkg: &Package) -> bool {
    bo_binary::vba::package_has_macros(pkg)
}

/// Result of [`convert`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    /// The package in the requested variant.
    pub bytes: Vec<u8>,
    /// Macros were present and have been removed (target is macro-free).
    pub macros_dropped: bool,
}

/// Re-type a saved OOXML package as `target`: switch the main part's content type,
/// and drop the macro parts when `target` is macro-free. Refuses packages of another
/// document type (a workbook cannot become a `.docx`).
pub fn convert(bytes: &[u8], target: Variant) -> Result<Converted> {
    let pkg = Package::read(bytes, &Limits::default()).map_err(opc)?;
    let (main, ct) = main_part(&pkg)?
        .ok_or_else(|| EngineError::Format("no officeDocument relationship".into()))?;
    let from = Variant::from_content_type(&ct)
        .ok_or_else(|| EngineError::Format(format!("unknown main part content type {ct}")))?;
    if from.app() != target.app() {
        return Err(EngineError::Unsupported(format!(
            "{} → {}",
            from.ext(),
            target.ext()
        )));
    }
    let macros_dropped = !target.macros() && has_vba(&pkg);
    let bytes = if macros_dropped {
        scrub(
            bytes,
            &Scrub {
                macros: true,
                ..Scrub::default()
            },
        )?
    } else {
        bytes.to_vec()
    };
    let mut pkg = Package::read(&bytes, &Limits::default()).map_err(opc)?;
    let mut types = pkg.content_types().map_err(opc)?;
    // ECMA-376 Part 2, §10.1.2.2.3 Override: the main part's content type.
    types.set_override(&main, &target.content_type());
    pkg.set_content_types(&types);
    Ok(Converted {
        bytes: pkg.write().map_err(opc)?,
        macros_dropped,
    })
}

/// The package a template creates: the same content re-typed as the matching
/// document ([`Variant::document`]); other packages unchanged.
pub fn new_from_template(bytes: &[u8]) -> Result<Vec<u8>> {
    match detect(bytes) {
        Some(v) if v.template() => Ok(convert(bytes, v.document())?.bytes),
        _ => Ok(bytes.to_vec()),
    }
}

// ---------------------------------------------------------------------------
// Theme files (.thmx)
// ---------------------------------------------------------------------------

/// ECMA-376 Part 1, §14.2.7 Theme part content type.
pub const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
/// Content type of the theme manager part of a theme file (the main part of
/// `.thmx` packages, black-box observation of theme files).
pub const CT_THEME_MANAGER: &str = "application/vnd.openxmlformats-officedocument.themeManager+xml";
/// ECMA-376 Part 1, §14.2.7 theme relationship.
pub const REL_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
/// DrawingML main namespace (ECMA-376 Part 1, §20.1).
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

/// The theme part (`a:theme`, §20.1.6.9) of a theme file. The main part is either
/// the theme part itself or a theme manager (`a:themeManager`, §20.1.6.11) that
/// points at it through a theme relationship. A bare theme part is accepted too.
pub fn theme_of_thmx(bytes: &[u8]) -> Option<Vec<u8>> {
    let is_theme = |d: &[u8]| {
        let head = String::from_utf8_lossy(&d[..d.len().min(4096)]).into_owned();
        root_local_name(&head).as_deref() == Some("theme")
    };
    if bytes.first() == Some(&b'<') || bytes.starts_with(b"\xEF\xBB\xBF") {
        return is_theme(bytes).then(|| bytes.to_vec());
    }
    let pkg = Package::read(bytes, &Limits::default()).ok()?;
    let main = pkg.related_part("/", REL_OFFICE_DOCUMENT).ok()??;
    let data = &pkg.part(&main)?.data;
    if is_theme(data) {
        return Some(data.clone());
    }
    let theme = pkg.related_part(&main, REL_THEME).ok()??;
    let data = &pkg.part(&theme)?.data;
    is_theme(data).then(|| data.clone())
}

/// Local name of the root element of an XML text.
fn root_local_name(xml: &str) -> Option<String> {
    let mut rest = xml;
    loop {
        let i = rest.find('<')?;
        rest = &rest[i + 1..];
        if rest.starts_with('?') || rest.starts_with('!') {
            continue;
        }
        let name: String = rest
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/')
            .collect();
        return Some(name.rsplit(':').next().unwrap_or(&name).to_owned());
    }
}

/// A `.thmx` package holding `theme_xml` (an `a:theme` part, §20.1.6.9): a theme
/// manager main part (`a:themeManager`, §20.1.6.11) with a theme relationship
/// (§14.2.7) to `theme1.xml`.
pub fn thmx_package(theme_xml: &[u8]) -> Result<Vec<u8>> {
    const DIR: &str = "/theme/theme";
    let manager = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>{}<a:themeManager xmlns:a="{NS_A}"/>"#,
        "\r\n"
    );
    let mut pkg = Package::new();
    // ECMA-376 Part 2, §10.1.2: `rels` / `xml` defaults come with `Package::new`.
    let mut types = pkg.content_types().map_err(opc)?;
    types.set_override(&format!("{DIR}/themeManager.xml"), CT_THEME_MANAGER);
    types.set_override(&format!("{DIR}/theme1.xml"), CT_THEME);
    pkg.set_content_types(&types);
    pkg.put_part(&format!("{DIR}/themeManager.xml"), manager.into_bytes());
    pkg.put_part(&format!("{DIR}/theme1.xml"), theme_xml.to_vec());
    pkg.set_relationships(
        "/",
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: "theme/theme/themeManager.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    pkg.set_relationships(
        &format!("{DIR}/themeManager.xml"),
        &[Relationship {
            id: "rId1".into(),
            rel_type: REL_THEME.into(),
            target: "theme1.xml".into(),
            mode: TargetMode::Internal,
        }],
    );
    pkg.write().map_err(opc)
}

/// The theme part of a saved document, workbook or presentation (the theme of the
/// main part — for a presentation, of its first slide master).
pub fn theme_of_package(bytes: &[u8]) -> Option<Vec<u8>> {
    let pkg = Package::read(bytes, &Limits::default()).ok()?;
    let main = pkg.related_part("/", REL_OFFICE_DOCUMENT).ok()??;
    // WordprocessingML (§11.3.10) and SpreadsheetML (§12.3.23): theme of the main part.
    if let Some(t) = pkg.related_part(&main, REL_THEME).ok()? {
        return pkg.part(&t).map(|p| p.data.clone());
    }
    // PresentationML: the first slide master's theme (§13.3.6 → §13.3.10 → §14.2.7).
    let master = pkg
        .related_part(
            &main,
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster",
        )
        .ok()??;
    let t = pkg.related_part(&master, REL_THEME).ok()??;
    pkg.part(&t).map(|p| p.data.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_and_types_are_distinct() {
        let mut exts: Vec<_> = Variant::ALL.iter().map(|v| v.ext()).collect();
        exts.dedup();
        assert_eq!(exts.len(), 14);
        for v in Variant::ALL {
            assert_eq!(Variant::from_ext(v.ext()), Some(v));
            assert_eq!(Variant::from_content_type(&v.content_type()), Some(v));
            assert!(v.content_type().ends_with(".main+xml"));
            assert_eq!(v.with_macros(v.macros()), v);
            assert_eq!(v.document().app(), v.app());
            assert!(!v.document().template());
        }
        assert_eq!(Variant::from_ext("Report.DOTM"), Some(Variant::Dotm));
        assert_eq!(Variant::Dotm.document(), Variant::Docm);
        assert_eq!(Variant::Ppsx.document(), Variant::Pptx);
        assert_eq!(Variant::Xltx.document(), Variant::Xlsx);
        assert_eq!(Variant::Ppsm.with_macros(false), Variant::Ppsx);
        assert_eq!(Variant::family("slide").len(), 6);
    }

    #[test]
    fn root_names() {
        assert_eq!(
            root_local_name(r#"<?xml version="1.0"?><!-- c --><a:theme xmlns:a="x">"#).as_deref(),
            Some("theme")
        );
        assert_eq!(
            root_local_name("<themeManager/>").as_deref(),
            Some("themeManager")
        );
    }

    #[test]
    fn thmx_round_trip() {
        let theme = format!(r#"<a:theme xmlns:a="{NS_A}" name="T"><a:themeElements/></a:theme>"#);
        let pkg = thmx_package(theme.as_bytes()).unwrap();
        assert_eq!(theme_of_thmx(&pkg).unwrap(), theme.as_bytes());
        assert_eq!(theme_of_thmx(theme.as_bytes()).unwrap(), theme.as_bytes());
        assert!(theme_of_thmx(b"<w:document/>").is_none());
        // Not a document variant.
        assert_eq!(detect(&pkg), None);
    }
}
