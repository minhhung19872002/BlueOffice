//! BlueReader round 9 (a child of `reader_window`): Advanced Search, a pane at the right of the
//! document like the reference reader's (Search › Advanced Search; its layout from the
//! reference's public user manual — the pane does not open for posted input). Where: the
//! current document or every PDF in a folder; options: whole words, case, include bookmarks,
//! include comments. Results are grouped per document and page, each with its context; pages
//! are read a slice at a time on the window's ticks so the window stays responsive.
use super::{elide, inside, rounded, ReaderRequest, ReaderWindow};
use crate::reader_view::Selection;
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_pdfread::{PdfDocument, RenderOptions, TextLayer};
use bo_render::Rgba;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Pane width (logical px).
pub(crate) const PANE_W: f32 = 320.0;
/// Time a tick may spend reading pages.
const SLICE: Duration = Duration::from_millis(45);
/// Characters of context on each side of a hit.
const CONTEXT: usize = 36;
/// Results kept (the count goes on).
const MAX_ROWS: usize = 5000;

const TEXT: Rgba = Rgba(51, 51, 51, 255);
const MUTED: Rgba = Rgba(110, 110, 112, 255);
const FIELD: Rgba = Rgba(255, 255, 255, 255);
const HIT_BG: Rgba = Rgba(255, 230, 120, 255);
const ROW_HOVER: Rgba = Rgba(230, 233, 240, 255);

/// Where a hit was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Text,
    Bookmark,
    Comment,
}

/// One result: page, character range (text hits), context split around the hit.
#[derive(Debug, Clone)]
pub(crate) struct Found {
    pub page: usize,
    pub range: Option<(usize, usize)>,
    pub source: Source,
    pub before: String,
    pub hit: String,
    pub after: String,
    /// Measured widths of `before` and `hit` (device px).
    widths: (f32, f32),
}

/// Results of one document.
#[derive(Debug, Clone)]
pub(crate) struct DocFound {
    pub name: String,
    pub path: Option<PathBuf>,
    pub found: Vec<Found>,
    pub count: usize,
}

/// The documents a search reads.
#[derive(Debug, Clone, PartialEq)]
enum Src {
    Current,
    File(PathBuf),
}

struct Job {
    srcs: Vec<Src>,
    file: usize,
    page: usize,
    pages: usize,
    doc: Option<PdfDocument>,
    t0: Instant,
}

/// Pane state.
#[derive(Default)]
pub(crate) struct Search9 {
    pub open: bool,
    pub query: String,
    pub focus: bool,
    pub in_folder: bool,
    pub folder: Option<PathBuf>,
    pub whole: bool,
    pub case: bool,
    pub bookmarks: bool,
    pub comments: bool,
    pub results: Vec<DocFound>,
    pub total: usize,
    pub pages_read: usize,
    /// Milliseconds the last search took (to its end).
    pub elapsed_ms: Option<f64>,
    job: Option<Job>,
    scroll: f32,
    hover: Option<usize>,
    /// Hit boxes of the last paint: rect, action.
    hits: Vec<([f32; 4], Action)>,
}

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Close,
    Current,
    Folder,
    Browse,
    Field,
    Toggle(u8),
    Search,
    Row(usize, usize),
}

/// Text hits of one page with their context.
fn page_hits(layer: &TextLayer, page: usize, q: &str, case: bool, whole: bool) -> Vec<Found> {
    let flat = |s: String| s.split_whitespace().collect::<Vec<_>>().join(" ");
    layer
        .find(q, case, whole)
        .into_iter()
        .map(|(a, b)| {
            let before = if a == 0 {
                String::new()
            } else {
                flat(layer.slice(a.saturating_sub(CONTEXT), a - 1))
            };
            Found {
                page,
                range: Some((a, b)),
                source: Source::Text,
                before,
                hit: flat(layer.slice(a, b)),
                after: flat(layer.slice(b + 1, b + CONTEXT)),
                widths: (0.0, 0.0),
            }
        })
        .collect()
}

