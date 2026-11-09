//! Flash Fill: hand-written cases. Each expected value is what a person would
//! type after looking at the examples.

use bo_sheet::flashfill::{flash_fill, Example};

struct Case {
    name: &'static str,
    /// (sources, output) pairs typed by the user.
    examples: &'static [(&'static [&'static str], &'static str)],
    /// Rows to fill and what a person expects there (`None`: left empty).
    rows: &'static [(&'static [&'static str], Option<&'static str>)],
}

const CASES: &[Case] = &[
    Case {
        name: "family name (first word)",
        examples: &[(&["Nguyễn Văn An"], "Nguyễn")],
        rows: &[
            (&["Trần Thị Bích"], Some("Trần")),
            (&["Lê Hoàng"], Some("Lê")),
        ],
    },
    Case {
        name: "given name (last word), names of different lengths",
        examples: &[
            (&["Nguyễn Văn An"], "An"),
            (&["Trần Thị Bích Ngọc"], "Ngọc"),
        ],
        rows: &[
            (&["Lê Minh Tuấn"], Some("Tuấn")),
            (&["Phạm Hùng"], Some("Hùng")),
        ],
    },
    Case {
        name: "middle name, missing in a one-word name",
        examples: &[(&["Nguyễn Văn An"], "Văn")],
        rows: &[(&["Lê Minh Tuấn"], Some("Minh")), (&["Hùng"], None)],
    },
    Case {
        name: "last name in English",
        examples: &[(&["John Smith"], "Smith")],
        rows: &[
            (&["Mary Jones"], Some("Jones")),
            (&["Anna Maria Lopez"], Some("Lopez")),
        ],
    },
    Case {
        name: "swap 'Last, First'",
        examples: &[(&["Smith, John"], "John Smith")],
        rows: &[(&["Doe, Jane"], Some("Jane Doe"))],
    },
    Case {
        name: "initials",
        examples: &[(&["Nguyễn Văn An"], "NVA")],
        rows: &[
            (&["Trần Thị Bích"], Some("TTB")),
            (&["Lê Minh Tuấn"], Some("LMT")),
            (&["Đỗ Đức Anh"], Some("ĐĐA")),
        ],
    },
    Case {
        name: "initials with dots",
        examples: &[(&["John Smith"], "J.S.")],
        rows: &[(&["Mary Jones"], Some("M.J."))],
    },
    Case {
        name: "email from first and last name",
        examples: &[
            (&["John", "Smith"], "john.smith@example.com"),
            (&["Mary", "Jones"], "mary.jones@example.com"),
        ],
        rows: &[
            (&["Peter", "Brown"], Some("peter.brown@example.com")),
            (&["Anna", "Lee"], Some("anna.lee@example.com")),
        ],
    },
    Case {
        name: "join two columns",
        examples: &[(&["Nguyễn", "An"], "An Nguyễn")],
        rows: &[(&["Trần", "Bích"], Some("Bích Trần"))],
    },
    Case {
        name: "local part of an email",
        examples: &[(&["an.nguyen@bluestar.vn"], "an.nguyen")],
        rows: &[(&["hung.bui@example.com"], Some("hung.bui"))],
    },
    Case {
        name: "domain of an email",
        examples: &[(&["an.nguyen@bluestar.vn"], "bluestar.vn")],
        rows: &[(&["hung@example.com"], Some("example.com"))],
    },
    Case {
        name: "phone digits",
        examples: &[(&["(090) 123-4567"], "0901234567")],
        rows: &[
            (&["091 765 4321"], Some("0917654321")),
            (&["(028) 3822-1234"], Some("02838221234")),
        ],
    },
    Case {
        name: "last four digits",
        examples: &[(&["0901234567"], "4567")],
        rows: &[(&["0987654321"], Some("4321"))],
    },
    Case {
        name: "upper case",
        examples: &[(&["hello world"], "HELLO WORLD")],
        rows: &[(&["xin chào"], Some("XIN CHÀO"))],
    },
    Case {
        name: "lower case with diacritics",
        examples: &[(&["ĐÀ NẴNG"], "đà nẵng")],
        rows: &[(&["HÀ NỘI"], Some("hà nội"))],
    },
    Case {
        name: "proper case with diacritics",
        examples: &[(&["nguyễn văn an"], "Nguyễn Văn An")],
        rows: &[(&["trần thị bích ngọc"], Some("Trần Thị Bích Ngọc"))],
    },
    Case {
        name: "reorder date parts",
        examples: &[(&["2024-03-15"], "15/03/2024")],
        rows: &[(&["1999-12-01"], Some("01/12/1999"))],
    },
    Case {
        name: "year of a date",
        examples: &[(&["15/03/2024"], "2024")],
        rows: &[(&["01/12/1999"], Some("1999"))],
    },
    Case {
        name: "date from Vietnamese text",
        examples: &[(&["Ngày 05 tháng 09 năm 2023"], "05/09/2023")],
        rows: &[(&["Ngày 20 tháng 11 năm 2025"], Some("20/11/2025"))],
    },
    Case {
        name: "letters of a product code",
        examples: &[(&["SP-00123"], "SP")],
        rows: &[(&["HH-778"], Some("HH"))],
    },
    Case {
        name: "number of a product code",
        examples: &[(&["SP-00123"], "00123")],
        rows: &[(&["HH-778"], Some("778"))],
    },
    Case {
        name: "constant prefix",
        examples: &[(&["An"], "Ông An")],
        rows: &[(&["Bình"], Some("Ông Bình"))],
    },
];

fn examples(c: &Case) -> Vec<Example> {
    c.examples.iter().map(|(i, o)| Example::new(i, o)).collect()
}

fn rows(list: &[&[&str]]) -> Vec<Vec<String>> {
    list.iter()
        .map(|r| r.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

#[test]
fn hand_written_cases() {
    let mut failures = Vec::new();
    for c in CASES {
        let inputs = rows(&c.rows.iter().map(|(i, _)| *i).collect::<Vec<_>>());
        let want: Vec<Option<String>> = c.rows.iter().map(|(_, o)| o.map(str::to_owned)).collect();
        let got = flash_fill(&examples(c), &inputs);
        if got.as_ref() != Some(&want) {
            failures.push(format!("{}: want {want:?}, got {got:?}", c.name));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn ambiguous_sources_give_no_result() {
    // Both columns read "abc": either could be meant, and they differ below.
    let ex = [Example::new(&["abc", "abc"], "abc")];
    assert_eq!(flash_fill(&ex, &rows(&[&["x", "y"]])), None);
}

#[test]
fn inconsistent_examples_give_no_result() {
    let ex = [Example::new(&["a"], "1"), Example::new(&["b"], "2")];
    assert_eq!(flash_fill(&ex, &rows(&[&["c"]])), None);
}

#[test]
fn constant_output_is_not_a_pattern() {
    let ex = [Example::new(&["x"], "N/A"), Example::new(&["y"], "N/A")];
    assert_eq!(flash_fill(&ex, &rows(&[&["z"]])), None);
}

#[test]
fn decomposed_input_matches_composed_examples() {
    // "Trần Thị" typed with combining marks still splits into whole words.
    let ex = [Example::new(&["Nguyễn Văn An"], "Nguyễn")];
    let got = flash_fill(&ex, &rows(&[&["Tra\u{302}\u{300}n Thi\u{323}"]]));
    assert_eq!(got, Some(vec![Some("Trần".to_owned())]));
}
