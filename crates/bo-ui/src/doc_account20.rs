//! File › Account of BlueDoc (round 20, docs/ui/REPORT.md "Vòng 110"): the reference File
//! screen's two columns — **User Information** (the user name and initials of File ›
//! Options › General, a round picture with the initials) and **Product Information**
//! (BlueDoc's name, version and licence, BlueOffice's own mark). BlueOffice has no sign-in;
//! the user is the one comments and tracked changes are written with. A child module of
//! `doc_backstage6`.
use super::{tr, DocBackstage, INK, MUTED};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

/// A circle of radius `r` about (`cx`, `cy`) as cubic segments.
fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = r * 0.552_284_8;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

/// Draw the Account page.
pub(super) fn paint(b: &DocBackstage, p: &mut dyn Painter, t: &Theme, s: &Strings) {
    let cx = b.content_x();
    let y = b.top + b.px(84.0);
    let body = b.body();
    let small = TextStyle {
        size: b.px(12.0),
        bold: false,
    };
    let big = TextStyle {
        size: b.px(14.5),
        bold: false,
    };
    // User Information.
    b.heading(
        p,
        &tr(s, "doc20.userInfo", "User Information"),
        cx,
        y,
        17.0,
        INK,
    );
    let (name, initials) = &b.account;
    let initials = if initials.trim().is_empty() {
        name.split_whitespace()
            .filter_map(|w| w.chars().next())
            .flat_map(char::to_uppercase)
            .take(2)
            .collect::<String>()
    } else {
        initials.trim().to_owned()
    };
    let r = b.px(36.0);
    let (ax, ay) = (cx + r, y + b.px(52.0) + r);
    p.path(&circle(ax, ay, r), Some(t.accent_strong), None);
    let label = if initials.is_empty() { "?" } else { &initials };
    let st = TextStyle {
        size: b.px(22.0),
        bold: false,
    };
    let w = label.chars().count() as f32 * st.size * 0.62;
    p.text(
        label,
        ax - w / 2.0,
        ay - st.size * 0.66,
        st,
        Rgba(255, 255, 255, 255),
    );
    let shown = if name.trim().is_empty() {
        tr(s, "doc20.noUser", "No user name set")
    } else {
        name.clone()
    };
    p.text(&shown, ax + r + b.px(18.0), ay - b.px(22.0), big, INK);
    p.text(
        &tr(
            s,
            "doc20.userHint",
            "Shown on your comments and tracked changes.",
        ),
        ax + r + b.px(18.0),
        ay + b.px(4.0),
        small,
        MUTED,
    );
    // Product Information.
    let px = cx + b.px(500.0);
    b.heading(
        p,
        &tr(s, "doc20.productInfo", "Product Information"),
        px,
        y,
        17.0,
        INK,
    );
    // BlueOffice's own mark: a blue square with a white page fold.
    let m = b.px(44.0);
    let (mx, my) = (px, y + b.px(52.0));
    p.fill_rect(mx, my, m, m, t.accent_strong);
    p.path(
        &[
            PathSeg::M(mx + m * 0.28, my + m * 0.2),
            PathSeg::L(mx + m * 0.6, my + m * 0.2),
            PathSeg::L(mx + m * 0.74, my + m * 0.34),
            PathSeg::L(mx + m * 0.74, my + m * 0.8),
            PathSeg::L(mx + m * 0.28, my + m * 0.8),
            PathSeg::Z,
        ],
        Some(Rgba(255, 255, 255, 255)),
        None,
    );
    let lines = [
        (
            tr(s, "doc20.product", "BlueDoc – Soạn thảo văn bản"),
            big,
            INK,
        ),
        (
            format!(
                "{} {}",
                tr(s, "doc20.version", "Version"),
                env!("CARGO_PKG_VERSION")
            ),
            body,
            INK,
        ),
        (
            tr(s, "doc20.suite", "BlueOffice · Bluestar Technology"),
            body,
            MUTED,
        ),
        (
            tr(
                s,
                "doc20.compat",
                "Compatible with Microsoft Word documents (.docx, .doc)",
            ),
            small,
            MUTED,
        ),
        (tr(s, "doc20.noSignIn", "No sign-in needed."), small, MUTED),
    ];
    let mut ly = my;
    for (text, st, c) in lines {
        p.text(&text, mx + m + b.px(16.0), ly, st, c);
        ly += st.size * 1.33 + b.px(6.0);
    }
}
