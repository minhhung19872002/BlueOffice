//! The steps of the query language, applied to a table. Steps that read other queries
//! (`merge`, `append`) get those tables from the evaluator.

use crate::def::{Aggregate, Condition, JoinKind, SortKey, Step};
use crate::value::{unique_names, ColType, QTable, Val};
use crate::QueryError;
use std::collections::{HashMap, HashSet};

fn col(t: &QTable, name: &str) -> Result<usize, QueryError> {
    t.col(name)
        .ok_or_else(|| QueryError::Column(name.to_owned()))
}

fn cols(t: &QTable, names: &[String]) -> Result<Vec<usize>, QueryError> {
    names.iter().map(|n| col(t, n)).collect()
}

/// Keep the columns `keep` (in that order).
fn project(t: QTable, keep: &[usize]) -> QTable {
    QTable {
        columns: keep.iter().map(|&i| t.columns[i].clone()).collect(),
        types: keep.iter().map(|&i| t.types[i]).collect(),
        rows: t
            .rows
            .into_iter()
            .map(|r| keep.iter().map(|&i| r[i].clone()).collect())
            .collect(),
    }
}

/// Apply a step that needs no other query. `others` gives the tables of the queries named
/// by `merge` / `append`.
pub fn apply(
    step: &Step,
    mut t: QTable,
    others: &dyn Fn(&str) -> Result<QTable, QueryError>,
) -> Result<QTable, QueryError> {
    match step {
        Step::PromoteHeaders => {
            if t.rows.is_empty() {
                return Ok(t);
            }
            let header = t.rows.remove(0);
            let names = header.iter().map(Val::display).collect();
            Ok(QTable::new(unique_names(names), t.rows))
        }
        Step::DemoteHeaders => {
            let header = t.columns.iter().map(|c| Val::Text(c.clone())).collect();
            t.rows.insert(0, header);
            Ok(QTable::from_rows(t.rows))
        }
        Step::ChangeType {
            columns,
            decimal,
            month_first,
        } => {
            let dec = decimal
                .as_deref()
                .and_then(|d| d.chars().next())
                .unwrap_or('.');
            for ct in columns {
                let i = col(&t, &ct.column)?;
                t.types[i] = ct.ty;
                for r in &mut t.rows {
                    r[i] = ct.ty.convert(&r[i], dec, *month_first);
                }
            }
            Ok(t)
        }
        Step::FilterRows {
            column,
            condition,
            value,
        } => {
            let i = col(&t, column)?;
            let ty = t.types[i];
            t.rows.retain(|r| matches(&r[i], *condition, value, ty));
            Ok(t)
        }
        Step::RemoveColumns { columns } => {
            let drop: HashSet<usize> = cols(&t, columns)?.into_iter().collect();
            let keep: Vec<usize> = (0..t.columns.len()).filter(|i| !drop.contains(i)).collect();
            Ok(project(t, &keep))
        }
        Step::KeepColumns { columns } => {
            let keep = cols(&t, columns)?;
            Ok(project(t, &keep))
        }
        Step::ReorderColumns { columns } => {
            let mut keep = cols(&t, columns)?;
            for i in 0..t.columns.len() {
                if !keep.contains(&i) {
                    keep.push(i);
                }
            }
            Ok(project(t, &keep))
        }
        Step::RenameColumns { renames } => {
            for r in renames {
                let i = col(&t, &r.from)?;
                let to = r.to.trim();
                if to.is_empty() {
                    return Err(QueryError::Step("empty column name".into()));
                }
                if t.col(to).is_some_and(|j| j != i) {
                    return Err(QueryError::Step(format!("duplicate column {to}")));
                }
                t.columns[i] = to.to_owned();
            }
            Ok(t)
        }
        Step::SplitColumn {
            column,
            delimiter,
            parts,
        } => {
            if delimiter.is_empty() {
                return Err(QueryError::Step("empty delimiter".into()));
            }
            let i = col(&t, column)?;
            let split: Vec<Vec<String>> = t
                .rows
                .iter()
                .map(|r| {
                    let s = r[i].display();
                    if r[i].is_blank() {
                        Vec::new()
                    } else {
                        match parts {
                            Some(n) => s
                                .splitn((*n).max(1), delimiter.as_str())
                                .map(str::to_owned)
                                .collect(),
                            None => s.split(delimiter.as_str()).map(str::to_owned).collect(),
                        }
                    }
                })
                .collect();
            let n = parts
                .unwrap_or_else(|| split.iter().map(Vec::len).max().unwrap_or(1))
                .max(1);
            let base = t.columns[i].clone();
            let new_names: Vec<String> = (1..=n).map(|k| format!("{base}.{k}")).collect();
            let mut names = t.columns.clone();
            names.splice(i..=i, new_names);
            let mut types = t.types.clone();
            types.splice(i..=i, vec![ColType::Text; n]);
            let rows = t
                .rows
                .into_iter()
                .zip(split)
                .map(|(mut r, pieces)| {
                    let vals: Vec<Val> = (0..n)
                        .map(|k| pieces.get(k).map_or(Val::Null, |p| Val::Text(p.clone())))
                        .collect();
                    r.splice(i..=i, vals);
                    r
                })
                .collect();
            Ok(QTable {
                columns: unique_names(names),
                types,
                rows,
            })
        }
        Step::ReplaceValues {
            columns,
            find,
            replace,
            match_entire,
        } => {
            let idx: Vec<usize> = if columns.is_empty() {
                (0..t.columns.len()).collect()
            } else {
                cols(&t, columns)?
            };
            for r in &mut t.rows {
                for &i in &idx {
                    let v = &r[i];
                    let text = v.display();
                    if *match_entire {
                        let hit = if find.is_empty() {
                            v.is_blank()
                        } else {
                            text == *find
                        };
                        if hit {
                            r[i] = retype(replace, t.types[i]);
                        }
                    } else if !find.is_empty() && text.contains(find.as_str()) {
                        r[i] = retype(&text.replace(find.as_str(), replace), t.types[i]);
                    }
                }
            }
            Ok(t)
        }
        Step::GroupBy { keys, aggregates } => group_by(&t, keys, aggregates),
        Step::Sort { keys } => sort(t, keys),
        Step::Merge {
            query,
            left_keys,
            right_keys,
            kind,
            columns,
        } => {
            let right = others(query)?;
            merge(&t, &right, left_keys, right_keys, *kind, columns)
        }
        Step::Append { queries } => {
            for q in queries {
                let other = others(q)?;
                t = append(t, other);
            }
            Ok(t)
        }
        Step::Unpivot {
            columns,
            other,
            attribute,
            value,
        } => {
            let listed: HashSet<usize> = cols(&t, columns)?.into_iter().collect();
            let pivot: Vec<usize> = (0..t.columns.len())
                .filter(|i| listed.contains(i) != *other)
                .collect();
            let keep: Vec<usize> = (0..t.columns.len())
                .filter(|i| !pivot.contains(i))
                .collect();
            let mut names: Vec<String> = keep.iter().map(|&i| t.columns[i].clone()).collect();
            names.push(attribute.clone());
            names.push(value.clone());
            let mut types: Vec<ColType> = keep.iter().map(|&i| t.types[i]).collect();
            types.push(ColType::Text);
            types.push(ColType::Any);
            let mut rows = Vec::new();
            for r in &t.rows {
                for &p in &pivot {
                    if r[p].is_blank() {
                        continue;
                    }
                    let mut out: Vec<Val> = keep.iter().map(|&i| r[i].clone()).collect();
                    out.push(Val::Text(t.columns[p].clone()));
                    out.push(r[p].clone());
                    rows.push(out);
                }
            }
            Ok(QTable {
                columns: unique_names(names),
                types,
                rows,
            })
        }
        Step::AddColumn { name, formula } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(QueryError::Step("empty column name".into()));
            }
            if t.col(name).is_some() {
                return Err(QueryError::Step(format!("duplicate column {name}")));
            }
            let vals = crate::formula::column(&t, formula)?;
            let ty = ColType::detect(vals.iter());
            let ty = if matches!(ty, ColType::Integer | ColType::Number)
                && vals.iter().all(|v| !matches!(v, Val::Text(_)))
            {
                ty
            } else {
                ColType::Any
            };
            t.columns.push(name.to_owned());
            t.types.push(ty);
            for (r, v) in t.rows.iter_mut().zip(vals) {
                r.push(v);
            }
            Ok(t)
        }
        Step::RemoveDuplicates { columns } => {
            let idx: Vec<usize> = if columns.is_empty() {
                (0..t.columns.len()).collect()
            } else {
                cols(&t, columns)?
            };
            let mut seen = HashSet::new();
            t.rows.retain(|r| {
                let key: Vec<String> = idx.iter().map(|&i| r[i].key().to_lowercase()).collect();
                seen.insert(key)
            });
            Ok(t)
        }
        Step::KeepTop { count } => {
            t.rows.truncate(*count);
            Ok(t)
        }
        Step::RemoveTop { count } => {
            let n = (*count).min(t.rows.len());
            t.rows.drain(..n);
            Ok(t)
        }
        Step::RemoveBlankRows => {
            t.rows.retain(|r| !r.iter().all(Val::is_blank));
            Ok(t)
        }
    }
}

