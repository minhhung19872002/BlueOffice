//! BlueDoc's Review tab of parity round 16 (docs/ui/REPORT.md "Vòng 106"), laid out as the
//! reference word processor's: Proofing (Spelling & Grammar, Thesaurus, Word Count),
//! Accessibility, Language, Comments (New Comment, Delete ▾, Previous, Next, Show Comments),
//! Markup (the display box, Show Markup ▾, Reviewing Pane ▾ and the Change Tracking Options
//! launcher), Tracking (Track Changes ▾, Accept ▾, Reject ▾, Previous, Next), Compare and
//! Protect. At 1500 px (125 %) Comments and Tracking collapse to one drop-down button each,
//! as the reference's; the drop-down shows the whole group (`doc_grouppop16`).
use crate::catalog::Entry;
use crate::icons::Icons;
use crate::ribbon::{Group, Item, Tab};
use bo_render::Rgba;

const fn e(id: &'static str, label: &'static str) -> Entry {
    Entry { id, label }
}

/// Collapsed group buttons of the Review tab.
pub(crate) const GROUP_IDS: [&str; 3] = [
    "review.commentsGroup",
    "review.trackingGroup",
    "review.protectGroup",
];

fn comments_group() -> Group {
    use Item::Large as L;
    Group {
        label: "ribbon.groups.comments",
        large: vec![
            L("review.newComment"),
            L("review.deleteMenu"),
            L("review.previousComment"),
            L("review.nextComment"),
            L("review.showComments"),
        ],
        rows: vec![],
    }
}

fn tracking_group() -> Group {
    use Item::{Labeled as S, Large as L};
    Group {
        label: "ribbon.groups.tracking",
        large: vec![
            L("review.trackChanges"),
            L("review.acceptMenu"),
            L("review.rejectMenu"),
        ],
        rows: vec![vec![S("review.previous")], vec![S("review.next")]],
    }
}

/// The whole group behind a collapsed group button.
pub(crate) fn full_group(id: &str) -> Option<Group> {
    match id {
        "review.commentsGroup" => Some(comments_group()),
        "review.trackingGroup" => Some(tracking_group()),
        "review.protectGroup" => Some(Group {
            label: "docR3.groupProtect",
            large: vec![Item::Large("review.restrictEditing")],
            rows: vec![],
        }),
        _ => None,
    }
}

/// The Review tab.
pub(crate) fn review_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.review",
        groups: vec![
            Group {
                label: "ribbon.groups.proofing",
                large: vec![L("review.spellingDialog")],
                rows: vec![vec![S("review.thesaurus")], vec![S("review.wordCount")]],
            },
            // Speech › Read Aloud (the platform's voice, round 16).
            Group {
                label: "doc16.groupSpeech",
                large: vec![L("review.readAloud")],
                rows: vec![],
            },
            // Check Accessibility opens the Accessibility pane (round 7).
            Group {
                label: "docR7.g.accessibility",
                large: vec![L("review.checkAccessibility")],
                rows: vec![],
            },
            Group {
                label: "docR3.groupLanguage",
                large: vec![L("review.languageMenu")],
                rows: vec![],
            },
            comments_group(),
            // Markup: the display box (Simple / All / No Markup / Original), the marks shown
            // and the Reviewing Pane, with Change Tracking Options in the corner.
            Group {
                label: "doc16.groupMarkup",
                large: vec![],
                rows: vec![
                    vec![S("review.markupMode")],
                    vec![S("review.showMarkup")],
                    vec![S("review.reviewingPane")],
                ],
            },
            tracking_group(),
            Group {
                label: "docR3.groupCompare",
                large: vec![L("review.compareMenu")],
                rows: vec![],
            },
            Group {
                label: "docR3.groupProtect",
                large: vec![L("review.restrictEditing")],
                rows: vec![],
            },
        ],
    }
}

/// Collapse steps of the Review tab, in the reference's order.
pub(crate) fn scaling(tab: &str) -> &'static [&'static str] {
    match tab {
        "ribbon.tabs.review" => &[
            "ribbon.groups.comments",
            "ribbon.groups.tracking",
            "docR3.groupProtect",
        ],
        _ => &[],
    }
}

/// Group `label` collapsed (one drop-down button).
pub(crate) fn shrink(label: &str, _stage: u8) -> Option<Group> {
    use Item::Large as L;
    let one = |label: &'static str, id: &'static str| Group {
        label,
        large: vec![L(id)],
        rows: vec![],
    };
    match label {
        "ribbon.groups.comments" => Some(one("ribbon.groups.comments", "review.commentsGroup")),
        "ribbon.groups.tracking" => Some(one("ribbon.groups.tracking", "review.trackingGroup")),
        "docR3.groupProtect" => Some(one("docR3.groupProtect", "review.protectGroup")),
        _ => None,
    }
}

/// The collapsed group button holding command `id` (its menus open under that button).
pub(crate) fn group_of(id: &str) -> Option<&'static str> {
    GROUP_IDS
        .iter()
        .find(|g| full_group(g).is_some_and(|grp| grp_has(&grp, id)))
        .copied()
}

fn grp_has(g: &Group, id: &str) -> bool {
    let has =
        |i: &Item| matches!(i, Item::Large(x) | Item::Labeled(x) | Item::Button(x) if *x == id);
    g.large.iter().any(has) || g.rows.iter().flatten().any(has)
}

/// Fixed widths (logical px) of Review row items the window draws: the markup display box
/// (icon + 182 px box at 125 % on the reference).
pub(crate) fn labeled_w(id: &str) -> Option<f32> {
    (id == "review.markupMode").then_some(170.0)
}

