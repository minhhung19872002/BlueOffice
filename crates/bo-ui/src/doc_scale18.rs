//! Ribbon scaling of every BlueDoc tab (round 18, docs/ui/REPORT.md "Vòng 108"): as the
//! window narrows, groups collapse stage by stage in the order measured on the reference
//! (UI Automation of the reference's ribbon at 1500…600 px, 50 px steps, 125 %), down to
//! one drop-down button per group, so every group stays reachable without scroll arrows.
//! A collapsed group's button opens the whole group in a drop-down panel
//! (`doc_grouppop16`). Builds on the stages of rounds 4 and 16 (`doc_r4_fit`,
//! `doc_review16`).
use crate::catalog::Entry;
use crate::ribbon::{Group, Item};

/// Collapsed-group buttons of round 18: (button id, tab, group label, icon of).
pub(crate) const GROUPS: [(&str, &str, &str, &str); 20] = [
    (
        "r18g.clipboard",
        "ribbon.tabs.home",
        "ribbon.groups.clipboard",
        "edit.paste",
    ),
    (
        "r18g.font",
        "ribbon.tabs.home",
        "ribbon.groups.font",
        "format.bold",
    ),
    (
        "r18g.paragraph",
        "ribbon.tabs.home",
        "ribbon.groups.paragraph",
        "format.alignLeft",
    ),
    (
        "r18g.styles",
        "ribbon.tabs.home",
        "ribbon.groups.styles",
        "style.normal",
    ),
    (
        "r18g.editing",
        "ribbon.tabs.home",
        "ribbon.groups.editing",
        "edit.find",
    ),
    (
        "r18g.illustrations",
        "ribbon.tabs.insert",
        "ribbon.groups.illustrations",
        "insert.picture",
    ),
    (
        "r18g.text",
        "ribbon.tabs.insert",
        "docR4.groupText",
        "insert.textBox",
    ),
    (
        "r18g.docFormatting",
        "ribbon.tabs.design",
        "ribbon.groups.documentFormatting",
        "design.themes",
    ),
    (
        "r18g.pageSetup",
        "ribbon.tabs.layout",
        "ribbon.groups.pageSetup",
        "layout.margins",
    ),
    (
        "r18g.layoutParagraph",
        "ribbon.tabs.layout",
        "docR4.groupParagraph",
        "format.indent",
    ),
    (
        "r18g.toc",
        "ribbon.tabs.references",
        "ribbon.groups.toc",
        "references.insertToc",
    ),
    (
        "r18g.footnotes",
        "ribbon.tabs.references",
        "ribbon.groups.footnotes",
        "insert.footnote",
    ),
    (
        "r18g.citations",
        "ribbon.tabs.references",
        "docR3.groupCitations",
        "references.insertCitation",
    ),
    (
        "r18g.captions",
        "ribbon.tabs.references",
        "ribbon.groups.captions",
        "references.insertCaption",
    ),
    (
        "r18g.index",
        "ribbon.tabs.references",
        "docR3.groupIndex",
        "references.markEntry",
    ),
    (
        "r18g.authorities",
        "ribbon.tabs.references",
        "doc14.groupAuthorities",
        "references.markCitation",
    ),
    (
        "r18g.create",
        "ribbon.tabs.mailings",
        "docR3.groupCreate",
        "mailings.envelopes",
    ),
    (
        "r18g.mergeStart",
        "ribbon.tabs.mailings",
        "ribbon.groups.mergeData",
        "mailings.startMerge",
    ),
    (
        "r18g.writeFields",
        "ribbon.tabs.mailings",
        "docR3.groupWriteFields",
        "mailings.addressBlock",
    ),
    (
        "r18g.preview",
        "ribbon.tabs.mailings",
        "ribbon.groups.mergePreview",
        "mailings.preview",
    ),
];

/// More collapsed-group buttons (Review and View).
pub(crate) const GROUPS2: [(&str, &str, &str, &str); 6] = [
    (
        "r18g.proofing",
        "ribbon.tabs.review",
        "ribbon.groups.proofing",
        "review.spellingDialog",
    ),
    (
        "r18g.markup",
        "ribbon.tabs.review",
        "doc16.groupMarkup",
        "review.reviewingPane",
    ),
    (
        "r18g.views",
        "ribbon.tabs.view",
        "ribbon.groups.views",
        "view.printLayout",
    ),
    (
        "r18g.show",
        "ribbon.tabs.view",
        "ribbon.groups.show",
        "view.ruler",
    ),
    (
        "r18g.zoom",
        "ribbon.tabs.view",
        "ribbon.groups.zoom",
        "view.zoomDialog",
    ),
    (
        "r18g.pageMovement",
        "ribbon.tabs.view",
        "docR4.groupPageMovement",
        "view.vertical",
    ),
];

