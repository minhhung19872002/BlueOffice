//! BlueEditor round 7: View › Page Transitions as the reference editor's dialog — Transition
//! (No Transition and the twelve PDF styles), Direction (the ones the style takes), Speed (Slow,
//! Medium, Fast) and the pages (all, or a range) — written as each page's `/Trans`
//! (`bo_pdfread::transitions`, one undo step).
use crate::editor_window::{inside, EditorWindow};
use crate::reader_window::rounded;
use crate::{Painter, TextStyle};
use bo_pdfread::{Transition, TransitionDirection as D, TransitionKind as K};
use bo_render::{PathSeg, Rgba};

/// Speeds offered: label key and seconds (`/D`).
const SPEEDS: [(&str, f32); 3] = [
    ("pdfEditor7.speedSlow", 2.0),
    ("pdfEditor7.speedMedium", 1.0),
    ("pdfEditor7.speedFast", 0.5),
];

fn kind_key(k: K) -> &'static str {
    match k {
        K::Split => "pdfEditor7.trSplit",
        K::Blinds => "pdfEditor7.trBlinds",
        K::Box => "pdfEditor7.trBox",
        K::Wipe => "pdfEditor7.trWipe",
        K::Dissolve => "pdfEditor7.trDissolve",
        K::Glitter => "pdfEditor7.trGlitter",
        K::Push => "pdfEditor7.trPush",
        K::Cover => "pdfEditor7.trCover",
        K::Uncover => "pdfEditor7.trUncover",
        K::Fade => "pdfEditor7.trFade",
        K::Replace => "pdfEditor7.trReplace",
        K::Fly => "pdfEditor7.trFly",
    }
}

fn dir_key(d: D) -> &'static str {
    match d {
        D::HorizontalIn => "pdfEditor7.dirHorizontalIn",
        D::HorizontalOut => "pdfEditor7.dirHorizontalOut",
        D::VerticalIn => "pdfEditor7.dirVerticalIn",
        D::VerticalOut => "pdfEditor7.dirVerticalOut",
        D::Horizontal => "pdfEditor7.dirHorizontal",
        D::Vertical => "pdfEditor7.dirVertical",
        D::In => "pdfEditor7.dirIn",
        D::Out => "pdfEditor7.dirOut",
        D::LeftToRight => "pdfEditor7.dirLeftToRight",
        D::TopToBottom => "pdfEditor7.dirTopToBottom",
        D::RightToLeft => "pdfEditor7.dirRightToLeft",
        D::BottomToTop => "pdfEditor7.dirBottomToTop",
        D::TopLeftToBottomRight => "pdfEditor7.dirTopLeft",
    }
}

impl EditorWindow {
    /// Transition labels: No Transition first, then the styles.
    fn transition_labels(&self) -> Vec<String> {
        std::iter::once(self.t("pdfEditor7.trNone"))
            .chain(K::ALL.iter().map(|k| self.t(kind_key(*k))))
            .collect()
    }

    /// Open the Page Transitions dialog with the current page's transition (the reference's
    /// classic layout, `TransDlg`).
    pub(crate) fn open_transitions(&mut self) {
        let page = self.r.view().map_or(0, crate::ReaderView::current_page);
        let n = self.r.view().map_or(0, crate::ReaderView::page_count);
        let now = self
            .with_doc(|d| Ok(d.page_transition(page)))
            .ok()
            .flatten();
        let kind = now.map_or(0, |t| {
            K::ALL
                .iter()
                .position(|k| *k == t.kind)
                .map_or(0, |i| i + 1)
        });
        let dir = now
            .and_then(|t| t.direction.map(|d| (t.kind, d)))
            .and_then(|(k, d)| k.directions().iter().position(|x| *x == d))
            .unwrap_or(0);
        let speed = now.map_or(1, |t| {
            SPEEDS
                .iter()
                .position(|s| (s.1 - t.seconds).abs() < 0.26)
                .unwrap_or(1)
        });
        let selected: Vec<usize> = if self.organize_on() {
            self.org.selected.iter().copied().collect()
        } else {
            Vec::new()
        };
        self.trans = Some(TransDlg {
            kind,
            dir,
            speed,
            range: 1,
            from: page + 1,
            to: page + 1,
            n,
            selected,
            list: None,
            hover: None,
            hits: Vec::new(),
            frame: [0.0; 4],
        });
    }

