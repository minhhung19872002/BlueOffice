//! BlueDoc dialogs of parity round 5 (docs/ui/REPORT.md rounds 80+), laid out after the reference
//! word processor's dialogs measured at 125 % (window rectangles as captured): the Mailings ›
//! Rules fields (Ask, Fill-in, If…Then…Else, Next Record If, Set Bookmark, Skip Record If —
//! ECMA-376 Part 1, §17.16.5 ASK, FILLIN, IF §17.16.5.26, NEXTIF §17.16.5.39, SET §17.16.5.57,
//! SKIPIF §17.16.5.58), Match Fields, Find in Field (Find Recipient) and Manage Styles. A child
//! module of `doc_forms` so the dialog builder is at hand; every dialog ends in engine
//! commands (`mergeInsertRule`, ADR-0008) or interface actions.
use super::{t, DocForm, DocFormKind, FormContext, FormOut};
use crate::strings::Strings;
use serde_json::{json, Value};

/// Comparisons of the rule dialogs in the reference's order: label key, fallback, operator
/// and whether the value box is used (Is blank / Is not blank compare with "").
pub(crate) const COMPARISONS: [(&str, &str, &str, bool); 8] = [
    ("eq", "Equal to", "=", true),
    ("ne", "Not equal to", "<>", true),
    ("lt", "Less than", "<", true),
    ("gt", "Greater than", ">", true),
    ("le", "Less than or equal", "<=", true),
    ("ge", "Greater than or equal", ">=", true),
    ("blank", "Is blank", "=", false),
    ("notBlank", "Is not blank", "<>", false),
];

/// Address fields of Match Fields (the reference's list order; own wording of the
/// conventional address-book fields).
pub(crate) const ADDRESS_FIELDS: [&str; 13] = [
    "Unique Identifier",
    "Courtesy Title",
    "First Name",
    "Middle Name",
    "Last Name",
    "Suffix",
    "Nickname",
    "Job Title",
    "Company",
    "Address 1",
    "Address 2",
    "City",
    "State",
];

/// Control ids of Match Fields' rows (combo boxes, labels).
const ROWS: [&str; 13] = [
    "m0", "m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8", "m9", "m10", "m11", "m12",
];
const LABELS: [&str; 13] = [
    "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11", "n12",
];

/// Whether round-5 dialog `kind` is built here.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::AskRule
            | DocFormKind::FillInRule
            | DocFormKind::IfRule
            | DocFormKind::NextIfRule
            | DocFormKind::SetRule
            | DocFormKind::SkipIfRule
            | DocFormKind::MatchFields
            | DocFormKind::FindRecipient
            | DocFormKind::ManageStyles
    )
}

/// The recipient column matching address field `name` by its letters ("First Name" →
/// "FirstName", "first_name"), as the reference pre-matches.
pub(crate) fn guess_column(name: &str, columns: &[String]) -> Option<usize> {
    let key = |s: &str| -> String {
        s.chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(char::to_lowercase)
            .collect()
    };
    let want = key(name);
    columns.iter().position(|c| key(c) == want)
}

fn comparison_items(s: &Strings) -> Vec<String> {
    COMPARISONS
        .iter()
        .map(|(k, f, ..)| t(s, &format!("docR5.cmp.{k}"), f))
        .collect()
}

