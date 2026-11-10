//! Evaluation of queries against the bytes the host provides.

use crate::def::{ColumnType, ItemKind, Query, Source, Step};
use crate::value::{ColType, QTable, Val};
use crate::QueryError;
use bo_model::sheet::Workbook;
use std::cell::RefCell;
use std::collections::HashMap;

/// What a query reads: source bytes by key, the other queries of the workbook, and the
/// workbook reader of the host.
pub trait Resolver {
    /// Bytes of a file or web page source (`None`: the host has not provided them).
    fn file(&self, key: &str) -> Option<&[u8]>;
    /// Files of a folder source, name and bytes (`None`: not provided).
    fn folder(&self, key: &str) -> Option<Vec<(&str, &[u8])>>;
    /// Another query of the workbook by name (case-insensitive).
    fn query(&self, name: &str) -> Option<Query>;
    /// Read a workbook file (the host knows every format; this default reads .xlsx).
    fn workbook(&self, bytes: &[u8], _name: &str) -> Result<Workbook, String> {
        bo_xlsx::read_xlsx(bytes).map_err(|e| e.to_string())
    }
}

/// An error and the step it happened at (`None`: the source).
#[derive(Debug, Clone, PartialEq)]
pub struct StepError {
    /// Index of the failing step.
    pub step: Option<usize>,
    /// The error.
    pub error: QueryError,
}

/// Evaluates queries, each at most once (results of queries read by `merge` / `append`
/// are kept), refusing cycles.
pub struct Evaluator<'a> {
    r: &'a dyn Resolver,
    stack: RefCell<Vec<String>>,
    cache: RefCell<HashMap<String, QTable>>,
}

