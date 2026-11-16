//! What a platform host needs from a PDF window, so one host runs BlueReader and BlueEditor.
use crate::editor_window::EditorWindow;
use crate::reader_view::{PdfStore, ReaderView};
use crate::reader_window::{ReaderRequest, ReaderWindow};
use crate::titlebar::NcHit;
use crate::{Input, Painter, TextMeasure};
use bo_pdfread::PdfDocument;
use std::path::{Path, PathBuf};

/// A PDF window (BlueReader or BlueEditor) as its host drives it.
pub trait PdfWindow {
    /// Handle an input event; true when a new frame is needed.
    fn handle(&mut self, input: Input, m: &dyn TextMeasure) -> bool;
    /// Bring page images up to date; true when more work remains.
    fn prepare(&mut self, store: &mut dyn PdfStore) -> bool;
    /// Paint the window.
    fn paint(&mut self, p: &mut dyn Painter);
    /// Window manager hit test.
    fn nc_hit(&self, x: f32, y: f32) -> NcHit;
    /// Pointer shape.
    fn cursor(&self) -> u8;
    /// Window title.
    fn title(&self) -> String;
    /// Requests since the last call.
    fn take_requests(&mut self) -> Vec<ReaderRequest>;
    /// The active document view.
    fn view_mut(&mut self) -> Option<&mut ReaderView>;
    /// File path of the active document.
    fn path(&self) -> Option<&Path>;
    /// The active document was saved as `path`.
    fn set_path(&mut self, path: PathBuf);
    /// A status message.
    fn set_status(&mut self, s: &str);
    /// Open a document in a new tab.
    fn open(&mut self, doc: PdfDocument, name: &str, path: Option<PathBuf>);
    /// Recent files.
    fn recent(&self) -> &[PathBuf];
    /// Recent files remembered by the host.
    fn set_recent(&mut self, recent: Vec<PathBuf>);
    /// Product name (window class, recent file list name).
    fn app(&self) -> &'static str;
    /// The bytes of a file picked for `purpose` ([`ReaderRequest::PickFile`]).
    fn file_picked(&mut self, _purpose: &str, _path: &Path, _bytes: &[u8]) {}
    /// Pages rendered as PNG for `purpose` ([`ReaderRequest::RenderPages`]).
    fn pages_rendered(&mut self, _purpose: &str, _pngs: Vec<Vec<u8>>) {}
    /// A file needs a password to open: ask for it (then [`ReaderRequest::OpenWithPassword`]).
    fn password_needed(&mut self, path: &Path) {
        self.set_status(&format!(
            "{}: the document is protected by a password",
            path.display()
        ));
    }
    /// Text caret in window pixels (for the input method window), when editing text.
    fn caret(&self) -> Option<[f32; 4]> {
        None
    }
    /// Show the window's own Print dialog with these printers; alse when it has none (the
    /// host then prints its own way).
    fn open_print(
        &mut self,
        _printers: Vec<String>,
        _default: Option<&str>,
        _m: &dyn TextMeasure,
    ) -> bool {
        false
    }
    /// Paper of each printer of the open Print dialog (in the order given to `open_print`).
    fn set_print_papers(&mut self, _papers: Vec<Option<crate::Paper>>) {}
    /// Stored preferences (`key=value` lines; BlueReader's File › Preferences).
    fn set_preferences(&mut self, _text: &str) {}
    /// Tab indices a Batch Print prints (taken once); empty for a normal print.
    fn take_batch(&mut self) -> Vec<usize> {
        Vec::new()
    }
    /// Document of tab `i`.
    fn view_at_mut(&mut self, _i: usize) -> Option<&mut ReaderView> {
        None
    }
    /// Paper of one printer after its driver settings changed.
    fn set_print_paper(&mut self, _name: &str, _paper: Option<crate::Paper>) {}
    /// Shift / Ctrl held with the last mouse message (BlueEditor: Ctrl+drag copies objects).
    fn set_pointer_mods(&mut self, _m: crate::Mods) {}
    /// Interface text `key` with `{{name}}` placeholders filled.
    fn text(&self, key: &str, _args: &[(&str, &str)]) -> String {
        key.to_owned()
    }
}

