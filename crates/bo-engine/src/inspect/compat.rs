//! Compatibility checker helpers: which features are lost or changed when a file
//! is saved in the 97–2003 binary formats (limits of those formats) or as
//! OpenDocument through BlueOffice's converters (`bo-odf` writes text and basic
//! formatting for `.odt`, values / formulas / cell styles for `.ods` and text
//! frames / notes for `.odp`).

use super::{CompatItem, Feature, Format};

/// Both formats.
pub const BOTH: &[Format] = &[Format::Legacy, Format::Odf];
/// The 97–2003 binary formats only.
pub const LEGACY: &[Format] = &[Format::Legacy];
/// OpenDocument only.
pub const ODF: &[Format] = &[Format::Odf];

/// One compatibility row.
pub fn item(feature: Feature, count: usize, formats: &[Format]) -> CompatItem {
    CompatItem {
        feature,
        count,
        formats: formats.to_vec(),
    }
}

/// Worksheet functions that the 97–2003 function table does not have (later
/// additions: lookup, dynamic array, text and statistics functions).
pub const NEW_FUNCTIONS: &[&str] = &[
    "XLOOKUP",
    "XMATCH",
    "FILTER",
    "SORT",
    "SORTBY",
    "UNIQUE",
    "SEQUENCE",
    "RANDARRAY",
    "LET",
    "LAMBDA",
    "TEXTJOIN",
    "CONCAT",
    "IFS",
    "SWITCH",
    "MAXIFS",
    "MINIFS",
    "IFNA",
    "IFERROR",
    "SUMIFS",
    "COUNTIFS",
    "AVERAGEIFS",
    "AGGREGATE",
    "TEXTBEFORE",
    "TEXTAFTER",
    "TEXTSPLIT",
    "VSTACK",
    "HSTACK",
    "TAKE",
    "DROP",
    "CHOOSECOLS",
    "CHOOSEROWS",
    "TOCOL",
    "TOROW",
    "WRAPROWS",
    "WRAPCOLS",
    "EXPAND",
    "MAP",
    "REDUCE",
    "SCAN",
    "MAKEARRAY",
    "BYROW",
    "BYCOL",
    "ISOMITTED",
    "STOCKHISTORY",
    "FORECAST.LINEAR",
    "STDEV.S",
    "STDEV.P",
    "VAR.S",
    "VAR.P",
    "NORM.DIST",
    "PERCENTILE.INC",
    "QUARTILE.INC",
    "RANK.EQ",
    "MODE.SNGL",
    "DAYS",
    "ISOWEEKNUM",
    "XOR",
    "BITAND",
    "BITOR",
    "IMAGE",
    "ENCODEURL",
    "FILTERXML",
    "WEBSERVICE",
];

/// The formula uses a function the 97–2003 function table lacks.
pub fn uses_new_function(formula: &str) -> bool {
    let up = formula.to_ascii_uppercase();
    // Newer functions are also written with the `_xlfn.` prefix (§18.17.7 future functions).
    if up.contains("_XLFN.") {
        return true;
    }
    NEW_FUNCTIONS.iter().any(|f| {
        up.match_indices(f).any(|(i, _)| {
            let before = up[..i].chars().next_back();
            let after = up[i + f.len()..].chars().next();
            !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
                && after == Some('(')
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_functions() {
        assert!(uses_new_function("=XLOOKUP(A1,B:B,C:C)"));
        assert!(uses_new_function("_xlfn.CONCAT(A1,B1)"));
        assert!(uses_new_function("SUM(A1)+IFERROR(1/0,0)"));
        assert!(!uses_new_function("SUM(A1:A3)+VLOOKUP(1,B:C,2)"));
        assert!(!uses_new_function("MYSORT(A1)"));
    }
}
