//! Text functions.
//!
//! Sources: ECMA-376 Part 1, §18.17.7 (LEFT, RIGHT, MID, LEN, FIND, SEARCH,
//! SUBSTITUTE, REPLACE, UPPER, LOWER, PROPER, TRIM, CLEAN, TEXT, VALUE,
//! CONCATENATE, REPT, EXACT, CHAR, CODE, T, FIXED) with their worked examples;
//! OpenFormula (ODF 1.3 Part 4) Text functions (UNICHAR, UNICODE, NUMBERVALUE,
//! TEXTJOIN, CONCAT); TEXTSPLIT / TEXTBEFORE / TEXTAFTER follow their published
//! function definitions. TEXT format codes follow ECMA-376 Part 1, §18.8.31 numFmt.
//! Character positions count Unicode scalar values.

mod common;
use common::*;

const DATA: &[(&str, &str)] = &[("A1", "a"), ("A2", "b"), ("A3", "c")];

#[test]
fn substrings() {
    check(
        DATA,
        &[
            (
                "=LEFT(\"Sale Price\",4)",
                T("Sale"),
                "ECMA-376 §18.17.7 LEFT",
            ),
            ("=LEFT(\"Sweden\")", T("S"), "ECMA-376 §18.17.7 LEFT"),
            ("=LEFT(\"abc\",10)", T("abc"), "ECMA-376 §18.17.7 LEFT"),
            ("=LEFT(\"abc\",0)", T(""), "ECMA-376 §18.17.7 LEFT"),
            ("=LEFT(\"abc\",-1)", E("#VALUE!"), "ECMA-376 §18.17.7 LEFT"),
            ("=LEFT(12345,2)", T("12"), "ECMA-376 §18.17.7 LEFT"),
            (
                "=RIGHT(\"Sale Price\",5)",
                T("Price"),
                "ECMA-376 §18.17.7 RIGHT",
            ),
            (
                "=RIGHT(\"Stock Number\")",
                T("r"),
                "ECMA-376 §18.17.7 RIGHT",
            ),
            (
                "=MID(\"Fluid Flow\",1,5)",
                T("Fluid"),
                "ECMA-376 §18.17.7 MID",
            ),
            (
                "=MID(\"Fluid Flow\",7,20)",
                T("Flow"),
                "ECMA-376 §18.17.7 MID",
            ),
            ("=MID(\"Fluid Flow\",20,5)", T(""), "ECMA-376 §18.17.7 MID"),
            ("=MID(\"abc\",0,1)", E("#VALUE!"), "ECMA-376 §18.17.7 MID"),
            (
                "=MID(\"Tiếng Việt\",7,4)",
                T("Việt"),
                "ECMA-376 §18.17.7 MID",
            ),
            ("=LEN(\"Phoenix, AZ\")", N(11.0), "ECMA-376 §18.17.7 LEN"),
            ("=LEN(\"\")", N(0.0), "ECMA-376 §18.17.7 LEN"),
            ("=LEN(\"  One   \")", N(8.0), "ECMA-376 §18.17.7 LEN"),
            ("=LEN(123)", N(3.0), "ECMA-376 §18.17.7 LEN"),
            ("=LEN(\"Tiếng Việt\")", N(10.0), "ECMA-376 §18.17.7 LEN"),
            ("=LEN(Z1)", N(0.0), "ECMA-376 §18.17.7 LEN"),
        ],
    );
}

