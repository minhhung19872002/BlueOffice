//! `bo-cli validate`: structural checks of OOXML packages that need no .NET, plus
//! the DocumentFormat.OpenXml schema validation of `tools/BlueOffice.Validate`
//! (prompt §7 "Validation") when a .NET SDK and the tool are available.
//!
//! Structural checks (ECMA-376 Part 2, Open Packaging Conventions):
//! - the file is a ZIP package with a `[Content_Types].xml` part (§10.1.2.2);
//! - every part has a content type, from an `Override` or a `Default` (§10.1.2.4);
//! - `Override` elements name existing parts, without duplicates (§10.1.2.2.3);
//! - every XML part (content type `…xml`, or a `.xml` / `.rels` name) is well-formed;
//! - relationships parts parse, have unique `Id`s (§9.3.2.2) and their internal targets
//!   resolve to parts of the package (§9.3.4);
//! - the package relationships name the main part (`officeDocument`, ECMA-376 Part 1
//!   §11.3.10 / §12.3.23 / §13.3.6) with the content type of a document, workbook
//!   or presentation; a workbook relates at least one sheet (§18.2.20 sheets) and a
//!   presentation at least one slide master (§19.3.1.42 sldMasterIdLst).

use bo_opc::{ContentTypes, Package, TargetMode};
use quick_xml::events::Event;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Extensions of the OOXML packages collected from a directory.
const EXTENSIONS: &[&str] = &[
    "docx", "docm", "dotx", "dotm", "xlsx", "xlsm", "xltx", "xltm", "pptx", "pptm", "potx", "potm",
    "ppsx", "ppsm",
];

/// Severity of a finding: errors fail the run, warnings are reported only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One finding of a check.
#[derive(Debug, Clone)]
pub struct Issue {
    pub severity: Severity,
    /// `structure` (built-in checks) or `openxml` (OpenXmlValidator).
    pub source: &'static str,
    /// Part name (empty for the package as a whole).
    pub part: String,
    pub message: String,
}

/// Findings for one file.
#[derive(Debug, Clone)]
pub struct FileReport {
    pub file: PathBuf,
    pub issues: Vec<Issue>,
}

impl FileReport {
    fn errors(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }
}

/// When to run the OpenXmlValidator stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenXmlMode {
    /// Run it when a validator is found, otherwise note that only structural checks ran.
    Auto,
    /// Fail when no validator can be run.
    Require,
    /// Structural checks only.
    Off,
}

impl OpenXmlMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "require" => Some(Self::Require),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

/// Options of the `validate` command.
pub struct Options {
    pub inputs: Vec<PathBuf>,
    pub openxml: OpenXmlMode,
    /// `--validator`, else the `BO_VALIDATE` environment variable.
    pub validator: Option<PathBuf>,
    /// `--format` of OpenXmlValidator (`Office2019` when `None`).
    pub file_format: Option<String>,
    /// Also validate the mc:Choice branches (`--choice` of the validator).
    pub choice: bool,
    pub json: bool,
}

/// Run the command; `Err` when a file has errors or the validator cannot run as asked.
pub fn run(opts: &Options) -> Result<(), String> {
    let files = collect_files(&opts.inputs)?;
    if files.is_empty() {
        return Err("no OOXML packages (.docx/.xlsx/.pptx/…) found".into());
    }
    let mut reports: Vec<FileReport> = files
        .iter()
        .map(|f| FileReport {
            file: f.clone(),
            issues: match std::fs::read(f) {
                Ok(bytes) => check_package(&bytes, f),
                Err(e) => vec![error("", format!("cannot read file: {e}"))],
            },
        })
        .collect();

    let openxml = if opts.openxml == OpenXmlMode::Off {
        "off".to_owned()
    } else {
        match locate_validator(opts.validator.clone()) {
            Ok(cmd) => {
                let results =
                    run_validator(&cmd, &opts.inputs, opts.file_format.as_deref(), opts.choice)?;
                merge_openxml(&mut reports, results);
                "ran".to_owned()
            }
            Err(reason) if opts.openxml == OpenXmlMode::Require || opts.validator.is_some() => {
                return Err(format!("OpenXmlValidator cannot run: {reason}"));
            }
            Err(reason) => {
                eprintln!(
                    "note: OpenXmlValidator not run ({reason}); only structural checks ran. \
                     Install the .NET 8 SDK or set BO_VALIDATE / --validator to tools/BlueOffice.Validate \
                     (a project directory, .csproj, .dll or executable)."
                );
                format!("skipped: {reason}")
            }
        }
    };

    let valid = reports.iter().filter(|r| r.errors() == 0).count();
    if opts.json {
        println!("{}", report_json(&reports, &openxml, valid));
    } else {
        print_text(&reports, &openxml, valid);
    }
    if valid == reports.len() {
        Ok(())
    } else {
        Err(format!(
            "{} of {} files have errors",
            reports.len() - valid,
            reports.len()
        ))
    }
}

