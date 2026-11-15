//! Controls BlueSlide draws over its ribbon slots (N6, round 3), laid out like the reference
//! presentation app's Transitions tab at 125 %: the Timing group's two columns (Sound
//! drop-down, Duration spin box, Apply To All; Advance Slide with On Mouse Click and After)
//! and Effect Options with its two-line label, dimmed when the effect has no variants.
use super::{MenuKind, SlideWindow};
use crate::icons::IconColors;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::spin_box::{self as sb, BoxLook, SpinEdit, SpinHit, SpinKey};
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_engine::slide_edit::show::TransitionSpec;
use bo_engine::slide_edit::SlideCommand;
use bo_render::Rgba;

/// What a slot shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Icon, label and a drop-down box.
    Combo,
    /// Icon, label and a spin box.
    Spin,
    /// Icon and label (a plain button).
    Button,
    /// A column heading.
    Header,
    /// Check box and label.
    Check,
    /// Check box, label and a spin box.
    CheckSpin,
}

/// The slots: command id, kind, icon. Box offsets (logical px from the slot's left edge)
/// measured on the reference: drop-down at 72 (113.6 wide), Duration at 92, After at 67.2
/// (80 wide), boxes 25.6 high; check boxes 14.4 square at 5.6.
const SLOTS: [(&str, Kind, Option<&str>); 12] = [
    ("anim.start", Kind::Combo, Some("anim.start")),
    ("anim.duration", Kind::Spin, Some("transition.clock")),
    ("anim.delay", Kind::Spin, Some("anim.delay")),
    ("anim.reorder", Kind::Header, None),
    ("anim.earlier", Kind::Button, Some("anim.earlier")),
    ("anim.later", Kind::Button, Some("anim.later")),
    ("transition.sound", Kind::Combo, Some("transition.sound")),
    ("transition.duration", Kind::Spin, Some("transition.clock")),
    (
        "transition.applyAll",
        Kind::Button,
        Some("transition.applyAll"),
    ),
    ("transition.advance", Kind::Header, None),
    ("transition.onClick", Kind::Check, None),
    ("transition.after", Kind::CheckSpin, None),
];

/// State of the drawn controls.
#[derive(Debug, Default)]
pub(super) struct Controls {
    /// A value being typed.
    pub edit: Option<SpinEdit>,
    /// Width of the typed text (caret position).
    caret_w: f32,
    /// Slot and part under the pointer.
    hover: Option<(&'static str, SpinHit)>,
    /// Measured label widths.
    label_w: Vec<(&'static str, f32)>,
    /// The After box is ticked on this slide (index) although no time is set yet.
    after_on: Option<usize>,
    /// Widths of Effect Options' two label lines.
    effect_w: (f32, f32),
}

/// Where a slot's parts are.
struct Parts {
    icon: Option<[f32; 4]>,
    label: (f32, f32),
    check: Option<[f32; 4]>,
    field: Option<[f32; 4]>,
}

const BG: Rgba = Rgba(0xf3, 0xf3, 0xf3, 255);
const SEPARATOR: Rgba = Rgba(0xd2, 0xd2, 0xd2, 255);
const DIM: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);
const HOVER: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);

impl SlideWindow {
    fn slot(&self, id: &'static str) -> Option<[f32; 4]> {
        self.ribbon.rect_of(RibbonHit::Command(id))
    }

    fn label_of(&self, id: &str) -> f32 {
        self.controls
            .label_w
            .iter()
            .find(|(k, _)| *k == id)
            .map_or(0.0, |(_, w)| *w)
    }

    /// Measure the slots' labels (after a relayout).
    pub(super) fn measure_controls(&mut self, text: &dyn TextMeasure) {
        let style = self.control_style();
        self.controls.label_w = SLOTS
            .iter()
            .map(|(id, _, _)| {
                let key = format!("commands.{id}");
                let label = self.strings.get(&key);
                (*id, text.measure(label, style).0)
            })
            .collect();
        let (a, b) = self.effect_lines();
        self.controls.effect_w = (text.measure(&a, style).0, text.measure(&b, style).0);
        if let Some(e) = &self.controls.edit {
            self.controls.caret_w = text.measure(&e.text, style).0;
        }
    }