fn all() -> impl Iterator<Item = &'static (&'static str, &'static str, &'static str, &'static str)>
{
    GROUPS.iter().chain(GROUPS2.iter())
}

/// Whether `id` is one of the collapsed-group buttons of this module.
pub(crate) fn is_group(id: &str) -> bool {
    all().any(|g| g.0 == id)
}

/// The tab and group label of collapsed-group button `id`.
pub(crate) fn group(id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    all().find(|g| g.0 == id).map(|g| (g.0, g.1, g.2))
}

/// Collapse steps per tab, in the reference's order (a group named again collapses one
/// stage further; its last stage is one drop-down button).
pub(crate) fn scaling(tab: &str) -> &'static [&'static str] {
    match tab {
        "ribbon.tabs.home" => &[
            "ribbon.groups.editing",
            "ribbon.groups.styles",
            "ribbon.groups.paragraph",
            "ribbon.groups.font",
            "ribbon.groups.clipboard",
        ],
        "ribbon.tabs.insert" => &[
            "ribbon.groups.pages",
            "ribbon.groups.links",
            "docR4.groupText",
            "ribbon.groups.symbols",
            "ribbon.groups.illustrations",
            "docR4.groupText",
            "ribbon.groups.headerFooter",
            "ribbon.groups.headerFooter",
            "ribbon.groups.illustrations",
        ],
        "ribbon.tabs.design" => &[
            "ribbon.groups.documentFormatting",
            "ribbon.groups.documentFormatting",
            "ribbon.groups.pageBackground",
            "ribbon.groups.documentFormatting",
        ],
        "ribbon.tabs.layout" => &[
            "docR4.groupArrange",
            "ribbon.groups.pageSetup",
            "docR4.groupArrange",
            "docR4.groupArrange",
            "docR4.groupParagraph",
            "ribbon.groups.pageSetup",
        ],
        "ribbon.tabs.references" => &[
            "ribbon.groups.captions",
            "ribbon.groups.footnotes",
            "ribbon.groups.toc",
            "doc14.groupAuthorities",
            "docR3.groupIndex",
            "ribbon.groups.captions",
            "docR3.groupCitations",
            "ribbon.groups.footnotes",
            "ribbon.groups.toc",
        ],
        "ribbon.tabs.mailings" => &[
            "docR3.groupWriteFields",
            "ribbon.groups.mergeData",
            "docR3.groupCreate",
            "ribbon.groups.mergePreview",
            "docR3.groupWriteFields",
            "ribbon.groups.mergeData",
        ],
        "ribbon.tabs.review" => &[
            "ribbon.groups.comments",
            "ribbon.groups.tracking",
            "docR3.groupProtect",
            "doc16.groupMarkup",
            "ribbon.groups.proofing",
        ],
        "ribbon.tabs.view" => &[
            "ribbon.groups.zoom",
            "docR4.groupWindow",
            "ribbon.groups.show",
            "ribbon.groups.zoom",
            "docR4.groupWindow",
            "ribbon.groups.views",
            "ribbon.groups.views",
            "docR4.groupPageMovement",
        ],
        _ => crate::doc_r4_fit::scaling(tab),
    }
}

/// Stages rounds 4 and 16 define for group `label` of `tab` (round 18 adds one more: the
/// drop-down button).
fn old_stages(tab: &str, label: &str) -> usize {
    crate::doc_r4_fit::scaling(tab)
        .iter()
        .filter(|l| **l == label)
        .count()
}

