//! ENCODEURL, FILTERXML and WEBSERVICE.
//!
//! Sources: [MS-XLSX] future functions `_xlfn.ENCODEURL`, `_xlfn.FILTERXML`,
//! `_xlfn.WEBSERVICE`; percent-encoding per RFC 3986 §2.1/§2.3 with UTF-8 bytes worked out by
//! hand; FILTERXML node selection per XPath 1.0 (§2 location paths, §2.4 predicates, §3.4
//! comparisons). WEBSERVICE runs against an in-test provider — no network.

mod common;
use common::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bo_formula::{
    from_storage, is_volatile_function, to_storage, ErrorKind, Value, WebError, WebProvider,
    Workbook,
};

const BOOKS: &str = "'<catalog>\
<book id=\"b1\" lang=\"vi\"><title>Truyện Kiều</title><price>120000</price></book>\
<book id=\"b2\" lang=\"en\"><title>Hamlet</title><price>85.5</price></book>\
<book id=\"b3\" lang=\"vi\"><title>Số đỏ</title><price>99000</price></book>\
</catalog>";

const DATA: &[(&str, &str)] = &[
    ("A1", BOOKS),
    ("A2", "'Hà Nội"),
    // Pretty-printed XML with an entity, CDATA and a namespace prefix.
    (
        "A3",
        "'<?xml version=\"1.0\"?>\n<r xmlns:m=\"urn:x\">\n  <m:v>a &amp; b</m:v>\n  <w><![CDATA[1<2]]></w>\n</r>",
    ),
];

#[test]
fn encodeurl() {
    check(
        DATA,
        &[
            (
                "=ENCODEURL(A2)",
                T("H%C3%A0%20N%E1%BB%99i"),
                "hand: à = U+00E0 = C3 A0, space = 20, ộ = U+1ED9 = E1 BB 99 (RFC 3986 §2.1)",
            ),
            (
                "=ENCODEURL(\"A-Z_a.z~09\")",
                T("A-Z_a.z~09"),
                "hand: unreserved characters stay (RFC 3986 §2.3)",
            ),
            (
                "=ENCODEURL(\"a b&c=d/e?f\")",
                T("a%20b%26c%3Dd%2Fe%3Ff"),
                "hand: space 20, & 26, = 3D, / 2F, ? 3F",
            ),
            (
                "=ENCODEURL(\"100%+1\")",
                T("100%25%2B1"),
                "hand: % 25, + 2B",
            ),
            (
                "=ENCODEURL(\"€\")",
                T("%E2%82%AC"),
                "hand: U+20AC = E2 82 AC",
            ),
            (
                "=ENCODEURL(12.5)",
                T("12.5"),
                "hand: number as text, digits and dot kept",
            ),
            ("=ENCODEURL(\"\")", T(""), "hand: empty text"),
            ("=ENCODEURL(1/0)", E("#DIV/0!"), "hand: errors propagate"),
            (
                "=ENCODEURL({\"a b\";\"c\"})",
                A("{\"a%20b\";\"c\"}"),
                "hand: scalar argument broadcast over an array",
            ),
        ],
    );
}

