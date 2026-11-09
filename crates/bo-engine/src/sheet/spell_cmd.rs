//! Spelling and grammar in BlueSheet (`docs/parity/BACKLOG.md` CORE-SPELL-006), through the checker
//! BlueDoc uses (`crate::spell`, `crate::proof_text`):
//!
//! * `spellText` — issues of the text typed in the cell editor (underlined while editing; a text
//!   starting with `=` is a formula and is not checked);
//! * `spellCheck` — issues of every text cell of the workbook, sheet by sheet, row by row: cells
//!   holding text (§18.3.1.4 `c` of type `s`, `inlineStr`, rich text §18.4.4) without a formula.
//!   Numbers, dates, booleans, errors and formula results are not text the user typed;
//! * `spellReplace` — corrections of cell text, all of them one undoable edit (replace all);
//!   rich text keeps its runs (§18.4.4 `r`);
//! * `spellIgnore`, `spellAddWord`, `grammarIgnore`, `spellOptions`.
//!
//! The proofing language belongs to the workbook view, not to the file: SpreadsheetML has no
//! language for cell text (run properties §18.4.7 `rPr` have none), so the host keeps it as a
//! BlueOffice preference and sends it with each command (`lang`, BCP 47; empty or absent: any
//! dictionary accepts a word).

use super::cmd::Recalc;
use super::SheetEngine;
use crate::proof_text::{self, OptionsIn, Replace, TextIssue};
use crate::spell::Span;
use crate::{EngineError, Result};
use bo_model::sheet::{CellValue, Workbook};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// A correction of the text of one cell.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellReplace {
    /// Sheet index.
    pub sheet: usize,
    /// 0-based row.
    pub row: u32,
    /// 0-based column.
    pub col: u32,
    /// Byte range and replacement.
    #[serde(flatten)]
    pub edit: Replace,
}

/// A spelling command of BlueSheet.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SpellCommand {
    /// Issues of a text being typed: `{ issues: [TextIssue] }`.
    SpellText {
        /// The text.
        text: String,
        /// Proofing language (BCP 47).
        #[serde(default)]
        lang: Option<String>,
    },
    /// Issues of every text cell: `{ issues: [{ sheet, row, col, …TextIssue }], unchecked,
    /// languages }`.
    SpellCheck {
        /// Proofing language (BCP 47).
        #[serde(default)]
        lang: Option<String>,
    },
    /// Apply corrections as one undoable edit: `{ changed }` (cells changed).
    SpellReplace {
        /// Corrections.
        edits: Vec<CellReplace>,
    },
    /// Ignore a word everywhere (this session).
    SpellIgnore {
        /// The word.
        word: String,
    },
    /// Add a word to the user dictionary.
    SpellAddWord {
        /// The word.
        word: String,
    },
    /// Ignore a grammar issue (rule id, flagged text).
    GrammarIgnore {
        /// Rule id.
        rule: String,
        /// Flagged text.
        text: String,
    },
    /// Read or change proofing options.
    SpellOptions {
        /// New option values (absent: unchanged).
        #[serde(flatten)]
        options: OptionsIn,
    },
}

