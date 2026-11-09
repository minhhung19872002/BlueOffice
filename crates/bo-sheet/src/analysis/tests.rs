//! Analysis tool tests. Every expected number is worked out by hand (closed forms noted in the
//! comments), read from standard statistical tables, or published for a textbook dataset.

use super::*;

/// A grid whose columns are `cols` (shorter columns padded with blanks).
fn grid(cols: &[&[f64]]) -> Grid {
    let h = cols.iter().map(|c| c.len()).max().unwrap_or(0);
    (0..h)
        .map(|r| {
            cols.iter()
                .map(|c| c.get(r).map_or(Item::Empty, |v| Item::Num(*v)))
                .collect()
        })
        .collect()
}

/// A grid from rows of numbers.
fn rows(r: &[&[f64]]) -> Grid {
    r.iter()
        .map(|row| row.iter().map(|v| Item::Num(*v)).collect())
        .collect()
}

fn txt(s: &str) -> Out {
    Out::Text(s.to_owned())
}

fn text(s: &str) -> Item {
    Item::Text(s.to_owned())
}

fn run_en(tool: &Tool, a: &Grid, b: Option<&Grid>) -> Table {
    run(tool, a, b, false).expect("tool runs")
}

/// The row whose first cell is `label`.
fn row<'a>(t: &'a Table, label: &str) -> &'a [Out] {
    t.rows
        .iter()
        .find(|r| matches!(r.first(), Some(Out::Text(s)) if s == label))
        .unwrap_or_else(|| panic!("no row {label}: {t:#?}"))
}

fn n(o: &Out) -> f64 {
    match o {
        Out::Num(x) | Out::Percent(x) => *x,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b} (±{tol})");
}

/// hand: CDF of Student's t with 4 degrees of freedom in closed form,
/// F(t) = 1/2 + (3/8)·u·(1 − w/12) with u = t/√(1 + t²/4), w = t²/(1 + t²/4).
fn t4_cdf(t: f64) -> f64 {
    let s = 1.0 + t * t / 4.0;
    0.5 + 0.375 * (t / s.sqrt()) * (1.0 - t * t / s / 12.0)
}

/// hand: right tail of F(2, ν) in closed form, (1 + 2f/ν)^(−ν/2).
fn f2_rt(f: f64, v: f64) -> f64 {
    (1.0 + 2.0 * f / v).powf(-v / 2.0)
}

/// hand: the F(2, ν) critical value at α from the closed form above, (α^(−2/ν) − 1)·ν/2.
fn f2_crit(alpha: f64, v: f64) -> f64 {
    (alpha.powf(-2.0 / v) - 1.0) * v / 2.0
}

const TOL: f64 = 1e-9;

#[test]
fn descriptive_statistics() {
    // hand: 2 4 4 4 5 5 7 9 — mean 5, deviations −3 −1 −1 −1 0 0 2 4, Σd² = 32,
    // sample variance 32/7, Σd³ = 42, Σd⁴ = 356.
    let data = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
    let tool = Tool::Descriptive {
        by_rows: false,
        labels: false,
        confidence: Some(0.95),
        kth_largest: Some(2),
        kth_smallest: Some(3),
    };
    let t = run_en(&tool, &grid(&[&data]), None);
    assert_eq!(t.rows[0], vec![txt("Column 1")]);
    let var: f64 = 32.0 / 7.0;
    let sd = var.sqrt();
    let cases: &[(&str, f64)] = &[
        ("Mean", 5.0),
        ("Standard Error", sd / 8f64.sqrt()),
        ("Median", 4.5),
        ("Mode", 4.0),
        ("Standard Deviation", sd),
        ("Sample Variance", var),
        // KURT = n(n+1)/((n−1)(n−2)(n−3))·Σd⁴/s⁴ − 3(n−1)²/((n−2)(n−3))
        //      = 72/210 · 356·49/1024 − 147/30 = 5.840625 − 4.9.
        ("Kurtosis", 0.940_625),
        // SKEW = n/((n−1)(n−2))·Σd³/s³ = 8/42 · 42/s³ = 8/s³.
        ("Skewness", 8.0 / var.powf(1.5)),
        ("Range", 7.0),
        ("Minimum", 2.0),
        ("Maximum", 9.0),
        ("Sum", 40.0),
        ("Count", 8.0),
        ("Largest(2)", 7.0),
        ("Smallest(3)", 4.0),
    ];
    for (label, want) in cases {
        close(n(&row(&t, label)[1]), *want, TOL);
    }
    // t table: t(0.975, 7) = 2.364624; half-width = t·s/√n.
    close(
        n(&row(&t, "Confidence Level(95.0%)")[1]),
        2.364_624 * sd / 8f64.sqrt(),
        1e-6,
    );
}

