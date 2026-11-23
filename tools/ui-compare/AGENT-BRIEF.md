## Common context (read first)

Repo: BlueOffice (Rust core + native Windows UI). Read `CLAUDE.md` (rules), `docs/adr/0018-native-ui-everywhere.md`,
`docs/plan/native-ui.md` (milestone N6 is what you start), `docs/ui/REPORT.md` rounds 7–40 (how BlueDoc's native UI was
matched to the desktop word processor by black-box measurement: Windows text size factor, ribbon metrics, palette, greys,
dot-ruler, scroll bar, status bar, menus, dialogs, grayscale text rendering).

BlueDoc already has a native self-drawn UI that the owner now accepts as "fast and sharp like Office":
- `crates/bo-ui/` — platform-neutral UI: `DocWindow` (shell.rs), `DocView` (doc_view.rs), `Ribbon` (ribbon.rs; build your
  own tabs with `Ribbon::with_tabs(Vec<Tab>)`, `Tab`/`Group`/`Item` are `pub(crate)`), `icons.rs` (own icon drawings +
  palette tuned to the reference: `SOLID`/`OVERRIDES` tables), `popup.rs` menus, `dialog.rs`, `ruler.rs`, `titlebar.rs`,
  `theme.rs`, `strings.rs` (i18n from `packages/i18n/src/locales/*.json`, add keys to BOTH vi-VN and en-US).
- `crates/bo-gpu-win/` — Direct2D/DirectWrite backend (swap chain flip + waitable, page bitmaps redrawn by band,
  grayscale document text tuned to the reference, UI text ClearType GDI-classic, UI text scaled by the Windows
  Accessibility text size factor in the host).
- `apps/native-win/` — Win32 host (`win32.rs` Handler trait + message loop, `app.rs` BlueDoc app) → `bluedoc-native.exe`.
Engine: `crates/bo-engine` (`SheetEngine` for workbooks: `exec_json` / `SheetCommand`; `Engine` slide commands:
`slide_command_json`), see the "Sơ đồ crate" section of CLAUDE.md.

