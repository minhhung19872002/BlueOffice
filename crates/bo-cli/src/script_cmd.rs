//! `bo-cli script run`: run a BlueScript headless on a document (CORE-SCRIPT-004,
//! `bo_script`) and optionally save the result.

use bo_script::{run_script, Bridge, Document, Limits, RunOptions, RunReport};
use serde::Serialize;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

type Error = Box<dyn std::error::Error>;

/// Options of `script run`.
pub struct ScriptRun {
    pub input: PathBuf,
    pub script: PathBuf,
    pub out: Option<PathBuf>,
    pub timeout_ms: u64,
    pub memory_mb: usize,
    pub max_calls: u64,
    pub max_output_kb: usize,
    pub locale: String,
    pub json: bool,
}

/// What `--json` prints.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CliReport<'a> {
    app: &'static str,
    #[serde(flatten)]
    run: &'a RunReport,
    /// The document differs from the opened one (only known with `--out`).
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<bool>,
    /// File written (only when the script succeeded).
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<String>,
}

fn ext_of(p: &Path) -> String {
    p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Runs the command; `Ok(false)` when the script failed (the report is printed).
pub fn run(o: &ScriptRun) -> Result<bool, Error> {
    let bytes = std::fs::read(&o.input).map_err(|e| format!("{}: {e}", o.input.display()))?;
    let code =
        std::fs::read_to_string(&o.script).map_err(|e| format!("{}: {e}", o.script.display()))?;
    let code = code.strip_prefix('\u{feff}').unwrap_or(&code);
    let name = o
        .input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");
    let doc = Document::open(&bytes, name, &o.locale)?;
    let app = doc.app();
    let doc = Rc::new(RefCell::new(doc));
    let out_ext = o.out.as_deref().map(ext_of);
    // The package as opened, to tell whether the script changed anything.
    let before = match &out_ext {
        Some(ext) => Some(doc.borrow_mut().save(ext)?),
        None => None,
    };

    let mut opts = RunOptions::new(app);
    opts.decimal = if o.locale.eq_ignore_ascii_case("en-US") {
        '.'
    } else {
        ','
    };
    opts.limits = Limits {
        timeout: Duration::from_millis(o.timeout_ms.max(1)),
        memory_bytes: o.memory_mb.max(1) << 20,
        max_calls: o.max_calls.max(1),
        max_output_bytes: o.max_output_kb.max(1) << 10,
        ..Limits::default()
    };
    let bridge: Rc<RefCell<dyn Bridge>> = doc.clone();
    let report = run_script(code, bridge, &opts);

    let mut modified = None;
    let mut output = None;
    if let (true, Some(out), Some(ext)) = (report.ok, &o.out, &out_ext) {
        let after = doc.borrow_mut().save(ext)?;
        modified = Some(before.as_deref() != Some(after.as_slice()));
        std::fs::write(out, &after).map_err(|e| format!("{}: {e}", out.display()))?;
        output = Some(out.display().to_string());
    }

    if o.json {
        let r = CliReport {
            app: app.as_str(),
            run: &report,
            modified,
            output,
        };
        println!("{}", serde_json::to_string(&r)?);
    } else {
        for l in &report.logs {
            println!("[{}] {}", l.level, l.text);
        }
        if let Some(v) = &report.value {
            println!("→ {v}");
        }
        if let Some(e) = &report.error {
            let at = match (e.line, e.column) {
                (Some(l), Some(c)) => format!(" (line {l}, column {c})"),
                _ => String::new(),
            };
            eprintln!("{:?}: {}{at}", e.kind, e.message);
        }
        if let Some(out) = &output {
            println!("{} calls, {} ms → {out}", report.calls, report.ms);
        }
    }
    Ok(report.ok)
}