#[test]
fn descriptive_edge_cases() {
    // hand: 1 2 3 has no repeated value (mode #N/A); n < 4 has no kurtosis.
    let tool = Tool::Descriptive {
        by_rows: true,
        labels: true,
        confidence: None,
        kth_largest: Some(5),
        kth_smallest: None,
    };
    let g = vec![vec![
        text("A"),
        Item::Num(1.0),
        Item::Num(2.0),
        Item::Num(3.0),
    ]];
    let t = run_en(&tool, &g, None);
    assert_eq!(t.rows[0][0], txt("A"));
    assert_eq!(row(&t, "Mode")[1], Out::Error(OutError::NA));
    assert_eq!(row(&t, "Kurtosis")[1], Out::Error(OutError::Div0));
    close(n(&row(&t, "Skewness")[1]), 0.0, TOL);
    assert_eq!(row(&t, "Largest(5)")[1], Out::Error(OutError::Num));
    // Text among the numbers is refused.
    let bad = vec![vec![Item::Num(1.0), text("x")]];
    assert_eq!(
        run(&tool, &bad, None, false).unwrap_err(),
        AnalysisError::NonNumeric
    );
    // Vietnamese labels.
    let vi = run(&tool, &g, None, true).expect("runs");
    assert!(vi
        .rows
        .iter()
        .any(|r| r.first() == Some(&txt("Trung bình"))));
}

#[test]
fn correlation_and_covariance() {
    // hand: x = 1..5, y = 2 4 5 4 5; Sxy = 6, Sxx = 10, Syy = 6 → r = 6/√60;
    // population covariance 6/5, variances 10/5 and 6/5.
    let g = grid(&[&[1.0, 2.0, 3.0, 4.0, 5.0], &[2.0, 4.0, 5.0, 4.0, 5.0]]);
    let t = run_en(
        &Tool::Correlation {
            by_rows: false,
            labels: false,
        },
        &g,
        None,
    );
    assert_eq!(
        t.rows[0],
        vec![Out::Empty, txt("Column 1"), txt("Column 2")]
    );
    close(n(&t.rows[1][1]), 1.0, TOL);
    assert_eq!(t.rows[1].len(), 2, "upper triangle left blank");
    close(n(&t.rows[2][1]), 6.0 / 60f64.sqrt(), TOL);
    close(n(&t.rows[2][2]), 1.0, TOL);
    let c = run_en(
        &Tool::Covariance {
            by_rows: false,
            labels: false,
        },
        &g,
        None,
    );
    close(n(&c.rows[1][1]), 2.0, TOL);
    close(n(&c.rows[2][1]), 1.2, TOL);
    close(n(&c.rows[2][2]), 1.2, TOL);
    // Columns of different lengths do not pair up.
    let uneven = grid(&[&[1.0, 2.0, 3.0], &[1.0, 2.0]]);
    assert_eq!(
        run(
            &Tool::Covariance {
                by_rows: false,
                labels: false
            },
            &uneven,
            None,
            false
        )
        .unwrap_err(),
        AnalysisError::Shape
    );
}

#[test]
fn anova_single_factor() {
    // hand: groups 1 2 3 | 4 5 6 | 7 8 9 — means 2 5 8, grand mean 5;
    // SSB = 3·(9 + 0 + 9) = 54 (df 2), SSW = 3·2 = 6 (df 6), F = 27 / 1 = 27.
    let g = grid(&[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0], &[7.0, 8.0, 9.0]]);
    let t = run_en(
        &Tool::AnovaSingle {
            by_rows: false,
            labels: false,
            alpha: 0.05,
        },
        &g,
        None,
    );
    assert_eq!(t.rows[0], vec![txt("One-way ANOVA")]);
    let s = row(&t, "Column 2");
    assert_eq!(
        s.iter().skip(1).map(n).collect::<Vec<_>>(),
        vec![3.0, 15.0, 5.0, 1.0]
    );
    let b = row(&t, "Between Groups");
    let want = [54.0, 2.0, 27.0, 27.0];
    for (i, w) in want.iter().enumerate() {
        close(n(&b[i + 1]), *w, TOL);
    }
    // F(2, 6) has a closed-form tail: (1 + 2·27/6)^−3 = 10^−3.
    close(n(&b[5]), 0.001, 1e-9);
    close(n(&b[6]), f2_crit(0.05, 6.0), 1e-7);
    close(n(&b[6]), 5.143_253, 1e-6); // F table: F(0.05; 2, 6) = 5.1433
    let w = row(&t, "Within Groups");
    assert_eq!(w.len(), 4);
    close(n(&w[1]), 6.0, TOL);
    close(n(&w[3]), 1.0, TOL);
    let tot = row(&t, "Total");
    close(n(&tot[1]), 60.0, TOL);
    close(n(&tot[2]), 8.0, TOL);
}

