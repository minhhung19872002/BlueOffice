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

    /// Icons of the menu open now (tests).
    #[cfg(test)]
    pub(crate) fn menu_has_icons14(&self) -> bool {
        self.menu.as_ref().is_some_and(|(m, _)| m.has_icons())
    }
}

#[cfg(test)]
#[path = "reader_r14_tests.rs"]
mod tests;
