//! The document's opening view (ISO 32000-1 §7.7.2 `/OpenAction`, §12.3.2.2 destinations):
//! the page and zoom the reference reader shows first (BlueReader round 13).
use crate::PdfDocument;
use lopdf::Object;

/// How the first view is zoomed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpenFit {
    /// `/Fit`, `/FitB`: the whole page.
    Page,
    /// `/FitH`, `/FitBH`: the page width.
    Width,
    /// `/XYZ` with a zoom (1 = 100 %).
    Zoom(f32),
    /// `/XYZ` without a zoom, `/FitV`, `/FitR`: the reader's own zoom.
    Keep,
}

impl PdfDocument {
    /// Page index and zoom of the `/OpenAction` destination (an explicit destination array,
    /// or a GoTo action holding one); `None` without one.
    #[must_use]
    pub fn open_destination(&self) -> Option<(usize, OpenFit)> {
        let doc = &self.doc;
        let r = |o: &Object| doc.dereference(o).map_or(o.clone(), |(_, v)| v.clone());
        let root = doc.trailer_ref().get(b"Root").ok()?;
        let cat = r(root);
        let cat = cat.as_dict().ok()?;
        let mut act = r(cat.get(b"OpenAction").ok()?);
        if let Ok(d) = act.as_dict() {
            if d.get(b"S").ok().and_then(|s| s.as_name().ok()) != Some(b"GoTo".as_slice()) {
                return None;
            }
            act = r(d.get(b"D").ok()?);
        }
        let a = act.as_array().ok()?;
        let page = match a.first()? {
            Object::Reference(id) => self.pages.iter().position(|p| p.id == *id)?,
            // Remote-style page number (used by some writers in local destinations too).
            Object::Integer(n) => usize::try_from(*n)
                .ok()?
                .min(self.pages.len().checked_sub(1)?),
            _ => return None,
        };
        let kind = a.get(1).and_then(|o| o.as_name().ok()).unwrap_or(b"Fit");
        let fit = match kind {
            b"Fit" | b"FitB" => OpenFit::Page,
            b"FitH" | b"FitBH" => OpenFit::Width,
            b"XYZ" => match a.get(4).map(r).and_then(|z| z.as_float().ok()) {
                Some(z) if z > 0.0 && z.is_finite() => OpenFit::Zoom(z),
                _ => OpenFit::Keep,
            },
            _ => OpenFit::Keep,
        };
        Some((page, fit))
    }
}
