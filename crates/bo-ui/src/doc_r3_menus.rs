//! Menus, labels and icons of BlueDoc's round-3 commands (Paste Options, picture Arrange,
//! References, Review, Mailings, View): menu ids, their entries in the reference's order
//! and casing, and BlueOffice's own icon drawings for the new buttons.
use crate::catalog::Entry;
use crate::icons::Icons;
use bo_render::Rgba;

/// Menu buttons added in round 3 (`&'static` ids to anchor their menus).
pub(crate) const MENUS: [&str; 12] = [
    "edit.paste",
    "picture.wrap",
    "picture.position",
    "picture.bringForward",
    "picture.sendBackward",
    "picture.align",
    "picture.rotate",
    "references.toc",
    "references.insertCitation",
    "references.insertBibliography",
    "references.citationStyle",
    "review.compareMenu",
];

/// More menu buttons (Review › Language / Protect).
pub(crate) const MORE_MENUS: [&str; 2] = ["review.languageMenu", "review.protectMenu"];

/// The `&'static str` of a round-3 menu id.
pub(crate) fn static_id(id: &str) -> Option<&'static str> {
    MENUS
        .iter()
        .chain(MORE_MENUS.iter())
        .chain(BUTTONS.iter())
        .find(|m| **m == id)
        .copied()
}

/// Buttons of round 3 that are not menus.
pub(crate) const BUTTONS: [&str; 16] = [
    "view.stylesPane",
    "view.outline",
    "references.manageSources",
    "references.markEntry",
    "references.insertIndex",
    "references.crossReference",
    "references.notesDialog",
    "references.tableOfFigures",
    "mailings.envelopes",
    "mailings.labels",
    "review.restrictEditing",
    "picture.group",
    "table.viewGridlines",
    "outline.promote",
    "outline.demote",
    "outline.close",
];

/// Outlining tab commands (Outline view).
pub(crate) const OUTLINE: [&str; 8] = [
    "outline.promoteHeading",
    "outline.promote",
    "outline.demote",
    "outline.demoteBody",
    "outline.moveUp",
    "outline.moveDown",
    "outline.showLevel",
    "outline.close",
];

const fn e(id: &'static str, label: &'static str) -> Entry {
    Entry { id, label }
}

/// Entries of round-3 menu `id` (labels are i18n keys of `docR3.*`).
pub(crate) fn menu(id: &str) -> Vec<Entry> {
    match id {
        "edit.paste" => vec![
            e("paste.keep", "docR3.keepSourceK"),
            e("paste.merge", "docR3.mergeFormattingM"),
            e("paste.text", "docR3.keepTextT"),
            e("paste.special", "docR3.pasteSpecialDots"),
            e("paste.defaults", "docR3.setDefaultPaste"),
        ],
        // §20.4.2.8 inline, §20.4.2.17–20 wrap elements, behindDoc.
        "picture.wrap" => vec![
            e("picture.wrap.inline", "docR3.inLine"),
            e("picture.wrap.square", "docR3.square"),
            e("picture.wrap.tight", "docR3.tight"),
            e("picture.wrap.through", "docR3.through"),
            e("picture.wrap.topAndBottom", "docR3.topAndBottom"),
            e("picture.wrap.behind", "docR3.behindText"),
            e("picture.wrap.inFront", "docR3.inFrontOfText"),
        ],
        "picture.position" => vec![
            e("picture.pos.inline", "docR3.inLine"),
            e("picture.pos.tl", "docR3.posTl"),
            e("picture.pos.tc", "docR3.posTc"),
            e("picture.pos.tr", "docR3.posTr"),
            e("picture.pos.ml", "docR3.posMl"),
            e("picture.pos.mc", "docR3.posMc"),
            e("picture.pos.mr", "docR3.posMr"),
            e("picture.pos.bl", "docR3.posBl"),
            e("picture.pos.bc", "docR3.posBc"),
            e("picture.pos.br", "docR3.posBr"),
        ],
        "picture.bringForward" => vec![
            e("picture.order.forward", "docR3.bringForward"),
            e("picture.order.front", "docR3.bringToFront"),
            e("picture.wrap.inFront", "docR3.bringInFrontOfText"),
        ],
        "picture.sendBackward" => vec![
            e("picture.order.backward", "docR3.sendBackward"),
            e("picture.order.back", "docR3.sendToBack"),
            e("picture.wrap.behind", "docR3.sendBehindText"),
        ],
        "picture.align" => vec![
            e("picture.align.left", "docR3.alignLeft"),
            e("picture.align.center", "docR3.alignCenter"),
            e("picture.align.right", "docR3.alignRight"),
            e("picture.align.top", "docR3.alignTop"),
            e("picture.align.middle", "docR3.alignMiddle"),
            e("picture.align.bottom", "docR3.alignBottom"),
            e("picture.alignTo.page", "docR3.alignToPage"),
            e("picture.alignTo.margin", "docR3.alignToMargin"),
        ],
        "picture.rotate" => vec![
            e("picture.rotate.right", "docR3.rotateRight"),
            e("picture.rotate.left", "docR3.rotateLeft"),
            e("picture.flip.vertical", "docR3.flipVertical"),
            e("picture.flip.horizontal", "docR3.flipHorizontal"),
        ],
        "references.toc" => vec![
            e("references.toc.auto1", "docR3.tocAuto1"),
            e("references.toc.auto2", "docR3.tocAuto2"),
            e("references.toc.manual", "docR3.tocManual"),
            e("references.toc.custom", "docR3.tocCustom"),
            e("references.toc.remove", "docR3.tocRemove"),
        ],
        "references.insertCitation" => vec![
            e("references.citation.new", "docR3.addNewSource"),
            e("references.citation.placeholder", "docR3.addNewPlaceholder"),
        ],
        "references.insertBibliography" => vec![
            e("references.bib.bibliography", "docR3.bibBibliography"),
            e("references.bib.references", "docR3.bibReferences"),
            e("references.bib.worksCited", "docR3.bibWorksCited"),
            e("references.bib.insert", "docR3.insertBibliography"),
        ],
        "references.citationStyle" => STYLES.iter().map(|(id, label)| e(id, label)).collect(),
        "review.compareMenu" => vec![
            e("review.compare.compare", "docR3.compareDots"),
            e("review.compare.combine", "docR3.combineDots"),
        ],
        "review.languageMenu" => vec![e("review.language.set", "docR3.setProofingLanguage")],
        "review.protectMenu" => vec![e("review.restrictEditing", "docR3.restrictEditing")],
        "outline.showLevel" => vec![
            e("outline.level.1", "docR3.level1"),
            e("outline.level.2", "docR3.level2"),
            e("outline.level.3", "docR3.level3"),
            e("outline.level.4", "docR3.level4"),
            e("outline.level.5", "docR3.level5"),
            e("outline.level.all", "docR3.allLevels"),
        ],
        _ => Vec::new(),
    }
}

