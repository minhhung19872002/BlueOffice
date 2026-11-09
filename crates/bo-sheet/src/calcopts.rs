//! Calculation options of a workbook (ECMA-376 Part 1, §18.2.2 `calcPr`).
//!
//! The `calcPr` element is kept verbatim in [`Workbook::unknown`] so that
//! attributes the model does not use survive a round trip. [`CalcSettings::read`]
//! parses the attributes BlueSheet acts on; [`CalcSettings::write`] rewrites
//! only those attributes, leaving the others (and their order) untouched, and
//! adds the element when the file had none.

use bo_model::sheet::{RawXml, Workbook};
use serde::{Deserialize, Serialize};

/// `calcPr/@calcMode` (§18.18.5 `ST_CalcMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CalcMode {
    /// `auto`: recalculate after every change (the default).
    #[default]
    Auto,
    /// `autoNoTable`: automatic, except data tables (§18.3.1.40 `f` with
    /// `t="dataTable"`) which wait for an explicit calculation.
    AutoNoTable,
    /// `manual`: only on request (calculate now / calculate sheet).
    Manual,
}

impl CalcMode {
    fn parse(s: &str) -> CalcMode {
        match s {
            "manual" => CalcMode::Manual,
            "autoNoTable" => CalcMode::AutoNoTable,
            _ => CalcMode::Auto,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            CalcMode::Auto => "auto",
            CalcMode::AutoNoTable => "autoNoTable",
            CalcMode::Manual => "manual",
        }
    }
}

/// Default `iterateCount` (§18.2.2).
pub const DEFAULT_ITERATE_COUNT: u32 = 100;
/// Default `iterateDelta` (§18.2.2).
pub const DEFAULT_ITERATE_DELTA: f64 = 0.001;

/// The calculation settings BlueSheet uses (§18.2.2 `calcPr`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalcSettings {
    /// `calcMode`.
    pub mode: CalcMode,
    /// `iterate`: circular references are calculated iteratively.
    pub iterate: bool,
    /// `iterateCount`: maximum number of iterations.
    pub iterate_count: u32,
    /// `iterateDelta`: iteration stops when no value changes by more than this.
    pub iterate_delta: f64,
    /// `fullPrecision`: `false` means "precision as displayed": numbers are
    /// used in calculations rounded as their number format shows them.
    pub full_precision: bool,
    /// `calcOnSave`: recalculate before saving (matters in manual mode).
    pub calc_on_save: bool,
    /// `fullCalcOnLoad`: recalculate everything when the file is opened.
    pub full_calc_on_load: bool,
}

impl Default for CalcSettings {
    fn default() -> Self {
        CalcSettings {
            mode: CalcMode::Auto,
            iterate: false,
            iterate_count: DEFAULT_ITERATE_COUNT,
            iterate_delta: DEFAULT_ITERATE_DELTA,
            full_precision: true,
            calc_on_save: true,
            full_calc_on_load: false,
        }
    }
}

fn is_calc_pr(x: &RawXml) -> bool {
    element(&x.xml).is_some_and(|(name, _)| local(&name) == "calcPr")
}

fn local(qname: &str) -> &str {
    qname.rsplit(':').next().unwrap_or(qname)
}