#[test]
fn filterxml_paths_and_predicates() {
    check(
        DATA,
        &[
            (
                "=FILTERXML(A1,\"//title\")",
                A("{\"Truyện Kiều\";\"Hamlet\";\"Số đỏ\"}"),
                "hand: every title in document order, one column (XPath 1.0 §2.5 //)",
            ),
            (
                "=FILTERXML(A1,\"/catalog/book[2]/title\")",
                T("Hamlet"),
                "hand: second book child (§2.4 positional predicate)",
            ),
            (
                "=FILTERXML(A1,\"catalog/book[1]/title\")",
                T("Truyện Kiều"),
                "hand: relative path from the document root",
            ),
            (
                "=FILTERXML(A1,\"//book[last()]/@id\")",
                T("b3"),
                "hand: last() is the context size 3 (§4.1)",
            ),
            (
                "=FILTERXML(A1,\"//book[@lang='vi']/title\")",
                A("{\"Truyện Kiều\";\"Số đỏ\"}"),
                "hand: attribute equality predicate (§3.4)",
            ),
            (
                "=FILTERXML(A1,\"//book[title='Hamlet']/price\")",
                N(85.5),
                "hand: child element equality; numeric text becomes a number",
            ),
            (
                "=FILTERXML(A1,\"//price\")",
                A("{120000;85.5;99000}"),
                "hand: numeric node values converted",
            ),
            (
                "=FILTERXML(A1,\"/catalog/book[position()<3]/@id\")",
                A("{\"b1\";\"b2\"}"),
                "hand: position() 1 and 2",
            ),
            (
                "=FILTERXML(A1,\"//book[price>90000]/@id\")",
                A("{\"b1\";\"b3\"}"),
                "hand: 120000 and 99000 exceed 90000 (§3.4 node-set vs number)",
            ),
            (
                "=FILTERXML(A1,\"//book[3]/*\")",
                A("{\"Số đỏ\";99000}"),
                "hand: * selects both element children",
            ),
            (
                "=FILTERXML(A1,\"//*[@id='b2']/../book[1]/@lang\")",
                T("vi"),
                "hand: .. goes back to catalog, then its first book",
            ),
            (
                "=FILTERXML(A1,\"(//title)[2]\")",
                T("Hamlet"),
                "hand: predicate on the whole node set (§3.3 filter expression)",
            ),
            (
                "=FILTERXML(A1,\"//book[@id='b1' or @id='b3']/price/text()\")",
                A("{120000;99000}"),
                "hand: or of two attribute tests, text() nodes",
            ),
            (
                "=FILTERXML(A1,\"//book[contains(title,'Kiều')]/@id\")",
                T("b1"),
                "hand: contains() (§4.2)",
            ),
            (
                "=FILTERXML(A1,\"//book/@id | //book[2]/title\")",
                A("{\"b1\";\"b2\";\"Hamlet\";\"b3\"}"),
                "hand: union in document order (§3.3)",
            ),
            (
                "=SUM(FILTERXML(A1,\"//price\"))",
                N(219_085.5),
                "hand: 120000 + 85.5 + 99000",
            ),
            (
                "=INDEX(FILTERXML(A1,\"//title\"),3)",
                T("Số đỏ"),
                "hand: third row of the result",
            ),
            (
                "=FILTERXML(A3,\"//m:v\")",
                T("a & b"),
                "hand: prefixed name as written, &amp; resolved",
            ),
            (
                "=FILTERXML(A3,\"/r/w\")",
                T("1<2"),
                "hand: CDATA text is literal",
            ),
            (
                "=FILTERXML(A3,\"count(/r/*)\")",
                E("#VALUE!"),
                "hand: a number, not a node set",
            ),
            (
                "=ROWS(FILTERXML(A3,\"/r/node()\"))",
                N(2.0),
                "hand: whitespace-only text between elements is not a node",
            ),
        ],
    );
}

#[test]
fn filterxml_errors() {
    check(
        DATA,
        &[
            (
                "=FILTERXML(A1,\"//magazine\")",
                E("#VALUE!"),
                "hand: no node selected",
            ),
            (
                "=FILTERXML(\"<a><b>1</a>\",\"//b\")",
                E("#VALUE!"),
                "hand: mismatched end tag",
            ),
            (
                "=FILTERXML(\"<a>1</a><b/>\",\"//a\")",
                E("#VALUE!"),
                "hand: two document elements",
            ),
            (
                "=FILTERXML(\"<a>&nbsp;</a>\",\"//a\")",
                E("#VALUE!"),
                "hand: undeclared entity",
            ),
            (
                "=FILTERXML(\"<a><b>\",\"//b\")",
                E("#VALUE!"),
                "hand: unclosed elements",
            ),
            (
                "=FILTERXML(\"plain text\",\"//a\")",
                E("#VALUE!"),
                "hand: not XML",
            ),
            (
                "=FILTERXML(A1,\"//book[\")",
                E("#VALUE!"),
                "hand: unterminated predicate",
            ),
            (
                "=FILTERXML(A1,\"//book/unknown-fn()\")",
                E("#VALUE!"),
                "hand: unknown function",
            ),
            ("=FILTERXML(A1,\"\")", E("#VALUE!"), "hand: empty XPath"),
            (
                "=FILTERXML(1/0,\"//a\")",
                E("#DIV/0!"),
                "hand: errors propagate",
            ),
            (
                "=FILTERXML(\"<a><b>x</b><b/></a>\",\"//b\")",
                A("{\"x\";\"\"}"),
                "hand: an empty element has the empty string value",
            ),
        ],
    );
}

