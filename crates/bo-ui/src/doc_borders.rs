//! Design › Page Borders dialog of BlueDoc (ECMA-376 Part 1, §17.6.10 pgBorders): setting
//! (none / box / shadow), line style, colour, width and the sections it applies to, with a
//! preview page, after the Page Border tab of a desktop word processor's Borders and
//! Shading dialog. Carried out by the engine's `setPageBorders`.
use crate::theme::Theme;
use crate::{Key, Mods, Painter, TextMeasure, TextStyle};
use bo_render::Rgba;
use serde_json::{json, Value};

const FACE: Rgba = Rgba(240, 240, 240, 255);
const CAPTION: Rgba = Rgba(243, 243, 243, 255);
const FRAME: Rgba = Rgba(170, 170, 170, 255);
const EDGE: Rgba = Rgba(171, 173, 179, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const GREY: Rgba = Rgba(160, 160, 160, 255);
const PAGE_EDGE: Rgba = Rgba(229, 229, 229, 255);
const PAGE: Rgba = Rgba(249, 249, 249, 255);
const TAB_OFF: Rgba = Rgba(243, 243, 243, 255);
const SELECT: Rgba = Rgba(0, 120, 212, 255);
const BUTTON: Rgba = Rgba(253, 253, 253, 255);
const BUTTON_EDGE: Rgba = Rgba(208, 208, 208, 255);

const W: f32 = 560.0;
const H: f32 = 430.0;
/// Content below the tab strip.
const DY: f32 = 36.0;

/// Line styles (§17.18.2 ST_Border) and their labels.
const STYLES: [(&str, &str, &str); 5] = [
    ("single", "docBorders.single", "Single"),
    ("dotted", "docBorders.dotted", "Dotted"),
    ("dashed", "docBorders.dashed", "Dashed"),
    ("double", "docBorders.double", "Double"),
    ("thick", "docBorders.thick", "Thick"),
];
/// Colours: automatic then seven of BlueOffice's palette.
const COLORS: [Option<&str>; 8] = [
    None,
    Some("1F6FB2"),
    Some("2BA3A0"),
    Some("3C7D3A"),
    Some("C9A227"),
    Some("D9603B"),
    Some("A23B5C"),
    Some("7F7F7F"),
];
/// Widths in eighths of a point (½ … 6 pt).
const WIDTHS: [u32; 7] = [4, 6, 8, 12, 18, 24, 36];

/// Labels: key, en-US fallback.
const LABELS: [(&str, &str); 21] = [
    ("docBorders.title", "Borders and Shading"),
    ("docBorders.setting", "Setting:"),
    ("docBorders.none", "None"),
    ("docBorders.box", "Box"),
    ("docBorders.shadow", "Shadow"),
    ("docBorders.style", "Style:"),
    ("docBorders.color", "Color:"),
    ("docBorders.width", "Width:"),
    ("docBorders.preview", "Preview"),
    ("docBorders.applyTo", "Apply to:"),
    ("docBorders.document", "Whole document"),
    ("docBorders.section", "This section"),
    ("findDlg.cancel", "Cancel"),
    ("common.ok", "OK"),
    ("docBorders.threeD", "3-D"),
    ("docBorders.custom", "Custom"),
    ("docBorders.tabBorders", "Borders"),
    ("docBorders.tabPage", "Page Border"),
    ("docBorders.tabShading", "Shading"),
    ("docBorders.art", "Art:"),
    ("docBorders.options", "Options…"),
];

/// What a press asks the window to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BordersAction {
    None,
    Redraw,
    Close,
    /// Run this engine command, then close.
    Apply(Value),
}