#[test]
fn find_search_substitute_replace() {
    check(
        DATA,
        &[
            (
                "=FIND(\"M\",\"Miriam McGovern\")",
                N(1.0),
                "ECMA-376 §18.17.7 FIND",
            ),
            (
                "=FIND(\"m\",\"Miriam McGovern\")",
                N(6.0),
                "ECMA-376 §18.17.7 FIND",
            ),
            (
                "=FIND(\"M\",\"Miriam McGovern\",3)",
                N(8.0),
                "ECMA-376 §18.17.7 FIND",
            ),
            (
                "=FIND(\"z\",\"abc\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 FIND",
            ),
            ("=FIND(\"\",\"abc\")", N(1.0), "ECMA-376 §18.17.7 FIND"),
            (
                "=FIND(\"a\",\"abc\",5)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 FIND",
            ),
            (
                "=SEARCH(\"e\",\"Statements\",6)",
                N(7.0),
                "ECMA-376 §18.17.7 SEARCH",
            ),
            (
                "=SEARCH(\"margin\",\"Profit Margin\")",
                N(8.0),
                "ECMA-376 §18.17.7 SEARCH",
            ),
            (
                "=SEARCH(\"p?o\",\"Profit\")",
                N(1.0),
                "ECMA-376 §18.17.7 SEARCH",
            ),
            (
                "=SEARCH(\"f*t\",\"Profit\")",
                N(4.0),
                "ECMA-376 §18.17.7 SEARCH",
            ),
            (
                "=SEARCH(\"~*\",\"a*b\")",
                N(2.0),
                "ECMA-376 §18.17.7 SEARCH",
            ),
            (
                "=SEARCH(\"x\",\"abc\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 SEARCH",
            ),
            (
                "=SUBSTITUTE(\"Sales Data\",\"Sales\",\"Cost\")",
                T("Cost Data"),
                "ECMA-376 §18.17.7 SUBSTITUTE",
            ),
            (
                "=SUBSTITUTE(\"Quarter 1, 2008\",\"1\",\"2\",1)",
                T("Quarter 2, 2008"),
                "ECMA-376 §18.17.7 SUBSTITUTE",
            ),
            (
                "=SUBSTITUTE(\"Quarter 1, 2011\",\"1\",\"2\",3)",
                T("Quarter 1, 2012"),
                "ECMA-376 §18.17.7 SUBSTITUTE",
            ),
            (
                "=SUBSTITUTE(\"aaa\",\"a\",\"b\")",
                T("bbb"),
                "ECMA-376 §18.17.7 SUBSTITUTE",
            ),
            (
                "=SUBSTITUTE(\"abc\",\"\",\"x\")",
                T("abc"),
                "ECMA-376 §18.17.7 SUBSTITUTE",
            ),
            (
                "=SUBSTITUTE(\"abc\",\"b\",\"x\",0)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 SUBSTITUTE",
            ),
            (
                "=REPLACE(\"abcdefghijk\",6,5,\"*\")",
                T("abcde*k"),
                "ECMA-376 §18.17.7 REPLACE",
            ),
            (
                "=REPLACE(\"2009\",3,2,\"10\")",
                T("2010"),
                "ECMA-376 §18.17.7 REPLACE",
            ),
            (
                "=REPLACE(\"123456\",1,3,\"@\")",
                T("@456"),
                "ECMA-376 §18.17.7 REPLACE",
            ),
            (
                "=REPLACE(\"abc\",0,1,\"x\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 REPLACE",
            ),
        ],
    );
}

#[test]
fn case_trim_clean() {
    check(
        DATA,
        &[
            ("=UPPER(\"total\")", T("TOTAL"), "ECMA-376 §18.17.7 UPPER"),
            ("=UPPER(\"việt\")", T("VIỆT"), "ECMA-376 §18.17.7 UPPER"),
            (
                "=LOWER(\"E. E. Cummings\")",
                T("e. e. cummings"),
                "ECMA-376 §18.17.7 LOWER",
            ),
            (
                "=PROPER(\"this is a TITLE\")",
                T("This Is A Title"),
                "ECMA-376 §18.17.7 PROPER",
            ),
            (
                "=PROPER(\"2-way street\")",
                T("2-Way Street"),
                "ECMA-376 §18.17.7 PROPER",
            ),
            (
                "=PROPER(\"76BudGet\")",
                T("76Budget"),
                "ECMA-376 §18.17.7 PROPER",
            ),
            (
                "=TRIM(\" First Quarter Earnings \")",
                T("First Quarter Earnings"),
                "ECMA-376 §18.17.7 TRIM",
            ),
            ("=TRIM(\"  a   b  \")", T("a b"), "ECMA-376 §18.17.7 TRIM"),
            (
                "=CLEAN(CHAR(9)&\"Monthly report\"&CHAR(10))",
                T("Monthly report"),
                "ECMA-376 §18.17.7 CLEAN",
            ),
            (
                "=EXACT(\"word\",\"word\")",
                B(true),
                "ECMA-376 §18.17.7 EXACT",
            ),
            (
                "=EXACT(\"Word\",\"word\")",
                B(false),
                "ECMA-376 §18.17.7 EXACT",
            ),
            (
                "=EXACT(\"w ord\",\"word\")",
                B(false),
                "ECMA-376 §18.17.7 EXACT",
            ),
            ("=T(\"Rainfall\")", T("Rainfall"), "ECMA-376 §18.17.7 T"),
            ("=T(19)", T(""), "ECMA-376 §18.17.7 T"),
            ("=T(TRUE)", T(""), "ECMA-376 §18.17.7 T"),
        ],
    );
}

