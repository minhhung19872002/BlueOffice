//! Headless BlueScript runtime (CORE-SCRIPT-004, ADR-0012 §10 and ADR-0013).
//!
//! Runs a user script in a fresh QuickJS-ng runtime (crate `rquickjs`, MIT) with the
//! **same** object API as the browser sandbox (`packages/script`): the runtime evaluates
//! the bundle generated from the TypeScript API hosts and prelude
//! ([`HEADLESS_JS`], built by `packages/script/scripts/build-headless.mjs`), whose only
//! way out is a synchronous engine [`Bridge`] (`bo_engine` calls on the open document).
//! There is no filesystem, network, module loader, `require`, `fetch` or timer inside
//! QuickJS: only ECMAScript built-ins plus `doc` / `workbook` / `sheet` / `presentation`
//! and `console`.
//!
//! Limits ([`Limits`]): wall-clock time (QuickJS interrupt handler), heap
//! (`JS_SetMemoryLimit`), interpreter stack, number of API calls, console lines and the
//! size of the output (logs + returned value).
#![forbid(unsafe_code)]

mod document;

pub use document::{Document, OpenError};

use rquickjs::context::EvalOptions;
use rquickjs::{Coerced, Context, Ctx, Exception, Function, Object, Persistent, Runtime, Value};
use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// The API hosts, prelude and wrapper of `packages/script` as one plain-JavaScript IIFE
/// (generated; `test/headless.test.ts` fails when it is stale).
pub const HEADLESS_JS: &str = include_str!("../js/bluescript-headless.js");

/// Which application object API a script gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum App {
    /// BlueDoc: `doc`.
    Doc,
    /// BlueSheet: `workbook`, `sheet`.
    Sheet,
    /// BlueSlide: `presentation`.
    Slide,
}

impl App {
    /// App id (`doc` / `sheet` / `slide`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            App::Doc => "doc",
            App::Sheet => "sheet",
            App::Slide => "slide",
        }
    }
}

/// Hard limits of one run. Defaults match the browser sandbox (`DEFAULT_LIMITS`).
#[derive(Clone, Debug)]
pub struct Limits {
    /// Wall time of the whole run, engine calls included.
    pub timeout: Duration,
    /// QuickJS heap available to the script (on top of the API set-up).
    pub memory_bytes: usize,
    /// Interpreter stack.
    pub stack_bytes: usize,
    /// API calls to the host.
    pub max_calls: u64,
    /// Console lines kept.
    pub max_log_lines: usize,
    /// Bytes of console text plus the JSON of the returned value.
    pub max_output_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            timeout: Duration::from_secs(30),
            memory_bytes: 64 * 1024 * 1024,
            stack_bytes: 512 * 1024,
            max_calls: 200_000,
            max_log_lines: 2_000,
            max_output_bytes: 1024 * 1024,
        }
    }
}

/// Options of a run.
#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Object API.
    pub app: App,
    /// Limits.
    pub limits: Limits,
    /// Decimal separator of BlueSheet input (the engine locale's: `,` for vi-VN).
    pub decimal: char,
    /// The sheet `sheet` refers to.
    pub active_sheet: usize,
}

impl RunOptions {
    /// Defaults for `app` (vi-VN decimal comma, first sheet).
    #[must_use]
    pub fn new(app: App) -> Self {
        RunOptions {
            app,
            limits: Limits::default(),
            decimal: ',',
            active_sheet: 0,
        }
    }
}

/// One console line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LogLine {
    /// `log`, `info`, `warn` or `error`.
    pub level: String,
    /// Text (at most 10 000 UTF-16 units, as in the browser).
    pub text: String,
}

/// Why a run did not finish normally (same kinds as the browser `ScriptErrorKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorKind {
    /// The script does not parse.
    Syntax,
    /// An exception escaped the script.
    Error,
    /// Wall-clock limit.
    Timeout,
    /// Heap limit.
    Memory,
    /// Stopped by the host.
    Cancelled,
    /// API call or output limit.
    Limit,
}

