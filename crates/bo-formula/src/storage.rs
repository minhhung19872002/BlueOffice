//! Formula text as stored in files versus as shown to people.
//!
//! [MS-XLSX] 2.2.2 (Formulas): functions added after the first edition of ECMA-376 are written
//! with the `_xlfn.` prefix (`_xlfn._xlws.` for FILTER and SORT), and the parameter names of
//! `LET` / `LAMBDA` with `_xlpm.`, so older consumers recognise them as unknown rather than as
//! defined names. [`to_storage`] adds the prefixes, [`from_storage`] removes them; both work on
//! the text and keep everything else (spacing, references, strings) as written.

use std::collections::HashSet;

use crate::functions::function_names;

/// Functions of ECMA-376 Part 1 (first edition), §18.17.7 — stored without a prefix.
const ECMA_2006: &str = "ABS ACCRINT ACCRINTM ACOS ACOSH ADDRESS AMORDEGRC AMORLINC AND AREAS ASC ASIN \
ASINH ATAN ATAN2 ATANH AVEDEV AVERAGE AVERAGEA AVERAGEIF AVERAGEIFS BAHTTEXT BESSELI BESSELJ BESSELK \
BESSELY BETADIST BETAINV BIN2DEC BIN2HEX BIN2OCT BINOMDIST CALL CEILING CELL CHAR CHIDIST CHIINV CHITEST \
CHOOSE CLEAN CODE COLUMN COLUMNS COMBIN COMPLEX CONCATENATE CONFIDENCE CONVERT CORREL COS COSH COUNT \
COUNTA COUNTBLANK COUNTIF COUNTIFS COUPDAYBS COUPDAYS COUPDAYSNC COUPNCD COUPNUM COUPPCD COVAR CRITBINOM \
CUBEKPIMEMBER CUBEMEMBER CUBEMEMBERPROPERTY CUBERANKEDMEMBER CUBESET CUBESETCOUNT CUBEVALUE CUMIPMT \
CUMPRINC DATE DATEDIF DATEVALUE DAVERAGE DAY DAYS360 DB DBCS DCOUNT DCOUNTA DDB DEC2BIN DEC2HEX DEC2OCT \
DEGREES DELTA DEVSQ DGET DISC DMAX DMIN DOLLAR DOLLARDE DOLLARFR DPRODUCT DSTDEV DSTDEVP DSUM DURATION \
DVAR DVARP EDATE EFFECT EOMONTH ERF ERFC ERROR.TYPE EVEN EXACT EXP EXPONDIST FACT FACTDOUBLE FALSE FDIST \
FIND FINDB FINV FISHER FISHERINV FIXED FLOOR FORECAST FREQUENCY FTEST FV FVSCHEDULE GAMMADIST GAMMAINV \
GAMMALN GCD GEOMEAN GESTEP GETPIVOTDATA GROWTH HARMEAN HEX2BIN HEX2DEC HEX2OCT HLOOKUP HOUR HYPERLINK \
HYPGEOMDIST IF IFERROR IMABS IMAGINARY IMARGUMENT IMCONJUGATE IMCOS IMDIV IMEXP IMLN IMLOG10 IMLOG2 \
IMPOWER IMPRODUCT IMREAL IMSIN IMSQRT IMSUB IMSUM INDEX INDIRECT INFO INT INTERCEPT INTRATE IPMT IRR \
ISBLANK ISERR ISERROR ISEVEN ISLOGICAL ISNA ISNONTEXT ISNUMBER ISODD ISPMT ISREF ISTEXT JIS KURT LARGE \
LCM LEFT LEFTB LEN LENB LINEST LN LOG LOG10 LOGEST LOGINV LOGNORMDIST LOOKUP LOWER MATCH MAX MAXA \
MDETERM MDURATION MEDIAN MID MIDB MIN MINA MINUTE MINVERSE MIRR MMULT MOD MODE MONTH MROUND MULTINOMIAL \
N NA NEGBINOMDIST NETWORKDAYS NOMINAL NORMDIST NORMINV NORMSDIST NORMSINV NOT NOW NPER NPV OCT2BIN \
OCT2DEC OCT2HEX ODD ODDFPRICE ODDFYIELD ODDLPRICE ODDLYIELD OFFSET OR PEARSON PERCENTILE PERCENTRANK \
PERMUT PHONETIC PI PMT POISSON POWER PPMT PRICE PRICEDISC PRICEMAT PROB PRODUCT PROPER PV QUARTILE \
QUOTIENT RADIANS RAND RANDBETWEEN RANK RATE RECEIVED REGISTER.ID REPLACE REPLACEB REPT RIGHT RIGHTB \
ROMAN ROUND ROUNDDOWN ROUNDUP ROW ROWS RSQ RTD SEARCH SEARCHB SECOND SERIESSUM SIGN SIN SINH SKEW SLN \
SLOPE SMALL SQRT SQRTPI STANDARDIZE STDEV STDEVA STDEVP STDEVPA STEYX SUBSTITUTE SUBTOTAL SUM SUMIF \
SUMIFS SUMPRODUCT SUMSQ SUMX2MY2 SUMX2PY2 SUMXMY2 SYD T TAN TANH TBILLEQ TBILLPRICE TBILLYIELD TDIST TEXT \
TIME TIMEVALUE TINV TODAY TRANSPOSE TREND TRIM TRIMMEAN TRUE TRUNC TTEST TYPE UPPER USDOLLAR VALUE VAR \
VARA VARP VARPA VDB VLOOKUP WEEKDAY WEEKNUM WEIBULL WORKDAY XIRR XNPV YEAR YEARFRAC YIELD YIELDDISC \
YIELDMAT ZTEST";

