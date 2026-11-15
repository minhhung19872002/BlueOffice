//! Slide show sequencing and presenting (N6, round 4), as the reference presentation app
//! plays a deck: the slides of the Set Up Slide Show range or a custom show, hidden slides
//! left out, looping until Esc, automatic timings on or off, Rehearse Timings (the time spent
//! on each slide is kept), and Presenter View (current slide, next slide, notes, timer).
use super::{SlideView, SHOW_KEY};
use crate::{Painter, TextStyle};
use bo_engine::slide_edit::present::{self, SlideRange, SlideTiming};
use bo_render::Rgba;

/// How a slide show plays.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShowPlan {
    /// Slides in playing order.
    pub order: Vec<usize>,
    /// Start again after the last slide (`showPr@loop`).
    pub looped: bool,
    /// Advance after the slides' timings (`showPr@useTimings`).
    pub use_timings: bool,
    /// Presenter View in the window.
    pub presenter: bool,
    /// Rehearse Timings: time each slide.
    pub rehearse: bool,
}

/// Rehearsal and presenter state of the running show.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Presenting {
    pub plan: ShowPlan,
    /// Clock when the show started and when the current slide arrived.
    pub started: f64,
    pub slide_started: f64,
    /// Time spent on each slide so far (ms), by slide index.
    pub spent: Vec<(usize, f64)>,
    /// Whole seconds shown by the timer (redraw when it changes).
    pub shown_sec: i64,
}