/// The open dialog.
pub(crate) struct BordersDialog {
    /// 0 none, 1 box, 2 shadow.
    pub(crate) setting: usize,
    pub(crate) style: usize,
    pub(crate) color: usize,
    pub(crate) width: usize,
    /// Whole document (else the caret's section).
    pub(crate) document: bool,
    scale: f32,
    x0: f32,
    y0: f32,
    labels: Vec<String>,
    styles: Vec<String>,
    /// Measured widths of the OK and Cancel labels.
    button_w: [f32; 2],
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

impl BordersDialog {
    pub(crate) fn new(has_borders: bool) -> Self {
        Self {
            setting: usize::from(has_borders),
            style: 0,
            color: 0,
            width: 1,
            document: true,
            scale: 1.0,
            x0: 0.0,
            y0: 0.0,
            labels: Vec::new(),
            styles: Vec::new(),
            button_w: [0.0; 2],
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        [
            self.x0 + self.px(x),
            self.y0 + self.px(y),
            self.px(w),
            self.px(h),
        ]
    }

    fn style(&self) -> TextStyle {
        TextStyle {
            size: self.px(10.0),
            bold: false,
        }
    }

    pub(crate) fn layout(
        &mut self,
        m: &dyn TextMeasure,
        strings: &crate::strings::Strings,
        size: (f32, f32),
        scale: f32,
    ) {
        self.scale = scale;
        self.x0 = ((size.0 - self.px(W)) / 2.0).round().max(0.0);
        self.y0 = (size.1 * 0.16).round().max(0.0);
        let get = |k: &str, f: &str| {
            let v = strings.get(k);
            if v == k {
                f.to_owned()
            } else {
                v.to_owned()
            }
        };
        self.labels = LABELS.iter().map(|(k, f)| get(k, f)).collect();
        self.styles = STYLES.iter().map(|(_, k, f)| get(k, f)).collect();
        let st = self.style();
        self.button_w = [
            m.measure(self.label(13), st).0,
            m.measure(self.label(12), st).0,
        ];
    }

    fn label(&self, i: usize) -> &str {
        self.labels.get(i).map_or(LABELS[i].1, String::as_str)
    }

    fn setting_rect(&self, i: usize) -> [f32; 4] {
        self.r(16.0, DY + 70.0 + i as f32 * 46.0, 130.0, 42.0)
    }

    fn style_rect(&self, i: usize) -> [f32; 4] {
        self.r(166.0, DY + 70.0 + i as f32 * 26.0, 170.0, 24.0)
    }

    fn color_rect(&self, i: usize) -> [f32; 4] {
        self.r(166.0 + i as f32 * 21.0, DY + 228.0, 18.0, 18.0)
    }

    fn width_rect(&self, i: usize) -> [f32; 4] {
        self.r(166.0 + i as f32 * 24.0, DY + 276.0, 22.0, 20.0)
    }

    fn apply_rect(&self, i: usize) -> [f32; 4] {
        self.r(360.0, DY + 270.0 + i as f32 * 22.0, 180.0, 20.0)
    }

    fn button_rect(&self, i: usize) -> [f32; 4] {
        self.r(
            W - 2.0 * (85.6 + 10.0) - 6.0 + i as f32 * 95.6,
            H - 38.0,
            85.6,
            23.2,
        )
    }

    /// The engine command for the choices (`setPageBorders`).
    pub(crate) fn command(&self) -> Value {
        let scope = if self.document { "document" } else { "section" };
        if self.setting == 0 {
            return json!({ "cmd": "setPageBorders", "borders": null, "scope": scope });
        }
        let (style, size) = if STYLES[self.style].0 == "thick" {
            ("thick", WIDTHS[self.width].max(12))
        } else {
            (STYLES[self.style].0, WIDTHS[self.width])
        };
        json!({
            "cmd": "setPageBorders",
            "borders": {
                "style": style,
                "size": size,
                "color": COLORS[self.color],
                "space": 24,
                "shadow": self.setting == 2,
            },
            "scope": scope,
        })
    }

    pub(crate) fn press(&mut self, x: f32, y: f32) -> BordersAction {
        for i in 0..3 {
            if inside(self.setting_rect(i), x, y) {
                self.setting = i;
                return BordersAction::Redraw;
            }
        }
        for i in 0..STYLES.len() {
            if inside(self.style_rect(i), x, y) {
                self.style = i;
                self.setting = self.setting.max(1);
                return BordersAction::Redraw;
            }
        }
        for i in 0..COLORS.len() {
            if inside(self.color_rect(i), x, y) {
                self.color = i;
                self.setting = self.setting.max(1);
                return BordersAction::Redraw;
            }
        }
        for i in 0..WIDTHS.len() {
            if inside(self.width_rect(i), x, y) {
                self.width = i;
                self.setting = self.setting.max(1);
                return BordersAction::Redraw;
            }
        }
        for i in 0..2 {
            if inside(self.apply_rect(i), x, y) {
                self.document = i == 0;
                return BordersAction::Redraw;
            }
        }
        if inside(self.button_rect(0), x, y) {
            return BordersAction::Apply(self.command());
        }
        if inside(self.button_rect(1), x, y) || inside(self.r(W - 46.0, 0.0, 46.0, 30.0), x, y) {
            return BordersAction::Close;
        }
        BordersAction::Redraw
    }

    pub(crate) fn key(&mut self, key: Key, _m: Mods) -> BordersAction {
        match key {
            Key::Escape => BordersAction::Close,
            Key::Enter => BordersAction::Apply(self.command()),
            _ => BordersAction::None,
        }
    }

    fn line(&self, p: &mut dyn Painter, x: f32, y: f32, w: f32, style: usize, color: Rgba) {
        let t = self.scale.max(1.0).round();
        match STYLES[style].0 {
            "dotted" => {
                let mut i = 0.0;
                while i < w {
                    p.fill_rect(x + i, y, t, t, color);
                    i += 3.0 * t;
                }
            }
            "dashed" => {
                let mut i = 0.0;
                while i < w {
                    p.fill_rect(x + i, y, (5.0 * t).min(w - i), t, color);
                    i += 8.0 * t;
                }
            }
            "double" => {
                p.fill_rect(x, y - t, w, t, color);
                p.fill_rect(x, y + t, w, t, color);
            }
            "thick" => p.fill_rect(x, y - t, w, 3.0 * t, color),
            _ => p.fill_rect(x, y, w, t, color),
        }
    }

    fn color(&self) -> Rgba {
        COLORS[self.color].map_or(INK, |h| {
            let v = u32::from_str_radix(h, 16).unwrap_or(0);
            Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
        })
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme) {
        let [x, y, w, h] = self.r(0.0, 0.0, W, H);
        for (k, a) in [(6.0, 12u8), (3.0, 20), (1.0, 28)] {
            let o = self.px(k);
            p.fill_rect(
                x - o,
                y - o + self.px(3.0),
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, FRAME);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, FACE);
        let cap = self.px(30.0);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, cap - 1.0, CAPTION);
        let st = self.style();
        let ty = |top: f32, hh: f32| (top + (hh - st.size * 1.33) / 2.0).round();
        p.text(self.label(0), x + self.px(10.0), ty(y, cap), st, INK);
        let c = self.r(W - 46.0, 0.0, 46.0, 30.0);
        p.text("\u{2715}", c[0] + self.px(17.0), ty(c[1], c[3]), st, INK);
        // Tabs: Borders, Page Border (shown), Shading; the others are not available yet.
        let mut tx = x + self.px(9.0);
        for (k, label) in [16, 17, 18].into_iter().enumerate() {
            let tw = self.px(96.0);
            let on = k == 1;
            let top = if on { 38.0 } else { 40.0 };
            let r = self.r(0.0, top, 0.0, 60.0 - top);
            p.fill_rect(tx, r[1], tw, r[3] + 1.0, PAGE_EDGE);
            p.fill_rect(
                tx + 1.0,
                r[1] + 1.0,
                tw - 2.0,
                r[3] - 1.0 + if on { 1.0 } else { 0.0 },
                if on { PAGE } else { TAB_OFF },
            );
            p.text(
                self.label(label),
                tx + self.px(12.0),
                ty(r[1], r[3]),
                st,
                if on { INK } else { GREY },
            );
            tx += tw;
        }
        let pg = self.r(7.0, 60.0, W - 14.0, H - 60.0 - 50.0);
        p.fill_rect(pg[0], pg[1], pg[2], 1.0, PAGE_EDGE);
        // Art ▾ and Options… (not available yet).
        p.text(
            self.label(19),
            x + self.px(166.0),
            y + self.px(DY + 306.0),
            st,
            GREY,
        );
        let art = self.r(166.0, DY + 324.0, 170.0, 20.8);
        p.fill_rect(art[0], art[1], art[2], art[3], Rgba(204, 204, 204, 255));
        p.fill_rect(
            art[0] + 1.0,
            art[1] + 1.0,
            art[2] - 2.0,
            art[3] - 2.0,
            Rgba(245, 245, 245, 255),
        );
        let opt = self.r(440.0, DY + 324.0, 100.0, 23.2);
        p.fill_rect(opt[0], opt[1], opt[2], opt[3], BUTTON_EDGE);
        p.fill_rect(
            opt[0] + 1.0,
            opt[1] + 1.0,
            opt[2] - 2.0,
            opt[3] - 2.0,
            BUTTON,
        );
        p.text(
            self.label(20),
            opt[0] + self.px(18.0),
            ty(opt[1], opt[3]),
            st,
            GREY,
        );
        // Setting.
        p.text(
            self.label(1),
            x + self.px(16.0),
            y + self.px(DY + 46.0),
            st,
            INK,
        );
        for i in 0..5 {
            let r = self.setting_rect(i);
            if self.setting == i {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(204, 228, 247, 255));
            }
            // A small page with the border (own drawing).
            let pg = [
                r[0] + self.px(6.0),
                r[1] + self.px(6.0),
                self.px(24.0),
                self.px(30.0),
            ];
            p.fill_rect(pg[0], pg[1], pg[2], pg[3], WHITE);
            if (1..3).contains(&i) {
                let e = self.scale.max(1.0).round();
                if i == 2 {
                    p.fill_rect(
                        pg[0] + 3.0 * e,
                        pg[1] + 3.0 * e,
                        pg[2],
                        pg[3],
                        Rgba(120, 120, 120, 255),
                    );
                    p.fill_rect(pg[0], pg[1], pg[2], pg[3], WHITE);
                }
                p.fill_rect(pg[0], pg[1], pg[2], e, INK);
                p.fill_rect(pg[0], pg[1] + pg[3] - e, pg[2], e, INK);
                p.fill_rect(pg[0], pg[1], e, pg[3], INK);
                p.fill_rect(pg[0] + pg[2] - e, pg[1], e, pg[3], INK);
            }
            let name = if i < 3 {
                self.label(2 + i)
            } else {
                self.label(11 + i)
            };
            p.text(
                name,
                r[0] + self.px(46.0),
                ty(r[1], r[3]),
                st,
                if i < 3 { INK } else { GREY },
            );
        }
        // Style list, colours, widths.
        p.text(
            self.label(5),
            x + self.px(166.0),
            y + self.px(DY + 46.0),
            st,
            INK,
        );
        let lr = self.r(164.0, DY + 68.0, 174.0, 136.0);
        p.fill_rect(lr[0], lr[1], lr[2], lr[3], EDGE);
        p.fill_rect(lr[0] + 1.0, lr[1] + 1.0, lr[2] - 2.0, lr[3] - 2.0, WHITE);
        for i in 0..STYLES.len() {
            let r = self.style_rect(i);
            let sel = i == self.style;
            if sel {
                p.fill_rect(r[0], r[1], r[2], r[3], SELECT);
            }
            let ink = if sel { WHITE } else { INK };
            self.line(
                p,
                r[0] + self.px(8.0),
                r[1] + r[3] / 2.0,
                self.px(60.0),
                i,
                ink,
            );
            if let Some(name) = self.styles.get(i) {
                p.text(name, r[0] + self.px(78.0), ty(r[1], r[3]), st, ink);
            }
        }
        p.text(
            self.label(6),
            x + self.px(166.0),
            y + self.px(DY + 208.0),
            st,
            INK,
        );
        for (i, color) in COLORS.iter().enumerate() {
            let r = self.color_rect(i);
            p.fill_rect(
                r[0] - 1.0,
                r[1] - 1.0,
                r[2] + 2.0,
                r[3] + 2.0,
                if i == self.color { SELECT } else { EDGE },
            );
            let fill = color.map_or(WHITE, |h| {
                let v = u32::from_str_radix(h, 16).unwrap_or(0);
                Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
            });
            p.fill_rect(r[0], r[1], r[2], r[3], fill);
            if color.is_none() {
                p.fill_rect(
                    r[0] + self.px(4.0),
                    r[1] + self.px(4.0),
                    r[2] - self.px(8.0),
                    r[3] - self.px(8.0),
                    INK,
                );
            }
        }
        p.text(
            self.label(7),
            x + self.px(166.0),
            y + self.px(DY + 256.0),
            st,
            INK,
        );
        for (i, eighths) in WIDTHS.iter().enumerate() {
            let r = self.width_rect(i);
            let sel = i == self.width;
            p.fill_rect(r[0], r[1], r[2], r[3], if sel { SELECT } else { EDGE });
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
            let lw = (*eighths as f32 / 8.0 * self.scale).max(1.0).round();
            p.fill_rect(
                r[0] + self.px(3.0),
                r[1] + (r[3] - lw) / 2.0,
                r[2] - self.px(6.0),
                lw,
                INK,
            );
        }
        // Preview page and Apply to.
        p.text(
            self.label(8),
            x + self.px(360.0),
            y + self.px(DY + 46.0),
            st,
            INK,
        );
        let pv = self.r(380.0, DY + 70.0, 120.0, 160.0);
        p.fill_rect(pv[0], pv[1], pv[2], pv[3], WHITE);
        if self.setting > 0 {
            let ink = self.color();
            let m = self.px(10.0);
            let (l, tp, wd, ht) = (pv[0] + m, pv[1] + m, pv[2] - 2.0 * m, pv[3] - 2.0 * m);
            if self.setting == 2 {
                p.fill_rect(
                    l + self.px(3.0),
                    tp + ht,
                    wd,
                    self.px(3.0),
                    Rgba(110, 110, 110, 255),
                );
                p.fill_rect(
                    l + wd,
                    tp + self.px(3.0),
                    self.px(3.0),
                    ht,
                    Rgba(110, 110, 110, 255),
                );
            }
            self.line(p, l, tp, wd, self.style, ink);
            self.line(p, l, tp + ht, wd, self.style, ink);
            let e = self.scale.max(1.0).round();
            for k in 0..(ht as usize) {
                let yy = tp + k as f32;
                let on = match STYLES[self.style].0 {
                    "dotted" => k % 3 == 0,
                    "dashed" => k % 8 < 5,
                    _ => true,
                };
                if on {
                    p.fill_rect(l, yy, e, 1.0, ink);
                    p.fill_rect(l + wd, yy, e, 1.0, ink);
                }
            }
        }
        for k in 0..6 {
            let ly = pv[1] + self.px(26.0) + k as f32 * self.px(9.0);
            p.fill_rect(
                pv[0] + self.px(20.0),
                ly,
                pv[2] - self.px(40.0),
                self.px(3.0),
                Rgba(200, 200, 200, 255),
            );
        }
        p.text(
            self.label(9),
            x + self.px(360.0),
            y + self.px(DY + 246.0),
            st,
            INK,
        );
        for i in 0..2 {
            let r = self.apply_rect(i);
            let on = self.document == (i == 0);
            let d = self.px(12.0);
            let (cx, cy) = (r[0] + d / 2.0, r[1] + r[3] / 2.0);
            p.fill_rect(cx - d / 2.0, cy - d / 2.0, d, d, EDGE);
            p.fill_rect(
                cx - d / 2.0 + 1.0,
                cy - d / 2.0 + 1.0,
                d - 2.0,
                d - 2.0,
                WHITE,
            );
            if on {
                let k = self.px(3.0);
                p.fill_rect(cx - k, cy - k, 2.0 * k, 2.0 * k, SELECT);
            }
            p.text(
                self.label(10 + i),
                r[0] + self.px(18.0),
                ty(r[1], r[3]),
                st,
                INK,
            );
        }
        for i in 0..2 {
            let r = self.button_rect(i);
            p.fill_rect(
                r[0],
                r[1],
                r[2],
                r[3],
                if i == 0 { SELECT } else { BUTTON_EDGE },
            );
            let inset = if i == 0 { 2.0 } else { 1.0 };
            p.fill_rect(
                r[0] + inset,
                r[1] + inset,
                r[2] - 2.0 * inset,
                r[3] - 2.0 * inset,
                BUTTON,
            );
            let label = if i == 0 {
                self.label(13)
            } else {
                self.label(12)
            };
            let lw = self.button_w[i];
            p.text(label, r[0] + (r[2] - lw) / 2.0, ty(r[1], r[3]), st, t.text);
        }
    }
}