/// Group `label` at collapse stage `stage`.
pub(crate) fn shrink(label: &str, stage: u8) -> Option<Group> {
    // Page Background: its three buttons in labeled rows (the reference never collapses it
    // further).
    if label == "ribbon.groups.pageBackground" {
        use Item::Labeled as S;
        return Some(Group {
            label: "ribbon.groups.pageBackground",
            large: vec![],
            rows: vec![
                vec![S("design.watermark")],
                vec![S("design.pageColor")],
                vec![S("design.pageBorders")],
            ],
        });
    }
    if let Some(&(id, tab, glabel, _)) = all().find(|g| g.2 == label) {
        let old = old_stages(tab, label);
        let n = usize::from(stage);
        if n <= old {
            return crate::doc_r4_fit::shrink(label, stage);
        }
        let kinds = extra(label);
        return match kinds.get(n - old - 1).copied().unwrap_or(Kind::Button) {
            Kind::Button => Some(Group {
                label: glabel,
                large: vec![Item::Large(id)],
                rows: vec![],
            }),
            Kind::Icons => crate::ribbon::doc_group(tab, label).map(icons),
            Kind::Rows => crate::ribbon::doc_group(tab, label).map(rows),
            Kind::Custom(k) => custom(label, k),
        };
    }
    crate::doc_r4_fit::shrink(label, stage)
}

/// A collapse stage round 18 adds after those of rounds 4 and 16.
#[derive(Clone, Copy)]
enum Kind {
    /// Small labeled buttons lose their labels.
    Icons,
    /// Large buttons become small labeled buttons in rows of three.
    Rows,
    /// A stage drawn by [`custom`].
    Custom(u8),
    /// The whole group as one drop-down button.
    Button,
}

/// Stages of a group after rounds 4 / 16's (measured on the reference; the last is the
/// drop-down button).
fn extra(label: &str) -> &'static [Kind] {
    match label {
        "ribbon.groups.documentFormatting" => &[Kind::Custom(1), Kind::Custom(2), Kind::Button],
        "ribbon.groups.toc"
        | "ribbon.groups.footnotes"
        | "ribbon.groups.captions"
        | "docR3.groupWriteFields" => &[Kind::Icons, Kind::Button],
        "ribbon.groups.mergeData" => &[Kind::Rows, Kind::Button],
        _ => &[Kind::Button],
    }
}

/// `g` with its labeled small buttons as icons only.
fn icons(mut g: Group) -> Group {
    for row in &mut g.rows {
        for item in row.iter_mut() {
            if let Item::Labeled(id) = *item {
                *item = Item::Button(id);
            }
        }
    }
    g
}

/// `g` with its large buttons as small labeled buttons, three to a column.
fn rows(mut g: Group) -> Group {
    let mut items: Vec<Item> = g
        .large
        .drain(..)
        .filter_map(|i| match i {
            Item::Large(id) => Some(Item::Labeled(id)),
            _ => None,
        })
        .collect();
    items.extend(g.rows.drain(..).flatten());
    // One column of up to three rows, then the next (rows read across).
    let per = items.len().clamp(1, 3);
    g.rows = (0..per)
        .map(|r| items.iter().skip(r).step_by(per).copied().collect())
        .collect();
    g
}

/// Custom stages (Design's Document Formatting: fewer style sets, then the style sets as
/// one gallery button, as the reference's Style Set gallery).
fn custom(label: &str, stage: u8) -> Option<Group> {
    use Item::{Labeled as S, Large as L};
    let rows = || {
        vec![
            vec![S("design.paragraphSpacing")],
            vec![S("design.effects")],
            vec![S("design.setDefault")],
        ]
    };
    match (label, stage) {
        ("ribbon.groups.documentFormatting", 1) => Some(Group {
            label: "ribbon.groups.documentFormatting",
            large: vec![
                L("design.themes"),
                L("design.styleSet.standard"),
                L("design.styleSet.capital"),
                L("design.styleSet.lotus"),
                L("design.styleSet.river"),
                L("design.colors"),
                L("design.fonts"),
            ],
            rows: rows(),
        }),
        ("ribbon.groups.documentFormatting", 2) => Some(Group {
            label: "ribbon.groups.documentFormatting",
            large: vec![
                L("design.themes"),
                L("design.styleSetMore"),
                L("design.colors"),
                L("design.fonts"),
            ],
            rows: rows(),
        }),
        _ => None,
    }
}

/// The collapsed group button holding command `id` (its menus open under that button).
pub(crate) fn group_of(id: &str) -> Option<&'static str> {
    crate::doc_r4_fit::group_of(id).or_else(|| holders().get(id).copied())
}