    fn parts(&self, id: &'static str, kind: Kind, r: [f32; 4]) -> Parts {
        let px = |v: f32| (v * self.scale).round();
        let (x, y, h) = (r[0], r[1], r[3]);
        let bh = px(25.6);
        let by = (y + (h - bh) / 2.0).round();
        let icon = px(19.2);
        let label_w = self.label_of(id);
        let after_label = |at: f32| (x + at + label_w + px(6.4)).round();
        match kind {
            Kind::Combo | Kind::Spin | Kind::Button => {
                let lx = x + px(25.6);
                let field = match kind {
                    // Start's box sits closer to its shorter label (reference x 1126 from 1046).
                    Kind::Combo => {
                        let at = if id == "anim.start" { 64.0 } else { 72.0 };
                        Some([after_label(25.6).max(x + px(at)), by, px(113.6), bh])
                    }
                    Kind::Spin => Some([after_label(25.6).max(x + px(92.0)), by, px(80.0), bh]),
                    _ => None,
                };
                Parts {
                    icon: Some([x + px(2.4), (y + (h - icon) / 2.0).round(), icon, icon]),
                    label: (lx, label_w),
                    check: None,
                    field,
                }
            }
            Kind::Header => Parts {
                icon: None,
                label: (x + px(2.4), label_w),
                check: None,
                field: None,
            },
            Kind::Check | Kind::CheckSpin => {
                let c = px(14.4);
                Parts {
                    icon: None,
                    label: (x + px(27.2), label_w),
                    check: Some([x + px(5.6), (y + (h - c) / 2.0).round(), c, c]),
                    field: (kind == Kind::CheckSpin)
                        .then(|| [after_label(27.2).max(x + px(67.2)), by, px(80.0), bh]),
                }
            }
        }
    }

    /// Box of a drawn control (`[x, y, w, h]`), else its check box or its slot.
    #[cfg(test)]
    pub(super) fn control_rect(&self, id: &'static str) -> Option<[f32; 4]> {
        let (_, kind, _) = SLOTS.iter().find(|(s, _, _)| *s == id)?;
        let r = self.slot(id)?;
        let parts = self.parts(id, *kind, r);
        Some(parts.field.or(parts.check).unwrap_or(r))
    }

    /// Current transition timing: duration, on click, after (ms), sound label.
    fn timing(&self) -> (u32, bool, Option<u32>, String) {
        let t = self.view.state().and_then(|s| s.transition.clone());
        let sound = match t.as_ref().and_then(|t| t.sound.as_ref()) {
            Some(s) if s.stop => self.t("slideNative.stopSound", "[Stop Previous Sound]"),
            Some(s) if !s.name.is_empty() => s.name.clone(),
            _ => self.t("slideNative.noSound", "[No Sound]"),
        };
        match t {
            Some(t) => (t.dur_ms, t.advance_on_click, t.advance_after_ms, sound),
            None => (2000, true, None, sound),
        }
    }

    fn after_ticked(&self) -> bool {
        self.timing().2.is_some() || self.controls.after_on == Some(self.view.current())
    }

    fn set_timing(&mut self, spec: TransitionSpec) -> bool {
        self.view.run(SlideCommand::SetTransition {
            slides: None,
            all: false,
            transition: TransitionSpec {
                kind: "keep".into(),
                ..spec
            },
        })
    }

    /// The effect the Animations tab's timing shows.
    fn anim_target(&self) -> Option<bo_engine::slide_edit::show::AnimOut> {
        let i = self.anim_index()?;
        self.view.state()?.animations.get(i).cloned()
    }

    /// Change the timing of the Animations tab's effect.
    fn set_anim(&mut self, spec: bo_engine::slide_edit::show::AnimSpec) -> bool {
        match self.anim_index() {
            Some(index) => self.view.run(SlideCommand::SetAnimation { index, spec }),
            None => false,
        }
    }

    fn trigger_label(&self, trigger: &str) -> String {
        match trigger {
            "withPrevious" => self.t("slideNative.withPrevious", "With Previous"),
            "afterPrevious" => self.t("slideNative.afterPrevious", "After Previous"),
            _ => self.t("slideNative.onClick", "On Click"),
        }
    }

    /// Commit a typed or stepped value of spin box `id`.
    fn commit_spin(&mut self, id: &str, ms: u32) -> bool {
        use bo_engine::slide_edit::show::AnimSpec;
        match id {
            "anim.duration" => self.set_anim(AnimSpec {
                dur_ms: Some(ms.clamp(10, 59_000)),
                ..AnimSpec::default()
            }),
            "anim.delay" => self.set_anim(AnimSpec {
                delay_ms: Some(ms.min(3_600_000)),
                ..AnimSpec::default()
            }),
            "transition.duration" => self.set_timing(TransitionSpec {
                dur_ms: Some(ms.clamp(10, 59_000)),
                ..TransitionSpec::default()
            }),
            "transition.after" => {
                self.controls.after_on = Some(self.view.current());
                self.set_timing(TransitionSpec {
                    advance_after_ms: Some(ms.min(3_600_000)),
                    ..TransitionSpec::default()
                })
            }
            _ => false,
        }
    }