/// Failure of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptError {
    /// Kind.
    pub kind: ErrorKind,
    /// Message (`Name: message` unless the name is `Error`).
    pub message: String,
    /// QuickJS stack trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    /// 1-based line in the user's script.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// 1-based column.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl ScriptError {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        ScriptError {
            kind,
            message: message.into(),
            stack: None,
            line: None,
            column: None,
        }
    }
}

/// Result of [`run_script`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    /// The script finished without an error.
    pub ok: bool,
    /// JSON value the script returned (`return …` at top level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ScriptError>,
    /// Console output.
    pub logs: Vec<LogLine>,
    /// Console output was cut by a limit.
    pub logs_truncated: bool,
    /// API calls made.
    pub calls: u64,
    /// Wall time.
    pub ms: u64,
}

/// Synchronous engine calls the API hosts make (`EngineBridge` in `headless.ts`).
pub trait Bridge {
    /// Runs `op` with JSON `args`; returns the JSON result (`""` = no value) or the
    /// error message the script sees.
    ///
    /// # Errors
    /// Unknown op, bad arguments or an engine error.
    fn call(&mut self, op: &str, args: &[serde_json::Value]) -> Result<String, String>;
}

/// Mutable state shared by the native functions of a run.
#[derive(Default)]
struct Shared {
    logs: Vec<LogLine>,
    log_lines: usize,
    output_bytes: usize,
    truncated: bool,
    calls: u64,
}

/// Runs `code` against `bridge`. Never fails: every problem (syntax, exception, limit)
/// comes back in [`RunReport::error`].
pub fn run_script(code: &str, bridge: Rc<RefCell<dyn Bridge>>, opts: &RunOptions) -> RunReport {
    let start = Instant::now();
    let shared = Rc::new(RefCell::new(Shared::default()));
    let stop: Rc<Cell<Option<ErrorKind>>> = Rc::new(Cell::new(None));
    let outcome = run_inner(code, bridge, opts, start, &shared, &stop);
    let stopped = stop.get();
    let (ok, value, error) = match (stopped, outcome) {
        (Some(kind), _) => (false, None, Some(stopped_error(kind, &opts.limits))),
        (None, Ok(value)) => (true, value, None),
        (None, Err(e)) => (false, None, Some(e)),
    };
    let s = shared.borrow();
    RunReport {
        ok,
        value,
        error,
        logs: s.logs.clone(),
        logs_truncated: s.truncated,
        calls: s.calls,
        ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}

fn stopped_error(kind: ErrorKind, limits: &Limits) -> ScriptError {
    let message = match kind {
        ErrorKind::Timeout => format!("timeout ({} ms)", limits.timeout.as_millis()),
        ErrorKind::Limit => format!("too many API calls (more than {})", limits.max_calls),
        ErrorKind::Cancelled => "cancelled".to_owned(),
        _ => "stopped".to_owned(),
    };
    ScriptError::new(kind, message)
}

/// Number of UTF-16 units of `s` kept within `max` (as `String.prototype.slice`).
fn slice_utf16(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, ch) in s.char_indices() {
        units += ch.len_utf16();
        if units > max {
            return &s[..i];
        }
    }
    s
}

fn run_inner(
    code: &str,
    bridge: Rc<RefCell<dyn Bridge>>,
    opts: &RunOptions,
    start: Instant,
    shared: &Rc<RefCell<Shared>>,
    stop: &Rc<Cell<Option<ErrorKind>>>,
) -> Result<Option<serde_json::Value>, ScriptError> {
    let limits = &opts.limits;
    let rt = Runtime::new().map_err(|e| setup_error("runtime", &e))?;
    rt.set_max_stack_size(limits.stack_bytes);
    let deadline = start + limits.timeout;
    {
        let stop = Rc::clone(stop);
        rt.set_interrupt_handler(Some(Box::new(move || {
            if stop.get().is_none() && Instant::now() > deadline {
                stop.set(Some(ErrorKind::Timeout));
            }
            stop.get().is_some()
        })));
    }
    let ctx = Context::full(&rt).map_err(|e| setup_error("context", &e))?;
    let prepared = ctx.with(|ctx| prepare(&ctx, bridge, opts, shared, stop))?;
    // The heap limit counts from here: the API set-up is not the script's.
    let used = usize::try_from(rt.memory_usage().malloc_size).unwrap_or(0);
    rt.set_memory_limit(used.saturating_add(limits.memory_bytes));
    ctx.with(|ctx| execute(&ctx, code, prepared, shared, limits.max_output_bytes))
}

