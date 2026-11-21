//! BlueEditor round 12: the editable document properties of File › Properties — the document
//! information dictionary (ISO 32000-1 §14.3.3, Table 317), the initial view (§7.7.2 Table 28
//! `/PageMode`, `/PageLayout`, `/OpenAction` with an explicit destination §12.3.2.2, §12.2
//! Table 150 viewer preferences `/DisplayDocTitle`, `/CenterWindow`, `/FitWindow`,
//! `/HideMenubar`, `/HideToolbar`, `/HideWindowUI`, `/NonFullScreenPageMode`) and the print and
//! reading presets (`/PrintScaling`, `/Duplex`, `/NumCopies`, `/PrintPageRange`,
//! `/PickTrayByPDFSize`, `/Direction`; the catalog's `/Lang` §14.9.2).
use crate::annot::text_obj;
use crate::{resolve, text_string, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId};

/// Navigation tabs shown on opening (`/PageMode`), in the reference's order.
pub const PAGE_MODES: [&str; 5] = [
    "UseNone",
    "UseOutlines",
    "UseThumbs",
    "UseAttachments",
    "UseOC",
];

/// Page layouts (`/PageLayout`; index 0 = not set), in the reference's order.
pub const PAGE_LAYOUTS: [&str; 7] = [
    "",
    "SinglePage",
    "OneColumn",
    "TwoPageLeft",
    "TwoColumnLeft",
    "TwoPageRight",
    "TwoColumnRight",
];

/// Magnifications offered: 0 Default (no zoom in the destination), 1 Actual Size, 2 Fit Page,
/// 3 Fit Width, 4 Fit Height, 5 Fit Visible, then the percentages of [`ZOOMS`].
pub const ZOOMS: [u32; 10] = [25, 50, 75, 100, 125, 150, 200, 400, 800, 1600];

/// The initial view of the document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitialView {
    /// Index in [`PAGE_MODES`].
    pub navigation: usize,
    /// Index in [`PAGE_LAYOUTS`].
    pub layout: usize,
    /// 0..=5 named fits, 6.. the percentages of [`ZOOMS`].
    pub magnification: usize,
    /// Page shown first (0-based).
    pub page: usize,
    /// Viewer preferences (Table 150): show the title, not the file name.
    pub display_title: bool,
    /// Open in full-screen mode.
    pub full_screen: bool,
    /// Centre the window on the screen.
    pub center_window: bool,
    /// Fit the window to the first page.
    pub resize_window: bool,
    /// Hide the menu bar.
    pub hide_menubar: bool,
    /// Hide the tool bars.
    pub hide_toolbars: bool,
    /// Hide the window controls.
    pub hide_window_controls: bool,
}

/// Print and reading presets of the Advanced page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrintPresets {
    /// 0 Default (`AppDefault`), 1 None.
    pub scaling: usize,
    /// 0 Simplex, 1 Duplex Flip Short Edge, 2 Duplex Flip Long Edge.
    pub duplex: usize,
    /// 0 Default, else the number of copies.
    pub copies: u32,
    /// Page ranges (1-based, inclusive) as typed, `1,5-9`.
    pub pages: String,
    /// Choose the paper source by PDF page size.
    pub tray_by_size: bool,
    /// 0 Left Edge (`L2R`), 1 Right Edge (`R2L`).
    pub binding: usize,
    /// Language tag (`/Lang`).
    pub language: String,
}

const DUPLEX: [&str; 3] = ["Simplex", "DuplexFlipShortEdge", "DuplexFlipLongEdge"];

fn name(o: Option<&Object>) -> String {
    match o {
        Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
        _ => String::new(),
    }
}