/// A replacement typed as the column (numbers stay numbers).
fn retype(text: &str, ty: ColType) -> Val {
    match ty {
        ColType::Any | ColType::Text => Val::Text(text.to_owned()),
        other => other.convert(&Val::Text(text.to_owned()), '.', false),
    }
}

fn matches(v: &Val, cond: Condition, value: &str, ty: ColType) -> bool {
    use Condition as C;
    match cond {
        C::IsBlank => return v.is_blank(),
        C::IsNotBlank => return !v.is_blank(),
        _ => {}
    }
    let text = v.display().to_lowercase();
    let needle = value.to_lowercase();
    match cond {
        C::Contains => return text.contains(&needle),
        C::NotContains => return !text.contains(&needle),
        C::BeginsWith => return text.starts_with(&needle),
        C::EndsWith => return text.ends_with(&needle),
        _ => {}
    }
    // Comparisons: as numbers (dates as serials) when both sides read as such.
    let rhs = match ty {
        ColType::Date => crate::value::parse_date(value, false)
            .or_else(|| crate::value::parse_number(value, '.')),
        _ => crate::value::parse_number(value, '.'),
    };
    let lhs = match v {
        Val::Num(n) | Val::Date(n) => Some(*n),
        Val::Text(s) if !matches!(ty, ColType::Text) => crate::value::parse_number(s, '.'),
        _ => None,
    };
    let ord = match (lhs, rhs) {
        (Some(a), Some(b)) => a.partial_cmp(&b),
        _ => Some(text.cmp(&needle)),
    };
    let Some(ord) = ord else {
        return false;
    };
    match cond {
        C::Equals => ord.is_eq(),
        C::NotEquals => !ord.is_eq(),
        C::GreaterThan => ord.is_gt(),
        C::GreaterOrEqual => ord.is_ge(),
        C::LessThan => ord.is_lt(),
        C::LessOrEqual => ord.is_le(),
        _ => false,
    }
}