    /// Labels of drop-down `i` (Transition, Direction, Speed) for the dialog's state.
    fn trans_list(&self, d: &TransDlg, i: usize) -> Vec<String> {
        match i {
            0 => self.transition_labels(),
            1 => {
                let k = d.kind.checked_sub(1).and_then(|i| K::ALL.get(i).copied());
                let dirs = k.map_or(&[][..], K::directions);
                dirs.iter().map(|x| self.t(dir_key(*x))).collect()
            }
            _ => SPEEDS.iter().map(|s| self.t(s.0)).collect(),
        }
    }

    /// Input while the Page Transitions dialog is open; true when used (it keeps everything).
    pub(crate) fn trans_input(&mut self, input: &crate::Input) -> bool {
        use crate::{Input, Key};
        let Some(d) = self.trans.as_mut() else {
            return false;
        };
        match input {
            Input::Resize { .. } | Input::Scale(_) | Input::Maximized(_) | Input::Tick => false,
            Input::Move { x, y } => {
                d.hover = d.hits.iter().position(|(r, _)| inside(*r, *x, *y));
                true
            }
            Input::Key(Key::Escape, _) => {
                if d.list.is_some() {
                    d.list = None;
                } else {
                    self.trans = None;
                }
                true
            }
            Input::Key(Key::Enter, _) => {
                self.trans_ok();
                true
            }
            Input::Press { x, y, .. } => {
                let hit = d.hits.iter().find(|(r, _)| inside(*r, *x, *y)).map(|h| h.1);
                let was_list = d.list.take();
                if let Some(h) = hit {
                    self.trans_hit(h, was_list);
                }
                true
            }
            _ => true,
        }
    }

    fn trans_hit(&mut self, h: TransHit, was_list: Option<usize>) {
        let Some(d) = self.trans.as_mut() else { return };
        match h {
            TransHit::Combo(i) => {
                let on = i == 0 || d.kind > 0;
                if on && was_list != Some(i) {
                    d.list = Some(i);
                }
            }
            TransHit::Item(i, k) => match i {
                0 => {
                    if d.kind != k {
                        d.kind = k;
                        d.dir = 0;
                    }
                }
                1 => d.dir = k,
                _ => d.speed = k,
            },
            TransHit::Range(r) => {
                if r != 0 || !d.selected.is_empty() {
                    d.range = r;
                }
            }
            TransHit::Spin(which, up) => {
                if d.range == 2 {
                    let v = if which == 0 { &mut d.from } else { &mut d.to };
                    *v = if up { *v + 1 } else { v.saturating_sub(1) }.clamp(1, d.n.max(1));
                }
            }
            TransHit::Ok => self.trans_ok(),
            TransHit::Cancel => self.trans = None,
        }
    }

