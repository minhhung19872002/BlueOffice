//! BlueDoc round 10 context menus of the document (docs/ui/REPORT.md "Vòng 10"), rows and
//! access keys as the reference shows them (posted right-clicks into its own instance, rows read
//! with UI Automation): on text, on a misspelt word (suggestions first), in a table, on a picture
//! and on a hyperlink, with the mini toolbar above the menu (below it when there is no room).
//! Rows the reference has and BlueDoc cannot do yet (Copy Link to Location, Read Aloud,
//! Translate, Synonyms) are left out rather than shown dead. A child module of the shell.
use super::{Choice, DocWindow, Menu, MenuItem, Request};
use crate::popup::MenuLook;
use crate::{Input, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

/// Rows of an open context menu that BlueDoc draws itself and the mini toolbar.
pub(super) struct Ctx {
    /// The menu these belong to (its box): another menu opening hides them.
    menu: [f32; 4],
    /// Row of the "Paste Options:" heading, the next row holds the paste icons.
    paste_head: Option<usize>,
    /// The mini toolbar.
    bar: Option<MiniBar>,
    /// The hyperlink under the pointer (Open / Copy Hyperlink).
    link: Option<String>,
}

#[cfg(test)]
impl Ctx {
    /// The mini toolbar's box.
    pub(super) fn bar_rect(&self) -> Option<[f32; 4]> {
        self.bar.as_ref().map(|b| b.rect)
    }

    /// The box of mini toolbar button `id`.
    pub(super) fn button(&self, id: &str) -> Option<[f32; 4]> {
        let bar = self.bar.as_ref()?;
        bar.buttons.iter().find(|(b, _)| *b == id).map(|(_, r)| *r)
    }
}

/// The mini toolbar: buttons with their boxes (window pixels).
struct MiniBar {
    rect: [f32; 4],
    buttons: Vec<(&'static str, [f32; 4])>,
    font: String,
    size: String,
    pressed: Option<usize>,
}

/// Paste option icons in the menu's paste row, as on the reference (keep source formatting,
/// merge formatting, keep text only).
const PASTE_MODES: [&str; 3] = ["paste.keep", "paste.merge", "paste.text"];

/// Reference measurements at 125 % (device pixels; scaled by `scale / 1.25`).
mod m {
    /// Gap between the mini toolbar and the menu.
    pub const GAP: f32 = 19.0;
    /// Mini toolbar size (text): 456 × 94; (picture) 114 × 94.
    pub const BAR: (f32, f32) = (456.0, 94.0);
    pub const BAR_PICTURE: (f32, f32) = (114.0, 94.0);
}

const BAR_FRAME: Rgba = Rgba(0xC6, 0xC6, 0xC6, 255);
const COMBO_FRAME: Rgba = Rgba(0xAB, 0xAB, 0xAB, 255);
const HEAD_FILL: Rgba = Rgba(0xE8, 0xE8, 0xE8, 255);
const HOVER: Rgba = Rgba(0xE5, 0xF1, 0xFB, 255);

/// One row being built.
struct Row {
    label: String,
    id: String,
    icon: Option<&'static str>,
    dim: bool,
    arrow: bool,
    sep: bool,
}

impl DocWindow {
    fn r10_label(&self, key: &str) -> String {
        self.strings.get(&format!("docCtx.{key}")).to_owned()
    }

    fn r10_row(&self, key: &str, id: &str, icon: Option<&'static str>) -> Row {
        Row {
            label: self.r10_label(key),
            id: id.to_owned(),
            icon,
            dim: false,
            arrow: false,
            sep: false,
        }
    }

    /// Open the context menu for window point (`x`, `y`) (right-click, Shift+F10, menu key).
    pub(super) fn r10_context_menu(&mut self, x: f32, y: f32, text: &dyn TextMeasure) {
        let f = self.view.format().clone();
        let selected = self.view.has_selection();
        let picture = f.picture.is_some() && selected;
        let table = f.table.is_some();
        let link = f.link.clone().filter(|l| !l.is_empty());
        self.spell_menu = if picture {
            None
        } else {
            self.view.spell_at(x, y)
        };
        let mut rows: Vec<Row> = Vec::new();
        // A misspelt word: its corrections, then Ignore All / Add to Dictionary.
        if let Some(info) = self.spell_menu.clone() {
            for (i, s) in info.suggestions.iter().take(5).enumerate() {
                rows.push(Row {
                    label: s.clone(),
                    id: format!("spell.use.{i}"),
                    icon: None,
                    dim: false,
                    arrow: false,
                    sep: false,
                });
            }
            if info.suggestions.is_empty() {
                let mut r = self.r10_row("noSuggestions", "ctx.none", None);
                r.dim = true;
                rows.push(r);
            }
            if info.rule.is_none() {
                rows.push(self.r10_row("ignoreAll", "spell.ignoreAll", None));
                rows.push(self.r10_row("addDictionary", "spell.addToDictionary", None));
            }
        }
        let first = rows.len();
        let mut cut = self.r10_row("cut", "edit.cut", Some("edit.cut"));
        cut.dim = !selected;
        cut.sep = first > 0;
        rows.push(cut);
        let mut copy = self.r10_row("copy", "edit.copy", Some("edit.copy"));
        copy.dim = !selected;
        rows.push(copy);
        let head = rows.len();
        rows.push(self.r10_row("pasteOptions", "ctx.none", Some("edit.paste")));
        rows.push(Row {
            label: String::new(),
            id: "ctx.pasteRow".into(),
            icon: None,
            dim: false,
            arrow: false,
            sep: false,
        });
        if let Some(target) = &link {
            let _ = target;
            let mut r = self.r10_row("editLink", "insert.link", Some("insert.link"));
            r.sep = true;
            rows.push(r);
            rows.push(self.r10_row("openLink", "ctx.link.open", None));
            rows.push(self.r10_row("copyLink", "ctx.link.copy", None));
            rows.push(self.r10_row("removeLink", "ctx.link.remove", None));
        }
        if picture {
            let mut r = self.r10_row("changePicture", "picture.change.file", None);
            r.sep = true;
            rows.push(r);
            let mut r = self.r10_row("link", "insert.link", Some("insert.link"));
            r.sep = true;
            rows.push(r);
            rows.push(self.r10_row("caption", "references.insertCaption", None));
            let mut r = self.r10_row("wrapText", "ctx.sub.wrap", None);
            r.arrow = true;
            rows.push(r);
            rows.push(self.r10_row("altText", "picture.altText", None));
            rows.push(self.r10_row("sizePosition", "picture.layoutOptions", None));
            rows.push(self.r10_row("formatPicture", "picture.layout", None));
        } else if table {
            let mut r = self.r10_row("insert", "ctx.sub.insert", None);
            r.arrow = true;
            r.sep = true;
            rows.push(r);
            rows.push(self.r10_row("deleteCells", "table.deleteCell", None));
            rows.push(self.r10_row("splitCells", "table.splitCell", None));
            rows.push(self.r10_row("borderStyles", "table.borderStyles", None));
            rows.push(self.r10_row("textDirection", "table.textDirection", None));
            rows.push(self.r10_row("tableProperties", "table.properties", None));
            let mut r = self.r10_row("link", "insert.link", Some("insert.link"));
            r.sep = true;
            rows.push(r);
            rows.push(self.r10_row("newComment", "review.newComment", Some("review.newComment")));
        } else {
            let mut r = self.r10_row("font", "format.fontDialog", None);
            r.sep = true;
            rows.push(r);
            rows.push(self.r10_row("paragraph", "format.paragraphSettings", None));
            if link.is_none() {
                let mut r = self.r10_row("link", "insert.link", Some("insert.link"));
                r.sep = true;
                rows.push(r);
            }
            let mut r = self.r10_row("newComment", "review.newComment", Some("review.newComment"));
            r.sep = link.is_some();
            rows.push(r);
        }
        let items = rows
            .iter()
            .map(|r| MenuItem {
                label: r.label.clone(),
                choice: Choice::Command(r.id.clone()),
                checked: false,
                dim: r.dim,
            })
            .collect();
        let icons = rows.iter().map(|r| r.icon).collect();
        let seps = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.sep)
            .map(|(i, _)| i)
            .collect();
        let arrows = rows.iter().map(|r| r.arrow).collect();
        let look = MenuLook {
            min_w: 342.0 / 1.25,
            ..MenuLook::default()
        };
        let menu = Menu::open_with(items, [x, y - 2.0, 1.0, 0.0], self.size, self.scale, look)
            .with_decor(icons, seps)
            .with_arrows(arrows, self.theme.hover)
            .with_access_keys(text, false);
        let rect = menu.rect();
        self.menu = Some(menu);
        let bar = Some(self.r10_minibar(rect, picture, &f));
        self.r10.ctx = Some(Ctx {
            menu: rect,
            paste_head: Some(head),
            bar,
            link,
        });
    }

    /// Lay out the mini toolbar next to a menu at `menu` (above, else below).
    fn r10_minibar(&self, menu: [f32; 4], picture: bool, f: &bo_edit::FormatState) -> MiniBar {
        let k = self.scale / 1.25;
        let (bw, bh) = if picture { m::BAR_PICTURE } else { m::BAR };
        let (w, h) = ((bw * k).round(), (bh * k).round());
        let gap = (m::GAP * k).round();
        let above = menu[1] - gap - h;
        let y = if above >= self.view_top() {
            above
        } else {
            menu[1] + menu[3] + gap
        };
        let x = menu[0].min((self.size.0 - w).max(0.0));
        let b = |id: &'static str, bx: f32, by: f32, bw: f32, bh: f32| {
            (
                id,
                [
                    x + (bx * k).round(),
                    y + (by * k).round(),
                    (bw * k).round(),
                    (bh * k).round(),
                ],
            )
        };
        let buttons = if picture {
            vec![
                b("picture.quickStyles", 5.0, 5.0, 58.0, 84.0),
                b("picture.crop", 63.0, 5.0, 46.0, 84.0),
            ]
        } else {
            vec![
                b("format.fontDialog", 5.0, 8.0, 144.0, 36.0),
                b("ctx.size", 149.0, 8.0, 68.0, 36.0),
                b("format.growStep", 217.0, 9.0, 32.0, 34.0),
                b("format.shrinkStep", 249.0, 9.0, 32.0, 34.0),
                b("format.painter", 281.0, 9.0, 32.0, 34.0),
                b("format.bold", 5.0, 51.0, 32.0, 34.0),
                b("format.italic", 37.0, 51.0, 32.0, 34.0),
                b("format.underline", 69.0, 51.0, 32.0, 34.0),
                b("format.highlight", 101.0, 51.0, 53.0, 34.0),
                b("format.color", 154.0, 51.0, 53.0, 34.0),
                b("format.bullets", 207.0, 51.0, 53.0, 34.0),
                b("format.numbering", 260.0, 51.0, 53.0, 34.0),
                b("view.stylesPane", 314.0, 5.0, 58.0, 84.0),
                b("review.newComment", 373.0, 5.0, 77.0, 84.0),
            ]
        };
        let size = f.size_half_points.map_or_else(String::new, |h| {
            if h % 2 == 0 {
                (h / 2).to_string()
            } else {
                format!("{}.5", h / 2)
            }
        });
        MiniBar {
            rect: [x, y, w, h],
            buttons,
            font: f.font.clone().unwrap_or_default(),
            size,
            pressed: None,
        }
    }

    /// Our context menu is the open menu.
    fn r10_ctx_open(&self) -> bool {
        match (&self.menu, &self.r10.ctx) {
            (Some(menu), Some(c)) => menu.rect() == c.menu,
            _ => false,
        }
    }

    /// Pointer input on the mini toolbar or the paste icons while our menu is open; `None`
    /// passes it on to the menu.
    pub(super) fn r10_menu_input(&mut self, input: &Input) -> Option<bool> {
        if !self.r10_ctx_open() {
            return None;
        }
        let ctx = self.r10.ctx.as_mut()?;
        match *input {
            Input::Press { x, y, .. } => {
                let bar = ctx.bar.as_mut()?;
                if !inside(bar.rect, x, y) {
                    return None;
                }
                bar.pressed = bar.buttons.iter().position(|(_, r)| inside(*r, x, y));
                Some(true)
            }
            Input::Release { x, y } => {
                let bar = ctx.bar.as_mut()?;
                let pressed = bar.pressed.take()?;
                let (id, r) = bar.buttons.get(pressed).copied()?;
                if !inside(r, x, y) {
                    return Some(true);
                }
                self.menu = None;
                self.r10.ctx = None;
                let id = if id == "ctx.size" {
                    "format.fontDialog"
                } else {
                    id
                };
                self.r10_run(id);
                Some(true)
            }
            _ => None,
        }
    }

    /// Run a context-menu row's command.
    fn r10_run(&mut self, id: &str) -> bool {
        self.action(id)
    }

    /// The round's context-menu commands (`None`: not one of them).
    pub(super) fn r10_action(&mut self, id: &str) -> Option<bool> {
        let ctx_link = self.r10.ctx.as_ref().and_then(|c| c.link.clone());
        Some(match id {
            "ctx.none" => false,
            "ctx.pasteRow" => {
                // The icon under the pointer (keep / merge / text).
                let (x, _) = self.r10.pointer;
                let i = self.r10_paste_icon_at(x).unwrap_or(0);
                self.r10.ctx = None;
                self.action(PASTE_MODES[i])
            }
            "ctx.link.open" => {
                if let Some(url) = ctx_link.filter(|u| is_web(u)) {
                    self.requests.push(Request::OpenUrl(url));
                }
                false
            }
            "ctx.link.copy" => {
                if let Some(url) = ctx_link {
                    self.requests.push(Request::SetClipboard {
                        text: url,
                        html: String::new(),
                        internal: String::new(),
                    });
                }
                false
            }
            "ctx.link.remove" => {
                // The whole link: select it first when the caret is only inside it.
                self.view.run_command(bo_engine::edit::Command::SetLink {
                    target: String::new(),
                    display: String::new(),
                })
            }
            "ctx.sub.insert" | "ctx.sub.wrap" => {
                self.r10_submenu(id);
                true
            }
            _ => return None,
        })
    }

    /// Open the Insert (table) or Wrap Text (picture) sub-menu beside the row chosen.
    fn r10_submenu(&mut self, id: &str) {
        let rows: &[(&str, &str)] = if id == "ctx.sub.insert" {
            &[
                ("insertAbove", "table.insertRowAbove"),
                ("insertBelow", "table.insertRowBelow"),
                ("insertLeft", "table.insertColumnLeft"),
                ("insertRight", "table.insertColumnRight"),
            ]
        } else {
            &[
                ("wrapInline", "picture.wrap.inline"),
                ("wrapSquare", "picture.wrap.square"),
                ("wrapTight", "picture.wrap.tight"),
                ("wrapThrough", "picture.wrap.through"),
                ("wrapTopBottom", "picture.wrap.topAndBottom"),
                ("wrapBehind", "picture.wrap.behind"),
                ("wrapFront", "picture.wrap.inFront"),
            ]
        };
        let anchor = self.r10.ctx.as_ref().map_or([0.0; 4], |c| {
            let (x, y) = self.r10.pointer;
            let _ = x;
            [c.menu[0] + c.menu[2] - 2.0, y - 12.0, 1.0, 0.0]
        });
        let items = rows
            .iter()
            .map(|(k, id)| MenuItem {
                label: self.r10_label(k),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: false,
            })
            .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        self.r10.ctx = None;
    }

    /// Which paste icon is at window x (the paste row).
    fn r10_paste_icon_at(&self, x: f32) -> Option<usize> {
        let menu = self.menu.as_ref()?;
        let rect = menu.rect();
        let k = self.scale / 1.25;
        let x0 = rect[0] + (46.0 * k).round();
        let step = (40.0 * k).round();
        let i = ((x - x0) / step).floor();
        (0.0..3.0).contains(&i).then_some(i as usize)
    }

    /// Draw the paste heading and icons over our menu, and the mini toolbar.
    pub(super) fn r10_paint_menu(&self, p: &mut dyn Painter) {
        if !self.r10_ctx_open() {
            return;
        }
        let (Some(menu), Some(ctx)) = (&self.menu, &self.r10.ctx) else {
            return;
        };
        let s = self.scale;
        let k = s / 1.25;
        let t = &self.theme;
        if let Some(head) = ctx.paste_head {
            let [hx, hy, hw, hh] = menu.row_rect(head);
            let pad = (4.0 * s).round();
            let text_x = hx + (34.0 * s).round();
            p.fill_rect(
                text_x - (6.0 * s).round(),
                hy,
                hw - (text_x - hx) + (6.0 * s).round() - pad,
                hh,
                HEAD_FILL,
            );
            let style = TextStyle {
                size: (13.0 * s).round(),
                bold: true,
            };
            p.text(
                &self.r10_label("pasteOptions"),
                text_x,
                hy + (hh - style.size * 1.33) / 2.0,
                style,
                t.text,
            );
            let [_, ry, _, rh] = menu.row_rect(head + 1);
            let size = (24.0 * k).round();
            for (i, name) in PASTE_MODES.iter().enumerate() {
                let ix = hx + (46.0 * k).round() + i as f32 * (40.0 * k).round();
                let iy = (ry + (rh - size) / 2.0).round();
                let hover = menu.hovered() == Some(head + 1)
                    && self.r10_paste_icon_at(self.r10.pointer.0) == Some(i);
                if hover {
                    p.fill_rect(ix - 4.0, iy - 4.0, size + 8.0, size + 8.0, HOVER);
                }
                paste_icon(p, ix, iy, size, i);
                let _ = name;
            }
        }
        if let Some(bar) = &ctx.bar {
            self.r10_paint_bar(p, bar);
        }
    }

    fn r10_paint_bar(&self, p: &mut dyn Painter, bar: &MiniBar) {
        let s = self.scale;
        let t = &self.theme;
        let [x, y, w, h] = bar.rect;
        for (o, a) in [(3.0, 14u8), (2.0, 22), (1.0, 30)] {
            let o = (o * s).round();
            p.fill_rect(
                x - o + s,
                y - o + 2.0 * s,
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, BAR_FRAME);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Rgba(255, 255, 255, 255));
        let style = TextStyle {
            size: (12.0 * s).round(),
            bold: false,
        };
        let f = self.view.format();
        let (px, py) = self.r10.pointer;
        for (i, (id, r)) in bar.buttons.iter().enumerate() {
            let [bx, by, bw, bh] = *r;
            let hover = inside(*r, px, py);
            let on = match *id {
                "format.bold" => f.bold,
                "format.italic" => f.italic,
                "format.underline" => f.underline,
                "format.bullets" => f.bullets,
                "format.numbering" => f.numbering,
                _ => false,
            };
            if hover || on || bar.pressed == Some(i) {
                p.fill_rect(
                    bx,
                    by,
                    bw,
                    bh,
                    if on {
                        Rgba(0xC5, 0xC5, 0xC5, 255)
                    } else {
                        HOVER
                    },
                );
            }
            match *id {
                "format.fontDialog" | "ctx.size" => {
                    // A combo box: frame, value, drop-down arrow.
                    let inset = (2.0 * s).round();
                    let (cx, cy, cw, ch) =
                        (bx + inset, by + inset, bw - 2.0 * inset, bh - 2.0 * inset);
                    p.fill_rect(cx, cy, cw, ch, COMBO_FRAME);
                    p.fill_rect(
                        cx + 1.0,
                        cy + 1.0,
                        cw - 2.0,
                        ch - 2.0,
                        Rgba(255, 255, 255, 255),
                    );
                    let value = if *id == "ctx.size" {
                        &bar.size
                    } else {
                        &bar.font
                    };
                    p.push_clip(cx + 1.0, cy, cw - (22.0 * s).round(), ch);
                    p.text(
                        value,
                        cx + (5.0 * s).round(),
                        cy + (ch - style.size * 1.33) / 2.0,
                        style,
                        t.text,
                    );
                    p.pop_clip();
                    arrow_down(p, cx + cw - (12.0 * s).round(), cy + ch / 2.0, s, t.text);
                }
                "view.stylesPane"
                | "review.newComment"
                | "picture.quickStyles"
                | "picture.crop" => {
                    let size = (32.0 * s / 1.25 * 1.0).round();
                    let icon = match *id {
                        "review.newComment" => "review.newComment",
                        "view.stylesPane" => "styles",
                        other => other,
                    };
                    let ix = (bx + (bw - size) / 2.0).round();
                    if self.icons.has(icon) {
                        self.icons
                            .draw(p, icon, ix, by + (6.0 * s).round(), size, colors(t, true));
                    }
                    let label = self.r10_label(match *id {
                        "review.newComment" => "newCommentBtn",
                        "picture.crop" => "crop",
                        "picture.quickStyles" => "style",
                        _ => "styles",
                    });
                    let mut ly = by + size + (10.0 * s).round();
                    for part in label.split(' ') {
                        let tw = part.chars().count() as f32 * style.size * 0.5;
                        p.text(part, (bx + (bw - tw) / 2.0).round(), ly, style, t.text);
                        ly += (style.size * 1.25).round();
                    }
                }
                _ => {
                    let size = (16.0 * s).round();
                    let main_w = (32.0 * s / 1.25).round();
                    let ix = (bx + (main_w - size) / 2.0).round();
                    let iy = (by + (bh - size) / 2.0).round();
                    glyph(p, &self.icons, t, id, ix, iy, size, s);
                    if bw > main_w + 1.0 {
                        arrow_down(
                            p,
                            bx + main_w + (8.0 * s / 1.25).round(),
                            by + bh / 2.0,
                            s,
                            t.text,
                        );
                    }
                }
            }
        }
    }
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn is_web(u: &str) -> bool {
    let l = u.to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:")
}

fn colors(t: &crate::theme::Theme, on: bool) -> crate::icons::IconColors {
    crate::icons::IconColors {
        ink: t.text,
        accent: t.accent,
        on: t.accent_on,
        tinted: on,
    }
}

fn arrow_down(p: &mut dyn Painter, cx: f32, cy: f32, s: f32, c: Rgba) {
    let (w, h) = ((4.0 * s).round(), (2.0 * s).round());
    p.path(
        &[
            PathSeg::M(cx - w, cy - h / 2.0),
            PathSeg::L(cx, cy + h),
            PathSeg::L(cx + w, cy - h / 2.0),
        ],
        None,
        Some((c, s.max(1.0))),
    );
}

/// A toolbar button's face: the shared icon when there is one, else a letter of our own
/// (bold B, italic I, underlined U, "A" over a colour bar for highlight / font colour).
#[allow(clippy::too_many_arguments)]
fn glyph(
    p: &mut dyn Painter,
    icons: &crate::icons::Icons,
    t: &crate::theme::Theme,
    id: &str,
    x: f32,
    y: f32,
    size: f32,
    s: f32,
) {
    if icons.has(id) {
        icons.draw(p, id, x, y, size, colors(t, true));
        return;
    }
    let style = TextStyle {
        size: (14.0 * s).round(),
        bold: matches!(id, "format.bold" | "format.growStep" | "format.shrinkStep"),
    };
    let (letter, bar) = match id {
        "format.bold" => ("B", None),
        "format.italic" => ("I", None),
        "format.underline" => ("U", Some(t.text)),
        "format.highlight" => ("ab", Some(Rgba(0xFF, 0xFF, 0x00, 255))),
        "format.color" => ("A", Some(Rgba(0xFF, 0x00, 0x00, 255))),
        "format.growStep" => ("A\u{02c4}", None),
        "format.shrinkStep" => ("A\u{02c5}", None),
        _ => ("", None),
    };
    let ty = y + (size - style.size * 1.33) / 2.0 - (2.0 * s).round();
    p.text(letter, x + (2.0 * s).round(), ty, style, t.text);
    if let Some(c) = bar {
        let h = if id == "format.underline" {
            1.0
        } else {
            (3.0 * s).round()
        };
        p.fill_rect(x, (y + size - h).round(), size, h, c);
    }
}

/// Paste option icons of our own: a clipboard with a page (keep source formatting), with a
/// brush mark (merge) and with an "A" (text only).
fn paste_icon(p: &mut dyn Painter, x: f32, y: f32, size: f32, kind: usize) {
    let u = size / 24.0;
    let board = Rgba(0x8A, 0x6A, 0x3A, 255);
    let clip = Rgba(0xF0, 0xB4, 0x3C, 255);
    let ink = Rgba(0x44, 0x44, 0x44, 255);
    p.fill_rect(x + 3.0 * u, y + 3.0 * u, 14.0 * u, 18.0 * u, board);
    p.fill_rect(x + 4.0 * u, y + 4.0 * u, 12.0 * u, 16.0 * u, clip);
    p.fill_rect(x + 7.0 * u, y + 2.0 * u, 6.0 * u, 3.0 * u, ink);
    p.fill_rect(x + 10.0 * u, y + 10.0 * u, 12.0 * u, 13.0 * u, ink);
    p.fill_rect(
        x + 11.0 * u,
        y + 11.0 * u,
        10.0 * u,
        11.0 * u,
        Rgba(255, 255, 255, 255),
    );
    match kind {
        0 => {
            for k in 0..3 {
                p.fill_rect(
                    x + 13.0 * u,
                    y + (14.0 + 2.5 * k as f32) * u,
                    6.0 * u,
                    u.max(1.0),
                    ink,
                );
            }
        }
        1 => {
            p.fill_rect(
                x + 13.0 * u,
                y + 13.0 * u,
                6.0 * u,
                3.0 * u,
                Rgba(0x2B, 0x7C, 0xD3, 255),
            );
            p.fill_rect(x + 15.0 * u, y + 16.0 * u, 2.0 * u, 4.0 * u, ink);
        }
        _ => {
            p.path(
                &[
                    PathSeg::M(x + 12.5 * u, y + 21.0 * u),
                    PathSeg::L(x + 16.0 * u, y + 12.5 * u),
                    PathSeg::L(x + 19.5 * u, y + 21.0 * u),
                    PathSeg::M(x + 13.8 * u, y + 18.0 * u),
                    PathSeg::L(x + 18.2 * u, y + 18.0 * u),
                ],
                None,
                Some((ink, u.max(1.0))),
            );
        }
    }
}