#[test]
fn anova_two_factor_without_replication() {
    // hand: rows 4 5 6 | 6 8 7 | 2 2 5 — row means 5 7 3, column means 4 5 6, grand mean 5;
    // SS rows = 3·(0 + 4 + 4) = 24, SS columns = 3·(1 + 0 + 1) = 6, SS total = 34,
    // SS error = 4 (df 4) → MS error 1, F rows = 12, F columns = 3.
    let g = rows(&[&[4.0, 5.0, 6.0], &[6.0, 8.0, 7.0], &[2.0, 2.0, 5.0]]);
    let t = run_en(
        &Tool::AnovaTwoFactor {
            labels: false,
            alpha: 0.05,
        },
        &g,
        None,
    );
    let r = row(&t, "Rows");
    for (i, w) in [24.0, 2.0, 12.0, 12.0].iter().enumerate() {
        close(n(&r[i + 1]), *w, TOL);
    }
    close(n(&r[5]), f2_rt(12.0, 4.0), 1e-9); // (1 + 6)^−2 = 1/49
    close(n(&r[6]), 6.944_272, 1e-6); // F table: F(0.05; 2, 4) = 6.9443
    let c = row(&t, "Columns");
    close(n(&c[1]), 6.0, TOL);
    close(n(&c[4]), 3.0, TOL);
    close(n(&c[5]), 0.16, 1e-9); // (1 + 1.5)^−2
    let e = row(&t, "Error");
    close(n(&e[1]), 4.0, 1e-9);
    close(n(&e[2]), 4.0, TOL);
    let tot = row(&t, "Total");
    close(n(&tot[1]), 34.0, TOL);
    close(n(&tot[2]), 8.0, TOL);
    // Summary of the second row: count 3, sum 21, average 7, variance 1.
    let s = row(&t, "Row 2");
    assert_eq!(
        s.iter().skip(1).map(n).collect::<Vec<_>>(),
        vec![3.0, 21.0, 7.0, 1.0]
    );
}

#[test]
fn anova_two_factor_with_replication() {
    // hand: two samples × two columns, two observations each:
    //   S1: C1 = 1 3, C2 = 5 7;  S2: C1 = 2 4, C2 = 10 12.
    // Cell means 2 6 3 11, grand mean 5.5; sample means 4 7, column means 2.5 8.5.
    // SS sample = 4·(2.25 + 2.25) = 18, SS columns = 4·(9 + 9) = 72,
    // SS cells = 2·(12.25 + 0.25 + 6.25 + 30.25) = 98 → interaction 8,
    // SS within = 4·2 = 8 (df 4, MS 2), total 106 (df 7).
    let g: Grid = vec![
        vec![Item::Empty, text("C1"), text("C2")],
        vec![text("S1"), Item::Num(1.0), Item::Num(5.0)],
        vec![Item::Empty, Item::Num(3.0), Item::Num(7.0)],
        vec![text("S2"), Item::Num(2.0), Item::Num(10.0)],
        vec![Item::Empty, Item::Num(4.0), Item::Num(12.0)],
    ];
    let t = run_en(
        &Tool::AnovaTwoFactorRep {
            rows_per_sample: 2,
            alpha: 0.05,
        },
        &g,
        None,
    );
    let cases: &[(&str, f64, f64, f64)] = &[
        ("Sample", 18.0, 1.0, 9.0),
        ("Columns", 72.0, 1.0, 36.0),
        ("Interaction", 8.0, 1.0, 4.0),
    ];
    for (label, ss, df, f) in cases {
        let r = row(&t, label);
        close(n(&r[1]), *ss, 1e-9);
        close(n(&r[2]), *df, TOL);
        close(n(&r[4]), *f, 1e-9);
        // F(1, 4) = T(4)²: P(F > f) = 2·(1 − F_t4(√f)).
        close(n(&r[5]), 2.0 * (1.0 - t4_cdf(f.sqrt())), 1e-9);
        close(n(&r[6]), 7.708_647, 1e-6); // F table: F(0.05; 1, 4) = 7.7086
    }
    let w = row(&t, "Within");
    close(n(&w[1]), 8.0, TOL);
    close(n(&w[3]), 2.0, TOL);
    let tot = t
        .rows
        .iter()
        .rev()
        .find(|r| r.first() == Some(&txt("Total")))
        .expect("total");
    close(n(&tot[1]), 106.0, TOL);
    close(n(&tot[2]), 7.0, TOL);
    // Summary block of S2: averages 3 and 11, total 7.
    let s2 = t
        .rows
        .iter()
        .position(|r| r.first() == Some(&txt("S2")))
        .expect("S2");
    let avg = &t.rows[s2 + 3];
    assert_eq!(avg[0], txt("Average"));
    assert_eq!(
        avg[1..].iter().map(n).collect::<Vec<_>>(),
        vec![3.0, 11.0, 7.0]
    );
    // Rows not a multiple of the block size.
    assert_eq!(
        run(
            &Tool::AnovaTwoFactorRep {
                rows_per_sample: 3,
                alpha: 0.05
            },
            &g,
            None,
            false
        )
        .unwrap_err(),
        AnalysisError::Shape
    );
}