    /// OK: write the transition of the chosen pages.
    fn trans_ok(&mut self) {
        let Some(d) = self.trans.take() else { return };
        let pages: Vec<usize> = match d.range {
            0 => d.selected.clone(),
            1 => (0..d.n).collect(),
            _ => {
                let (a, b) = (d.from.min(d.to), d.from.max(d.to));
                (a.saturating_sub(1)..b.min(d.n)).collect()
            }
        };
        if pages.is_empty() {
            return;
        }
        let t = d
            .kind
            .checked_sub(1)
            .and_then(|i| K::ALL.get(i))
            .map(|k| Transition {
                kind: *k,
                direction: k.directions().get(d.dir).copied(),
                seconds: SPEEDS.get(d.speed).map_or(1.0, |s| s.1),
            });
        match self.doc_edit(|doc| doc.set_page_transitions(&pages, t.as_ref())) {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }

    /// The dialog as the reference draws it (125 % pixels from its capture, scaled): 530 x 470,
    /// "Page Transition" group (labels right-aligned left of 302 px drop-downs 27 high, face
    /// `#E2E2E2`, `#CCCCCC` while disabled, frame `#999999`), "Page Range" group with radio
    /// buttons and spin boxes, OK / Cancel 91 x 28 at the bottom right.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_trans(&mut self, p: &mut dyn Painter) {
        let Some(mut d) = self.trans.take() else {
            return;
        };
        let s = self.ui_scale() / 1.25;
        let q = |v: f32| (v * s).round();
        let (ww, wh) = self.window_size();
        let (w, h) = (q(530.0), q(470.0));
        let x0 = ((ww - w) / 2.0).round();
        let y0 = ((wh - h) / 2.0).round();
        d.frame = [x0, y0, w, h];
        let mut hits: Vec<([f32; 4], TransHit)> = Vec::new();
        let text = TextStyle {
            size: q(15.0),
            bold: false,
        };
        let th = text.size * 1.33;
        let ink = Rgba(0, 0, 0, 255);
        let grey = Rgba(0x6D, 0x6D, 0x6D, 255);
        let est = |t: &str| t.chars().count() as f32 * text.size * 0.5;
        // Backdrop, shadow, frame, caption.
        p.fill_rect(0.0, 0.0, ww, wh, Rgba(0, 0, 0, 40));
        for (k, a) in [(3.0, 18u8), (2.0, 28), (1.0, 40)] {
            p.fill_rect(
                x0 - k,
                y0 - k + 1.0,
                w + 2.0 * k,
                h + 2.0 * k,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x0, y0, w, h, Rgba(0xA0, 0xA0, 0xA0, 255));
        p.fill_rect(x0 + 1.0, y0 + 1.0, w - 2.0, h - 2.0, WHITE);
        p.fill_rect(
            x0 + 1.0,
            y0 + 1.0,
            w - 2.0,
            q(31.0),
            Rgba(0xF2, 0xF3, 0xF3, 255),
        );
        let title = self.t("pdfEditor7.transitionsTitle");
        p.text(
            &title,
            x0 + q(7.0),
            y0 + ((q(32.0) - th) / 2.0).round(),
            text,
            ink,
        );
        let (cx, cy, r) = (x0 + w - q(23.0), y0 + q(16.0), q(5.5));
        p.path(
            &[
                PathSeg::M(cx - r, cy - r),
                PathSeg::L(cx + r, cy + r),
                PathSeg::M(cx + r, cy - r),
                PathSeg::L(cx - r, cy + r),
            ],
            None,
            Some((Rgba(0x40, 0x40, 0x40, 255), s.max(1.0))),
        );
        hits.push(([x0 + w - q(46.0), y0, q(46.0), q(32.0)], TransHit::Cancel));
        // Group boxes with their titles on the top line.
        let group = |p: &mut dyn Painter, label: &str, top: f32, bottom: f32| {
            let (gx, gw) = (x0 + q(12.0), q(505.0));
            let (gt, gb) = (y0 + top, y0 + bottom);
            let edge = Rgba(0xDC, 0xDC, 0xDC, 255);
            p.fill_rect(gx, gt, gw, 1.0, edge);
            p.fill_rect(gx, gb, gw, 1.0, edge);
            p.fill_rect(gx, gt, 1.0, gb - gt, edge);
            p.fill_rect(gx + gw, gt, 1.0, gb - gt + 1.0, edge);
            let lw = est(label) + q(6.0);
            p.fill_rect(gx + q(8.0), gt - th / 2.0, lw, th, WHITE);
            p.text(label, gx + q(10.0), gt - th / 2.0, text, ink);
        };
        group(p, &self.t("pdfEditor7.groupTransition"), q(65.0), q(200.0));
        group(p, &self.t("pdfEditor7.groupRange"), q(237.0), q(351.0));
        // Drop-downs with right-aligned labels.
        let labels = [
            self.t("pdfEditor7.transitionColon"),
            self.t("pdfEditor7.directionColon"),
            self.t("pdfEditor7.speedColon"),
        ];
        let lists: Vec<Vec<String>> = (0..3).map(|i| self.trans_list(&d, i)).collect();
        let values = [d.kind, d.dir, d.speed];
        let mut list_box = None;
        for i in 0..3 {
            let cy = y0 + q(102.0 + 36.0 * i as f32);
            let on = i == 0 || d.kind > 0;
            p.text(
                &labels[i],
                x0 + q(112.0) - est(&labels[i]),
                (cy - th / 2.0).round(),
                text,
                ink,
            );
            let r = [x0 + q(118.0), cy - q(14.0), q(302.0), q(27.0)];
            let frame = if d
                .hover
                .and_then(|k| hits.get(k))
                .is_some_and(|h| h.1 == TransHit::Combo(i))
            {
                Rgba(0x00, 0x78, 0xD4, 255)
            } else {
                Rgba(0x99, 0x99, 0x99, 255)
            };
            p.fill_rect(r[0], r[1], r[2], r[3], frame);
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                if on {
                    Rgba(0xE2, 0xE2, 0xE2, 255)
                } else {
                    Rgba(0xCC, 0xCC, 0xCC, 255)
                },
            );
            if on {
                if let Some(v) = lists[i].get(values[i]) {
                    p.text(
                        v,
                        r[0] + q(4.0),
                        (cy - th / 2.0).round(),
                        text,
                        Rgba(0x33, 0, 0, 255),
                    );
                }
            }
            let (ax, ay, a) = (r[0] + r[2] - q(14.0), cy, q(4.5));
            p.path(
                &[
                    PathSeg::M(ax - a, ay - a / 2.0),
                    PathSeg::L(ax, ay + a / 2.0),
                    PathSeg::L(ax + a, ay - a / 2.0),
                ],
                None,
                Some((Rgba(0x80, 0x80, 0x80, 255), s.max(1.0))),
            );
            hits.push((r, TransHit::Combo(i)));
            if d.list == Some(i) {
                list_box = Some((i, r));
            }
        }
        // Range: radio buttons, spin boxes, page count.
        let radios = [
            self.t("pdfEditor7.selectedPages"),
            self.t("pdfEditor7.allPages"),
            self.t("pdfEditor7.pagesFrom"),
        ];
        for (k, label) in radios.iter().enumerate() {
            let cy = y0 + q([271.0, 298.0, 327.0][k]);
            let on = k != 0 || !d.selected.is_empty();
            let (rx, rr) = (x0 + q(52.0), q(8.5));
            let ring = if on {
                Rgba(0x00, 0x5F, 0xB8, 255)
            } else {
                Rgba(0xC6, 0xC6, 0xC6, 255)
            };
            if d.range == k as u8 {
                p.path(&circle(rx, cy, rr), Some(ring), None);
                p.path(&circle(rx, cy, rr * 0.45), Some(WHITE), None);
            } else {
                p.path(
                    &circle(rx, cy, rr - 0.5),
                    Some(WHITE),
                    Some((Rgba(0x85, 0x85, 0x85, 255), 1.0)),
                );
            }
            p.text(
                label,
                x0 + q(64.0),
                (cy - th / 2.0).round(),
                text,
                if on { ink } else { grey },
            );
            if on {
                hits.push((
                    [x0 + q(40.0), cy - q(12.0), q(110.0), q(24.0)],
                    TransHit::Range(k as u8),
                ));
            }
        }
        let cy = y0 + q(327.0);
        let spin_on = d.range == 2;
        for (which, (sx, v)) in [(q(156.0), d.from), (q(294.0), d.to)]
            .into_iter()
            .enumerate()
        {
            let r = [x0 + sx, cy - q(13.0), q(87.0), q(26.0)];
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xAB, 0xAD, 0xB3, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                if spin_on {
                    WHITE
                } else {
                    Rgba(0xF0, 0xF0, 0xF0, 255)
                },
            );
            let bw = q(22.0);
            p.fill_rect(
                r[0] + r[2] - bw - 1.0,
                r[1] + 1.0,
                bw,
                r[3] - 2.0,
                Rgba(0xE5, 0xE5, 0xE5, 255),
            );
            p.fill_rect(
                r[0] + r[2] - bw - 1.0,
                cy,
                bw,
                1.0,
                Rgba(0xC8, 0xC8, 0xC8, 255),
            );
            let ax = r[0] + r[2] - 1.0 - bw / 2.0;
            for (up, ay) in [(true, cy - q(6.0)), (false, cy + q(6.0))] {
                let a = q(3.0);
                let dy = if up { -a / 2.0 } else { a / 2.0 };
                p.path(
                    &[
                        PathSeg::M(ax - a, ay - dy),
                        PathSeg::L(ax, ay + dy),
                        PathSeg::L(ax + a, ay - dy),
                    ],
                    None,
                    Some((Rgba(0x60, 0x60, 0x60, 255), 1.0)),
                );
                hits.push((
                    [
                        r[0] + r[2] - bw - 1.0,
                        if up { r[1] } else { cy },
                        bw,
                        r[3] / 2.0,
                    ],
                    TransHit::Spin(which, up),
                ));
            }
            p.text(
                &v.to_string(),
                r[0] + q(8.0),
                (cy - th / 2.0).round(),
                text,
                if spin_on { ink } else { grey },
            );
        }
        p.text(
            &self.t("pdfEditor7.to"),
            x0 + q(254.0),
            (cy - th / 2.0).round(),
            text,
            ink,
        );
        p.text(
            &self.t("pdfEditor7.of"),
            x0 + q(394.0),
            (cy - th / 2.0).round(),
            text,
            ink,
        );
        p.text(
            &d.n.to_string(),
            x0 + q(437.0),
            (cy - th / 2.0).round(),
            text,
            ink,
        );
        // OK / Cancel.
        for (k, (label, hit)) in [
            (self.t("pdfReader.ok"), TransHit::Ok),
            (self.t("pdfReader.cancel"), TransHit::Cancel),
        ]
        .into_iter()
        .enumerate()
        {
            let r = [
                x0 + q(326.0 + 100.0 * k as f32),
                y0 + q(442.0),
                q(91.0),
                q(28.0),
            ];
            let hot = d
                .hover
                .and_then(|i| hits.get(i))
                .is_some_and(|h| h.1 == hit && h.0 == r);
            let edge = if k == 0 {
                Rgba(0x00, 0x78, 0xD4, 255)
            } else {
                Rgba(0xD0, 0xD0, 0xD0, 255)
            };
            p.path(&rounded(r[0], r[1], r[2], r[3], q(4.0)), Some(edge), None);
            let i = if k == 0 { 1.5 } else { 1.0 };
            p.path(
                &rounded(r[0] + i, r[1] + i, r[2] - 2.0 * i, r[3] - 2.0 * i, q(3.0)),
                Some(if hot {
                    Rgba(0xF5, 0xF5, 0xF5, 255)
                } else {
                    Rgba(0xFD, 0xFD, 0xFD, 255)
                }),
                None,
            );
            p.text(
                &label,
                (r[0] + (r[2] - est(&label)) / 2.0).round(),
                (r[1] + (r[3] - th) / 2.0).round(),
                text,
                ink,
            );
            hits.push((r, hit));
        }
        // An open drop-down list over everything, with its own hit boxes first: the Windows
        // combo list of the reference (round 8, `editor_form8::paint_list`).
        if let Some((i, r)) = list_box {
            let hover = match d.hover.and_then(|k| hits.get(k)).map(|h| h.1) {
                Some(TransHit::Item(j, k)) if j == i => Some(k),
                _ => None,
            };
            let mut front: Vec<([f32; 4], TransHit)> =
                crate::editor_form8::paint_list(p, r, &lists[i], values[i], hover, s, text)
                    .into_iter()
                    .map(|(ir, k)| (ir, TransHit::Item(i, k)))
                    .collect();
            front.extend(hits);
            hits = front;
        }
        d.hits = hits;
        self.trans = Some(d);
    }
}