/// Citation styles of the engine (`bo_edit::cite::StyleId`) as menu entries.
pub(crate) const STYLES: [(&str, &str); 8] = [
    ("references.style.APA", "APA"),
    ("references.style.MLA", "MLA"),
    ("references.style.Chicago", "Chicago"),
    ("references.style.IEEE", "IEEE"),
    ("references.style.Harvard", "Harvard"),
    ("references.style.ISO690", "ISO 690"),
    ("references.style.ISO690Numeric", "ISO 690 (numeric)"),
    ("references.style.VNThesis", "VN thesis"),
];

/// Menu entries shown dimmed (the engine has no stacking order of floating pictures yet,
/// pictures are not flipped and groups need two pictures).
pub(crate) fn dim(id: &str) -> bool {
    matches!(
        id,
        "picture.order.forward"
            | "picture.order.front"
            | "picture.order.backward"
            | "picture.order.back"
            | "picture.flip.vertical"
            | "picture.flip.horizontal"
            | "picture.group"
            | "references.citation.placeholder"
    )
}

/// Menu icons (own drawings, see [`install_icons`]).
pub(crate) fn icon(id: &str) -> Option<&'static str> {
    Some(match id {
        "picture.wrap.inline" | "picture.pos.inline" => "r3.wrapInline",
        "picture.wrap.square" => "r3.wrapSquare",
        "picture.wrap.tight" => "r3.wrapTight",
        "picture.wrap.through" => "r3.wrapThrough",
        "picture.wrap.topAndBottom" => "r3.wrapTopBottom",
        "picture.wrap.behind" => "r3.wrapBehind",
        "picture.wrap.inFront" => "r3.wrapFront",
        "picture.order.forward" | "picture.order.front" => "picture.bringForward",
        "picture.order.backward" | "picture.order.back" => "picture.sendBackward",
        "picture.align.left" => "format.alignLeft",
        "picture.align.center" => "format.alignCenter",
        "picture.align.right" => "format.alignRight",
        "picture.align.top" | "picture.align.middle" | "picture.align.bottom" => "picture.align",
        "picture.rotate.right" | "picture.rotate.left" => "picture.rotate",
        "references.citation.new" => "references.manageSources",
        "references.bib.insert" => "references.insertBibliography",
        "references.toc.custom" => "references.toc",
        "review.compare.compare" | "review.compare.combine" => "review.compareMenu",
        "review.language.set" => "review.languageMenu",
        "review.restrictEditing" => "review.protectMenu",
        _ => return None,
    })
}