fn setup_error(what: &str, e: &dyn std::fmt::Display) -> ScriptError {
    ScriptError::new(ErrorKind::Error, format!("runtime set-up ({what}): {e}"))
}

/// A set-up step failed: the pending exception (if any) as text.
fn setup_fail(ctx: &Ctx<'_>, what: &str, e: &rquickjs::Error) -> ScriptError {
    let detail = if matches!(e, rquickjs::Error::Exception) {
        describe(ctx, &ctx.catch(), 0).message
    } else {
        e.to_string()
    };
    setup_error(what, &detail)
}

/// What [`prepare`] hands to [`execute`].
struct Prepared {
    wrap: Persistent<Function<'static>>,
    wrap_lines: u32,
}

/// Evaluates the bundle, installs `__bo_call` / `__bo_log` / `__bo_app` and the prelude.
fn prepare<'js>(
    ctx: &Ctx<'js>,
    bridge: Rc<RefCell<dyn Bridge>>,
    opts: &RunOptions,
    shared: &Rc<RefCell<Shared>>,
    stop: &Rc<Cell<Option<ErrorKind>>>,
) -> Result<Prepared, ScriptError> {
    let limits = &opts.limits;
    let globals = ctx.globals();
    let bundle = |e: rquickjs::Error| setup_fail(ctx, "bundle", &e);
    ctx.eval_with_options::<(), _>(HEADLESS_JS, eval_options("bluescript-headless.js"))
        .map_err(bundle)?;
    let api: Object<'js> = globals.get("__bo_headless").map_err(bundle)?;
    globals.remove("__bo_headless").map_err(bundle)?;
    let prelude: String = api.get("prelude").map_err(bundle)?;
    let wrap: Function<'js> = api.get("wrap").map_err(bundle)?;
    let wrap_lines: u32 = api.get("wrapLines").map_err(bundle)?;
    let create: Function<'js> = api.get("create").map_err(bundle)?;

    // The engine bridge is reachable only from the bundle's closures.
    let engine = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, op: String, json: String| -> rquickjs::Result<String> {
            let args: Vec<serde_json::Value> = serde_json::from_str(&json)
                .map_err(|e| Exception::throw_message(&ctx, &format!("bad arguments: {e}")))?;
            let result = bridge.borrow_mut().call(&op, &args);
            result.map_err(|m| Exception::throw_message(&ctx, &m))
        },
    )
    .map_err(|e| setup_fail(ctx, "engine", &e))?;
    let dispatch: Function<'js> = create
        .call((
            opts.app.as_str(),
            engine,
            opts.decimal.to_string(),
            opts.active_sheet,
        ))
        .map_err(|e| setup_fail(ctx, "host", &e))?;

    // `__bo_call(method, json)`: counts calls, then the host (a promise of JSON). The
    // composition is JavaScript so that no Rust closure holds a JS value (QuickJS' cycle
    // collector cannot see through native closures).
    let max_calls = limits.max_calls;
    let calls_state = Rc::clone(shared);
    let calls_stop = Rc::clone(stop);
    let count = Function::new(ctx.clone(), move |ctx: Ctx<'js>| -> rquickjs::Result<()> {
        let n = {
            let mut s = calls_state.borrow_mut();
            s.calls += 1;
            s.calls
        };
        if n > max_calls {
            if calls_stop.get().is_none() {
                calls_stop.set(Some(ErrorKind::Limit));
            }
            return Err(Exception::throw_message(&ctx, "too many API calls"));
        }
        Ok(())
    })
    .map_err(|e| setup_fail(ctx, "call", &e))?;
    let compose: Function<'js> = ctx
        .eval_with_options(
            "(count, dispatch) => (method, json) => { count(); return dispatch(method, json); }",
            eval_options("bluescript-call.js"),
        )
        .map_err(|e| setup_fail(ctx, "call", &e))?;
    let call: Function<'js> = compose
        .call((count, dispatch))
        .map_err(|e| setup_fail(ctx, "call", &e))?;
    globals
        .set("__bo_call", call)
        .map_err(|e| setup_fail(ctx, "call", &e))?;

    let log_state = Rc::clone(shared);
    let max_lines = limits.max_log_lines;
    let max_output = limits.max_output_bytes;
    let log = Function::new(ctx.clone(), move |level: String, text: String| {
        let mut s = log_state.borrow_mut();
        if s.log_lines >= max_lines {
            s.truncated = true;
            return;
        }
        s.log_lines += 1;
        let text = if s.log_lines == max_lines {
            "…".to_owned()
        } else {
            slice_utf16(&text, 10_000).to_owned()
        };
        if s.output_bytes + text.len() > max_output {
            s.truncated = true;
            s.log_lines = max_lines;
            return;
        }
        s.output_bytes += text.len();
        let level = match level.as_str() {
            "warn" | "error" | "info" => level,
            _ => "log".to_owned(),
        };
        s.logs.push(LogLine { level, text });
    })
    .map_err(|e| setup_fail(ctx, "console", &e))?;
    globals
        .set("__bo_log", log)
        .map_err(|e| setup_fail(ctx, "console", &e))?;
    globals
        .set("__bo_app", opts.app.as_str())
        .map_err(|e| setup_fail(ctx, "app", &e))?;

    ctx.eval_with_options::<(), _>(prelude, eval_options("bluescript-prelude.js"))
        .map_err(|e| setup_fail(ctx, "prelude", &e))?;
    Ok(Prepared {
        wrap: Persistent::save(ctx, wrap),
        wrap_lines,
    })
}