#[test]
fn webservice_disabled_by_default() {
    check(
        DATA,
        &[
            (
                "=WEBSERVICE(\"https://example.com/rate\")",
                E("#VALUE!"),
                "hand: no provider installed — web access is off by default",
            ),
            (
                "=WEBSERVICE(\"ftp://example.com\")",
                E("#VALUE!"),
                "hand: only http and https",
            ),
        ],
    );
    assert!(is_volatile_function("WEBSERVICE"));
    assert!(!is_volatile_function("FILTERXML"));
    assert!(!is_volatile_function("ENCODEURL"));
}

/// In-test provider: canned responses, records every request.
#[derive(Default)]
struct Canned {
    calls: AtomicUsize,
    urls: Mutex<Vec<String>>,
}

impl WebProvider for Canned {
    fn fetch(&self, url: &str) -> Result<String, WebError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut u) = self.urls.lock() {
            u.push(url.to_owned());
        }
        match url {
            "https://api.example.vn/rate?ccy=USD" => Ok("25400".to_owned()),
            "https://api.example.vn/rates.xml" => Ok(
                "<rates><rate ccy=\"USD\">25400</rate><rate ccy=\"EUR\">27500.5</rate></rates>"
                    .to_owned(),
            ),
            "https://api.example.vn/w?city=H%C3%A0%20N%E1%BB%99i" => Ok("29°C".to_owned()),
            "https://api.example.vn/big" => Ok("x".repeat(32_768)),
            "http://api.example.vn/blocked" => Err(WebError::Blocked),
            _ => Err(WebError::Failed("404".to_owned())),
        }
    }
}

#[test]
fn webservice_with_provider() {
    let provider = Arc::new(Canned::default());
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.set_web_provider(Some(provider.clone()));
    let long = format!("=WEBSERVICE(\"https://e.vn/{}\")", "a".repeat(2_040));
    let cases: &[(&str, Value)] = &[
        (
            "=WEBSERVICE(\"https://api.example.vn/rate?ccy=USD\")",
            Value::Text("25400".to_owned()),
        ),
        (
            "=VALUE(WEBSERVICE(\"https://api.example.vn/rate?ccy=USD\"))*2",
            Value::Number(50_800.0),
        ),
        (
            "=FILTERXML(WEBSERVICE(\"https://api.example.vn/rates.xml\"),\"//rate[@ccy='EUR']\")",
            Value::Number(27_500.5),
        ),
        (
            "=WEBSERVICE(\"https://api.example.vn/w?city=\"&ENCODEURL(\"Hà Nội\"))",
            Value::Text("29°C".to_owned()),
        ),
        (
            "=WEBSERVICE(\"https://api.example.vn/missing\")",
            Value::Error(ErrorKind::Value),
        ),
        (
            "=WEBSERVICE(\"http://api.example.vn/blocked\")",
            Value::Error(ErrorKind::Value),
        ),
        (
            "=WEBSERVICE(\"https://api.example.vn/big\")",
            Value::Error(ErrorKind::Value),
        ),
        (
            "=WEBSERVICE(\"ftp://api.example.vn/x\")",
            Value::Error(ErrorKind::Value),
        ),
        (
            "=WEBSERVICE(\"api.example.vn\")",
            Value::Error(ErrorKind::Value),
        ),
        (&long, Value::Error(ErrorKind::Value)),
    ];
    for (i, (f, _)) in cases.iter().enumerate() {
        wb.set_formula(0, i as u32, 0, f).expect("formula parses");
    }
    wb.recalculate();
    for (i, (f, want)) in cases.iter().enumerate() {
        assert_eq!(&wb.value(0, i as u32, 0), want, "{f}");
    }
    // Rejected URLs (scheme, length) never reach the provider: 7 fetches.
    assert_eq!(provider.calls.load(Ordering::Relaxed), 7);
    let urls = provider.urls.lock().map(|u| u.clone()).unwrap_or_default();
    assert!(urls
        .iter()
        .all(|u| !u.starts_with("ftp") && u.len() <= 2_048));
    // Volatile: every recalculation fetches again.
    wb.recalculate();
    assert_eq!(provider.calls.load(Ordering::Relaxed), 14);
    // Removing the provider disables web access again.
    wb.set_web_provider(None);
    wb.recalculate();
    assert_eq!(wb.value(0, 0, 0), Value::Error(ErrorKind::Value));
    assert_eq!(provider.calls.load(Ordering::Relaxed), 14);
}