impl PdfWindow for ReaderWindow {
    fn handle(&mut self, input: Input, m: &dyn TextMeasure) -> bool {
        ReaderWindow::handle(self, input, m)
    }
    fn prepare(&mut self, store: &mut dyn PdfStore) -> bool {
        ReaderWindow::prepare(self, store)
    }
    fn paint(&mut self, p: &mut dyn Painter) {
        ReaderWindow::paint(self, p);
    }
    fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        ReaderWindow::nc_hit(self, x, y)
    }
    fn cursor(&self) -> u8 {
        ReaderWindow::cursor(self)
    }
    fn title(&self) -> String {
        ReaderWindow::title(self)
    }
    fn take_requests(&mut self) -> Vec<ReaderRequest> {
        ReaderWindow::take_requests(self)
    }
    fn view_mut(&mut self) -> Option<&mut ReaderView> {
        ReaderWindow::view_mut(self)
    }
    fn path(&self) -> Option<&Path> {
        ReaderWindow::path(self)
    }
    fn set_path(&mut self, path: PathBuf) {
        ReaderWindow::set_path(self, path);
    }
    fn set_status(&mut self, s: &str) {
        ReaderWindow::set_status(self, s);
    }
    fn open(&mut self, doc: PdfDocument, name: &str, path: Option<PathBuf>) {
        ReaderWindow::open(self, doc, name, path);
    }
    fn recent(&self) -> &[PathBuf] {
        ReaderWindow::recent(self)
    }
    fn set_recent(&mut self, recent: Vec<PathBuf>) {
        ReaderWindow::set_recent(self, recent);
    }
    fn app(&self) -> &'static str {
        "BlueReader"
    }
    fn open_print(
        &mut self,
        printers: Vec<String>,
        default: Option<&str>,
        m: &dyn TextMeasure,
    ) -> bool {
        ReaderWindow::open_print(self, printers, default, m);
        true
    }
    fn set_preferences(&mut self, text: &str) {
        ReaderWindow::set_preferences(self, text);
    }
    fn take_batch(&mut self) -> Vec<usize> {
        ReaderWindow::take_batch(self)
    }
    fn view_at_mut(&mut self, i: usize) -> Option<&mut ReaderView> {
        ReaderWindow::view_at_mut(self, i)
    }
    fn set_print_papers(&mut self, papers: Vec<Option<crate::Paper>>) {
        ReaderWindow::set_print_papers(self, papers);
    }
    fn set_print_paper(&mut self, name: &str, paper: Option<crate::Paper>) {
        ReaderWindow::set_print_paper(self, name, paper);
    }
    fn file_picked(&mut self, purpose: &str, path: &Path, bytes: &[u8]) {
        if ReaderWindow::file_picked9(self, purpose, path) {
            return;
        }
        if !ReaderWindow::file_picked4(self, purpose, bytes) {
            ReaderWindow::file_picked5(self, purpose, path, bytes);
        }
    }
    fn text(&self, key: &str, args: &[(&str, &str)]) -> String {
        ReaderWindow::strings_fmt(self, key, args)
    }
}

impl PdfWindow for EditorWindow {
    fn set_pointer_mods(&mut self, m: crate::Mods) {
        EditorWindow::set_pointer_mods(self, m);
    }
    fn handle(&mut self, input: Input, m: &dyn TextMeasure) -> bool {
        EditorWindow::handle(self, input, m)
    }
    fn prepare(&mut self, store: &mut dyn PdfStore) -> bool {
        EditorWindow::prepare(self, store)
    }
    fn paint(&mut self, p: &mut dyn Painter) {
        EditorWindow::paint(self, p);
    }
    fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        EditorWindow::nc_hit(self, x, y)
    }
    fn cursor(&self) -> u8 {
        EditorWindow::cursor(self)
    }
    fn title(&self) -> String {
        EditorWindow::title(self)
    }
    fn take_requests(&mut self) -> Vec<ReaderRequest> {
        EditorWindow::take_requests(self)
    }
    fn view_mut(&mut self) -> Option<&mut ReaderView> {
        EditorWindow::view_mut(self)
    }
    fn path(&self) -> Option<&Path> {
        EditorWindow::path(self)
    }
    fn set_path(&mut self, path: PathBuf) {
        EditorWindow::set_path(self, path);
    }
    fn set_status(&mut self, s: &str) {
        EditorWindow::set_status(self, s);
    }
    fn open(&mut self, doc: PdfDocument, name: &str, path: Option<PathBuf>) {
        EditorWindow::open(self, doc, name, path);
    }
    fn recent(&self) -> &[PathBuf] {
        EditorWindow::recent(self)
    }
    fn set_recent(&mut self, recent: Vec<PathBuf>) {
        EditorWindow::set_recent(self, recent);
    }
    fn app(&self) -> &'static str {
        "BlueEditor"
    }
    fn file_picked(&mut self, purpose: &str, path: &Path, bytes: &[u8]) {
        EditorWindow::file_picked(self, purpose, path, bytes);
    }
    fn pages_rendered(&mut self, purpose: &str, pngs: Vec<Vec<u8>>) {
        EditorWindow::pages_rendered(self, purpose, pngs);
    }
    fn password_needed(&mut self, path: &Path) {
        EditorWindow::password_needed(self, path);
    }
    fn caret(&self) -> Option<[f32; 4]> {
        EditorWindow::caret(self)
    }
}
