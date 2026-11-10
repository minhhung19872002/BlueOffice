//! Equations in the style other office applications write (CORE-MATH-010, ADR-0015 addendum C): a
//! math font in `w:rPr` on every run, `m:ctrlPr` in every structure, non-default structure
//! properties. Read into the tree, edited and written again: what was not edited is byte-identical,
//! what was edited carries the properties it inherits.
#![allow(clippy::unwrap_used)]

use bo_math::edit::{
    self as me, all_slots, autobuild, delete, delete_selected, insert_text, left, right, slot_at,
    Deleted, MathPos, Step,
};
use bo_math::{from_omml, to_omml, Node};

const NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
/// The math font (ECMA-376 Part 1, §17.3.2.26 rFonts) and italic, as on every math run.
const FI: &str =
    r#"<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/><w:i/></w:rPr>"#;
/// The math font, upright (function names).
const F: &str = r#"<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/></w:rPr>"#;

fn r(t: &str) -> String {
    format!("<m:r>{FI}<m:t>{t}</m:t></m:r>")
}

fn ctrl() -> String {
    format!("<m:ctrlPr>{FI}</m:ctrlPr>")
}

fn zone(inner: &str) -> String {
    format!(r#"<m:oMath xmlns:m="{NS}">{inner}</m:oMath>"#)
}

/// The tree written as the in-place editor writes it.
fn write(nodes: &[Node]) -> String {
    let mut n = nodes.to_vec();
    me::normalize(&mut n);
    to_omml(&n).replacen("<m:oMath>", &format!(r#"<m:oMath xmlns:m="{NS}">"#), 1)
}

fn p(steps: &[(usize, usize)], offset: usize) -> MathPos {
    MathPos::new(
        steps
            .iter()
            .map(|&(item, slot)| Step { item, slot })
            .collect(),
        offset,
    )
}

// Parts of the equation (each a structure or run as another application writes it).
fn nary() -> String {
    // ∑ with its limits below and above although the file says subSup, growing; ECMA-376 Part 1,
    // §22.1.2.72 naryPr.
    format!(
        r#"<m:nary><m:naryPr><m:chr m:val="∑"/><m:limLoc m:val="subSup"/><m:grow m:val="1"/>{}</m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary>"#,
        ctrl(),
        r("k=1"),
        r("n"),
        r("k")
    )
}

fn frac() -> String {
    // A linear fraction (§22.1.2.117 type lin).
    format!(
        r#"<m:f><m:fPr><m:type m:val="lin"/>{}</m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>"#,
        ctrl(),
        r("1"),
        r("k")
    )
}

fn delim() -> String {
    // Square brackets, a separator character, no growth.
    format!(
        r#"<m:d><m:dPr><m:begChr m:val="["/><m:sepChr m:val=";"/><m:endChr m:val="]"/><m:grow m:val="0"/>{}</m:dPr><m:e>{}</m:e><m:e>{}</m:e></m:d>"#,
        ctrl(),
        r("a"),
        r("b")
    )
}

fn func() -> String {
    format!(
        r#"<m:func><m:funcPr>{}</m:funcPr><m:fName><m:r><m:rPr><m:sty m:val="p"/></m:rPr>{F}<m:t>sin</m:t></m:r></m:fName><m:e>{}</m:e></m:func>"#,
        ctrl(),
        r("x")
    )
}

fn matrix() -> String {
    format!(
        r#"<m:m><m:mPr><m:mcs><m:mc><m:mcPr><m:count m:val="2"/><m:mcJc m:val="center"/></m:mcPr></m:mc></m:mcs>{}</m:mPr><m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr></m:m>"#,
        ctrl(),
        r("1"),
        r("0")
    )
}

fn script_run() -> String {
    // Script letters (§22.1.2 scr) with the run's own colour.
    r#"<m:r><m:rPr><m:scr m:val="script"/><m:sty m:val="i"/></m:rPr><w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/><w:color w:val="C00000"/></w:rPr><m:t>L</m:t></m:r>"#.to_owned()
}

fn equation() -> String {
    zone(
        &[
            script_run(),
            r("="),
            nary(),
            r("+"),
            frac(),
            r("+"),
            delim(),
            func(),
            matrix(),
        ]
        .concat(),
    )
}

// source: ECMA-376 Part 1, §22.1.2 (property elements of each structure and run kept verbatim).
#[test]
fn foreign_equation_round_trips_byte_identical() {
    let xml = equation();
    let nodes = from_omml(&xml).unwrap();
    assert_eq!(write(&nodes), xml);
    assert!(bo_math::read::same_omml(&xml, &write(&nodes)));
    // The values the tree models are read through the properties.
    assert!(matches!(
        &nodes[2],
        Node::Nary {
            chr: '∑',
            sub: Some(_),
            sup: Some(_),
            ..
        }
    ));
    assert!(
        matches!(&nodes[6], Node::Delim { open: Some('['), close: Some(']'), parts, .. } if parts.len() == 2)
    );
}

#[test]
fn typing_into_a_run_keeps_its_properties() {
    let xml = equation();
    let mut nodes = from_omml(&xml).unwrap();
    // The denominator of the fraction (item 4, slot 1): "k" → "kx".
    let at = insert_text(&mut nodes, &p(&[(4, 1)], 1), "x").unwrap();
    assert_eq!(at, p(&[(4, 1)], 2));
    let out = write(&nodes);
    let edited = format!(
        r#"<m:f><m:fPr><m:type m:val="lin"/>{}</m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>"#,
        ctrl(),
        r("1"),
        r("kx")
    );
    assert_eq!(
        out,
        zone(
            &[
                script_run(),
                r("="),
                nary(),
                r("+"),
                edited,
                r("+"),
                delim(),
                func(),
                matrix()
            ]
            .concat()
        )
    );
    // Typing after the script letter: into that run, colour and script kept.
    let mut nodes = from_omml(&xml).unwrap();
    insert_text(&mut nodes, &p(&[], 1), "′").unwrap();
    let out = write(&nodes);
    assert!(
        out.contains(r#"<w:color w:val="C00000"/></w:rPr><m:t>L′</m:t>"#),
        "{out}"
    );
    assert!(out.contains(&nary()) && out.contains(&frac()) && out.contains(&matrix()));
}

#[test]
fn a_run_typed_into_an_empty_slot_takes_the_structures_formatting() {
    let xml = zone(&format!(
        "<m:f><m:fPr>{}</m:fPr><m:num></m:num><m:den>{}</m:den></m:f>",
        ctrl(),
        r("2")
    ));
    let mut nodes = from_omml(&xml).unwrap();
    insert_text(&mut nodes, &p(&[(0, 0)], 0), "a").unwrap();
    assert_eq!(
        write(&nodes),
        zone(&format!(
            "<m:f><m:fPr>{}</m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>",
            ctrl(),
            r("a"),
            r("2")
        ))
    );
}

// source: UnicodeMath build-up (UTN #28); ctrlPr carries the run formatting of the structure
// (ECMA-376 Part 1, §22.1.2 ctrlPr).
#[test]
fn autobuild_takes_the_formatting_of_the_typed_run() {
    let xml = equation();
    let mut nodes = from_omml(&xml).unwrap();
    // Type "+x^2" after the matrix: a new run with the formatting of the zone (FI), then build.
    let end = me::zone_end(&nodes, true);
    let at = insert_text(&mut nodes, &end, "+x^2").unwrap();
    let at = autobuild(&mut nodes, &at).unwrap();
    let out = write(&nodes);
    let built = format!(
        "{}<m:sSup><m:sSupPr>{}</m:sSupPr><m:e>{}</m:e><m:sup>{}</m:sup></m:sSup>",
        r("+"),
        ctrl(),
        r("x"),
        r("2")
    );
    assert_eq!(
        out,
        zone(
            &[
                script_run(),
                r("="),
                nary(),
                r("+"),
                frac(),
                r("+"),
                delim(),
                func(),
                matrix(),
                built
            ]
            .concat()
        )
    );
    assert_eq!(at, me::zone_end(&from_omml(&out).unwrap(), true));
    // A function name built up is upright (m:sty p) with the run's formatting; √ takes ctrlPr.
    let mut nodes = from_omml(&zone(&r("a"))).unwrap();
    let at = insert_text(&mut nodes, &p(&[], 1), "+cos").unwrap();
    autobuild(&mut nodes, &at).unwrap();
    let end = me::zone_end(&nodes, true);
    let at = insert_text(&mut nodes, &end, "\\sqrt").unwrap();
    autobuild(&mut nodes, &at).unwrap();
    let out = write(&nodes);
    assert!(
        out.contains(&format!(
            r#"<m:r><m:rPr><m:sty m:val="p"/></m:rPr>{FI}<m:t>cos</m:t></m:r>"#
        )),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            r#"<m:rad><m:radPr><m:degHide m:val="1"/>{}</m:radPr><m:deg/>"#,
            ctrl()
        )),
        "{out}"
    );
}

#[test]
fn deleting_keeps_the_neighbours_properties() {
    let xml = equation();
    let mut nodes = from_omml(&xml).unwrap();
    // Backspace after the fraction selects it, then it goes: the runs around it join.
    let after_frac = p(&[], 4 + 1);
    let Some(Deleted::Select(sel)) = delete(&mut nodes, &after_frac, true) else {
        panic!("not selected");
    };
    delete_selected(&mut nodes, &sel).unwrap();
    let out = write(&nodes);
    assert_eq!(
        out,
        zone(
            &[
                script_run(),
                r("="),
                nary(),
                r("++"),
                delim(),
                func(),
                matrix()
            ]
            .concat()
        )
    );
    // A character of a delimiter part goes; the delimiter's properties stay.
    let mut nodes = from_omml(&xml).unwrap();
    let Some(Deleted::Char(_)) = delete(&mut nodes, &p(&[(6, 0)], 1), true) else {
        panic!("no char");
    };
    let out = write(&nodes);
    assert!(out.contains(&format!(
        r#"<m:d><m:dPr><m:begChr m:val="["/><m:sepChr m:val=";"/><m:endChr m:val="]"/><m:grow m:val="0"/>{}</m:dPr><m:e></m:e><m:e>{}</m:e></m:d>"#,
        ctrl(),
        r("b")
    )), "{out}");
}

// source: §22.1.2.72 naryPr (subHide), schema order chr, limLoc, grow, subHide, supHide, ctrlPr.
#[test]
fn a_changed_value_is_set_in_place_and_the_rest_kept() {
    // Empty the lower limit with Backspace: it is hidden (subHide), inserted before ctrlPr; limLoc
    // and grow stay as the file has them.
    let mut nodes = from_omml(&zone(&nary())).unwrap();
    for k in (1..=3).rev() {
        assert!(matches!(
            delete(&mut nodes, &p(&[(0, 0)], k), true),
            Some(Deleted::Char(_))
        ));
    }
    assert_eq!(slot_at(&nodes, &[Step { item: 0, slot: 0 }]), Some(&[][..]));
    let out = write(&nodes);
    assert_eq!(
        out,
        zone(&format!(
            r#"<m:nary><m:naryPr><m:chr m:val="∑"/><m:limLoc m:val="subSup"/><m:grow m:val="1"/><m:subHide m:val="1"/>{}</m:naryPr><m:sub></m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary>"#,
            ctrl(),
            r("n"),
            r("k")
        ))
    );
}

/// Walks right from the start of the zone to its end and back; the positions.
fn walk(root: &[Node]) -> Vec<MathPos> {
    let mut out = vec![me::zone_end(root, false)];
    while let Some(n) = right(root, out.last().unwrap()) {
        assert!(me::is_valid(root, &n));
        out.push(n);
        assert!(out.len() < 100);
    }
    let mut back = vec![out.last().unwrap().clone()];
    while let Some(n) = left(root, back.last().unwrap()) {
        back.push(n);
    }
    back.reverse();
    assert_eq!(back, out);
    out
}

// source: ECMA-376 Part 1, §22.1.2 groupChr, box, borderBox, phant, sPre.
#[test]
fn grouping_box_border_phantom_and_prescripts_edit_in_place() {
    let cases = [
        (
            format!(
                r#"<m:groupChr><m:groupChrPr><m:chr m:val="⏞"/><m:pos m:val="top"/><m:vertJc m:val="bot"/>{}</m:groupChrPr><m:e>{}</m:e></m:groupChr>"#,
                ctrl(),
                r("ab")
            ),
            1,
        ),
        (
            format!(
                r#"<m:box><m:boxPr><m:opEmu m:val="1"/>{}</m:boxPr><m:e>{}</m:e></m:box>"#,
                ctrl(),
                r("ab")
            ),
            1,
        ),
        (
            format!(
                r#"<m:borderBox><m:borderBoxPr><m:hideTop m:val="1"/><m:strikeBLTR m:val="1"/>{}</m:borderBoxPr><m:e>{}</m:e></m:borderBox>"#,
                ctrl(),
                r("ab")
            ),
            1,
        ),
        (
            format!(
                r#"<m:phant><m:phantPr><m:show m:val="0"/><m:zeroWid m:val="1"/>{}</m:phantPr><m:e>{}</m:e></m:phant>"#,
                ctrl(),
                r("ab")
            ),
            1,
        ),
        (
            format!(
                "<m:sPre><m:sPrePr>{}</m:sPrePr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:sPre>",
                ctrl(),
                r("6"),
                r("14"),
                r("C")
            ),
            3,
        ),
    ];
    for (inner, nslots) in cases {
        let xml = zone(&[r("y="), inner.clone(), r("+1")].concat());
        let nodes = from_omml(&xml).unwrap();
        // Lossless, byte for byte.
        assert_eq!(write(&nodes), xml, "{inner}");
        // One structure with its slots (zone + slots), every slot reached by the arrows.
        assert_eq!(nodes.len(), 3, "{inner}");
        assert_eq!(all_slots(&nodes).len(), 1 + nslots, "{inner}");
        let ps = walk(&nodes);
        for k in 0..nslots {
            assert!(ps.contains(&p(&[(1, k)], 0)), "{inner}: slot {k}");
        }
        // Typing into the first slot keeps the structure's properties and the run's.
        let mut edited = nodes.clone();
        insert_text(&mut edited, &p(&[(1, 0)], 0), "z").unwrap();
        let out = write(&edited);
        let pr_end = inner
            .find("</m:sub>")
            .or_else(|| inner.find("</m:e>"))
            .unwrap();
        assert!(
            out.contains(&inner[..inner[..pr_end].rfind("<m:t>").unwrap()]),
            "{out}"
        );
        assert!(out.contains("<m:t>z"), "{out}");
        assert_eq!(
            from_omml(&out).map(|n| all_slots(&n)),
            Some(all_slots(&nodes))
        );
    }
}

#[test]
fn display_properties_are_kept() {
    let xml = format!(
        r#"<m:oMathPara xmlns:m="{NS}"><m:oMathParaPr><m:jc m:val="left"/></m:oMathParaPr><m:oMath>{}</m:oMath></m:oMathPara>"#,
        r("q")
    );
    assert_eq!(
        bo_math::read::para_props(&xml).as_deref(),
        Some(r#"<m:oMathParaPr><m:jc m:val="left"/></m:oMathParaPr>"#)
    );
    assert_eq!(bo_math::read::para_props(&zone(&r("q"))), None);
}
