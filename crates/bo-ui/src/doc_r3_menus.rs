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
pub(crate) const MORE_MENUS: [&str; 7] = [
    "mailings.startMerge",
    "mailings.insertField",
    "mailings.rules",
    "review.languageMenu",
    "review.protectMenu",
    "references.addText",
    "references.nextFootnote",
];

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
pub(crate) const BUTTONS: [&str; 31] = [
    "mailings.editRecipients",
    "mailings.highlightFields",
    "mailings.addressBlock",
    "mailings.greetingLine",
    "mailings.matchFields",
    "mailings.updateLabels",
    "mailings.firstRecord",
    "mailings.lastRecord",
    "mailings.recordBox",
    "mailings.findRecipient",
    "mailings.checkErrors",
    "mailings.recipients",
    "references.showNotes",
    "references.updateFigures",
    "references.notesLauncher",
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
pub(crate) const OUTLINE: [&str; 15] = [
    "outline.promoteHeading",
    "outline.promote",
    "outline.level",
    "outline.demote",
    "outline.demoteBody",
    "outline.moveUp",
    "outline.moveDown",
    "outline.expand",
    "outline.collapse",
    "outline.showLevel",
    "outline.showFormatting",
    "outline.firstLine",
    "outline.showDocument",
    "outline.collapseSubdocs",
    "outline.close",
];

/// Drop-down boxes drawn over their ribbon placeholders (current value and ▾).
pub(crate) const BOXES: [&str; 3] = ["outline.level", "outline.showLevel", "references.citationStyle"];

/// Widths (logical px) of the round-3 placeholders measured on the reference at 125 %.
pub(crate) fn labeled_w(id: &str) -> Option<f32> {
    Some(match id {
        "outline.level" => 114.0,
        "outline.showLevel" => 214.0,
        "outline.showFormatting" | "outline.firstLine" => 160.0,
        "references.citationStyle" => 134.0,
        "mailings.recordBox" => 54.0,
        _ => return None,
    })
}

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
        "references.addText" => vec![
            e("references.addText.none", "docR3.notShownInToc"),
            e("references.addText.0", "docR3.level1"),
            e("references.addText.1", "docR3.level2"),
            e("references.addText.2", "docR3.level3"),
        ],
        "references.nextFootnote" => vec![
            e("references.note.nextFoot", "docR3.nextFootnote"),
            e("references.note.prevFoot", "docR3.previousFootnote"),
            e("references.note.nextEnd", "docR3.nextEndnote"),
            e("references.note.prevEnd", "docR3.previousEndnote"),
        ],
        "mailings.startMerge" => vec![
            e("mailings.type.letters", "docR3.mergeLetters"),
            e("mailings.type.labels", "docR3.mergeLabels"),
            e("mailings.type.directory", "docR3.mergeDirectory"),
            e("mailings.type.normal", "docR3.mergeNormal"),
        ],
        // Fields come from the data source at open time (see the window).
        "mailings.insertField" => vec![e("mailings.field.none", "docR3.noRecipients")],
        "mailings.rules" => vec![
            e("mailings.rule.ask", "docR3.ruleAsk"),
            e("mailings.rule.fillIn", "docR3.ruleFillIn"),
            e("mailings.rule.if", "docR3.ruleIf"),
            e("mailings.rule.mergeRec", "docR3.ruleMergeRec"),
            e("mailings.rule.mergeSeq", "docR3.ruleMergeSeq"),
            e("mailings.rule.next", "docR3.ruleNext"),
            e("mailings.rule.skipIf", "docR3.ruleSkipIf"),
        ],
        "review.languageMenu" => vec![e("review.language.set", "docR3.setProofingLanguage")],
        "review.protectMenu" => vec![e("review.restrictEditing", "docR3.restrictEditing")],
        "outline.level" => vec![
            e("outline.setLevel.0", "docR3.level1"),
            e("outline.setLevel.1", "docR3.level2"),
            e("outline.setLevel.2", "docR3.level3"),
            e("outline.setLevel.3", "docR3.level4"),
            e("outline.setLevel.4", "docR3.level5"),
            e("outline.setLevel.5", "docR3.level6"),
            e("outline.setLevel.6", "docR3.level7"),
            e("outline.setLevel.7", "docR3.level8"),
            e("outline.setLevel.8", "docR3.level9"),
            e("outline.setLevel.body", "docR3.bodyText"),
        ],
        "outline.showLevel" => vec![
            e("outline.level.1", "docR3.level1"),
            e("outline.level.2", "docR3.level2"),
            e("outline.level.3", "docR3.level3"),
            e("outline.level.4", "docR3.level4"),
            e("outline.level.5", "docR3.level5"),
            e("outline.level.6", "docR3.level6"),
            e("outline.level.7", "docR3.level7"),
            e("outline.level.8", "docR3.level8"),
            e("outline.level.9", "docR3.level9"),
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
            | "mailings.rule.ask"
            | "mailings.rule.if"
            | "mailings.rule.skipIf"
            | "mailings.field.none"
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

fn green_add() -> Option<Rgba> {
    Some(Rgba(0x2e, 0x8b, 0x57, 255))
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
    // The reference's Title Case on the References / Mailings / Review / View / picture tabs
    // (the shared interface keeps its own wording).
    for target in TITLE_CASE {
        let key = format!("docR3.tc.{}", target.replace('.', "_"));
        let v = s.get(&key).to_owned();
        if v != key {
            s.set(target, &v);
        }
    }
}

/// Labels shown in Title Case like the reference (`docR3.tc.*`).
pub(crate) const TITLE_CASE: [&str; 33] = [
    "ribbon.groups.toc",
    "ribbon.groups.footnotes",
    "ribbon.groups.mergeData",
    "ribbon.groups.mergePreview",
    "pictureTools.groupAdjust",
    "pictureTools.groupSize",
    "commands.insert.footnote",
    "commands.insert.endnote",
    "commands.references.updateToc",
    "commands.mailings.preview",
    "commands.mailings.nextRecord",
    "commands.mailings.previousRecord",
    "commands.mailings.finish",
    "commands.mailings.recipients",
    "commands.review.newComment",
    "commands.review.deleteComment",
    "commands.review.nextComment",
    "commands.review.previousComment",
    "commands.review.trackChanges",
    "commands.review.next",
    "commands.review.previous",
    "commands.review.markupMode",
    "commands.review.showMarkup",
    "commands.review.spellingDialog",
    "commands.view.readMode",
    "commands.view.printLayout",
    "commands.view.webLayout",
    "commands.view.formattingMarks",
    "commands.view.navigationPane",
    "commands.view.zoomIn",
    "commands.view.zoomOut",
    "commands.picture.reset",
    "commands.picture.crop",
];

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
            // Outlines: what a dimmed (disabled) icon shows.
            ("M3.5 3.5h17v17h-17Z", None),
            ("M6.5 6.5h6v5h-6ZM11.5 11.5h6v6h-6Z", None),
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
        "references.addText",
        &[("M7 7h10v1.5h-10ZM7 11h7v1.5h-7Z", grey), ("M14 14h1.5v2.5h2.5v1.5h-2.5v2.5h-1.5v-2.5h-2.5v-1.5h2.5Z", green_add())],
    );
    doc(
        icons,
        "references.nextFootnote",
        &[("M7 7h10v1.5h-10Z", grey), ("M8 12h6v-2l3.5 3.5l-3.5 3.5v-2h-6Z", blue)],
    );
    doc(
        icons,
        "references.showNotes",
        &[("M7 7h10v1.5h-10ZM7 10h10v1.5h-10Z", grey), ("M7 16h10v1h-10ZM7 18.5h6v1h-6Z", blue)],
    );
    doc(
        icons,
        "references.updateFigures",
        &[("M7 7h10v1.5h-10Z", grey), ("M11 11h2v5h-2ZM11 17.5h2v2h-2Z", rgb(0xc0, 0x39, 0x2b))],
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
    doc(
        icons,
        "mailings.startMerge",
        &[("M7 7h10v1.5h-10ZM7 10h10v1.5h-10Z", grey), ("M7 14h4v5h-4ZM13 14h4v5h-4Z", blue)],
    );
    doc(
        icons,
        "mailings.editRecipients",
        &[("M7 7h10v1.5h-10ZM7 10.5h10v1.5h-10ZM7 14h10v1.5h-10Z", grey), ("M13 19l5-5l1.5 1.5l-5 5h-1.5Z", blue)],
    );
    doc(
        icons,
        "mailings.highlightFields",
        &[("M6 9h12v4h-12Z", rgb(0xf5, 0xe6, 0x7a)), ("M7 10.3h10v1.4h-10Z", grey), ("M7 16h10v1.5h-10Z", grey)],
    );
    doc(icons, "mailings.addressBlock", &[("M7 8h10v1.5h-10ZM7 11h8v1.5h-8ZM7 14h9v1.5h-9Z", blue)]);
    doc(icons, "mailings.greetingLine", &[("M7 9h3v1.5h-3Z", blue), ("M11 9h6v1.5h-6ZM7 13h10v1.5h-10Z", grey)]);
    doc(icons, "mailings.insertField", &[("M7 10h10v4h-10Z", blue), ("M8 11.3h8v1.4h-8Z", pale)]);
    doc(icons, "mailings.rules", &[("M8 8h3v3h-3ZM8 13h3v3h-3Z", blue), ("M12.5 9h5v1h-5ZM12.5 14h5v1h-5Z", grey)]);
    doc(icons, "mailings.matchFields", &[("M7 8h4v3h-4ZM13 13h4v3h-4Z", blue), ("M11 9.5h2l1 3.5h-1.5l-0.5-2h-1Z", grey)]);
    doc(icons, "mailings.updateLabels", &[("M7 7h4v4h-4ZM13 7h4v4h-4ZM7 13h4v4h-4Z", blue), ("M13 15a2 2 0 1 1 4 0h-1.2a0.8 0.8 0 1 0-1.6 0Z", grey)]);
    icons.add_solid("mailings.firstRecord", &[("M6 6h2v12h-2ZM18 6v12l-9-6Z", grey)]);
    icons.add_solid("mailings.lastRecord", &[("M16 6h2v12h-2ZM6 6v12l9-6Z", grey)]);
    icons.add_solid("mailings.recordPrevious", &[("M16 6v12l-9-6Z", grey)]);
    icons.add_solid("mailings.recordNext", &[("M8 6v12l9-6Z", grey)]);
    icons.add_solid("mailings.recordBox", &[("M3 7h18v10h-18Z", white)]);
    doc(icons, "mailings.findRecipient", &[("M10 9a3 3 0 1 1 0 6a3 3 0 1 1 0-6ZM10 10.3a1.7 1.7 0 1 0 0 3.4a1.7 1.7 0 1 0 0-3.4ZM12 14.5l1-1l3 3l-1 1Z", grey)]);
    doc(icons, "mailings.checkErrors", &[("M7 13l2-2l2 2l4-5l2 2l-6 7Z", rgb(0x2e, 0x8b, 0x57))]);
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
    let green = rgb(0x2e, 0x8b, 0x57);
    icons.add_solid("outline.promote", &[("M4 12l7-6v4h9v4h-9v4Z", green)]);
    icons.add_solid("outline.demote", &[("M20 12l-7-6v4h-9v4h9v4Z", green)]);
    icons.add_solid(
        "outline.promoteHeading",
        &[("M3 12l6-6v3.5l3-3.5v12l-3-3.5v3.5ZM12 10h9v4h-9Z", green)],
    );
    icons.add_solid(
        "outline.demoteBody",
        &[("M21 12l-6-6v3.5l-3-3.5v12l3-3.5v3.5ZM3 10h9v4h-9Z", green)],
    );
    icons.add_solid("outline.moveUp", &[("M12 7l6 8h-12Z", grey)]);
    icons.add_solid("outline.moveDown", &[("M12 17l6-8h-12Z", grey)]);
    icons.add_solid("outline.expand", &[("M10.5 5h3v5.5h5.5v3h-5.5v5.5h-3v-5.5h-5.5v-3h5.5Z", blue)]);
    icons.add_solid("outline.collapse", &[("M5 10.5h14v3h-14Z", blue)]);
    icons.add_solid(
        "outline.close",
        &[
            ("M3 3h18v18h-18Z", rgb(0xd0, 0x45, 0x2f)),
            ("M7.5 6l4.5 4.5l4.5-4.5l1.5 1.5l-4.5 4.5l4.5 4.5l-1.5 1.5l-4.5-4.5l-4.5 4.5l-1.5-1.5l4.5-4.5l-4.5-4.5Z", white),
        ],
    );
    icons.add_solid(
        "outline.showLevel",
        &[
            ("M5 7a2.5 2.5 0 1 1 0 0.1ZM4 6h2v2h-2Z", grey),
            ("M9 6.5h11v1.5h-11ZM11 12h9v1.5h-9ZM11 17h9v1.5h-9Z", grey),
        ],
    );
    icons.alias("outline.level", "outline.showLevel");
    icons.alias("outline.showFormatting", "outline.showLevel");
    icons.alias("outline.firstLine", "outline.showLevel");
    doc(icons, "outline.showDocument", &[("M7 8h10v1.5h-10ZM7 12h10v1.5h-10Z", blue), ("M7 16h6v1.5h-6Z", grey)]);
    doc(icons, "outline.collapseSubdocs", &[("M7 8h10v1.5h-10Z", blue), ("M10 13l2 3l2-3Z", grey)]);
}