fn sort(mut t: QTable, keys: &[SortKey]) -> Result<QTable, QueryError> {
    let idx: Vec<(usize, bool)> = keys
        .iter()
        .map(|k| col(&t, &k.column).map(|i| (i, k.descending)))
        .collect::<Result<_, _>>()?;
    // Stable: equal rows keep their order.
    t.rows.sort_by(|a, b| {
        for &(i, desc) in &idx {
            let (a, b) = (&a[i], &b[i]);
            // Nulls last in both directions.
            let o = match (a.is_blank(), b.is_blank()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ if desc => b.compare(a),
                _ => a.compare(b),
            };
            if o.is_ne() {
                return o;
            }
        }
        std::cmp::Ordering::Equal
    });
    Ok(t)
}

fn group_by(t: &QTable, keys: &[String], aggs: &[Aggregate]) -> Result<QTable, QueryError> {
    let kidx = cols(t, keys)?;
    let aidx: Vec<Option<usize>> = aggs
        .iter()
        .map(|a| {
            if a.op == "count" {
                Ok(None)
            } else {
                col(t, &a.column).map(Some)
            }
        })
        .collect::<Result<_, _>>()?;
    let mut order: Vec<Vec<Val>> = Vec::new();
    let mut groups: HashMap<Vec<String>, Vec<usize>> = HashMap::new();
    for (ri, r) in t.rows.iter().enumerate() {
        let key: Vec<String> = kidx.iter().map(|&i| r[i].key()).collect();
        let e = groups.entry(key).or_default();
        if e.is_empty() {
            order.push(kidx.iter().map(|&i| r[i].clone()).collect());
        }
        e.push(ri);
    }
    let mut names: Vec<String> = kidx.iter().map(|&i| t.columns[i].clone()).collect();
    names.extend(aggs.iter().map(|a| a.name.clone()));
    let mut types: Vec<ColType> = kidx.iter().map(|&i| t.types[i]).collect();
    for (a, i) in aggs.iter().zip(&aidx) {
        types.push(match a.op.as_str() {
            "count" | "countDistinct" => ColType::Integer,
            "min" | "max" => i.map_or(ColType::Any, |i| t.types[i]),
            _ => ColType::Number,
        });
    }
    let mut rows = Vec::with_capacity(order.len());
    for keyvals in order {
        let key: Vec<String> = keyvals.iter().map(Val::key).collect();
        let members = &groups[&key];
        let mut row = keyvals;
        for (a, i) in aggs.iter().zip(&aidx) {
            let vals: Vec<&Val> = match i {
                Some(i) => members.iter().map(|&r| &t.rows[r][*i]).collect(),
                None => Vec::new(),
            };
            row.push(aggregate(&a.op, members.len(), &vals)?);
        }
        rows.push(row);
    }
    Ok(QTable {
        columns: unique_names(names),
        types,
        rows,
    })
}

