//! Theme files (`.thmx`, CORE-FMT-001) for BlueSheet: apply the theme of a theme
//! file (or of another document, workbook or presentation) to the workbook, and
//! save the workbook's theme as a theme file.
//!
//! The workbook's theme part (ECMA-376 Part 1, §14.2.7, reached from the workbook
//! part §12.3.23 through a theme relationship) is kept as an opaque part; applying a
//! theme replaces its bytes (or adds the part) and the colour scheme the cells
//! resolve theme colours with (§20.1.6.2 clrScheme). One undo step.

use super::{SheetEngine, UndoEntry};
use crate::variant::{thmx_package, CT_THEME, REL_THEME};
use crate::{EngineError, Result};
use bo_model::sheet::{OpaquePart, OpaqueRel};

/// Name of the theme part added to a workbook that has none.
const THEME_PART: &str = "/xl/theme/theme1.xml";

impl SheetEngine {
    /// Index of the workbook's theme part among the opaque parts.
    fn theme_part(&self) -> Result<Option<usize>> {
        let wb = self.wb()?;
        let Some(rel) = wb
            .opaque
            .workbook_rels
            .iter()
            .find(|r| r.rel_type == REL_THEME && !r.external)
        else {
            return Ok(None);
        };
        Ok(wb
            .opaque
            .parts
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&rel.target)))
    }

    /// The workbook's theme as a `.thmx` package; [`EngineError::Unsupported`]
    /// (`noTheme`) when the workbook has no theme part.
    pub fn theme_file(&self) -> Result<Vec<u8>> {
        let i = self
            .theme_part()?
            .ok_or_else(|| EngineError::Unsupported("noTheme".into()))?;
        thmx_package(&self.wb()?.opaque.parts[i].data)
    }

    /// Apply the theme of `bytes` (a `.thmx`, or a `.docx` / `.xlsx` / `.pptx` of any
    /// variant). Returns `false` when the file holds no usable theme.
    pub fn apply_theme_file(&mut self, bytes: &[u8]) -> Result<bool> {
        let Some(xml) = crate::theme_file::theme_xml_of(bytes) else {
            return Ok(false);
        };
        let Some(colors) = bo_xlsx::theme_colors(&xml) else {
            return Ok(false);
        };
        let at = self.theme_part()?;
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let before = Box::new(wb.clone());
        if let Some(i) = at {
            wb.opaque.parts[i].data = xml;
        } else {
            let mut name = THEME_PART.to_owned();
            let mut n = 1;
            while wb
                .opaque
                .parts
                .iter()
                .any(|p| p.name.eq_ignore_ascii_case(&name))
            {
                n += 1;
                name = format!("/xl/theme/theme{n}.xml");
            }
            let mut k = wb.opaque.workbook_rels.len() + 1;
            while wb
                .opaque
                .workbook_rels
                .iter()
                .any(|r| r.id == format!("rIdTheme{k}"))
            {
                k += 1;
            }
            wb.opaque.parts.push(OpaquePart {
                name: name.clone(),
                content_type: Some(CT_THEME.into()),
                data: xml,
            });
            // ECMA-376 Part 1, §14.2.7: workbook → theme relationship.
            wb.opaque.workbook_rels.push(OpaqueRel {
                id: format!("rIdTheme{k}"),
                rel_type: REL_THEME.into(),
                target: name,
                external: false,
            });
        }
        wb.theme = Some(colors);
        self.live.rebuild(wb);
        self.invalidate(None);
        self.push_undo(UndoEntry::Workbook(before));
        Ok(true)
    }
}
