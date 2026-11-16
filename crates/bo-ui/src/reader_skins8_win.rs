//! BlueReader round 8 (a child of `reader_window`): the File page's Share and Skins, done with
//! our own code. Share: send by e-mail (a `mailto:` message naming the file), copy the file's
//! path, show it in its folder. Skins: our own palettes of the window's greys (Classic, the
//! measured greys; Light; Blue; Dark, round 9: the whole window painted through the dark
//! skin's palette function, `dark9`).
use super::{rounded, Look, ReaderRequest, ReaderWindow};
use crate::reader_icons::{self, Ink};
use crate::{Painter, TextStyle};
use bo_render::Rgba;

const TEXT: Rgba = Rgba(0x22, 0x22, 0x22, 255);
const MUTED: Rgba = Rgba(0x70, 0x70, 0x70, 255);
const GREY: Rgba = Rgba(0xC1, 0xC1, 0xC1, 255);
const FRAME: Rgba = Rgba(199, 204, 217, 255);

/// Skins: label key, palette (chrome, edge, canvas, nav, pressed, hover, strip); `None` for one
/// not available yet.
type Palette = [Rgba; 7];
pub(crate) const SKINS: [(&str, Option<Palette>); 4] = [
    ("pdfSkins.classic", None),
    (
        "pdfSkins.light",
        Some([
            Rgba(255, 255, 255, 255),
            Rgba(214, 214, 216, 255),
            Rgba(243, 243, 244, 255),
            Rgba(250, 250, 250, 255),
            Rgba(229, 229, 231, 255),
            Rgba(240, 240, 242, 255),
            Rgba(236, 236, 238, 255),
        ]),
    ),
    (
        "pdfSkins.blue",
        Some([
            Rgba(246, 249, 253, 255),
            Rgba(190, 204, 224, 255),
            Rgba(228, 236, 246, 255),
            Rgba(236, 242, 250, 255),
            Rgba(214, 226, 242, 255),
            Rgba(230, 238, 248, 255),
            Rgba(214, 226, 242, 255),
        ]),
    ),
    ("pdfSkins.dark", None),
];

/// Skin index of the dark skin, and its palette function.
use super::dark9::{dark_ink, DARK};

/// Look of skin `k` over the measured BlueReader look.
pub(crate) fn skin_look(k: usize) -> Look {
    let mut l = Look::READER;
    if let Some((_, Some(p))) = SKINS.get(k) {
        l.chrome = p[0];
        l.edge = p[1];
        l.canvas = p[2];
        l.strip_bg = p[2];
        l.nav_bg = p[3];
        l.pressed = p[4];
        l.hover = p[5];
        l.strip = p[6];
    }
    l
}