#[test]
fn f_test_variances() {
    // hand: var(1..5) = 2.5, var(2 4 6 8 10) = 10 → F = 0.25 with df 4, 4 (lower tail).
    // F(4, 4) CDF: I_x(2, 2) = 3x² − 2x³ with x = 4f/(4f + 4) = 0.2 → 0.104.
    let a = grid(&[&[1.0, 2.0, 3.0, 4.0, 5.0]]);
    let b = grid(&[&[2.0, 4.0, 6.0, 8.0, 10.0]]);
    let t = run_en(
        &Tool::FTest {
            labels: false,
            alpha: 0.05,
        },
        &a,
        Some(&b),
    );
    assert_eq!(t.rows[0], vec![txt("F-test: are two variances equal?")]);
    assert_eq!(
        row(&t, "Variance")[1..].iter().map(n).collect::<Vec<_>>(),
        vec![2.5, 10.0]
    );
    close(n(&row(&t, "F")[1]), 0.25, TOL);
    close(n(&row(&t, "p-value (one-sided)")[1]), 0.104, 1e-9);
    // F table: F(0.05; 4, 4) = 6.3882 upper, so the lower critical value is 1/6.3882.
    close(n(&row(&t, "Critical F (one-sided)")[1]), 1.0 / 6.388_233, 1e-6);
    // Swapped, F = 4 > 1 uses the upper tail: P = 1 − I_0.8(2, 2) = 1 − (1.92 − 1.024).
    let t = run_en(
        &Tool::FTest {
            labels: false,
            alpha: 0.05,
        },
        &b,
        Some(&a),
    );
    close(n(&row(&t, "p-value (one-sided)")[1]), 0.104, 1e-9);
    close(n(&row(&t, "Critical F (one-sided)")[1]), 6.388_233, 1e-6);
}

#[test]
fn t_test_paired() {
    // hand: a = 5 7 9 6 8, b = 4 6 7 6 5 → d = 1 1 2 0 3, mean 1.4, var 5.2/4 = 1.3,
    // t = 1.4/√(1.3/5), df 4; r = 5/√(10·5.2).
    let a = grid(&[&[5.0, 7.0, 9.0, 6.0, 8.0]]);
    let b = grid(&[&[4.0, 6.0, 7.0, 6.0, 5.0]]);
    let t = run_en(
        &Tool::TTestPaired {
            labels: false,
            alpha: 0.05,
            hypothesized: 0.0,
        },
        &a,
        Some(&b),
    );
    let ts = 1.4 / (1.3f64 / 5.0).sqrt();
    close(
        n(&row(&t, "Pearson Correlation")[1]),
        5.0 / 52f64.sqrt(),
        TOL,
    );
    close(n(&row(&t, "df")[1]), 4.0, TOL);
    close(n(&row(&t, "t statistic")[1]), ts, 1e-12);
    close(n(&row(&t, "p-value (one-sided)")[1]), 1.0 - t4_cdf(ts), 1e-9);
    close(
        n(&row(&t, "p-value (two-sided)")[1]),
        2.0 * (1.0 - t4_cdf(ts)),
        1e-9,
    );
    // t table: t(0.95, 4) = 2.131847, t(0.975, 4) = 2.776445.
    close(n(&row(&t, "Critical t (one-sided)")[1]), 2.131_847, 1e-6);
    close(n(&row(&t, "Critical t (two-sided)")[1]), 2.776_445, 1e-6);
    // Unequal counts cannot be paired.
    let short = grid(&[&[1.0, 2.0, 3.0]]);
    assert_eq!(
        run(
            &Tool::TTestPaired {
                labels: false,
                alpha: 0.05,
                hypothesized: 0.0
            },
            &a,
            Some(&short),
            false
        )
        .unwrap_err(),
        AnalysisError::Shape
    );
}

#[test]
fn t_test_two_sample() {
    // hand: a = 1 2 3, b = 3 4 5 (with labels) — means 2 and 4, variances 1 and 1,
    // pooled variance 1, t = −2/√(2/3) = −√6, df 4.
    let a: Grid = vec![
        vec![text("A")],
        vec![Item::Num(1.0)],
        vec![Item::Num(2.0)],
        vec![Item::Num(3.0)],
    ];
    let b: Grid = vec![
        vec![text("B")],
        vec![Item::Num(3.0)],
        vec![Item::Num(4.0)],
        vec![Item::Num(5.0)],
    ];
    let t = run_en(
        &Tool::TTestEqual {
            labels: true,
            alpha: 0.05,
            hypothesized: 0.0,
        },
        &a,
        Some(&b),
    );
    assert_eq!(t.rows[2], vec![Out::Empty, txt("A"), txt("B")]);
    close(n(&row(&t, "Pooled Variance")[1]), 1.0, TOL);
    close(n(&row(&t, "t statistic")[1]), -(6f64.sqrt()), 1e-12);
    close(
        n(&row(&t, "p-value (one-sided)")[1]),
        1.0 - t4_cdf(6f64.sqrt()),
        1e-9,
    );
    // Hypothesized difference −2 makes t = 0, p = 0.5.
    let t0 = run_en(
        &Tool::TTestEqual {
            labels: true,
            alpha: 0.05,
            hypothesized: -2.0,
        },
        &a,
        Some(&b),
    );
    close(n(&row(&t0, "t statistic")[1]), 0.0, 1e-12);
    close(n(&row(&t0, "p-value (one-sided)")[1]), 0.5, 1e-12);

    // hand: Welch with a = 1 2 3 (var 1, n 3), b = 2 4 6 8 10 (var 10, n 5):
    // qa = 1/3, qb = 2, df = (7/3)²/((1/9)/2 + 4/4) = 5.158 → 5, t = −4/√(7/3).
    let c = grid(&[&[1.0, 2.0, 3.0]]);
    let d = grid(&[&[2.0, 4.0, 6.0, 8.0, 10.0]]);
    let u = run_en(
        &Tool::TTestUnequal {
            labels: false,
            alpha: 0.05,
            hypothesized: 0.0,
        },
        &c,
        Some(&d),
    );
    close(n(&row(&u, "df")[1]), 5.0, TOL);
    close(n(&row(&u, "t statistic")[1]), -4.0 / (7f64 / 3.0).sqrt(), 1e-12);
    // t table: t(0.95, 5) = 2.015048, t(0.975, 5) = 2.570582.
    close(n(&row(&u, "Critical t (one-sided)")[1]), 2.015_048, 1e-6);
    close(n(&row(&u, "Critical t (two-sided)")[1]), 2.570_582, 1e-6);
}

