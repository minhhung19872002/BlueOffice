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
            ("=LEFT(\"Sale Price\",4)", T("Sale")),
            ("=LEFT(\"Sweden\")", T("S")),
            ("=LEFT(\"abc\",10)", T("abc")),
            ("=LEFT(\"abc\",0)", T("")),
            ("=LEFT(\"abc\",-1)", E("#VALUE!")),
            ("=LEFT(12345,2)", T("12")),
            ("=RIGHT(\"Sale Price\",5)", T("Price")),
            ("=RIGHT(\"Stock Number\")", T("r")),
            ("=MID(\"Fluid Flow\",1,5)", T("Fluid")),
            ("=MID(\"Fluid Flow\",7,20)", T("Flow")),
            ("=MID(\"Fluid Flow\",20,5)", T("")),
            ("=MID(\"abc\",0,1)", E("#VALUE!")),
            ("=MID(\"Tiếng Việt\",7,4)", T("Việt")),
            ("=LEN(\"Phoenix, AZ\")", N(11.0)),
            ("=LEN(\"\")", N(0.0)),
            ("=LEN(\"  One   \")", N(8.0)),
            ("=LEN(123)", N(3.0)),
            ("=LEN(\"Tiếng Việt\")", N(10.0)),
            ("=LEN(Z1)", N(0.0)),
        ],
    );
}

#[test]
fn find_search_substitute_replace() {
    check(
        DATA,
        &[
            ("=FIND(\"M\",\"Miriam McGovern\")", N(1.0)),
            ("=FIND(\"m\",\"Miriam McGovern\")", N(6.0)),
            ("=FIND(\"M\",\"Miriam McGovern\",3)", N(8.0)),
            ("=FIND(\"z\",\"abc\")", E("#VALUE!")),
            ("=FIND(\"\",\"abc\")", N(1.0)),
            ("=FIND(\"a\",\"abc\",5)", E("#VALUE!")),
            ("=SEARCH(\"e\",\"Statements\",6)", N(7.0)),
            ("=SEARCH(\"margin\",\"Profit Margin\")", N(8.0)),
            ("=SEARCH(\"p?o\",\"Profit\")", N(1.0)),
            ("=SEARCH(\"f*t\",\"Profit\")", N(4.0)),
            ("=SEARCH(\"~*\",\"a*b\")", N(2.0)),
            ("=SEARCH(\"x\",\"abc\")", E("#VALUE!")),
            (
                "=SUBSTITUTE(\"Sales Data\",\"Sales\",\"Cost\")",
                T("Cost Data"),
            ),
            (
                "=SUBSTITUTE(\"Quarter 1, 2008\",\"1\",\"2\",1)",
                T("Quarter 2, 2008"),
            ),
            (
                "=SUBSTITUTE(\"Quarter 1, 2011\",\"1\",\"2\",3)",
                T("Quarter 1, 2012"),
            ),
            ("=SUBSTITUTE(\"aaa\",\"a\",\"b\")", T("bbb")),
            ("=SUBSTITUTE(\"abc\",\"\",\"x\")", T("abc")),
            ("=SUBSTITUTE(\"abc\",\"b\",\"x\",0)", E("#VALUE!")),
            ("=REPLACE(\"abcdefghijk\",6,5,\"*\")", T("abcde*k")),
            ("=REPLACE(\"2009\",3,2,\"10\")", T("2010")),
            ("=REPLACE(\"123456\",1,3,\"@\")", T("@456")),
            ("=REPLACE(\"abc\",0,1,\"x\")", E("#VALUE!")),
        ],
    );
}

#[test]
fn case_trim_clean() {
    check(
        DATA,
        &[
            ("=UPPER(\"total\")", T("TOTAL")),
            ("=UPPER(\"việt\")", T("VIỆT")),
            ("=LOWER(\"E. E. Cummings\")", T("e. e. cummings")),
            ("=PROPER(\"this is a TITLE\")", T("This Is A Title")),
            ("=PROPER(\"2-way street\")", T("2-Way Street")),
            ("=PROPER(\"76BudGet\")", T("76Budget")),
            (
                "=TRIM(\" First Quarter Earnings \")",
                T("First Quarter Earnings"),
            ),
            ("=TRIM(\"  a   b  \")", T("a b")),
            (
                "=CLEAN(CHAR(9)&\"Monthly report\"&CHAR(10))",
                T("Monthly report"),
            ),
            ("=EXACT(\"word\",\"word\")", B(true)),
            ("=EXACT(\"Word\",\"word\")", B(false)),
            ("=EXACT(\"w ord\",\"word\")", B(false)),
            ("=T(\"Rainfall\")", T("Rainfall")),
            ("=T(19)", T("")),
            ("=T(TRUE)", T("")),
        ],
    );
}