/// Separators before these entries.
pub(crate) fn separator(id: &str) -> bool {
    matches!(
        id,
        "picture.wrap.behind"
            | "picture.align.top"
            | "picture.alignTo.page"
            | "picture.flip.vertical"
            | "references.toc.custom"
            | "references.bib.insert"
            | "references.citation.new"
            | "paste.special"
    )
}

/// Ribbon labels of the round-3 buttons in the reference's casing (`docR3.cmd.*` copied to
/// `commands.<id>`, so the shared interface keeps its own wording).
pub(crate) fn install_strings(s: &mut crate::strings::Strings) {
    for id in MENUS
        .iter()
        .chain(MORE_MENUS.iter())
        .chain(BUTTONS.iter())
        .chain(OUTLINE.iter())
        .chain(
            [
                "references.insertCaption",
                "review.restrictEditing",
                "design.effects",
                "design.setDefault",
            ]
            .iter(),
        )
    {
        let key = format!("docR3.cmd.{}", id.replace('.', "_"));
        let v = s.get(&key).to_owned();
        if v != key {
            s.set(&format!("commands.{id}"), &v);
        }
    }
}

/// Own icon shapes (24-unit SVG paths, filled like the rest of BlueDoc's ribbon) for the
/// round-3 buttons, and aliases to existing icons.
pub(crate) fn install_icons(icons: &mut Icons) {
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    let grey = rgb(0x6e, 0x6e, 0x6e);
    let blue = rgb(0x2f, 0x6f, 0xba);
    let pale = rgb(0xcf, 0xe2, 0xf5);
    let white = rgb(255, 255, 255);
    let page = "M5 2h14v20h-14Z";
    let lines_full = "M7 5h10v1h-10ZM7 8h10v1h-10ZM7 11h10v1h-10ZM7 14h10v1h-10ZM7 17h10v1h-10Z";
    // Wrap modes: a page of lines with a picture box placed and the text around it.
    icons.add_solid(
        "r3.wrapInline",
        &[
            (page, white),
            ("M5 2h14v20h-14ZM6 3v18h12v-18Z", grey),
            (
                "M7 5h10v1h-10ZM7 8h3v1h-3ZM7 15h10v1h-10ZM7 18h10v1h-10Z",
                grey,
            ),
            ("M10.5 7h6v6h-6Z", blue),
            ("M11.5 8h4v4h-4Z", pale),
        ],
    );
    icons.add_solid(
        "r3.wrapSquare",
        &[
            (page, white),
            ("M5 2h14v20h-14ZM6 3v18h12v-18Z", grey),
            ("M7 5h10v1h-10ZM7 8h2v1h-2ZM7 11h2v1h-2ZM7 14h2v1h-2ZM7 17h10v1h-10ZM15 8h2v1h-2ZM15 11h2v1h-2ZM15 14h2v1h-2Z", grey),
            ("M10 7.5h4v7h-4Z", blue),
        ],
    );
    icons.add_solid(
        "r3.wrapTight",
        &[
            (page, white),
            ("M5 2h14v20h-14ZM6 3v18h12v-18Z", grey),
            ("M7 5h10v1h-10ZM7 8h3v1h-3ZM7 11h2v1h-2ZM7 14h3v1h-3ZM7 17h10v1h-10ZM14 8h3v1h-3ZM15 11h2v1h-2ZM14 14h3v1h-3Z", grey),
            ("M12 7l3 4l-3 4l-3-4Z", blue),
        ],
    );
    icons.add_solid(
        "r3.wrapThrough",
        &[
            (page, white),
            ("M5 2h14v20h-14ZM6 3v18h12v-18Z", grey),
            ("M7 5h10v1h-10ZM7 8h10v1h-10ZM7 11h2v1h-2ZM7 14h10v1h-10ZM7 17h10v1h-10ZM15 11h2v1h-2Z", grey),
            ("M12 7.5l3 3.5l-3 3.5l-3-3.5Z", blue),
        ],
    );
    icons.add_solid(
        "r3.wrapTopBottom",
        &[
            (page, white),
            ("M5 2h14v20h-14ZM6 3v18h12v-18Z", grey),
            ("M7 5h10v1h-10ZM7 17h10v1h-10Z", grey),
            ("M9 8h6v7h-6Z", blue),
        ],
    );
    icons.add_solid(
        "r3.wrapBehind",
        &[
            (page, white),
            ("M8 6h8v11h-8Z", pale),
            ("M5 2h14v20h-14ZM6 3v18h12v-18Z", grey),
            (lines_full, grey),
        ],
    );
    icons.add_solid(
        "r3.wrapFront",
        &[
            (page, white),
            ("M5 2h14v20h-14ZM6 3v18h12v-18Z", grey),
            (lines_full, grey),
            ("M9 7h7v9h-7Z", blue),
        ],
    );
    icons.alias("picture.wrap", "r3.wrapSquare");
    icons.alias("picture.position", "r3.wrapInline");
    // Stacking: two overlapping squares, the front one blue (forward) or behind (backward).
    icons.add_solid(
        "picture.bringForward",
        &[
            ("M3 3h11v11h-11Z", grey),
            ("M4 4h9v9h-9Z", white),
            ("M9 9h12v12h-12Z", blue),
        ],
    );
    icons.add_solid(
        "picture.sendBackward",
        &[
            ("M9 9h12v12h-12Z", blue),
            ("M3 3h11v11h-11Z", grey),
            ("M4 4h9v9h-9Z", white),
        ],
    );
    icons.add_solid(
        "picture.align",
        &[
            ("M3 3h2v18h-2Z", grey),
            ("M7 5h12v5h-12Z", blue),
            ("M7 13h8v5h-8Z", blue),
        ],
    );
    icons.add_solid(
        "picture.group",
        &[
            ("M3 3h18v18h-18ZM4 4v16h16v-16Z", grey),
            ("M6 6h7v6h-7Z", blue),
            ("M11 11h7v7h-7Z", pale),
        ],
    );
    icons.add_solid(
        "picture.rotate",
        &[
            ("M5 9h12v12h-12Z", blue),
            ("M6 10h10v10h-10Z", pale),
            (
                "M8 2a7 7 0 0 1 10 3l1.5-1.5v5h-5l1.8-1.8a5 5 0 0 0-7.5-2.2Z",
                grey,
            ),
        ],
    );
    // References.
    let doc = |icons: &mut Icons, name: &str, extra: &[(&str, Option<Rgba>)]| {
        let mut parts: Vec<(&str, Option<Rgba>)> = vec![
            ("M4 2h12l4 4v16h-16Z", white),
            ("M4 2h12l4 4v16h-16ZM5 3v18h14v-14.6l-3.4-3.4Z", grey),
        ];
        parts.extend_from_slice(extra);
        icons.add_solid(name, &parts);
    };
    doc(
        icons,
        "references.toc",
        &[(
            "M7 7h10v1.5h-10ZM9 11h8v1.5h-8ZM9 15h8v1.5h-8ZM7 18h4v1.5h-4Z",
            blue,
        )],
    );
    doc(
        icons,
        "references.insertCitation",
        &[
            (
                "M7 9h3l-1.5 5h-1.5ZM12 9h3l-1.5 5h-1.5Z",
                rgb(0xc0, 0x39, 0x2b),
            ),
            ("M7 17h10v1.5h-10Z", grey),
        ],
    );
    doc(
        icons,
        "references.manageSources",
        &[("M7 7h10v2h-10ZM7 11h10v2h-10ZM7 15h6v2h-6Z", blue)],
    );
    doc(
        icons,
        "references.insertBibliography",
        &[
            ("M7 7h4v12h-4ZM12 7h4v12h-4Z", blue),
            ("M8 8h2v10h-2Z", pale),
        ],
    );
    doc(
        icons,
        "references.citationStyle",
        &[
            ("M7 8h10v1.5h-10ZM7 12h7v1.5h-7Z", grey),
            ("M13 15h5v5h-5Z", blue),
        ],
    );
    doc(
        icons,
        "references.markEntry",
        &[
            ("M7 7h10v1.5h-10ZM7 10h10v1.5h-10Z", grey),
            ("M8 13h8v6h-8Z", rgb(0x1e, 0x8e, 0x3e)),
        ],
    );
    doc(
        icons,
        "references.insertIndex",
        &[(
            "M7 7h4v1.5h-4ZM7 10h6v1.5h-6ZM9 13h6v1.5h-6ZM7 16h5v1.5h-5Z",
            blue,
        )],
    );
    doc(
        icons,
        "references.crossReference",
        &[
            ("M7 9h8l-2-2h2l3 3l-3 3h-2l2-2h-8Z", blue),
            ("M7 16h10v1.5h-10Z", grey),
        ],
    );
    doc(
        icons,
        "references.tableOfFigures",
        &[
            ("M7 7h6v5h-6Z", blue),
            ("M14 8h3v1.5h-3ZM7 15h10v1.5h-10Z", grey),
        ],
    );
    doc(
        icons,
        "references.notesDialog",
        &[("M7 15h10v1.5h-10Z", grey), ("M7 18h6v1.5h-6Z", blue)],
    );
    // Review / Mailings / View.
    doc(
        icons,
        "review.compareMenu",
        &[("M6 6h5v13h-5Z", pale), ("M13 6h5v13h-5Z", blue)],
    );
    doc(
        icons,
        "review.languageMenu",
        &[("M7 8h10v1.5h-10Z", grey), ("M12 11a4 4 0 1 1 0 8a4 4 0 1 1 0-8ZM12 12.3a2.7 2.7 0 1 0 0 5.4a2.7 2.7 0 1 0 0-5.4Z", blue)],
    );
    doc(
        icons,
        "review.protectMenu",
        &[
            ("M9 12h8v7h-8Z", rgb(0xd8, 0x9b, 0x2b)),
            (
                "M10.5 12v-2a2.5 2.5 0 0 1 5 0v2h-1.3v-2a1.2 1.2 0 0 0-2.4 0v2Z",
                grey,
            ),
        ],
    );
    icons.alias("review.restrictEditing", "review.protectMenu");
    icons.add_solid(
        "mailings.envelopes",
        &[
            ("M2 6h20v13h-20Z", white),
            ("M2 6h20v13h-20ZM3 7v11h18v-11Z", grey),
            ("M4 8h4v1h-4ZM16 8h4v3h-4ZM9 13h7v1h-7ZM9 15h7v1h-7Z", grey),
        ],
    );
    icons.add_solid(
        "mailings.labels",
        &[
            ("M3 3h18v18h-18Z", white),
            ("M3 3h18v18h-18ZM4 4v16h16v-16Z", grey),
            ("M5 5h6v4h-6ZM13 5h6v4h-6ZM5 11h6v4h-6ZM13 11h6v4h-6Z", blue),
        ],
    );
    doc(
        icons,
        "view.outline",
        &[(
            "M6 7h2v2h-2ZM9 7.5h8v1h-8ZM8 11h2v2h-2ZM11 11.5h6v1h-6ZM8 15h2v2h-2ZM11 15.5h6v1h-6Z",
            grey,
        )],
    );
    icons.add_solid(
        "view.stylesPane",
        &[
            ("M3 3h18v18h-18Z", white),
            ("M3 3h18v18h-18ZM4 4v16h16v-16Z", grey),
            ("M14 4h1v16h-1Z", grey),
            ("M6 7h6v1.5h-6ZM6 11h6v1.5h-6ZM6 15h6v1.5h-6Z", blue),
        ],
    );
    icons.add_solid(
        "styles.newStyle",
        &[
            (
                "M5 18l5-13h2l5 13h-2.2l-1.2-3.4h-5.2l-1.2 3.4ZM9 12.8h4l-2-5.8Z",
                blue,
            ),
            (
                "M17 2h1.5v3h3v1.5h-3v3h-1.5v-3h-3v-1.5h3Z",
                rgb(0xd8, 0x9b, 0x2b),
            ),
        ],
    );
    icons.add_solid(
        "styles.inspector",
        &[
            ("M4 18l5-13h2l5 13h-2.2l-1.2-3.4h-5.2l-1.2 3.4ZM8 12.8h4l-2-5.8Z", blue),
            ("M17 13a3 3 0 1 1 0 6a3 3 0 1 1 0-6ZM17 14.3a1.7 1.7 0 1 0 0 3.4a1.7 1.7 0 1 0 0-3.4ZM19 18.5l1-1l2.5 2.5l-1 1Z", grey),
        ],
    );
    icons.add_solid(
        "styles.manage",
        &[
            (
                "M4 18l5-13h2l5 13h-2.2l-1.2-3.4h-5.2l-1.2 3.4ZM8 12.8h4l-2-5.8Z",
                blue,
            ),
            ("M15 19l5-5l1.5 1.5l-5 5h-1.5Z", grey),
        ],
    );
    icons.alias("table.viewGridlines", "table.bordersInside");
    for (id, existing) in [
        ("outline.promoteHeading", "format.outdent"),
        ("outline.promote", "format.outdent"),
        ("outline.demote", "format.indent"),
        ("outline.demoteBody", "format.indent"),
        ("outline.moveUp", "data.sortAsc"),
        ("outline.moveDown", "data.sortDesc"),
        ("outline.showLevel", "format.numbering"),
        ("outline.close", "view.outline"),
    ] {
        if !icons.has(id) {
            icons.alias(id, existing);
        }
    }
}