#[test]
fn concat_join_split() {
    check(
        DATA,
        &[
            ("=CONCAT(\"a\",\"b\",1)", T("ab1"), "MS-XLSX _xlfn.CONCAT"),
            ("=CONCAT(A1:A3)", T("abc"), "MS-XLSX _xlfn.CONCAT"),
            ("=CONCAT(A1:A3,\"!\")", T("abc!"), "MS-XLSX _xlfn.CONCAT"),
            (
                "=CONCATENATE(\"a\",1,TRUE)",
                T("a1TRUE"),
                "ECMA-376 §18.17.7 CONCATENATE",
            ),
            ("=1&2", T("12"), "ECMA-376 §18.17.2 formula syntax"),
            (
                "=\"a\"&TRUE",
                T("aTRUE"),
                "ECMA-376 §18.17.2 formula syntax",
            ),
            (
                "=(0.1+0.2)&\"\"",
                T("0.3"),
                "ECMA-376 §18.17.2 formula syntax",
            ),
            (
                "=TEXTJOIN(\", \",TRUE,\"a\",\"\",\"b\")",
                T("a, b"),
                "MS-XLSX _xlfn.TEXTJOIN",
            ),
            (
                "=TEXTJOIN(\", \",FALSE,\"a\",\"\",\"b\")",
                T("a, , b"),
                "MS-XLSX _xlfn.TEXTJOIN",
            ),
            (
                "=TEXTJOIN(\"-\",TRUE,{1,2,3})",
                T("1-2-3"),
                "MS-XLSX _xlfn.TEXTJOIN",
            ),
            (
                "=TEXTJOIN(\"\",TRUE,A1:A3,Z1)",
                T("abc"),
                "MS-XLSX _xlfn.TEXTJOIN",
            ),
            (
                "=TEXTSPLIT(\"a,b,c\",\",\")",
                A("{\"a\",\"b\",\"c\"}"),
                "MS-XLSX _xlfn.TEXTSPLIT",
            ),
            (
                "=TEXTSPLIT(\"a,b;c,d\",\",\",\";\")",
                A("{\"a\",\"b\";\"c\",\"d\"}"),
                "MS-XLSX _xlfn.TEXTSPLIT",
            ),
            (
                "=TEXTSPLIT(\"a,,b\",\",\",,TRUE)",
                A("{\"a\",\"b\"}"),
                "MS-XLSX _xlfn.TEXTSPLIT",
            ),
            (
                "=TEXTSPLIT(\"a,b;c\",\",\",\";\")",
                A("{\"a\",\"b\";\"c\",#N/A}"),
                "MS-XLSX _xlfn.TEXTSPLIT",
            ),
            (
                "=TEXTSPLIT(\"a,b;c\",\",\",\";\",,,\"-\")",
                A("{\"a\",\"b\";\"c\",\"-\"}"),
                "MS-XLSX _xlfn.TEXTSPLIT",
            ),
            (
                "=TEXTSPLIT(\"aXbxc\",\"x\",,,1)",
                A("{\"a\",\"b\",\"c\"}"),
                "MS-XLSX _xlfn.TEXTSPLIT",
            ),
            (
                "=TEXTBEFORE(\"Red riding hood\",\"hood\")",
                T("Red riding "),
                "MS-XLSX _xlfn.TEXTBEFORE",
            ),
            (
                "=TEXTBEFORE(\"a-b-c\",\"-\",2)",
                T("a-b"),
                "MS-XLSX _xlfn.TEXTBEFORE",
            ),
            (
                "=TEXTBEFORE(\"a-b-c\",\"-\",-1)",
                T("a-b"),
                "MS-XLSX _xlfn.TEXTBEFORE",
            ),
            (
                "=TEXTBEFORE(\"abc\",\"x\")",
                E("#N/A"),
                "MS-XLSX _xlfn.TEXTBEFORE",
            ),
            (
                "=TEXTBEFORE(\"abc\",\"x\",,,,\"none\")",
                T("none"),
                "MS-XLSX _xlfn.TEXTBEFORE",
            ),
            (
                "=TEXTAFTER(\"Red riding hood\",\"riding \")",
                T("hood"),
                "MS-XLSX _xlfn.TEXTAFTER",
            ),
            (
                "=TEXTAFTER(\"a-b-c\",\"-\")",
                T("b-c"),
                "MS-XLSX _xlfn.TEXTAFTER",
            ),
            (
                "=TEXTAFTER(\"a-b-c\",\"-\",-1)",
                T("c"),
                "MS-XLSX _xlfn.TEXTAFTER",
            ),
            (
                "=TEXTAFTER(\"aXb\",\"x\",1,1)",
                T("b"),
                "MS-XLSX _xlfn.TEXTAFTER",
            ),
            (
                "=TEXTAFTER(\"aXb\",\"x\")",
                E("#N/A"),
                "MS-XLSX _xlfn.TEXTAFTER",
            ),
            (
                "=TEXTAFTER(\"a-b\",\"-\",0)",
                E("#VALUE!"),
                "MS-XLSX _xlfn.TEXTAFTER",
            ),
        ],
    );
}

