//! Sources part, citation and bibliography fields in an editing session; BibTeX and RIS.
#![allow(clippy::unwrap_used)]

use super::source::{new_tag, parse_sources, write_sources, NS};
use super::*;
use bo_model::{Run, Section};

fn person(last: &str, first: &str) -> Person {
    Person::new(last, first)
}

fn book(tag: &str, last: &str, first: &str, title: &str, year: &str) -> Source {
    let mut s = Source {
        tag: tag.into(),
        kind: "Book".into(),
        contributors: vec![Contributor {
            role: "Author".into(),
            corporate: None,
            people: vec![person(last, first)],
        }],
        ..Source::default()
    };
    s.set("Title", title);
    s.set("Year", year);
    s.set("Publisher", "Blue Press");
    s
}

fn session(text: &str) -> Session {
    let p = Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    );
    let mut s = Session::new(Document {
        sections: vec![Section::new(vec![Block::Paragraph(p)])],
        ..Document::default()
    });
    s.set_selection(Selection::caret(Pos::new(0, text.len())));
    s
}

fn body(s: &Session) -> Vec<String> {
    s.doc().sections[0]
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(Paragraph::plain_text)
        .collect()
}

fn one(tag: &str) -> Citation {
    Citation {
        items: vec![CiteItem {
            tag: tag.into(),
            ..CiteItem::default()
        }],
        lcid: None,
    }
}

/// §22.6: every element of a source survives reading and writing the part.
#[test]
fn sources_part_round_trip() {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<b:Sources SelectedStyle="\Some.XSL" StyleName="IEEE" xmlns:b="{NS}" xmlns="{NS}">
<b:Source><b:Tag>Smi20</b:Tag><b:SourceType>JournalArticle</b:SourceType>
<b:Guid>{{11111111-2222-3333-4444-555555555555}}</b:Guid><b:LCID>1033</b:LCID>
<b:Author><b:Author><b:NameList>
<b:Person><b:Last>Smith</b:Last><b:First>John</b:First><b:Middle>K.</b:Middle></b:Person>
<b:Person><b:Last>Đỗ</b:Last><b:First>Thị Hà</b:First></b:Person>
</b:NameList></b:Author><b:Editor><b:NameList><b:Person><b:Last>Brown</b:Last></b:Person></b:NameList></b:Editor></b:Author>
<b:Title>Signals &amp; systems</b:Title><b:JournalName>Journal of Data</b:JournalName>
<b:Year>2020</b:Year><b:Month>May</b:Month><b:Day>4</b:Day><b:Volume>12</b:Volume><b:Issue>3</b:Issue>
<b:Pages>100-120</b:Pages><b:URL>https://example.org/a?x=1&amp;y=2</b:URL><b:DOI>10.1000/x</b:DOI>
<b:YearAccessed>2024</b:YearAccessed><b:MonthAccessed>1</b:MonthAccessed><b:DayAccessed>15</b:DayAccessed>
<b:RefOrder>1</b:RefOrder></b:Source>
<b:Source><b:Tag>WHO22</b:Tag><b:SourceType>Report</b:SourceType>
<b:Author><b:Author><b:Corporate>World Health Organization</b:Corporate></b:Author></b:Author>
<b:Title>Global report</b:Title><b:Year>2022</b:Year><b:Publisher>WHO Press</b:Publisher><b:City>Geneva</b:City></b:Source>
<b:Source><b:Tag>Placeholder1</b:Tag><b:SourceType>Book</b:SourceType><b:Guid>{{AAAAAAAA-2222-3333-4444-555555555555}}</b:Guid></b:Source>
</b:Sources>"#
    );
    let list = parse_sources(&xml).unwrap();
    assert_eq!(list.style_name, "IEEE");
    assert_eq!(list.sources.len(), 3);
    let a = &list.sources[0];
    assert_eq!(
        (a.tag.as_str(), a.kind.as_str()),
        ("Smi20", "JournalArticle")
    );
    assert_eq!(a.role("Author").unwrap().people.len(), 2);
    assert_eq!(a.role("Author").unwrap().people[0].given(), "John K.");
    assert_eq!(a.role("Editor").unwrap().people[0].last, "Brown");
    assert_eq!(a.get("Title"), "Signals & systems");
    assert_eq!(a.get("URL"), "https://example.org/a?x=1&y=2");
    assert_eq!(a.fields.len(), 16);
    assert_eq!(
        list.sources[1].role("Author").unwrap().corporate.as_deref(),
        Some("World Health Organization")
    );
    assert!(list.sources[2].is_placeholder() && !list.sources[1].is_placeholder());
    // Written and read again: the same sources.
    let again = parse_sources(&write_sources(&list)).unwrap();
    assert_eq!(again, list);
    // Not a sources part.
    assert!(parse_sources("<a/>").is_none());
    assert!(parse_sources(&format!("<x xmlns=\"{NS}\"/>")).is_none());
}