/// `text` matches the query (bookmark titles, comment contents) with its context.
fn text_hit(
    text: &str,
    page: usize,
    q: &str,
    case: bool,
    whole: bool,
    source: Source,
) -> Option<Found> {
    let norm = |s: &str| if case { s.to_owned() } else { s.to_lowercase() };
    let (hay, needle) = (norm(text), norm(q.trim()));
    if needle.is_empty() {
        return None;
    }
    let mut from = 0;
    while let Some(k) = hay[from..].find(&needle) {
        let a = from + k;
        let b = a + needle.len();
        let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
        let ok = !whole || (!word(hay[..a].chars().last()) && !word(hay[b..].chars().next()));
        if ok && text.is_char_boundary(a) && text.is_char_boundary(b) {
            return Some(Found {
                page,
                range: None,
                source,
                before: text[..a].to_owned(),
                hit: text[a..b].to_owned(),
                after: text[b..].to_owned(),
                widths: (0.0, 0.0),
            });
        }
        from = a + needle.len().max(1);
        while from < hay.len() && !hay.is_char_boundary(from) {
            from += 1;
        }
    }
    None
}

/// Bookmark titles that match, with their pages.
fn outline_hits(doc: &PdfDocument, q: &str, case: bool, whole: bool) -> Vec<Found> {
    fn walk(items: &[bo_pdfread::OutlineItem], out: &mut Vec<(String, usize)>) {
        for it in items {
            let page = match it.target {
                Some(bo_pdfread::LinkTarget::Page { index, .. }) => index,
                _ => 0,
            };
            out.push((it.title.clone(), page));
            walk(&it.children, out);
        }
    }
    let mut all = Vec::new();
    walk(&doc.outline(), &mut all);
    all.into_iter()
        .filter_map(|(t, p)| text_hit(&t, p, q, case, whole, Source::Bookmark))
        .collect()
}

/// Comment contents that match.
fn comment_hits(doc: &PdfDocument, q: &str, case: bool, whole: bool) -> Vec<Found> {
    doc.annotations()
        .into_iter()
        .filter_map(|a| text_hit(&a.contents, a.page, q, case, whole, Source::Comment))
        .collect()
}

