//! The Timing dialog of an effect, round 19 (N6): the Animation Pane row menu's Timing…,
//! with the reference's Timing page fields — Start (On Click / With Previous / After
//! Previous), Delay in seconds, Duration (Very Fast 0.5 s … Very Slow 5 s, or the effect's
//! own) and Repeat. OK sets them in one undoable step (ECMA-376 Part 1, §19.5.29 cTn
//! `dur` / `delay` / `repeatCount`, `nodeType` clickEffect / withEffect / afterEffect).
use super::forms8::Open;
use super::sform::{Form, Kind};
use super::SlideWindow;
use bo_engine::slide_edit::show::AnimSpec;
use bo_engine::slide_edit::SlideCommand;

/// Start choices.
pub(super) const STARTS: [&str; 3] = ["onClick", "withPrevious", "afterPrevious"];
/// Duration choices (ms) and their names.
pub(super) const DURATIONS: [(u32, &str); 5] = [
    (500, "0.5 seconds (Very Fast)"),
    (1000, "1 seconds (Fast)"),
    (2000, "2 seconds (Medium)"),
    (3000, "3 seconds (Slow)"),
    (5000, "5 seconds (Very Slow)"),
];
/// Repeat choices (0: none).
pub(super) const REPEATS: [u32; 6] = [0, 2, 3, 4, 5, 10];

/// Seconds typed in the Delay box ("1.25", "01,5") as milliseconds.
pub(super) fn parse_seconds(s: &str) -> Option<u32> {
    let v: f64 = s.trim().replace(',', ".").parse().ok()?;
    (v.is_finite() && (0.0..=3600.0).contains(&v)).then(|| (v * 1000.0).round() as u32)
}

impl SlideWindow {
    /// The form for effect `index`.
    pub(super) fn timing_form(&self, index: usize, ok: &str, cancel: &str) -> Form {
        let a = self
            .view
            .state()
            .and_then(|s| s.animations.get(index))
            .cloned();
        let mut f = Form::new(
            &self.t("slideNative.anim9.timingTitle", "Timing"),
            (420.0, 300.0),
            Vec::new(),
        );
        let rows = [
            ("l_start", "start", "Start:", 52.0),
            ("l_delay", "delay", "Delay:", 96.0),
            ("l_dur", "duration", "Duration:", 140.0),
            ("l_rep", "repeat", "Repeat:", 184.0),
        ];
        for (id, key, d, y) in rows {
            f.add(
                id,
                None,
                [20.0, y + 4.0, 100.0, 20.0],
                Kind::Label(self.t(&format!("slideNative.anim9.{key}"), d)),
            );
        }
        let trigger = a.as_ref().map_or("onClick", |a| a.trigger);
        f.add(
            "start",
            None,
            [130.0, 52.0, 260.0, 28.0],
            Kind::Combo {
                items: [
                    ("onClick", "On Click"),
                    ("withPrevious", "With Previous"),
                    ("afterPrevious", "After Previous"),
                ]
                .iter()
                .map(|(k, d)| self.t(&format!("slideNative.anim9.at.{k}"), d))
                .collect(),
                sel: STARTS.iter().position(|s| *s == trigger).unwrap_or(0),
            },
        );
        let delay = a.as_ref().map_or(0, |a| a.delay_ms);
        f.add(
            "delay",
            None,
            [130.0, 96.0, 120.0, 28.0],
            Kind::Edit {
                value: format!("{:.2}", f64::from(delay) / 1000.0)
                    .chars()
                    .collect(),
                caret: 0,
            },
        );
        f.add(
            "l_sec",
            None,
            [260.0, 100.0, 100.0, 20.0],
            Kind::Label(self.t("slideNative.anim9.seconds", "seconds")),
        );
        let dur = a.as_ref().map_or(500, |a| a.dur_ms);
        let mut items: Vec<String> = DURATIONS
            .iter()
            .map(|(ms, d)| self.t(&format!("slideNative.anim9.dur{ms}"), d))
            .collect();
        let sel = DURATIONS
            .iter()
            .position(|(ms, _)| *ms == dur)
            .unwrap_or_else(|| {
                items.push(format!("{:.2}", f64::from(dur) / 1000.0));
                items.len() - 1
            });
        f.add(
            "duration",
            None,
            [130.0, 140.0, 260.0, 28.0],
            Kind::Combo { items, sel },
        );
        let repeat = a.as_ref().map_or(0, |a| a.repeat);
        f.add(
            "repeat",
            None,
            [130.0, 184.0, 260.0, 28.0],
            Kind::Combo {
                items: REPEATS
                    .iter()
                    .map(|r| {
                        if *r == 0 {
                            self.t("slideNative.anim9.none", "(none)")
                        } else {
                            r.to_string()
                        }
                    })
                    .collect(),
                sel: REPEATS.iter().position(|r| *r == repeat).unwrap_or(0),
            },
        );
        f.add(
            "ok",
            None,
            [180.0, 246.0, 106.0, 32.0],
            Kind::Button {
                label: ok.to_owned(),
                default: true,
            },
        );
        f.add(
            "cancel",
            None,
            [296.0, 246.0, 106.0, 32.0],
            Kind::Button {
                label: cancel.to_owned(),
                default: false,
            },
        );
        f
    }

    /// OK: set start, delay, duration and repeat of effect `index`.
    pub(super) fn timing_ok(&mut self, index: usize, open: &Open) {
        let f = &open.form;
        let trigger = f.sel("start").and_then(|i| STARTS.get(i)).copied();
        let delay_ms = parse_seconds(&f.text("delay"));
        let dur_ms = f
            .sel("duration")
            .and_then(|i| DURATIONS.get(i).map(|d| d.0))
            .or_else(|| {
                // The effect's own duration listed last.
                self.view
                    .state()
                    .and_then(|s| s.animations.get(index))
                    .map(|a| a.dur_ms)
            });
        let repeat = f.sel("repeat").and_then(|i| REPEATS.get(i)).copied();
        self.view.run(SlideCommand::SetAnimation {
            index,
            spec: AnimSpec {
                trigger: trigger.map(str::to_owned),
                delay_ms,
                dur_ms,
                repeat,
                ..AnimSpec::default()
            },
        });
        self.relayout_pending = true;
    }
}

#[cfg(test)]
mod tests {
    use super::parse_seconds;

    #[test]
    fn seconds() {
        assert_eq!(parse_seconds("1.25"), Some(1250));
        assert_eq!(parse_seconds(" 01,5 "), Some(1500));
        assert_eq!(parse_seconds("-1"), None);
        assert_eq!(parse_seconds("x"), None);
    }
}
