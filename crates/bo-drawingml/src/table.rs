//! DrawingML tables — ECMA-376 Part 1, §21.1.3.

use crate::dom::{walk, AttrBag, El, Kids};
use crate::props::{is_fill, parse_fill, parse_line, write_fill, write_line};
use crate::text::{parse_text_body, write_text_body};
use bo_model::slide::{CellProps, GridCol, Table, TableCell, TableRow, XmlElement};

const TBL: &[&str] = &["a:tblPr", "a:tblGrid", "a:tr", "a:extLst"];
const TR: &[&str] = &["a:tc", "a:extLst"];
const TC: &[&str] = &["a:txBody", "a:tcPr", "a:extLst"];
const TC_PR: &[&str] = &[
    "a:lnL",
    "a:lnR",
    "a:lnT",
    "a:lnB",
    "a:lnTlToBr",
    "a:lnBlToTr",
    "a:cell3D",
    "a:noFill",
    "a:solidFill",
    "a:gradFill",
    "a:blipFill",
    "a:pattFill",
    "a:grpFill",
    "a:headers",
    "a:extLst",
];

/// §21.1.3.17 tcPr.
fn parse_tc_pr(el: &XmlElement) -> CellProps {
    let mut bag = AttrBag::of(el);
    let mut p = CellProps {
        mar_l: bag.int("marL"),
        mar_r: bag.int("marR"),
        mar_t: bag.int("marT"),
        mar_b: bag.int("marB"),
        anchor: bag.take("anchor"),
        ..CellProps::default()
    };
    p.attrs = bag.rest();
    p.extra = walk(el, TC_PR, |c| {
        if is_fill(&c.name) {
            p.fill = parse_fill(c);
            return true;
        }
        let l = Some(parse_line(c));
        match c.name.as_str() {
            "a:lnL" => p.ln_l = l,
            "a:lnR" => p.ln_r = l,
            "a:lnT" => p.ln_t = l,
            "a:lnB" => p.ln_b = l,
            _ => return false,
        }
        true
    });
    p
}

fn write_tc_pr(p: &CellProps) -> XmlElement {
    let mut k = Kids::new(TC_PR);
    k.push_opt(p.ln_l.as_ref().map(|l| write_line(l, "a:lnL")));
    k.push_opt(p.ln_r.as_ref().map(|l| write_line(l, "a:lnR")));
    k.push_opt(p.ln_t.as_ref().map(|l| write_line(l, "a:lnT")));
    k.push_opt(p.ln_b.as_ref().map(|l| write_line(l, "a:lnB")));
    k.push_opt(p.fill.as_ref().map(write_fill));
    k.extras(&p.extra);
    k.build(
        El::new("a:tcPr")
            .opt("marL", p.mar_l)
            .opt("marR", p.mar_r)
            .opt("marT", p.mar_t)
            .opt("marB", p.mar_b)
            .opt("anchor", p.anchor.as_ref())
            .rest(&p.attrs),
    )
}

/// §21.1.3.13 tbl.
pub fn parse_table(el: &XmlElement) -> Table {
    let mut t = Table::default();
    t.extra = walk(el, TBL, |c| {
        match c.name.as_str() {
            "a:tblPr" => t.props = Some(c.clone()),
            "a:tblGrid" => {
                t.grid = c
                    .elements()
                    .map(|g| GridCol {
                        w: crate::dom::attr_int(g, "w").unwrap_or(0),
                        extra: g.elements().cloned().collect(),
                    })
                    .collect();
            }
            "a:tr" => {
                let mut row = TableRow {
                    h: crate::dom::attr_int(c, "h").unwrap_or(0),
                    ..TableRow::default()
                };
                row.extra = walk(c, TR, |tc| {
                    if tc.name != "a:tc" {
                        return false;
                    }
                    let mut bag = AttrBag::of(tc);
                    let mut cell = TableCell {
                        grid_span: bag.int("gridSpan"),
                        row_span: bag.int("rowSpan"),
                        h_merge: bag.bool("hMerge"),
                        v_merge: bag.bool("vMerge"),
                        ..TableCell::default()
                    };
                    cell.attrs = bag.rest();
                    cell.extra = walk(tc, TC, |x| {
                        match x.name.as_str() {
                            "a:txBody" => cell.text = Some(parse_text_body(x)),
                            "a:tcPr" => cell.props = Some(parse_tc_pr(x)),
                            _ => return false,
                        }
                        true
                    });
                    row.cells.push(cell);
                    true
                });
                t.rows.push(row);
            }
            _ => return false,
        }
        true
    });
    t
}

/// Write a tbl element.
pub fn write_table(t: &Table) -> XmlElement {
    let mut k = Kids::new(TBL);
    k.push_opt(t.props.clone());
    let mut g = El::new("a:tblGrid");
    for c in &t.grid {
        let mut ge = El::new("a:gridCol").a("w", c.w);
        for x in &c.extra {
            ge = ge.child(x.clone());
        }
        g = g.child(ge.done());
    }
    k.push(g.done());
    for r in &t.rows {
        let mut rk = Kids::new(TR);
        for c in &r.cells {
            let mut ck = Kids::new(TC);
            ck.push_opt(c.text.as_ref().map(|b| write_text_body(b, "a:txBody")));
            ck.push_opt(c.props.as_ref().map(write_tc_pr));
            ck.extras(&c.extra);
            rk.push(
                ck.build(
                    El::new("a:tc")
                        .opt("gridSpan", c.grid_span)
                        .opt("rowSpan", c.row_span)
                        .optb("hMerge", c.h_merge)
                        .optb("vMerge", c.v_merge)
                        .rest(&c.attrs),
                ),
            );
        }
        rk.extras(&r.extra);
        k.push(rk.build(El::new("a:tr").a("h", r.h)));
    }
    k.extras(&t.extra);
    k.build(El::new("a:tbl"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{canonical, parse};

    #[test]
    fn table_roundtrip() {
        let xml = r#"<a:tbl><a:tblPr firstRow="1" bandRow="1"><a:tableStyleId>{X}</a:tableStyleId></a:tblPr><a:tblGrid><a:gridCol w="100"><a:extLst><a:ext uri="u"/></a:extLst></a:gridCol><a:gridCol w="200"/></a:tblGrid><a:tr h="50"><a:tc gridSpan="2"><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>A</a:t></a:r></a:p></a:txBody><a:tcPr marL="0" anchor="ctr"><a:lnL w="12700"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:lnL><a:solidFill><a:srgbClr val="EEEEEE"/></a:solidFill></a:tcPr></a:tc><a:tc hMerge="1"><a:txBody><a:bodyPr/><a:lstStyle/><a:p/></a:txBody><a:tcPr/></a:tc><a:extLst><a:ext uri="r"/></a:extLst></a:tr></a:tbl>"#;
        let el = parse(xml.as_bytes(), "/t").unwrap();
        let t = parse_table(&el);
        assert_eq!(t.grid.len(), 2);
        assert_eq!(t.rows[0].cells.len(), 2);
        assert_eq!(t.rows[0].cells[0].grid_span, Some(2));
        assert_eq!(t.rows[0].extra.len(), 1);
        assert_eq!(canonical(&write_table(&t)), canonical(&el));
    }
}
