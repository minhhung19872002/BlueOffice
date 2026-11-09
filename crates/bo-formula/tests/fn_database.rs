//! Database functions (DAVERAGE … DVARP).
//!
//! Sources: ECMA-376 Part 1, §18.17.7 for the database / field / criteria arguments (criteria
//! in one row combine with AND, rows with OR). Expected values are worked by hand from the
//! table below.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[
    ("A1", "Tree"),
    ("B1", "Height"),
    ("C1", "Age"),
    ("D1", "Yield"),
    ("E1", "Profit"),
    ("A2", "Apple"),
    ("B2", "18"),
    ("C2", "20"),
    ("D2", "14"),
    ("E2", "105"),
    ("A3", "Pear"),
    ("B3", "12"),
    ("C3", "12"),
    ("D3", "10"),
    ("E3", "96"),
    ("A4", "Cherry"),
    ("B4", "13"),
    ("C4", "14"),
    ("D4", "9"),
    ("E4", "105"),
    ("A5", "Apple"),
    ("B5", "14"),
    ("C5", "15"),
    ("D5", "10"),
    ("E5", "75"),
    ("A6", "Pear"),
    ("B6", "9"),
    ("C6", "8"),
    ("D6", "8"),
    ("E6", "76.8"),
    ("A7", "Apple"),
    ("B7", "8"),
    ("C7", "9"),
    ("D7", "6"),
    ("E7", "45"),
    // Apples taller than 10, or any pear.
    ("G1", "Tree"),
    ("H1", "Height"),
    ("G2", "Apple"),
    ("H2", ">10"),
    ("G3", "Pear"),
    // Cherries only.
    ("J1", "Tree"),
    ("J2", "Cherry"),
    // No match.
    ("K1", "Tree"),
    ("K2", "Plum"),
    // Every apple.
    ("L1", "tree"),
    ("L2", "Apple"),
];

#[test]
fn database_functions() {
    check(
        DATA,
        &[
            (
                "=DSUM(A1:E7,\"Profit\",G1:H3)",
                N(352.8),
                "hand: 105+75+96+76.8",
            ),
            (
                "=DSUM(A1:E7,5,G1:H3)",
                N(352.8),
                "hand: field by column number",
            ),
            ("=DCOUNT(A1:E7,\"Age\",G1:H3)", N(4.0), "hand: four records"),
            (
                "=DCOUNT(A1:E7,,G1:H3)",
                N(4.0),
                "hand: records when the field is omitted",
            ),
            (
                "=DCOUNT(A1:E7,\"Tree\",G1:H3)",
                N(0.0),
                "hand: no numbers in Tree",
            ),
            (
                "=DCOUNTA(A1:E7,\"Tree\",G1:H3)",
                N(4.0),
                "hand: non-blank values",
            ),
            (
                "=DAVERAGE(A1:E7,\"Yield\",G1:H3)",
                N(10.5),
                "hand: (14+10+10+8)/4",
            ),
            (
                "=DMAX(A1:E7,\"Profit\",G1:H3)",
                N(105.0),
                "hand: by definition",
            ),
            (
                "=DMIN(A1:E7,\"Profit\",G1:H3)",
                N(75.0),
                "hand: by definition",
            ),
            (
                "=DPRODUCT(A1:E7,\"Yield\",G1:H3)",
                N(11_200.0),
                "hand: 14*10*10*8",
            ),
            (
                "=DSTDEV(A1:E7,\"Yield\",G1:H3)",
                N((19.0f64 / 3.0).sqrt()),
                "hand: squared deviations sum to 19",
            ),
            (
                "=DSTDEVP(A1:E7,\"Yield\",G1:H3)",
                N((19.0f64 / 4.0).sqrt()),
                "hand: 19/4",
            ),
            ("=DVAR(A1:E7,\"Yield\",G1:H3)", N(19.0 / 3.0), "hand: 19/3"),
            ("=DVARP(A1:E7,\"Yield\",G1:H3)", N(4.75), "hand: 19/4"),
            (
                "=DGET(A1:E7,\"Yield\",J1:J2)",
                N(9.0),
                "hand: the only cherry",
            ),
            (
                "=DGET(A1:E7,\"Yield\",G1:H3)",
                E("#NUM!"),
                "ECMA-376 §18.17.7 DGET",
            ),
            (
                "=DGET(A1:E7,\"Yield\",K1:K2)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 DGET",
            ),
            (
                "=DAVERAGE(A1:E7,\"Yield\",K1:K2)",
                E("#DIV/0!"),
                "hand: no record to average",
            ),
            (
                "=DSUM(A1:E7,\"Yield\",L1:L2)",
                N(30.0),
                "hand: labels match ignoring case, 14+10+6",
            ),
            (
                "=DSUM(A1:E7,\"Weight\",G1:H3)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 DSUM",
            ),
            (
                "=DSUM(A1:E7,\"Profit\",A1:A1)",
                N(502.8),
                "hand: a criteria range without conditions keeps every record",
            ),
        ],
    );
}
