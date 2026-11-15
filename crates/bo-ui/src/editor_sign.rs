//! BlueEditor signing (PAdES through `bo-sign`): after the certificate dialog the signature is
//! placed by dragging a box on a page, drawn with the signer's name (or a picture), "Digitally
//! signed by", the name and the date (`bo_pdfread::signature_appearance`), optionally with a
//! time-stamp from a time-stamp authority (baseline B-T). The Signatures panel lists the
//! document's signatures checked with `bo_sign::pdf::verify_pdf`: a green mark when the
//! signature is valid and the document unchanged since, amber when it changed after signing,
//! red when the signature does not verify.
#![allow(clippy::single_match_else)]
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_window::{inside, EditorWindow};
use crate::reader_window::{ExtraPanel, ReaderRequest};
use crate::{Input, Painter};
use bo_render::{PathSeg, Rgba};

/// Green, amber and red status marks of the Signatures panel.
pub(crate) const SIG_VALID: Rgba = Rgba(0x2E, 0x7D, 0x32, 255);
pub(crate) const SIG_CHANGED: Rgba = Rgba(0xD9, 0x8E, 0x04, 255);
pub(crate) const SIG_INVALID: Rgba = Rgba(0xC6, 0x28, 0x28, 255);

/// A signature waiting to be placed on a page.
pub(crate) struct Pending {
    pub signer: bo_sign::SoftwareSigner,
    pub reason: String,
    pub location: String,
    /// Time-stamp authority URL (empty: none).
    pub tsa: String,
    /// Picture drawn instead of the name (PNG or JPEG).
    pub picture: Option<Vec<u8>>,
}

/// One row of the Signatures panel.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SigRow {
    pub color: Rgba,
    pub title: String,
    pub detail: String,
    /// Lines of the details dialog.
    pub lines: Vec<String>,
}

/// Signing state of the window.
#[derive(Default)]
pub(crate) struct SignState {
    pub pending: Option<Pending>,
    /// Box being dragged: page, start and current point (page points).
    pub drag: Option<crate::editor_actions::Marking>,
    /// Panel rows of the document bytes they were made from (address, length).
    rows: Option<((usize, usize), Vec<SigRow>)>,
}

/// `YYYY.MM.DD hh:mm:ss UTC` of Unix seconds.
fn date_line(secs: u64) -> String {
    let iso = bo_sign::iso_utc(secs);
    // 2026-11-15T21:30:05Z → 2026.11.15 21:30:05 UTC
    format!(
        "{} {} UTC",
        iso.get(..10).unwrap_or("").replace('-', "."),
        iso.get(11..19).unwrap_or("")
    )
}

impl EditorWindow {
    /// The signature dialog was answered: keep the signer and let the user drag its box.
    pub(crate) fn sign_answer(&mut self, cert: &[u8], values: &[String]) {
        let get = |i: usize| values.get(i).cloned().unwrap_or_default();
        match bo_sign::SoftwareSigner::from_pkcs12(cert, &get(0)) {
            Ok(signer) => {
                let picture = Some(get(4))
                    .filter(|p| !p.trim().is_empty())
                    .and_then(|p| std::fs::read(p.trim().trim_matches('"')).ok());
                self.sig.pending = Some(Pending {
                    signer,
                    reason: get(1),
                    location: get(2),
                    tsa: get(3).trim().to_owned(),
                    picture,
                });
                self.set_status(&self.t("pdfEditor.placeSignature"));
            }
            Err(e) => self.set_status(&e.to_string()),
        }
    }