/// PDF files of `dir` (not its subfolders), by name.
fn pdfs_in(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

impl ReaderWindow {
    /// Open the Advanced Search pane with the title bar's query.
    pub(crate) fn open_search9(&mut self) {
        let s = &mut self.search9;
        s.open = true;
        s.focus = true;
        if s.query.is_empty() {
            s.query.clone_from(&self.search_text);
        }
        let (case, word) = self.find_options8();
        if self.search9.results.is_empty() {
            self.search9.case = case;
            self.search9.whole = word;
        }
        if self.search9.folder.is_none() {
            self.search9.folder = self
                .tabs
                .get(self.active)
                .and_then(|t| t.path.as_ref())
                .and_then(|p| p.parent().map(std::path::Path::to_path_buf));
        }
        self.set_right_pane(PANE_W);
    }

    fn close_search9(&mut self) {
        self.search9.open = false;
        self.search9.focus = false;
        self.search9.job = None;
        self.set_right_pane(0.0);
    }

    /// Start the search with the pane's settings.
    pub(crate) fn run_search9(&mut self) {
        let s = &mut self.search9;
        s.results.clear();
        s.total = 0;
        s.pages_read = 0;
        s.scroll = 0.0;
        s.elapsed_ms = None;
        s.job = None;
        if s.query.trim().is_empty() {
            return;
        }
        let srcs = if s.in_folder {
            s.folder
                .as_deref()
                .map(pdfs_in)
                .unwrap_or_default()
                .into_iter()
                .map(Src::File)
                .collect()
        } else if self.tabs.is_empty() {
            Vec::new()
        } else {
            vec![Src::Current]
        };
        self.search9.job = Some(Job {
            srcs,
            file: 0,
            page: 0,
            pages: 0,
            doc: None,
            t0: Instant::now(),
        });
    }

    /// A folder picked for the search (any PDF in it was chosen); `true` when it was ours.
    pub(crate) fn file_picked9(&mut self, purpose: &str, path: &std::path::Path) -> bool {
        if purpose != "searchFolder" {
            return false;
        }
        self.search9.folder = path.parent().map(std::path::Path::to_path_buf);
        self.search9.in_folder = true;
        true
    }

    /// Read pages of the running search for one slice; true when something changed.
    pub(crate) fn tick_search9(&mut self, m: &dyn TextMeasure) -> bool {
        if self.search9.job.is_none() {
            return false;
        }
        let t0 = Instant::now();
        let (q, case, whole) = (
            self.search9.query.clone(),
            self.search9.case,
            self.search9.whole,
        );
        let (bm, cm) = (self.search9.bookmarks, self.search9.comments);
        let st = self.row_style9();
        let bold = TextStyle { bold: true, ..st };
        while t0.elapsed() < SLICE {
            let Some(job) = self.search9.job.as_mut() else {
                break;
            };
            let Some(src) = job.srcs.get(job.file).cloned() else {
                let ms = job.t0.elapsed().as_secs_f64() * 1e3;
                self.search9.job = None;
                self.search9.elapsed_ms = Some(ms);
                break;
            };
            // Starting a document: open it (folder), list its bookmarks and comments.
            if job.page == 0 && job.pages == 0 {
                let (name, path, extra) = match &src {
                    Src::Current => {
                        let t = &self.tabs[self.active];
                        let doc = t.view.doc();
                        job.pages = doc.page_count();
                        let mut extra = Vec::new();
                        if bm {
                            extra.extend(outline_hits(doc, &q, case, whole));
                        }
                        if cm {
                            extra.extend(comment_hits(doc, &q, case, whole));
                        }
                        (t.name.clone(), t.path.clone(), extra)
                    }
                    Src::File(p) => {
                        let opened = std::fs::read(p)
                            .ok()
                            .and_then(|b| PdfDocument::open(b, bo_pdfread::standard_fonts()).ok());
                        let mut extra = Vec::new();
                        if let Some(d) = &opened {
                            job.pages = d.page_count();
                            if bm {
                                extra.extend(outline_hits(d, &q, case, whole));
                            }
                            if cm {
                                extra.extend(comment_hits(d, &q, case, whole));
                            }
                        }
                        job.doc = opened;
                        let name = p
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        (name, Some(p.clone()), extra)
                    }
                };
                if job.pages == 0 {
                    job.file += 1;
                    job.doc = None;
                    continue;
                }
                self.search9.results.push(DocFound {
                    name,
                    path,
                    found: Vec::new(),
                    count: 0,
                });
                self.add_found9(extra, m, st, bold);
            }
            let Some(job) = self.search9.job.as_mut() else {
                break;
            };
            let page = job.page;
            let rendered = match (&src, job.doc.as_mut()) {
                (Src::File(_), Some(d)) => d.render(page, RenderOptions::default()).ok(),
                _ => self
                    .tabs
                    .get_mut(self.active)
                    .and_then(|t| t.view.doc_mut().render(page, RenderOptions::default()).ok()),
            };
            let found = rendered
                .map(|p| page_hits(&TextLayer::new(p.chars), page, &q, case, whole))
                .unwrap_or_default();
            self.search9.pages_read += 1;
            self.add_found9(found, m, st, bold);
            if let Some(job) = self.search9.job.as_mut() {
                job.page += 1;
                if job.page >= job.pages {
                    job.page = 0;
                    job.pages = 0;
                    job.file += 1;
                    job.doc = None;
                }
            }
        }
        true
    }

    fn add_found9(
        &mut self,
        found: Vec<Found>,
        m: &dyn TextMeasure,
        st: TextStyle,
        bold: TextStyle,
    ) {
        let s = &mut self.search9;
        let rows: usize = s.results.iter().map(|d| d.found.len()).sum();
        let Some(doc) = s.results.last_mut() else {
            return;
        };
        doc.count += found.len();
        s.total += found.len();
        let room = MAX_ROWS.saturating_sub(rows);
        for mut f in found.into_iter().take(room) {
            let before = elide_start(&f.before, CONTEXT / 2);
            f.widths = (m.measure(&before, st).0, m.measure(&f.hit, bold).0);
            f.before = before;
            doc.found.push(f);
        }
    }

    fn row_style9(&self) -> TextStyle {
        self.style(12.5, false)
    }

    /// Input for the pane; `Some(redraw)` when it took it.
    pub(crate) fn route_search9(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Input::Key(Key::Letter('f'), mods) = input {
            if mods.ctrl && mods.shift && self.profile.is_none() {
                self.open_search9();
                return Some(true);
            }
        }
        if !self.search9.open {
            return None;
        }
        let pane = self.right_pane_rect();
        match input {
            Input::Text(t) if self.search9.focus => {
                self.search9.query.push_str(t);
                Some(true)
            }
            Input::Key(k, _) if self.search9.focus => {
                match k {
                    Key::Backspace => {
                        self.search9.query.pop();
                    }
                    Key::Enter => self.run_search9(),
                    Key::Escape => self.search9.focus = false,
                    _ => return None,
                }
                Some(true)
            }
            Input::Move { x, y } if inside(pane, *x, *y) => {
                let h = self
                    .search9
                    .hits
                    .iter()
                    .position(|(r, _)| inside(*r, *x, *y));
                let changed = h != self.search9.hover;
                self.search9.hover = h;
                Some(changed)
            }
            Input::Wheel(dy) if inside(pane, self.pointer.0, self.pointer.1) => {
                self.search9.scroll = (self.search9.scroll + dy).max(0.0);
                Some(true)
            }
            Input::Press { x, y, .. } if inside(pane, *x, *y) => {
                let act = self
                    .search9
                    .hits
                    .iter()
                    .find(|(r, _)| inside(*r, *x, *y))
                    .map(|(_, a)| a.clone());
                self.search9.focus = act == Some(Action::Field);
                match act {
                    Some(Action::Close) => self.close_search9(),
                    Some(Action::Current) => self.search9.in_folder = false,
                    Some(Action::Folder) => self.search9.in_folder = true,
                    Some(Action::Browse) => self.requests.push(ReaderRequest::PickFile {
                        purpose: "searchFolder".into(),
                        filters: vec![("PDF".into(), "*.pdf".into())],
                    }),
                    Some(Action::Toggle(k)) => {
                        let s = &mut self.search9;
                        let v = match k {
                            0 => &mut s.whole,
                            1 => &mut s.case,
                            2 => &mut s.bookmarks,
                            _ => &mut s.comments,
                        };
                        *v = !*v;
                    }
                    Some(Action::Search) => self.run_search9(),
                    Some(Action::Row(d, i)) => self.show_found9(d, i, m),
                    Some(Action::Field) | None => {}
                }
                Some(true)
            }
            Input::Press { .. } => {
                self.search9.focus = false;
                None
            }
            _ => None,
        }
    }

    /// Go to result `i` of document `d` (another file opens in a tab).
    fn show_found9(&mut self, d: usize, i: usize, m: &dyn TextMeasure) {
        let Some(doc) = self.search9.results.get(d) else {
            return;
        };
        let Some(f) = doc.found.get(i).cloned() else {
            return;
        };
        let current = self.tabs.get(self.active).map(|t| t.path.clone());
        let same = !self.search9.in_folder || current.as_ref() == Some(&doc.path);
        if !same {
            if let Some(p) = doc.path.clone() {
                self.requests.push(ReaderRequest::Open(Some(p)));
            }
            return;
        }
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.go_to(f.page, None);
            if let Some((a, b)) = f.range {
                t.view.set_selection(Some(Selection { page: f.page, a, b }));
                if let Some(r) = t.view.selection_rects().first() {
                    t.view.go_to(f.page, Some((r[1] - 72.0).max(0.0)));
                }
            }
        }
        let _ = m;
    }

    /// Paint the pane.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint_search9(&mut self, p: &mut dyn Painter) {
        if !self.search9.open {
            return;
        }
        let r = self.right_pane_rect();
        if r[2] <= 0.0 {
            return;
        }
        let mut hits = Vec::new();
        let px = |v: f32| (v * self.scale).round();
        p.fill_rect(r[0], r[1], r[2], r[3], self.look.nav_bg);
        p.fill_rect(r[0], r[1], 1.0, r[3], self.look.edge);
        p.push_clip(r[0], r[1], r[2], r[3]);
        let x0 = r[0] + px(12.0);
        let w = r[2] - px(24.0);
        let title = self.style(16.0, false);
        let body = self.style(13.0, false);
        let small = self.row_style9();
        let mut y = r[1] + px(10.0);
        p.text(&self.t("pdfSearch9.title"), x0, y, title, TEXT);
        // Close button.
        let cb = [r[0] + r[2] - px(32.0), y - px(2.0), px(24.0), px(24.0)];
        let (cx, cy, k) = (cb[0] + cb[2] / 2.0, cb[1] + cb[3] / 2.0, px(5.0));
        p.path(
            &[
                bo_render::PathSeg::M(cx - k, cy - k),
                bo_render::PathSeg::L(cx + k, cy + k),
                bo_render::PathSeg::M(cx + k, cy - k),
                bo_render::PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((TEXT, (1.2 * self.scale).max(1.0))),
        );
        hits.push((cb, Action::Close));
        y += px(36.0);
        p.fill_rect(r[0] + 1.0, y - px(6.0), r[2] - 1.0, 1.0, self.look.edge);
        // Where.
        p.text(&self.t("pdfSearch9.where"), x0, y, body, MUTED);
        y += px(24.0);
        let radio = |p: &mut dyn Painter, x: f32, y: f32, on: bool, edge: Rgba, accent: Rgba| {
            let d = (12.0 * self.scale).round();
            p.path(
                &rounded(x, y, d, d, d / 2.0),
                Some(FIELD),
                Some((edge, 1.0)),
            );
            if on {
                let e = (3.0 * self.scale).round();
                p.path(
                    &rounded(x + e, y + e, d - 2.0 * e, d - 2.0 * e, (d - 2.0 * e) / 2.0),
                    Some(accent),
                    None,
                );
            }
        };
        let accent = self.theme.accent;
        for (on, key, act) in [
            (
                !self.search9.in_folder,
                "pdfSearch9.current",
                Action::Current,
            ),
            (self.search9.in_folder, "pdfSearch9.folder", Action::Folder),
        ] {
            radio(p, x0, y + px(3.0), on, MUTED, accent);
            p.text(&self.t(key), x0 + px(20.0), y, body, TEXT);
            hits.push(([x0, y - px(2.0), w, px(20.0)], act));
            y += px(22.0);
        }
        // Folder and Browse.
        let fb = [x0 + px(20.0), y, w - px(20.0) - px(76.0), px(24.0)];
        p.path(
            &rounded(fb[0] + 0.5, fb[1] + 0.5, fb[2] - 1.0, fb[3] - 1.0, px(3.0)),
            Some(FIELD),
            Some((self.look.edge, 1.0)),
        );
        let folder = self.search9.folder.as_ref().map_or_else(
            || self.t("pdfSearch9.noFolder"),
            |f| f.to_string_lossy().into_owned(),
        );
        let shown = elide(
            &folder,
            self.width_of(&folder, small),
            fb[2] - px(10.0),
            small.size,
        );
        p.push_clip(fb[0], fb[1], fb[2], fb[3]);
        p.text(
            &shown,
            fb[0] + px(5.0),
            fb[1] + px(4.0),
            small,
            if self.search9.in_folder { TEXT } else { MUTED },
        );
        p.pop_clip();
        let bb = [fb[0] + fb[2] + px(6.0), y, px(70.0), px(24.0)];
        p.path(
            &rounded(bb[0] + 0.5, bb[1] + 0.5, bb[2] - 1.0, bb[3] - 1.0, px(3.0)),
            Some(FIELD),
            Some((self.look.edge, 1.0)),
        );
        p.text(
            &self.t("pdfSearch9.browse"),
            bb[0] + px(9.0),
            bb[1] + px(4.0),
            small,
            TEXT,
        );
        hits.push((bb, Action::Browse));
        y += px(36.0);
        // What.
        p.text(&self.t("pdfSearch9.what"), x0, y, body, MUTED);
        y += px(22.0);
        let tb = [x0, y, w, px(28.0)];
        let edge = if self.search9.focus {
            accent
        } else {
            self.look.edge
        };
        p.path(
            &rounded(tb[0] + 0.5, tb[1] + 0.5, tb[2] - 1.0, tb[3] - 1.0, px(3.0)),
            Some(FIELD),
            Some((edge, 1.0)),
        );
        p.push_clip(tb[0], tb[1], tb[2], tb[3]);
        p.text(
            &self.search9.query,
            tb[0] + px(7.0),
            tb[1] + px(6.0),
            body,
            TEXT,
        );
        if self.search9.focus {
            let cx = tb[0] + px(7.0) + self.width_of(&self.search9.query, body) + 1.0;
            p.fill_rect(cx, tb[1] + px(6.0), 1.0, px(16.0), TEXT);
        }
        p.pop_clip();
        hits.push((tb, Action::Field));
        y += px(38.0);
        // Options.
        for (k, (on, key)) in [
            (self.search9.whole, "pdfSearch9.whole"),
            (self.search9.case, "pdfSearch9.case"),
            (self.search9.bookmarks, "pdfSearch9.bookmarks"),
            (self.search9.comments, "pdfSearch9.comments"),
        ]
        .into_iter()
        .enumerate()
        {
            let d = px(13.0);
            p.path(
                &rounded(x0 + 0.5, y + px(3.0) + 0.5, d - 1.0, d - 1.0, px(2.0)),
                Some(if on { accent } else { FIELD }),
                Some((if on { accent } else { MUTED }, 1.0)),
            );
            if on {
                let (ax, ay) = (x0 + px(3.0), y + px(3.0) + d / 2.0);
                p.path(
                    &[
                        bo_render::PathSeg::M(ax, ay),
                        bo_render::PathSeg::L(ax + px(2.5), ay + px(2.5)),
                        bo_render::PathSeg::L(ax + px(7.5), ay - px(3.0)),
                    ],
                    None,
                    Some((FIELD, (1.5 * self.scale).max(1.0))),
                );
            }
            p.text(&self.t(key), x0 + px(21.0), y, body, TEXT);
            #[allow(clippy::cast_possible_truncation)]
            hits.push(([x0, y, w, px(20.0)], Action::Toggle(k as u8)));
            y += px(23.0);
        }
        y += px(6.0);
        // Search button: the accent at full strength with white text (4.5 : 1).
        let sb = [x0, y, px(88.0), px(28.0)];
        p.path(
            &rounded(sb[0], sb[1], sb[2], sb[3], px(3.0)),
            Some(self.theme.accent_strong),
            None,
        );
        let label = self.t("pdfSearch9.search");
        let lw = self.width_of(&label, body);
        p.text(
            &label,
            sb[0] + ((sb[2] - lw) / 2.0).round(),
            sb[1] + px(6.0),
            body,
            self.theme.accent_on,
        );
        hits.push((sb, Action::Search));
        y += px(40.0);
        // Status.
        let s = &self.search9;
        let docs = s.results.iter().filter(|d| d.count > 0).count();
        let status = if s.job.is_some() {
            self.strings.fmt(
                "pdfSearch9.reading",
                &[
                    ("pages", &s.pages_read.to_string()),
                    ("hits", &s.total.to_string()),
                ],
            )
        } else if s.elapsed_ms.is_some() {
            self.strings.fmt(
                "pdfSearch9.summary",
                &[("docs", &docs.to_string()), ("hits", &s.total.to_string())],
            )
        } else {
            String::new()
        };
        p.text(&status, x0, y, small, MUTED);
        y += px(22.0);
        p.fill_rect(r[0] + 1.0, y - px(4.0), r[2] - 1.0, 1.0, self.look.edge);
        // Results.
        let list = [r[0] + 1.0, y, r[2] - 1.0, (r[1] + r[3] - y).max(0.0)];
        let row_h = px(22.0);
        let head_h = px(24.0);
        let total_h: f32 = s
            .results
            .iter()
            .map(|d| head_h + d.found.len() as f32 * row_h)
            .sum();
        let scroll = s.scroll.min((total_h - list[3]).max(0.0));
        p.push_clip(list[0], list[1], list[2], list[3]);
        let mut ry = list[1] - scroll;
        let bold = TextStyle {
            bold: true,
            ..small
        };
        let page_label = self.t("pdfSearch9.page");
        let hover = s
            .hover
            .and_then(|h| self.search9.hits.get(h))
            .map(|h| h.1.clone());
        for (di, d) in s.results.iter().enumerate() {
            if ry + head_h >= list[1] && ry <= list[1] + list[3] {
                let head = format!("{} ({})", d.name, d.count);
                p.text(&head, x0, ry + px(4.0), bold, TEXT);
            }
            ry += head_h;
            for (i, f) in d.found.iter().enumerate() {
                if ry + row_h < list[1] {
                    ry += row_h;
                    continue;
                }
                if ry > list[1] + list[3] {
                    break;
                }
                let rr = [list[0], ry, list[2], row_h];
                if hover.as_ref() == Some(&Action::Row(di, i)) {
                    p.fill_rect(rr[0], rr[1], rr[2], rr[3], ROW_HOVER);
                }
                let tag = match f.source {
                    Source::Text => format!("{page_label} {}", f.page + 1),
                    Source::Bookmark => self.t("pdfSearch9.inBookmark"),
                    Source::Comment => self.t("pdfSearch9.inComment"),
                };
                let ty = ry + px(3.0);
                p.text(&tag, x0 + px(8.0), ty, small, MUTED);
                let mut tx = x0 + px(64.0);
                p.push_clip(tx, ry, list[0] + list[2] - tx - px(6.0), row_h);
                p.text(&f.before, tx, ty, small, TEXT);
                tx += f.widths.0;
                p.fill_rect(tx, ry + px(3.0), f.widths.1, row_h - px(6.0), HIT_BG);
                p.text(&f.hit, tx, ty, bold, TEXT);
                tx += f.widths.1;
                p.text(&f.after, tx, ty, small, TEXT);
                p.pop_clip();
                hits.push((rr, Action::Row(di, i)));
                ry += row_h;
            }
        }
        p.pop_clip();
        p.pop_clip();
        self.search9.scroll = scroll;
        self.search9.hits = hits;
    }

    /// Results (tests).
    #[cfg(test)]
    pub(crate) fn search9_results(&self) -> &[DocFound] {
        &self.search9.results
    }

    #[cfg(test)]
    pub(crate) fn search9_busy(&self) -> bool {
        self.search9.job.is_some()
    }
}