#[test]
fn rept_char_code() {
    check(
        DATA,
        &[
            ("=REPT(\"*-\",3)", T("*-*-*-"), "ECMA-376 §18.17.7 REPT"),
            ("=REPT(\"a\",0)", T(""), "ECMA-376 §18.17.7 REPT"),
            ("=REPT(\"a\",-1)", E("#VALUE!"), "ECMA-376 §18.17.7 REPT"),
            ("=CHAR(65)", T("A"), "ECMA-376 §18.17.7 CHAR"),
            ("=CHAR(33)", T("!"), "ECMA-376 §18.17.7 CHAR"),
            ("=CHAR(0)", E("#VALUE!"), "ECMA-376 §18.17.7 CHAR"),
            ("=CHAR(256)", E("#VALUE!"), "ECMA-376 §18.17.7 CHAR"),
            ("=CODE(\"A\")", N(65.0), "ECMA-376 §18.17.7 CODE"),
            ("=CODE(\"!\")", N(33.0), "ECMA-376 §18.17.7 CODE"),
            ("=CODE(\"Alphabet\")", N(65.0), "ECMA-376 §18.17.7 CODE"),
            ("=CODE(\"\")", E("#VALUE!"), "ECMA-376 §18.17.7 CODE"),
            ("=UNICHAR(66)", T("B"), "OpenFormula UNICHAR"),
            ("=UNICHAR(32)", T(" "), "OpenFormula UNICHAR"),
            ("=UNICHAR(7879)", T("ệ"), "OpenFormula UNICHAR"),
            ("=UNICHAR(0)", E("#VALUE!"), "OpenFormula UNICHAR"),
            ("=UNICODE(\"B\")", N(66.0), "OpenFormula UNICODE"),
            ("=UNICODE(\"ệ\")", N(7879.0), "OpenFormula UNICODE"),
            ("=UNICODE(\"\")", E("#VALUE!"), "OpenFormula UNICODE"),
        ],
    );
}