#[allow(clippy::too_many_lines)]
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let fields = c.recipients.0.clone();
    match kind {
        // 742 × 314.
        DocFormKind::AskRule => {
            let mut f = DocForm::new(
                kind,
                &tr("docR5.askTitle", "Insert Word Field: Ask"),
                (742.0, 314.0),
                vec![],
            );
            f.label(
                "bmL",
                [18.0, 47.0, 300.0, 18.0],
                &tr("docR5.bookmark", "Bookmark:"),
            );
            f.edit("bookmark", [18.0, 68.0, 314.0, 26.0], "");
            f.list("bookmarks", [18.0, 94.0, 314.0, 157.0], Vec::new(), None);
            f.label(
                "promptL",
                [346.0, 53.0, 300.0, 18.0],
                &tr("docR5.prompt", "Prompt:"),
            );
            f.edit("prompt", [346.0, 74.0, 378.0, 74.0], "");
            f.label(
                "defaultL",
                [346.0, 157.0, 300.0, 18.0],
                &tr("docR5.defaultBookmark", "Default bookmark text:"),
            );
            f.edit("default", [346.0, 177.0, 378.0, 74.0], "");
            f.check(
                "once",
                [18.0, 271.0, 200.0, 20.0],
                &tr("docR5.askOnce", "Ask once"),
                false,
            );
            f.button(
                "ok",
                [485.0, 265.0, 110.0, 30.0],
                &tr("common.ok", "OK"),
                false,
            );
            f.button(
                "cancel",
                [612.0, 265.0, 110.0, 30.0],
                &tr("common.cancel", "Cancel"),
                true,
            );
            f.disable("ok");
            f
        }
        // 414 × 334.
        DocFormKind::FillInRule => {
            let mut f = DocForm::new(
                kind,
                &tr("docR5.fillInTitle", "Insert Word Field: Fill-in"),
                (414.0, 334.0),
                vec![],
            );
            f.label(
                "promptL",
                [18.0, 47.0, 300.0, 18.0],
                &tr("docR5.prompt", "Prompt:"),
            );
            f.edit("prompt", [18.0, 68.0, 378.0, 91.0], "");
            f.label(
                "defaultL",
                [18.0, 163.0, 300.0, 18.0],
                &tr("docR5.defaultFillIn", "Default fill-in text:"),
            );
            f.edit("default", [18.0, 184.0, 378.0, 91.0], "");
            f.check(
                "once",
                [18.0, 291.0, 120.0, 20.0],
                &tr("docR5.askOnce", "Ask once"),
                false,
            );
            f.button(
                "ok",
                [156.0, 285.0, 110.0, 30.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [283.0, 285.0, 110.0, 30.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // 814 × 377.
        DocFormKind::IfRule => {
            let mut f = DocForm::new(
                kind,
                &tr("docR5.ifTitle", "Insert Word Field: IF"),
                (814.0, 377.0),
                vec![],
            );
            f.label("ifL", [18.0, 47.0, 40.0, 18.0], "IF");
            f.label(
                "fieldL",
                [32.0, 74.0, 200.0, 18.0],
                &tr("docR5.fieldName", "Field name:"),
            );
            f.combo("field", [32.0, 95.0, 223.0, 26.0], fields, 0);
            f.label(
                "cmpL",
                [270.0, 74.0, 200.0, 18.0],
                &tr("docR5.comparison", "Comparison:"),
            );
            f.combo("cmp", [270.0, 95.0, 224.0, 26.0], comparison_items(s), 0);
            f.label(
                "valueL",
                [508.0, 74.0, 200.0, 18.0],
                &tr("docR5.compareTo", "Compare to:"),
            );
            f.edit("value", [508.0, 95.0, 288.0, 26.0], "");
            f.label(
                "thenL",
                [18.0, 133.0, 300.0, 18.0],
                &tr("docR5.insertThis", "Insert this text:"),
            );
            f.edit("then", [18.0, 153.0, 778.0, 68.0], "");
            f.label(
                "elseL",
                [18.0, 226.0, 300.0, 18.0],
                &tr("docR5.otherwise", "Otherwise insert this text:"),
            );
            f.edit("else", [18.0, 246.0, 778.0, 68.0], "");
            f.button(
                "ok",
                [556.0, 329.0, 110.0, 30.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [683.0, 329.0, 110.0, 30.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // 440 × 263 (both).
        DocFormKind::NextIfRule | DocFormKind::SkipIfRule => {
            let title = if kind == DocFormKind::NextIfRule {
                tr("docR5.nextIfTitle", "Insert Word Field: Next Record If")
            } else {
                tr("docR5.skipIfTitle", "Insert Word Field: Skip Record If")
            };
            let mut f = DocForm::new(kind, &title, (440.0, 263.0), vec![]);
            f.label(
                "fieldL",
                [18.0, 48.0, 300.0, 18.0],
                &tr("docR5.fieldName", "Field name:"),
            );
            f.combo("field", [18.0, 69.0, 404.0, 26.0], fields, 0);
            f.label(
                "cmpL",
                [18.0, 101.0, 300.0, 18.0],
                &tr("docR5.comparison", "Comparison:"),
            );
            f.combo("cmp", [18.0, 122.0, 404.0, 26.0], comparison_items(s), 0);
            f.label(
                "valueL",
                [18.0, 154.0, 300.0, 18.0],
                &tr("docR5.compareTo", "Compare to:"),
            );
            f.edit("value", [18.0, 175.0, 404.0, 26.0], "");
            f.button(
                "ok",
                [182.0, 215.0, 110.0, 30.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [309.0, 215.0, 110.0, 30.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // 756 × 297.
        DocFormKind::SetRule => {
            let mut f = DocForm::new(
                kind,
                &tr("docR5.setTitle", "Insert Word Field: Set"),
                (756.0, 297.0),
                vec![],
            );
            f.label(
                "bmL",
                [18.0, 47.0, 300.0, 18.0],
                &tr("docR5.bookmark", "Bookmark:"),
            );
            f.edit("bookmark", [18.0, 68.0, 404.0, 26.0], "");
            f.list("bookmarks", [18.0, 94.0, 404.0, 140.0], Vec::new(), None);
            f.label(
                "valueL",
                [436.0, 47.0, 300.0, 18.0],
                &tr("docR5.value", "Value:"),
            );
            f.edit("value", [436.0, 68.0, 302.0, 166.0], "");
            f.button(
                "ok",
                [499.0, 248.0, 110.0, 30.0],
                &tr("common.ok", "OK"),
                false,
            );
            f.button(
                "cancel",
                [626.0, 248.0, 110.0, 30.0],
                &tr("common.cancel", "Cancel"),
                true,
            );
            f.disable("ok");
            f
        }
        // 384 × 601.
        DocFormKind::MatchFields => {
            let mut f = DocForm::new(
                kind,
                &tr("docR5.matchTitle", "Match Fields"),
                (384.0, 601.0),
                vec![],
            );
            f.label(
                "intro1",
                [18.0, 47.0, 350.0, 18.0],
                &tr(
                    "docR5.matchIntro1",
                    "In order to use special features, Mail Merge needs",
                ),
            );
            f.label(
                "intro2",
                [18.0, 64.0, 350.0, 18.0],
                &tr(
                    "docR5.matchIntro2",
                    "to know which fields in your recipient list match",
                ),
            );
            f.label(
                "intro3",
                [18.0, 81.0, 350.0, 18.0],
                &tr(
                    "docR5.matchIntro3",
                    "the required fields. Use the drop-down list to pick",
                ),
            );
            f.label(
                "intro4",
                [18.0, 98.0, 350.0, 18.0],
                &tr(
                    "docR5.matchIntro4",
                    "the recipient list field of each address part.",
                ),
            );
            let not_matched = tr("docR5.notMatched", "(not matched)");
            let mut items = vec![not_matched];
            items.extend(fields.iter().cloned());
            for (i, name) in ADDRESS_FIELDS.iter().enumerate() {
                let y = 128.0 + 22.0 * i as f32;
                f.label(
                    LABELS[i],
                    [38.0, y, 175.0, 20.0],
                    &tr(&format!("docR5.addr.{i}"), name),
                );
                let sel = c
                    .match_fields
                    .get(*name)
                    .and_then(|col| fields.iter().position(|x| x == col))
                    .or_else(|| guess_column(name, &fields))
                    .map_or(0, |k| k + 1);
                f.combo(ROWS[i], [220.0, y, 120.0, 20.0], items.clone(), sel);
            }
            f.label(
                "outro1",
                [18.0, 433.0, 350.0, 18.0],
                &tr(
                    "docR5.matchOutro1",
                    "Use the drop-down lists to choose the field from your",
                ),
            );
            f.label(
                "outro2",
                [18.0, 450.0, 350.0, 18.0],
                &tr(
                    "docR5.matchOutro2",
                    "data that holds each address part (listed on the left).",
                ),
            );
            f.check(
                "remember",
                [18.0, 495.0, 350.0, 20.0],
                &tr(
                    "docR5.remember",
                    "Remember this matching for this data source",
                ),
                false,
            );
            f.button(
                "ok",
                [126.0, 546.0, 110.0, 30.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [253.0, 546.0, 110.0, 30.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // 414 × 210.
        DocFormKind::FindRecipient => {
            let mut f = DocForm::new(
                kind,
                &tr("docR5.findTitle", "Find in Field"),
                (414.0, 210.0),
                vec![],
            );
            f.label(
                "whatL",
                [18.0, 47.0, 300.0, 18.0],
                &tr("docR5.findWhat", "Find what:"),
            );
            f.edit("what", [18.0, 68.0, 378.0, 26.0], "");
            f.label(
                "inL",
                [18.0, 98.0, 300.0, 18.0],
                &tr("docR5.inField", "In field:"),
            );
            f.combo("field", [18.0, 119.0, 378.0, 28.0], fields, 0);
            f.button(
                "findFirst",
                [157.0, 162.0, 110.0, 30.0],
                &tr("docR5.findFirst", "Find First"),
                true,
            );
            f.button(
                "cancel",
                [284.0, 162.0, 110.0, 30.0],
                &tr("docR3.close", "Close"),
                false,
            );
            f.disable("findFirst");
            f
        }
        _ => DocForm::new(kind, "", (300.0, 200.0), vec![]),
    }
}

/// One style of Manage Styles: id, name, font, size (pt), bold, italic, recommended, built-in.
#[derive(Debug, Clone)]
pub(crate) struct ManagedStyle {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) font: String,
    pub(crate) size: f32,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) recommended: bool,
    pub(crate) paragraph: bool,
}

/// Styles a document always keeps (the reference dims Delete for them).
fn built_in(id: &str) -> bool {
    id == "Normal" || id.starts_with("Heading") || id == "DefaultParagraphFont"
}

/// Manage Styles (Styles pane › Manage Styles), 629 × 661 at 125 %: Edit (sort order, the
/// styles, preview and description, Delete, New Style…), Recommend, Restrict, Set Defaults
/// (font and size of Normal).
pub(crate) fn open_manage_styles(s: &Strings, styles: Vec<ManagedStyle>) -> DocForm {
    use super::Kind;
    let tr = |k: &str, f: &str| t(s, k, f);
    let tabs = vec![
        tr("docR5.msEdit", "Edit"),
        tr("docR5.msRecommend", "Recommend"),
        tr("docR5.msRestrict", "Restrict"),
        tr("docR5.msDefaults", "Set Defaults"),
    ];
    let mut f = DocForm::new(
        DocFormKind::ManageStyles,
        &tr("docR5.manageTitle", "Manage Styles"),
        (629.0, 661.0),
        tabs,
    );
    let normal = styles.iter().find(|x| x.id == "Normal").cloned();
    f.managed = styles;
    f.label_on(
        0,
        "sortL",
        [32.0, 92.0, 80.0, 18.0],
        &tr("docR5.sortOrder", "Sort order:"),
    );
    f.form.add(
        "sort",
        Some(0),
        [110.0, 88.0, 223.0, 26.0],
        Kind::Combo {
            items: vec![
                tr("docR5.sortRecommended", "As Recommended"),
                tr("docR5.sortAlpha", "Alphabetical"),
                tr("docR5.sortType", "By type"),
            ],
            sel: 0,
        },
    );
    f.check_on(
        0,
        "recOnly",
        [387.0, 92.0, 215.0, 20.0],
        &tr("docR5.recommendedOnly", "Show recommended styles only"),
        false,
    );
    f.label_on(
        0,
        "pickL",
        [32.0, 130.0, 300.0, 18.0],
        &tr("docR5.selectStyle", "Select a style to edit"),
    );
    f.form.add(
        "list",
        Some(0),
        [32.0, 168.0, 565.0, 173.0],
        Kind::List {
            items: Vec::new(),
            sel: None,
            top: 0,
        },
    );
    f.label_on(0, "previewL", [32.0, 361.0, 400.0, 18.0], "");
    f.form
        .add("preview", Some(0), [32.0, 382.0, 309.0, 40.0], Kind::Canvas);
    f.button_on(
        0,
        "modify",
        [357.0, 387.0, 110.0, 30.0],
        &tr("docR5.modify", "Modify..."),
    );
    f.button_on(
        0,
        "delete",
        [484.0, 387.0, 110.0, 30.0],
        &tr("docR5.delete", "Delete"),
    );
    f.label_on(0, "desc1", [36.0, 434.0, 560.0, 18.0], "");
    f.label_on(0, "desc2", [52.0, 451.0, 544.0, 18.0], "");
    f.button_on(
        0,
        "newStyle",
        [32.0, 507.0, 110.0, 30.0],
        &tr("docR5.newStyle", "New Style..."),
    );
    f.disable("modify");
    // Recommend.
    f.label_on(
        1,
        "recL",
        [32.0, 92.0, 560.0, 18.0],
        &tr(
            "docR5.recommendHelp",
            "Styles marked recommended show first in the Styles pane.",
        ),
    );
    // Restrict.
    f.label_on(
        2,
        "resL",
        [32.0, 92.0, 560.0, 18.0],
        &tr(
            "docR5.restrictHelp",
            "Formatting restrictions are set in Restrict Editing.",
        ),
    );
    // Set Defaults: the font and size of Normal.
    let (font, size) = normal.map_or_else(
        || ("Times New Roman".to_owned(), 12.0),
        |n| (n.font.clone(), n.size),
    );
    f.label_on(
        3,
        "fontL",
        [32.0, 92.0, 60.0, 18.0],
        &tr("docR5.font", "Font:"),
    );
    f.edit_on(3, "font", [100.0, 88.0, 240.0, 26.0], &font);
    f.label_on(
        3,
        "sizeL",
        [360.0, 92.0, 50.0, 18.0],
        &tr("docR5.size", "Size:"),
    );
    f.edit_on(3, "size", [410.0, 88.0, 80.0, 26.0], &format!("{size}"));
    f.radio(
        "thisDoc",
        "scope",
        [18.0, 578.0, 160.0, 20.0],
        &tr("docR5.onlyThisDoc", "Only in this document"),
        true,
    );
    f.radio(
        "template",
        "scope",
        [184.0, 578.0, 280.0, 20.0],
        &tr("docR5.newDocs", "New documents based on this template"),
        false,
    );
    f.disable("template");
    f.button(
        "importExport",
        [18.0, 613.0, 116.0, 30.0],
        &tr("docR5.importExport", "Import/Export..."),
        false,
    );
    f.disable("importExport");
    f.button(
        "ok",
        [372.0, 613.0, 110.0, 30.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [498.0, 613.0, 110.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.manage_text = vec![
        tr("docR5.previewOf", "Preview of {name}:"),
        tr("docR5.descFont", "Font:"),
        tr("docR5.paragraphStyle", "Style: paragraph style"),
        tr("docR5.characterStyle", "Style: character style"),
    ];
    f.manage_fill();
    f
}

impl DocForm {
    /// Manage Styles: the list in the chosen order, the selection's preview and description.
    fn manage_fill(&mut self) {
        let mut v: Vec<ManagedStyle> = self.managed.clone();
        if self.on("recOnly") {
            v.retain(|x| x.recommended);
        }
        match self.sel("sort").unwrap_or(0) {
            1 => v.sort_by_key(|x| x.name.to_lowercase()),
            2 => v.sort_by_key(|x| (!x.paragraph, x.name.to_lowercase())),
            _ => v.sort_by_key(|x| !x.recommended),
        }
        self.ids = v.iter().map(|x| x.id.clone()).collect();
        let pick = self
            .ids
            .iter()
            .position(|i| i == "Normal")
            .or((!self.ids.is_empty()).then_some(0));
        let items = v
            .iter()
            .map(|x| {
                let mark = if x.paragraph { "\u{b6} " } else { "a  " };
                format!("{mark}{}", x.name)
            })
            .collect();
        self.form.set_items("list", items, pick);
        self.manage_show();
    }

    /// Preview and description of the selected style.
    fn manage_show(&mut self) {
        let Some(x) = self
            .sel("list")
            .and_then(|i| self.ids.get(i))
            .and_then(|id| self.managed.iter().find(|m| &m.id == id))
            .cloned()
        else {
            return;
        };
        let texts = self.manage_text.clone();
        let text = |i: usize| texts.get(i).cloned().unwrap_or_default();
        let head = text(0).replace("{name}", &x.name);
        self.set_text("previewL", &head);
        let mut d = format!("{} {}, {} pt", text(1), x.font, x.size);
        if x.bold {
            d.push_str(", Bold");
        }
        if x.italic {
            d.push_str(", Italic");
        }
        self.set_text("desc1", &d);
        let kind = if x.paragraph { text(2) } else { text(3) };
        self.set_text("desc2", &kind);
        self.preview = vec![x.font.clone(), format!("{}", x.size), x.name.clone()];
        self.form.enable("delete", !built_in(&x.id));
    }

    /// The rule a rule dialog inserts (`mergeInsertRule`).
    fn rule(&self) -> Option<Value> {
        let cmp = || {
            let (_, _, op, uses) = COMPARISONS[self.sel("cmp").unwrap_or(0).min(7)];
            let value = if uses {
                self.text("value")
            } else {
                String::new()
            };
            (op, value)
        };
        let field = self.text("field");
        Some(match self.kind {
            DocFormKind::AskRule => json!({
                "kind": "ask",
                "name": self.text("bookmark").trim(),
                "prompt": self.text("prompt"),
                "default": self.text("default"),
                "once": self.on("once"),
            }),
            DocFormKind::FillInRule => json!({
                "kind": "fillIn",
                "prompt": self.text("prompt"),
                "default": self.text("default"),
                "once": self.on("once"),
            }),
            DocFormKind::IfRule => {
                let (op, value) = cmp();
                json!({
                    "kind": "if", "field": field, "op": op, "value": value,
                    "then": self.text("then"), "else": self.text("else"),
                })
            }
            DocFormKind::NextIfRule | DocFormKind::SkipIfRule => {
                let (op, value) = cmp();
                let kind = if self.kind == DocFormKind::NextIfRule {
                    "nextIf"
                } else {
                    "skipIf"
                };
                json!({ "kind": kind, "field": field, "op": op, "value": value })
            }
            DocFormKind::SetRule => json!({
                "kind": "set",
                "name": self.text("bookmark").trim(),
                "value": self.text("value"),
            }),
            _ => return None,
        })
    }

    /// OK of a round-5 dialog.
    pub(super) fn r5_ok(&mut self) -> FormOut {
        match self.kind {
            DocFormKind::MatchFields => {
                let mut map = serde_json::Map::new();
                for (i, name) in ADDRESS_FIELDS.iter().enumerate() {
                    let id = format!("m{i}");
                    if self.sel(&id).unwrap_or(0) > 0 {
                        map.insert((*name).to_owned(), json!(self.text(&id)));
                    }
                }
                FormOut::Run(
                    vec![json!({
                        "cmd": "ui.action",
                        "id": format!("mailings.matchFields.set:{}", Value::Object(map)),
                    })],
                    true,
                )
            }
            DocFormKind::FindRecipient => self.r5_clicked("findFirst"),
            DocFormKind::ManageStyles => {
                // Set Defaults: the font and size of Normal, when changed.
                let normal = self.managed.iter().find(|m| m.id == "Normal");
                let font = self.text("font");
                let size = self.text("size").trim().replace(',', ".");
                let changed = normal.is_none_or(|n| {
                    n.font != font.trim() || size.parse::<f32>().ok() != Some(n.size)
                });
                if changed && !font.trim().is_empty() {
                    FormOut::Run(
                        vec![json!({
                            "cmd": "ui.action",
                            "id": format!("styles.setDefaults:{}\u{1f}{size}", font.trim()),
                        })],
                        true,
                    )
                } else {
                    FormOut::Close
                }
            }
            _ => match self.rule() {
                // Ask and Set need a bookmark name (OK stays dimmed until there is one).
                Some(r)
                    if matches!(self.kind, DocFormKind::AskRule | DocFormKind::SetRule)
                        && r["name"].as_str().is_none_or(str::is_empty) =>
                {
                    FormOut::Redraw
                }
                Some(r) => FormOut::Run(vec![json!({ "cmd": "mergeInsertRule", "rule": r })], true),
                None => FormOut::Close,
            },
        }
    }

    /// A button of a round-5 dialog other than OK / Cancel.
    pub(super) fn r5_clicked(&mut self, id: &str) -> FormOut {
        match (self.kind, id) {
            (DocFormKind::ManageStyles, "newStyle") => FormOut::Open(DocFormKind::NewStyle),
            (DocFormKind::ManageStyles, "delete") => {
                let Some(i) = self.sel("list") else {
                    return FormOut::Redraw;
                };
                let Some(id) = self.ids.get(i).cloned() else {
                    return FormOut::Redraw;
                };
                if built_in(&id) {
                    return FormOut::Redraw;
                }
                self.managed.retain(|m| m.id != id);
                let mut ids = std::mem::take(&mut self.ids);
                ids.remove(i);
                let items: Vec<String> = match self.form.get("list").map(|c| &c.kind) {
                    Some(super::Kind::List { items, .. }) => {
                        let mut v = items.clone();
                        v.remove(i.min(v.len().saturating_sub(1)));
                        v
                    }
                    _ => Vec::new(),
                };
                self.ids = ids;
                let next = (!items.is_empty()).then(|| i.min(items.len() - 1));
                self.form.set_items("list", items, next);
                FormOut::Run(vec![json!({ "cmd": "styleDelete", "id": id })], false)
            }
            (DocFormKind::FindRecipient, "findFirst") => {
                let what = self.text("what");
                if what.is_empty() {
                    return FormOut::Redraw;
                }
                // Stays open as the reference (Find Next).
                FormOut::Run(
                    vec![json!({
                        "cmd": "ui.action",
                        "id": format!("mailings.find:{}\u{1f}{what}", self.text("field")),
                    })],
                    false,
                )
            }
            _ => FormOut::Redraw,
        }
    }

    /// Dependent controls of a round-5 dialog follow a change.
    pub(super) fn r5_changed(&mut self, id: &str) {
        match (self.kind, id) {
            (DocFormKind::AskRule | DocFormKind::SetRule, "bookmark") => {
                let named = !self.text("bookmark").trim().is_empty();
                self.form.enable("ok", named);
            }
            (DocFormKind::ManageStyles, "sort" | "recOnly") => self.manage_fill(),
            (DocFormKind::ManageStyles, "list") => self.manage_show(),
            (DocFormKind::FindRecipient, "what") => {
                let any = !self.text("what").is_empty();
                self.form.enable("findFirst", any);
            }
            (DocFormKind::IfRule | DocFormKind::NextIfRule | DocFormKind::SkipIfRule, "cmp") => {
                let uses = COMPARISONS[self.sel("cmp").unwrap_or(0).min(7)].3;
                self.form.enable("value", uses);
            }
            _ => {}
        }
    }
}