/// What a click on the Page Transitions dialog hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransHit {
    Combo(usize),
    Item(usize, usize),
    Range(u8),
    Spin(usize, bool),
    Ok,
    Cancel,
}

/// The Page Transitions dialog's state.
#[derive(Debug, Clone)]
pub(crate) struct TransDlg {
    /// 0: No Transition, else index into `TransitionKind::ALL` + 1.
    pub kind: usize,
    pub dir: usize,
    pub speed: usize,
    /// 0 selected pages, 1 all pages, 2 pages from … to.
    pub range: u8,
    pub from: usize,
    pub to: usize,
    pub n: usize,
    pub selected: Vec<usize>,
    /// Drop-down list shown (0 Transition, 1 Direction, 2 Speed).
    pub list: Option<usize>,
    hover: Option<usize>,
    pub hits: Vec<([f32; 4], TransHit)>,
    pub frame: [f32; 4],
}

const WHITE: Rgba = Rgba(255, 255, 255, 255);

fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = r * 0.552_285;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

impl EditorWindow {
    /// Form › XFA: a static XFA form becomes a plain AcroForm form (its fields stay); a
    /// dynamic one is not offered.
    pub(crate) fn static_xfa(&mut self) {
        let kind = self
            .with_doc(|d| Ok(d.xfa_kind()))
            .unwrap_or(bo_pdfread::XfaKind::None);
        let key = match kind {
            bo_pdfread::XfaKind::None => "pdfEditor7.xfaNone",
            bo_pdfread::XfaKind::Dynamic => "pdfEditor7.xfaDynamic",
            bo_pdfread::XfaKind::Static => {
                match self.doc_edit(bo_pdfread::PdfDocument::drop_static_xfa) {
                    Ok(()) => {
                        self.after_doc_edit();
                        "pdfEditor7.xfaDone"
                    }
                    Err(e) => {
                        self.set_status(&e);
                        return;
                    }
                }
            }
        };
        let msg = self.t(key);
        self.set_status(&msg);
    }
}
