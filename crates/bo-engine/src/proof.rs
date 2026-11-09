//! Whole-document spelling and grammar check (BlueDoc, `docs/parity/BACKLOG.md` CORE-SPELL-005):
//! every issue of every story in checking order (main text, headers and footers, footnotes,
//! endnotes, comments — [`bo_edit::proofing::stories`]) with its sentence, suggestions and the
//! language it was checked in, the languages that could not be checked, and readability
//! statistics of the main text. Corrections come back as `proofEdit` commands.

use crate::spell::spans;
use crate::Engine;
use bo_edit::proofing::{self, Story};
use serde::Serialize;
use std::collections::BTreeSet;

/// One spelling or grammar issue.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofIssue {
    /// Story of the paragraph.
    pub story: Story,
    /// Position of the story in checking order (issues are sorted by `order`, `para`, `start`).
    pub order: usize,
    /// Paragraph index in the story (body paragraph id for the main text).
    pub para: usize,
    /// Start byte offset in the paragraph index text.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
    /// The flagged text.
    pub word: String,
    /// A grammar or style issue (rather than a misspelt word).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub grammar: bool,
    /// Grammar rule id (`bo_spell::grammar::Rule::id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<&'static str>,
    /// Corrections, best first.
    pub suggestions: Vec<String>,
    /// Language the word was checked in (`None`: any language).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Sentence text before the flagged text.
    pub before: String,
    /// Sentence text after the flagged text.
    pub after: String,
}

/// Readability statistics of the main text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofStats {
    /// Words.
    pub words: usize,
    /// Characters of the words.
    pub characters: usize,
    /// Sentences.
    pub sentences: usize,
    /// Non-empty paragraphs.
    pub paragraphs: usize,
    /// Average words per sentence.
    pub words_per_sentence: f64,
    /// Average sentences per paragraph.
    pub sentences_per_paragraph: f64,
    /// Average characters per word.
    pub characters_per_word: f64,
}

/// Result of `proofCheck`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofReport {
    /// Issues in checking order.
    pub issues: Vec<ProofIssue>,
    /// Languages found in the text without a dictionary (their text was not checked).
    pub unchecked: Vec<String>,
    /// Languages with a dictionary (primary subtags such as `vi`, `en`).
    pub languages: Vec<String>,
    /// Readability statistics of the main text.
    pub stats: ProofStats,
}

/// Proofing options (`proofOptions`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofOptionsOut {
    /// Words in capitals are not checked.
    pub ignore_caps: bool,
    /// Words with digits are not checked.
    pub ignore_digits: bool,
    /// Typed text is tagged with its detected language.
    pub auto_language: bool,
}

/// Text shown around an issue: placeholders of inline objects left out.
fn shown(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\u{FFFC}' | '\u{000C}' | '\u{000B}'))
        .map(|c| if c == '\u{2028}' || c == '\t' { ' ' } else { c })
        .collect()
}

impl Engine {
    /// Current proofing options.
    pub fn proof_options(&self) -> ProofOptionsOut {
        ProofOptionsOut {
            ignore_caps: self.spell.speller.ignore_all_caps,
            ignore_digits: self.spell.speller.ignore_digits,
            auto_language: self.spell.auto_language,
        }
    }