#[test]
fn web_functions_use_xlfn_storage_prefix() {
    let shown = "ENCODEURL(A1)&WEBSERVICE(B1)&FILTERXML(C1,\"//a\")";
    let stored = "_xlfn.ENCODEURL(A1)&_xlfn.WEBSERVICE(B1)&_xlfn.FILTERXML(C1,\"//a\")";
    assert_eq!(to_storage(shown), stored);
    assert_eq!(from_storage(stored), shown);
}

/// A workbook saved where web access worked keeps the response it saved (ECMA-376 Part 1,
/// §18.3.1.96 `v`) while no provider is installed, instead of turning into `#VALUE!` on
/// the first recalculation; dependents read the kept value. A formula typed afresh (no
/// cache) or a re-entered formula still gives `#VALUE!`.
#[test]
fn webservice_keeps_cached_value_without_provider() {
    let mut wb = Workbook::with_sheets(&["Sheet1"]);
    wb.load_formula(
        0,
        0,
        0,
        "WEBSERVICE(\"https://api.example.vn/rate?ccy=USD\")",
        Some(Value::Text("25400".to_owned())),
    )
    .expect("formula parses");
    wb.load_formula(
        0,
        1,
        0,
        "FILTERXML(WEBSERVICE(\"https://api.example.vn/rates.xml\"),\"//rate[@ccy='EUR']\")",
        Some(Value::Number(27_500.5)),
    )
    .expect("formula parses");
    wb.load_formula(0, 2, 0, "VALUE(A1)*2", Some(Value::Number(0.0)))
        .expect("formula parses");
    // No cached value: nothing to keep.
    wb.load_formula(0, 3, 0, "WEBSERVICE(\"https://api.example.vn/x\")", None)
        .expect("formula parses");
    wb.recalculate_all();
    assert_eq!(wb.value(0, 0, 0), Value::Text("25400".to_owned()));
    assert_eq!(wb.value(0, 1, 0), Value::Number(27_500.5));
    assert_eq!(
        wb.value(0, 2, 0),
        Value::Number(50_800.0),
        "hand: 25400 × 2"
    );
    assert_eq!(wb.value(0, 3, 0), Value::Error(ErrorKind::Value));
    wb.recalculate();
    assert_eq!(wb.value(0, 0, 0), Value::Text("25400".to_owned()));
    // Once a provider exists the function fetches again.
    let provider = Arc::new(Canned::default());
    wb.set_web_provider(Some(provider.clone()));
    wb.recalculate();
    assert_eq!(wb.value(0, 0, 0), Value::Text("25400".to_owned()));
    assert_eq!(wb.value(0, 3, 0), Value::Error(ErrorKind::Value));
    assert!(provider.calls.load(Ordering::Relaxed) >= 3);
    // Re-entering the formula drops the file's cache.
    wb.set_web_provider(None);
    wb.set_formula(0, 1, 0, "=WEBSERVICE(\"https://api.example.vn/rates.xml\")")
        .expect("formula parses");
    wb.recalculate();
    assert_eq!(wb.value(0, 1, 0), Value::Error(ErrorKind::Value));
}

#[test]
fn filterxml_resolves_document_namespaces() {
    let rss = "'<rss xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><channel>\
<item><title>A</title><creator xmlns=\"http://purl.org/dc/elements/1.1/\">Lan</creator></item>\
<item><title>B</title><dc:creator>Minh</dc:creator></item></channel></rss>";
    check(
        &[("A1", rss)],
        &[
            (
                "=FILTERXML(A1,\"//dc:creator\")",
                A("{\"Lan\";\"Minh\"}"),
                "hand: dc bound to the Dublin Core URI; the first creator is in that namespace \
                 through a default declaration (Namespaces in XML 1.0 §6.2)",
            ),
            (
                "=FILTERXML(A1,\"//item[dc:creator='Minh']/title\")",
                T("B"),
                "hand: prefixed name inside a predicate",
            ),
            (
                "=FILTERXML(A1,\"namespace-uri(//dc:creator)\")",
                E("#VALUE!"),
                "hand: a string, not a node set",
            ),
            (
                "=FILTERXML(A1,\"//*[namespace-uri()='http://purl.org/dc/elements/1.1/']\")",
                A("{\"Lan\";\"Minh\"}"),
                "hand: namespace-uri() (XPath 1.0 §4.1)",
            ),
        ],
    );
}
