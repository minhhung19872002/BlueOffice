//! BlueSlide's Animations tab, round 17 (N6): the gallery's drop-down and Add Animation open
//! the full gallery of effects by class — Entrance (green stars), Emphasis (amber), Exit
//! (red), Motion Paths (dotted paths from a green start to a red end) — as the reference
//! lays it out (own drawings), on the engine's effect catalog (`bo_pptx::anim_catalog`).
use super::SlideWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::slide_pickers::PickKind;
use crate::{Painter, TextStyle};
use bo_engine::slide_edit::show::AnimSpec;
use bo_engine::slide_edit::SlideCommand;
use bo_render::Rgba;

impl SlideWindow {
    /// Open the full effects gallery under ribbon command `anchor`; with `add`, a choice is
    /// added to the selection's effects (Add Animation), else it replaces them.
    pub(super) fn open_effects(&mut self, anchor: &'static str, add: bool) {
        let anchor = if add { "anim.add" } else { anchor };
        self.open_picker(PickKind::Effects, anchor);
    }

    /// More … Effects of a class (`entr`, `emph`, `exit`, `path`) from the gallery.
    pub(super) fn open_more_effects(&mut self, class: &str, add: bool) -> bool {
        let class: &'static str = match class {
            "emph" => "emph",
            "exit" => "exit",
            "path" => "path",
            _ => "entr",
        };
        self.open_form8(super::forms8::Dlg::MoreEffects(class, add))
    }

    /// An effect chosen in the gallery.
    pub(super) fn pick_effect(&mut self, effect: &str, class: &str, add: bool) -> bool {
        if self.view.selection().is_empty() {
            return false;
        }
        if effect == "none" {
            return !add && self.apply_effect(None, None);
        }
        if add {
            return self.view.run(SlideCommand::AddAnimation {
                spec: AnimSpec {
                    effect: Some(effect.to_owned()),
                    class: Some(class.to_owned()),
                    ..AnimSpec::default()
                },
            });
        }
        self.apply_effect(Some((effect, class)), None)
    }

    /// The drop-down of effect `i` in the Animation Pane: how it starts (the current way
    /// checked), and Remove.
    pub(super) fn open_effect_menu(&mut self, i: usize) -> bool {
        let Some(a) = self.view.state().and_then(|s| s.animations.get(i)).cloned() else {
            return false;
        };
        let Some(r) = self
            .anim_pane
            .hits
            .iter()
            .find(|(_, h)| *h == crate::slide_anim_pane::AnimHit::Select(i))
            .map(|(r, _)| *r)
        else {
            return false;
        };
        // As the reference's (round 19): Start On Click / With Previous / After Previous,
        // Effect Options… and Timing…, Hide / Show Advanced Timeline, Remove.
        let mut items: Vec<MenuItem> = [
            ("onClick", "Start On Click"),
            ("withPrevious", "Start With Previous"),
            ("afterPrevious", "Start After Previous"),
        ]
        .iter()
        .map(|(t, d)| MenuItem {
            label: self.t(&format!("slideNative.anim9.{t}"), d),
            choice: Choice::Command(format!("arow:{i}:{t}")),
            checked: a.trigger == *t,
            dim: false,
        })
        .collect();
        let has_options = !a.dirs.is_empty();
        for (id, key, d, dim) in [
            (
                "options",
                "effectOptions",
                "Effect Options...",
                !has_options,
            ),
            ("timing", "timing", "Timing...", false),
            (
                "timeline",
                if self.anim_pane.hide_timeline {
                    "showTimeline"
                } else {
                    "hideTimeline"
                },
                if self.anim_pane.hide_timeline {
                    "Show Advanced Timeline"
                } else {
                    "Hide Advanced Timeline"
                },
                false,
            ),
            ("remove", "remove", "Remove", false),
        ] {
            items.push(MenuItem {
                label: self.t(&format!("slideNative.anim9.{key}"), d),
                choice: Choice::Command(format!("arow:{i}:{id}")),
                checked: false,
                dim,
            });
        }
        self.menu_kind = super::MenuKind::Duration;
        self.menu =
            Some(Menu::open(items, r, self.size, self.scale).with_decor(vec![None; 7], vec![3, 5]));
        true
    }

    /// A choice of an effect's drop-down (`arow:<index>:<what>`).
    pub(super) fn choose_effect_menu(&mut self, v: &str) -> Option<bool> {
        let (i, what) = v.split_once(':')?;
        let index = i.parse::<usize>().ok()?;
        Some(match what {
            "remove" => {
                self.anim_pane.selected = None;
                self.view.run(SlideCommand::RemoveAnimation { index })
            }
            "timeline" => {
                self.anim_pane.hide_timeline = !self.anim_pane.hide_timeline;
                self.relayout_pending = true;
                true
            }
            "timing" => self.open_form8(super::forms8::Dlg::Timing(index)),
            "options" => self.action("anim.effect"),
            t => self.view.run(SlideCommand::SetAnimation {
                index,
                spec: AnimSpec {
                    trigger: Some(t.to_owned()),
                    ..AnimSpec::default()
                },
            }),
        })
    }

    /// Whether the click-number tags are drawn: on the Animations tab in Normal view, and
    /// not while a gallery, menu or dialog is open over the slide (those are drawn above).
    pub(super) fn anim_tags_shown(&self) -> bool {
        // Animations is the fourth fixed tab from the end (then Slide Show, Review, View).
        Some(self.ribbon.active_tab()) == self.base_tabs.checked_sub(4)
            && self.view.mode() == crate::slide_view::ViewMode::Normal
            && self.picker.is_none()
            && self.menu.is_none()
            && self.form8.is_none()
            && self.dialog.is_none()
    }

    /// On the Animations tab, each animated shape's click numbers in small tags left of its
    /// top-left corner (one per effect, stacked), the chosen effect's tag filled — as the
    /// reference marks them.
    pub(super) fn paint_anim_tags(&self, p: &mut dyn Painter) {
        if !self.anim_tags_shown() {
            return;
        }
        let Some(st) = self.view.state() else {
            return;
        };
        let area = self.view.canvas_rect();
        p.push_clip(area[0], area[1], area[2], area[3]);
        let (tw, th) = (self.px(25.6), self.px(17.6));
        let style = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        let mut placed: Vec<(u32, usize)> = Vec::new();
        for a in &st.animations {
            let Some(r) = a.rect else {
                continue;
            };
            let k = placed.iter().filter(|(id, _)| *id == a.shape_id).count();
            placed.push((a.shape_id, a.index));
            let (x, y) = self.view.to_px(r[0], r[1]);
            let tx = (x - tw - self.px(8.0)).round();
            let ty = (y + k as f32 * (th + self.px(2.0))).round();
            let chosen = self.anim_pane.selected == Some(a.index);
            let (edge, fill) = if chosen {
                (Rgba(0x5b, 0x8f, 0xd0, 255), Rgba(0xcd, 0xe6, 0xf7, 255))
            } else {
                (Rgba(0x80, 0x80, 0x80, 255), Rgba(0xff, 0xff, 0xff, 255))
            };
            p.fill_rect(tx, ty, tw, th, edge);
            p.fill_rect(tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0, fill);
            let label = a.click.to_string();
            let w = style.size * 0.55 * label.len() as f32;
            p.text(
                &label,
                (tx + (tw - w) / 2.0).round(),
                (ty + (th - style.size * 1.33) / 2.0).round(),
                style,
                Rgba(0x26, 0x26, 0x26, 255),
            );
        }
        p.pop_clip();
    }
}
