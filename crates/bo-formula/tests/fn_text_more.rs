//! Byte variants, width conversion, DOLLAR, BAHTTEXT, PHONETIC, ARRAYTOTEXT / VALUETOTEXT,
//! REGEX* and BO.DOCSO.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (LENB … REPLACEB count two bytes for a double-byte
//! character; ASC, DBCS, DOLLAR, BAHTTEXT, PHONETIC); [MS-XLSX] future functions for
//! ARRAYTOTEXT, VALUETOTEXT and REGEX*. Thai and Vietnamese number words follow the languages'
//! counting rules, worked by hand.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[("A1", "東京")];

#[test]
fn byte_variants_and_width() {
    check(
        DATA,
        &[
            ("=LENB(\"abc\")", N(3.0), "hand: one byte each"),
            ("=LENB(\"日本\")", N(4.0), "hand: two bytes per ideograph"),
            (
                "=LEFTB(\"日本語\",3)",
                T("日"),
                "hand: a half character is not taken",
            ),
            ("=LEFTB(\"日本語\",4)", T("日本"), "hand: 4 bytes"),
            ("=RIGHTB(\"日本語\",4)", T("本語"), "hand: last 4 bytes"),
            ("=MIDB(\"日本語\",3,2)", T("本"), "hand: bytes 3-4"),
            (
                "=FINDB(\"本\",\"日本語\")",
                N(3.0),
                "hand: after one 2-byte character",
            ),
            (
                "=SEARCHB(\"B\",\"a日b\")",
                N(4.0),
                "hand: case-insensitive, a + 2 bytes",
            ),
            (
                "=REPLACEB(\"日本語\",3,2,\"x\")",
                T("日x語"),
                "hand: bytes 3-4 replaced",
            ),
            (
                "=LEFTB(\"abc\",-1)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 LEFTB",
            ),
            (
                "=ASC(\"ＡＢＣ　１\")",
                T("ABC 1"),
                "hand: full-width forms U+FF21… to ASCII",
            ),
            (
                "=DBCS(\"AB 1\")",
                T("ＡＢ　１"),
                "hand: ASCII to full-width forms",
            ),
            (
                "=PHONETIC(A1)",
                T("東京"),
                "hand: no phonetic guide, the text",
            ),
        ],
    );
}

#[test]
fn currency_and_number_words() {
    check(
        DATA,
        &[
            (
                "=DOLLAR(1234.567,2)",
                T("$1,234.57"),
                "ECMA-376 §18.17.7 DOLLAR",
            ),
            (
                "=DOLLAR(-1234.567,-2)",
                T("($1,200)"),
                "ECMA-376 §18.17.7 DOLLAR",
            ),
            ("=DOLLAR(0.123,4)", T("$0.1230"), "ECMA-376 §18.17.7 DOLLAR"),
            ("=DOLLAR(99.888)", T("$99.89"), "ECMA-376 §18.17.7 DOLLAR"),
            (
                "=BAHTTEXT(1234)",
                T("หนึ่งพันสองร้อยสามสิบสี่บาทถ้วน"),
                "hand: 1 thousand 2 hundred 3 tens 4, exact",
            ),
            (
                "=BAHTTEXT(21)",
                T("ยี่สิบเอ็ดบาทถ้วน"),
                "hand: yi-sip for 20, et for a final 1",
            ),
            ("=BAHTTEXT(11)", T("สิบเอ็ดบาทถ้วน"), "hand: sip for 10"),
            (
                "=BAHTTEXT(1.5)",
                T("หนึ่งบาทห้าสิบสตางค์"),
                "hand: 1 baht 50 satang",
            ),
            ("=BAHTTEXT(1000000)", T("หนึ่งล้านบาทถ้วน"), "hand: one million"),
            ("=BAHTTEXT(0)", T("ศูนย์บาทถ้วน"), "hand: zero"),
            ("=BO.DOCSO(0)", T("Không đồng"), "hand: zero"),
            (
                "=BO.DOCSO(15)",
                T("Mười lăm đồng"),
                "hand: 5 after a ten reads lăm",
            ),
            (
                "=BO.DOCSO(21)",
                T("Hai mươi mốt đồng"),
                "hand: 1 after twenty reads mốt",
            ),
            (
                "=BO.DOCSO(105)",
                T("Một trăm linh năm đồng"),
                "hand: zero tens read linh",
            ),
            (
                "=BO.DOCSO(110)",
                T("Một trăm mười đồng"),
                "hand: mười for a ten",
            ),
            (
                "=BO.DOCSO(1234567)",
                T("Một triệu hai trăm ba mươi bốn nghìn năm trăm sáu mươi bảy đồng"),
                "hand: groups of three digits",
            ),
            (
                "=BO.DOCSO(1000005)",
                T("Một triệu không trăm linh năm đồng"),
                "hand: inner group reads không trăm",
            ),
            (
                "=BO.DOCSO(2025)",
                T("Hai nghìn không trăm hai mươi lăm đồng"),
                "hand: inner group reads không trăm",
            ),
            (
                "=BO.DOCSO(1500000)",
                T("Một triệu năm trăm nghìn đồng"),
                "hand: by the reading rules",
            ),
            ("=BO.DOCSO(2000000000)", T("Hai tỷ đồng"), "hand: tỷ"),
            (
                "=BO.DOCSO(1000000000000)",
                T("Một nghìn tỷ đồng"),
                "hand: thousands of tỷ",
            ),
            (
                "=BO.DOCSO(-50000)",
                T("Âm năm mươi nghìn đồng"),
                "hand: negative amount",
            ),
            (
                "=BO.DOCSO(12.5,\"\")",
                T("Mười hai phẩy năm"),
                "hand: decimals after phẩy, no unit",
            ),
            (
                "=BO.DOCSO(1.05,\"\")",
                T("Một phẩy không năm"),
                "hand: a leading zero decimal is read",
            ),
            (
                "=BO.DOCSO(1000,\"USD\")",
                T("Một nghìn USD"),
                "hand: another unit",
            ),
            ("=BO.DOCSO(\"x\")", E("#VALUE!"), "hand: not a number"),
        ],
    );
}