fn aggregate(op: &str, count: usize, vals: &[&Val]) -> Result<Val, QueryError> {
    let nums: Vec<f64> = vals
        .iter()
        .filter(|v| !v.is_blank())
        .filter_map(|v| v.as_num())
        .collect();
    let date = vals.iter().any(|v| matches!(v, Val::Date(_)));
    Ok(match op {
        "count" => Val::Num(count as f64),
        "countDistinct" => {
            let set: HashSet<String> = vals
                .iter()
                .filter(|v| !v.is_blank())
                .map(|v| v.key())
                .collect();
            Val::Num(set.len() as f64)
        }
        "sum" => Val::Num(nums.iter().sum()),
        "average" | "median" if nums.is_empty() => Val::Null,
        "average" => Val::Num(nums.iter().sum::<f64>() / nums.len() as f64),
        "median" => {
            let mut s = nums.clone();
            s.sort_by(f64::total_cmp);
            let m = s.len() / 2;
            Val::Num(if s.len().is_multiple_of(2) {
                f64::midpoint(s[m - 1], s[m])
            } else {
                s[m]
            })
        }
        "min" | "max" => {
            let pick = vals
                .iter()
                .filter(|v| !v.is_blank())
                .copied()
                .reduce(|a, b| {
                    let o = a.compare(b);
                    if (op == "min") == o.is_le() {
                        a
                    } else {
                        b
                    }
                });
            match pick {
                Some(v) => v.clone(),
                None => Val::Null,
            }
        }
        other => return Err(QueryError::Step(format!("unknown aggregate {other}"))),
    })
    .map(|v| match v {
        Val::Num(n) if date && op != "count" && op != "countDistinct" && op != "sum" => {
            Val::Date(n)
        }
        v => v,
    })
}