#[test]
fn tags() {
    let s = book("", "Đặng", "Hồng", "Thống kê", "2019");
    assert_eq!(new_tag(&s, &[]), "Dan19");
    assert_eq!(new_tag(&s, &["dan19".into()]), "Dan191");
    let mut t = Source::default();
    t.set("Title", "the quiet book");
    assert_eq!(new_tag(&t, &[]), "The");
    assert_eq!(new_tag(&Source::default(), &[]), "Src");
}

/// Insert a citation and a bibliography, switch style (one undo step), edit the citation.
#[test]
fn cite_in_a_session() {
    let mut s = session("As shown ");
    s.cite_save_sources(
        vec![
            book("Smi20", "Smith", "John", "Research methods", "2020"),
            book("Doe18", "Doe", "Jane", "Field notes", "2018"),
        ],
        None,
    )
    .unwrap();
    // The part is a custom XML part with the bibliography namespace as its schema.
    let part = &s.doc().custom_xml[0];
    assert!(part.xml.contains("<b:Tag>Smi20</b:Tag>"));
    assert_eq!(part.schema_refs, [NS]);
    assert!(part.item_id.is_some() && part.props_part.is_some());
    assert_eq!(s.cite_state().style, StyleId::Apa);

    s.cite_insert(&one("Smi20"), Vec::new()).unwrap();
    assert_eq!(body(&s), ["As shown (Smith, 2020)"]);
    // The caret is after the citation; it touches it.
    // (A field counts as one object character in paragraph offsets.)
    let after = "As shown ".len() + '\u{FFFC}'.len_utf8();
    assert_eq!(s.selection().focus, Pos::new(0, after));
    assert_eq!(s.cite_state().citation, Some(one("Smi20")));
    s.insert_text(".").unwrap();
    s.split_paragraph().unwrap();
    s.cite_insert_bibliography(Some("References"), None)
        .unwrap();
    assert_eq!(
        body(&s),
        [
            "As shown (Smith, 2020).",
            "References",
            "Doe, J. (2018). Field notes. Blue Press.",
            "Smith, J. (2020). Research methods. Blue Press.",
            ""
        ]
    );
    let blocks = &s.doc().sections[0].blocks;
    let para = |i: usize| blocks[i].as_paragraph().unwrap();
    assert_eq!(para(1).props.style.as_deref(), Some("Heading1"));
    assert_eq!(para(2).props.style.as_deref(), Some("Bibliography"));
    assert!(s.doc().style("Bibliography").is_some() && s.doc().style("Heading1").is_some());
    // The title is italic (§17.3.2.16).
    assert!(para(2)
        .runs
        .iter()
        .any(|r| r.props.italic == Some(true) && r.plain_text() == "Field notes"));
    assert!(s.cite_state().has_bibliography);

    // Switching style rewrites the citation and the list: one undo step.
    let n = s.cite_set_style(StyleId::Ieee).unwrap();
    assert_eq!(n, 2);
    assert_eq!(
        body(&s)[..4],
        [
            "As shown [1].",
            "References",
            "[1]\tJ. Smith, Research methods. Blue Press, 2020.",
            "[2]\tJ. Doe, Field notes. Blue Press, 2018.",
        ]
    );
    assert!(s.doc().custom_xml[0].xml.contains("StyleName=\"IEEE\""));
    assert!(s.undo());
    assert_eq!(body(&s)[0], "As shown (Smith, 2020).");
    assert_eq!(body(&s)[2], "Doe, J. (2018). Field notes. Blue Press.");
    assert!(s.doc().custom_xml[0].xml.contains("StyleName=\"APA\""));
    assert!(s.redo());
    assert_eq!(body(&s)[0], "As shown [1].");
    assert!(s.undo());

    // Edit the citation: a page, a second source, no author.
    s.set_selection(Selection::caret(Pos::new(0, "As shown ".len())));
    let mut c = s.cite_state().citation.unwrap();
    c.items[0].pages = "12".into();
    c.items[0].no_author = true;
    c.items.push(CiteItem {
        tag: "Doe18".into(),
        ..CiteItem::default()
    });
    s.cite_edit(&c).unwrap();
    assert_eq!(body(&s)[0], "As shown (2020, p. 12; Doe, 2018).");
    // Not at a citation.
    s.set_selection(Selection::caret(Pos::new(1, 0)));
    assert!(s.cite_edit(&c).is_err());

    // A cited source cannot be deleted; an uncited one can, and leaves the list.
    assert!(s.cite_delete_source("Smi20").is_err());
    s.cite_save_sources(vec![book("", "Lee", "Anna", "Sampling", "2019")], None)
        .unwrap();
    assert_eq!(body(&s)[3], "Lee, A. (2019). Sampling. Blue Press.");
    assert_eq!(s.cite_state().sources[2].source.tag, "Lee19");
    s.cite_delete_source("Lee19").unwrap();
    assert_eq!(body(&s).len(), 5);
    assert!(s.cite_delete_source("Lee19").is_err());

    // Editing a source updates what cites it; a changed tag moves the citations along.
    let mut edited = book("Smith2020", "Smith", "John", "Research methods", "2021");
    edited.set("Edition", "2");
    s.cite_save_sources(vec![edited], Some("Smi20")).unwrap();
    assert_eq!(body(&s)[0], "As shown (2021, p. 12; Doe, 2018).");
    assert!(body(&s)[3].starts_with("Smith, J. (2021). Research methods (2nd ed.)."));
    let state = s.cite_state();
    assert!(state
        .sources
        .iter()
        .any(|x| x.source.tag == "Smith2020" && x.cited));
    // A tag that another source has is refused.
    assert!(s
        .cite_save_sources(vec![book("Doe18", "X", "Y", "Z", "1")], Some("Smith2020"))
        .is_err());
}