    fn spin_value(&self, id: &str) -> u32 {
        let (dur, _, after, _) = self.timing();
        let fx = self.anim_target();
        match id {
            "anim.duration" => fx.map_or(500, |a| a.dur_ms),
            "anim.delay" => fx.map_or(0, |a| a.delay_ms),
            "transition.duration" => dur,
            _ => after.unwrap_or(0),
        }
    }

    /// Step spin box `id` (Duration by a quarter second, After by a second).
    fn step_spin(&mut self, id: &str, up: bool) -> bool {
        let step = if matches!(id, "transition.duration" | "anim.duration" | "anim.delay") {
            250
        } else {
            1000
        };
        let v = self.spin_value(id);
        let v = if up { v + step } else { v.saturating_sub(step) };
        self.commit_spin(id, v)
    }

    fn finish_edit(&mut self, commit: bool) -> bool {
        let Some(e) = self.controls.edit.take() else {
            return false;
        };
        if commit {
            if let Some(ms) = sb::parse_ms(&e.text) {
                self.commit_spin(e.id, ms);
            }
        }
        true
    }

    /// A press on the ribbon: the drawn controls take it when it lands on one.
    pub(super) fn controls_press(&mut self, x: f32, y: f32) -> Option<bool> {
        let mut hit = None;
        for (id, kind, _) in SLOTS {
            let Some(r) = self.slot(id) else { continue };
            let parts = self.parts(id, kind, r);
            if let Some(f) = parts.field {
                if let Some(part) = sb::spin_hit(f, self.scale, x, y) {
                    hit = Some((id, kind, Some(part)));
                    break;
                }
            }
            if sb::inside(r, x, y) && kind != Kind::Button {
                hit = Some((id, kind, None));
                break;
            }
        }
        let editing = self.controls.edit.as_ref().map(|e| e.id);
        let Some((id, kind, part)) = hit else {
            // A press elsewhere commits what was typed and goes on as usual.
            self.finish_edit(true);
            return None;
        };
        if editing.is_some() && editing != Some(id) {
            self.finish_edit(true);
        }
        Some(match (kind, part) {
            (Kind::Combo, Some(_)) if id == "anim.start" => {
                self.open_start_menu(id);
                true
            }
            (Kind::Combo, Some(_)) => {
                self.open_sound_menu(id);
                true
            }
            (Kind::Spin | Kind::CheckSpin, Some(SpinHit::Up | SpinHit::Down)) => {
                self.finish_edit(false);
                self.step_spin(id, part == Some(SpinHit::Up))
            }
            (Kind::Spin | Kind::CheckSpin, Some(SpinHit::Field)) => {
                if self.controls.edit.is_none() {
                    self.controls.edit = Some(SpinEdit {
                        id,
                        text: String::new(),
                    });
                    self.controls.caret_w = 0.0;
                }
                true
            }
            (Kind::Check, _) => {
                let on = self.timing().1;
                self.set_timing(TransitionSpec {
                    advance_on_click: Some(!on),
                    ..TransitionSpec::default()
                })
            }
            (Kind::CheckSpin, None) => {
                if self.after_ticked() {
                    self.controls.after_on = None;
                    self.set_timing(TransitionSpec {
                        advance_after_ms: Some(0),
                        ..TransitionSpec::default()
                    });
                    true
                } else {
                    let v = self.spin_value(id);
                    self.commit_spin(id, v);
                    true
                }
            }
            _ => false,
        })
    }