/// Menus of round 16 (entries: id, i18n key).
pub(crate) fn menu(id: &str) -> Vec<Entry> {
    match id {
        "review.deleteMenu" => vec![
            e("review.deleteComment", "doc16.delete"),
            e("review.deleteShown", "doc16.deleteShown"),
            e("review.deleteAll", "doc16.deleteAll"),
        ],
        // Shown in their own drop-down (`doc_grouppop16`); the entries mark them as menus.
        "review.commentsGroup" => vec![
            e("review.newComment", "commands.review.newComment"),
            e("review.showComments", "commands.review.showComments"),
        ],
        "review.trackingGroup" => vec![
            e("review.trackChanges", "commands.review.trackChanges"),
            e("review.acceptMenu", "commands.review.acceptMenu"),
        ],
        "review.protectGroup" => vec![e(
            "review.restrictEditing",
            "commands.review.restrictEditing",
        )],
        _ => Vec::new(),
    }
}

/// Labels (id → English) of round-16 commands; translations under `doc16.cmd.<id with _>`.
pub(crate) const LABELS: [(&str, &str); 18] = [
    ("review.readAloud", "Read Aloud"),
    ("review.protectGroup", "Protect"),
    ("review.thesaurus", "Thesaurus"),
    ("review.commentsGroup", "Comments"),
    ("review.trackingGroup", "Tracking"),
    ("review.deleteMenu", "Delete"),
    ("review.previousComment", "Previous"),
    ("review.nextComment", "Next"),
    ("review.restrictEditing", "Restrict Editing"),
    ("review.trackingOptions", "Change Tracking Options"),
    ("review.markupSimple", "Simple Markup"),
    ("review.markupAll", "All Markup"),
    ("review.markupNone", "No Markup"),
    ("review.markupOriginal", "Original"),
    ("review.markupComments", "Comments"),
    ("review.markupInsDel", "Insertions and Deletions"),
    ("review.markupFormatting", "Formatting"),
    ("review.markupBalloons", "Balloons"),
];

/// Ids of this module (`&'static` for the ribbon and menus).
pub(crate) const IDS: [&str; 10] = [
    "review.readAloud",
    "review.protectGroup",
    "review.thesaurus",
    "review.commentsGroup",
    "review.trackingGroup",
    "review.deleteMenu",
    "review.trackingOptions",
    "review.deleteShown",
    "review.deleteAll",
    "review.restrictEditing",
];

/// The `&'static` id of a round-16 id.
pub(crate) fn static_id(id: &str) -> Option<&'static str> {
    IDS.iter().find(|m| **m == id).copied()
}

/// English labels into `commands.<id>` (translations under `doc16.cmd.*`).
pub(crate) fn install_strings(s: &mut crate::strings::Strings) {
    let english = s.get("ribbon.tabs.home") == "Home";
    for (id, en) in LABELS {
        let key = format!("doc16.cmd.{}", id.replace('.', "_"));
        let v = s.get(&key).to_owned();
        if v != key {
            s.set(&format!("commands.{id}"), &v);
        } else if english {
            s.set(&format!("commands.{id}"), en);
        }
    }
}

/// Icons of round 16 (own drawings in the BlueDoc icon family).
pub(crate) fn install_icons(icons: &mut Icons) {
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    let grey = rgb(0x6e, 0x6e, 0x6e);
    let blue = rgb(0x2f, 0x6f, 0xba);
    let white = rgb(255, 255, 255);
    // Thesaurus: an open book with a blue bookmark ribbon.
    icons.add_solid(
        "review.thesaurus",
        &[
            ("M3 5h8v14h-8ZM13 5h8v14h-8Z", white),
            (
                "M3 5h8.5v14h-8.5ZM4 6v12h7v-12ZM12.5 5h8.5v14h-8.5ZM13.5 6v12h6.5v-12Z",
                grey,
            ),
            ("M15 6h2.4v6l-1.2-1l-1.2 1Z", blue),
            ("M5 8h4v1h-4ZM5 11h4v1h-4ZM5 14h4v1h-4Z", grey),
        ],
    );
    icons.alias("review.commentsGroup", "review.newComment");
    icons.alias("review.trackingGroup", "review.trackChanges");
    icons.alias("review.protectGroup", "review.protectMenu");
    // Read Aloud: a speaker with sound waves (own drawing).
    icons.add_solid(
        "review.readAloud",
        &[
            ("M4 9h4l5-4v14l-5-4h-4Z", grey),
            ("M15.5 8.5a5 5 0 0 1 0 7l-1-1a3.6 3.6 0 0 0 0-5Z", blue),
            ("M18 6a8.5 8.5 0 0 1 0 12l-1-1a7 7 0 0 0 0-10Z", blue),
        ],
    );
    icons.alias("review.deleteMenu", "review.resolveComment");
    icons.alias("review.deleteShown", "review.resolveComment");
    icons.alias("review.deleteAll", "review.resolveComment");
    icons.alias("review.trackingOptions", "launcher");
    // Finish & Merge › Send Email Messages: an envelope (own drawing).
    icons.add_solid(
        "mailings.finish.email",
        &[
            ("M3 7h18v11h-18Z", white),
            ("M3 7h18v11h-18ZM4 8.4v8.6h16v-8.6l-8 5Z", grey),
            ("M4.5 8h15l-7.5 4.7Z", blue),
        ],
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn collapsed_groups_hold_their_commands() {
        assert_eq!(
            super::group_of("review.deleteMenu"),
            Some("review.commentsGroup")
        );
        assert_eq!(
            super::group_of("review.acceptMenu"),
            Some("review.trackingGroup")
        );
        assert_eq!(super::group_of("review.compareMenu"), None);
        assert!(crate::catalog::is_menu("review.deleteMenu"));
        assert!(crate::catalog::is_menu("review.commentsGroup"));
    }
}