fn xsd_bool(s: &str) -> Option<bool> {
    match s.trim() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

impl CalcSettings {
    /// Settings of `wb` (defaults when it has no `calcPr`).
    pub fn read(wb: &Workbook) -> CalcSettings {
        let mut s = CalcSettings::default();
        let Some((_, attrs)) = wb
            .unknown
            .iter()
            .find(|x| is_calc_pr(x))
            .and_then(|x| element(&x.xml))
        else {
            return s;
        };
        for (k, v) in &attrs {
            match local(k) {
                "calcMode" => s.mode = CalcMode::parse(v),
                "iterate" => s.iterate = xsd_bool(v).unwrap_or(false),
                "iterateCount" => {
                    if let Ok(n) = v.trim().parse::<u32>() {
                        s.iterate_count = n;
                    }
                }
                "iterateDelta" => {
                    if let Ok(d) = v.trim().parse::<f64>() {
                        if d.is_finite() && d >= 0.0 {
                            s.iterate_delta = d;
                        }
                    }
                }
                "fullPrecision" => s.full_precision = xsd_bool(v).unwrap_or(true),
                "calcOnSave" => s.calc_on_save = xsd_bool(v).unwrap_or(true),
                "fullCalcOnLoad" => s.full_calc_on_load = xsd_bool(v).unwrap_or(false),
                _ => {}
            }
        }
        s
    }

    /// Writes the settings into `wb`'s `calcPr`, changing only attributes whose
    /// value differs from what the element says (absent attributes stand for
    /// their schema default). Returns whether anything changed.
    pub fn write(&self, wb: &mut Workbook) -> bool {
        if CalcSettings::read(wb) == *self {
            return false;
        }
        let pos = wb.unknown.iter().position(is_calc_pr);
        let (qname, mut attrs) = pos
            .and_then(|i| element(&wb.unknown[i].xml))
            .unwrap_or_else(|| ("calcPr".to_owned(), Vec::new()));
        let d = CalcSettings::default();
        // The attributes of calcPr are unqualified, whatever the element prefix.
        let mut set = |name: &str, value: Option<String>| {
            let i = attrs.iter().position(|(k, _)| local(k) == name);
            match (i, value) {
                (Some(i), Some(v)) => attrs[i].1 = v,
                (Some(i), None) => {
                    attrs.remove(i);
                }
                (None, Some(v)) => attrs.push((name.to_owned(), v)),
                (None, None) => {}
            }
        };
        let old = CalcSettings::read(wb);
        // §18.2.2: calcMode default "auto"; iterate false; iterateCount 100;
        // iterateDelta 0.001; fullPrecision true; calcOnSave true; fullCalcOnLoad false.
        if old.mode != self.mode {
            set(
                "calcMode",
                (self.mode != d.mode).then(|| self.mode.as_str().to_owned()),
            );
        }
        if old.iterate != self.iterate {
            set("iterate", self.iterate.then(|| "1".to_owned()));
        }
        if old.iterate_count != self.iterate_count {
            set(
                "iterateCount",
                (self.iterate_count != d.iterate_count).then(|| self.iterate_count.to_string()),
            );
        }
        if old.iterate_delta.to_bits() != self.iterate_delta.to_bits() {
            set(
                "iterateDelta",
                (self.iterate_delta.to_bits() != d.iterate_delta.to_bits())
                    .then(|| self.iterate_delta.to_string()),
            );
        }
        if old.full_precision != self.full_precision {
            set(
                "fullPrecision",
                (!self.full_precision).then(|| "0".to_owned()),
            );
        }
        if old.calc_on_save != self.calc_on_save {
            set("calcOnSave", (!self.calc_on_save).then(|| "0".to_owned()));
        }
        if old.full_calc_on_load != self.full_calc_on_load {
            set(
                "fullCalcOnLoad",
                self.full_calc_on_load.then(|| "1".to_owned()),
            );
        }
        let mut xml = format!("<{qname}");
        for (k, v) in &attrs {
            xml.push(' ');
            xml.push_str(k);
            xml.push_str("=\"");
            xml.push_str(&escape(v));
            xml.push('"');
        }
        xml.push_str("/>");
        match pos {
            Some(i) => wb.unknown[i].xml = xml,
            None => wb.unknown.push(RawXml {
                after: "calcPr".into(),
                xml,
            }),
        }
        true
    }

    /// Iterative calculation settings for the formula engine (`None`: off).
    pub fn iterative(&self) -> Option<bo_formula::IterativeConfig> {
        self.iterate.then_some(bo_formula::IterativeConfig {
            max_iterations: self.iterate_count.max(1),
            max_change: self.iterate_delta,
        })
    }
}

pub(crate) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Name and attributes of the first element of an XML fragment.
pub(crate) fn element(xml: &str) -> Option<(String, Vec<(String, String)>)> {
    let start = xml.find('<')?;
    let rest = &xml[start + 1..];
    let name_end = rest
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .unwrap_or(rest.len());
    let name = rest[..name_end].to_owned();
    if name.is_empty() || name.starts_with(['?', '!']) {
        return None;
    }
    let mut attrs = Vec::new();
    let mut s = &rest[name_end..];
    loop {
        s = s.trim_start();
        if s.is_empty() || s.starts_with('/') || s.starts_with('>') {
            break;
        }
        let eq = s.find('=')?;
        let key = s[..eq].trim().to_owned();
        let after = s[eq + 1..].trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let body = &after[1..];
        let close = body.find(quote)?;
        attrs.push((key, unescape(&body[..close])));
        s = &body[close + 1..];
    }
    Some((name, attrs))
}

/// Decimal places a number format shows, for "precision as displayed":
/// digit placeholders (`0`, `#`, `?`) after the decimal point, plus two per
/// `%`, minus three per thousands-scaling comma after the last placeholder
/// (ECMA-376 Part 1, §18.8.31 numFmt). `None` for `General`, text, date/time,
/// scientific and fraction formats, which are not rounded.
pub fn display_decimals(code: &str) -> Option<i32> {
    // First section: positive numbers.
    let mut section = String::new();
    let mut chars = code.chars().peekable();
    let mut in_quote = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => in_quote = !in_quote,
            _ if in_quote => {}
            ';' => break,
            '\\' | '_' | '*' => {
                chars.next();
            }
            '[' => {
                // Colours / conditions are skipped; elapsed time is a date-time code.
                let mut inner = String::new();
                for d in chars.by_ref() {
                    if d == ']' {
                        break;
                    }
                    inner.push(d);
                }
                let l = inner.to_ascii_lowercase();
                if l.starts_with('h') || l.starts_with('m') || l.starts_with('s') {
                    return None;
                }
            }
            _ => section.push(c),
        }
    }
    let lower = section.to_ascii_lowercase();
    if lower.trim().is_empty() || lower.contains("general") || section.contains('@') {
        return None;
    }
    if lower.contains(['y', 'd', 'h', 's', 'm', 'e', '/']) {
        return None;
    }
    let is_digit = |c: char| matches!(c, '0' | '#' | '?');
    let last = section.rfind(is_digit)?;
    let decimals = section.find('.').map_or(0, |dot| {
        section[dot..].chars().filter(|&c| is_digit(c)).count()
    });
    let percents = section.chars().filter(|&c| c == '%').count();
    let scaling = section[last + 1..]
        .chars()
        .take_while(|&c| c == ',')
        .count();
    let d = decimals as i64 + 2 * percents as i64 - 3 * scaling as i64;
    i32::try_from(d).ok()
}