#[test]
fn concat_join_split() {
    check(
        DATA,
        &[
            ("=CONCAT(\"a\",\"b\",1)", T("ab1")),
            ("=CONCAT(A1:A3)", T("abc")),
            ("=CONCAT(A1:A3,\"!\")", T("abc!")),
            ("=CONCATENATE(\"a\",1,TRUE)", T("a1TRUE")),
            ("=1&2", T("12")),
            ("=\"a\"&TRUE", T("aTRUE")),
            ("=(0.1+0.2)&\"\"", T("0.3")),
            ("=TEXTJOIN(\", \",TRUE,\"a\",\"\",\"b\")", T("a, b")),
            ("=TEXTJOIN(\", \",FALSE,\"a\",\"\",\"b\")", T("a, , b")),
            ("=TEXTJOIN(\"-\",TRUE,{1,2,3})", T("1-2-3")),
            ("=TEXTJOIN(\"\",TRUE,A1:A3,Z1)", T("abc")),
            ("=TEXTSPLIT(\"a,b,c\",\",\")", A("{\"a\",\"b\",\"c\"}")),
            (
                "=TEXTSPLIT(\"a,b;c,d\",\",\",\";\")",
                A("{\"a\",\"b\";\"c\",\"d\"}"),
            ),
            ("=TEXTSPLIT(\"a,,b\",\",\",,TRUE)", A("{\"a\",\"b\"}")),
            (
                "=TEXTSPLIT(\"a,b;c\",\",\",\";\")",
                A("{\"a\",\"b\";\"c\",#N/A}"),
            ),
            (
                "=TEXTSPLIT(\"a,b;c\",\",\",\";\",,,\"-\")",
                A("{\"a\",\"b\";\"c\",\"-\"}"),
            ),
            ("=TEXTSPLIT(\"aXbxc\",\"x\",,,1)", A("{\"a\",\"b\",\"c\"}")),
            (
                "=TEXTBEFORE(\"Red riding hood\",\"hood\")",
                T("Red riding "),
            ),
            ("=TEXTBEFORE(\"a-b-c\",\"-\",2)", T("a-b")),
            ("=TEXTBEFORE(\"a-b-c\",\"-\",-1)", T("a-b")),
            ("=TEXTBEFORE(\"abc\",\"x\")", E("#N/A")),
            ("=TEXTBEFORE(\"abc\",\"x\",,,,\"none\")", T("none")),
            ("=TEXTAFTER(\"Red riding hood\",\"riding \")", T("hood")),
            ("=TEXTAFTER(\"a-b-c\",\"-\")", T("b-c")),
            ("=TEXTAFTER(\"a-b-c\",\"-\",-1)", T("c")),
            ("=TEXTAFTER(\"aXb\",\"x\",1,1)", T("b")),
            ("=TEXTAFTER(\"aXb\",\"x\")", E("#N/A")),
            ("=TEXTAFTER(\"a-b\",\"-\",0)", E("#VALUE!")),
        ],
    );
}

#[test]
fn rept_char_code() {
    check(
        DATA,
        &[
            ("=REPT(\"*-\",3)", T("*-*-*-")),
            ("=REPT(\"a\",0)", T("")),
            ("=REPT(\"a\",-1)", E("#VALUE!")),
            ("=CHAR(65)", T("A")),
            ("=CHAR(33)", T("!")),
            ("=CHAR(0)", E("#VALUE!")),
            ("=CHAR(256)", E("#VALUE!")),
            ("=CODE(\"A\")", N(65.0)),
            ("=CODE(\"!\")", N(33.0)),
            ("=CODE(\"Alphabet\")", N(65.0)),
            ("=CODE(\"\")", E("#VALUE!")),
            ("=UNICHAR(66)", T("B")),
            ("=UNICHAR(32)", T(" ")),
            ("=UNICHAR(7879)", T("ệ")),
            ("=UNICHAR(0)", E("#VALUE!")),
            ("=UNICODE(\"B\")", N(66.0)),
            ("=UNICODE(\"ệ\")", N(7879.0)),
            ("=UNICODE(\"\")", E("#VALUE!")),
        ],
    );
}

#[test]
fn value_numbervalue_text_fixed() {
    check(
        DATA,
        &[
            ("=VALUE(\"1,000\")", N(1000.0)),
            ("=VALUE(\"50%\")", N(0.5)),
            ("=VALUE(\"1e3\")", N(1000.0)),
            ("=VALUE(\"16:48:00\")-VALUE(\"12:00:00\")", Near(0.2, 1e-12)),
            ("=VALUE(\"abc\")", E("#VALUE!")),
            ("=VALUE(\" 12 \")", N(12.0)),
            ("=NUMBERVALUE(\"2.500,27\",\",\",\".\")", N(2500.27)),
            ("=NUMBERVALUE(\"3.5%\")", N(0.035)),
            ("=NUMBERVALUE(\"1 000\")", N(1000.0)),
            ("=NUMBERVALUE(\"1.2.3\")", E("#VALUE!")),
            ("=NUMBERVALUE(\"\")", N(0.0)),
            ("=TEXT(1234.567,\"$#,##0.00\")", T("$1,234.57")),
            ("=TEXT(0.285,\"0.0%\")", T("28.5%")),
            ("=TEXT(45366,\"yyyy-mm-dd\")", T("2024-03-15")),
            ("=TEXT(45366,\"dd/mm/yyyy\")", T("15/03/2024")),
            ("=TEXT(45366,\"dddd\")", T("Friday")),
            ("=TEXT(45366,\"mmm\")", T("Mar")),
            ("=TEXT(4.34,\"# ?/?\")", T("4 1/3")),
            ("=TEXT(12200000,\"0.00E+00\")", T("1.22E+07")),
            ("=TEXT(1234,\"0000000\")", T("0001234")),
            ("=TEXT(0.5,\"h:mm AM/PM\")", T("12:00 PM")),
            ("=TEXT(-5,\"0;(0)\")", T("(5)")),
            ("=TEXT(\"abc\",\"@\")", T("abc")),
            ("=TEXT(1234.5,\"#,##0\")", T("1,235")),
            ("=FIXED(1234.567,1)", T("1,234.6")),
            ("=FIXED(1234.567,-1)", T("1,230")),
            ("=FIXED(-1234.567,-1,TRUE)", T("-1230")),
            ("=FIXED(44.332)", T("44.33")),
        ],
    );
}