impl<'a> Evaluator<'a> {
    /// An evaluator over `r`.
    pub fn new(r: &'a dyn Resolver) -> Self {
        Evaluator {
            r,
            stack: RefCell::new(Vec::new()),
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// The table of `q` after its first `upto` steps (all when `None`).
    pub fn run(&self, q: &Query, upto: Option<usize>) -> Result<QTable, StepError> {
        let key = q.name.to_lowercase();
        if self.stack.borrow().contains(&key) {
            return Err(StepError {
                step: None,
                error: QueryError::Cycle(q.name.clone()),
            });
        }
        self.stack.borrow_mut().push(key);
        let out = self.run_inner(q, upto);
        self.stack.borrow_mut().pop();
        out
    }

    fn run_inner(&self, q: &Query, upto: Option<usize>) -> Result<QTable, StepError> {
        let mut t = self
            .source(&q.source)
            .map_err(|error| StepError { step: None, error })?;
        let n = upto.unwrap_or(q.steps.len()).min(q.steps.len());
        let others = |name: &str| self.named(name);
        for (i, step) in q.steps[..n].iter().enumerate() {
            t = crate::steps::apply(step, t, &others).map_err(|error| StepError {
                step: Some(i),
                error,
            })?;
        }
        Ok(t)
    }

    /// The full result of the query named `name`.
    pub fn named(&self, name: &str) -> Result<QTable, QueryError> {
        let key = name.to_lowercase();
        if let Some(t) = self.cache.borrow().get(&key) {
            return Ok(t.clone());
        }
        let q = self
            .r
            .query(name)
            .ok_or_else(|| QueryError::Query(name.to_owned()))?;
        let t = self.run(&q, None).map_err(|e| match e.error {
            e @ QueryError::Cycle(_) => e,
            other => QueryError::Dependency(name.to_owned(), Box::new(other)),
        })?;
        self.cache.borrow_mut().insert(key, t.clone());
        Ok(t)
    }

    fn file(&self, key: &str) -> Result<&'a [u8], QueryError> {
        self.r
            .file(key)
            .ok_or_else(|| QueryError::MissingSource(key.to_owned()))
    }

    /// The table of a source.
    pub fn source(&self, s: &Source) -> Result<QTable, QueryError> {
        match s {
            Source::Csv {
                file,
                delimiter,
                encoding,
            } => Ok(crate::text::read_csv(
                self.file(file)?,
                delimiter.as_deref(),
                encoding.as_deref(),
            )?
            .0),
            Source::Json { file, path } => {
                crate::json_src::read_json(self.file(file)?, path.as_deref())
            }
            Source::Xml { file, element } => {
                crate::xml_src::read_xml(self.file(file)?, element.as_deref())
            }
            Source::Workbook {
                file,
                item,
                item_kind,
            } => {
                let wb = self
                    .r
                    .workbook(self.file(file)?, file)
                    .map_err(QueryError::Source)?;
                crate::workbook_src::read_item(&wb, item, *item_kind)
            }
            Source::Web { url, table } => {
                let html = crate::text::decode(self.file(url)?, None).text;
                let tables = crate::html::tables(&html);
                tables
                    .get(*table)
                    .map(crate::html::WebTable::to_table)
                    .ok_or_else(|| QueryError::Source(format!("no table {table} on the page")))
            }
            Source::Folder {
                folder,
                file_kind,
                delimiter,
                item,
                promote_headers,
            } => {
                let files = self
                    .r
                    .folder(folder)
                    .ok_or_else(|| QueryError::MissingSource(folder.clone()))?;
                let mut tables = Vec::new();
                for (name, bytes) in files {
                    if !kind_matches(file_kind, name) {
                        continue;
                    }
                    let mut t = match file_kind.as_str() {
                        "csv" => crate::text::read_csv(bytes, delimiter.as_deref(), None)?.0,
                        "json" => crate::json_src::read_json(bytes, item.as_deref())?,
                        "xml" => crate::xml_src::read_xml(bytes, item.as_deref())?,
                        _ => {
                            let wb = self.r.workbook(bytes, name).map_err(QueryError::Source)?;
                            let first = crate::workbook_src::items(&wb)
                                .into_iter()
                                .find(|i| i.kind == ItemKind::Sheet)
                                .map(|i| i.name)
                                .unwrap_or_default();
                            let target = item.clone().unwrap_or(first);
                            let kind = if wb.sheet(&target).is_some() {
                                ItemKind::Sheet
                            } else {
                                ItemKind::Table
                            };
                            crate::workbook_src::read_item(&wb, &target, kind)?
                        }
                    };
                    if *promote_headers && matches!(file_kind.as_str(), "csv" | "workbook") {
                        t = crate::steps::apply(&Step::PromoteHeaders, t, &|_| {
                            Err(QueryError::Step("no queries".into()))
                        })?;
                    }
                    tables.push((name.to_owned(), t));
                }
                Ok(crate::steps::combine(tables))
            }
            Source::Query { query } => self.named(query),
        }
    }
}

/// Does a file name belong to a folder source of `kind`?
pub fn kind_matches(kind: &str, name: &str) -> bool {
    let lower = name.to_lowercase();
    let ext = lower.rsplit_once('.').map_or("", |(_, e)| e);
    match kind {
        "csv" => matches!(ext, "csv" | "txt" | "tsv"),
        "json" => ext == "json",
        "xml" => ext == "xml",
        _ => matches!(ext, "xlsx" | "xlsm" | "xls" | "xlsb" | "ods"),
    }
}

/// Steps proposed for a new query (as a person would add them first): promote headers when
/// the first row reads as column names, then the detected column types.
pub fn suggest_steps(t: &QTable, promote: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut t = t.clone();
    if promote && looks_like_header(&t) {
        steps.push(Step::PromoteHeaders);
        if let Ok(p) = crate::steps::apply(&Step::PromoteHeaders, t.clone(), &|_| {
            Err(QueryError::Step(String::new()))
        }) {
            t = p;
        }
    }
    let columns: Vec<ColumnType> = t
        .columns
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let ty = ColType::detect(t.rows.iter().take(1000).map(|r| &r[i]));
            let already = t.rows.iter().all(|r| {
                matches!(
                    (&r[i], ty),
                    (Val::Num(_), ColType::Number | ColType::Integer)
                        | (Val::Bool(_), ColType::Bool)
                        | (Val::Date(_), ColType::Date)
                        | (Val::Null, _)
                )
            });
            (!matches!(ty, ColType::Any | ColType::Text) && !already).then(|| ColumnType {
                column: c.clone(),
                ty,
            })
        })
        .collect();
    if !columns.is_empty() {
        steps.push(Step::ChangeType {
            columns,
            decimal: None,
            month_first: false,
        });
    }
    steps
}

fn looks_like_header(t: &QTable) -> bool {
    let Some(first) = t.rows.first() else {
        return false;
    };
    if t.rows.len() < 2 {
        return false;
    }
    let all_text = first
        .iter()
        .all(|v| matches!(v, Val::Text(s) if !s.trim().is_empty() && crate::value::parse_number(s, '.').is_none()));
    let mut seen = std::collections::HashSet::new();
    all_text
        && first
            .iter()
            .all(|v| seen.insert(v.display().to_lowercase()))
}