impl SlideView {
    /// The deck's playing order: the setup's range or custom show (`custom` overrides it),
    /// without hidden slides; the plan's flags from the setup.
    pub fn show_plan(&self, custom: Option<u32>) -> ShowPlan {
        let Some(pres) = self.engine.presentation() else {
            return ShowPlan {
                order: (0..self.ids.len()).collect(),
                use_timings: true,
                ..ShowPlan::default()
            };
        };
        let out = present::present_out(pres);
        let n = self.ids.len();
        let shows = |id: u32| {
            out.custom_shows
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.slides.clone())
        };
        let mut order: Vec<usize> = match (custom, &out.setup.range) {
            (Some(id), _) => shows(id).unwrap_or_default(),
            (None, SlideRange::Range { from, to }) => {
                let (a, b) = ((*from as usize).saturating_sub(1), (*to as usize).min(n));
                (a..b.max(a)).collect()
            }
            (None, SlideRange::Custom { id }) => shows(*id).unwrap_or_default(),
            (None, SlideRange::All) => (0..n).collect(),
        };
        // Hidden slides are left out (§19.3.1.38 sld@show), except in a custom show.
        if custom.is_none() {
            order.retain(|i| !out.hidden.contains(i));
        }
        order.retain(|i| *i < n);
        if order.is_empty() {
            order = (0..n).collect();
        }
        ShowPlan {
            order,
            looped: out.setup.looped,
            use_timings: out.setup.use_timings,
            presenter: false,
            rehearse: false,
        }
    }

    /// Start the show with `plan` at slide `from` (also when it is not in the plan: the
    /// reference plays a hidden slide when the show starts from it).
    pub fn start_show_with(&mut self, from: usize, plan: ShowPlan) -> bool {
        let start = self.start_show(from);
        let now = self.clock;
        self.presenting = Some(Presenting {
            plan,
            started: now,
            slide_started: now,
            spent: Vec::new(),
            shown_sec: 0,
        });
        start
    }

    /// The slide after `index` in the show (`None`: the end screen).
    pub(super) fn next_in_show(&self, index: usize) -> Option<usize> {
        let n = self.ids.len();
        let Some(p) = self.presenting.as_ref() else {
            return (index + 1 < n).then_some(index + 1);
        };
        let order = &p.plan.order;
        let next = match order.iter().position(|i| *i == index) {
            Some(k) => order.get(k + 1).copied(),
            // Started on a slide outside the order: the next slide of the order after it.
            None => order.iter().copied().find(|i| *i > index),
        };
        match next {
            Some(i) => Some(i),
            None if p.plan.looped => order.first().copied(),
            None => None,
        }
    }

    /// The slide before `index` in the show.
    pub(super) fn prev_in_show(&self, index: usize) -> Option<usize> {
        let Some(p) = self.presenting.as_ref() else {
            return index.checked_sub(1);
        };
        let order = &p.plan.order;
        match order.iter().position(|i| *i == index) {
            Some(0) if p.plan.looped => order.last().copied(),
            Some(k) => k.checked_sub(1).map(|k| order[k]),
            None => order.iter().rev().copied().find(|i| *i < index),
        }
    }

    /// Whether the show advances on the slides' timings.
    pub(super) fn timings_on(&self) -> bool {
        self.presenting
            .as_ref()
            .is_none_or(|p| p.plan.use_timings && !p.plan.rehearse)
    }

    /// The show left slide `index`: count the time spent on it (rehearsal).
    pub(super) fn left_slide(&mut self, index: usize) {
        let now = self.clock;
        if let Some(p) = self.presenting.as_mut() {
            let spent = now - p.slide_started;
            match p.spent.iter_mut().find(|(i, _)| *i == index) {
                Some((_, t)) => *t += spent,
                None => p.spent.push((index, spent)),
            }
            p.slide_started = now;
        }
    }

    /// Whether Presenter View is showing.
    pub fn presenter(&self) -> bool {
        self.show.is_some() && self.presenting.as_ref().is_some_and(|p| p.plan.presenter)
    }

    /// After a rehearsal ended: the timing of each slide played (taken once).
    pub fn take_rehearsal(&mut self) -> Option<Vec<SlideTiming>> {
        self.rehearsed.take()
    }

    /// The show ended: keep a rehearsal's timings for the window to offer.
    pub(super) fn finish_presenting(&mut self, last: usize) {
        if self.show.is_some() {
            self.left_slide(last);
        }
        if let Some(p) = self.presenting.take() {
            if p.plan.rehearse && !p.spent.is_empty() {
                let mut t: Vec<SlideTiming> = p
                    .spent
                    .iter()
                    .map(|(slide, ms)| SlideTiming {
                        slide: *slide,
                        // Whole seconds, as the reference keeps rehearsed times.
                        ms: Some(((ms / 1000.0).round().max(1.0) * 1000.0) as u32),
                    })
                    .collect();
                t.sort_by_key(|s| s.slide);
                self.rehearsed = Some(t);
            }
        }
    }

    /// Seconds since the show started and on the current slide.
    fn timer(&self) -> Option<(f64, f64)> {
        let p = self.presenting.as_ref()?;
        Some((
            (self.clock - p.started).max(0.0) / 1000.0,
            (self.clock - p.slide_started).max(0.0) / 1000.0,
        ))
    }

    /// A tick of the clock: `true` when the timer shown moved to the next second.
    pub(super) fn timer_tick(&mut self) -> bool {
        let Some((total, _)) = self.timer() else {
            return false;
        };
        let visible = self
            .presenting
            .as_ref()
            .is_some_and(|p| p.plan.presenter || p.plan.rehearse);
        let sec = total.floor() as i64;
        match self.presenting.as_mut() {
            Some(p) if visible && p.shown_sec != sec => {
                p.shown_sec = sec;
                true
            }
            _ => false,
        }
    }

    /// Where Presenter View puts the current slide (the window's left 62 %).
    pub(super) fn presenter_slide_area(&self) -> [f32; 4] {
        let (w, h) = self.window;
        let top = self.px(64.0);
        let bottom = self.px(64.0);
        [
            self.px(24.0),
            top,
            (w * 0.62).round(),
            (h - top - bottom).max(1.0),
        ]
    }

    fn presenter_next_area(&self) -> [f32; 4] {
        let (w, _) = self.window;
        let [_, top, cw, _] = self.presenter_slide_area();
        let x = self.px(24.0) + cw + self.px(32.0);
        let nw = (w - x - self.px(24.0)).max(1.0);
        [
            x,
            top + self.px(28.0),
            nw,
            (nw * self.slide_size.1 / self.slide_size.0).round(),
        ]
    }

    /// Presenter View around the current slide (drawn before it): dark background, timer,
    /// next slide, notes and the slide counter.
    pub(super) fn paint_presenter(&self, p: &mut dyn Painter, text_size: f32) {
        let (w, h) = self.window;
        let ink = Rgba(0xf2, 0xf2, 0xf2, 255);
        let dim = Rgba(0xa6, 0xa6, 0xa6, 255);
        p.fill_rect(0.0, 0.0, w, h, Rgba(0x26, 0x26, 0x26, 255));
        let style = TextStyle {
            size: text_size,
            bold: false,
        };
        let big = TextStyle {
            size: (text_size * 1.6).round(),
            bold: false,
        };
        if let Some((total, here)) = self.timer() {
            let fmt = |s: f64| {
                let s = s.floor() as u64;
                format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
            };
            p.text(&fmt(total), self.px(24.0), self.px(18.0), big, ink);
            let rehearse = self.presenting.as_ref().is_some_and(|q| q.plan.rehearse);
            if rehearse {
                p.text(&fmt(here), self.px(180.0), self.px(18.0), big, dim);
            }
        }
        let cur = self.show_slide();
        let next = cur.and_then(|i| self.next_in_show(i));
        let [nx, ny, nw, nh] = self.presenter_next_area();
        p.text(&self.present_labels.0, nx, ny - self.px(26.0), style, dim);
        if let Some(j) = next {
            let key = SHOW_KEY + self.ids[j];
            if let Some((iw, ih)) = self.image_size(key) {
                p.page_part(key, [0.0, 0.0, iw, ih], [nx, ny, nw, nh], 1.0);
            } else {
                p.fill_rect(nx, ny, nw, nh, Rgba(0x40, 0x40, 0x40, 255));
            }
        } else {
            p.fill_rect(nx, ny, nw, nh, Rgba(0, 0, 0, 255));
            p.text(
                &self.labels.1,
                nx + self.px(8.0),
                ny + self.px(8.0),
                style,
                ink,
            );
        }
        // Notes of the current slide, one paragraph per line, under the next slide.
        let notes = cur
            .and_then(|i| self.engine.slide_notes(i).ok())
            .unwrap_or_default();
        let mut y = ny + nh + self.px(24.0);
        let line = (text_size * 1.5).round();
        p.push_clip(nx, y, nw, (h - y - self.px(64.0)).max(0.0));
        for para in notes.lines() {
            for chunk in wrap(para, (nw / (text_size * 0.55)).max(8.0) as usize) {
                p.text(&chunk, nx, y, style, ink);
                y += line;
            }
        }
        p.pop_clip();
        // Slide counter under the current slide.
        if let Some(i) = cur {
            let counter = self
                .present_labels
                .1
                .replace("{{n}}", &(i + 1).to_string())
                .replace("{{total}}", &self.ids.len().to_string());
            let [sx, ..] = self.presenter_slide_area();
            p.text(&counter, sx, h - self.px(44.0), style, ink);
        }
    }

    /// Rehearse Timings outside Presenter View: the reference's Recording bar at the top
    /// left (time on this slide, then the whole show).
    pub(super) fn paint_rehearsal_bar(&self, p: &mut dyn Painter, text_size: f32) {
        let rehearse = self
            .presenting
            .as_ref()
            .is_some_and(|q| q.plan.rehearse && !q.plan.presenter);
        let Some((total, here)) = self.timer().filter(|_| rehearse) else {
            return;
        };
        let fmt = |s: f64| {
            let s = s.floor() as u64;
            format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
        };
        let style = TextStyle {
            size: text_size,
            bold: false,
        };
        let (x, y, w, h) = (self.px(8.0), self.px(8.0), self.px(200.0), self.px(32.0));
        p.fill_rect(x, y, w, h, Rgba(0xc6, 0xc6, 0xc6, 255));
        p.fill_rect(
            x + 1.0,
            y + 1.0,
            w - 2.0,
            h - 2.0,
            Rgba(0xf3, 0xf3, 0xf3, 255),
        );
        let ty = y + (h - text_size * 1.33) / 2.0;
        let ink = Rgba(0x26, 0x26, 0x26, 255);
        p.text(&fmt(here), x + self.px(12.0), ty, style, ink);
        p.text(&fmt(total), x + self.px(110.0), ty, style, ink);
    }

    /// Size (device px) of a stored image.
    fn image_size(&self, key: usize) -> Option<(f32, f32)> {
        self.shown
            .get(&key)
            .map(|(s, l)| ((l.width * s).ceil(), (l.height * s).ceil()))
    }
}

/// Break `text` into lines of at most `cols` characters at spaces.
fn wrap(text: &str, cols: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > cols {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    out.push(line);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_breaks_at_spaces() {
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap("", 5), vec![""]);
    }
}
