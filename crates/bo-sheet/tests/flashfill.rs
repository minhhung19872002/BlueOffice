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
    // --- repeat over every token (names of any length) ---
    Case {
        name: "initials of every word, any number of words",
        examples: &[(&["Nguyễn Văn An"], "NVA")],
        rows: &[
            (&["Trần Thị Bích Ngọc"], Some("TTBN")),
            (&["Lê Hùng"], Some("LH")),
            (&["Tôn Nữ Thị Minh Châu"], Some("TNTMC")),
        ],
    },
    Case {
        name: "initials with dots, any number of words",
        examples: &[(&["John Ronald Tolkien"], "J.R.T.")],
        rows: &[
            (&["Mary Jones"], Some("M.J.")),
            (&["Ho Chi Minh City"], Some("H.C.M.C.")),
        ],
    },
    Case {
        name: "slug: every word lower case joined by hyphens",
        examples: &[(&["Hà Nội Mới"], "hà-nội-mới")],
        rows: &[
            (&["Thành phố Hồ Chí Minh"], Some("thành-phố-hồ-chí-minh")),
            (&["Đà Nẵng"], Some("đà-nẵng")),
        ],
    },
    Case {
        name: "initials of every word then the last word",
        examples: &[(&["Nguyễn Văn An"], "NVA - An")],
        rows: &[(&["Trần Thị Bích Ngọc"], Some("TTBN - Ngọc"))],
    },
    // --- nested tokens ---
    Case {
        name: "first label of an e-mail domain (field inside a field)",
        examples: &[
            (&["an.nguyen@bluestar.vn"], "bluestar"),
            (&["hung@cty.com.vn"], "cty"),
        ],
        rows: &[
            (&["lan.tran.thi@mail.gov.vn"], Some("mail")),
            (&["minh@example.org"], Some("example")),
        ],
    },
    Case {
        name: "file name without extension from a path",
        examples: &[
            (&["/home/an/bao-cao.v2.pdf"], "bao-cao"),
            (&["/srv/data/ke-hoach-2025.xlsx"], "ke-hoach-2025"),
        ],
        rows: &[
            (&["/tmp/so-lieu.csv"], Some("so-lieu")),
            (&["/a/b/c/du-toan-q3.final.docx"], Some("du-toan-q3")),
        ],
    },
    // --- month names and numbers ---
    Case {
        name: "month number to Vietnamese month name",
        examples: &[(&["15/03/2024"], "tháng Ba")],
        rows: &[
            (&["01/12/1999"], Some("tháng Mười Hai")),
            (&["20/1/2025"], Some("tháng Một")),
            (&["07/11/2023"], Some("tháng Mười Một")),
            (&["30/04/1975"], Some("tháng Tư")),
        ],
    },
    Case {
        name: "month number without leading zero",
        examples: &[(&["15/03/2024"], "tháng 3 năm 2024")],
        rows: &[
            (&["01/12/1999"], Some("tháng 12 năm 1999")),
            (&["02/09/1945"], Some("tháng 9 năm 1945")),
        ],
    },
    Case {
        name: "Vietnamese date in words to dd/mm/yyyy",
        examples: &[(&["ngày 5 tháng Ba năm 2024"], "05/03/2024")],
        rows: &[
            (&["ngày 17 tháng Mười Một năm 2023"], Some("17/11/2023")),
            (&["ngày 1 tháng Giêng năm 2025"], Some("01/01/2025")),
            (&["Ngày 20 tháng 10 năm 2020"], Some("20/10/2020")),
        ],
    },
    Case {
        name: "English month name to number",
        examples: &[(&["March 5, 2024"], "2024-03-05")],
        rows: &[
            (&["December 25, 1999"], Some("1999-12-25")),
            (&["Sep 2, 1945"], Some("1945-09-02")),
        ],
    },
    Case {
        name: "month number to English abbreviation",
        examples: &[(&["2024-03-15"], "15 Mar 2024")],
        rows: &[(&["1999-12-01"], Some("01 Dec 1999"))],
    },
    Case {
        name: "month name of a day/month/year date",
        examples: &[(&["15/03/2024"], "March")],
        rows: &[
            (&["02/11/2023"], Some("November")),
            (&["7/1/2022"], Some("January")),
        ],
    },
    Case {
        name: "short month name with the year",
        examples: &[(&["15/03/2024"], "Mar 2024")],
        rows: &[(&["28/12/2025"], Some("Dec 2025"))],
    },
    Case {
        name: "month name to a two-digit number",
        examples: &[(&["Mar 2024"], "03/2024")],
        rows: &[
            (&["Oct 2023"], Some("10/2023")),
            (&["december 2025"], Some("12/2025")),
        ],
    },
    Case {
        name: "full month name to its number",
        examples: &[(&["Ngày 5 September"], "9")],
        rows: &[(&["Ngày 20 May"], Some("5"))],
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
