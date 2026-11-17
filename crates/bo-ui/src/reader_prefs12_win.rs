//! BlueReader round 12 (a child of `reader_prefs6_win`): the Preferences dialog laid out like
//! the reference reader's — a large dialog with a title bar and close button, the reference's
//! whole category list at the left, framed groups of options at the right. Options BlueReader
//! acts on are live; the reference's other options are shown grey so no page is empty.
use super::{check_mut, PrefsDialog, HISTORY, LAYOUTS, RATES, UNITS, ZOOMS};
use crate::reader_window::{rounded, ReaderWindow, MUTED, TEXT, WHITE};
use crate::Painter;
use bo_render::{PathSeg, Rgba};

/// A row of a group.
#[derive(Clone, Copy)]
pub(super) enum Row {
    /// An option BlueReader acts on (its id: check box, drop-down, name box or button).
    Live(&'static str),
    /// One of the reference's options BlueReader does not offer: label key and how it shows.
    Grey(&'static str, Grey),
}

/// How a grey option shows.
#[derive(Clone, Copy)]
pub(super) enum Grey {
    Check(bool),
    /// A drop-down showing this value (string key).
    Pick(&'static str),
    Text,
}

/// A framed group: title key and rows.
pub(super) struct Group {
    pub title: &'static str,
    pub rows: &'static [Row],
}

use Grey::{Check as GC, Pick as GP, Text as GT};
use Row::{Grey as G, Live as L};

/// The reference's categories (alphabetical, as its list) and their groups.
pub(super) const CATS: [(&str, &[Group]); 26] = [
    (
        "pdfPrefs12.cAccessibility",
        &[Group {
            title: "pdfPrefs12.gColors",
            rows: &[
                G("pdfPrefs12.oReplaceColors", GC(false)),
                G("pdfPrefs12.oHighContrast", GC(false)),
            ],
        }],
    ),
    (
        "pdfPrefs12.cCommenting",
        &[
            Group {
                title: "pdfPrefs12.gMaking",
                rows: &[
                    G("pdfPrefs12.oCopyEncircled", GC(true)),
                    G("pdfPrefs12.oCopySelected", GC(true)),
                ],
            },
            Group {
                title: "pdfPrefs12.gPopup",
                rows: &[
                    G("pdfPrefs12.oOpenPopup", GC(false)),
                    G("pdfPrefs12.oHidePopups", GC(false)),
                ],
            },
        ],
    ),
    (
        "pdfPrefs12.cDocuments",
        &[
            Group {
                title: "pdfPrefs12.gOpen",
                rows: &[
                    G("pdfPrefs12.oRestoreView", GC(false)),
                    G("pdfPrefs12.oMultiInstance", GC(false)),
                ],
            },
            Group {
                title: "pdfPrefs12.gSave",
                rows: &[G("pdfPrefs12.oIncremental", GC(true))],
            },
        ],
    ),
    (
        "pdfPrefs12.cEmail",
        &[Group {
            title: "pdfPrefs12.gEmail",
            rows: &[G("pdfPrefs12.oDefaultMail", GC(true))],
        }],
    ),
    (
        "pdfPrefs12.cFileAssoc",
        &[Group {
            title: "pdfPrefs12.gDefault",
            rows: &[L("setDefault")],
        }],
    ),
    (
        "pdfPrefs12.forms",
        &[Group {
            title: "pdfPrefs12.gGeneral",
            rows: &[
                L("formsHighlight"),
                G("pdfPrefs12.oAutoCalc", GC(true)),
                G("pdfPrefs12.oFocusRect", GC(true)),
            ],
        }],
    ),
    (
        "pdfPrefs12.fullScreen",
        &[
            Group {
                title: "pdfPrefs12.gFsNav",
                rows: &[L("fsEscape"), L("fsClick")],
            },
            Group {
                title: "pdfPrefs12.gFsLook",
                rows: &[
                    G("pdfPrefs12.oFsBackground", GP("pdfPrefs12.vBlack")),
                    G("pdfPrefs12.oFsNavBar", GC(false)),
                ],
            },
        ],
    ),
    (
        "pdfPrefs.general",
        &[
            Group {
                title: "pdfPrefs12.gTools",
                rows: &[
                    G("pdfPrefs12.oSingleKey", GC(false)),
                    G("pdfPrefs12.oFixedSnap", GC(false)),
                ],
            },
            Group {
                title: "pdfPrefs12.gScrolling",
                rows: &[L("smooth")],
            },
            Group {
                title: "pdfPrefs12.gStartup",
                rows: &[G("pdfPrefs12.oStartPage", GC(true))],
            },
        ],
    ),
    (
        "pdfPrefs12.history",
        &[Group {
            title: "pdfPrefs12.gHistory",
            rows: &[
                L("history"),
                L("clearHistory"),
                G("pdfPrefs12.oRestoreSession", GC(false)),
            ],
        }],
    ),
    (
        "pdfPrefs.identity",
        &[Group {
            title: "pdfPrefs12.gIdentity",
            rows: &[
                L("author"),
                G("pdfPrefs12.oOrganization", GT),
                G("pdfPrefs12.oEmail", GT),
            ],
        }],
    ),
    (
        "pdfPrefs12.cInternet",
        &[Group {
            title: "pdfPrefs12.gBrowser",
            rows: &[G("pdfPrefs12.oInBrowser", GC(false))],
        }],
    ),
    (
        "pdfPrefs12.cJavaScript",
        &[Group {
            title: "pdfPrefs12.gJs",
            rows: &[G("pdfPrefs12.oJs", GC(false))],
        }],
    ),
    (
        "pdfPrefs12.cLanguages",
        &[Group {
            title: "pdfPrefs12.gLanguage",
            rows: &[G("pdfPrefs12.oLanguage", GP("pdfPrefs12.vSystem"))],
        }],
    ),
    (
        "pdfPrefs.measure",
        &[Group {
            title: "pdfPrefs12.gUnits",
            rows: &[L("units")],
        }],
    ),
    (
        "pdfPrefs12.cMultimedia",
        &[Group {
            title: "pdfPrefs12.gPlayer",
            rows: &[G("pdfPrefs12.oPlayer", GP("pdfPrefs12.vNone"))],
        }],
    ),
    (
        "pdfPrefs.pageDisplay",
        &[
            Group {
                title: "pdfPrefs12.gLayoutZoom",
                rows: &[L("layout"), L("zoom")],
            },
            Group {
                title: "pdfPrefs12.gRendering",
                rows: &[
                    G("pdfPrefs12.oSmoothText", GC(true)),
                    G("pdfPrefs12.oSmoothLine", GC(true)),
                    G("pdfPrefs12.oSmoothImages", GC(true)),
                ],
            },
            Group {
                title: "pdfPrefs12.gResolution",
                rows: &[G("pdfPrefs12.oSystemResolution", GC(true))],
            },
        ],
    ),
    (
        "pdfPrefs12.cPdfSign",
        &[Group {
            title: "pdfPrefs12.gSigning",
            rows: &[G("pdfPrefs12.oSignAppearance", GP("pdfPrefs12.vStandard"))],
        }],
    ),
    (
        "pdfPrefs12.cReading",
        &[Group {
            title: "pdfPrefs12.gReadOrder",
            rows: &[G("pdfPrefs12.oReadOrder", GP("pdfPrefs12.vInfer"))],
        }],
    ),
    (
        "pdfPrefs12.cReviewing",
        &[Group {
            title: "pdfPrefs12.gReview",
            rows: &[G("pdfPrefs12.oReviewServer", GT)],
        }],
    ),
    (
        "pdfPrefs12.search",
        &[Group {
            title: "pdfPrefs12.gSearch",
            rows: &[
                L("searchWhole"),
                L("searchCase"),
                G("pdfPrefs12.oDiacritics", GC(false)),
            ],
        }],
    ),
    (
        "pdfPrefs12.cSecurity",
        &[Group {
            title: "pdfPrefs12.gSafe",
            rows: &[G("pdfPrefs12.oSafeReading", GC(true))],
        }],
    ),
    (
        "pdfPrefs12.cSignature",
        &[Group {
            title: "pdfPrefs12.gVerify",
            rows: &[G("pdfPrefs12.oVerifyOpen", GC(false))],
        }],
    ),
    (
        "pdfPrefs12.speech",
        &[Group {
            title: "pdfPrefs12.gVoice",
            rows: &[
                L("speechRate"),
                G("pdfPrefs12.oVoice", GP("pdfPrefs12.vSystem")),
            ],
        }],
    ),
    (
        "pdfPrefs12.cSpelling",
        &[Group {
            title: "pdfPrefs12.gSpelling",
            rows: &[G("pdfPrefs12.oSpellTyping", GC(false))],
        }],
    ),
    (
        "pdfPrefs12.cTrust",
        &[Group {
            title: "pdfPrefs12.gTrust",
            rows: &[G("pdfPrefs12.oAllowLinks", GC(false))],
        }],
    ),
    (
        "pdfPrefs12.cUpdater",
        &[Group {
            title: "pdfPrefs12.gUpdater",
            rows: &[G("pdfPrefs12.oAutoUpdate", GP("pdfPrefs12.vNever"))],
        }],
    ),
];

/// The category the dialog opens on (General, like the reference).
pub(super) const FIRST: usize = 7;
/// Text of options not offered (the disabled-command grey, exempt from WCAG 1.4.3).
const GREY: Rgba = Rgba(175, 175, 176, 255);
const FRAME: Rgba = Rgba(199, 204, 217, 255);
const LIST_SEL: Rgba = Rgba(204, 228, 247, 255);

impl ReaderWindow {
    fn live_label12(&self, d: &PrefsDialog, id: &str) -> (String, String) {
        match id {
            "zoom" => (
                self.t("pdfPrefs.zoom"),
                self.zoom_label(ZOOMS[d.edit.zoom].0, ZOOMS[d.edit.zoom].1),
            ),
            "layout" => (self.t("pdfPrefs.layout"), self.t(LAYOUTS[d.edit.layout].0)),
            "author" => (
                self.t("pdfPrefs.author"),
                if d.edit.author.is_empty() {
                    std::env::var("USERNAME").unwrap_or_default()
                } else {
                    d.edit.author.clone()
                },
            ),
            "units" => (self.t("pdfPrefs.units"), self.t(UNITS[d.edit.units].0)),
            "history" => (
                self.t("pdfPrefs12.historyMax"),
                HISTORY[d.edit.history.min(HISTORY.len() - 1)].to_string(),
            ),
            "speechRate" => (
                self.t("pdfPrefs12.rate"),
                self.t(RATES[d.edit.speech_rate.min(RATES.len() - 1)].0),
            ),
            "smooth" => (self.t("pdfPrefs.smooth"), String::new()),
            other => (self.t(&format!("pdfPrefs12.{other}")), String::new()),
        }
    }

    fn box12(&self, p: &mut dyn Painter, r: [f32; 4], on: bool, live: bool) {
        let edge = if live {
            Rgba(51, 51, 51, 255)
        } else {
            Rgba(190, 190, 190, 255)
        };
        p.fill_rect(r[0], r[1], r[2], r[3], edge);
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
        if on {
            let k = r[2] / 4.0;
            p.path(
                &[
                    PathSeg::M(r[0] + k, r[1] + 2.0 * k),
                    PathSeg::L(r[0] + 1.8 * k, r[1] + 3.0 * k),
                    PathSeg::L(r[0] + 3.2 * k, r[1] + k),
                ],
                None,
                Some((if live { TEXT } else { GREY }, self.px(1.6))),
            );
        }
    }

    fn field12(&self, p: &mut dyn Painter, r: [f32; 4], value: &str, live: bool, drop: bool) {
        let body = self.style(13.0, false);
        let line = body.size * 1.33;
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(190, 190, 190, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            if live {
                WHITE
            } else {
                Rgba(250, 250, 250, 255)
            },
        );
        p.text(
            value,
            r[0] + self.px(6.0),
            r[1] + ((r[3] - line) / 2.0).round(),
            body,
            if live { TEXT } else { GREY },
        );
        if drop {
            let (ax, ay) = (r[0] + r[2] - self.px(12.0), r[1] + r[3] / 2.0);
            let k = self.px(3.5);
            p.path(
                &[
                    PathSeg::M(ax - k, ay - k / 2.0),
                    PathSeg::L(ax + k, ay - k / 2.0),
                    PathSeg::L(ax, ay + k / 2.0),
                    PathSeg::Z,
                ],
                Some(if live { MUTED } else { GREY }),
                None,
            );
        }
    }

    /// Paint the dialog (round 12 layout) and record its hit boxes.
    #[allow(clippy::too_many_lines)]
    pub(super) fn paint_dialog12(&self, p: &mut dyn Painter, d: &mut PrefsDialog) {
        let (w, h) = self.size;
        p.fill_rect(0.0, 0.0, w, h, Rgba(0, 0, 0, 40));
        let (dw, dh) = (self.px(760.0), self.px(560.0).min(h - self.px(20.0)));
        let r = [((w - dw) / 2.0).round(), ((h - dh) / 2.0).round(), dw, dh];
        d.rect = r;
        d.hits.clear();
        p.fill_rect(
            r[0] - 1.0,
            r[1] - 1.0,
            r[2] + 2.0,
            r[3] + 2.0,
            Rgba(130, 130, 130, 255),
        );
        p.fill_rect(r[0], r[1], r[2], r[3], WHITE);
        // Title bar with the close button.
        let tb = self.px(25.6);
        p.fill_rect(r[0], r[1], r[2], tb, Rgba(243, 243, 243, 255));
        let title = self.style(15.0, false);
        let body = self.style(13.0, false);
        let line = body.size * 1.33;
        p.text(
            &self.t("pdfPrefs.title"),
            r[0] + self.px(6.4),
            r[1] + self.px(4.0),
            title,
            TEXT,
        );
        let cb = [r[0] + r[2] - tb - self.px(4.0), r[1], tb + self.px(4.0), tb];
        let (cx, cy, k) = (cb[0] + cb[2] / 2.0, cb[1] + cb[3] / 2.0, self.px(4.8));
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((TEXT, 1.0)),
        );
        d.hits.push((cb, "cancel".to_owned()));
        // The category list: white, framed, rows 21 px at 125 %.
        let bottom = r[1] + r[3] - self.px(52.0);
        let lx = r[0] + self.px(12.0);
        let ly = r[1] + tb + self.px(12.0);
        let lw = self.px(168.0);
        let lh = bottom - ly;
        p.fill_rect(lx, ly, lw, lh, FRAME);
        p.fill_rect(lx + 1.0, ly + 1.0, lw - 2.0, lh - 2.0, WHITE);
        let row = ((lh - self.px(4.0)) / CATS.len() as f32).min(self.px(16.8));
        for (k, (key, _)) in CATS.iter().enumerate() {
            let rr = [lx + 2.0, ly + 2.0 + k as f32 * row, lw - 4.0, row];
            if k == d.page {
                p.fill_rect(rr[0], rr[1], rr[2], rr[3], LIST_SEL);
            }
            p.text(
                &self.t(key),
                rr[0] + self.px(6.0),
                rr[1] + ((rr[3] - line) / 2.0).round(),
                body,
                TEXT,
            );
            d.hits.push((rr, format!("page.{k}")));
        }
        // Groups of the chosen category.
        let gx = lx + lw + self.px(14.0);
        let gw = r[0] + r[2] - gx - self.px(12.0);
        let rh = self.px(26.0);
        let mut y = ly;
        for g in CATS[d.page.min(CATS.len() - 1)].1 {
            let gh = self.px(22.0) + g.rows.len() as f32 * (rh + self.px(6.0)) + self.px(6.0);
            // Frame with its title over the top edge.
            let fy = y + line / 2.0;
            p.fill_rect(gx, fy, gw, gh - line / 2.0, FRAME);
            p.fill_rect(gx + 1.0, fy + 1.0, gw - 2.0, gh - line / 2.0 - 2.0, WHITE);
            let t = self.t(g.title);
            let tw = self.width_of(&t, body);
            p.fill_rect(gx + self.px(8.0), y, tw + self.px(8.0), line, WHITE);
            p.text(&t, gx + self.px(12.0), y, body, TEXT);
            let mut ry = y + self.px(22.0);
            let rx = gx + self.px(14.0);
            let vx = gx + self.px(250.0);
            let vw = (gx + gw - vx - self.px(14.0)).max(self.px(80.0));
            for row in g.rows {
                let ty = ry + ((rh - line) / 2.0).round();
                let b = self.px(13.0);
                match *row {
                    L(id) if super::CHECKS.contains(&id) => {
                        let on = check_mut(&mut d.edit.clone(), id).is_some_and(|c| *c);
                        self.box12(p, [rx, ry + (rh - b) / 2.0, b, b], on, true);
                        let (label, _) = self.live_label12(d, id);
                        p.text(&label, rx + b + self.px(8.0), ty, body, TEXT);
                        d.hits
                            .push(([rx, ry, gw - self.px(28.0), rh], id.to_owned()));
                    }
                    L(id @ ("clearHistory" | "setDefault")) => {
                        let label = self.t(&format!("pdfPrefs12.{id}"));
                        let bw = self.width_of(&label, body) + self.px(28.0);
                        let bb = [rx, ry, bw, rh];
                        p.path(
                            &rounded(bb[0], bb[1], bb[2], bb[3], self.px(3.0)),
                            Some(Rgba(230, 233, 240, 255)),
                            None,
                        );
                        p.text(&label, rx + self.px(14.0), ty, body, TEXT);
                        d.hits.push((bb, id.to_owned()));
                    }
                    L(id) => {
                        let (label, value) = self.live_label12(d, id);
                        p.text(&label, rx, ty, body, TEXT);
                        let f = [vx, ry, vw, rh];
                        self.field12(p, f, &value, true, id != "author");
                        d.hits.push((f, id.to_owned()));
                    }
                    G(key, Grey::Check(on)) => {
                        self.box12(p, [rx, ry + (rh - b) / 2.0, b, b], on, false);
                        p.text(&self.t(key), rx + b + self.px(8.0), ty, body, GREY);
                    }
                    G(key, Grey::Pick(v)) => {
                        p.text(&self.t(key), rx, ty, body, GREY);
                        self.field12(p, [vx, ry, vw, rh], &self.t(v), false, true);
                    }
                    G(key, Grey::Text) => {
                        p.text(&self.t(key), rx, ty, body, GREY);
                        self.field12(p, [vx, ry, vw, rh], "", false, false);
                    }
                }
                ry += rh + self.px(6.0);
            }
            y += gh + self.px(12.0);
        }
        // OK / Cancel at the bottom right.
        let (bw, bh) = (self.px(84.0), self.px(27.2));
        let by = r[1] + r[3] - bh - self.px(13.0);
        for (k, (key, id)) in [("pdfReader.ok", "ok"), ("pdfPrint.cancel", "cancel")]
            .iter()
            .enumerate()
        {
            let bx = r[0] + r[2] - (2.0 - k as f32) * (bw + self.px(9.6)) - self.px(3.0);
            let primary = k == 0;
            p.path(
                &rounded(bx, by, bw, bh, self.px(3.0)),
                Some(if primary { self.theme.accent } else { WHITE }),
                if primary {
                    None
                } else {
                    Some((Rgba(190, 190, 190, 255), 1.0))
                },
            );
            let label = self.t(key);
            let tw = self.width_of(&label, body);
            p.text(
                &label,
                (bx + (bw - tw) / 2.0).round(),
                by + ((bh - line) / 2.0).round(),
                body,
                if primary { WHITE } else { TEXT },
            );
            d.hits.push(([bx, by, bw, bh], (*id).to_owned()));
        }
    }
}
