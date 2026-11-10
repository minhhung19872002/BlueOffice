//! The workbook data model in BlueSheet (`docs/parity/BACKLOG.md` SHEET-DM-001): the
//! `model*` commands of the JSON bridge over `bo_sheet::datamodel`.
//!
//! * `modelInfo` — tables (sheet tables, in the model or not), relationships, calculated
//!   columns, measures, the functions of the expression language and the data model parts
//!   of other software (read only);
//! * `modelAddTable` / `modelRemoveTable`, `modelSetRelationship` / `modelDeleteRelationship`
//!   (`modelDetect`: cardinality of two columns), `modelSetMeasure` / `modelDeleteMeasure`,
//!   `modelSetColumn` / `modelDeleteColumn` — one undo step each; pivot tables over the model
//!   are re-rendered;
//! * `modelCheck` (syntax and names of an expression), `modelEvaluate` (value of an
//!   expression without filters);
//! * `modelPivotCreate` — a pivot table over the model (fields are then set with
//!   `pivotUpdate`).

use super::cmd::{At, Recalc};
use super::SheetEngine;
use crate::{EngineError, Result};
use bo_model::sheet::{CellRef, Workbook, Worksheet};
use bo_sheet::datamodel::eval::{check, Context, Evaluator, Kind, V};
use bo_sheet::datamodel::{
    detect, CalcColumn, Cardinality, DataModel, Measure, ModelData, ModelTable, Relationship,
};
use bo_sheet::pivot::{PivotLabels, PivotSpec};
use serde::Deserialize;
use serde_json::{json, Value};

fn yes() -> bool {
    true
}