impl PdfDocument {
    fn catalog_id12(&self) -> Result<ObjectId, String> {
        self.doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|e| e.to_string())
    }

    fn catalog12(&self) -> Option<&Dictionary> {
        let id = self.catalog_id12().ok()?;
        self.doc.get_dictionary(id).ok()
    }

    /// The catalog's viewer preferences (resolved).
    fn viewer_prefs12(&self) -> Dictionary {
        self.catalog12()
            .and_then(|c| c.get(b"ViewerPreferences").ok())
            .map(|o| resolve(&self.doc, o))
            .and_then(|o| o.as_dict().ok().cloned())
            .unwrap_or_default()
    }

    /// The document's initial view.
    #[must_use]
    pub fn initial_view(&self) -> InitialView {
        let Some(cat) = self.catalog12() else {
            return InitialView::default();
        };
        let get = |k: &[u8]| cat.get(k).ok().map(|o| resolve(&self.doc, o));
        let vp = self.viewer_prefs12();
        let flag = |k: &[u8]| {
            vp.get(k)
                .ok()
                .and_then(|o| o.as_bool().ok())
                .unwrap_or(false)
        };
        let mut mode = name(get(b"PageMode"));
        let full_screen = mode == "FullScreen";
        if full_screen {
            mode = name(vp.get(b"NonFullScreenPageMode").ok());
        }
        let layout = name(get(b"PageLayout"));
        let (page, fit) = self.open_destination().unwrap_or((0, crate::OpenFit::Keep));
        let magnification = match fit {
            crate::OpenFit::Page => 2,
            crate::OpenFit::Width => 3,
            crate::OpenFit::Zoom(z) => {
                let pct = (z * 100.0).round() as u32;
                ZOOMS
                    .iter()
                    .position(|&p| p == pct)
                    .map_or(usize::from(pct == 100), |i| 6 + i)
            }
            crate::OpenFit::Keep => {
                // /FitV and /FitBH are not told apart by `open_destination`.
                let kind = self.open_fit_name12();
                match kind.as_str() {
                    "FitV" | "FitBV" => 4,
                    "FitB" | "FitBH" => 5,
                    _ => 0,
                }
            }
        };
        InitialView {
            navigation: PAGE_MODES.iter().position(|m| *m == mode).unwrap_or(0),
            layout: PAGE_LAYOUTS
                .iter()
                .position(|l| !l.is_empty() && *l == layout)
                .unwrap_or(0),
            magnification,
            page,
            display_title: flag(b"DisplayDocTitle"),
            full_screen,
            center_window: flag(b"CenterWindow"),
            resize_window: flag(b"FitWindow"),
            hide_menubar: flag(b"HideMenubar"),
            hide_toolbars: flag(b"HideToolbar"),
            hide_window_controls: flag(b"HideWindowUI"),
        }
    }

    /// Name of the `/OpenAction` destination's fit (`XYZ`, `Fit`, `FitV`…), empty if none.
    fn open_fit_name12(&self) -> String {
        let Some(cat) = self.catalog12() else {
            return String::new();
        };
        let Some(oa) = cat.get(b"OpenAction").ok().map(|o| resolve(&self.doc, o)) else {
            return String::new();
        };
        let arr = match oa {
            Object::Array(a) => Some(a.clone()),
            Object::Dictionary(d) => d
                .get(b"D")
                .ok()
                .map(|o| resolve(&self.doc, o))
                .and_then(|o| o.as_array().ok().cloned()),
            _ => None,
        };
        name(arr.as_ref().and_then(|a| a.get(1)))
    }

    /// Write the initial view into the catalog (one undo step).
    ///
    /// # Errors
    /// When the catalog is missing.
    pub fn set_initial_view(&mut self, v: &InitialView) -> Result<(), String> {
        let id = self.catalog_id12()?;
        let page_id = self
            .pages
            .get(v.page.min(self.pages.len().saturating_sub(1)))
            .ok_or("no pages")?
            .id;
        let mut vp = self.viewer_prefs12();
        let vp_ref = self
            .catalog12()
            .and_then(|c| c.get(b"ViewerPreferences").ok())
            .and_then(|o| o.as_reference().ok());
        for (k, on) in [
            (b"DisplayDocTitle".as_slice(), v.display_title),
            (b"CenterWindow".as_slice(), v.center_window),
            (b"FitWindow".as_slice(), v.resize_window),
            (b"HideMenubar".as_slice(), v.hide_menubar),
            (b"HideToolbar".as_slice(), v.hide_toolbars),
            (b"HideWindowUI".as_slice(), v.hide_window_controls),
        ] {
            if on {
                vp.set(k.to_vec(), true);
            } else {
                vp.remove(k);
            }
        }
        let mode = PAGE_MODES.get(v.navigation).copied().unwrap_or("UseNone");
        if v.full_screen && mode != "UseNone" {
            vp.set(
                "NonFullScreenPageMode",
                Object::Name(mode.as_bytes().to_vec()),
            );
        } else {
            vp.remove(b"NonFullScreenPageMode");
        }
        // The destination (§12.3.2.2 Table 151).
        let p = Object::Reference(page_id);
        let null = Object::Null;
        let dest: Vec<Object> = match v.magnification {
            1 => vec![p, "XYZ".into(), null.clone(), null, Object::Real(1.0)],
            2 => vec![p, "Fit".into()],
            3 => vec![p, "FitH".into(), Object::Null],
            4 => vec![p, "FitV".into(), Object::Null],
            5 => vec![p, "FitBH".into(), Object::Null],
            k if k >= 6 => {
                let z = ZOOMS.get(k - 6).copied().unwrap_or(100);
                #[allow(clippy::cast_precision_loss)]
                let z = z as f32 / 100.0;
                vec![p, "XYZ".into(), null.clone(), null, Object::Real(z)]
            }
            _ => vec![p, "XYZ".into(), null.clone(), null.clone(), null],
        };
        self.begin("initial view");
        if let Some(r) = vp_ref {
            self.touch(r);
            if let Ok(slot) = self.doc.get_object_mut(r) {
                *slot = Object::Dictionary(vp.clone());
            }
        }
        self.touch(id);
        let c = self.doc.get_dictionary_mut(id).map_err(|e| e.to_string())?;
        if v.full_screen {
            c.set("PageMode", Object::Name(b"FullScreen".to_vec()));
        } else if v.navigation == 0 {
            c.remove(b"PageMode");
        } else {
            c.set("PageMode", Object::Name(mode.as_bytes().to_vec()));
        }
        match PAGE_LAYOUTS.get(v.layout).copied().unwrap_or("") {
            "" => {
                c.remove(b"PageLayout");
            }
            l => c.set("PageLayout", Object::Name(l.as_bytes().to_vec())),
        }
        if v.magnification == 0 && v.page == 0 {
            c.remove(b"OpenAction");
        } else {
            c.set("OpenAction", dest);
        }
        if vp_ref.is_none() {
            if vp.is_empty() {
                c.remove(b"ViewerPreferences");
            } else {
                c.set("ViewerPreferences", vp);
            }
        }
        self.commit();
        Ok(())
    }

    /// Set (or, with an empty value, remove) an entry of the document information dictionary
    /// (`Title`, `Author`, `Subject`, `Keywords`, `Creator`, `Producer` or a custom key); one
    /// undo step.
    ///
    /// # Errors
    /// When the trailer has no catalog.
    pub fn set_info(&mut self, key: &str, value: &str) -> Result<(), String> {
        if key.is_empty() || key.contains(['/', ' ', '(', ')', '<', '>', '[', ']', '%']) {
            return Err("bad property name".into());
        }
        self.begin("document properties");
        let info = self.doc.trailer.get(b"Info").ok().cloned();
        let id = match info {
            Some(Object::Reference(r)) => r,
            other => {
                let d = match other {
                    Some(Object::Dictionary(d)) => d,
                    _ => Dictionary::new(),
                };
                let r = self.add(Object::Dictionary(d));
                self.doc.trailer.set("Info", Object::Reference(r));
                r
            }
        };
        self.touch(id);
        if let Ok(d) = self.doc.get_dictionary_mut(id) {
            if value.trim().is_empty() {
                d.remove(key.as_bytes());
            } else {
                d.set(key.as_bytes().to_vec(), text_obj(value));
            }
        }
        self.commit();
        Ok(())
    }

    /// Entries of the information dictionary other than the standard ones (Custom).
    #[must_use]
    pub fn custom_info(&self) -> Vec<(String, String)> {
        const STANDARD: [&[u8]; 9] = [
            b"Title",
            b"Author",
            b"Subject",
            b"Keywords",
            b"Creator",
            b"Producer",
            b"CreationDate",
            b"ModDate",
            b"Trapped",
        ];
        let doc = &self.doc;
        let Some(d) = doc
            .trailer_ref()
            .get(b"Info")
            .ok()
            .and_then(|o| resolve(doc, o).as_dict().ok())
        else {
            return Vec::new();
        };
        d.iter()
            .filter(|(k, _)| !STANDARD.contains(&k.as_slice()))
            .filter_map(|(k, v)| {
                let s = resolve(doc, v).as_str().ok()?;
                Some((String::from_utf8_lossy(k).into_owned(), text_string(s)))
            })
            .collect()
    }

    /// The print and reading presets.
    #[must_use]
    pub fn print_presets(&self) -> PrintPresets {
        let vp = self.viewer_prefs12();
        let ranges: Vec<i64> = vp
            .get(b"PrintPageRange")
            .ok()
            .map(|o| resolve(&self.doc, o))
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_i64().ok()).collect())
            .unwrap_or_default();
        let pages = ranges
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| {
                if c[0] == c[1] {
                    format!("{}", c[0] + 1)
                } else {
                    format!("{}-{}", c[0] + 1, c[1] + 1)
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let lang = self
            .catalog12()
            .and_then(|c| c.get(b"Lang").ok())
            .map(|o| resolve(&self.doc, o))
            .and_then(|o| o.as_str().ok())
            .map(text_string)
            .unwrap_or_default();
        PrintPresets {
            scaling: usize::from(name(vp.get(b"PrintScaling").ok()) == "None"),
            duplex: DUPLEX
                .iter()
                .position(|d| *d == name(vp.get(b"Duplex").ok()))
                .unwrap_or(0),
            copies: vp
                .get(b"NumCopies")
                .ok()
                .and_then(|o| o.as_i64().ok())
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0),
            pages,
            tray_by_size: vp
                .get(b"PickTrayByPDFSize")
                .ok()
                .and_then(|o| o.as_bool().ok())
                .unwrap_or(false),
            binding: usize::from(name(vp.get(b"Direction").ok()) == "R2L"),
            language: lang,
        }
    }

    /// Write the print and reading presets (one undo step).
    ///
    /// # Errors
    /// When the catalog is missing.
    pub fn set_print_presets(&mut self, p: &PrintPresets) -> Result<(), String> {
        let id = self.catalog_id12()?;
        let mut vp = self.viewer_prefs12();
        let vp_ref = self
            .catalog12()
            .and_then(|c| c.get(b"ViewerPreferences").ok())
            .and_then(|o| o.as_reference().ok());
        let set_name = |vp: &mut Dictionary, k: &str, v: Option<&str>| match v {
            Some(v) => vp.set(k, Object::Name(v.as_bytes().to_vec())),
            None => {
                vp.remove(k.as_bytes());
            }
        };
        set_name(&mut vp, "PrintScaling", (p.scaling == 1).then_some("None"));
        set_name(
            &mut vp,
            "Duplex",
            (p.duplex > 0).then(|| DUPLEX[p.duplex.min(2)]),
        );
        set_name(&mut vp, "Direction", (p.binding == 1).then_some("R2L"));
        if p.copies > 0 {
            vp.set("NumCopies", i64::from(p.copies.min(5)));
        } else {
            vp.remove(b"NumCopies");
        }
        if p.tray_by_size {
            vp.set("PickTrayByPDFSize", true);
        } else {
            vp.remove(b"PickTrayByPDFSize");
        }
        let n = self.pages.len();
        let mut ranges: Vec<Object> = Vec::new();
        for part in p.pages.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (a, b) = part.split_once('-').unwrap_or((part, part));
            let (Ok(a), Ok(b)) = (a.trim().parse::<usize>(), b.trim().parse::<usize>()) else {
                return Err("bad page range".into());
            };
            if a == 0 || b < a || b > n {
                return Err("bad page range".into());
            }
            ranges.push(Object::Integer(a as i64 - 1));
            ranges.push(Object::Integer(b as i64 - 1));
        }
        if ranges.is_empty() {
            vp.remove(b"PrintPageRange");
        } else {
            vp.set("PrintPageRange", ranges);
        }
        self.begin("print presets");
        if let Some(r) = vp_ref {
            self.touch(r);
            if let Ok(slot) = self.doc.get_object_mut(r) {
                *slot = Object::Dictionary(vp.clone());
            }
        }
        self.touch(id);
        let c = self.doc.get_dictionary_mut(id).map_err(|e| e.to_string())?;
        if vp_ref.is_none() {
            if vp.is_empty() {
                c.remove(b"ViewerPreferences");
            } else {
                c.set("ViewerPreferences", vp);
            }
        }
        if p.language.trim().is_empty() {
            c.remove(b"Lang");
        } else {
            c.set("Lang", text_obj(p.language.trim()));
        }
        self.commit();
        Ok(())
    }
}

#[cfg(test)]
#[path = "docprops12_tests.rs"]
mod tests;
