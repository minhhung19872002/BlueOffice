//! Text scaled with the content when the slide size changes (SLIDE-MASTER-001):
//! font sizes (ECMA-376 Part 1, §21.1.2.3.9 rPr@sz, ST_TextFontSize 100–400 000
//! hundredths of a point), character spacing (rPr@spc), point line / paragraph
//! spacing (§21.1.2.2.10 spcPts in lnSpc / spcBef / spcAft) and point bullet
//! sizes (§21.1.2.4.10 buSzPts) of runs, paragraphs, list styles
//! (§21.1.2.4.12 lstStyle), master text styles (§19.3.1.52 txStyles), table
//! cells (§21.1.3.16 tc txBody) and grouped shapes.

use bo_model::slide::{
    GraphicContent, ListStyle, ParaProps, RunProps, ShapeNode, Spacing, TextBody, TextRun,
};

/// §20.1.10.70 ST_TextFontSize bounds (1/100 pt).
const MIN_SZ: f64 = 100.0;
const MAX_SZ: f64 = 400_000.0;

fn same(k: f64) -> bool {
    (k - 1.0).abs() < 1e-9 || !k.is_finite() || k <= 0.0
}

/// A size in 1/100 pt times `k`, rounded to the nearest whole hundredth.
fn sz(v: i32, k: f64) -> i32 {
    (f64::from(v) * k).round().clamp(MIN_SZ, MAX_SZ) as i32
}

fn spacing(s: &mut Spacing, k: f64) {
    // Percent spacing follows the font on its own.
    if let Spacing::Points(v) = s {
        // §21.1.2.2.10 spcPts@val: ST_TextSpacingPoint 0–158 400.
        *v = (f64::from(*v) * k).round().clamp(0.0, 158_400.0) as i32;
    }
}

/// Scale a run's size and character spacing.
pub fn run_props(r: &mut RunProps, k: f64) {
    if let Some(v) = &mut r.sz {
        *v = sz(*v, k);
    }
    if let Some(v) = &mut r.spc {
        // ST_TextPoint −400 000 – 400 000.
        *v = (f64::from(*v) * k).round().clamp(-400_000.0, 400_000.0) as i32;
    }
}

/// Scale a paragraph's (or list level's) point spacing, bullet size and default run.
pub fn para_props(p: &mut ParaProps, k: f64) {
    for s in [&mut p.ln_spc, &mut p.spc_bef, &mut p.spc_aft]
        .into_iter()
        .flatten()
    {
        spacing(s, k);
    }
    if let Some(Some(s)) = &mut p.bu_sz {
        spacing(s, k);
    }
    if let Some(r) = &mut p.def_rpr {
        run_props(r, k);
    }
}

/// Scale every level of a list style.
pub fn list_style(ls: &mut ListStyle, k: f64) {
    if same(k) {
        return;
    }
    for p in ls.def.iter_mut().chain(ls.levels.iter_mut().flatten()) {
        para_props(p, k);
    }
}

/// Scale a text body: its list style, paragraphs and runs.
pub fn text_body(tb: &mut TextBody, k: f64) {
    if same(k) {
        return;
    }
    if let Some(ls) = &mut tb.list_style {
        list_style(ls, k);
    }
    for p in &mut tb.paragraphs {
        if let Some(pp) = &mut p.props {
            para_props(pp, k);
        }
        for r in &mut p.runs {
            match r {
                TextRun::Run { props, .. } | TextRun::Break { props } => run_props(props, k),
                TextRun::Field { props, ppr, .. } => {
                    run_props(props, k);
                    if let Some(pp) = ppr {
                        para_props(pp, k);
                    }
                }
                TextRun::Other(_) => {}
            }
        }
        if let Some(r) = &mut p.end_props {
            run_props(r, k);
        }
    }
}

/// Scale the text of every shape of a tree (groups and tables included).
pub fn nodes(nodes: &mut [ShapeNode], k: f64) {
    if same(k) {
        return;
    }
    for n in nodes {
        match n {
            ShapeNode::Shape(s) => {
                if let Some(tb) = &mut s.text {
                    text_body(tb, k);
                }
            }
            ShapeNode::Group(g) => self::nodes(&mut g.children, k),
            ShapeNode::Frame(f) => {
                if let GraphicContent::Table(t) = &mut f.content {
                    for c in t.rows.iter_mut().flat_map(|r| r.cells.iter_mut()) {
                        if let Some(tb) = &mut c.text {
                            text_body(tb, k);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::slide::Paragraph;

    #[test]
    fn sizes_and_point_spacing_scale() {
        let mut tb = TextBody {
            paragraphs: vec![Paragraph {
                props: Some(ParaProps {
                    spc_bef: Some(Spacing::Points(1200)),
                    ln_spc: Some(Spacing::Percent(90_000)),
                    bu_sz: Some(Some(Spacing::Points(2000))),
                    ..ParaProps::default()
                }),
                runs: vec![TextRun::Run {
                    props: RunProps {
                        sz: Some(4400),
                        spc: Some(-100),
                        ..RunProps::default()
                    },
                    text: "Title".into(),
                }],
                end_props: Some(RunProps {
                    sz: Some(1800),
                    ..RunProps::default()
                }),
            }],
            ..TextBody::default()
        };
        // 16:9 → 4:3 fit: 9 144 000 / 12 192 000 = 0.75.
        text_body(&mut tb, 0.75);
        let p = &tb.paragraphs[0];
        let pp = p.props.as_ref().unwrap();
        assert_eq!(pp.spc_bef, Some(Spacing::Points(900)));
        assert_eq!(pp.ln_spc, Some(Spacing::Percent(90_000)));
        assert_eq!(pp.bu_sz, Some(Some(Spacing::Points(1500))));
        let TextRun::Run { props, .. } = &p.runs[0] else {
            panic!()
        };
        // 44 pt → 33 pt, −1 pt spacing → −0.75 pt.
        assert_eq!((props.sz, props.spc), (Some(3300), Some(-75)));
        // 18 pt → 13.5 pt.
        assert_eq!(p.end_props.as_ref().unwrap().sz, Some(1350));
        // The smallest size stays within ST_TextFontSize.
        let mut r = RunProps {
            sz: Some(100),
            ..RunProps::default()
        };
        run_props(&mut r, 0.5);
        assert_eq!(r.sz, Some(100));
    }
}