/// Evaluates the wrapped user code and drives the job queue until it settles.
fn execute(
    ctx: &Ctx<'_>,
    code: &str,
    prepared: Prepared,
    shared: &Rc<RefCell<Shared>>,
    max_output: usize,
) -> Result<Option<serde_json::Value>, ScriptError> {
    let wrap_lines = prepared.wrap_lines;
    let wrap = prepared
        .wrap
        .restore(ctx)
        .map_err(|e| setup_fail(ctx, "wrap", &e))?;
    let wrapped: String = wrap
        .call((code,))
        .map_err(|e| setup_fail(ctx, "wrap", &e))?;
    let started = ctx.eval_with_options::<Value, _>(wrapped, eval_options("script.js"));
    let promise = match started {
        Ok(v) => v,
        Err(rquickjs::Error::Exception) => {
            let mut e = describe(ctx, &ctx.catch(), wrap_lines);
            if e.kind == ErrorKind::Error {
                e.kind = ErrorKind::Syntax;
            }
            return Err(e);
        }
        Err(e) => return Err(classify(e.to_string())),
    };
    let Some(promise) = promise.as_promise().cloned() else {
        return Err(ScriptError::new(ErrorKind::Error, "script did not start"));
    };
    let value = match promise.finish::<Value>() {
        Ok(v) => v,
        Err(rquickjs::Error::Exception) => return Err(describe(ctx, &ctx.catch(), wrap_lines)),
        Err(rquickjs::Error::WouldBlock) => {
            return Err(ScriptError::new(
                ErrorKind::Error,
                "the script waits for a promise that never settles",
            ));
        }
        Err(e) => return Err(classify(e.to_string())),
    };
    to_json(ctx, &value, shared, max_output)
}

fn eval_options(file: &str) -> EvalOptions {
    let mut o = EvalOptions::default();
    o.global = true;
    o.strict = false;
    o.filename = Some(file.to_owned());
    o
}

fn classify(message: String) -> ScriptError {
    let kind = if message.to_ascii_lowercase().contains("out of memory") {
        ErrorKind::Memory
    } else {
        ErrorKind::Error
    };
    ScriptError::new(kind, message)
}