impl SpellCommand {
    /// Parses a spelling command (`None`: another command).
    pub fn parse(v: &Value) -> Option<SpellCommand> {
        const NAMES: &[&str] = &[
            "spellText",
            "spellCheck",
            "spellReplace",
            "spellIgnore",
            "spellAddWord",
            "grammarIgnore",
            "spellOptions",
        ];
        let name = v.get("cmd")?.as_str()?;
        if !NAMES.contains(&name) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// An issue of a cell.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellIssue {
    /// Sheet index.
    pub sheet: usize,
    /// Sheet name.
    pub sheet_name: String,
    /// 0-based row.
    pub row: u32,
    /// 0-based column.
    pub col: u32,
    /// The issue (byte offsets in the cell text).
    #[serde(flatten)]
    pub issue: TextIssue,
}

/// Text the user typed into a cell: text values without a formula.
fn cell_text(c: &bo_model::sheet::Cell) -> Option<String> {
    if c.formula.is_some() {
        return None;
    }
    match &c.value {
        CellValue::Text(_) | CellValue::RichText(_) => c.value.as_text(),
        _ => None,
    }
}

/// Grammar issues that do not fit cells: a cell is a label more often than a sentence, so its
/// first letter may be small (own choice).
fn keep_in_cell(i: &TextIssue) -> bool {
    !(i.rule == Some("capitalize") && i.start == 0)
}

/// Apply the corrections of one cell (`None`: they do not fit its text).
fn replace_cell(value: &mut CellValue, edits: &[&Replace]) -> Option<()> {
    match value {
        CellValue::Text(s) => {
            *s = proof_text::apply(s, edits)?;
            Some(())
        }
        CellValue::RichText(runs) => {
            let text: String = runs.iter().map(|r| r.text.as_str()).collect();
            let plan = proof_text::plan(&text, edits)?;
            for e in plan {
                let mut parts: Vec<&mut String> = runs.iter_mut().map(|r| &mut r.text).collect();
                if !proof_text::splice_parts(&mut parts, &|_| true, e.start, e.end, &e.text) {
                    return None;
                }
            }
            // Runs emptied by a correction go (one run stays).
            if runs.len() > 1 {
                runs.retain(|r| !r.text.is_empty());
            }
            Some(())
        }
        _ => None,
    }
}

/// A text cell to check: (sheet, row, column), text, language spans.
type TextCell = ((usize, u32, u32), String, Vec<Span>);

fn stale() -> EngineError {
    EngineError::Sheet("spelling: the cell text changed".into())
}

impl SheetEngine {
    /// Loads a Hunspell dictionary (`.aff` + `.dic` bytes) for English spelling (web hosts fetch
    /// it; native builds bundle `en_US`).
    pub fn load_dictionary(&mut self, aff: &[u8], dic: &[u8]) -> Result<()> {
        let d =
            bo_spell::Dictionary::from_aff_dic(aff, dic).map_err(crate::EngineError::BadCommand)?;
        self.spell.speller.add_language("en", Box::new(d));
        self.spell.invalidate();
        Ok(())
    }

    /// Text cells of the workbook in checking order with their language spans.
    fn text_cells(&self, lang: Option<&str>) -> Vec<TextCell> {
        let Some(wb) = self.wb.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (si, ws) in wb.sheets.iter().enumerate() {
            // Cells are keyed (row, col): row by row, left to right.
            for (&(r, c), cell) in &ws.cells {
                if let Some(t) = cell_text(cell) {
                    let spans = proof_text::whole(&t, lang);
                    out.push(((si, r, c), t, spans));
                }
            }
        }
        out
    }

    /// Runs a spelling command.
    pub fn spell_command(&mut self, cmd: SpellCommand) -> Result<Value> {
        match cmd {
            SpellCommand::SpellText { text, lang } => {
                if text.starts_with('=') {
                    return Ok(json!({ "issues": [] }));
                }
                let spans = proof_text::whole(&text, lang.as_deref());
                let (mut issues, unchecked) =
                    proof_text::check(&mut self.spell, &text, &spans, None, true);
                issues.retain(keep_in_cell);
                Ok(json!({ "issues": issues, "unchecked": unchecked }))
            }
            SpellCommand::SpellCheck { lang } => Ok(self.spell_check(lang.as_deref())),
            SpellCommand::SpellReplace { edits } => {
                let changed = self.spell_replace(&edits)?;
                Ok(json!({ "changed": changed }))
            }
            SpellCommand::SpellIgnore { word } => {
                self.spell.learn(&word, true);
                Ok(json!({ "ok": true }))
            }
            SpellCommand::SpellAddWord { word } => {
                self.spell.learn(&word, false);
                Ok(json!({ "ok": true }))
            }
            SpellCommand::GrammarIgnore { rule, text } => {
                self.spell.ignore_issue(&rule, &text);
                Ok(json!({ "ok": true }))
            }
            SpellCommand::SpellOptions { options } => Ok(json!(self.spell.set_options(&options))),
        }
    }