/// A placeholder is cited by its tag until it is filled in; a new source can be added
/// while citing (one undo step).
#[test]
fn placeholders_and_new_sources() {
    let mut s = session("");
    let placeholder = Source {
        tag: "Placeholder1".into(),
        kind: "Book".into(),
        ..Source::default()
    };
    s.cite_insert(&one("Placeholder1"), vec![placeholder])
        .unwrap();
    assert_eq!(body(&s), ["(Placeholder1)"]);
    let st = s.cite_state();
    assert!(st.sources[0].placeholder && st.sources[0].cited);
    // Filled in: the citation follows.
    s.cite_save_sources(
        vec![book("Placeholder1", "Tran", "Lan", "Notes", "2022")],
        Some("Placeholder1"),
    )
    .unwrap();
    assert_eq!(body(&s), ["(Tran, 2022)"]);
    assert!(s.undo());
    assert_eq!(body(&s), ["(Placeholder1)"]);
    assert!(s.undo());
    assert_eq!(body(&s), [""]);
    assert_eq!(s.doc().custom_xml.len(), 0);
    assert!(s.cite_insert(&Citation::default(), Vec::new()).is_err());
}

/// "Update fields" refreshes citations and the bibliography too; a document without a
/// sources part keeps the results it came with.
#[test]
fn update_fields_refreshes_citations() {
    let field = |instr: &str, cached: &str| Run {
        props: RunProps::default(),
        content: vec![RunContent::Field {
            instr: instr.into(),
            cached: cached.into(),
            locked: false,
            codes: false,
        }],
    };
    let doc = Document {
        sections: vec![Section::new(vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![field("CITATION Smi20 \\l 1033", "(stale)")],
        ))])],
        ..Document::default()
    };
    let env = fields::FieldEnv::default();
    let up = fields::update(&doc, &[], &env);
    assert_eq!(up.count, 0);
    let mut with = doc.clone();
    with.custom_xml.push(CustomXmlPart {
        part_name: "/customXml/item1.xml".into(),
        xml: write_sources(&SourceList {
            style_name: "Harvard - Anglia".into(),
            selected_style: String::new(),
            sources: vec![book("Smi20", "Smith", "John", "Research methods", "2020")],
        }),
        item_id: None,
        schema_refs: Vec::new(),
        props_part: None,
    });
    let up = fields::update(&with, &[], &env);
    assert_eq!(up.count, 1);
    assert_eq!(
        up.sections[0][0].as_paragraph().unwrap().plain_text(),
        "(Smith, 2020)"
    );
}