    /// Pointer input while a signature box is being placed.
    pub(crate) fn sign_input(&mut self, input: &Input) -> bool {
        if self.sig.pending.is_none() {
            return false;
        }
        let area = self.r.doc_area();
        match *input {
            Input::Press { x, y, .. } if inside(area, x, y) => {
                if let Some((page, px, py)) = self.view().and_then(|v| v.window_to_doc(x, y)) {
                    self.sig.drag = Some((page, (px, py), (px, py)));
                }
                true
            }
            Input::Drag { x, y } => {
                let Some((page, a, _)) = self.sig.drag else {
                    return false;
                };
                let p = self.view().and_then(|v| {
                    let pr = v.page_rect(page)?;
                    let s = v.scale();
                    let (qx, qy) = ((x - pr[0]) / s, (y - pr[1]) / s);
                    let r = v.unrotate_rect(page, [qx, qy, qx, qy]);
                    Some((r[0], r[1]))
                });
                if let Some(p) = p {
                    self.sig.drag = Some((page, a, p));
                }
                true
            }
            Input::Release { .. } => {
                if let Some((page, a, b)) = self.sig.drag.take() {
                    let r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
                    // A click without a drag places a box of the usual size.
                    let r = if r[2] - r[0] < 8.0 || r[3] - r[1] < 8.0 {
                        [r[0], r[1], r[0] + 180.0, r[1] + 54.0]
                    } else {
                        r
                    };
                    if let Some(p) = self.sig.pending.take() {
                        let tsa = (!p.tsa.is_empty()).then(|| {
                            bo_sign::http::HttpTimestampClient::new(
                                p.tsa.clone(),
                                Box::new(bo_sign::http::UreqTransport::new(
                                    std::time::Duration::from_secs(20),
                                )),
                            )
                        });
                        let r = self.sign_placed(
                            &p,
                            page,
                            r,
                            tsa.as_ref()
                                .map(|t| t as &dyn bo_sign::timestamp::TimestampClient),
                        );
                        if let Err(e) = r {
                            self.set_status(&e);
                        }
                    }
                }
                true
            }
            Input::Key(crate::Key::Escape, _) => {
                self.sig.pending = None;
                self.sig.drag = None;
                true
            }
            _ => false,
        }
    }

    /// Sign the active document (as saved now) with a visible signature in `rect` (page
    /// points) of `page`; the signed copy opens in a new tab and is offered for saving.
    pub(crate) fn sign_placed(
        &mut self,
        p: &Pending,
        page: usize,
        rect: [f32; 4],
        tsa: Option<&dyn bo_sign::timestamp::TimestampClient>,
    ) -> Result<(), String> {
        use bo_sign::Signer as _;
        let bytes = self.with_doc(bo_pdfread::PdfDocument::save)?;
        let user = self.with_doc(|d| {
            d.page_rect_to_user(page, rect)
                .ok_or_else(|| "page".to_owned())
        })?;
        let name = bo_sign::trust::certificate_subject(p.signer.certificate_der())
            .map_or_else(|| "?".to_owned(), |s| bo_sign::trust::common_name(&s));
        let now = std::time::SystemTime::now();
        let secs = now
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut lines = vec![self.t("pdfEditor.signedBy"), name.clone()];
        lines.push(format!(
            "{}: {}",
            self.t("pdfEditor.signDate"),
            date_line(secs)
        ));
        if !p.reason.is_empty() {
            lines.push(format!("{}: {}", self.t("pdfEditor.signReason"), p.reason));
        }
        let look = bo_pdfread::signature_appearance(
            user[2] - user[0],
            user[3] - user[1],
            &name,
            &lines,
            p.picture.as_deref(),
        )?;
        let visible = bo_sign::pdf::VisibleSignature {
            page,
            rect: user,
            content: look.content,
            image: look.image,
        };
        let opts = bo_sign::pdf::SignOptions {
            name: Some(name),
            reason: (!p.reason.is_empty()).then(|| p.reason.clone()),
            location: (!p.location.is_empty()).then(|| p.location.clone()),
            time: now,
            ..bo_sign::pdf::SignOptions::default()
        };
        let signed = bo_sign::pdf::sign_pdf_visible(&bytes, &p.signer, &opts, tsa, &visible)
            .map_err(|e| e.to_string())?;
        let file = format!("{}-signed.pdf", self.doc_stem());
        self.set_status(&self.t("pdfEditor.signed"));
        self.push(ReaderRequest::SaveBytes {
            name: file.clone(),
            bytes: signed.clone(),
        });
        self.push(ReaderRequest::OpenDocument {
            name: file,
            bytes: signed,
        });
        Ok(())
    }