#[test]
fn z_test_means() {
    // hand: a = 1..5 (mean 3, σ² 2), b = 4 5 6 (mean 5, σ² 3): z = −2/√(2/5 + 3/3) = −2/√1.4.
    let a = grid(&[&[1.0, 2.0, 3.0, 4.0, 5.0]]);
    let b = grid(&[&[4.0, 5.0, 6.0]]);
    let t = run_en(
        &Tool::ZTest {
            labels: false,
            alpha: 0.05,
            hypothesized: 0.0,
            var_a: 2.0,
            var_b: 3.0,
        },
        &a,
        Some(&b),
    );
    let z = -2.0 / 1.4f64.sqrt();
    close(n(&row(&t, "z")[1]), z, 1e-12);
    // Normal table: Φ(−1.69) = 0.0455.
    close(n(&row(&t, "p-value (one-sided)")[1]), 0.0455, 2e-4);
    // Normal table: z(0.95) = 1.644854, z(0.975) = 1.959964.
    close(n(&row(&t, "Critical z (one-sided)")[1]), 1.644_854, 1e-6);
    close(n(&row(&t, "Critical z (two-sided)")[1]), 1.959_964, 1e-6);
    let bad = Tool::ZTest {
        labels: false,
        alpha: 0.05,
        hypothesized: 0.0,
        var_a: 0.0,
        var_b: 3.0,
    };
    assert_eq!(
        run(&bad, &a, Some(&b), false).unwrap_err(),
        AnalysisError::Param
    );
}

#[test]
fn exponential_smoothing() {
    // hand: y = 10 20 30 40, damping 0.5 (α = 0.5): F = #N/A, 10, 15, 22.5;
    // standard error at row 4 = √(((20−10)² + (30−15)² + (40−22.5)²)/3).
    let g = grid(&[&[10.0, 20.0, 30.0, 40.0]]);
    let t = run_en(
        &Tool::ExpSmoothing {
            labels: false,
            damping: 0.5,
            std_errors: true,
        },
        &g,
        None,
    );
    assert_eq!(t.rows[0][0], Out::Error(OutError::NA));
    close(n(&t.rows[1][0]), 10.0, TOL);
    close(n(&t.rows[2][0]), 15.0, TOL);
    close(n(&t.rows[3][0]), 22.5, TOL);
    assert_eq!(t.rows[2][1], Out::Error(OutError::NA));
    close(
        n(&t.rows[3][1]),
        ((100.0 + 225.0 + 306.25) / 3.0f64).sqrt(),
        1e-12,
    );
    assert_eq!(
        run(
            &Tool::ExpSmoothing {
                labels: false,
                damping: 1.5,
                std_errors: false
            },
            &g,
            None,
            false
        )
        .unwrap_err(),
        AnalysisError::Param
    );
}

#[test]
fn moving_average() {
    // hand: y = 1..5, interval 3: #N/A, #N/A, 2, 3, 4; standard error at row 5 =
    // √(((3−2)² + (4−3)² + (5−4)²)/3) = 1.
    let g: Grid = std::iter::once(vec![text("Sales")])
        .chain((1..=5).map(|i| vec![Item::Num(f64::from(i))]))
        .collect();
    let t = run_en(
        &Tool::MovingAverage {
            labels: true,
            interval: 3,
            std_errors: true,
        },
        &g,
        None,
    );
    assert_eq!(t.rows[0], vec![txt("Forecast"), txt("Standard Errors")]);
    let f: Vec<&Out> = t.rows[1..].iter().map(|r| &r[0]).collect();
    assert_eq!(f[0], &Out::Error(OutError::NA));
    assert_eq!(f[1], &Out::Error(OutError::NA));
    assert_eq!(
        f[2..].iter().map(|o| n(o)).collect::<Vec<_>>(),
        vec![2.0, 3.0, 4.0]
    );
    assert_eq!(t.rows[4][1], Out::Error(OutError::NA));
    close(n(&t.rows[5][1]), 1.0, 1e-12);
}

