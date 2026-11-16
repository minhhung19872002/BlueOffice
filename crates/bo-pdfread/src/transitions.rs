//! Page transitions for presentations (ISO 32000-1 §12.4.4, Table 162 `/Trans`): the effect a
//! viewer shows when it moves to a page in full screen. The kinds and directions are the ones
//! the reference editor's Page Transitions dialog offers; speeds are its Slow / Medium / Fast.
use crate::PdfDocument;
use lopdf::{Dictionary, Object};

/// A transition style (`/S`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    /// Two lines sweep across the screen (`/Split`).
    Split,
    /// Several lines sweep across the screen (`/Blinds`).
    Blinds,
    /// A rectangle sweeps in or out (`/Box`).
    Box,
    /// A single line sweeps across (`/Wipe`).
    Wipe,
    /// The old page dissolves gradually (`/Dissolve`).
    Dissolve,
    /// Like dissolve, sweeping across (`/Glitter`).
    Glitter,
    /// The new page pushes the old one away (`/Push`).
    Push,
    /// The new page slides over the old one (`/Cover`).
    Cover,
    /// The old page slides off (`/Uncover`).
    Uncover,
    /// The new page fades in (`/Fade`).
    Fade,
    /// The new page simply replaces the old (`/R`).
    Replace,
    /// Changes fly in or out (`/Fly`).
    Fly,
}

impl TransitionKind {
    /// Every kind, in the reference dialog's order.
    pub const ALL: [TransitionKind; 12] = [
        Self::Split,
        Self::Blinds,
        Self::Box,
        Self::Wipe,
        Self::Dissolve,
        Self::Glitter,
        Self::Push,
        Self::Cover,
        Self::Uncover,
        Self::Fade,
        Self::Replace,
        Self::Fly,
    ];

    /// The `/S` name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Split => "Split",
            Self::Blinds => "Blinds",
            Self::Box => "Box",
            Self::Wipe => "Wipe",
            Self::Dissolve => "Dissolve",
            Self::Glitter => "Glitter",
            Self::Push => "Push",
            Self::Cover => "Cover",
            Self::Uncover => "Uncover",
            Self::Fade => "Fade",
            Self::Replace => "R",
            Self::Fly => "Fly",
        }
    }

    fn from_name(n: &[u8]) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name().as_bytes() == n)
    }

    /// Directions this kind takes (in the order offered).
    #[must_use]
    pub fn directions(self) -> &'static [TransitionDirection] {
        use TransitionDirection as D;
        match self {
            Self::Split => &[
                D::HorizontalIn,
                D::HorizontalOut,
                D::VerticalIn,
                D::VerticalOut,
            ],
            Self::Blinds => &[D::Horizontal, D::Vertical],
            Self::Box | Self::Fly => &[D::In, D::Out],
            Self::Wipe | Self::Push | Self::Cover | Self::Uncover => &[
                D::LeftToRight,
                D::TopToBottom,
                D::RightToLeft,
                D::BottomToTop,
            ],
            Self::Glitter => &[D::LeftToRight, D::TopToBottom, D::TopLeftToBottomRight],
            Self::Dissolve | Self::Fade | Self::Replace => &[],
        }
    }
}

/// Direction of a transition (`/Dm`, `/M`, `/Di`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionDirection {
    /// Split: horizontal, inwards.
    HorizontalIn,
    /// Split: horizontal, outwards.
    HorizontalOut,
    /// Split: vertical, inwards.
    VerticalIn,
    /// Split: vertical, outwards.
    VerticalOut,
    /// Blinds: horizontal.
    Horizontal,
    /// Blinds: vertical.
    Vertical,
    /// Box, Fly: inwards.
    In,
    /// Box, Fly: outwards.
    Out,
    /// `/Di 0`.
    LeftToRight,
    /// `/Di 270`.
    TopToBottom,
    /// `/Di 180`.
    RightToLeft,
    /// `/Di 90`.
    BottomToTop,
    /// `/Di 315` (Glitter).
    TopLeftToBottomRight,
}

/// A page's transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// Style.
    pub kind: TransitionKind,
    /// Direction (ignored by kinds without one).
    pub direction: Option<TransitionDirection>,
    /// Duration in seconds (`/D`).
    pub seconds: f32,
}

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn dict_of(t: &Transition) -> Dictionary {
    use TransitionDirection as D;
    let mut d = Dictionary::new();
    d.set("Type", name("Trans"));
    d.set("S", name(t.kind.name()));
    d.set("D", Object::Real(t.seconds.clamp(0.1, 60.0)));
    let dir = t
        .direction
        .filter(|x| t.kind.directions().contains(x))
        .or_else(|| t.kind.directions().first().copied());
    match dir {
        Some(D::HorizontalIn | D::HorizontalOut | D::VerticalIn | D::VerticalOut) => {
            let h = matches!(dir, Some(D::HorizontalIn | D::HorizontalOut));
            let i = matches!(dir, Some(D::HorizontalIn | D::VerticalIn));
            d.set("Dm", name(if h { "H" } else { "V" }));
            d.set("M", name(if i { "I" } else { "O" }));
        }
        Some(D::Horizontal) => d.set("Dm", name("H")),
        Some(D::Vertical) => d.set("Dm", name("V")),
        Some(D::In) => d.set("M", name("I")),
        Some(D::Out) => d.set("M", name("O")),
        Some(D::LeftToRight) => d.set("Di", 0),
        Some(D::BottomToTop) => d.set("Di", 90),
        Some(D::RightToLeft) => d.set("Di", 180),
        Some(D::TopToBottom) => d.set("Di", 270),
        Some(D::TopLeftToBottomRight) => d.set("Di", 315),
        None => {}
    }
    d
}