fn error(part: &str, message: impl Into<String>) -> Issue {
    Issue {
        severity: Severity::Error,
        source: "structure",
        part: part.to_owned(),
        message: message.into(),
    }
}

fn warning(part: &str, message: impl Into<String>) -> Issue {
    Issue {
        severity: Severity::Warning,
        ..error(part, message)
    }
}

/// Files named on the command line (any extension) plus the OOXML packages found
/// recursively in directories, sorted and without duplicates.
fn collect_files(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for input in inputs {
        if input.is_dir() {
            walk(input, &mut out).map_err(|e| format!("{}: {e}", input.display()))?;
        } else if input.is_file() {
            out.push(input.clone());
        } else {
            return Err(format!("{}: no such file or directory", input.display()));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            walk(&p, out)?;
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        {
            out.push(p);
        }
    }
    Ok(())
}

/// Built-in structural checks of one package.
pub fn check_package(bytes: &[u8], file: &Path) -> Vec<Issue> {
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return vec![error(
            "",
            "compound file (encrypted package or legacy binary format), not an OPC ZIP package",
        )];
    }
    let pkg = match Package::read(bytes, &bo_opc::Limits::default()) {
        Ok(p) => p,
        Err(e) => return vec![error("", format!("not a valid OPC package: {e}"))],
    };
    let mut issues = Vec::new();
    let ct = match pkg.content_types() {
        Ok(ct) => ct,
        Err(e) => {
            issues.push(error("/[Content_Types].xml", e.to_string()));
            return issues;
        }
    };
    let names: HashSet<String> = pkg
        .parts()
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();
    check_content_types(&pkg, &ct, &names, &mut issues);
    for part in pkg.parts() {
        if is_xml_part(&part.name, ct.get(&part.name)) {
            if let Err(m) = well_formed(&part.data) {
                issues.push(error(&part.name, format!("XML is not well-formed: {m}")));
            }
        }
    }
    check_relationships(&pkg, &names, &mut issues);
    check_main_part(&pkg, &ct, file, &mut issues);
    issues
}

/// §10.1.2: every part has a content type; overrides name existing parts once.
fn check_content_types(
    pkg: &Package,
    ct: &ContentTypes,
    names: &HashSet<String>,
    issues: &mut Vec<Issue>,
) {
    const CT: &str = "/[Content_Types].xml";
    for part in pkg.parts() {
        if part.name == CT {
            continue;
        }
        if ct.get(&part.name).is_none() {
            issues.push(error(
                &part.name,
                "part has no content type (no Override and no Default for its extension)",
            ));
        }
    }
    let mut seen = HashSet::new();
    for (name, _) in &ct.overrides {
        let lower = name.to_ascii_lowercase();
        if !seen.insert(lower.clone()) {
            issues.push(error(CT, format!("duplicate Override for {name}")));
        }
        if !names.contains(&lower) {
            issues.push(warning(CT, format!("Override names a missing part {name}")));
        }
    }
    let mut exts = HashSet::new();
    for (ext, _) in &ct.defaults {
        if !exts.insert(ext.to_ascii_lowercase()) {
            issues.push(error(CT, format!("duplicate Default for extension {ext}")));
        }
    }
}

fn is_xml_part(name: &str, content_type: Option<&str>) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".xml")
        || lower.ends_with(".rels")
        || content_type.is_some_and(|t| {
            let t = t.split(';').next().unwrap_or(t).trim();
            t.ends_with("+xml") || t.ends_with("/xml")
        })
}