#[test]
fn histogram_bins() {
    // hand: 1 2 2 3 3 3 4 5 into bins ≤2, (2, 4], More → 3, 4, 1; cumulative 3/8, 7/8, 1.
    let data = grid(&[&[1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 5.0]]);
    let bins = grid(&[&[4.0, 2.0]]);
    let t = run_en(
        &Tool::Histogram {
            labels: false,
            pareto: true,
            cumulative: true,
        },
        &data,
        Some(&bins),
    );
    assert_eq!(
        t.rows[0][..3],
        [txt("Bin"), txt("Frequency"), txt("Cumulative %")]
    );
    let col = |c: usize| t.rows[1..].iter().map(|r| r[c].clone()).collect::<Vec<_>>();
    assert_eq!(col(0), vec![Out::Num(2.0), Out::Num(4.0), txt("More")]);
    assert_eq!(col(1), vec![Out::Num(3.0), Out::Num(4.0), Out::Num(1.0)]);
    assert_eq!(
        col(2),
        vec![Out::Percent(0.375), Out::Percent(0.875), Out::Percent(1.0)]
    );
    // Pareto: sorted by frequency (4, 3, 1).
    assert_eq!(col(3), vec![Out::Num(4.0), Out::Num(2.0), txt("More")]);
    assert_eq!(col(4), vec![Out::Num(4.0), Out::Num(3.0), Out::Num(1.0)]);
    assert_eq!(
        col(5),
        vec![Out::Percent(0.5), Out::Percent(0.875), Out::Percent(1.0)]
    );
    // hand: automatic bins for 1..4 — ⌈√4⌉ = 2 bins of width 1.5: edges 2.5 and 4 → 2, 2, 0.
    let auto = run_en(
        &Tool::Histogram {
            labels: false,
            pareto: false,
            cumulative: false,
        },
        &grid(&[&[1.0, 2.0, 3.0, 4.0]]),
        None,
    );
    let got: Vec<Vec<Out>> = auto.rows[1..].to_vec();
    assert_eq!(
        got,
        vec![
            vec![Out::Num(2.5), Out::Num(2.0)],
            vec![Out::Num(4.0), Out::Num(2.0)],
            vec![txt("More"), Out::Num(0.0)],
        ]
    );
}

#[test]
fn rank_and_percentile() {
    // hand: 3 1 4 1 5 sorted descending: 5 (point 5), 4 (3), 3 (1), 1 (2), 1 (4);
    // ranks 1 2 3 4 4; percent = values below / (n − 1): 1, 0.75, 0.5, 0, 0.
    let t = run_en(
        &Tool::RankPercentile {
            by_rows: false,
            labels: false,
        },
        &grid(&[&[3.0, 1.0, 4.0, 1.0, 5.0]]),
        None,
    );
    assert_eq!(
        t.rows[0],
        vec![txt("Point"), txt("Column 1"), txt("Rank"), txt("Percent")]
    );
    let want = [
        (5.0, 5.0, 1.0, 1.0),
        (3.0, 4.0, 2.0, 0.75),
        (1.0, 3.0, 3.0, 0.5),
        (2.0, 1.0, 4.0, 0.0),
        (4.0, 1.0, 4.0, 0.0),
    ];
    for (r, (p, v, k, pct)) in t.rows[1..].iter().zip(want) {
        assert_eq!(
            r,
            &vec![Out::Num(p), Out::Num(v), Out::Num(k), Out::Percent(pct)]
        );
    }
}

#[test]
fn random_numbers() {
    let empty: Grid = Vec::new();
    let gen = |dist: RandomDist, count: usize, seed: u64| {
        run_en(
            &Tool::Random {
                columns: 2,
                count,
                seed: Some(seed),
                dist,
            },
            &empty,
            None,
        )
    };
    let values = |t: &Table| t.rows.iter().flatten().map(n).collect::<Vec<_>>();
    // Same seed, same numbers; another seed, other numbers.
    let u = RandomDist::Uniform { lo: 10.0, hi: 20.0 };
    let a = gen(u.clone(), 50, 7);
    assert_eq!(a, gen(u.clone(), 50, 7));
    assert_ne!(a, gen(u, 50, 8));
    assert_eq!(a.rows.len(), 50);
    assert_eq!(a.width(), 2);
    assert!(values(&a).iter().all(|x| (10.0..20.0).contains(x)));
    // Statistical checks (10 000 draws; bounds are > 4 standard errors wide).
    let normal = values(&gen(RandomDist::Normal { mean: 3.0, sd: 2.0 }, 5000, 1));
    close(mean(&normal), 3.0, 0.1);
    close(var_s(&normal).sqrt(), 2.0, 0.1);
    let bern = values(&gen(RandomDist::Bernoulli { p: 0.3 }, 5000, 2));
    assert!(bern.iter().all(|x| *x == 0.0 || *x == 1.0));
    close(mean(&bern), 0.3, 0.02);
    let bin = values(&gen(RandomDist::Binomial { p: 0.5, trials: 10 }, 5000, 3));
    assert!(bin
        .iter()
        .all(|x| x.fract() == 0.0 && (0.0..=10.0).contains(x)));
    close(mean(&bin), 5.0, 0.08);
    let poi = values(&gen(RandomDist::Poisson { lambda: 4.0 }, 5000, 4));
    assert!(poi.iter().all(|x| x.fract() == 0.0 && *x >= 0.0));
    close(mean(&poi), 4.0, 0.1);
    // Discrete: values 1, 2, 3 with probabilities 0.2, 0.5, 0.3.
    let table = rows(&[&[1.0, 0.2], &[2.0, 0.5], &[3.0, 0.3]]);
    let disc = run_en(
        &Tool::Random {
            columns: 1,
            count: 10_000,
            seed: Some(5),
            dist: RandomDist::Discrete,
        },
        &table,
        None,
    );
    let d = values(&disc);
    assert!(d.iter().all(|x| [1.0, 2.0, 3.0].contains(x)));
    close(
        d.iter().filter(|x| **x == 2.0).count() as f64 / 10_000.0,
        0.5,
        0.025,
    );
    // Probabilities must add up to 1.
    let bad = rows(&[&[1.0, 0.2], &[2.0, 0.5]]);
    let tool = Tool::Random {
        columns: 1,
        count: 1,
        seed: None,
        dist: RandomDist::Discrete,
    };
    assert_eq!(
        run(&tool, &bad, None, false).unwrap_err(),
        AnalysisError::Param
    );
    let huge = Tool::Random {
        columns: 2000,
        count: 1000,
        seed: None,
        dist: RandomDist::Bernoulli { p: 0.5 },
    };
    assert_eq!(
        run(&huge, &empty, None, false).unwrap_err(),
        AnalysisError::TooLarge
    );
}