/// The returned value as JSON (like the browser: `undefined` → none, else a JSON copy).
fn to_json<'js>(
    ctx: &Ctx<'js>,
    value: &Value<'js>,
    shared: &Rc<RefCell<Shared>>,
    max_output: usize,
) -> Result<Option<serde_json::Value>, ScriptError> {
    if value.is_undefined() {
        return Ok(None);
    }
    let text = match ctx.json_stringify(value.clone()) {
        Ok(Some(s)) => s.to_string().unwrap_or_default(),
        Ok(None) => {
            // Not representable in JSON (a function, a symbol): its string form.
            let s = value.get::<Coerced<String>>().map(|c| c.0);
            return Ok(Some(serde_json::Value::String(s.unwrap_or_default())));
        }
        Err(rquickjs::Error::Exception) => return Err(describe(ctx, &ctx.catch(), 0)),
        Err(e) => return Err(classify(e.to_string())),
    };
    let used = shared.borrow().output_bytes;
    if used + text.len() > max_output {
        return Err(ScriptError::new(
            ErrorKind::Limit,
            format!("output larger than {max_output} bytes"),
        ));
    }
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| ScriptError::new(ErrorKind::Error, format!("result: {e}")))
}

/// An exception value as a [`ScriptError`] (same text rules as the browser `dumpError`).
fn describe<'js>(ctx: &Ctx<'js>, v: &Value<'js>, wrap_lines: u32) -> ScriptError {
    let prop = |o: &Object<'_>, k: &str| -> Option<String> {
        o.get::<_, Value>(k)
            .ok()
            .and_then(|v| v.as_string().and_then(|s| s.to_string().ok()))
    };
    let (name, message, stack) = if let Some(o) = v.as_object() {
        let message = prop(o, "message").unwrap_or_else(|| {
            ctx.json_stringify(v.clone())
                .ok()
                .flatten()
                .and_then(|s| s.to_string().ok())
                .unwrap_or_default()
        });
        (
            prop(o, "name").unwrap_or_default(),
            message,
            prop(o, "stack").filter(|s| !s.is_empty()),
        )
    } else if let Some(s) = v.as_string() {
        (String::new(), s.to_string().unwrap_or_default(), None)
    } else if v.is_null() {
        // QuickJS throws `null` when it cannot even allocate the error.
        (String::new(), "out of memory".to_owned(), None)
    } else {
        let s = ctx
            .json_stringify(v.clone())
            .ok()
            .flatten()
            .and_then(|s| s.to_string().ok())
            .unwrap_or_default();
        (String::new(), s, None)
    };
    let text = if !name.is_empty() && name != "Error" {
        format!("{name}: {message}")
    } else {
        message.clone()
    };
    let kind = if message.to_ascii_lowercase().contains("out of memory") {
        ErrorKind::Memory
    } else if name == "SyntaxError" {
        ErrorKind::Syntax
    } else {
        ErrorKind::Error
    };
    let (line, column) = stack
        .as_deref()
        .and_then(|s| script_position(s, wrap_lines))
        .map_or((None, None), |(l, c)| (Some(l), Some(c)));
    ScriptError {
        kind,
        message: text,
        stack,
        line,
        column,
    }
}

/// First `script.js:<line>:<col>` of a QuickJS stack, as a line of the user's code.
fn script_position(stack: &str, wrap_lines: u32) -> Option<(u32, u32)> {
    const FILE: &str = "script.js:";
    let mut rest = stack;
    while let Some(at) = rest.find(FILE) {
        rest = &rest[at + FILE.len()..];
        let mut parts = rest.splitn(3, |c: char| !c.is_ascii_digit());
        let line: Option<u32> = parts.next().and_then(|s| s.parse().ok());
        let col: Option<u32> = parts.next().and_then(|s| s.parse().ok());
        if let (Some(line), Some(col)) = (line, col) {
            if line > wrap_lines {
                return Some((line - wrap_lines, col));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_skip_the_wrapper_line() {
        let s = "    at <anonymous> (script.js:4:9)\n    at <eval> (script.js:1:1)\n";
        assert_eq!(script_position(s, 1), Some((3, 9)));
        assert_eq!(script_position("at script.js:1:5", 1), None);
        assert_eq!(script_position("no file here", 1), None);
    }

    #[test]
    fn slices_by_utf16_units() {
        assert_eq!(slice_utf16("abc", 2), "ab");
        assert_eq!(slice_utf16("a😀b", 2), "a");
        assert_eq!(slice_utf16("a😀b", 3), "a😀");
        assert_eq!(slice_utf16("ổn", 5), "ổn");
    }
}