/// `%XX` escapes for a `mailto:` field (RFC 6068).
fn mail_escape(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl ReaderWindow {
    /// Use skin `k` (BlueReader only).
    pub(crate) fn apply_skin(&mut self, k: usize) {
        if self.profile.is_some() || k >= SKINS.len() {
            return;
        }
        self.look = skin_look(k);
    }

    /// File page commands of Share and Skins; true when handled.
    pub(crate) fn backstage_command8(&mut self, cmd: &str) -> bool {
        let path = self.tabs.get(self.active).and_then(|t| t.path.clone());
        let name = self
            .tabs
            .get(self.active)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        if let Some(k) = cmd
            .strip_prefix("bs.skin.")
            .and_then(|v| v.parse::<usize>().ok())
        {
            if k < SKINS.len() {
                self.prefs6.prefs.skin = k;
                self.apply_skin(k);
                self.requests
                    .push(ReaderRequest::SavePreferences(self.prefs6.prefs.to_text()));
            }
            return true;
        }
        match cmd {
            "bs.shareMail" => {
                let mut url = format!("mailto:?subject={}", mail_escape(&name));
                if let Some(p) = &path {
                    url.push_str("&body=");
                    url.push_str(&mail_escape(&p.to_string_lossy()));
                }
                self.requests.push(ReaderRequest::OpenUrl(url));
            }
            "bs.shareCopy" => {
                if let Some(p) = &path {
                    self.requests
                        .push(ReaderRequest::Copy(p.to_string_lossy().into_owned()));
                    self.status = self.t("pdfShare.copied");
                }
            }
            "bs.shareFolder" => {
                if let Some(p) = &path {
                    self.requests.push(ReaderRequest::Command(format!(
                        "host.reveal:{}",
                        p.to_string_lossy()
                    )));
                }
            }
            _ => return false,
        }
        true
    }

    /// Paint the Share or Skins page right of the File column.
    pub(crate) fn paint_backstage8(
        &mut self,
        p: &mut dyn Painter,
        page: &str,
        mx: f32,
        top: f32,
        hits: &mut Vec<([f32; 4], String)>,
    ) {
        let sc = self.scale;
        let px = move |v: f32| (v * sc).round();
        let style = |s: f32| TextStyle {
            size: s / 1.375 * sc,
            bold: false,
        };
        let (title, item, small) = (style(26.0), style(17.0), style(14.0));
        let ty = top + px(14.8);
        let ink = Ink {
            ink: Rgba(0x40, 0x40, 0x40, 255),
            accent: self.theme.accent,
        };
        if page == "bs.share" {
            p.text(&self.t("pdfShare.title"), mx - px(0.8), ty, title, MUTED);
            let has_path = self.tabs.get(self.active).is_some_and(|t| t.path.is_some());
            let has_doc = self.view().is_some();
            let rows = [
                (
                    "bs.shareMail",
                    "mail",
                    "pdfShare.mail",
                    "pdfShare.mailHint",
                    has_doc,
                ),
                (
                    "bs.shareCopy",
                    "copy",
                    "pdfShare.copy",
                    "pdfShare.copyHint",
                    has_path,
                ),
                (
                    "bs.shareFolder",
                    "open",
                    "pdfShare.folder",
                    "pdfShare.folderHint",
                    has_path,
                ),
            ];
            let mut y = top + px(85.6);
            for (cmd, icon, key, hint, on) in rows {
                let r = [mx - px(9.6), y, px(460.0), px(54.4)];
                p.path(
                    &rounded(r[0], r[1], r[2], r[3], px(3.2)),
                    None,
                    Some((FRAME, 1.0)),
                );
                let is = px(24.0);
                let c = if on {
                    ink
                } else {
                    Ink {
                        ink: GREY,
                        accent: GREY,
                    }
                };
                reader_icons::draw(p, icon, r[0] + px(12.0), r[1] + (r[3] - is) / 2.0, is, c);
                p.text(
                    &self.t(key),
                    r[0] + px(48.0),
                    r[1] + px(7.0),
                    item,
                    if on { TEXT } else { GREY },
                );
                p.text(
                    &self.t(hint),
                    r[0] + px(48.0),
                    r[1] + px(30.0),
                    small,
                    MUTED,
                );
                if on {
                    hits.push((r, cmd.to_owned()));
                }
                y += px(64.0);
            }
            return;
        }
        p.text(&self.t("pdfSkins.title"), mx - px(0.8), ty, title, MUTED);
        let mut x = mx - px(9.6);
        let y = top + px(85.6);
        let current = self.prefs6.prefs.skin;
        for (k, (key, _)) in SKINS.iter().enumerate() {
            let r = [x, y, px(150.0), px(132.0)];
            let edge = if k == current {
                self.theme.accent
            } else {
                FRAME
            };
            p.path(
                &rounded(r[0], r[1], r[2], r[3], px(3.2)),
                Some(Rgba(255, 255, 255, 255)),
                Some((edge, if k == current { 2.0 } else { 1.0 })),
            );
            // A small window in the skin's greys.
            let look = skin_look(k);
            let ink = |c: Rgba| if k == DARK { dark_ink(c) } else { c };
            let (edge_c, chrome, nav, canvas) = (
                ink(look.edge),
                ink(look.chrome),
                ink(look.nav_bg),
                ink(look.canvas),
            );
            let (ix, iy, iw, ih) = (r[0] + px(10.0), r[1] + px(10.0), r[2] - px(20.0), px(80.0));
            // The swatch shows the skin's own colours whatever skin paints the page.
            p.raw_colours(true);
            p.fill_rect(ix, iy, iw, ih, edge_c);
            p.fill_rect(ix + 1.0, iy + 1.0, iw - 2.0, px(18.0), chrome);
            p.fill_rect(ix + 1.0, iy + px(19.0), px(16.0), ih - px(20.0), nav);
            p.fill_rect(
                ix + px(17.0),
                iy + px(19.0),
                iw - px(18.0),
                ih - px(20.0),
                canvas,
            );
            p.fill_rect(
                ix + px(42.0),
                iy + px(26.0),
                iw - px(68.0),
                ih - px(30.0),
                Rgba(255, 255, 255, 255),
            );
            p.fill_rect(
                ix + px(6.0),
                iy + px(6.0),
                px(26.0),
                px(4.0),
                self.theme.accent,
            );
            p.raw_colours(false);
            let label = self.t(key);
            p.text(&label, r[0] + px(10.0), r[1] + px(98.0), item, TEXT);
            hits.push((r, format!("bs.skin.{k}")));
            x += r[2] + px(16.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skins_and_mail_links() {
        assert_eq!(skin_look(0), Look::READER);
        assert_eq!(skin_look(1).chrome, Rgba(255, 255, 255, 255));
        assert_eq!(skin_look(DARK), Look::READER);
        assert_eq!(mail_escape("a b&c.pdf"), "a%20b%26c.pdf");
    }
}