/// A data model command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ModelCommand {
    /// The model and the sheet tables.
    ModelInfo,
    /// Add sheet table `name` to the model.
    ModelAddTable {
        /// Table name.
        name: String,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Remove a table (its relationships, calculated columns and measures go with it).
    ModelRemoveTable {
        /// Table name.
        name: String,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Cardinality of `fromTable[fromColumn]` → `toTable[toColumn]`.
    ModelDetect {
        /// First table.
        from_table: String,
        /// First column.
        from_column: String,
        /// Second table.
        to_table: String,
        /// Second column.
        to_column: String,
    },
    /// Add (`index` absent) or replace a relationship; the one side is detected.
    ModelSetRelationship {
        /// Relationship replaced.
        #[serde(default)]
        index: Option<usize>,
        /// First table.
        from_table: String,
        /// First column.
        from_column: String,
        /// Second table.
        to_table: String,
        /// Second column.
        to_column: String,
        /// Active (filters flow through it).
        #[serde(default = "yes")]
        active: bool,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Delete relationship `index`.
    ModelDeleteRelationship {
        /// Index.
        index: usize,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Check an expression: `{ ok, error?: { message, start, end } }`.
    ModelCheck {
        /// The expression.
        expression: String,
        /// Table of a calculated column (absent: a measure).
        #[serde(default)]
        table: Option<String>,
        /// Name of the measure being edited (self references).
        #[serde(default)]
        name: Option<String>,
    },
    /// Add or replace a measure.
    ModelSetMeasure {
        /// Measure replaced.
        #[serde(default)]
        original: Option<String>,
        /// Name.
        name: String,
        /// Home table.
        table: String,
        /// Expression.
        expression: String,
        /// Number format code.
        #[serde(default)]
        format: Option<String>,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Delete a measure.
    ModelDeleteMeasure {
        /// Name.
        name: String,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Add or replace a calculated column.
    ModelSetColumn {
        /// Column replaced (same table).
        #[serde(default)]
        original: Option<String>,
        /// Table.
        table: String,
        /// Name.
        name: String,
        /// Expression.
        expression: String,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Delete a calculated column.
    ModelDeleteColumn {
        /// Table.
        table: String,
        /// Name.
        name: String,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
    /// Value of an expression without filters: `{ value }` or `{ error }`.
    ModelEvaluate {
        /// The expression.
        expression: String,
    },
    /// Create a pivot table over the model: `{ sheet, index }`.
    ModelPivotCreate {
        /// Top-left cell; absent: a new sheet.
        #[serde(default)]
        dest: Option<At>,
        /// Pivot captions.
        #[serde(default)]
        labels: Option<PivotLabels>,
    },
}

impl ModelCommand {
    /// Parses a model command (`None`: another command).
    pub fn parse(v: &Value) -> Option<Result<ModelCommand>> {
        let name = v.get("cmd")?.as_str()?;
        if !name.starts_with("model") {
            return None;
        }
        Some(
            serde_json::from_value(v.clone())
                .map_err(|e| EngineError::Sheet(format!("bad command: {e}"))),
        )
    }

    fn edits(&self) -> bool {
        !matches!(
            self,
            ModelCommand::ModelInfo
                | ModelCommand::ModelDetect { .. }
                | ModelCommand::ModelCheck { .. }
                | ModelCommand::ModelEvaluate { .. }
        )
    }
}

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

fn value_json(v: &V) -> Value {
    match v {
        V::Blank => Value::Null,
        V::Num(n) => json!(n),
        V::Text(s) => json!(s),
        V::Bool(b) => json!(b),
        V::Err(e) => json!({ "error": e.as_str() }),
    }
}

/// Values of `table[column]` (all rows).
fn column_values(
    md: &ModelData,
    table: &str,
    column: &str,
) -> Result<Vec<bo_model::sheet::CellValue>> {
    let (t, c) = md
        .column(table, column)
        .ok_or_else(|| bad(format!("model: unknown column {table}[{column}]")))?;
    Ok(md.tables[t].rows.iter().map(|r| r[c].clone()).collect())
}

fn card_name(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ManyToOne => "manyToOne",
        Cardinality::OneToMany => "oneToMany",
        Cardinality::OneToOne => "oneToOne",
        Cardinality::ManyToMany => "manyToMany",
    }
}

/// The model information shown by the dialogs.
fn info_json(wb: &Workbook) -> Value {
    let dm = DataModel::read(wb);
    let md = ModelData::load_def(wb, dm.clone());
    let queries = super::query_cmd::read_defs(wb);
    let mut tables: Vec<Value> = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        for t in &ws.tables {
            let in_model = dm.table(&t.name);
            let query = queries.iter().find(|q| {
                q.load
                    .as_ref()
                    .is_some_and(|l| l.table.eq_ignore_ascii_case(&t.name))
            });
            let columns: Vec<Value> = match in_model {
                Some(mi) => {
                    let td = &md.tables[mi];
                    td.columns
                        .iter()
                        .enumerate()
                        .map(|(ci, c)| {
                            let calc = (ci >= td.source_columns).then(|| {
                                dm.columns.iter().find(|x| {
                                    x.table.eq_ignore_ascii_case(&t.name)
                                        && x.name.eq_ignore_ascii_case(c)
                                })
                            });
                            json!({
                                "name": c,
                                "calculated": calc.is_some(),
                                "expression": calc.flatten().map(|x| x.expression.clone()),
                            })
                        })
                        .collect()
                }
                None => t
                    .columns
                    .iter()
                    .map(|c| json!({ "name": c.name, "calculated": false }))
                    .collect(),
            };
            let header = t.header_row_count.unwrap_or(1);
            let totals = t.totals_row_count.unwrap_or(0);
            tables.push(json!({
                "name": t.name,
                "inModel": in_model.is_some(),
                "sheet": si,
                "sheetName": ws.name,
                "query": query.map(|q| q.name.clone()),
                "rows": (t.range.rows()).saturating_sub(header + totals),
                "columns": columns,
            }));
        }
    }
    // Model tables whose sheet table is gone.
    for t in &dm.tables {
        if bo_sheet::datamodel::find_sheet_table(wb, &t.name).is_none() {
            tables.push(json!({ "name": t.name, "inModel": true, "missing": true, "columns": [] }));
        }
    }
    let problem = |kind: &str, i: usize| {
        md.problems
            .iter()
            .find(|p| p.kind == kind && p.index == i)
            .map(|p| p.message.clone())
    };
    let relationships: Vec<Value> = dm
        .relationships
        .iter()
        .enumerate()
        .map(|(i, r)| {
            json!({
                "index": i,
                "fromTable": r.from_table,
                "fromColumn": r.from_column,
                "toTable": r.to_table,
                "toColumn": r.to_column,
                "active": r.active,
                "problem": problem("relationship", i),
            })
        })
        .collect();
    let measures: Vec<Value> = dm
        .measures
        .iter()
        .map(|m| {
            let error = check(&md, &m.expression, Kind::Measure, Some(&m.name))
                .err()
                .map(|e| e.message);
            json!({
                "name": m.name,
                "table": m.table,
                "expression": m.expression,
                "format": m.format,
                "error": error,
            })
        })
        .collect();
    let columns: Vec<Value> = dm
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "table": c.table,
                "name": c.name,
                "expression": c.expression,
                "error": problem("column", i),
            })
        })
        .collect();
    json!({
        "tables": tables,
        "relationships": relationships,
        "measures": measures,
        "columns": columns,
        "functions": bo_sheet::datamodel::expr::FUNCTIONS.iter().map(|(n, lo, hi)| json!({ "name": n, "min": lo, "max": hi })).collect::<Vec<_>>(),
        "pivots": dm.pivots.len(),
        "foreign": bo_sheet::datamodel::foreign_parts(wb),
    })
}

impl SheetEngine {
    /// Runs a model command (read-only workbooks refuse the editing ones).
    pub(crate) fn model_command_checked(&mut self, cmd: ModelCommand) -> Result<Value> {
        if self.sec.read_only && cmd.edits() {
            return Err(EngineError::Sheet(crate::security::READ_ONLY.into()));
        }
        self.model_command(cmd)
    }

    /// Change the model in one undo step and re-render the pivot tables over it.
    fn model_edit(
        &mut self,
        labels: Option<PivotLabels>,
        f: impl FnOnce(&Workbook, &mut DataModel) -> Result<()>,
    ) -> Result<Value> {
        let labels = self.pivot_labels(labels);
        let locale = self.locale;
        self.edit(0, Recalc::No, |wb, _| {
            let mut dm = DataModel::read(wb);
            f(wb, &mut dm)?;
            dm.write(wb);
            let mut l = labels;
            l.locale = Some(locale);
            // A report that no longer renders (its fields are gone) keeps its cells.
            let _ = bo_sheet::datamodel::pivot::render_all(wb, &l);
            Ok(())
        })?;
        self.invalidate(None);
        Ok(info_json(self.wb()?))
    }

    /// Runs a model command.
    #[allow(clippy::too_many_lines)]
    pub fn model_command(&mut self, cmd: ModelCommand) -> Result<Value> {
        match cmd {
            ModelCommand::ModelInfo => Ok(info_json(self.wb()?)),
            ModelCommand::ModelAddTable { name, labels } => self.model_edit(labels, |wb, dm| {
                let (si, ti) = bo_sheet::datamodel::find_sheet_table(wb, &name)
                    .ok_or_else(|| bad(format!("model: no table {name}")))?;
                if dm.table(&name).is_some() {
                    return Err(bad(format!("model: {name} is already in the model")));
                }
                dm.tables.push(ModelTable {
                    name: wb.sheets[si].tables[ti].name.clone(),
                });
                Ok(())
            }),
            ModelCommand::ModelRemoveTable { name, labels } => self.model_edit(labels, |_, dm| {
                let i = dm
                    .table(&name)
                    .ok_or_else(|| bad(format!("model: {name} is not in the model")))?;
                dm.tables.remove(i);
                let l = name.to_lowercase();
                dm.relationships.retain(|r| {
                    r.from_table.to_lowercase() != l && r.to_table.to_lowercase() != l
                });
                dm.columns.retain(|c| c.table.to_lowercase() != l);
                dm.measures.retain(|m| m.table.to_lowercase() != l);
                Ok(())
            }),
            ModelCommand::ModelDetect {
                from_table,
                from_column,
                to_table,
                to_column,
            } => {
                let md = ModelData::load(self.wb()?);
                let a = column_values(&md, &from_table, &from_column)?;
                let b = column_values(&md, &to_table, &to_column)?;
                let (card, unmatched) = detect(&a, &b);
                Ok(json!({ "cardinality": card_name(card), "unmatched": unmatched }))
            }
            ModelCommand::ModelSetRelationship {
                index,
                from_table,
                from_column,
                to_table,
                to_column,
                active,
                labels,
            } => self.model_edit(labels, |wb, dm| {
                if from_table.eq_ignore_ascii_case(&to_table) {
                    return Err(bad("model: a relationship joins two different tables"));
                }
                let md = ModelData::load_def(wb, dm.clone());
                let a = column_values(&md, &from_table, &from_column)?;
                let b = column_values(&md, &to_table, &to_column)?;
                let (card, _) = detect(&a, &b);
                let mut r = Relationship {
                    from_table,
                    from_column,
                    to_table,
                    to_column,
                    active,
                };
                match card {
                    Cardinality::ManyToOne | Cardinality::OneToOne => {}
                    Cardinality::OneToMany => {
                        std::mem::swap(&mut r.from_table, &mut r.to_table);
                        std::mem::swap(&mut r.from_column, &mut r.to_column);
                    }
                    Cardinality::ManyToMany => {
                        return Err(bad(
                            "model: many-to-many relationships are not supported (neither column has distinct values)",
                        ));
                    }
                }
                // Names as the tables spell them.
                for (t, c) in [
                    (&mut r.from_table, &mut r.from_column),
                    (&mut r.to_table, &mut r.to_column),
                ] {
                    if let Some((ti, ci)) = md.column(t, c) {
                        t.clone_from(&md.tables[ti].name);
                        c.clone_from(&md.tables[ti].columns[ci]);
                    }
                }
                let same = |x: &Relationship| {
                    (x.from_table.eq_ignore_ascii_case(&r.from_table)
                        && x.to_table.eq_ignore_ascii_case(&r.to_table))
                        || (x.from_table.eq_ignore_ascii_case(&r.to_table)
                            && x.to_table.eq_ignore_ascii_case(&r.from_table))
                };
                if r.active {
                    // One active relationship between two tables.
                    for (i, x) in dm.relationships.iter_mut().enumerate() {
                        if Some(i) != index && same(x) {
                            x.active = false;
                        }
                    }
                }
                match index {
                    Some(i) => {
                        let slot = dm
                            .relationships
                            .get_mut(i)
                            .ok_or_else(|| bad("model: no such relationship"))?;
                        *slot = r;
                    }
                    None => dm.relationships.push(r),
                }
                Ok(())
            }),
            ModelCommand::ModelDeleteRelationship { index, labels } => {
                self.model_edit(labels, |_, dm| {
                    if index >= dm.relationships.len() {
                        return Err(bad("model: no such relationship"));
                    }
                    dm.relationships.remove(index);
                    Ok(())
                })
            }
            ModelCommand::ModelCheck {
                expression,
                table,
                name,
            } => {
                let md = ModelData::load(self.wb()?);
                let kind = match &table {
                    Some(t) => Kind::Column(
                        md.table(t)
                            .ok_or_else(|| bad(format!("model: {t} is not in the model")))?,
                    ),
                    None => Kind::Measure,
                };
                Ok(match check(&md, &expression, kind, name.as_deref()) {
                    Ok(_) => json!({ "ok": true }),
                    Err(e) => json!({
                        "ok": false,
                        "error": { "message": e.message, "start": e.start, "end": e.end },
                    }),
                })
            }
            ModelCommand::ModelSetMeasure {
                original,
                name,
                table,
                expression,
                format,
                labels,
            } => self.model_edit(labels, |wb, dm| {
                let name = name.trim().to_owned();
                if name.is_empty() || name.contains(['[', ']']) {
                    return Err(bad("model: bad measure name"));
                }
                let prev = original.as_deref().and_then(|o| dm.measure(o));
                if dm.measure(&name).is_some_and(|i| Some(i) != prev) {
                    return Err(bad(format!("model: duplicate measure {name}")));
                }
                let home = dm
                    .table(&table)
                    .ok_or_else(|| bad(format!("model: {table} is not in the model")))?;
                let md = ModelData::load_def(wb, dm.clone());
                check(&md, &expression, Kind::Measure, Some(&name))
                    .map_err(|e| bad(format!("expression: {}", e.message)))?;
                let m = Measure {
                    name,
                    table: dm.tables[home].name.clone(),
                    expression,
                    format: format.filter(|f| !f.trim().is_empty()),
                };
                match prev {
                    Some(i) => dm.measures[i] = m,
                    None => dm.measures.push(m),
                }
                Ok(())
            }),
            ModelCommand::ModelDeleteMeasure { name, labels } => self.model_edit(labels, |_, dm| {
                let i = dm
                    .measure(&name)
                    .ok_or_else(|| bad(format!("model: no measure {name}")))?;
                dm.measures.remove(i);
                Ok(())
            }),
            ModelCommand::ModelSetColumn {
                original,
                table,
                name,
                expression,
                labels,
            } => self.model_edit(labels, |wb, dm| {
                let name = name.trim().to_owned();
                if name.is_empty() || name.contains(['[', ']']) {
                    return Err(bad("model: bad column name"));
                }
                let t = dm
                    .table(&table)
                    .ok_or_else(|| bad(format!("model: {table} is not in the model")))?;
                let tname = dm.tables[t].name.clone();
                let same = |c: &CalcColumn, n: &str| {
                    c.table.eq_ignore_ascii_case(&tname) && c.name.eq_ignore_ascii_case(n)
                };
                let prev = original
                    .as_deref()
                    .and_then(|o| dm.columns.iter().position(|c| same(c, o)));
                // Check against the model without the column being replaced.
                let mut probe = dm.clone();
                if let Some(i) = prev {
                    probe.columns.truncate(i);
                }
                let md = ModelData::load_def(wb, probe);
                if md.tables[t].column(&name).is_some() {
                    return Err(bad(format!("model: duplicate column {name}")));
                }
                check(&md, &expression, Kind::Column(t), None)
                    .map_err(|e| bad(format!("expression: {}", e.message)))?;
                let c = CalcColumn {
                    table: tname.clone(),
                    name,
                    expression,
                };
                match prev {
                    Some(i) => dm.columns[i] = c,
                    None => dm.columns.push(c),
                }
                Ok(())
            }),
            ModelCommand::ModelDeleteColumn {
                table,
                name,
                labels,
            } => self.model_edit(labels, |_, dm| {
                let i = dm
                    .columns
                    .iter()
                    .position(|c| {
                        c.table.eq_ignore_ascii_case(&table) && c.name.eq_ignore_ascii_case(&name)
                    })
                    .ok_or_else(|| bad(format!("model: no column {table}[{name}]")))?;
                dm.columns.remove(i);
                Ok(())
            }),
            ModelCommand::ModelEvaluate { expression } => {
                let md = ModelData::load(self.wb()?);
                Ok(match check(&md, &expression, Kind::Measure, None) {
                    Ok(e) => {
                        json!({ "value": value_json(&Evaluator::new(&md).value(&e, &Context::default())) })
                    }
                    Err(e) => json!({ "error": e.message }),
                })
            }
            ModelCommand::ModelPivotCreate { dest, labels } => {
                let labels = self.pivot_labels(labels);
                let first = dest.as_ref().map_or(0, |d| d.sheet);
                let out = self.edit(first, Recalc::All, |wb, _| {
                    super::protect::check_structure(wb).or_else(|e| {
                        if dest.is_some() {
                            Ok(())
                        } else {
                            Err(e)
                        }
                    })?;
                    let (sheet, at) = if let Some(a) = &dest {
                        if a.sheet >= wb.sheets.len() {
                            return Err(bad("no such sheet"));
                        }
                        (a.sheet, CellRef::new(a.row, a.col))
                    } else {
                        let mut n = wb.sheets.len() + 1;
                        let name = loop {
                            let cand = format!("Sheet{n}");
                            if wb.sheets.iter().all(|s| !s.name.eq_ignore_ascii_case(&cand)) {
                                break cand;
                            }
                            n += 1;
                        };
                        let mut ws = Worksheet::new(&name);
                        ws.sheet_id = wb.sheets.iter().map(|s| s.sheet_id).max().unwrap_or(0) + 1;
                        wb.sheets.push(ws);
                        (wb.sheets.len() - 1, CellRef::new(2, 0))
                    };
                    let index = bo_sheet::datamodel::pivot::create_model_pivot(
                        wb,
                        sheet,
                        at,
                        &PivotSpec::default(),
                        &labels,
                    )
                    .map_err(bad)?;
                    Ok(json!({ "sheet": sheet, "index": index }))
                })?;
                self.invalidate(None);
                Ok(out)
            }
        }
    }
}