#[test]
fn sampling_methods() {
    // hand: every 3rd of 1..10 → 3, 6, 9.
    let g = grid(&[&(1..=10).map(f64::from).collect::<Vec<_>>()]);
    let t = run_en(
        &Tool::Sampling {
            labels: false,
            method: SamplingMethod::Periodic { period: 3 },
            seed: None,
        },
        &g,
        None,
    );
    assert_eq!(
        t.rows,
        vec![
            vec![Out::Num(3.0)],
            vec![Out::Num(6.0)],
            vec![Out::Num(9.0)]
        ]
    );
    let random = Tool::Sampling {
        labels: false,
        method: SamplingMethod::Random { count: 20 },
        seed: Some(11),
    };
    let r = run_en(&random, &g, None);
    assert_eq!(r.rows.len(), 20);
    assert!(r.rows.iter().all(|x| (1.0..=10.0).contains(&n(&x[0]))));
    assert_eq!(r, run_en(&random, &g, None));
}

#[test]
fn regression_anscombe() {
    // Anscombe (1973), "Graphs in Statistical Analysis", data set I: fitted line
    // y = 3.00 + 0.500x, regression SS 27.51, residual SS 13.76 (df 9), standard error of the
    // slope 0.118, R² 0.667 (r = 0.816), 11 observations.
    let x = [10.0, 8.0, 13.0, 9.0, 11.0, 14.0, 6.0, 4.0, 12.0, 7.0, 5.0];
    let y = [
        8.04, 6.95, 7.58, 8.81, 8.33, 9.96, 7.24, 4.26, 10.84, 4.82, 5.68,
    ];
    let tool = Tool::Regression {
        labels: false,
        constant: true,
        confidence: 0.90,
        residuals: true,
        std_residuals: true,
    };
    let t = run_en(&tool, &grid(&[&y]), Some(&grid(&[&x])));
    assert_eq!(t.rows[0], vec![txt("Regression results")]);
    close(n(&row(&t, "R²")[1]), 0.667, 5e-4);
    close(n(&row(&t, "Correlation R")[1]), 0.816, 5e-4);
    close(n(&row(&t, "Observations")[1]), 11.0, TOL);
    let reg = row(&t, "Regression");
    close(n(&reg[1]), 1.0, TOL);
    close(n(&reg[2]), 27.51, 5e-3);
    let res = row(&t, "Residual");
    close(n(&res[1]), 9.0, TOL);
    close(n(&res[2]), 13.76, 5e-3);
    let total = row(&t, "Total");
    close(n(&total[1]), 10.0, TOL);
    let b0 = row(&t, "Intercept");
    close(n(&b0[1]), 3.00, 5e-3);
    let b1 = row(&t, "X Variable 1");
    close(n(&b1[1]), 0.500, 5e-4);
    close(n(&b1[2]), 0.118, 5e-4);
    // hand: t = b/se, interval b ± t(0.975, 9)·se with t table t(0.975, 9) = 2.262157.
    close(n(&b1[3]), n(&b1[1]) / n(&b1[2]), 1e-12);
    close(n(&b1[5]), n(&b1[1]) - 2.262_157 * n(&b1[2]), 1e-6);
    close(n(&b1[6]), n(&b1[1]) + 2.262_157 * n(&b1[2]), 1e-6);
    // hand: adjusted R² = 1 − (1 − R²)·10/9; F = MS reg / MS resid.
    let r2 = n(&row(&t, "R²")[1]);
    close(
        n(&row(&t, "Adjusted R²")[1]),
        1.0 - (1.0 - r2) * 10.0 / 9.0,
        1e-12,
    );
    close(n(&reg[4]), n(&reg[3]) / n(&res[3]), 1e-9);
    // The extra 90% interval uses t table t(0.95, 9) = 1.833113.
    let head = t
        .rows
        .iter()
        .find(|r| r.get(1) == Some(&txt("Coefficients")))
        .expect("coefficient header");
    assert_eq!(head[7], txt("Lower 90%"));
    close(n(&b1[7]), n(&b1[1]) - 1.833_113 * n(&b1[2]), 1e-6);
    // Residual output: 11 rows; residuals add up to 0 with a constant.
    let start = t
        .rows
        .iter()
        .position(|r| r.first() == Some(&txt("Observation")))
        .expect("residual header");
    assert_eq!(
        t.rows[start],
        vec![
            txt("Observation"),
            txt("Predicted Y"),
            txt("Residuals"),
            txt("Standardized residuals")
        ]
    );
    let resid: Vec<f64> = t.rows[start + 1..].iter().map(|r| n(&r[2])).collect();
    assert_eq!(resid.len(), 11);
    close(resid.iter().sum::<f64>(), 0.0, 1e-9);
    // hand: first observation x = 10 → predicted b0 + 10·b1.
    close(n(&t.rows[start + 1][1]), n(&b0[1]) + 10.0 * n(&b1[1]), 1e-9);
}