/// Future functions stored as `_xlfn._xlws.NAME`.
const WORKSHEET_ONLY: [&str; 2] = ["FILTER", "SORT"];

/// The storage prefix of a built-in function name (upper case), if it needs one.
#[must_use]
pub fn storage_prefix(name: &str) -> Option<&'static str> {
    use std::sync::OnceLock;
    static FUTURE: OnceLock<HashSet<&'static str>> = OnceLock::new();
    let future = FUTURE.get_or_init(|| {
        let old: HashSet<&str> = ECMA_2006.split_whitespace().collect();
        function_names()
            .into_iter()
            // BlueOffice's own functions (BO.*) have no storage prefix.
            .filter(|n| !old.contains(n) && !n.starts_with("BO."))
            .collect()
    });
    if WORKSHEET_ONLY.contains(&name) {
        Some("_xlfn._xlws.")
    } else if future.contains(name) {
        Some("_xlfn.")
    } else {
        None
    }
}

/// A token of formula text for the rewrite: everything is copied, identifiers are inspected.
enum Piece<'a> {
    /// Text copied verbatim (operators, numbers, strings, quoted sheet names, …).
    Other(&'a str),
    /// An identifier (function, name, parameter, cell reference …) and whether `(` follows.
    Ident { text: &'a str, call: bool },
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '\\'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.' || c == '\\'
}

/// Split formula text into identifiers and verbatim pieces.
fn pieces(f: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut chars = f.char_indices().peekable();
    let mut other_start = 0;
    while let Some(&(i, c)) = chars.peek() {
        if c == '"' || c == '\'' {
            // A string or a quoted sheet name: the quote doubled inside.
            chars.next();
            while let Some((_, d)) = chars.next() {
                if d == c {
                    if chars.peek().is_some_and(|&(_, e)| e == c) {
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
            continue;
        }
        if c == '[' {
            // Structured references and external-book indices are copied as they are.
            let mut depth = 0;
            for (_, d) in chars.by_ref() {
                match d {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }
        if is_ident_start(c) {
            // An identifier only when it does not continue a number or another token.
            let prev = f[..i].chars().next_back();
            if prev.is_some_and(|p| p.is_alphanumeric() || p == '.' || p == '$') {
                chars.next();
                continue;
            }
            let mut end = i;
            while let Some(&(j, d)) = chars.peek() {
                if is_ident_char(d) {
                    end = j + d.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            let call = f[end..].starts_with('(');
            // A column range such as `A:A` is a reference, not a name.
            if f[end..].starts_with(':') || prev == Some(':') {
                continue;
            }
            if other_start < i {
                out.push(Piece::Other(&f[other_start..i]));
            }
            out.push(Piece::Ident {
                text: &f[i..end],
                call,
            });
            other_start = end;
            continue;
        }
        chars.next();
    }
    if other_start < f.len() {
        out.push(Piece::Other(&f[other_start..]));
    }
    out
}

/// Strip a storage prefix (case-insensitive) from an identifier.
fn strip_prefix(id: &str) -> &str {
    for p in ["_xlfn._xlws.", "_xlfn.", "_xlws.", "_xlpm."] {
        if id.len() > p.len() && id[..p.len()].eq_ignore_ascii_case(p) {
            return &id[p.len()..];
        }
    }
    id
}

/// Parameter names declared by `LET` and `LAMBDA` calls in the text (upper case).
fn parameters(f: &str, list: &[Piece<'_>]) -> HashSet<String> {
    let _ = f;
    let mut params = HashSet::new();
    let mut i = 0;
    while i < list.len() {
        if let Piece::Ident { text, call: true } = &list[i] {
            let name = strip_prefix(text).to_uppercase();
            if name == "LET" || name == "LAMBDA" {
                // Arguments at depth 1 of this call: identifiers right after `(` or `,`.
                let mut depth = 0;
                let mut arg = 0;
                let mut args: Vec<Option<String>> = vec![None];
                let mut fresh = true;
                for p in &list[i + 1..] {
                    match p {
                        Piece::Other(t) => {
                            for ch in t.chars() {
                                match ch {
                                    '(' | '{' => {
                                        depth += 1;
                                        fresh = depth == 1;
                                    }
                                    ')' | '}' => depth -= 1,
                                    ',' if depth == 1 => {
                                        arg += 1;
                                        args.push(None);
                                        fresh = true;
                                    }
                                    c if c.is_whitespace() => {}
                                    _ => fresh = false,
                                }
                                if depth == 0 {
                                    break;
                                }
                            }
                            if depth == 0 {
                                break;
                            }
                        }
                        Piece::Ident { text, call } => {
                            if depth == 1 && fresh && !call {
                                args[arg] = Some(strip_prefix(text).to_uppercase());
                            }
                            fresh = false;
                        }
                    }
                }
                let n = args.len();
                for (k, a) in args.into_iter().enumerate() {
                    let declares = if name == "LET" {
                        k % 2 == 0 && k + 1 < n
                    } else {
                        k + 1 < n
                    };
                    if let (true, Some(a)) = (declares, a) {
                        params.insert(a);
                    }
                }
            }
        }
        i += 1;
    }
    params
}

/// Add the storage prefixes of future functions and `LET` / `LAMBDA` parameters.
#[must_use]
pub fn to_storage(f: &str) -> String {
    let list = pieces(f);
    let params = parameters(f, &list);
    let mut out = String::with_capacity(f.len() + 8);
    for p in &list {
        match p {
            Piece::Other(t) => out.push_str(t),
            Piece::Ident { text, call } => {
                let bare = strip_prefix(text);
                let upper = bare.to_uppercase();
                if params.contains(&upper) {
                    out.push_str("_xlpm.");
                    out.push_str(bare);
                } else if *call {
                    match storage_prefix(&upper) {
                        Some(prefix) => {
                            out.push_str(prefix);
                            out.push_str(bare);
                        }
                        None => out.push_str(text),
                    }
                } else {
                    out.push_str(text);
                }
            }
        }
    }
    out
}

/// Remove storage prefixes, giving the formula as people write it. Only prefixes that
/// [`to_storage`] puts back are removed (a future function BlueOffice does not know keeps its
/// `_xlfn.`), so stored text survives a round trip unchanged.
#[must_use]
pub fn from_storage(f: &str) -> String {
    let list = pieces(f);
    let params = parameters(f, &list);
    let mut out = String::with_capacity(f.len());
    for p in &list {
        match p {
            Piece::Other(t) => out.push_str(t),
            Piece::Ident { text, call } => {
                let bare = strip_prefix(text);
                let prefix = &text[..text.len() - bare.len()];
                let upper = bare.to_uppercase();
                let restored = if prefix.eq_ignore_ascii_case("_xlpm.") {
                    params.contains(&upper)
                } else {
                    *call && storage_prefix(&upper).is_some_and(|p| p.eq_ignore_ascii_case(prefix))
                };
                out.push_str(if restored { bare } else { text });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_future_functions_and_parameters() {
        assert_eq!(
            to_storage("SUM(A1:A3)+XLOOKUP(1,A:A,B:B)"),
            "SUM(A1:A3)+_xlfn.XLOOKUP(1,A:A,B:B)"
        );
        assert_eq!(
            to_storage("FILTER(A1:A9,B1:B9>0)"),
            "_xlfn._xlws.FILTER(A1:A9,B1:B9>0)"
        );
        assert_eq!(
            to_storage("SORTBY(A1:A3,B1:B3)"),
            "_xlfn.SORTBY(A1:A3,B1:B3)"
        );
        assert_eq!(
            to_storage("LET(x, 2, y, x*3, x+y)"),
            "_xlfn.LET(_xlpm.x, 2, _xlpm.y, _xlpm.x*3, _xlpm.x+_xlpm.y)"
        );
        assert_eq!(
            to_storage("LAMBDA(a,b,a+b)(1,2)"),
            "_xlfn.LAMBDA(_xlpm.a,_xlpm.b,_xlpm.a+_xlpm.b)(1,2)"
        );
        // Strings, sheet names and table references are left alone.
        assert_eq!(
            to_storage("CONCAT(\"XLOOKUP(\",'IFS sheet'!A1)"),
            "_xlfn.CONCAT(\"XLOOKUP(\",'IFS sheet'!A1)"
        );
        assert_eq!(to_storage("SUM(Table1[IFS])"), "SUM(Table1[IFS])");
        // Already stored text is kept; BlueOffice functions get no prefix.
        assert_eq!(to_storage("_xlfn.IFS(A1,1)"), "_xlfn.IFS(A1,1)");
        assert_eq!(to_storage("BO.DOCSO(A1)"), "BO.DOCSO(A1)");
        // Old functions and cell references stay unprefixed.
        assert_eq!(
            to_storage("IF(LOG10(A1)>1,AB12,0)"),
            "IF(LOG10(A1)>1,AB12,0)"
        );
    }

    #[test]
    fn removes_prefixes() {
        for shown in [
            "SUM(A1:A3)+XLOOKUP(1,A:A,B:B)",
            "FILTER(A1:A9,B1:B9>0)",
            "LET(x, 2, y, x*3, x+y)",
            "LAMBDA(a,b,a+b)(1,2)",
            "CONCAT(\"_xlfn.\",'S'!A1)",
        ] {
            assert_eq!(from_storage(&to_storage(shown)), shown);
        }
        // Unknown future functions keep their prefix both ways.
        assert_eq!(from_storage("_xlfn.NEWFN(1)"), "_xlfn.NEWFN(1)");
        assert_eq!(to_storage("_xlfn.NEWFN(1)"), "_xlfn.NEWFN(1)");
    }
}