#[test]
fn value_numbervalue_text_fixed() {
    check(
        DATA,
        &[
            ("=VALUE(\"1,000\")", N(1000.0), "ECMA-376 §18.17.7 VALUE"),
            ("=VALUE(\"50%\")", N(0.5), "ECMA-376 §18.17.7 VALUE"),
            ("=VALUE(\"1e3\")", N(1000.0), "ECMA-376 §18.17.7 VALUE"),
            (
                "=VALUE(\"16:48:00\")-VALUE(\"12:00:00\")",
                Near(0.2, 1e-12),
                "ECMA-376 §18.17.7 VALUE",
            ),
            ("=VALUE(\"abc\")", E("#VALUE!"), "ECMA-376 §18.17.7 VALUE"),
            ("=VALUE(\" 12 \")", N(12.0), "ECMA-376 §18.17.7 VALUE"),
            (
                "=NUMBERVALUE(\"2.500,27\",\",\",\".\")",
                N(2500.27),
                "OpenFormula NUMBERVALUE",
            ),
            (
                "=NUMBERVALUE(\"3.5%\")",
                N(0.035),
                "OpenFormula NUMBERVALUE",
            ),
            (
                "=NUMBERVALUE(\"1 000\")",
                N(1000.0),
                "OpenFormula NUMBERVALUE",
            ),
            (
                "=NUMBERVALUE(\"1.2.3\")",
                E("#VALUE!"),
                "OpenFormula NUMBERVALUE",
            ),
            ("=NUMBERVALUE(\"\")", N(0.0), "OpenFormula NUMBERVALUE"),
            (
                "=TEXT(1234.567,\"$#,##0.00\")",
                T("$1,234.57"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            (
                "=TEXT(0.285,\"0.0%\")",
                T("28.5%"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            (
                "=TEXT(45366,\"yyyy-mm-dd\")",
                T("2024-03-15"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            (
                "=TEXT(45366,\"dd/mm/yyyy\")",
                T("15/03/2024"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            (
                "=TEXT(45366,\"dddd\")",
                T("Friday"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            ("=TEXT(45366,\"mmm\")", T("Mar"), "ECMA-376 §18.17.7 TEXT"),
            (
                "=TEXT(4.34,\"# ?/?\")",
                T("4 1/3"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            (
                "=TEXT(12200000,\"0.00E+00\")",
                T("1.22E+07"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            (
                "=TEXT(1234,\"0000000\")",
                T("0001234"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            (
                "=TEXT(0.5,\"h:mm AM/PM\")",
                T("12:00 PM"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            ("=TEXT(-5,\"0;(0)\")", T("(5)"), "ECMA-376 §18.17.7 TEXT"),
            ("=TEXT(\"abc\",\"@\")", T("abc"), "ECMA-376 §18.17.7 TEXT"),
            (
                "=TEXT(1234.5,\"#,##0\")",
                T("1,235"),
                "ECMA-376 §18.17.7 TEXT",
            ),
            (
                "=FIXED(1234.567,1)",
                T("1,234.6"),
                "ECMA-376 §18.17.7 FIXED",
            ),
            ("=FIXED(1234.567,-1)", T("1,230"), "ECMA-376 §18.17.7 FIXED"),
            (
                "=FIXED(-1234.567,-1,TRUE)",
                T("-1230"),
                "ECMA-376 §18.17.7 FIXED",
            ),
            ("=FIXED(44.332)", T("44.33"), "ECMA-376 §18.17.7 FIXED"),
        ],
    );
}

/// Additional value, edge and error rows for the Text list (gap #37).
#[test]
fn text_edge_cases() {
    check(
        DATA,
        &[
            ("=RIGHT(\"abc\",0)", T(""), "hand: zero characters"),
            (
                "=RIGHT(\"abc\",5)",
                T("abc"),
                "hand: more than the length → whole text",
            ),
            (
                "=RIGHT(\"abc\",-1)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 RIGHT; hand: negative count",
            ),
            (
                "=RIGHT(\"Việt\",2)",
                T("ệt"),
                "hand: counts characters, not bytes",
            ),
            ("=UPPER(12)", T("12"), "hand: number converted to text"),
            ("=UPPER(TRUE)", T("TRUE"), "hand: logical converted to text"),
            ("=UPPER(\"\")", T(""), "hand: empty text"),
            (
                "=LOWER(\"ĐÀ NẴNG\")",
                T("đà nẵng"),
                "hand: Unicode lower case",
            ),
            ("=LOWER(\"\")", T(""), "hand: empty text"),
            (
                "=LOWER(1/0)",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 LOWER; hand: error propagates",
            ),
            (
                "=PROPER(\"nguyễn văn an\")",
                T("Nguyễn Văn An"),
                "hand: capitalise after non-letters",
            ),
            ("=PROPER(\"\")", T(""), "hand: empty text"),
            ("=TRIM(\"\")", T(""), "hand: empty text"),
            ("=TRIM(\"   \")", T(""), "hand: only spaces"),
            (
                "=TRIM(\"a\"&CHAR(9)&\"b\")",
                T("a\tb"),
                "ECMA-376 §18.17.7 TRIM; hand: only ASCII space (32) is trimmed",
            ),
            ("=CLEAN(\"abc\")", T("abc"), "hand: nothing to remove"),
            (
                "=CLEAN(CHAR(7)&\"x\"&CHAR(31))",
                T("x"),
                "ECMA-376 §18.17.7 CLEAN; hand: codes 0-31 removed",
            ),
            (
                "=LEN(CLEAN(CHAR(32)))",
                N(1.0),
                "hand: space (32) is printable",
            ),
            (
                "=CONCAT({1,2;3,4})",
                T("1234"),
                "MS-XLSX _xlfn.CONCAT; hand: row-major order",
            ),
            (
                "=CONCAT(\"a\",#N/A)",
                E("#N/A"),
                "MS-XLSX _xlfn.CONCAT; hand: error propagates",
            ),
            (
                "=CONCAT(Z1:Z3)",
                T(""),
                "MS-XLSX _xlfn.CONCAT; hand: blanks are empty text",
            ),
            (
                "=CONCATENATE(\"Stream population for \",\"brook trout\",\" is \",32,\"/mile\")",
                T("Stream population for brook trout is 32/mile"),
                "hand: pieces joined in order",
            ),
            (
                "=CONCATENATE(\"a\",1/0)",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 CONCATENATE; hand: error propagates",
            ),
            ("=CONCATENATE(\"\")", T(""), "hand: empty text"),
            (
                "=REPT(\"ab\",2.9)",
                T("abab"),
                "ECMA-376 §18.17.7 REPT; hand: count truncated to 2",
            ),
            ("=REPT(\"\",5)", T(""), "hand: repeating empty text"),
            ("=EXACT(\"\",\"\")", B(true), "hand: both empty"),
            (
                "=EXACT(1,\"1\")",
                B(true),
                "ECMA-376 §18.17.7 EXACT; hand: arguments compared as text",
            ),
            (
                "=UNICODE(\"ab\")",
                N(97.0),
                "OpenFormula UNICODE; hand: first character a = U+0061",
            ),
            ("=UNICODE(\"€\")", N(8364.0), "hand: € = U+20AC"),
            ("=UNICHAR(8364)", T("€"), "hand: U+20AC"),
            (
                "=UNICHAR(1114112)",
                E("#VALUE!"),
                "OpenFormula UNICHAR; hand: beyond U+10FFFF",
            ),
            (
                "=CHAR(\"a\")",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 CHAR; hand: non-numeric text",
            ),
            ("=CODE(1)", N(49.0), "hand: \"1\" = 49"),
            ("=LEN(TRUE)", N(4.0), "hand: \"TRUE\""),
            (
                "=MID(\"abc\",2,-1)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 MID; hand: negative length",
            ),
            (
                "=FIND(\"b\",\"abc\",0)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 FIND; hand: start < 1",
            ),
            ("=SEARCH(\"B\",\"abc\")", N(2.0), "hand: case-insensitive"),
            (
                "=SUBSTITUTE(\"a-b-c\",\"-\",\"\")",
                T("abc"),
                "hand: remove all",
            ),
            (
                "=REPLACE(\"abc\",4,0,\"d\")",
                T("abcd"),
                "hand: insert after the end",
            ),
            (
                "=VALUE(TRUE)",
                E("#VALUE!"),
                "ECMA-376 §18.17.7 VALUE; hand: logical is not text",
            ),
            (
                "=NUMBERVALUE(\"1,5\",\",\",\".\")",
                N(1.5),
                "OpenFormula NUMBERVALUE; hand: comma as decimal",
            ),
            (
                "=TEXTJOIN(\",\",TRUE,Z1:Z3)",
                T(""),
                "MS-XLSX _xlfn.TEXTJOIN; hand: all blanks skipped",
            ),
            (
                "=TEXTSPLIT(\"abc\",\",\")",
                T("abc"),
                "MS-XLSX _xlfn.TEXTSPLIT; hand: no delimiter → whole text",
            ),
            (
                "=TEXTBEFORE(\"a-b\",\"-\",3)",
                E("#N/A"),
                "MS-XLSX _xlfn.TEXTBEFORE; hand: fewer occurrences",
            ),
            (
                "=TEXTAFTER(\"a-b\",\"\")",
                T("a-b"),
                "MS-XLSX _xlfn.TEXTAFTER; hand: empty delimiter matches at start",
            ),
            (
                "=TEXT(1234.5,\"\")",
                T(""),
                "ECMA-376 §18.17.7 TEXT; hand: empty format code",
            ),
            (
                "=TEXT(1/0,\"0\")",
                E("#DIV/0!"),
                "ECMA-376 §18.17.7 TEXT; hand: error propagates",
            ),
            (
                "=TEXT(\"12\",\"0.0\")",
                T("12.0"),
                "ECMA-376 §18.17.7 TEXT; hand: numeric text converted",
            ),
            (
                "=TEXT(0.25,\"0%\")",
                T("25%"),
                "ECMA-376 §18.8.31 numFmt; hand: 0.25·100",
            ),
            (
                "=TEXT(-1234.5,\"#,##0.0\")",
                T("-1,234.5"),
                "ECMA-376 §18.8.31 numFmt; hand: thousands separator, sign",
            ),
            (
                "=TEXT(0,\"0;-0;\"\"zero\"\"\")",
                T("zero"),
                "ECMA-376 §18.8.31 numFmt; hand: third section for 0",
            ),
        ],
    );
}

/// `TEXT` uses the number-format renderer installed by the host (BlueSheet
/// installs its full engine from `bo-sheet`); `None` from it means `#VALUE!`.
#[test]
fn text_uses_installed_formatter() {
    fn fake(
        input: bo_formula::FormatInput<'_>,
        code: &str,
        _: bo_formula::DateSystem,
    ) -> Option<String> {
        match input {
            bo_formula::FormatInput::Number(n) if n < 0.0 => None,
            bo_formula::FormatInput::Number(n) => Some(format!("<{n}|{code}>")),
            bo_formula::FormatInput::Text(s) => Some(format!("<'{s}|{code}>")),
        }
    }
    let mut wb = workbook(
        &[
            ("A1", "=TEXT(2,\"0.0\")"),
            ("A2", "=TEXT(\"ab\",\"@\")"),
            ("A3", "=TEXT(-1,\"0\")"),
        ],
        Opts::default(),
    );
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A1"), bo_formula::Value::Text("2.0".into()));
    wb.set_text_formatter(Some(fake));
    wb.recalculate();
    assert_eq!(
        wb.value_a1(0, "A1"),
        bo_formula::Value::Text("<2|0.0>".into())
    );
    assert_eq!(
        wb.value_a1(0, "A2"),
        bo_formula::Value::Text("<'ab|@>".into())
    );
    assert_eq!(
        wb.value_a1(0, "A3"),
        bo_formula::Value::Error(bo_formula::ErrorKind::Value)
    );
    wb.set_text_formatter(None);
    wb.recalculate();
    assert_eq!(wb.value_a1(0, "A3"), bo_formula::Value::Text("-1".into()));
}