    fn spell_check(&mut self, lang: Option<&str>) -> Value {
        let cells = self.text_cells(lang);
        let texts: Vec<(String, Vec<Span>)> = cells
            .iter()
            .map(|(_, t, s)| (t.clone(), s.clone()))
            .collect();
        let style = proof_text::style_of(&mut self.spell, &texts);
        let names: Vec<String> = self
            .wb
            .as_ref()
            .map(|wb| wb.sheets.iter().map(|s| s.name.clone()).collect())
            .unwrap_or_default();
        let mut issues = Vec::new();
        let mut unchecked = BTreeSet::new();
        for ((sheet, row, col), text, spans) in &cells {
            let (found, none) = proof_text::check(&mut self.spell, text, spans, style, true);
            unchecked.extend(none);
            issues.extend(
                found
                    .into_iter()
                    .filter(keep_in_cell)
                    .map(|issue| CellIssue {
                        sheet: *sheet,
                        sheet_name: names.get(*sheet).cloned().unwrap_or_default(),
                        row: *row,
                        col: *col,
                        issue,
                    }),
            );
        }
        json!({
            "issues": issues,
            "unchecked": unchecked,
            "languages": proof_text::languages(&self.spell),
        })
    }

    /// Applies corrections as one undoable edit; returns the number of cells changed.
    fn spell_replace(&mut self, edits: &[CellReplace]) -> Result<usize> {
        self.writable()?;
        let mut by_cell: BTreeMap<(usize, u32, u32), Vec<&Replace>> = BTreeMap::new();
        for e in edits {
            by_cell
                .entry((e.sheet, e.row, e.col))
                .or_default()
                .push(&e.edit);
        }
        let Some(&(first, _, _)) = by_cell.keys().next() else {
            return Ok(0);
        };
        let changed = self.edit(first, Recalc::All, |wb: &mut Workbook, _| {
            let mut n = 0;
            for (&(sheet, row, col), list) in &by_cell {
                let cell = wb
                    .sheets
                    .get_mut(sheet)
                    .ok_or(EngineError::PageOutOfRange(sheet))?
                    .cells
                    .get_mut(&(row, col))
                    .filter(|c| cell_text(c).is_some())
                    .ok_or_else(stale)?;
                replace_cell(&mut cell.value, list).ok_or_else(stale)?;
                n += 1;
            }
            Ok(n)
        })?;
        // Corrections may touch several sheets: lay them all out again.
        self.invalidate(None);
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use crate::sheet::SheetEngine;
    use serde_json::Value;

    fn engine(cells: &str) -> SheetEngine {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("t.xlsx").unwrap();
        e.exec_json(&format!(
            r#"{{"cmd":"setInputs","sheet":0,"cells":{cells}}}"#
        ))
        .unwrap();
        e
    }

    fn run(e: &mut SheetEngine, json: &str) -> Value {
        serde_json::from_str(&e.exec_json(json).unwrap()).unwrap()
    }

    fn text(e: &SheetEngine, row: u32, col: u32) -> Option<String> {
        e.wb.as_ref()?.sheets[0].cell(row, col)?.value.as_text()
    }

    // source: misspellings of common Vietnamese and English words written for this test; the
    // number, date, boolean and formula cells hold misspelt-looking text only in their formulas.
    #[test]
    fn check_walks_text_cells_only() {
        let mut e = engine(
            r#"[[0,0,"Doanh thu thág"],[0,1,"12345"],[1,0,"=\"documnet\"&\"x\""],
                [1,1,"2024-05-01"],[2,0,"TRUE"],[2,1,"the documnet"],[3,0,"Tổng cộng"]]"#,
        );
        let r = run(&mut e, r#"{"cmd":"spellCheck","lang":"vi-VN"}"#);
        let issues = r["issues"].as_array().unwrap();
        let words: Vec<(&str, u64, u64)> = issues
            .iter()
            .map(|i| {
                (
                    i["word"].as_str().unwrap(),
                    i["row"].as_u64().unwrap(),
                    i["col"].as_u64().unwrap(),
                )
            })
            .collect();
        // "thág" (A1); B3 is checked as Vietnamese: "the" is a Vietnamese syllable, "documnet" is not.
        assert_eq!(words, [("thág", 0, 0), ("documnet", 2, 1)], "{r}");
        assert_eq!(issues[0]["sheetName"], "Sheet1");
        assert!(
            issues[0]["suggestions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s == "thá"),
            "dropping the stray g: {r}"
        );
        // In English the Vietnamese cells keep their Vietnamese words (tone marks make them so).
        let r = run(&mut e, r#"{"cmd":"spellCheck","lang":"en-US"}"#);
        let words: Vec<&str> = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["word"].as_str().unwrap())
            .collect();
        assert!(
            words.contains(&"documnet") && words.contains(&"thág"),
            "{r}"
        );
    }

    #[test]
    fn replace_all_is_one_undo_step() {
        let mut e = engine(r#"[[0,0,"documnet one"],[1,0,"two documnet, documnet"]]"#);
        let r = run(&mut e, r#"{"cmd":"spellCheck","lang":"en-US"}"#);
        let same: Vec<String> = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["word"] == "documnet")
            .map(|i| {
                format!(
                    r#"{{"sheet":0,"row":{},"col":{},"start":{},"end":{},"text":"document","word":"documnet"}}"#,
                    i["row"], i["col"], i["start"], i["end"]
                )
            })
            .collect();
        assert_eq!(same.len(), 3);
        let r = run(
            &mut e,
            &format!(r#"{{"cmd":"spellReplace","edits":[{}]}}"#, same.join(",")),
        );
        assert_eq!(r["changed"], 2);
        assert_eq!(text(&e, 0, 0).as_deref(), Some("document one"));
        assert_eq!(text(&e, 1, 0).as_deref(), Some("two document, document"));
        // One undo brings every cell back.
        assert!(e.undo().unwrap());
        assert_eq!(text(&e, 0, 0).as_deref(), Some("documnet one"));
        assert_eq!(text(&e, 1, 0).as_deref(), Some("two documnet, documnet"));
        // A stale correction (the expected word is gone) changes nothing.
        let err = e.exec_json(
            r#"{"cmd":"spellReplace","edits":[{"sheet":0,"row":0,"col":0,"start":0,"end":3,"text":"x","word":"abc"}]}"#,
        );
        assert!(err.is_err());
        assert_eq!(text(&e, 0, 0).as_deref(), Some("documnet one"));
    }

    #[test]
    fn rich_text_keeps_its_runs() {
        use bo_model::sheet::{Cell, CellValue, TextRun};
        let mut e = engine("[]");
        let runs = vec![
            TextRun {
                text: "the doc".into(),
                font: None,
            },
            TextRun {
                text: "umnet".into(),
                font: Some(bo_model::sheet::Font::default()),
            },
        ];
        e.wb.as_mut().unwrap().sheets[0].set_cell(0, 0, Cell::new(CellValue::RichText(runs)));
        let r = run(
            &mut e,
            r#"{"cmd":"spellReplace","edits":[{"sheet":0,"row":0,"col":0,"start":4,"end":12,"text":"document"}]}"#,
        );
        assert_eq!(r["changed"], 1);
        let CellValue::RichText(runs) = &e.wb.as_ref().unwrap().sheets[0].cell(0, 0).unwrap().value
        else {
            panic!("rich text expected");
        };
        // "doc" + "umnet" → "document" in the first run; the emptied second run goes.
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "the document");
    }

    #[test]
    fn cell_editor_text_and_dictionary() {
        let mut e = engine("[]");
        let r = run(
            &mut e,
            r#"{"cmd":"spellText","text":"bảng tínhh","lang":"vi-VN"}"#,
        );
        let issues = r["issues"].as_array().unwrap();
        // The small first letter of a cell is not a grammar issue; "tínhh" is misspelt.
        assert_eq!(issues.len(), 1, "{r}");
        assert_eq!(issues[0]["word"], "tínhh");
        // UTF-16 offsets for the text input: "bảng " is 5 units.
        assert_eq!(
            (issues[0]["from"].as_u64(), issues[0]["to"].as_u64()),
            (Some(5), Some(10))
        );
        // Formulas are not checked.
        let r = run(
            &mut e,
            r#"{"cmd":"spellText","text":"=SUMM(A1)","lang":"vi-VN"}"#,
        );
        assert_eq!(r["issues"].as_array().map(Vec::len), Some(0));
        run(&mut e, r#"{"cmd":"spellAddWord","word":"tínhh"}"#);
        let r = run(
            &mut e,
            r#"{"cmd":"spellText","text":"bảng tínhh","lang":"vi-VN"}"#,
        );
        assert_eq!(r["issues"].as_array().map(Vec::len), Some(0));
        let o = run(&mut e, r#"{"cmd":"spellOptions","enabled":false}"#);
        assert_eq!(o["enabled"], false);
        assert_eq!(o["languages"], serde_json::json!(["vi", "en"]));
    }
}
