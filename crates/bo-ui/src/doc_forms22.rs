//! Dialogs of BlueDoc round 22 (docs/ui/REPORT.md "Vòng 112"): Insert › Object › Display as
//! icon with its preview and Change Icon… (measured on the reference at 125 %: the check box
//! right of the type list, the icon and caption under it, Change Icon… below; Change Icon is
//! 490 × 318 px with File name, Icon list, Caption, OK / Cancel). The icons are BlueOffice's
//! own drawings — the same as the picture the engine stores (`bo_engine` `ole_icon`).
use super::{DocForm, DocFormKind, FormContext, FormOut};
use crate::strings::Strings;
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// File › Options › Add-ins › Manage ▾ choices (key, English), in the combo's order.
pub(crate) const MANAGE: [(&str, &str); 4] = [
    ("doc20.manageWeb", "Web Add-ins"),
    ("doc20.manageScripts", "BlueScript scripts"),
    ("doc22.manageBuiltIn", "BlueDoc Add-ins"),
    ("doc22.manageDisabled", "Disabled Items"),
];

/// The Go… dialog of Manage choice `c.extra["r22"]["which"]`: what is installed of that
/// kind, as the reference's COM Add-ins / Templates / Disabled Items dialogs.
fn open_addins(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let which = c.extra["r22"]["which"].as_u64().unwrap_or(0) as usize;
    let (tk, ten) = MANAGE[which.min(MANAGE.len() - 1)];
    let mut f = DocForm::new(kind, &tr(tk, ten), (560.0, 380.0), vec![]);
    let (head, rows, empty): (String, Vec<String>, String) = match which {
        2 => {
            let (active, _) = super::options20::addin_rows(s);
            (
                tr("doc22.addinsAvailable", "Add-ins available:"),
                active.into_iter().map(|r| r[0].clone()).collect(),
                String::new(),
            )
        }
        3 => (
            tr(
                "doc22.disabledHead",
                "The items below are disabled because they prevented BlueDoc from working correctly.",
            ),
            Vec::new(),
            tr("doc22.noDisabled", "No items are disabled."),
        ),
        1 => (
            tr("doc22.scriptsHead", "Scripts saved for you:"),
            Vec::new(),
            tr(
                "doc22.noScripts",
                "No BlueScript scripts are saved on this computer.",
            ),
        ),
        _ => (
            tr("doc22.webHead", "Web add-ins of your organisation:"),
            Vec::new(),
            tr(
                "doc22.noWeb",
                "No web add-ins: this computer is not connected to a BlueOffice server.",
            ),
        ),
    };
    f.label("head", [20.0, 52.0, 520.0, 18.0], &head);
    let n = rows.len();
    f.list(
        "items",
        [20.0, 78.0, 400.0, 220.0],
        rows,
        (n > 0).then_some(0),
    );
    f.label("empty", [30.0, 90.0, 380.0, 18.0], &empty);
    f.form.show("empty", n == 0);
    f.label(
        "location",
        [20.0, 310.0, 520.0, 18.0],
        &if n > 0 {
            format!(
                "{} {}",
                tr("doc22.location", "Location:"),
                tr("doc20.aiLocation", "Built into BlueDoc")
            )
        } else {
            String::new()
        },
    );
    f.button(
        "ok",
        [436.0, 78.0, 104.0, 30.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [436.0, 116.0, 104.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    if which == 3 {
        f.button(
            "enable",
            [436.0, 154.0, 104.0, 30.0],
            &tr("doc22.enable", "Enable"),
            false,
        );
        f.form.enable("enable", false);
    }
    f
}

fn t(s: &Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

/// Round 22: the [MS-DOCX] keymap code of a shortcut named like "Ctrl+Shift+H" or
/// "Alt+F5": the virtual key in the low byte, 0x01 Shift / 0x02 Ctrl / 0x04 Alt above it.
pub(crate) fn key_code(name: &str) -> Option<u16> {
    let mut mods = 0u16;
    let mut key = None;
    for part in name.split('+') {
        match part {
            "Ctrl" => mods |= 0x0200,
            "Shift" => mods |= 0x0100,
            "Alt" => mods |= 0x0400,
            p if p.len() == 1 => {
                let c = p.chars().next()?.to_ascii_uppercase();
                key = c.is_ascii_alphanumeric().then_some(c as u16);
            }
            p => {
                let n: u16 = p.strip_prefix('F')?.parse().ok()?;
                key = (1..=12).contains(&n).then_some(0x70 + n - 1);
            }
        }
    }
    Some(mods | key?)
}

/// The name of a keymap code (see [`key_code`]), in the order Customize Keyboard writes.
pub(crate) fn key_label(code: u16) -> Option<String> {
    let vk = code & 0xFF;
    let key = match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from(vk as u8).to_string(),
        0x70..=0x7B => format!("F{}", vk - 0x70 + 1),
        _ => return None,
    };
    let mut s = String::new();
    for (bit, name) in [(0x0200, "Ctrl+"), (0x0400, "Alt+"), (0x0100, "Shift+")] {
        if code & bit != 0 {
            s.push_str(name);
        }
    }
    s.push_str(&key);
    Some(s)
}

/// Default caption of a new embedded workbook (the engine's default).
pub(crate) const DEFAULT_CAPTION: &str = "Worksheet";

/// Add Display as icon, the icon preview and Change Icon… to both pages of Insert › Object.
pub(crate) fn add_object_icon(f: &mut DocForm, s: &Strings) {
    let label = t(s, "doc22.displayAsIcon", "Display as icon");
    let change = t(s, "doc22.changeIcon", "Change Icon...");
    for (page, check, prev, button) in [
        (0, "asIcon", "iconPrev", "changeIcon"),
        (1, "asIcon1", "iconPrev1", "changeIcon1"),
    ] {
        f.check_on(page, check, [512.0, 228.0, 200.0, 22.0], &label, false);
        f.form.add(
            prev,
            Some(page),
            [530.0, 258.0, 180.0, 90.0],
            super::form::Kind::Canvas,
        );
        f.button_on(page, button, [567.0, 362.0, 112.0, 28.0], &change);
        f.form.show(prev, false);
        f.form.show(button, false);
    }
    // The caption and icon drawing chosen in Change Icon (kept out of sight).
    f.edit("iconCaption", [0.0, 0.0, 0.0, 0.0], "");
    f.form.show("iconCaption", false);
    f.edit("iconStyle", [0.0, 0.0, 0.0, 0.0], "0");
    f.form.show("iconStyle", false);
    f.edit("iconFile", [0.0, 0.0, 0.0, 0.0], "");
    f.form.show("iconFile", false);
    // The Result text "… as an icon." shown while Display as icon is on, as the reference.
    let texts = [
        t(
            s,
            "doc22.resultNewIcon",
            "Inserts a new BlueSheet Worksheet object into your document as an icon.",
        ),
        t(
            s,
            "doc22.resultFileIcon",
            "Inserts the contents of the workbook into your document as an icon.",
        ),
    ];
    for (page, ids) in [(0, ["rIcon0", "rIcon0b"]), (1, ["rIcon1", "rIcon1b"])] {
        let lines = wrap(&texts[page], 44);
        for (i, id) in ids.iter().enumerate() {
            let line = lines.get(i).cloned().unwrap_or_default();
            f.label_on(
                page,
                id,
                [134.0, 280.0 + 22.0 * i as f32, 350.0, 20.0],
                &line,
            );
            f.form.show(id, false);
        }
    }
}

/// Result lines of Insert › Object without the icon (page 0, page 1).
const RESULT_IDS: [&str; 7] = [
    "result", "resultB", "resultC", "result2", "result2B", "result2C", "result2D",
];

/// The Change Icon dialog: `c.extra["r22"]` = `{caption, style}`.
pub(crate) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    if kind == DocFormKind::AddinList {
        return open_addins(kind, s, c);
    }
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let caption = c.extra["r22"]["caption"]
        .as_str()
        .unwrap_or(DEFAULT_CAPTION)
        .to_owned();
    let style = c.extra["r22"]["style"].as_u64().unwrap_or(0);
    let file = c.extra["r22"]["file"].as_str().unwrap_or("").to_owned();
    let file_name = c.extra["r22"]["fileName"].as_str().unwrap_or("").to_owned();
    let mut f = DocForm::new(
        kind,
        &tr("doc22.changeIconTitle", "Change Icon"),
        (490.0, 318.0),
        vec![],
    );
    f.label(
        "fileL",
        [18.0, 53.0, 70.0, 18.0],
        &tr("doc22.fileName", "File name:"),
    );
    f.label(
        "file",
        [92.0, 53.0, 260.0, 18.0],
        &if file_name.is_empty() {
            tr("doc22.builtInIcons", "BlueOffice icons (built in)")
        } else {
            file_name
        },
    );
    // Browse…: an .ico, .exe / .dll (its icon resources are only read) or .png file.
    f.button(
        "browse",
        [360.0, 46.0, 110.0, 30.0],
        &tr("doc22.browse", "Browse..."),
        false,
    );
    f.edit("iconFile", [0.0, 0.0, 0.0, 0.0], &file);
    f.form.show("iconFile", false);
    f.label(
        "iconL",
        [18.0, 90.0, 60.0, 18.0],
        &tr("doc22.icon", "Icon:"),
    );
    f.form.add(
        "iconList",
        None,
        [90.0, 88.0, 110.0, 128.0],
        super::form::Kind::Canvas,
    );
    f.label(
        "captionL",
        [18.0, 233.0, 70.0, 18.0],
        &tr("doc22.caption", "Caption:"),
    );
    f.edit("caption", [90.0, 229.0, 382.0, 26.0], &caption);
    f.edit("iconStyle", [0.0, 0.0, 0.0, 0.0], &style.to_string());
    f.form.show("iconStyle", false);
    f.button(
        "ok",
        [232.0, 271.0, 111.0, 30.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [359.0, 271.0, 111.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.form.focus("caption");
    f
}

/// Height of one icon row of the Change Icon list (reference px).
const ICON_ROW: f32 = 46.0;

/// Draw BlueOffice's workbook icon (`style` 0 cells, 1 chart) in a `size`-px square at
/// (`x`, `y`) — the drawing of `bo_engine`'s `ole_icon`.
pub(crate) fn paint_icon(p: &mut dyn crate::Painter, x: f32, y: f32, size: f32, style: u64) {
    let k = size / 32.0;
    let accent = Rgba(0x21, 0x73, 0x46, 255);
    let grey = Rgba(0x80, 0x80, 0x80, 255);
    let (l, t, r, b) = (x + 4.0 * k, y + 0.5 * k, x + 28.0 * k, y + 31.5 * k);
    let fold = 8.0 * k;
    p.path(
        &[
            PathSeg::M(l, t),
            PathSeg::L(r - fold, t),
            PathSeg::L(r, t + fold),
            PathSeg::L(r, b),
            PathSeg::L(l, b),
            PathSeg::Z,
        ],
        Some(Rgba(255, 255, 255, 255)),
        Some((grey, k.max(1.0))),
    );
    p.path(
        &[
            PathSeg::M(r - fold, t),
            PathSeg::L(r - fold, t + fold),
            PathSeg::L(r, t + fold),
        ],
        None,
        Some((grey, k.max(1.0))),
    );
    if style % 2 == 1 {
        let base = b - 5.0 * k;
        let cw = (r - l - 8.0 * k) / 4.0;
        for (n, frac) in [0.45_f32, 0.8, 0.6].into_iter().enumerate() {
            let h = (base - t - 10.0 * k) * frac;
            p.fill_rect(l + 4.0 * k + cw * n as f32 * 1.3, base - h, cw, h, accent);
        }
        p.fill_rect(l + 3.0 * k, base, r - l - 6.0 * k, k.max(1.0), grey);
    } else {
        let (gx, gy, cw, ch) = (l + 3.0 * k, t + 11.0 * k, (r - l - 6.0 * k) / 3.0, 4.0 * k);
        p.fill_rect(gx, gy, cw * 3.0, ch, accent);
        let lw = (0.75 * k).max(1.0);
        for i in 0..=4 {
            p.fill_rect(gx, gy + ch * i as f32, cw * 3.0, lw, accent);
        }
        for j in 0..=3 {
            p.fill_rect(gx + cw * j as f32, gy, lw, ch * 4.0, accent);
        }
    }
}

/// The icon of a picked file, before it is read by the engine: a page with a blue corner
/// (own drawing; the file's own picture is put in the object when it is inserted).
fn paint_file_icon(p: &mut dyn crate::Painter, x: f32, y: f32, size: f32, font: &crate::Font<'_>) {
    let k = size / 32.0;
    p.fill_rect(
        x + 4.0 * k,
        y,
        24.0 * k,
        31.0 * k,
        Rgba(0x80, 0x80, 0x80, 255),
    );
    p.fill_rect(
        x + 5.0 * k,
        y + k,
        22.0 * k,
        29.0 * k,
        Rgba(255, 255, 255, 255),
    );
    p.fill_rect(
        x + 8.0 * k,
        y + 8.0 * k,
        16.0 * k,
        16.0 * k,
        Rgba(0x0F, 0x6C, 0xBD, 255),
    );
    let small = crate::Font {
        size: font.size * 0.8,
        ..*font
    };
    p.text_font(
        "+",
        x + 13.0 * k,
        y + 9.0 * k,
        &small,
        Rgba(255, 255, 255, 255),
    );
}

/// Paint the icon preview of Insert › Object and the icon list of Change Icon.
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn crate::Painter,
    font: &crate::Font<'_>,
) -> bool {
    match (f.kind, id) {
        (DocFormKind::InsertObject, "iconPrev" | "iconPrev1") => {
            let k = r[3] / 90.0;
            let style = f.text("iconStyle").parse().unwrap_or(0);
            let size = 36.0 * k;
            if style == 2 {
                paint_file_icon(p, r[0] + (r[2] - size) / 2.0, r[1] + 4.0 * k, size, font);
            } else {
                paint_icon(p, r[0] + (r[2] - size) / 2.0, r[1] + 4.0 * k, size, style);
            }
            let mut caption = f.text("iconCaption");
            if caption.is_empty() {
                caption = object_caption(f);
            }
            // The caption centred under the icon, two lines at most (estimated widths).
            let per_line = ((r[2] / (font.size * 0.5)) as usize).max(8);
            let lines = wrap(&caption, per_line);
            for (i, line) in lines.iter().take(2).enumerate() {
                let w = line.chars().count() as f32 * font.size * 0.5;
                p.text_font(
                    line,
                    r[0] + (r[2] - w).max(0.0) / 2.0,
                    r[1] + 46.0 * k + i as f32 * font.size * 1.3,
                    font,
                    Rgba(0x1a, 0x1a, 0x1a, 255),
                );
            }
            true
        }
        (DocFormKind::ChangeIcon, "iconList") => {
            let k = r[3] / 128.0;
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x7a, 0x7a, 0x7a, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(255, 255, 255, 255),
            );
            let sel: u64 = f.text("iconStyle").parse().unwrap_or(0);
            let rows = if f.text("iconFile").is_empty() { 2 } else { 3 };
            for i in 0..rows {
                let y = r[1] + 2.0 * k + ICON_ROW * k * i as f32;
                if i == sel {
                    p.fill_rect(
                        r[0] + 2.0 * k,
                        y,
                        r[2] - 4.0 * k,
                        ICON_ROW * k - 2.0 * k,
                        Rgba(0x00, 0x78, 0xd4, 255),
                    );
                }
                let size = 36.0 * k;
                if i == 2 {
                    paint_file_icon(p, r[0] + (r[2] - size) / 2.0, y + 4.0 * k, size, font);
                } else {
                    paint_icon(p, r[0] + (r[2] - size) / 2.0, y + 4.0 * k, size, i);
                }
            }
            true
        }
        _ => false,
    }
}

/// `text` in lines of at most `n` characters, broken at spaces.
fn wrap(text: &str, n: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.split_whitespace() {
        match out.last_mut() {
            Some(l) if l.chars().count() + 1 + w.chars().count() <= n => {
                l.push(' ');
                l.push_str(w);
            }
            _ => out.push(w.to_owned()),
        }
    }
    out
}

/// Caption an icon gets when none was typed: the picked file's name, else "Worksheet".
fn object_caption(f: &DocForm) -> String {
    if f.form.tab == 1 {
        if let Some((name, _)) = &f.files[0] {
            return name.rsplit(['/', '\\']).next().unwrap_or(name).to_owned();
        }
    }
    DEFAULT_CAPTION.to_owned()
}

impl DocForm {
    /// Display as icon toggled on page 0 or 1: show the preview and Change Icon….
    pub(super) fn r22_icon_toggled(&mut self, id: &str) {
        let on = self.on(id);
        for rid in RESULT_IDS {
            self.form.show(rid, !on);
        }
        for rid in ["rIcon0", "rIcon0b", "rIcon1", "rIcon1b"] {
            self.form.show(rid, on);
        }
        let (other, prev, prev1) = if id == "asIcon" {
            ("asIcon1", "iconPrev", "changeIcon")
        } else {
            ("asIcon", "iconPrev1", "changeIcon1")
        };
        self.form.set_on(other, on);
        for (p, b) in [
            (prev, prev1),
            ("iconPrev", "changeIcon"),
            ("iconPrev1", "changeIcon1"),
        ] {
            self.form.show(p, on);
            self.form.show(b, on);
        }
    }

    /// Fields Insert › Object adds to `oleInsert` when Display as icon is on.
    pub(super) fn r22_icon_fields(&self, cmd: &mut Value) {
        if self.kind != DocFormKind::InsertObject || !self.on("asIcon") {
            return;
        }
        let mut caption = self.text("iconCaption");
        if caption.trim().is_empty() {
            caption = object_caption(self);
        }
        cmd["icon"] = json!(true);
        cmd["caption"] = json!(caption);
        let style = self.text("iconStyle").parse::<u64>().unwrap_or(0);
        if style == 2 && !self.text("iconFile").is_empty() {
            cmd["iconFile"] = json!(self.text("iconFile"));
            cmd["iconStyle"] = json!(0);
        } else {
            cmd["iconStyle"] = json!(style.min(1));
        }
    }

    /// The (caption, icon drawing, icon file) Change Icon starts from.
    pub(crate) fn r22_icon_state(&self) -> (String, u64, String) {
        let mut caption = self.text("iconCaption");
        if caption.trim().is_empty() {
            caption = object_caption(self);
        }
        (
            caption,
            self.text("iconStyle").parse().unwrap_or(0),
            self.text("iconFile"),
        )
    }

    /// Change Icon › Browse… read a file: its icon becomes the third choice, chosen.
    pub(crate) fn r22_set_icon_file(&mut self, name: &str, b64: &str) {
        if self.kind != DocFormKind::ChangeIcon {
            return;
        }
        self.set_label("file", name);
        self.set_text("iconFile", b64);
        self.set_text("iconStyle", "2");
    }

    /// Change Icon answered: keep its caption and icon in the Object dialog.
    pub(crate) fn r22_set_icon(&mut self, style: u64, caption: &str, file: &str) {
        self.set_text("iconCaption", caption);
        self.set_text("iconStyle", &style.to_string());
        self.set_text("iconFile", file);
    }

    /// Buttons of round-22 dialogs; `None` when not handled here.
    pub(super) fn r22_clicked(&mut self, id: &str) -> Option<FormOut> {
        match (self.kind, id) {
            (DocFormKind::ChangeIcon, "browse") => Some(FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "r17.pick:changeIcon" })],
                false,
            )),
            (DocFormKind::InsertObject, "changeIcon" | "changeIcon1") => Some(FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "r22.changeIcon" })],
                false,
            )),
            _ => None,
        }
    }

    /// A press in a round-22 canvas (the icon list); `true` when handled.
    pub(super) fn r22_canvas(&mut self, id: &str, _x: f32, y: f32) -> bool {
        if self.kind != DocFormKind::ChangeIcon || id != "iconList" {
            return false;
        }
        let row = (y / ICON_ROW).floor();
        let rows = if self.text("iconFile").is_empty() {
            2.0
        } else {
            3.0
        };
        if (0.0..rows).contains(&row) {
            self.set_text("iconStyle", &(row as u64).to_string());
        }
        true
    }

    /// OK of Change Icon: back to Insert › Object with the caption and icon.
    pub(super) fn r22_ok(&mut self) -> Option<FormOut> {
        if self.kind == DocFormKind::AddinList {
            return Some(FormOut::Close);
        }
        if self.kind != DocFormKind::ChangeIcon {
            return None;
        }
        let style: u64 = self.text("iconStyle").parse().unwrap_or(0);
        let caption = self.text("caption");
        let file = if style == 2 {
            self.text("iconFile")
        } else {
            String::new()
        };
        Some(FormOut::Run(
            vec![json!({
                "cmd": "ui.action",
                "id": format!("r22.iconSet:{style}:{caption}\u{1f}{file}")
            })],
            true,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{key_code, key_label};

    #[test]
    fn shortcut_names_and_keymap_codes() {
        assert_eq!(key_code("Ctrl+Shift+H"), Some(0x0348));
        assert_eq!(key_code("Alt+F5"), Some(0x0474));
        assert_eq!(key_code("Ctrl+1"), Some(0x0231));
        assert_eq!(key_label(0x0348).as_deref(), Some("Ctrl+Shift+H"));
        assert_eq!(key_label(0x0474).as_deref(), Some("Alt+F5"));
        assert_eq!(key_code("Ctrl+Home"), None);
    }
}