#[test]
fn bibtex_import() {
    let text = r#"
This line is a comment.
@comment{ignored = {x}}
@string{jd = "Journal of " # {Data}}
@STRING(bp = {Blue Press})

@article{nguyen2021deep,
  author  = {Nguyen, Minh T. and John K. Smith and van der Berg, Anna},
  title   = {Deep {L}earning for Text: A~Survey},
  journal = jd,
  year    = 2021,
  month   = may,
  volume  = {12},
  number  = "3",
  pages   = {100--120},
  doi     = {10.1000/jd.2021.5},
  langid  = {english}
}

@book{Müller:2019,
  author    = "M{\"u}ller, J{\'o}zef and {\DJ}\`{a}o, V\u{a}n and \v{C}apek, Karel",
  title     = "Caf\'{e} \& Na\"ive \c{C}a \~nu {\ss} \o{} \ae",
  publisher = bp,
  address   = {Hà Nội},
  edition   = {2},
  year      = {2019},
  isbn      = {978-0-00-000000-0},
}

@techreport{who,
  author = {{World Health Organization}},
  title = {Global report},
  institution = {WHO Press},
  year = {2022}
}

@phdthesis{pham17,
  author = {Hoa Pham},
  title = {Graph algorithms},
  school = {Hanoi University},
  year = 2017,
}

@inproceedings{garcia,
  author = {Garcia, Luis and others},
  title = {Fast parsing},
  booktitle = {Proceedings of the Conference},
  editor = {Brown, Mark},
  year = {2018}, pages = {10-15}
}

@misc{site, title = {Getting started}, url = {https://example.org/a_b?x=1}, urldate = {2024-01-15}, date = {2023-05-04}}
"#;
    let v = bibtex::import(text, &["who".into()]);
    let tags: Vec<&str> = v.iter().map(|s| s.tag.as_str()).collect();
    // Keys become tags: ASCII only, unique among the taken ones.
    assert_eq!(
        tags,
        [
            "nguyen2021deep",
            "Muller:2019",
            "who1",
            "pham17",
            "garcia",
            "site"
        ]
    );
    let a = &v[0];
    assert_eq!(a.kind, "JournalArticle");
    // Names: "Family, Given", "Given Family", and a particle that belongs to the family name.
    assert_eq!(
        a.role("Author").unwrap().people,
        [
            person("Nguyen", "Minh T."),
            person("Smith", "John K."),
            person("van der Berg", "Anna")
        ]
    );
    // Braces protect case and are dropped; `~` is a space; @string and `#` are expanded.
    assert_eq!(a.get("Title"), "Deep Learning for Text: A Survey");
    assert_eq!(a.get("JournalName"), "Journal of Data");
    assert_eq!((a.get("Year"), a.get("Month")), ("2021", "5"));
    assert_eq!((a.get("Volume"), a.get("Issue")), ("12", "3"));
    assert_eq!(a.get("Pages"), "100-120");
    assert_eq!(a.get("LCID"), "1033");
    // LaTeX accents and letters become Unicode.
    let b = &v[1];
    assert_eq!(b.kind, "Book");
    assert_eq!(
        b.role("Author").unwrap().people,
        [
            person("Müller", "Józef"),
            person("Đào", "Văn"),
            person("Čapek", "Karel")
        ]
    );
    assert_eq!(b.get("Title"), "Café & Naïve Ça ñu ß ø æ");
    assert_eq!(b.get("Publisher"), "Blue Press");
    assert_eq!((b.get("City"), b.get("Edition")), ("Hà Nội", "2"));
    assert_eq!(b.get("StandardNumber"), "978-0-00-000000-0");
    // A name in double braces is a corporate author.
    assert_eq!(
        v[2].role("Author").unwrap().corporate.as_deref(),
        Some("World Health Organization")
    );
    assert_eq!(
        (v[2].kind.as_str(), v[2].get("Institution")),
        ("Report", "WHO Press")
    );
    assert_eq!(v[3].get("ThesisType"), "PhD thesis");
    assert_eq!(v[3].role("Author").unwrap().people, [person("Pham", "Hoa")]);
    // "and others" is not a person.
    assert_eq!(
        v[4].role("Author").unwrap().people,
        [person("Garcia", "Luis")]
    );
    assert_eq!(v[4].kind, "ConferenceProceedings");
    assert_eq!(v[4].get("ConferenceName"), "Proceedings of the Conference");
    assert_eq!(
        v[4].role("Editor").unwrap().people,
        [person("Brown", "Mark")]
    );
    // A misc entry with a URL is a web page; dates are split.
    let w = &v[5];
    assert_eq!(w.kind, "InternetSite");
    assert_eq!(w.get("URL"), "https://example.org/a_b?x=1");
    assert_eq!(
        (w.get("Year"), w.get("Month"), w.get("Day")),
        ("2023", "5", "4")
    );
    assert_eq!(
        (
            w.get("YearAccessed"),
            w.get("MonthAccessed"),
            w.get("DayAccessed")
        ),
        ("2024", "1", "15")
    );
    // Broken input does not panic and yields what is complete.
    assert_eq!(bibtex::import("@book{x, title = {Unclosed", &[]).len(), 1);
    assert_eq!(bibtex::import("@", &[]).len(), 0);
    assert_eq!(bibtex::import("@book", &[]).len(), 0);
}

#[test]
fn bibtex_export_round_trip() {
    let mut art = Source {
        tag: "Smi20".into(),
        kind: "JournalArticle".into(),
        contributors: vec![Contributor {
            role: "Author".into(),
            corporate: None,
            people: vec![person("Smith", "John K."), person("Đỗ", "Thị Hà")],
        }],
        ..Source::default()
    };
    art.set("Title", "Signals & systems: 100% of_it");
    art.set("JournalName", "Journal of Data");
    art.set("Year", "2020");
    art.set("Volume", "12");
    art.set("Issue", "3");
    art.set("Pages", "100-120");
    art.set("DOI", "10.1000/x");
    let mut org = Source {
        tag: "WHO22".into(),
        kind: "Report".into(),
        contributors: vec![Contributor {
            role: "Author".into(),
            corporate: Some("World Health Organization".into()),
            people: Vec::new(),
        }],
        ..Source::default()
    };
    org.set("Title", "Global report");
    org.set("Year", "2022");
    org.set("Institution", "WHO Press");
    let placeholder = Source {
        tag: "Placeholder1".into(),
        kind: "Book".into(),
        ..Source::default()
    };
    let text = bibtex::export(&[art.clone(), org.clone(), placeholder]);
    assert!(text.starts_with("@article{Smi20,\n  author = {Smith, John K. and Đỗ, Thị Hà},\n"));
    assert!(text.contains("  title = {Signals \\& systems: 100\\% of\\_it},\n"));
    assert!(text.contains("  pages = {100--120},\n"));
    assert!(text.contains("@techreport{WHO22,\n  author = {{World Health Organization}},\n"));
    assert!(!text.contains("Placeholder1"));
    // Imported again: the same sources.
    assert_eq!(bibtex::import(&text, &[]), [art, org]);
}

#[test]
fn ris_import() {
    let text = "\u{FEFF}TY  - JOUR\r\nAU  - Nguyen, Minh T.\r\nAU  - Smith, John K., Jr.\r\nTI  - Deep learning\r\n      for text\r\nJO  - Journal of Data\r\nPY  - 2021/05/04/\r\nVL  - 12\r\nIS  - 3\r\nSP  - 100\r\nEP  - 120\r\nDO  - 10.1000/jd.2021.5\r\nLA  - vi\r\nID  - nguyen 2021\r\nER  - \r\n\r\nTY  - CHAP\nA1  - Lee, Anna\nA2  - Brown, Mark\nT1  - Sampling\nT2  - Handbook of statistics\nY1  - 2019\nPB  - Blue Press\nCY  - London\nSP  - 45-67\nER  -\nTY  - THES\nAU  - Pham, Hoa\nTI  - Graph algorithms\nPB  - Hanoi University\nM3  - PhD thesis\nPY  - 2017\nER  - \nTY  - ELEC\nTI  - Getting started\nUR  - https://example.org/start\nY2  - 2024/01/15\nTY  - BOOK\nAU  - Plato\nTI  - Republic\n";
    let v = ris::import(text, &[]);
    assert_eq!(v.len(), 5);
    let a = &v[0];
    assert_eq!(
        (a.tag.as_str(), a.kind.as_str()),
        ("nguyen2021", "JournalArticle")
    );
    assert_eq!(
        a.role("Author").unwrap().people,
        [person("Nguyen", "Minh T."), person("Smith", "John K.")]
    );
    // A continuation line belongs to the field before it.
    assert_eq!(a.get("Title"), "Deep learning for text");
    assert_eq!(a.get("JournalName"), "Journal of Data");
    assert_eq!(
        (a.get("Year"), a.get("Month"), a.get("Day")),
        ("2021", "5", "4")
    );
    assert_eq!(a.get("Pages"), "100-120");
    assert_eq!((a.get("DOI"), a.get("LCID")), ("10.1000/jd.2021.5", "1066"));
    let c = &v[1];
    assert_eq!((c.tag.as_str(), c.kind.as_str()), ("Lee19", "BookSection"));
    assert_eq!(c.get("BookTitle"), "Handbook of statistics");
    assert_eq!(c.role("Editor").unwrap().people, [person("Brown", "Mark")]);
    assert_eq!((c.get("City"), c.get("Pages")), ("London", "45-67"));
    let t = &v[2];
    assert_eq!(t.kind, "Report");
    assert_eq!(
        (t.get("ThesisType"), t.get("Institution")),
        ("PhD thesis", "Hanoi University")
    );
    assert_eq!(t.get("Publisher"), "");
    // A record without ER ends at the next TY (and at the end of the text).
    assert_eq!(
        (v[3].kind.as_str(), v[3].get("YearAccessed")),
        ("InternetSite", "2024")
    );
    assert_eq!(v[4].role("Author").unwrap().people, [person("Plato", "")]);
    assert_eq!(ris::import("no records here", &[]).len(), 0);
}