#[test]
fn to_text_and_regex() {
    check(
        DATA,
        &[
            (
                "=ARRAYTOTEXT({1,\"a\";TRUE,#N/A})",
                T("1, a, TRUE, #N/A"),
                "MS-XLSX _xlfn.ARRAYTOTEXT",
            ),
            (
                "=ARRAYTOTEXT({1,\"a\";TRUE,#N/A},1)",
                T("{1,\"a\";TRUE,#N/A}"),
                "MS-XLSX _xlfn.ARRAYTOTEXT",
            ),
            (
                "=ARRAYTOTEXT({1},2)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.ARRAYTOTEXT",
            ),
            (
                "=VALUETOTEXT(\"a\",1)",
                T("\"a\""),
                "MS-XLSX _xlfn.VALUETOTEXT",
            ),
            ("=VALUETOTEXT(1.5)", T("1.5"), "MS-XLSX _xlfn.VALUETOTEXT"),
            (
                "=REGEXTEST(\"abc123\",\"[0-9]+\")",
                B(true),
                "MS-XLSX _xlfn.REGEXTEST",
            ),
            (
                "=REGEXTEST(\"ABC\",\"abc\")",
                B(false),
                "MS-XLSX _xlfn.REGEXTEST: case-sensitive by default",
            ),
            (
                "=REGEXTEST(\"ABC\",\"abc\",1)",
                B(true),
                "MS-XLSX _xlfn.REGEXTEST",
            ),
            (
                "=REGEXTEST(\"a\",\"(\")",
                E("#VALUE!"),
                "MS-XLSX _xlfn.REGEXTEST: invalid pattern",
            ),
            (
                "=REGEXEXTRACT(\"a1b22c333\",\"[0-9]+\")",
                T("1"),
                "MS-XLSX _xlfn.REGEXEXTRACT",
            ),
            (
                "=REGEXEXTRACT(\"a1b22c333\",\"[0-9]+\",1)",
                A("{\"1\";\"22\";\"333\"}"),
                "MS-XLSX _xlfn.REGEXEXTRACT: every match",
            ),
            (
                r#"=REGEXEXTRACT("John Smith","(\w+) (\w+)",2)"#,
                A("{\"John\",\"Smith\"}"),
                "MS-XLSX _xlfn.REGEXEXTRACT: groups",
            ),
            (
                "=REGEXEXTRACT(\"abc\",\"[0-9]\")",
                E("#N/A"),
                "MS-XLSX _xlfn.REGEXEXTRACT: no match",
            ),
            (
                "=REGEXREPLACE(\"a1b2\",\"[0-9]\",\"#\")",
                T("a#b#"),
                "MS-XLSX _xlfn.REGEXREPLACE",
            ),
            (
                "=REGEXREPLACE(\"a1b2\",\"[0-9]\",\"#\",2)",
                T("a1b#"),
                "MS-XLSX _xlfn.REGEXREPLACE: second match",
            ),
            (
                "=REGEXREPLACE(\"a1b2c3\",\"[0-9]\",\"#\",-1)",
                T("a1b2c#"),
                "MS-XLSX _xlfn.REGEXREPLACE: last match",
            ),
            (
                r#"=REGEXREPLACE("John Smith","(\w+) (\w+)","$2, $1")"#,
                T("Smith, John"),
                "MS-XLSX _xlfn.REGEXREPLACE: groups",
            ),
        ],
    );
}
