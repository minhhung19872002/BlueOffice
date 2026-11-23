//! Mailings › Start Mail Merge › Step-by-Step Mail Merge Wizard of BlueDoc (round 16): the
//! reference's six-step task pane at the window's right (250 px at 125 % on #E6E6E6) —
//! 1 document type, 2 starting document, 3 recipients, 4 write the letter, 5 preview, 6
//! complete — each step's choices as radio rows and its actions as links that run BlueDoc's
//! own Mailings commands; Next / Previous at the foot.
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

const BG: Rgba = Rgba(0xE6, 0xE6, 0xE6, 255);
const EDGE: Rgba = Rgba(0xBF, 0xBF, 0xBF, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const LINK: Rgba = Rgba(0x00, 0x5A, 0x9E, 255);
const RULE: Rgba = Rgba(0xC6, 0xC6, 0xC6, 255);

/// What a press asks the window to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WizAction {
    None,
    Close,
    /// Run interface command `id`.
    Run(String),
}

/// One row of a step.
enum Line {
    Head(String),
    Text(String),
    /// Radio: index in the step's choice, label.
    Radio(usize, String),
    /// Link: command id, label.
    Link(String, String),
}

/// The wizard pane.
#[derive(Debug, Clone, Default)]
pub(crate) struct WizardPane {
    /// Step 1…6.
    pub(crate) step: u8,
    /// Document type (0 letters, 1 e-mail, 2 envelopes, 3 labels, 4 directory).
    pub(crate) kind: usize,
    /// Starting document (0 current, 1 template, 2 existing).
    pub(crate) start: usize,
    /// Recipients (0 existing list, 1 new list).
    pub(crate) recipients: usize,
    /// Record shown in step 5 (0-based) and the number of records.
    pub(crate) record: usize,
    pub(crate) records: usize,
    rect: [f32; 4],
    scale: f32,
    hits: Vec<([f32; 4], WizAction)>,
    hover: Option<usize>,
}

/// Interface strings: key, English.
pub(crate) type Tr<'a> = &'a dyn Fn(&str, &str) -> String;

const KINDS: [&str; 5] = ["letters", "email", "envelopes", "labels", "directory"];

impl WizardPane {
    pub(crate) fn width(scale: f32) -> f32 {
        (200.0 * scale).round()
    }