    /// Check every story of the document.
    pub fn proof_check(&mut self) -> ProofReport {
        let languages: Vec<String> = ["vi", "en"]
            .iter()
            .filter(|l| self.spell.speller.has_language(Some(l)))
            .map(|l| (*l).to_owned())
            .collect();
        let Some(doc) = self.document() else {
            return ProofReport {
                issues: Vec::new(),
                unchecked: Vec::new(),
                languages,
                stats: stats(&[]),
            };
        };
        // Text and language spans of every paragraph of every story.
        let stories: Vec<(Story, Vec<crate::spell::ProofText>)> = proofing::stories(doc)
            .into_iter()
            .map(|(s, ps)| {
                let texts = ps
                    .into_iter()
                    .map(|p| (bo_model::index::para_index_text(p), spans(doc, p)))
                    .collect();
                (s, texts)
            })
            .collect();
        let body = stories.first().map(|(_, t)| t.clone()).unwrap_or_default();
        let style = self.spell.document_style(&body);
        let mut issues = Vec::new();
        let mut unchecked = BTreeSet::new();
        for (order, (story, texts)) in stories.iter().enumerate() {
            for (para, (text, sp)) in texts.iter().enumerate() {
                let a = self.spell.analysis(text, sp);
                unchecked.extend(a.unchecked.iter().cloned());
                let misspelt = a.misspelt.clone();
                let mut found: Vec<ProofIssue> = Vec::new();
                for m in misspelt {
                    let word = text[m.range.clone()].to_owned();
                    found.push(issue(
                        *story,
                        order,
                        para,
                        text,
                        m.range.clone(),
                        self.spell.suggest(&word, m.lang.as_deref()),
                        m.lang,
                        None,
                    ));
                }
                for i in self.spell.issues(text, sp, style) {
                    found.push(issue(
                        *story,
                        order,
                        para,
                        text,
                        i.range.clone(),
                        i.replacement.into_iter().collect(),
                        None,
                        Some(i.rule.id()),
                    ));
                }
                found.sort_by_key(|i| (i.start, i.end));
                issues.extend(found);
            }
        }
        let body_texts: Vec<&str> = body.iter().map(|(t, _)| t.as_str()).collect();
        ProofReport {
            issues,
            unchecked: unchecked.into_iter().collect(),
            languages,
            stats: stats(&body_texts),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn issue(
    story: Story,
    order: usize,
    para: usize,
    text: &str,
    range: std::ops::Range<usize>,
    suggestions: Vec<String>,
    lang: Option<String>,
    rule: Option<&'static str>,
) -> ProofIssue {
    let s = bo_spell::lang::sentence_around(text, &range);
    ProofIssue {
        story,
        order,
        para,
        start: range.start,
        end: range.end,
        word: text[range.clone()].to_owned(),
        grammar: rule.is_some(),
        rule,
        suggestions,
        lang,
        before: shown(&text[s.start.min(range.start)..range.start]),
        after: shown(&text[range.end..s.end.max(range.end)]),
    }
}

fn stats(texts: &[&str]) -> ProofStats {
    let r = bo_spell::lang::readability(texts.iter().copied());
    ProofStats {
        words: r.words,
        characters: r.characters,
        sentences: r.sentences,
        paragraphs: r.paragraphs,
        words_per_sentence: r.words_per_sentence(),
        sentences_per_paragraph: r.sentences_per_paragraph(),
        characters_per_word: r.characters_per_word(),
    }
}

#[cfg(test)]
mod tests {
    use crate::edit::Command;
    use crate::Engine;
    use bo_edit::proofing::{ProofEdit, Story};
    use bo_edit::HfVariant;
    use bo_model::{Block, Comment, Document, Lang, Paragraph, Run, RunProps, Table, Twips};

    fn run(text: &str, lang: Option<&str>) -> Run {
        Run::text(
            text,
            RunProps {
                lang: lang.map(Lang::latin),
                ..RunProps::default()
            },
        )
    }

    fn para(runs: Vec<Run>) -> Block {
        Block::Paragraph(Paragraph::new(bo_model::ParaProps::default(), runs))
    }

    fn engine(doc: Document) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.load(doc, "t.docx").unwrap();
        e
    }

    /// Misspelt words reported by the check (grammar issues left out).
    fn words(e: &mut Engine) -> Vec<String> {
        e.proof_check()
            .issues
            .into_iter()
            .filter(|i| !i.grammar)
            .map(|i| i.word)
            .collect()
    }

    // source: Vietnamese syllable rules (bo_spell::vi) and the SCOWL en_US word list.
    #[test]
    fn each_run_is_checked_in_its_language() {
        let mut doc = crate::plain_text_document("x");
        doc.sections[0].blocks = vec![para(vec![
            run("Xin chào bạn ready ", Some("vi-VN")),
            run("the documnet is ready", Some("en-US")),
        ])];
        let mut e = engine(doc);
        let r = e.proof_check();
        let got: Vec<(&str, Option<&str>)> = r
            .issues
            .iter()
            .map(|i| (i.word.as_str(), i.lang.as_deref()))
            .collect();
        // "ready" is no Vietnamese syllable; "documnet" no English word.
        assert_eq!(got, [("ready", Some("vi-VN")), ("documnet", Some("en-US"))]);
        assert!(r.issues[1].suggestions.contains(&"document".to_owned()));
        assert!(!r.issues[0].suggestions.contains(&"ready".to_owned()));
        assert_eq!(r.unchecked, Vec::<String>::new());
        assert_eq!(r.languages, ["vi", "en"]);
        // The underline marks follow the same rules.
        assert_eq!(e.spelling_marks(None).len(), 2);
    }

    #[test]
    fn no_proof_hidden_and_languages_without_dictionary_are_skipped() {
        let mut doc = crate::plain_text_document("x");
        let skip = |text: &str, no_proof: bool| {
            Run::text(
                text,
                RunProps {
                    no_proof: no_proof.then_some(true),
                    hidden: (!no_proof).then_some(true),
                    ..RunProps::default()
                },
            )
        };
        doc.sections[0].blocks = vec![
            para(vec![
                run("Bonjour mes amies ", Some("fr-FR")),
                skip("documnet ", true),
                skip("wrold ", false),
                run("sai chínhh tả rồi", None),
            ]),
            // A Vietnamese style wins over the docDefaults language.
            para(vec![run("Hello", None)]),
        ];
        doc.default_run.lang = Some(Lang::latin("en-US"));
        let mut style = bo_model::Style::new("Viet", "Viet", bo_model::StyleKind::Paragraph);
        style.rpr.lang = Some(Lang::latin("vi-VN"));
        doc.styles.styles.push(style);
        if let Block::Paragraph(p) = &mut doc.sections[0].blocks[1] {
            p.props.style = Some("Viet".into());
        }
        let mut e = engine(doc);
        let r = e.proof_check();
        let got: Vec<&str> = r.issues.iter().map(|i| i.word.as_str()).collect();
        // "chínhh" has Vietnamese letters: checked as Vietnamese in the en-US default.
        assert_eq!(got, ["chínhh", "Hello"]);
        assert_eq!(r.issues[0].lang.as_deref(), Some("vi-VN"));
        assert_eq!(r.issues[1].lang.as_deref(), Some("vi-VN"));
        assert_eq!(r.unchecked, ["fr-FR"]);
    }

    /// A document with one misspelt word in every story.
    fn stories_doc() -> Document {
        let mut doc = crate::plain_text_document("x");
        let mut t = Table::simple(1, 1, Twips(3000));
        t.rows[0].cells[0].blocks = vec![para(vec![run("Bảng cóx lỗi.", None)])];
        doc.sections[0].blocks = vec![
            para(vec![run("Một lỗii ở đây.", None)]),
            Block::Table(t),
            para(vec![]),
        ];
        doc.sections[0].headers.default = Some(vec![para(vec![run("Đầu trangg", None)])]);
        doc.footnotes
            .insert(1, vec![para(vec![run("Chú thíchh", None)])]);
        doc.endnotes
            .insert(1, vec![para(vec![run("Kết thúcc", None)])]);
        doc.comments.push(Comment {
            id: 0,
            author: "A".into(),
            blocks: vec![para(vec![run("Nhận xétt", None)])],
            ..Comment::default()
        });
        doc
    }

    #[test]
    fn issues_come_in_order_across_stories() {
        let mut e = engine(stories_doc());
        let r = e.proof_check();
        let got: Vec<(&str, Story, usize)> = r
            .issues
            .iter()
            .map(|i| (i.word.as_str(), i.story, i.para))
            .collect();
        let header = Story::Header {
            section: 0,
            variant: HfVariant::Default,
        };
        assert_eq!(
            got,
            [
                ("lỗii", Story::Body, 0),
                ("cóx", Story::Body, 1),
                ("trangg", header, 0),
                ("thíchh", Story::Footnote { id: 1 }, 0),
                ("thúcc", Story::Endnote { id: 1 }, 0),
                ("xétt", Story::Comment { index: 0 }, 0),
            ]
        );
        let orders: Vec<usize> = r.issues.iter().map(|i| i.order).collect();
        assert!(orders.windows(2).all(|w| w[0] <= w[1]), "{orders:?}");
        // Sentence context around the word.
        assert_eq!(
            (r.issues[0].before.as_str(), r.issues[0].after.as_str()),
            ("Một ", " ở đây.")
        );
        // Readability of the main text: 7 words in 2 sentences, 2 paragraphs.
        assert_eq!(
            (r.stats.words, r.stats.sentences, r.stats.paragraphs),
            (7, 2, 2)
        );
        assert!((r.stats.words_per_sentence - 3.5).abs() < 1e-9);
        // The JSON shape the dialog reads.
        let json = e.command_json(r#"{"cmd":"proofCheck"}"#).unwrap();
        assert!(
            json.contains(r#""story":{"kind":"header","section":0,"variant":"default"}"#),
            "{json}"
        );
        assert!(json.contains(r#""wordsPerSentence":3.5"#), "{json}");
    }

    #[test]
    fn change_all_is_one_undo_step_across_stories() {
        let mut doc = crate::plain_text_document("Một lỗii và lỗii nữa.");
        doc.sections[0].headers.default = Some(vec![para(vec![run("lỗii", None)])]);
        doc.comments.push(Comment {
            id: 0,
            author: "A".into(),
            blocks: vec![para(vec![run("cũng lỗii", None)])],
            ..Comment::default()
        });
        let mut e = engine(doc);
        let r = e.proof_check();
        let edits: Vec<ProofEdit> = r
            .issues
            .iter()
            .filter(|i| !i.grammar && i.word == "lỗii")
            .map(|i| ProofEdit {
                story: i.story,
                para: i.para,
                start: i.start,
                end: i.end,
                text: Some("lỗi".into()),
                lang: None,
            })
            .collect();
        assert_eq!(edits.len(), 4);
        e.command(Command::ProofEdit { edits }).unwrap();
        assert_eq!(words(&mut e), Vec::<String>::new());
        let doc = e.document().unwrap();
        assert_eq!(
            doc.sections[0].blocks[0]
                .as_paragraph()
                .unwrap()
                .plain_text(),
            "Một lỗi và lỗi nữa."
        );
        assert_eq!(doc.comments[0].text(), "cũng lỗi");
        let hf = doc.sections[0].headers.default.as_ref().unwrap();
        assert_eq!(hf[0].as_paragraph().unwrap().plain_text(), "lỗi");
        // One undo brings every story back.
        e.command(Command::Undo).unwrap();
        assert_eq!(words(&mut e), ["lỗii", "lỗii", "lỗii", "lỗii"]);
        e.command(Command::Redo).unwrap();
        assert_eq!(words(&mut e), Vec::<String>::new());
    }

    #[test]
    fn language_of_the_selection_and_do_not_check() {
        let mut e = engine(crate::plain_text_document("Hello wrold"));
        assert_eq!(words(&mut e), ["wrold"]);
        e.command(Command::SelectAll).unwrap();
        let r = e
            .command_json(r#"{"cmd":"formatText","format":{"kind":"language","lang":"fr-FR"}}"#)
            .unwrap();
        assert!(r.contains(r#""lang":"fr-FR""#), "{r}");
        let rep = e.proof_check();
        assert_eq!(rep.issues, Vec::new());
        assert_eq!(rep.unchecked, ["fr-FR"]);
        e.command(Command::Undo).unwrap();
        assert_eq!(words(&mut e), ["wrold"]);
        let r = e
            .command_json(r#"{"cmd":"formatText","format":{"kind":"noProof","on":true}}"#)
            .unwrap();
        assert!(r.contains(r#""noProof":true"#), "{r}");
        assert_eq!(words(&mut e), Vec::<String>::new());
        // The language selector of the dialog tags one word.
        e.command(Command::Undo).unwrap();
        let edit = ProofEdit {
            story: Story::Body,
            para: 0,
            start: 6,
            end: 11,
            text: None,
            lang: Some("de-DE".into()),
        };
        e.command(Command::ProofEdit { edits: vec![edit] }).unwrap();
        let p = e.document().unwrap().sections[0].blocks[0]
            .as_paragraph()
            .unwrap()
            .clone();
        assert_eq!(p.runs.len(), 2);
        assert_eq!(p.runs[1].props.lang, Some(Lang::latin("de-DE")));
        assert_eq!(e.proof_check().unchecked, ["de-DE"]);
    }

    #[test]
    fn options_and_automatic_language() {
        let mut e = engine(crate::plain_text_document("A4 mp3x UBNDD"));
        assert_eq!(words(&mut e), Vec::<String>::new());
        let r = e
            .command_json(r#"{"cmd":"proofOptions","ignoreCaps":false,"ignoreDigits":false}"#)
            .unwrap();
        assert!(
            r.contains(
                r#""proofOptions":{"ignoreCaps":false,"ignoreDigits":false,"autoLanguage":false}"#
            ),
            "{r}"
        );
        assert_eq!(words(&mut e), ["A4", "mp3x", "UBNDD"]);
        // Automatic language: typed Vietnamese text is tagged vi-VN after the word ends.
        let mut e = engine(crate::plain_text_document(""));
        e.command_json(r#"{"cmd":"proofOptions","autoLanguage":true}"#)
            .unwrap();
        for c in "Hôm nay trời đẹp ".chars() {
            e.command(Command::InsertText {
                text: c.to_string(),
            })
            .unwrap();
        }
        let r = e.command(Command::State).unwrap();
        assert_eq!(r.format.lang.as_deref(), Some("vi-VN"));
        let p = e.document().unwrap().sections[0].blocks[0]
            .as_paragraph()
            .unwrap()
            .clone();
        assert_eq!(p.plain_text(), "Hôm nay trời đẹp ");
        assert!(p
            .runs
            .iter()
            .all(|r| r.props.lang == Some(Lang::latin("vi-VN"))));
    }
}
