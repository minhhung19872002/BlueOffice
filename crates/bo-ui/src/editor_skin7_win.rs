//! BlueEditor round 7 (a child of `reader_window`): the File page's Skins in BlueEditor. The
//! palettes are BlueReader's (`reader_skins8_win.rs`) laid over the editor's own measured look
//! (Classic keeps it); the dark skin stays grey there too.
use super::skins8::SKINS;
use super::ReaderWindow;

/// Index of the dark skin (offered grey, as in BlueReader).
const DARK: usize = 3;

impl ReaderWindow {
    /// Documents (tab indices) the next print prints whole with one set of settings.
    pub(crate) fn set_batch(&mut self, tabs: Vec<usize>) {
        self.prefs6.batch = tabs;
    }

    /// Skin chosen on the File page.
    #[cfg(test)]
    pub(crate) fn skin_index(&self) -> usize {
        self.prefs6.prefs.skin
    }

    /// Use skin `k` over the product's own look (BlueEditor); false when it is not offered.
    pub(crate) fn use_product_skin(&mut self, k: usize) -> bool {
        let Some(p) = self.profile.as_ref() else {
            return false;
        };
        if k == DARK || k >= SKINS.len() {
            return false;
        }
        let mut l = p.look;
        if let Some((_, Some(pal))) = SKINS.get(k) {
            l.chrome = pal[0];
            l.edge = pal[1];
            l.canvas = pal[2];
            l.strip_bg = pal[2];
            l.nav_bg = pal[3];
            l.pressed = pal[4];
            l.hover = pal[5];
            l.strip = pal[6];
        }
        self.look = l;
        self.prefs6.prefs.skin = k;
        true
    }
}