fn merge(
    left: &QTable,
    right: &QTable,
    left_keys: &[String],
    right_keys: &[String],
    kind: JoinKind,
    columns: &[String],
) -> Result<QTable, QueryError> {
    if left_keys.is_empty() || left_keys.len() != right_keys.len() {
        return Err(QueryError::Step("merge needs matching key columns".into()));
    }
    let lk = cols(left, left_keys)?;
    let rk = cols(right, right_keys)?;
    let bring: Vec<usize> = if columns.is_empty() {
        (0..right.columns.len())
            .filter(|i| !rk.contains(i))
            .collect()
    } else {
        cols(right, columns)?
    };
    let key = |r: &[Val], idx: &[usize]| -> Vec<String> {
        idx.iter().map(|&i| r[i].key().to_lowercase()).collect()
    };
    let mut index: HashMap<Vec<String>, Vec<usize>> = HashMap::new();
    for (i, r) in right.rows.iter().enumerate() {
        index.entry(key(r, &rk)).or_default().push(i);
    }
    let mut names = left.columns.clone();
    names.extend(bring.iter().map(|&i| right.columns[i].clone()));
    let mut types = left.types.clone();
    types.extend(bring.iter().map(|&i| right.types[i]));
    let nl = left.columns.len();
    let mut used = vec![false; right.rows.len()];
    let mut rows = Vec::new();
    for r in &left.rows {
        let hits = index.get(&key(r, &lk));
        match (hits, kind) {
            (
                Some(h),
                JoinKind::LeftOuter | JoinKind::Inner | JoinKind::FullOuter | JoinKind::RightOuter,
            ) => {
                for &j in h {
                    used[j] = true;
                    let mut row = r.clone();
                    row.extend(bring.iter().map(|&i| right.rows[j][i].clone()));
                    rows.push(row);
                }
            }
            (Some(h), JoinKind::RightAnti) => {
                for &j in h {
                    used[j] = true;
                }
            }
            (None, JoinKind::LeftOuter | JoinKind::FullOuter | JoinKind::LeftAnti) => {
                let mut row = r.clone();
                row.extend(bring.iter().map(|_| Val::Null));
                rows.push(row);
            }
            _ => {}
        }
    }
    if matches!(
        kind,
        JoinKind::RightOuter | JoinKind::FullOuter | JoinKind::RightAnti
    ) {
        for (j, r) in right.rows.iter().enumerate() {
            if used[j] {
                continue;
            }
            let mut row = vec![Val::Null; nl];
            // The keys of an unmatched right row fill the left key columns.
            for (&l, &k) in lk.iter().zip(&rk) {
                row[l] = r[k].clone();
            }
            row.extend(bring.iter().map(|&i| r[i].clone()));
            rows.push(row);
        }
    }
    if kind == JoinKind::LeftAnti || kind == JoinKind::RightAnti {
        // Anti joins keep only the side's own columns.
        let mut out = QTable {
            columns: unique_names(names),
            types,
            rows,
        };
        if kind == JoinKind::LeftAnti {
            let keep: Vec<usize> = (0..nl).collect();
            out = project(out, &keep);
        }
        return Ok(out);
    }
    Ok(QTable {
        columns: unique_names(names),
        types,
        rows,
    })
}

fn append(mut base: QTable, other: QTable) -> QTable {
    let mut map = Vec::with_capacity(other.columns.len());
    for (k, c) in other.columns.iter().enumerate() {
        let i = if let Some(i) = base.col(c) {
            i
        } else {
            base.columns.push(c.clone());
            base.types.push(other.types[k]);
            for r in &mut base.rows {
                r.push(Val::Null);
            }
            base.columns.len() - 1
        };
        if base.types[i] != other.types[k] {
            base.types[i] = ColType::Any;
        }
        map.push(i);
    }
    let n = base.columns.len();
    for r in other.rows {
        let mut row = vec![Val::Null; n];
        for (k, v) in r.into_iter().enumerate() {
            row[map[k]] = v;
        }
        base.rows.push(row);
    }
    base
}

/// The table of several files combined (folder source): `Source.Name` first, then the
/// columns of every file matched by name.
pub fn combine(files: Vec<(String, QTable)>) -> QTable {
    let mut out = QTable::new(vec!["Source.Name".into()], Vec::new());
    for (name, t) in files {
        let mut cols = vec!["Source.Name".to_owned()];
        cols.extend(t.columns.iter().cloned());
        let mut types = vec![ColType::Text];
        types.extend(t.types.iter().copied());
        let rows = t
            .rows
            .into_iter()
            .map(|r| {
                let mut row = vec![Val::Text(name.clone())];
                row.extend(r);
                row
            })
            .collect();
        out = append(
            out,
            QTable {
                columns: unique_names(cols),
                types,
                rows,
            },
        );
    }
    out
}
