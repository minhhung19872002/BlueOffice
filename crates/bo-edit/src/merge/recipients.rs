//! Recipient list editing for mail merge: filter by field conditions joined with
//! AND / OR, sort by up to three fields, include / exclude single records, and find
//! duplicate records.
//!
//! Our own rules (no specification covers them): AND binds tighter than OR (the
//! list is an OR of AND-groups); comparisons are numeric when both sides read as
//! numbers, otherwise case-insensitive text in Vietnamese dictionary order
//! ([`crate::collate::vi_cmp`]); sorting is stable.

use super::MergeData;
use std::cmp::Ordering;

/// Comparison of a filter condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
    /// The field is empty.
    Empty,
    /// The field is not empty.
    NotEmpty,
    /// The field contains the value.
    Contains,
    /// The field does not contain the value.
    NotContains,
}

/// One filter condition; `or` joins it to the previous condition with OR (else AND).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRule {
    /// Field (column) name.
    pub field: String,
    /// Comparison.
    pub op: FilterOp,
    /// Value compared with (unused by `empty` / `notEmpty`).
    #[serde(default)]
    pub value: String,
    /// Joined to the previous condition with OR.
    #[serde(default)]
    pub or: bool,
}

/// A sort key.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortKey {
    /// Field (column) name.
    pub field: String,
    /// Descending order.
    #[serde(default)]
    pub descending: bool,
}

/// Maximum number of sort keys.
pub const MAX_SORT_KEYS: usize = 3;

/// Filter, sort and excluded records of the recipient list.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipientOptions {
    /// Conditions (empty: every record).
    #[serde(default)]
    pub filter: Vec<FilterRule>,
    /// Sort keys (at most [`MAX_SORT_KEYS`] are used).
    #[serde(default)]
    pub sort: Vec<SortKey>,
    /// Data rows left out of the merge.
    #[serde(default)]
    pub excluded: Vec<usize>,
}

impl RecipientOptions {
    /// No filter, sort or exclusion.
    pub fn is_default(&self) -> bool {
        self.filter.is_empty() && self.sort.is_empty() && self.excluded.is_empty()
    }
}

fn number(s: &str) -> Option<f64> {
    let t: String = s.trim().chars().filter(|c| !c.is_whitespace()).collect();
    if t.is_empty() {
        return None;
    }
    t.parse().ok().or_else(|| {
        (t.matches(',').count() == 1 && !t.contains('.'))
            .then(|| t.replace(',', ".").parse().ok())
            .flatten()
    })
}

/// Compare two values: as numbers when both are, else as text (case-insensitive).
pub fn compare(a: &str, b: &str) -> Ordering {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => crate::collate::vi_cmp(&a.trim().to_lowercase(), &b.trim().to_lowercase()),
    }
}

fn holds(rule: &FilterRule, v: &str) -> bool {
    let (lv, lw) = (v.trim().to_lowercase(), rule.value.trim().to_lowercase());
    match rule.op {
        FilterOp::Eq => compare(v, &rule.value) == Ordering::Equal,
        FilterOp::Ne => compare(v, &rule.value) != Ordering::Equal,
        FilterOp::Lt => compare(v, &rule.value) == Ordering::Less,
        FilterOp::Le => compare(v, &rule.value) != Ordering::Greater,
        FilterOp::Gt => compare(v, &rule.value) == Ordering::Greater,
        FilterOp::Ge => compare(v, &rule.value) != Ordering::Less,
        FilterOp::Empty => lv.is_empty(),
        FilterOp::NotEmpty => !lv.is_empty(),
        FilterOp::Contains => lv.contains(&lw),
        FilterOp::NotContains => !lv.contains(&lw),
    }
}

/// Record `row` passes the filter (an OR of AND-groups).
pub fn passes(data: &MergeData, row: usize, filter: &[FilterRule]) -> bool {
    if filter.is_empty() {
        return true;
    }
    let mut any = false;
    let mut group = true;
    for (i, r) in filter.iter().enumerate() {
        if i > 0 && r.or {
            any |= group;
            group = true;
        }
        group &= holds(r, data.value(row, &r.field).unwrap_or(""));
    }
    any || group
}

/// Data rows of the recipient list: filtered, without excluded rows, then sorted.
pub fn active(data: &MergeData, opts: &RecipientOptions) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..data.rows.len())
        .filter(|r| !opts.excluded.contains(r) && passes(data, *r, &opts.filter))
        .collect();
    let keys: Vec<&SortKey> = opts.sort.iter().take(MAX_SORT_KEYS).collect();
    if !keys.is_empty() {
        rows.sort_by(|&a, &b| {
            for k in &keys {
                let o = compare(
                    data.value(a, &k.field).unwrap_or(""),
                    data.value(b, &k.field).unwrap_or(""),
                );
                let o = if k.descending { o.reverse() } else { o };
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
    }
    rows
}

/// Groups of data rows with the same values in `fields` (every field when empty),
/// compared trimmed, case-insensitive and with runs of spaces collapsed; groups of
/// two or more, in order of first appearance.
pub fn duplicates(data: &MergeData, fields: &[String]) -> Vec<Vec<usize>> {
    let fields: Vec<String> = if fields.is_empty() {
        data.fields.clone()
    } else {
        fields.to_vec()
    };
    let key = |r: usize| -> Vec<String> {
        fields
            .iter()
            .map(|f| {
                data.value(r, f)
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase()
            })
            .collect()
    };
    let mut groups: Vec<(Vec<String>, Vec<usize>)> = Vec::new();
    let mut index: std::collections::HashMap<Vec<String>, usize> = std::collections::HashMap::new();
    for r in 0..data.rows.len() {
        let k = key(r);
        if k.iter().all(String::is_empty) {
            continue;
        }
        if let Some(&g) = index.get(&k) {
            groups[g].1.push(r);
        } else {
            index.insert(k.clone(), groups.len());
            groups.push((k, vec![r]));
        }
    }
    groups
        .into_iter()
        .map(|(_, v)| v)
        .filter(|v| v.len() > 1)
        .collect()
}