/// XML 1.0 well-formedness as far as the parser sees it: one root element, matching
/// end tags, well-formed attributes (no duplicates) and known entity references.
fn well_formed(data: &[u8]) -> Result<(), String> {
    let mut r = bo_opc::xml::XmlReader::new(data, "");
    let mut roots = 0usize;
    loop {
        let depth = r.depth();
        let ev = r.read().map_err(|e| e.to_string())?;
        match ev {
            Event::Start(e) | Event::Empty(e) => {
                if depth == 0 {
                    roots += 1;
                }
                for a in e.attributes() {
                    a.map_err(|e| e.to_string())?;
                }
            }
            Event::Text(t) => {
                // quick-xml has already checked the text is UTF-8.
                if depth == 0 && !t.bytes().all(|b| b.is_ascii_whitespace()) {
                    return Err("text outside the root element".into());
                }
            }
            Event::GeneralRef(g) => {
                let name: &str = &g;
                let known = matches!(name, "amp" | "lt" | "gt" | "quot" | "apos");
                if !known && g.resolve_char_ref().map_err(|e| e.to_string())?.is_none() {
                    return Err(format!("undefined entity &{name};"));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if r.depth() != 0 {
        return Err("unexpected end of document (unclosed element)".into());
    }
    match roots {
        1 => Ok(()),
        0 => Err("no root element".into()),
        n => Err(format!("{n} root elements")),
    }
}

/// Source part of a relationships part name (`/word/_rels/document.xml.rels` →
/// `/word/document.xml`, `/_rels/.rels` → `/`).
fn rels_source(name: &str) -> Option<String> {
    let (dir, file) = name.rsplit_once('/')?;
    let dir = dir
        .strip_suffix("/_rels")
        .or(if dir == "/_rels" { Some("") } else { None })?;
    let file = file.strip_suffix(".rels")?;
    if dir.is_empty() && file.is_empty() {
        return Some("/".into());
    }
    Some(format!("{dir}/{file}"))
}

/// §9.3: every relationships part parses, ids are unique and internal targets exist.
fn check_relationships(pkg: &Package, names: &HashSet<String>, issues: &mut Vec<Issue>) {
    for part in pkg.parts() {
        if !part.name.to_ascii_lowercase().ends_with(".rels") {
            continue;
        }
        let Some(source) = rels_source(&part.name) else {
            issues.push(warning(
                &part.name,
                "relationships part outside a _rels folder",
            ));
            continue;
        };
        if source != "/" && !names.contains(&source.to_ascii_lowercase()) {
            issues.push(warning(
                &part.name,
                format!("relationships part of a missing source part {source}"),
            ));
        }
        let rels = match bo_opc::parse_relationships(&part.data, &part.name) {
            Ok(r) => r,
            Err(e) => {
                issues.push(error(&part.name, e.to_string()));
                continue;
            }
        };
        let mut ids = HashSet::new();
        for rel in &rels {
            if !ids.insert(rel.id.as_str()) {
                issues.push(error(
                    &part.name,
                    format!("duplicate relationship Id {}", rel.id),
                ));
            }
            if rel.mode == TargetMode::External {
                continue;
            }
            let target = rel.target.split('#').next().unwrap_or("");
            if target.is_empty() {
                continue; // same-part fragment reference
            }
            let resolved = bo_opc::resolve_target(&source, &percent_decode(target));
            if !names.contains(&resolved.to_ascii_lowercase()) {
                issues.push(error(
                    &part.name,
                    format!(
                        "relationship {} ({}) targets missing part {resolved}",
                        rel.id,
                        short_type(&rel.rel_type)
                    ),
                ));
            }
        }
    }
}

/// Decode `%XX` escapes of a part-name IRI (ECMA-376 Part 2 §9.1.1.1).
fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_owned();
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn short_type(t: &str) -> &str {
    t.rsplit('/').next().unwrap_or(t)
}

/// Kind of document by main-part content type.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Doc,
    Sheet,
    Slide,
}

fn family_of_content_type(t: &str) -> Option<Family> {
    if !t.ends_with(".main+xml") {
        return None;
    }
    if t.contains("wordprocessingml") || t.contains("ms-word") {
        Some(Family::Doc)
    } else if t.contains("spreadsheetml") || t.contains("ms-excel") {
        Some(Family::Sheet)
    } else if t.contains("presentationml") || t.contains("ms-powerpoint") {
        Some(Family::Slide)
    } else {
        None
    }
}

fn family_of_extension(file: &Path) -> Option<Family> {
    let ext = file.extension()?.to_str()?.to_ascii_lowercase();
    match ext.get(..2)? {
        "do" => Some(Family::Doc),
        "xl" => Some(Family::Sheet),
        "pp" | "po" => Some(Family::Slide),
        _ => None,
    }
}

/// The package names a document / workbook / presentation main part that exists.
fn check_main_part(pkg: &Package, ct: &ContentTypes, file: &Path, issues: &mut Vec<Issue>) {
    const ROOT: &str = "/_rels/.rels";
    if pkg.part(ROOT).is_none() {
        issues.push(error(ROOT, "package relationships part is missing"));
        return;
    }
    let Ok(rels) = pkg.relationships("/") else {
        return; // reported by check_relationships
    };
    let Some(main) = rels
        .iter()
        .find(|r| r.rel_type.ends_with("/officeDocument") && r.mode == TargetMode::Internal)
    else {
        issues.push(error(
            ROOT,
            "no officeDocument relationship to the main part",
        ));
        return;
    };
    let name = bo_opc::resolve_target("/", &main.target);
    if pkg.part(&name).is_none() {
        return; // reported as a missing target
    }
    let t = ct.get(&name).unwrap_or("");
    let Some(family) = family_of_content_type(t) else {
        issues.push(error(
            &name,
            format!("main part content type {t:?} is not a document, workbook or presentation"),
        ));
        return;
    };
    if family_of_extension(file).is_some_and(|f| f != family) {
        issues.push(warning(
            &name,
            format!("main part content type {t} does not match the file extension"),
        ));
    }
    let Ok(main_rels) = pkg.relationships(&name) else {
        return;
    };
    let has = |suffixes: &[&str]| {
        main_rels
            .iter()
            .any(|r| suffixes.iter().any(|s| r.rel_type.ends_with(s)))
    };
    match family {
        Family::Sheet if !has(&["/worksheet", "/chartsheet", "/dialogsheet"]) => {
            issues.push(error(&name, "workbook relates no sheet (§18.2.20 sheets)"));
        }
        Family::Slide if !has(&["/slideMaster"]) => issues.push(error(
            &name,
            "presentation relates no slide master (§19.3.1.42 sldMasterIdLst)",
        )),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// OpenXmlValidator (tools/BlueOffice.Validate)
// ---------------------------------------------------------------------------

/// How to start the validator.
#[derive(Debug)]
pub struct ValidatorCmd {
    program: PathBuf,
    args: Vec<String>,
}

const PROJECT: &str = "tools/BlueOffice.Validate/BlueOffice.Validate.csproj";

/// Find the validator: `explicit` (`--validator`, else `BO_VALIDATE`) or the project
/// in the repository around the current directory / executable / build tree.
pub fn locate_validator(explicit: Option<PathBuf>) -> Result<ValidatorCmd, String> {
    let explicit = explicit.or_else(|| {
        std::env::var_os("BO_VALIDATE")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    });
    if let Some(path) = explicit {
        if !path.exists() {
            return Err(format!(
                "validator {} (from --validator / BO_VALIDATE) does not exist",
                path.display()
            ));
        }
        return command_for(&path);
    }
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
    for start in starts {
        for dir in start.ancestors() {
            let project = dir.join(PROJECT);
            if project.is_file() {
                return command_for(&project);
            }
        }
    }
    Err(format!(
        "{PROJECT} not found above the current directory or the bo-cli executable"
    ))
}

fn command_for(path: &Path) -> Result<ValidatorCmd, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let project = if path.is_dir() {
        let found = std::fs::read_dir(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "csproj"));
        Some(found.ok_or_else(|| format!("{} contains no .csproj", path.display()))?)
    } else if ext.as_deref() == Some("csproj") {
        Some(path.to_path_buf())
    } else {
        None
    };
    if let Some(project) = project {
        require_dotnet()?;
        return Ok(ValidatorCmd {
            program: "dotnet".into(),
            args: vec![
                "run".into(),
                "--project".into(),
                project.to_string_lossy().into_owned(),
                "-c".into(),
                "Release".into(),
                "--".into(),
            ],
        });
    }
    if ext.as_deref() == Some("dll") {
        require_dotnet()?;
        return Ok(ValidatorCmd {
            program: "dotnet".into(),
            args: vec![path.to_string_lossy().into_owned()],
        });
    }
    Ok(ValidatorCmd {
        program: path.to_path_buf(),
        args: Vec::new(),
    })
}

fn require_dotnet() -> Result<(), String> {
    match Command::new("dotnet")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("`dotnet --version` failed ({s})")),
        Err(e) => Err(format!("the .NET SDK (`dotnet`) is not on PATH: {e}")),
    }
}

/// Validator result per file: (file as printed by the validator, issues).
type OpenXmlResults = Vec<(PathBuf, Vec<Issue>)>;

fn run_validator(
    cmd: &ValidatorCmd,
    inputs: &[PathBuf],
    format: Option<&str>,
    choice: bool,
) -> Result<OpenXmlResults, String> {
    let json = std::env::temp_dir().join(format!(
        "bo-validate-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let mut c = Command::new(&cmd.program);
    c.args(&cmd.args).args(inputs).arg("--json").arg(&json);
    if let Some(f) = format {
        c.args(["--format", f]);
    }
    if choice {
        c.arg("--choice");
    }
    // The validator prints its own summary; ours (with the structural checks) replaces it.
    let status = c
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("cannot start {}: {e}", cmd.program.display()))?;
    let text = std::fs::read_to_string(&json).map_err(|_| {
        format!(
            "OpenXmlValidator ({}) wrote no report (exit {status})",
            cmd.program.display()
        )
    })?;
    let _ = std::fs::remove_file(&json);
    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("bad validator report: {e}"))?;
    let mut out = Vec::new();
    for r in value.as_array().map(Vec::as_slice).unwrap_or_default() {
        let Some(file) = r.get("File").and_then(Value::as_str) else {
            continue;
        };
        let issues = r
            .get("Errors")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|e| {
                let field = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
                let path = field("Path");
                let mut message = field("Description");
                if !path.is_empty() {
                    message = format!("{message} (at {path})");
                }
                Issue {
                    severity: Severity::Error,
                    source: "openxml",
                    part: field("Part"),
                    message,
                }
            })
            .collect();
        out.push((PathBuf::from(file), issues));
    }
    Ok(out)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn merge_openxml(reports: &mut [FileReport], results: OpenXmlResults) {
    for (file, issues) in results {
        if let Some(r) = reports.iter_mut().find(|r| same_file(&r.file, &file)) {
            r.issues.extend(issues);
        }
    }
}

fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn report_json(reports: &[FileReport], openxml: &str, valid: usize) -> Value {
    let files: Vec<Value> = reports
        .iter()
        .map(|r| {
            let list = |s: Severity| {
                r.issues
                    .iter()
                    .filter(|i| i.severity == s)
                    .map(|i| json!({ "source": i.source, "part": i.part, "message": i.message }))
                    .collect::<Vec<_>>()
            };
            json!({
                "file": r.file.to_string_lossy(),
                "valid": r.errors() == 0,
                "errors": list(Severity::Error),
                "warnings": list(Severity::Warning),
            })
        })
        .collect();
    json!({ "openxml": openxml, "total": reports.len(), "valid": valid, "files": files })
}

fn print_text(reports: &[FileReport], openxml: &str, valid: usize) {
    for r in reports {
        let errors = r.errors();
        let warnings = r.issues.len() - errors;
        let status = if errors == 0 { "OK  " } else { "FAIL" };
        println!(
            "{status} {} ({errors} errors, {warnings} warnings)",
            r.file.display()
        );
        for i in &r.issues {
            let part = if i.part.is_empty() { "-" } else { &i.part };
            println!(
                "     {} [{}] {part}: {}",
                severity_name(i.severity),
                i.source,
                i.message
            );
        }
    }
    let stage = if openxml == "ran" {
        "structure + OpenXmlValidator"
    } else {
        "structure only"
    };
    println!("{valid}/{} files valid ({stage})", reports.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rels_source_names() {
        assert_eq!(rels_source("/_rels/.rels").as_deref(), Some("/"));
        assert_eq!(
            rels_source("/word/_rels/document.xml.rels").as_deref(),
            Some("/word/document.xml")
        );
        assert_eq!(rels_source("/_rels/a.xml.rels").as_deref(), Some("/a.xml"));
        assert_eq!(rels_source("/word/document.xml.rels"), None);
    }

    #[test]
    fn well_formedness() {
        assert!(well_formed(b"<?xml version=\"1.0\"?><a x=\"1\"><b/>t&amp;&#65;</a>").is_ok());
        assert!(well_formed(b"<a><b></a>").is_err());
        assert!(well_formed(b"<a x=\"1\" x=\"2\"/>").is_err());
        assert!(well_formed(b"<a/><b/>").is_err());
        assert!(well_formed(b"<a>&nbsp;</a>").is_err());
        assert!(well_formed(b"<a>").is_err());
        assert!(well_formed(b"").is_err());
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("media/a%20b.png"), "media/a b.png");
        assert_eq!(percent_decode("x%2"), "x%2");
    }
}