impl PdfDocument {
    /// Set (or with `None` remove) the transition of `pages` (one undo step).
    pub fn set_page_transitions(
        &mut self,
        pages: &[usize],
        t: Option<&Transition>,
    ) -> Result<(), String> {
        if pages.iter().any(|&p| p >= self.pages.len()) {
            return Err("no such page".into());
        }
        self.begin("page transitions");
        for &p in pages {
            let id = self.pages[p].id;
            self.touch(id);
            let page = self.doc.get_dictionary_mut(id).map_err(|e| e.to_string())?;
            match t {
                Some(t) => page.set("Trans", dict_of(t)),
                None => {
                    page.remove(b"Trans");
                }
            }
        }
        self.commit();
        Ok(())
    }

    /// The transition of page `index`, when it has one.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn page_transition(&self, index: usize) -> Option<Transition> {
        use TransitionDirection as D;
        let id = self.pages.get(index)?.id;
        let page = self.doc.get_dictionary(id).ok()?;
        let d = match page.get(b"Trans").ok()? {
            Object::Reference(r) => self.doc.get_dictionary(*r).ok()?,
            Object::Dictionary(d) => d,
            _ => return None,
        };
        let kind = TransitionKind::from_name(d.get(b"S").ok()?.as_name().ok()?)?;
        let n = |k: &[u8]| d.get(k).ok().and_then(|o| o.as_name().ok());
        let di = d.get(b"Di").ok().and_then(|o| o.as_float().ok());
        let direction = match kind {
            TransitionKind::Split => Some(match (n(b"Dm"), n(b"M")) {
                (Some(b"V"), Some(b"O")) => D::VerticalOut,
                (Some(b"V"), _) => D::VerticalIn,
                (_, Some(b"O")) => D::HorizontalOut,
                _ => D::HorizontalIn,
            }),
            TransitionKind::Blinds => Some(if n(b"Dm") == Some(b"V") {
                D::Vertical
            } else {
                D::Horizontal
            }),
            TransitionKind::Box | TransitionKind::Fly => {
                Some(if n(b"M") == Some(b"O") { D::Out } else { D::In })
            }
            _ => di.map(|a| match a.round() as i64 {
                90 => D::BottomToTop,
                180 => D::RightToLeft,
                270 => D::TopToBottom,
                315 => D::TopLeftToBottomRight,
                _ => D::LeftToRight,
            }),
        };
        let seconds = d
            .get(b"D")
            .ok()
            .and_then(|o| o.as_float().ok())
            .unwrap_or(1.0);
        Some(Transition {
            kind,
            direction,
            seconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standard_fonts;

    fn doc() -> PdfDocument {
        let bytes = crate::tests::build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >>",
        ]);
        PdfDocument::open(bytes, standard_fonts()).expect("opens")
    }

    #[test]
    fn transitions_are_written_read_back_saved_and_undone() {
        let mut d = doc();
        let t = Transition {
            kind: TransitionKind::Wipe,
            direction: Some(TransitionDirection::TopToBottom),
            seconds: 2.0,
        };
        d.set_page_transitions(&[1], Some(&t)).expect("sets");
        assert_eq!(d.page_transition(0), None);
        assert_eq!(d.page_transition(1), Some(t));
        let saved = d.save().expect("saves");
        let s = String::from_utf8_lossy(&saved);
        assert!(s.contains("/Trans"), "{s}");
        let again = PdfDocument::open(saved, standard_fonts()).expect("opens");
        assert_eq!(again.page_transition(1), Some(t));
        // Split keeps both its keys; a direction another kind does not take falls back.
        let split = Transition {
            kind: TransitionKind::Split,
            direction: Some(TransitionDirection::VerticalOut),
            seconds: 0.5,
        };
        d.set_page_transitions(&[0, 1], Some(&split)).expect("sets");
        assert_eq!(d.page_transition(0), Some(split));
        let odd = Transition {
            kind: TransitionKind::Box,
            direction: Some(TransitionDirection::LeftToRight),
            seconds: 1.0,
        };
        d.set_page_transitions(&[0], Some(&odd)).expect("sets");
        assert_eq!(
            d.page_transition(0).and_then(|t| t.direction),
            Some(TransitionDirection::In)
        );
        d.set_page_transitions(&[0, 1], None).expect("removes");
        assert_eq!(d.page_transition(1), None);
        assert!(d.undo());
        assert_eq!(d.page_transition(1), Some(split));
    }
}