    pub(crate) fn new() -> Self {
        Self {
            step: 1,
            ..Self::default()
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect;
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    fn lines(&self, t: Tr<'_>) -> Vec<Line> {
        let r = |i: usize, k: &str, en: &str| Line::Radio(i, t(k, en));
        let l = |id: &str, k: &str, en: &str| Line::Link(id.to_owned(), t(k, en));
        match self.step {
            1 => vec![
                Line::Head(t("doc16.wizKind", "Select document type")),
                Line::Text(t(
                    "doc16.wizKindQ",
                    "What type of document are you working on?",
                )),
                r(0, "docR3.mergeLetters", "Letters"),
                r(1, "doc16.mergeEmail", "E-mail messages"),
                r(2, "doc16.wizEnvelopes", "Envelopes"),
                r(3, "doc16.wizLabels", "Labels"),
                r(4, "docR3.mergeDirectory", "Directory"),
            ],
            2 => vec![
                Line::Head(t("doc16.wizStart", "Select starting document")),
                Line::Text(t(
                    "doc16.wizStartQ",
                    "How do you want to set up your letters?",
                )),
                r(0, "doc16.wizCurrent", "Use the current document"),
                r(1, "doc16.wizTemplate", "Start from a template"),
                r(2, "doc16.wizExisting", "Start from existing document"),
            ],
            3 => {
                let mut v = vec![
                    Line::Head(t("doc16.wizRecipients", "Select recipients")),
                    r(0, "doc16.wizUseList", "Use an existing list"),
                    r(1, "doc16.wizTypeList", "Type a new list"),
                ];
                v.push(if self.recipients == 0 {
                    l(
                        "mailings.recipients.existing",
                        "doc16.wizBrowse",
                        "Browse...",
                    )
                } else {
                    l("mailings.recipients.new", "doc16.wizCreate", "Create...")
                });
                if self.records > 0 {
                    v.push(l(
                        "mailings.editRecipients",
                        "doc16.wizEditList",
                        "Edit recipient list...",
                    ));
                }
                v
            }
            4 => vec![
                Line::Head(t("doc16.wizWrite", "Write your letter")),
                Line::Text(t(
                    "doc16.wizWriteQ",
                    "Click a location in the document, then click one of the items below.",
                )),
                l(
                    "mailings.addressBlock",
                    "doc16.wizAddress",
                    "Address block...",
                ),
                l(
                    "mailings.greetingLine",
                    "doc16.wizGreeting",
                    "Greeting line...",
                ),
                l("mailings.insertField", "doc16.wizMore", "More items..."),
            ],
            5 => vec![
                Line::Head(t("doc16.wizPreview", "Preview your letters")),
                Line::Text(format!(
                    "{} {} / {}",
                    t("doc16.wizRecipient", "Recipient:"),
                    self.record + 1,
                    self.records.max(1)
                )),
                l(
                    "mailings.previousRecord",
                    "doc16.wizPrevRec",
                    "<< Previous recipient",
                ),
                l(
                    "mailings.nextRecord",
                    "doc16.wizNextRec",
                    "Next recipient >>",
                ),
                l(
                    "mailings.findRecipient",
                    "doc16.wizFind",
                    "Find a recipient...",
                ),
                l(
                    "mailings.editRecipients",
                    "doc16.wizEditList",
                    "Edit recipient list...",
                ),
            ],
            _ => vec![
                Line::Head(t("doc16.wizComplete", "Complete the merge")),
                Line::Text(t(
                    "doc16.wizCompleteQ",
                    "Mail Merge is ready to produce your letters.",
                )),
                l("mailings.finish.print", "doc16.wizPrint", "Print..."),
                l(
                    "mailings.finish.edit",
                    "doc16.wizEdit",
                    "Edit individual letters...",
                ),
            ],
        }
    }

    fn chosen(&self) -> usize {
        match self.step {
            1 => self.kind,
            2 => self.start,
            _ => self.recipients,
        }
    }

    /// Place the pane and its rows at the window's right.
    pub(crate) fn layout(&mut self, right: f32, top: f32, bottom: f32, scale: f32, t: Tr<'_>) {
        self.scale = scale;
        let w = Self::width(scale);
        self.rect = [right - w, top, w, bottom - top];
        let (x, mut y) = (self.rect[0] + self.px(12.0), top + self.px(52.0));
        let rw = w - self.px(24.0);
        let row = self.px(22.0);
        self.hits.clear();
        self.hits.push((
            [
                right - self.px(28.0),
                top + self.px(12.0),
                self.px(24.0),
                self.px(24.0),
            ],
            WizAction::Close,
        ));
        for line in self.lines(t) {
            match line {
                Line::Head(_) => y += self.px(28.0),
                Line::Text(s) => y += row * (1.0 + (s.chars().count() / 30) as f32),
                Line::Radio(i, _) => {
                    self.hits.push((
                        [x, y, rw, row],
                        WizAction::Run(format!("r16.wiz.choose:{i}")),
                    ));
                    y += row;
                }
                Line::Link(id, _) => {
                    self.hits.push(([x, y, rw, row], WizAction::Run(id)));
                    y += row;
                }
            }
        }
        let foot = bottom - self.px(70.0);
        if self.step < 6 {
            self.hits
                .push(([x, foot, rw, row], WizAction::Run("r16.wiz.next".into())));
        }
        if self.step > 1 {
            self.hits.push((
                [x, foot + row, rw, row],
                WizAction::Run("r16.wiz.prev".into()),
            ));
        }
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.hits.iter().position(|(r, _)| inside(*r, x, y));
        let c = h != self.hover;
        self.hover = h;
        c
    }

    pub(crate) fn press(&self, x: f32, y: f32) -> WizAction {
        self.hits
            .iter()
            .find(|(r, _)| inside(*r, x, y))
            .map_or(WizAction::None, |(_, a)| a.clone())
    }

    /// Choose radio `i` of the step; for step 1 the merge type to set.
    pub(crate) fn choose(&mut self, i: usize) -> Option<&'static str> {
        match self.step {
            1 => {
                self.kind = i.min(4);
                Some(KINDS[self.kind])
            }
            2 => {
                self.start = i.min(2);
                None
            }
            _ => {
                self.recipients = i.min(1);
                None
            }
        }
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, t: Tr<'_>) {
        let [x0, y0, w, h] = self.rect;
        p.fill_rect(x0, y0, w, h, BG);
        p.fill_rect(x0, y0, 1.0, h, EDGE);
        let title = TextStyle {
            size: self.px(20.0),
            bold: false,
        };
        p.text(
            &t("doc16.wizTitle", "Mail Merge"),
            x0 + self.px(12.0),
            y0 + self.px(12.0),
            title,
            INK,
        );
        let k = self.px(5.0);
        let (cx, cy) = (x0 + w - self.px(16.0), y0 + self.px(24.0));
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((INK, self.scale.max(1.0).round())),
        );
        let body = TextStyle {
            size: self.px(11.5),
            bold: false,
        };
        let head = TextStyle {
            size: self.px(13.0),
            bold: true,
        };
        let (x, mut y) = (x0 + self.px(12.0), y0 + self.px(52.0));
        let row = self.px(22.0);
        let lh = body.size * 1.33;
        let chosen = self.chosen();
        for line in self.lines(t) {
            match line {
                Line::Head(s) => {
                    p.text(&s, x, y, head, INK);
                    y += self.px(28.0);
                }
                Line::Text(s) => {
                    // Wrapped about 30 characters a line.
                    let mut cur = String::new();
                    for word in s.split(' ') {
                        if cur.chars().count() + word.chars().count() > 30 && !cur.is_empty() {
                            p.text(&cur, x, y, body, INK);
                            y += row;
                            cur.clear();
                        }
                        if !cur.is_empty() {
                            cur.push(' ');
                        }
                        cur.push_str(word);
                    }
                    p.text(&cur, x, y, body, INK);
                    y += row;
                }
                Line::Radio(i, s) => {
                    let r = self.px(6.0);
                    let (cx, cy) = (x + r, y + row / 2.0);
                    let n = 16;
                    let ring: Vec<PathSeg> = (0..=n)
                        .map(|j| {
                            let a = std::f32::consts::TAU * j as f32 / n as f32;
                            let (px, py) = (cx + r * a.cos(), cy + r * a.sin());
                            if j == 0 {
                                PathSeg::M(px, py)
                            } else {
                                PathSeg::L(px, py)
                            }
                        })
                        .collect();
                    p.path(&ring, Some(Rgba(255, 255, 255, 255)), Some((INK, 1.0)));
                    if i == chosen {
                        // A round dot inside the ring.
                        let d = r * 0.5;
                        let dot: Vec<PathSeg> = (0..=n)
                            .map(|j| {
                                let a = std::f32::consts::TAU * j as f32 / n as f32;
                                let (px, py) = (cx + d * a.cos(), cy + d * a.sin());
                                if j == 0 {
                                    PathSeg::M(px, py)
                                } else {
                                    PathSeg::L(px, py)
                                }
                            })
                            .collect();
                        p.path(&dot, Some(INK), None);
                    }
                    p.text(&s, x + self.px(20.0), y + (row - lh) / 2.0, body, INK);
                    y += row;
                }
                Line::Link(_, s) => {
                    p.text(&s, x + self.px(20.0), y + (row - lh) / 2.0, body, LINK);
                    y += row;
                }
            }
        }
        let foot = y0 + h - self.px(70.0);
        p.fill_rect(x, foot - self.px(30.0), w - self.px(24.0), 1.0, RULE);
        p.text(
            &format!("{} {} / 6", t("doc16.wizStep", "Step"), self.step),
            x,
            foot - self.px(24.0),
            body,
            INK,
        );
        let next = [
            ("doc16.wizNext2", "Next: Starting document"),
            ("doc16.wizNext3", "Next: Select recipients"),
            ("doc16.wizNext4", "Next: Write your letter"),
            ("doc16.wizNext5", "Next: Preview your letters"),
            ("doc16.wizNext6", "Next: Complete the merge"),
        ];
        let prev = [
            ("doc16.wizPrev1", "Previous: Select document type"),
            ("doc16.wizPrev2", "Previous: Starting document"),
            ("doc16.wizPrev3", "Previous: Select recipients"),
            ("doc16.wizPrev4", "Previous: Write your letter"),
            ("doc16.wizPrev5", "Previous: Preview your letters"),
        ];
        let s = usize::from(self.step);
        if (1..6).contains(&s) {
            let (k, en) = next[s - 1];
            p.text(
                &format!("→ {}", t(k, en)),
                x,
                foot + (row - lh) / 2.0,
                body,
                LINK,
            );
        }
        if s > 1 {
            let (k, en) = prev[s - 2];
            p.text(
                &format!("← {}", t(k, en)),
                x,
                foot + row + (row - lh) / 2.0,
                body,
                LINK,
            );
        }
    }
}

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}