#[test]
fn regression_exact_two_variables() {
    // hand: y = 1 + 2·x1 + 3·x2 exactly; no constant → intercept 0 shown with #N/A.
    let x = rows(&[
        &[1.0, 0.0],
        &[0.0, 1.0],
        &[1.0, 1.0],
        &[2.0, 1.0],
        &[0.0, 2.0],
    ]);
    let y: Vec<f64> = [(1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (2.0, 1.0), (0.0, 2.0)]
        .iter()
        .map(|(a, b)| 1.0 + 2.0 * a + 3.0 * b)
        .collect();
    let mut yg = grid(&[&y]);
    let mut xg = x;
    yg.insert(0, vec![text("Y")]);
    xg.insert(0, vec![text("P"), text("Q")]);
    let tool = Tool::Regression {
        labels: true,
        constant: true,
        confidence: 0.95,
        residuals: false,
        std_residuals: false,
    };
    let t = run_en(&tool, &yg, Some(&xg));
    close(n(&row(&t, "Intercept")[1]), 1.0, 1e-9);
    close(n(&row(&t, "P")[1]), 2.0, 1e-9);
    close(n(&row(&t, "Q")[1]), 3.0, 1e-9);
    close(n(&row(&t, "R²")[1]), 1.0, 1e-12);
    assert_eq!(row(&t, "Regression")[1], Out::Num(2.0));
    let none = Tool::Regression {
        labels: true,
        constant: false,
        confidence: 0.95,
        residuals: false,
        std_residuals: false,
    };
    let t = run_en(&none, &yg, Some(&xg));
    let b0 = row(&t, "Intercept");
    assert_eq!(b0[1], Out::Num(0.0));
    assert_eq!(b0[2], Out::Error(OutError::NA));
    // Different row counts.
    let short = grid(&[&[1.0, 2.0]]);
    assert_eq!(
        run(&tool, &short, Some(&xg), false).unwrap_err(),
        AnalysisError::Shape
    );
}

#[test]
fn fourier_transform() {
    // hand: DFT of 1 2 3 4 — X0 = 10, X1 = 1 − 2i − 3 + 4i = −2 + 2i, X2 = 1 − 2 + 3 − 4 = −2,
    // X3 = −2 − 2i; the inverse gives the input back; an impulse transforms to all ones.
    let g = grid(&[&[1.0, 2.0, 3.0, 4.0]]);
    let tool = Tool::Fourier {
        labels: false,
        inverse: false,
    };
    let t = run_en(&tool, &g, None);
    let texts: Vec<Out> = t.rows.iter().map(|r| r[0].clone()).collect();
    assert_eq!(
        texts,
        vec![txt("10"), txt("-2+2i"), txt("-2"), txt("-2-2i")]
    );
    let back: Grid = vec![
        vec![text("FFT")],
        vec![text("10")],
        vec![text("-2+2i")],
        vec![text("-2")],
        vec![text("-2-2i")],
    ];
    let inv = run_en(
        &Tool::Fourier {
            labels: true,
            inverse: true,
        },
        &back,
        None,
    );
    let texts: Vec<Out> = inv.rows.iter().map(|r| r[0].clone()).collect();
    assert_eq!(
        texts,
        vec![
            txt("Inverse Fourier Transform"),
            txt("1"),
            txt("2"),
            txt("3"),
            txt("4")
        ]
    );
    let imp = run_en(
        &tool,
        &grid(&[&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]]),
        None,
    );
    assert!(imp.rows.iter().all(|r| r[0] == txt("1")));
    assert_eq!(
        run(&tool, &grid(&[&[1.0, 2.0, 3.0]]), None, false).unwrap_err(),
        AnalysisError::PowerOfTwo
    );
}

#[test]
fn tool_json() {
    let t: Tool = serde_json::from_str(
        r#"{ "tool": "tTestPaired", "labels": true, "alpha": 0.01, "hypothesized": 1 }"#,
    )
    .expect("json");
    assert_eq!(
        t,
        Tool::TTestPaired {
            labels: true,
            alpha: 0.01,
            hypothesized: 1.0
        }
    );
    let r: Tool = serde_json::from_str(
        r#"{ "tool": "random", "columns": 1, "count": 3, "dist": { "kind": "binomial", "p": 0.5, "trials": 4 } }"#,
    )
    .expect("json");
    assert!(!r.uses_input());
    let a: Tool = serde_json::from_str(r#"{ "tool": "anovaSingle" }"#).expect("json");
    assert_eq!(
        a,
        Tool::AnovaSingle {
            by_rows: false,
            labels: false,
            alpha: 0.05
        }
    );
}
