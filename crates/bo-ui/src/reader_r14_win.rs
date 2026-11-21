//! BlueReader round 14 (a child of `reader_window`):
//!
//! - View tab ribbon: buttons placed like the reference's (measured at 125 %: labels were 2–17
//!   px left of the reference's from Select to View Setting; now within 1.5 px).
//! - Icons in the Select and Assistant menus (own shapes in `reader_icons`).
use super::ReaderWindow;
use crate::popup::MenuItem;

/// The View tab (ribbon index).
const VIEW_TAB: usize = 2;

impl ReaderWindow {
    /// Extra width of a large button (logical px), measured on the reference's View tab.
    pub(crate) fn widen14(&self, id: &str) -> f32 {
        if self.profile.is_some() || self.ribbon_tab != VIEW_TAB {
            return 0.0;
        }
        match id {
            // 4 / 4 / 5 / 5 device px at 125 %.
            "tool.select" | "view.reflow" => 3.2,
            "view.wordCount" | "menu.viewSetting" => 4.0,
            _ => 0.0,
        }
    }

    /// Space added before a ribbon item (logical px), measured on the reference's View tab:
    /// the separator after Select is 2 px wider, the page display grid 2.5 px wider, the
    /// separator before Assistant 3.5 px narrower (device px at 125 %).
    pub(crate) fn nudge14(&self, id: &str) -> f32 {
        if self.profile.is_some() || self.ribbon_tab != VIEW_TAB {
            return 0.0;
        }
        match id {
            "view.readMode" => 1.6,
            "view.autoScroll" => 2.0,
            "menu.assistant" => -2.8,
            _ => 0.0,
        }
    }

    /// A drop-down menu under `r` with an icon on each row, like the reference's Select and
    /// Assistant menus.
    pub(crate) fn menu_icons14(
        &mut self,
        items: Vec<MenuItem>,
        r: [f32; 4],
        id: &'static str,
        icons: &[&'static str],
    ) {
        let icons = icons.iter().map(|i| Some(*i)).collect();
        // Measured 1:1 at 125 %: the reference's icon ink is 20 px from 10 px after the frame,
        // rows 30 px, label at 41 px. Our shapes leave a 2.5 / 24 margin in their box, so the
        // box is 24 px from 8 px (BlueEditor keeps the shared look).
        let menu = if self.profile.is_none() {
            let look = crate::popup::MenuLook {
                icon: 19.2,
                icon_x: 6.4,
                ..super::READER_MENU
            };
            crate::popup::Menu::open_with(items, r, self.size, self.scale, look)
        } else {
            self.reader_menu(items, r)
        };
        let menu = menu
            .with_decor(icons, Vec::new())
            .with_icon_fn(crate::reader_icons::menu_icon);
        self.menu = Some((menu, id));
    }

    /// Commands the reference greys for the open document (measured on its View tab): with one
    /// page, Reverse View and the continuous / two-page layouts (Single Page, the cover page
    /// layout and Split stay on).
    pub(crate) fn fourteen_disabled(&self, id: &str) -> bool {
        if self.profile.is_some() {
            return false;
        }
        let one_page = self.view().is_some_and(|v| v.page_count() <= 1);
        one_page
            && matches!(
                id,
                "view.reverse" | "view.continuous" | "view.facing" | "view.contFacing"
            )
    }

    /// Round 14 elements for assistive technology: the active document tab's close button
    /// (`doc:close`).
    pub(crate) fn access14(&self) -> Vec<super::AccessItem> {
        if self.tabs.is_empty() || self.show_start {
            return Vec::new();
        }
        // Its box from the last paint (none before the first frame).
        let close = self
            .doc_tab_rects
            .get(self.active)
            .map_or([0.0; 4], |(_, c)| *c);
        vec![super::AccessItem {
            key: "doc:close".into(),
            name: self.t("pdfReader.close"),
            role: super::AccessRole::Button,
            rect: close,
            enabled: true,
            selected: false,
        }]
    }

    /// Invoke a round 14 element; `true` when it was one.
    pub(crate) fn invoke14(&mut self, key: &str, m: &dyn crate::TextMeasure) -> bool {
        if key == "doc:close" && !self.tabs.is_empty() {
            self.command("file.close", m);
            return true;
        }
        false
    }

    /// The subject of comment `a` shown in its pop-up note's header.
    pub(crate) fn subject14(&self, a: &bo_pdfread::Annot) -> Option<String> {
        self.view()?.doc().annot_subject(a)
    }

    /// Whether command `id` is offered now (tests).
    #[cfg(test)]
    pub(crate) fn is_enabled(&self, id: &str) -> bool {
        self.enabled(id)
    }

    /// Icons of the menu open now (tests).
    #[cfg(test)]
    pub(crate) fn menu_has_icons14(&self) -> bool {
        self.menu.as_ref().is_some_and(|(m, _)| m.has_icons())
    }
}

#[cfg(test)]
#[path = "reader_r14_tests.rs"]
mod tests;