/// "Precision as displayed": every numeric cell (constant or formula) whose
/// number format shows a fixed number of decimals is read rounded to them by
/// the formula engine.
pub(crate) fn apply_display_precision(fw: &mut bo_formula::Workbook, wb: &Workbook) {
    let locale = crate::numfmt::Locale::default();
    let mut by_style: std::collections::HashMap<u32, Option<i32>> =
        std::collections::HashMap::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        for (&(r, c), cell) in &ws.cells {
            let numeric = cell.formula.is_some()
                || matches!(cell.value, bo_model::sheet::CellValue::Number(_));
            if !numeric {
                continue;
            }
            let d = *by_style.entry(cell.style).or_insert_with(|| {
                wb.styles.xf(cell.style).and_then(|xf| {
                    display_decimals(&crate::numfmt::format_code(
                        &wb.styles,
                        xf.num_fmt_id,
                        &locale,
                    ))
                })
            });
            if d.is_some() {
                fw.set_display_precision(si, r, c, d);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_of_number_formats() {
        assert_eq!(display_decimals("0.00"), Some(2));
        assert_eq!(display_decimals("#,##0"), Some(0));
        assert_eq!(display_decimals("#,##0.000;[Red]-#,##0.000"), Some(3));
        assert_eq!(display_decimals("0.0%"), Some(3));
        assert_eq!(display_decimals("#,##0,"), Some(-3));
        assert_eq!(display_decimals("0.0,,"), Some(-5));
        assert_eq!(display_decimals("0.00\" kg\""), Some(2));
        assert_eq!(display_decimals("General"), None);
        assert_eq!(display_decimals("dd/mm/yyyy"), None);
        assert_eq!(display_decimals("[h]:mm"), None);
        assert_eq!(display_decimals("0.00E+00"), None);
        assert_eq!(display_decimals("# ?/?"), None);
        assert_eq!(display_decimals("@"), None);
    }

    #[test]
    fn settings_round_trip_through_calc_pr() {
        let mut wb = Workbook::new();
        assert_eq!(CalcSettings::read(&wb), CalcSettings::default());
        // Unchanged settings do not touch the workbook.
        assert!(!CalcSettings::default().write(&mut wb));
        assert_eq!(wb.unknown.len(), 0);
        wb.unknown.push(RawXml {
            after: "calcPr".into(),
            xml: r#"<calcPr calcId="191029" fullCalcOnLoad="1"/>"#.into(),
        });
        let s = CalcSettings::read(&wb);
        assert!(s.full_calc_on_load);
        let mut s2 = s;
        s2.mode = CalcMode::Manual;
        s2.iterate = true;
        s2.iterate_count = 50;
        s2.iterate_delta = 0.01;
        s2.full_precision = false;
        assert!(s2.write(&mut wb));
        // Other attributes and their order are kept; ours are appended.
        assert_eq!(
            wb.unknown[0].xml,
            r#"<calcPr calcId="191029" fullCalcOnLoad="1" calcMode="manual" iterate="1" iterateCount="50" iterateDelta="0.01" fullPrecision="0"/>"#
        );
        assert_eq!(CalcSettings::read(&wb), s2);
        // Back to defaults removes them again.
        assert!(s.write(&mut wb));
        assert_eq!(
            wb.unknown[0].xml,
            r#"<calcPr calcId="191029" fullCalcOnLoad="1"/>"#
        );
        // A workbook without calcPr gets one in the calcPr slot.
        let mut wb = Workbook::new();
        let m = CalcSettings {
            mode: CalcMode::AutoNoTable,
            ..CalcSettings::default()
        };
        assert!(m.write(&mut wb));
        assert_eq!(wb.unknown[0].after, "calcPr");
        assert_eq!(wb.unknown[0].xml, r#"<calcPr calcMode="autoNoTable"/>"#);
    }
}
