//! `bo-cli fidelity-report`: the fidelity report of prompt §7 (round trip of the
//! corpus + OpenXmlValidator → `docs/fidelity-report.md`, `tests/fidelity/history.csv`).
//!
//! The report itself is produced by `scripts/fidelity-report.mjs` (Node ≥ 22); this
//! command finds the script in the repository and runs it with *this* executable as
//! the converter (`BO_CLI`), so a release build reports on itself.

use std::path::{Path, PathBuf};
use std::process::Command;

const SCRIPT: &str = "scripts/fidelity-report.mjs";

/// Options of `bo-cli fidelity-report`.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Skip OpenXmlValidator (conversion only).
    pub no_validate: bool,
    /// Corpus root with `doc/`, `sheet/` and `slide/`.
    pub corpus: Option<PathBuf>,
    /// Markdown report to write.
    pub report: Option<PathBuf>,
    /// History CSV to append to.
    pub history: Option<PathBuf>,
    /// Directory of the round-tripped files.
    pub work: Option<PathBuf>,
    /// Report script (default: `scripts/fidelity-report.mjs` of the repository).
    pub script: Option<PathBuf>,
}

/// `scripts/fidelity-report.mjs` above the current directory, the executable or
/// the crate sources.
fn locate_script() -> Option<PathBuf> {
    let mut starts: Vec<PathBuf> = Vec::new();
    if let Ok(d) = std::env::current_dir() {
        starts.push(d);
    }
    if let Some(d) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        starts.push(d);
    }
    starts.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    starts
        .iter()
        .flat_map(|s| s.ancestors())
        .map(|d| d.join(SCRIPT))
        .find(|p| p.is_file())
}

/// The `node` arguments for `opts` (script first).
fn script_args(script: &Path, opts: &Options) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![script.into()];
    if opts.no_validate {
        args.push("--no-validate".into());
    }
    for (flag, value) in [
        ("--corpus", &opts.corpus),
        ("--report", &opts.report),
        ("--history", &opts.history),
        ("--work", &opts.work),
    ] {
        if let Some(v) = value {
            args.push(flag.into());
            args.push(v.into());
        }
    }
    args
}

/// Runs the report script; fails when it is missing or exits with an error.
pub fn run(opts: &Options) -> Result<(), String> {
    let script = match &opts.script {
        Some(s) if s.is_file() => s.clone(),
        Some(s) => return Err(format!("script {} does not exist", s.display())),
        None => locate_script()
            .ok_or_else(|| format!("{SCRIPT} not found above the current directory"))?,
    };
    let exe = std::env::current_exe().map_err(|e| format!("current executable: {e}"))?;
    let status = Command::new("node")
        .args(script_args(&script, opts))
        .env("BO_CLI", &exe)
        .status()
        .map_err(|e| format!("cannot run node (Node.js ≥ 22 is needed): {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} failed ({status})", script.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_is_found_from_the_crate() {
        assert!(locate_script().is_some_and(|p| p.ends_with(SCRIPT)));
    }

    #[test]
    fn options_become_script_flags() {
        let opts = Options {
            no_validate: true,
            corpus: Some(PathBuf::from("c")),
            history: Some(PathBuf::from("h.csv")),
            ..Options::default()
        };
        let args: Vec<String> = script_args(Path::new("s.mjs"), &opts)
            .into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "s.mjs",
                "--no-validate",
                "--corpus",
                "c",
                "--history",
                "h.csv"
            ]
        );
    }
}