    fn open_sound_menu(&mut self, id: &'static str) {
        let Some(r) = self.slot(id) else { return };
        let Some(f) = self.parts(id, Kind::Combo, r).field else {
            return;
        };
        let (_, _, _, cur) = self.timing();
        let items: Vec<MenuItem> = [
            ("slideNative.noSound", "[No Sound]", "snd:none"),
            ("slideNative.stopSound", "[Stop Previous Sound]", "snd:stop"),
        ]
        .iter()
        .map(|(key, fallback, c)| {
            let label = self.t(key, fallback);
            MenuItem {
                checked: label == cur,
                label,
                choice: Choice::Command((*c).to_owned()),
                dim: false,
            }
        })
        .collect();
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, f, self.size, self.scale));
    }

    fn open_start_menu(&mut self, id: &'static str) {
        let Some(cur) = self.anim_target().map(|a| a.trigger) else {
            return;
        };
        let Some(r) = self.slot(id) else { return };
        let Some(f) = self.parts(id, Kind::Combo, r).field else {
            return;
        };
        let items: Vec<MenuItem> = ["onClick", "withPrevious", "afterPrevious"]
            .iter()
            .map(|t| MenuItem {
                checked: *t == cur,
                label: self.trigger_label(t),
                choice: Choice::Command(format!("trig:{t}")),
                dim: false,
            })
            .collect();
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, f, self.size, self.scale));
    }

    /// A menu choice of the drawn controls (`snd:…`, `trig:…`).
    pub(super) fn choose_control(&mut self, kind: &str, v: &str) -> Option<bool> {
        match kind {
            "snd" => Some(self.set_timing(TransitionSpec {
                sound: Some(v.to_owned()),
                ..TransitionSpec::default()
            })),
            "trig" => Some(self.set_anim(bo_engine::slide_edit::show::AnimSpec {
                trigger: Some(v.to_owned()),
                ..Default::default()
            })),
            _ => None,
        }
    }

    /// Typing while a spin box has the keyboard.
    pub(super) fn controls_key(&mut self, input: &Input, text: &dyn TextMeasure) -> Option<bool> {
        let e = self.controls.edit.as_mut()?;
        let id = e.id;
        let r = match input {
            Input::Text(t) => {
                e.type_text(t);
                true
            }
            Input::Key(k, m) if !m.ctrl => match e.key(*k) {
                SpinKey::Edited => true,
                SpinKey::Commit => self.finish_edit(true),
                SpinKey::Cancel => self.finish_edit(false),
                SpinKey::Up | SpinKey::Down => {
                    let up = matches!(k, Key::Up);
                    self.controls.edit = None;
                    self.step_spin(id, up)
                }
                SpinKey::Ignored => return Some(false),
            },
            _ => return None,
        };
        if let Some(e) = &self.controls.edit {
            self.controls.caret_w = text.measure(&e.text, self.control_style()).0;
        }
        Some(r)
    }

    /// Pointer over the ribbon: hover on the drawn controls; `true` when it changed.
    pub(super) fn controls_hover(&mut self, x: f32, y: f32) -> bool {
        let mut hover = None;
        for (id, kind, _) in SLOTS {
            let Some(r) = self.slot(id) else { continue };
            if let Some(f) = self.parts(id, kind, r).field {
                if let Some(part) = sb::spin_hit(f, self.scale, x, y) {
                    hover = Some((id, part));
                }
            }
        }
        let changed = hover != self.controls.hover;
        self.controls.hover = hover;
        changed
    }

    /// Effect Options in two lines, as the reference writes it.
    fn effect_lines(&self) -> (String, String) {
        let label = self.strings.get("commands.transition.effect").to_owned();
        let words: Vec<&str> = label.split(' ').collect();
        if words.len() < 2 {
            return (label, "\u{25BE}".to_owned());
        }
        // Break at the space nearest the middle.
        let mid = label.chars().count() / 2;
        let mut best = (usize::MAX, 1);
        let mut pos = 0;
        for (k, w) in words.iter().enumerate().take(words.len() - 1) {
            pos += w.chars().count();
            let d = pos.abs_diff(mid);
            if d < best.0 {
                best = (d, k + 1);
            }
            pos += 1;
        }
        (
            words[..best.1].join(" "),
            format!("{} \u{25BE}", words[best.1..].join(" ")),
        )
    }

    /// Whether Effect Options has variants to offer (transition or selected effect).
    fn effect_options_enabled(&self, id: &str) -> bool {
        let Some(st) = self.view.state() else {
            return false;
        };
        if id == "transition.effect" {
            return st.transition.as_ref().is_some_and(|t| t.dirs.len() > 1);
        }
        let ids: Vec<u32> = st.selection.iter().map(|s| s.id).collect();
        st.animations
            .iter()
            .any(|a| ids.contains(&a.shape_id) && a.dirs.len() > 1)
    }

    /// Draw the controls over their ribbon slots.
    pub(super) fn paint_controls(&self, p: &mut dyn Painter) {
        let style = self.control_style();
        let ink = self.theme.text;
        let px = |v: f32| (v * self.scale).round();
        let colors = IconColors {
            ink,
            accent: self.theme.accent,
            on: self.theme.accent_on,
            tinted: true,
        };
        let ty = |r: [f32; 4]| (r[1] + (r[3] - style.size * 1.33) / 2.0 - px(2.4)).round();
        let (dur, on_click, after, sound) = self.timing();
        let fx = self.anim_target();
        let after_on = self.after_ticked();
        let hovered = self.ribbon.hovered();
        for (id, kind, icon) in SLOTS {
            let Some(r) = self.slot(id) else { continue };
            p.fill_rect(r[0], r[1], r[2] + px(2.0), r[3], BG);
            if kind == Kind::Button && hovered == Some(RibbonHit::Command(id)) {
                p.fill_rect(r[0], r[1], self.label_of(id) + px(30.4), r[3], HOVER);
            }
            let parts = self.parts(id, kind, r);
            if let (Some(i), Some(name)) = (parts.icon, icon) {
                self.icons.draw(p, name, i[0], i[1], i[2], colors);
            }
            let key = format!("commands.{id}");
            let label = self.strings.get(&key);
            let enabled = !id.starts_with("anim.") || id == "anim.reorder" || fx.is_some();
            let ink = if enabled { ink } else { DIM };
            p.text(label, parts.label.0, ty(r), style, ink);
            if let Some(c) = parts.check {
                let on = if kind == Kind::Check {
                    on_click
                } else {
                    after_on
                };
                sb::paint_check(p, c, on, true, ink);
            }
            let Some(f) = parts.field else { continue };
            let editing = self.controls.edit.as_ref().filter(|e| e.id == id);
            let value = match (editing, kind) {
                (Some(e), _) => e.text.clone(),
                (None, Kind::Combo) if id == "anim.start" => fx
                    .as_ref()
                    .map(|a| self.trigger_label(a.trigger))
                    .unwrap_or_default(),
                (None, Kind::Combo) => sound.clone(),
                (None, Kind::Spin) if id == "anim.duration" => fx
                    .as_ref()
                    .map(|a| sb::fmt_seconds(a.dur_ms))
                    .unwrap_or_default(),
                (None, Kind::Spin) if id == "anim.delay" => fx
                    .as_ref()
                    .map(|a| sb::fmt_seconds(a.delay_ms))
                    .unwrap_or_default(),
                (None, Kind::Spin) => sb::fmt_seconds(dur),
                (None, _) => sb::fmt_clock(after.unwrap_or(0)),
            };
            let look = BoxLook {
                text: &value,
                style,
                ink,
                enabled,
                caret: editing.map(|_| self.controls.caret_w),
                hover: self
                    .controls
                    .hover
                    .filter(|(h, _)| *h == id)
                    .map(|(_, part)| part),
            };
            if kind == Kind::Combo {
                sb::paint_combo(p, f, self.scale, &look);
            } else {
                sb::paint_spin(p, f, self.scale, &look);
            }
        }
        // The light line between the two columns (reference: x 1235, grey 210).
        for (top, bottom) in [
            ("transition.advance", "transition.after"),
            ("anim.reorder", "anim.later"),
        ] {
            if let (Some(a), Some(c)) = (self.slot(top), self.slot(bottom)) {
                p.fill_rect(
                    a[0] - px(2.4),
                    a[1] + px(5.6),
                    1.0,
                    c[1] + c[3] - a[1] - px(5.6),
                    SEPARATOR,
                );
            }
        }
        self.paint_effect_options(p, style);
    }

    /// Effect Options: a large picture over a two-line label, dimmed without variants.
    fn paint_effect_options(&self, p: &mut dyn Painter, style: TextStyle) {
        let px = |v: f32| (v * self.scale).round();
        let hovered = self.ribbon.hovered();
        for id in ["transition.effect", "anim.effect"] {
            let Some(r) = self.slot(id) else { continue };
            let enabled = self.effect_options_enabled(id);
            // Also over the ribbon's own label, wider than the slot.
            p.fill_rect(r[0] - px(2.4), r[1], r[2] + px(4.8), r[3], BG);
            // Reference: picture centred 42 px into the slot (it reaches into the gap).
            let cx = r[0] + px(34.0);
            if enabled && hovered == Some(RibbonHit::Command(id)) {
                p.fill_rect(cx - px(30.0), r[1], px(60.0), r[3], HOVER);
            }
            let size = px(38.4);
            self.icons.draw(
                p,
                id,
                (cx - size / 2.0).round(),
                r[1] + px(4.0),
                size,
                IconColors {
                    ink: if enabled { self.theme.text } else { DIM },
                    accent: self.theme.accent,
                    on: self.theme.accent_on,
                    tinted: enabled,
                },
            );
            let ink = if enabled { self.theme.text } else { DIM };
            let (a, b) = self.effect_lines();
            let (wa, wb) = self.controls.effect_w;
            p.text(&a, (cx - wa / 2.0).round(), r[1] + px(43.2), style, ink);
            p.text(&b, (cx - wb / 2.0).round(), r[1] + px(62.4), style, ink);
        }
    }
}