### Rules
- Clean-room (CLAUDE.md): never copy/trace Microsoft icons, fonts, names. Owner decision 2026-11-15 (memory
  `ribbon-look-like-word.md`, CLAUDE.md naming section): native UI SHOULD match the reference desktop app's palette,
  drawing style, sizes and layout by black-box measurement; icon shapes are our own drawings; keep BlueSheet/BlueSlide
  names and our own title-bar colour (the reference's title-bar brand colour is not copied).
- Rust: `#![forbid(unsafe_code)]` except the existing Win32 files, no `unwrap()` on file data, clippy pedantic clean
  (`cargo clippy -p <crate> --all-targets -- -D warnings`), `cargo fmt`. Tests for new behaviour (headless with bo-ui's
  `Fixed` text measure like `crates/bo-ui/src/shell.rs` tests).
- Keep your work in NEW modules where possible to avoid merge conflicts with the other parallel agent:
  your own files under `crates/bo-ui/src/<sheet|slide>_*.rs`, your own host module + `[[bin]]` in `apps/native-win`
  (new `src/<sheet|slide>_app.rs` + `src/bin/...` or a new `[[bin]]` entry). Only add small, additive edits to shared
  files (`lib.rs` exports, `Cargo.toml` bin entry, `win32.rs` only if strictly needed). Do NOT restyle or refactor
  BlueDoc's ribbon/shell/dialog code; do not change BlueDoc behaviour.
- Commits: conventional, small, each with tests green, ending with exactly these two lines:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
  `Claude-Session: <this session URL>`
  Commit on your worktree branch only (do not touch `main` or `work/desktop-performance`).

### Build environment (disk is tight — important)
- Every cargo command: `CARGO_TARGET_DIR=E:/bo-targets/<your-name> CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`,
  and `-j 2` (memory pressure kills jobs). PowerShell: `$env:Path="$env:USERPROFILE\.cargo\bin;$env:Path"`.
- Do not run full `cargo test --workspace`; run the crates you touch (`bo-ui`, `bo-gpu-win`, `bo-native-win`, engine
  tests you touch).
- The user is running `bluedoc-native.exe` from `D:\Source\BlueOffice\target\release` and `out\desktop` — never kill
  or overwrite those. Put your release exe in `out/desktop/<bluesheet|blueslide>-native.exe` at the end.

### Comparison method (as done for BlueDoc)
Windows is at 125 % DPI with Accessibility text size 110 %. Capture helper scripts are in `tools/ui-compare/`:
`cap2.ps1 <window-title-substring> <out.png> [W H]` (PrintWindow, resizes window to 1500×1102 at 40,40 without
activating), `cap.ps1`, `drive.ps1` (launch + post clicks), `type2.ps1` (typing latency via BLUEOFFICE_NATIVE_PERF).
Copy and adapt them into your own scratch folder. Open the same file in the reference app (Excel / PowerPoint at
`C:\Program Files\Microsoft Office\root\Office16\`) via COM on a COPY of the file (never the user's documents),
capture both at the same size, crop/zoom regions with Python PIL, measure colours/sizes, fix, re-capture. Close any
reference-app window you opened. Record rounds and numbers in `docs/ui/REPORT-<sheet|slide>.md`.

### Report back
A short summary: what works, screenshots paths compared, numbers (typing/keystroke latency, colours, sizes),
commits on your branch, what remains.

## Screenshots for the owner (added 2026-11-15)
The owner wants to see every comparison. Save a side-by-side image for each (reference top/left, ours bottom/right,
cropped to the relevant area, ≤ 1600 px wide) as `scratchpad\show\<app>-<NN>-<topic>.png`
(<SCRATCH>\show\),
SendMessage "main" the new paths after each step, and list them in the final report. Never include the reference's
recent-file lists or the owner's own files.

## Icons: own shapes only (reminder 2026-11-16)
Match the reference's layout, sizes, greys and the MEANING of accent colours, but never trace or closely copy its
icon shapes (Microsoft or Foxit). If your icon looks like a redraw of the reference's at 4× zoom, redesign it in the
BlueOffice icon family (icons.rs / reader_icons.rs style).

## Printing and the owner's documents (added 2026-11-17)
Never press Print / OK in a reference app's print dialog and never click its quick-access Print button; open the
dialog with a posted command in your own instance on your own scratch copy, capture, then Cancel. Our own print
tests target "Microsoft Print to PDF" with an output file in scratch — never the default physical printer. If a
document that is not one of your scratch copies shows up in a reference window (the owner may be using the app),
stop interacting with that window at once and leave it open.

## Real keyboard / mouse input (owner decision 2026-11-24)
Default stays: drive the reference app with posted messages, UI Automation or COM on scratch copies. Real input
(SendInput, keybd_event, SetCursorPos, `agent-desktop --headed`) is allowed ONLY while the owner is physically idle:
1. `powershell -File tools/ui-compare/physical-idle.ps1 -Start` (watcher with low-level hooks that counts only
   NON-injected keyboard / mouse events; a no-op when it already runs).
2. Before every real-input burst run `physical-idle.ps1 -Check 10` — exit 0 means no physical input for ≥ 10 min.
   Re-check between steps (at most every 30 s of input); the moment it says busy, stop sending input, leave every
   window as it is and fall back to posted messages / ours-only images.
3. Only into your own reference instance on a scratch copy, its window brought up just for the burst; never into
   another window, never Print, never a shortcut that closes or saves something that is not yours; restore the
   clipboard and the previous foreground window afterwards.
4. One agent at a time: create `%TEMP%\bo-real-input.lock` (with your agent name) before the burst, delete it after;
   if it exists and is younger than 10 min, wait.
5. First use: verify the watcher ignores your synthetic input (inject one zero-distance mouse move while `-Check`
   says idle; the idle seconds must keep growing) and note it in REPORT.
Use it for what posted messages cannot reach (hover states, drag and drop, real typing latency of the reference,
dropdown menus that ignore posted clicks). Record in REPORT which measurements used real input.