/// The last `n` characters of `s` after an ellipsis when it is longer.
fn elide_start(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        return s.to_owned();
    }
    let tail: String = s.chars().skip(count - n).collect();
    format!("…{}", tail.trim_start())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader_tests::{pdf, Fixed};
    use crate::Mods;

    fn window(pages: usize) -> ReaderWindow {
        let doc = PdfDocument::open(pdf(pages), bo_pdfread::standard_fonts()).expect("pdf");
        let mut w = ReaderWindow::new("en-US");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        w.open(doc, "test.pdf", None);
        w
    }

    #[test]
    fn advanced_search_groups_hits_per_page_with_context() {
        let mut w = window(4);
        w.handle(
            Input::Key(
                Key::Letter('f'),
                Mods {
                    ctrl: true,
                    shift: true,
                    alt: false,
                },
            ),
            &Fixed,
        );
        assert!(w.search9.open);
        assert!(w.right_pane_rect()[2] > 0.0);
        w.handle(Input::Text("hello".into()), &Fixed);
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
        for _ in 0..50 {
            if !w.search9_busy() {
                break;
            }
            w.handle(Input::Tick, &Fixed);
        }
        let r = w.search9_results();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].count, 4);
        let f = &r[0].found[2];
        assert_eq!(f.page, 2);
        assert_eq!(f.hit, "hello");
        assert_eq!(f.before, "Page 3");
        assert_eq!(f.after, "world");
        // Whole words: "hell" finds nothing; case: "Hello" finds nothing.
        w.search9.query = "hell".into();
        w.search9.whole = true;
        w.run_search9();
        while w.search9_busy() {
            w.handle(Input::Tick, &Fixed);
        }
        assert_eq!(w.search9.total, 0);
        w.search9.query = "HELLO".into();
        w.search9.whole = false;
        w.search9.case = true;
        w.run_search9();
        while w.search9_busy() {
            w.handle(Input::Tick, &Fixed);
        }
        assert_eq!(w.search9.total, 0);
        // A result row goes to its page and selects the hit.
        w.search9.case = false;
        w.run_search9();
        while w.search9_busy() {
            w.handle(Input::Tick, &Fixed);
        }
        let mut p = crate::reader_tests::Rec::default();
        w.paint(&mut p);
        let row = w
            .search9
            .hits
            .iter()
            .find(|h| h.1 == Action::Row(0, 3))
            .map(|h| h.0)
            .expect("row");
        w.handle(
            Input::Press {
                x: row[0] + 20.0,
                y: row[1] + 5.0,
                shift: false,
                count: 1,
            },
            &Fixed,
        );
        let want = w.search9_results()[0].found[3].range;
        let v = w.view().expect("view");
        assert_eq!(v.selection().map(|s| s.page), Some(3));
        assert_eq!(v.selection().map(|s| (s.a, s.b)), want);
        assert_eq!(v.current_page(), 3);
    }

    #[test]
    fn bookmark_and_comment_text_and_folder_listing() {
        let f = text_hit(
            "Intro to Hello World",
            3,
            "hello",
            false,
            true,
            Source::Bookmark,
        )
        .expect("hit");
        assert_eq!(
            (f.before.as_str(), f.hit.as_str(), f.after.as_str()),
            ("Intro to ", "Hello", " World")
        );
        assert!(text_hit("Othello", 0, "hello", false, true, Source::Comment).is_none());
        assert!(text_hit("Othello", 0, "hello", false, false, Source::Comment).is_some());
        let dir = std::env::temp_dir().join(format!("bo-search9-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("b.PDF"), pdf(2)).expect("write");
        std::fs::write(dir.join("a.pdf"), pdf(1)).expect("write");
        std::fs::write(dir.join("c.txt"), b"x").expect("write");
        let found = pdfs_in(&dir);
        assert_eq!(found.len(), 2);
        let mut w = window(1);
        w.search9.folder = Some(dir.clone());
        w.search9.in_folder = true;
        w.search9.query = "world".into();
        w.run_search9();
        while w.search9_busy() {
            w.handle(Input::Tick, &Fixed);
        }
        let names: Vec<(&str, usize)> = w
            .search9_results()
            .iter()
            .map(|d| (d.name.as_str(), d.count))
            .collect();
        assert_eq!(names, vec![("a.pdf", 1), ("b.PDF", 2)]);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(elide_start("abcdef", 3), "…def");
    }
}