/// Times the holder map was built (tests: once per process).
#[cfg(test)]
static BUILDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Command id → first collapsed-group button holding it, built once. Round 19: `group_of`
/// runs for every ribbon rectangle looked up while painting; building all eight tabs per
/// lookup cost 3–5 ms a frame (typing guard, docs/ui/REPORT.md "Vòng 109").
fn holders() -> &'static std::collections::HashMap<&'static str, &'static str> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static str>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        #[cfg(test)]
        BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut map = std::collections::HashMap::new();
        for g in all() {
            let Some(grp) = crate::ribbon::doc_group(g.1, g.2) else {
                continue;
            };
            for i in grp.large.iter().chain(grp.rows.iter().flatten()) {
                if let Item::Large(x) | Item::Labeled(x) | Item::Button(x) = i {
                    map.entry(*x).or_insert(g.0);
                }
            }
        }
        map
    })
}

/// A collapsed-group button is a drop-down (its label carries the mark); the panel shows
/// the group itself.
pub(crate) fn menu(id: &str) -> Vec<Entry> {
    all()
        .find(|g| g.0 == id)
        .map(|g| {
            vec![Entry {
                id: g.3,
                label: g.2,
            }]
        })
        .unwrap_or_default()
}

/// Labels of the buttons: the group's own name.
pub(crate) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, _, label, _) in all() {
        let v = s.get(label).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
}

/// Icons of the buttons: the group's main command.
pub(crate) fn install_icons(icons: &mut crate::icons::Icons) {
    for (id, _, _, of) in all() {
        icons.alias(id, of);
    }
    // Spelling & Grammar (round 18): a bolder own drawing — a solid letter A with a check
    // mark, as heavy as the reference's ribbon icon (the thin outline read too light).
    let ink = Some(bo_render::Rgba(0x44, 0x44, 0x44, 255));
    let blue = Some(bo_render::Rgba(0x2B, 0x57, 0x9A, 255));
    icons.add_solid(
        "review.spellingDialog",
        &[
            (
                "M3 16L8 3h2.6l5 13h-2.7l-1.2-3.3H7l-1.2 3.3ZM7.8 10.4h3.6L9.6 5.4Z",
                ink,
            ),
            ("M9.5 18.2l2.1-2.1l2.4 2.4l5.9-5.9l2.1 2.1l-8 8Z", blue),
        ],
    );
    // The group button of a collapsed Proofing group shows the same picture.
    icons.alias("r18g.proofing", "review.spellingDialog");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_group_button_names_a_real_group_and_its_tab_collapses_to_it() {
        for (id, tab, label, _) in all() {
            assert!(crate::ribbon::doc_group(tab, label).is_some(), "{id}");
            let stages = scaling(tab).iter().filter(|l| *l == label).count();
            let last = shrink(label, u8::try_from(stages).expect("stages")).expect("last");
            assert_eq!(last.large, vec![Item::Large(id)], "{id}");
            assert!(is_group(id) && !menu(id).is_empty());
        }
        assert_eq!(group_of("format.bold"), Some("r18g.font"));
        assert_eq!(group_of("insert.coverPage"), Some("insert.pagesGroup"));
    }

    /// Round 19 typing guard: painting looks up holders of every off-tab ribbon rectangle;
    /// the lookup must not rebuild the tabs each call.
    #[test]
    fn holder_lookup_is_built_once() {
        for _ in 0..1000 {
            assert_eq!(group_of("design.colors"), group_of("design.colors"));
            let _ = group_of("no.such.command");
        }
        assert_eq!(BUILDS.load(std::sync::atomic::Ordering::Relaxed), 1);
        // Same answers as a scan of every group in order (first group wins).
        for (id, tab, label, _) in all() {
            let grp = crate::ribbon::doc_group(tab, label).expect("group");
            for i in grp.large.iter().chain(grp.rows.iter().flatten()) {
                if let Item::Large(x) | Item::Labeled(x) | Item::Button(x) = i {
                    let first = all()
                        .find(|g| {
                            crate::ribbon::doc_group(g.1, g.2).is_some_and(|g2| {
                                g2.large
                                    .iter()
                                    .chain(g2.rows.iter().flatten())
                                    .any(|j| j == i)
                            })
                        })
                        .map(|g| g.0);
                    assert!(holders().get(x).copied() == first, "{id} {x}");
                }
            }
        }
    }
}