    /// Rows of the Signatures panel for the active document (verified once per file state).
    pub(crate) fn signature_rows(&mut self) -> Vec<SigRow> {
        let Some(v) = self.view() else {
            return Vec::new();
        };
        let bytes = v.doc().bytes();
        let key = (bytes.as_ptr() as usize, bytes.len());
        if let Some((k, rows)) = &self.sig.rows {
            if *k == key {
                return rows.clone();
            }
        }
        let rows = match bo_sign::pdf::verify_pdf(bytes) {
            Ok(list) => list
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let name = bo_sign::trust::common_name(&s.signer.subject);
                    let intact = s.covers_whole_file || s.only_validation_data_after;
                    let (color, state) = if intact {
                        (SIG_VALID, self.t("pdfEditor.sigValid"))
                    } else {
                        (SIG_CHANGED, self.t("pdfEditor.sigChanged"))
                    };
                    let mut lines = vec![
                        format!("{}: {}", self.t("pdfEditor.signer"), s.signer.subject),
                        state.clone(),
                    ];
                    let detail = match &s.signer.timestamp {
                        Some(ts) => {
                            let d = date_line(ts.gen_time_unix);
                            lines.push(format!(
                                "{}: {d} ({})",
                                self.t("pdfEditor.timestamp"),
                                bo_sign::trust::common_name(&ts.tsa_subject)
                            ));
                            format!("{state} · {d}")
                        }
                        None => {
                            lines.push(self.t("pdfEditor.noTimestamp"));
                            state
                        }
                    };
                    lines.push(self.t("pdfEditor.identityUnknown"));
                    SigRow {
                        color,
                        title: format!("{} {}: {name}", self.t("pdfEditor.signature"), i + 1),
                        detail,
                        lines,
                    }
                })
                .collect(),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("no signature") || !bytes.windows(8).any(|w| w == b"/ByteRan") {
                    Vec::new()
                } else {
                    vec![SigRow {
                        color: SIG_INVALID,
                        title: self.t("pdfEditor.signature"),
                        detail: self.t("pdfEditor.sigInvalid"),
                        lines: vec![self.t("pdfEditor.sigInvalid"), msg],
                    }]
                }
            }
        };
        self.sig.rows = Some((key, rows.clone()));
        rows
    }

    /// Keep the Signatures panel's rows in step with the active document.
    pub(crate) fn sync_signatures(&mut self) {
        if !self.r.showing_doc() {
            self.r.extra_panel = None;
            return;
        }
        let rows = self.signature_rows();
        self.r.extra_panel = Some(ExtraPanel {
            nav_id: "nav.signatures",
            icon: "sign",
            title: self.t("pdfEditor.signatures"),
            empty: self.t("pdfEditor.noSignatures"),
            rows: rows
                .into_iter()
                .enumerate()
                .map(|(i, r)| (r.color, r.title, r.detail, format!("sig.detail.{i}")))
                .collect(),
        });
    }

    /// Signature commands; false when `cmd` is not one.
    pub(crate) fn sign_command(&mut self, cmd: &str) -> bool {
        if cmd == "prot.sigPanel" {
            self.r.toggle_panel(crate::reader_window::Panel::Extra);
            return true;
        }
        let Some(i) = cmd
            .strip_prefix("sig.detail.")
            .and_then(|s| s.parse::<usize>().ok())
        else {
            return false;
        };
        let Some(row) = self.signature_rows().get(i).cloned() else {
            return true;
        };
        let mut d = Dialog::new(
            DialogKind::Comment,
            row.title,
            Vec::new(),
            vec![(self.t("pdfReader.ok"), DialogButton::Ok)],
            row.lines,
        );
        d.layout(
            &crate::editor_window::Estimate,
            self.r.window_size(),
            self.r.ui_scale(),
        );
        self.dialog2 = Some((d, crate::editor_actions::Ask2::Info));
        true
    }

    /// The box being dragged for a signature.
    pub(crate) fn paint_sign(&mut self, p: &mut dyn Painter) {
        let Some((page, a, b)) = self.sig.drag else {
            return;
        };
        let accent = self.r.theme().accent;
        let Some(v) = self.view() else { return };
        let r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
        if let Some(w) = v.doc_rect_to_window(page, r) {
            let (x0, y0, x1, y1) = (
                w[0].round() + 0.5,
                w[1].round() + 0.5,
                (w[0] + w[2]).round() - 0.5,
                (w[1] + w[3]).round() - 0.5,
            );
            p.fill_rect(
                w[0],
                w[1],
                w[2],
                w[3],
                Rgba(accent.0, accent.1, accent.2, 30),
            );
            p.path(
                &[
                    PathSeg::M(x0, y0),
                    PathSeg::L(x1, y0),
                    PathSeg::L(x1, y1),
                    PathSeg::L(x0, y1),
                    PathSeg::Z,
                ],
                None,
                Some((accent, 1.0)),
            );
        }
    }
}
