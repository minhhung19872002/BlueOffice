//! Format checker for Vietnamese administrative documents (Nghị định 30/2020/NĐ-CP,
//! Phụ lục I): page size, margins, font, and the size, style, alignment and position of
//! every component it recognises, against the rule table of [`super::nd30`].
//!
//! Components are recognised from their text ("CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM",
//! "Số:", "Nơi nhận:", a document type name, …) and their place: the header block is the
//! first table holding the national title or the number, the signature block the last
//! table after it whose columns start with an upper-case title. Each deviation is an
//! issue `<component>.<property>` (e.g. `quocHieu.size`, `tieuNgu.underline`,
//! `page.marginLeft`); repeated deviations of one kind are counted, not repeated.

use super::nd30::{self, Area, Case, Component, Range, BODY, PAGE};
use bo_edit::{effective_para, effective_run};
use bo_model::{index, Alignment, Block, Document, LineSpacing, Paragraph, RunContent};
use serde::Serialize;

/// How serious a deviation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// The decree's rule is not met.
    Error,
    /// Recommended presentation (page numbers, …).
    Warning,
}

/// One deviation from the decree.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatIssue {
    /// `<component>.<property>`, e.g. `quocHieu.size`.
    pub id: String,
    /// Component id ([`nd30::Component::id`], `page`, `font`, `body`).
    pub component: String,
    /// Checked property: `size`, `bold`, `italic`, `case`, `align`, `position`,
    /// `underline`, `missing`, `font`, `paper`, `marginTop`, …, `indent`, `spacing`,
    /// `lineSpacing`, `firstPage`.
    pub property: String,
    /// Error or warning.
    pub severity: Severity,
    /// Expected value: a number with its unit ("12–13 pt", "30–35 mm") or a token
    /// (`yes`, `no`, `upper`, `center`, `justify`, `present`, `headerRight`, …).
    pub expected: String,
    /// Value found, same notation.
    pub actual: String,
    /// Body paragraph id (layout / selection numbering) of the first occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub para: Option<usize>,
    /// Section index for page rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<usize>,
    /// Start of the paragraph's text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Number of occurrences of this issue.
    pub count: usize,
}

/// Result of a check.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FormatReport {
    /// Deviations, in document order of their first occurrence (page rules first).
    pub issues: Vec<FormatIssue>,
    /// Ids of the components recognised.
    pub found: Vec<String>,
}

impl FormatReport {
    /// Ids of the issues.
    pub fn ids(&self) -> Vec<&str> {
        self.issues.iter().map(|i| i.id.as_str()).collect()
    }
}

/// Check a document against the decree.
pub fn check(doc: &Document) -> FormatReport {
    let mut out = Out::default();
    check_page(doc, &mut out);
    check_font(doc, &mut out);
    let entries = entries(doc);
    let roles = classify(&entries);
    for c in nd30::COMPONENTS {
        if roles.iter().any(|r| r.map(|r| r.id) == Some(c.id)) && !out.found.contains(&c.id) {
            out.found.push(c.id);
        }
    }
    for (e, role) in entries.iter().zip(&roles) {
        if let Some(c) = role {
            check_component(doc, c, e, &entries, &mut out);
        }
    }
    check_missing(&mut out);
    check_body(doc, &entries, &roles, &mut out);
    check_page_number(doc, &mut out);
    FormatReport {
        issues: out.issues,
        found: out.found.iter().map(|s| (*s).to_owned()).collect(),
    }
}

// ---- issue collection ------------------------------------------------------------

#[derive(Default)]
struct Out {
    issues: Vec<FormatIssue>,
    found: Vec<&'static str>,
}

/// What an issue is about.
struct At<'a> {
    para: Option<usize>,
    section: Option<usize>,
    text: Option<&'a str>,
}

const NOWHERE: At<'static> = At {
    para: None,
    section: None,
    text: None,
};

impl Out {
    fn add(&mut self, component: &str, property: &str, expected: String, actual: String, at: &At) {
        let severity = if component == nd30::SO_TRANG.id || component == nd30::KINH_GUI.id {
            Severity::Warning
        } else {
            Severity::Error
        };
        let id = format!("{component}.{property}");
        if let Some(i) = self
            .issues
            .iter_mut()
            .find(|i| i.id == id && i.section == at.section)
        {
            i.count += 1;
            return;
        }
        self.issues.push(FormatIssue {
            id,
            component: component.to_owned(),
            property: property.to_owned(),
            severity,
            expected,
            actual,
            para: at.para,
            section: at.section,
            text: at.text.map(excerpt),
            count: 1,
        });
    }
}

fn excerpt(s: &str) -> String {
    const MAX: usize = 48;
    if s.chars().count() <= MAX {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(MAX).collect();
        t.push('…');
        t
    }
}

/// Size in half-points as points ("13 pt", "12.5 pt").
fn pt(h: u32) -> String {
    if h.is_multiple_of(2) {
        format!("{} pt", h / 2)
    } else {
        format!("{}.5 pt", h / 2)
    }
}

fn pt_range(r: Range<u32>) -> String {
    if r.min == r.max {
        pt(r.min)
    } else {
        format!("{}–{}", pt(r.min).trim_end_matches(" pt"), pt(r.max))
    }
}

fn yes(b: bool) -> String {
    if b { "yes" } else { "no" }.to_owned()
}

fn align_name(a: Alignment) -> &'static str {
    match a {
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
        Alignment::Justify => "justify",
    }
}

// ---- page and font -----------------------------------------------------------------

/// Allowed rounding of a margin, mm (twips are whole numbers).
const MARGIN_TOLERANCE: f64 = 0.1;

fn check_page(doc: &Document, out: &mut Out) {
    for (si, s) in doc.sections.iter().enumerate() {
        let at = At {
            section: Some(si),
            ..NOWHERE
        };
        let (w, h) = (
            nd30::twips_to_mm(s.page.width.0),
            nd30::twips_to_mm(s.page.height.0),
        );
        if (w - PAGE.width).abs() > PAGE.size_tolerance
            || (h - PAGE.height).abs() > PAGE.size_tolerance
        {
            out.add(
                "page",
                "paper",
                format!("{:.0} × {:.0} mm", PAGE.width, PAGE.height),
                format!("{w:.0} × {h:.0} mm"),
                &at,
            );
        }
        let margins = [
            ("marginTop", s.page.margin_top.0, PAGE.top),
            ("marginBottom", s.page.margin_bottom.0, PAGE.bottom),
            ("marginLeft", s.page.margin_left.0, PAGE.left),
            ("marginRight", s.page.margin_right.0, PAGE.right),
        ];
        for (name, twips, range) in margins {
            let mm = nd30::twips_to_mm(twips);
            if mm < range.min - MARGIN_TOLERANCE || mm > range.max + MARGIN_TOLERANCE {
                out.add(
                    "page",
                    name,
                    format!("{:.0}–{:.0} mm", range.min, range.max),
                    format!("{mm:.1} mm"),
                    &at,
                );
            }
        }
    }
}

/// Font of the hAnsi slot (Vietnamese letters) and of the ASCII slot of a run.
fn run_fonts(doc: &Document, p: &Paragraph, props: &bo_model::RunProps) -> [Option<String>; 2] {
    let r = effective_run(doc, p, props);
    let theme = |t: &Option<String>| {
        t.as_deref()
            .and_then(|t| doc.theme_fonts.resolve(t))
            .map(str::to_owned)
    };
    let ascii = theme(&r.theme_ascii).or_else(|| r.font.clone());
    let h_ansi = theme(&r.theme_h_ansi)
        .or_else(|| r.font_h_ansi.clone())
        .or_else(|| ascii.clone());
    [h_ansi, ascii]
}

fn check_font(doc: &Document, out: &mut Out) {
    let ids = index::body_paragraphs(doc);
    let visit = |p: &Paragraph, para: Option<usize>, out: &mut Out| {
        for run in text_runs(p) {
            for f in run_fonts(doc, p, &run.props) {
                let ok = f
                    .as_deref()
                    .is_some_and(|f| f.eq_ignore_ascii_case(nd30::FONT));
                if !ok {
                    let text = p.plain_text();
                    out.add(
                        "font",
                        "font",
                        nd30::FONT.to_owned(),
                        f.unwrap_or_else(|| "-".to_owned()),
                        &At {
                            para,
                            section: None,
                            text: Some(&text),
                        },
                    );
                    return;
                }
            }
        }
    };
    for (id, path) in ids.iter().enumerate() {
        if let Some(p) = index::para_at(doc, path) {
            visit(p, Some(id), out);
        }
    }
    for s in &doc.sections {
        for blocks in hf_parts(s) {
            for p in all_paragraphs(blocks) {
                visit(p, None, out);
            }
        }
    }
}

fn hf_parts(s: &bo_model::Section) -> impl Iterator<Item = &Vec<Block>> {
    [
        &s.headers.default,
        &s.headers.first,
        &s.headers.even,
        &s.footers.default,
        &s.footers.first,
        &s.footers.even,
    ]
    .into_iter()
    .flatten()
}

fn all_paragraphs(blocks: &[Block]) -> Vec<&Paragraph> {
    let mut v = Vec::new();
    for b in blocks {
        match b {
            Block::Paragraph(p) => v.push(p),
            Block::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        v.extend(all_paragraphs(&c.blocks));
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
    v
}

/// Runs that show text (not only white space).
fn text_runs(p: &Paragraph) -> impl Iterator<Item = &bo_model::Run> {
    p.runs.iter().filter(|r| {
        r.props.del.is_none()
            && r.content.iter().any(|c| match c {
                RunContent::Text(t) => !t.trim().is_empty(),
                RunContent::Field { cached, .. } => !cached.trim().is_empty(),
                _ => false,
            })
    })
}

// ---- recognition --------------------------------------------------------------------

/// A body paragraph with its place.
struct Entry<'d> {
    /// Body paragraph id.
    id: usize,
    path: Vec<usize>,
    p: &'d Paragraph,
    /// Trimmed text with single spaces.
    text: String,
    area: Area,
    /// Table cell `(section, block, row, cell)` for paragraphs of a top-level table.
    cell: Option<[usize; 4]>,
}

impl Entry<'_> {
    fn top_level(&self) -> bool {
        self.path.len() == 2
    }
    /// `(section, block)` of the top-level block holding the paragraph.
    fn block(&self) -> (usize, usize) {
        (self.path[0], self.path[1])
    }
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '–' | '—' | '‑'))
        .collect()
}

fn is_quoc_hieu(t: &str) -> bool {
    let u = squash(&t.to_uppercase()).replace("HOÀ", "HÒA");
    u == "CỘNGHÒAXÃHỘICHỦNGHĨAVIỆTNAM"
}

fn is_tieu_ngu(t: &str) -> bool {
    squash(&t.to_lowercase()) == "độclậptựdohạnhphúc"
}

fn is_so(t: &str) -> bool {
    t.strip_prefix("Số")
        .is_some_and(|r| r.trim_start().starts_with(':'))
}

fn is_date(t: &str) -> bool {
    let l = t.to_lowercase();
    l.contains("ngày") && l.contains("tháng") && l.contains("năm") && t.chars().count() < 90
}

/// All letters are capitals (and there is at least one).
fn is_upper(t: &str) -> bool {
    let mut letters = t.chars().filter(|c| c.is_alphabetic()).peekable();
    letters.peek().is_some() && letters.all(|c| !c.is_lowercase())
}

fn type_name(t: &str) -> bool {
    let u = normalize(&t.to_uppercase())
        .replace("THOẢ", "THỎA")
        .replace("UỶ", "ỦY");
    nd30::TYPE_NAMES.contains(&u.as_str())
}

fn has_text(e: &Entry) -> bool {
    !e.text.is_empty()
}

/// A paragraph that draws a horizontal line: an empty paragraph with a visible top or
/// bottom border (§17.3.1.24 pBdr), a line of rule characters, or a line shape.
fn is_rule(p: &Paragraph) -> bool {
    let text = p.plain_text();
    let t = text.trim();
    let bordered = p.props.borders.as_ref().is_some_and(|b| {
        [&b.top, &b.bottom]
            .iter()
            .any(|e| e.as_ref().is_some_and(bo_model::Border::is_visible))
    });
    (t.is_empty() && (bordered || has_line_shape(p)))
        || (t.chars().count() >= 3 && t.chars().all(|c| "_-—–―─━‒".contains(c)))
}

/// A drawing whose preset geometry is a line (ECMA-376 Part 1, §20.1.10.56 ST_ShapeType
/// `line`, `straightConnector1`) or a VML line (§14.1.2.12 v:line).
fn has_line_shape(p: &Paragraph) -> bool {
    p.runs.iter().flat_map(|r| &r.content).any(|c| match c {
        RunContent::Image(i) => {
            i.shape
                .as_ref()
                .is_some_and(|s| matches!(s.preset.as_str(), "line" | "straightConnector1"))
                || i.src.as_ref().is_some_and(|s| line_xml(&s.xml))
        }
        RunContent::Opaque(r) | RunContent::Markup(r) => line_xml(&r.xml),
        _ => false,
    })
}

fn line_xml(xml: &str) -> bool {
    xml.contains("prst=\"line\"")
        || xml.contains("prst=\"straightConnector1\"")
        || xml.contains("<v:line")
}

/// The paragraph's own text is underlined or bordered below.
fn self_underlined(doc: &Document, p: &Paragraph) -> bool {
    let bottom = effective_para(doc, &p.props)
        .borders
        .is_some_and(|b| b.bottom.is_some_and(|e| e.is_visible()));
    let mut runs = text_runs(p).peekable();
    let underlined = runs.peek().is_some()
        && runs.all(|r| {
            effective_run(doc, p, &r.props)
                .underline
                .is_some_and(|u| u != bo_model::Underline::None)
        });
    bottom || underlined || has_line_shape(p)
}

fn entries(doc: &Document) -> Vec<Entry<'_>> {
    let paths = index::body_paragraphs(doc);
    // Top-level tables: header = first one with the national title or the number;
    // signature = last one after it whose columns start with an upper-case title.
    let mut tables: Vec<(usize, usize)> = Vec::new();
    for (si, s) in doc.sections.iter().enumerate() {
        for (bi, b) in s.blocks.iter().enumerate() {
            if matches!(b, Block::Table(_)) {
                tables.push((si, bi));
            }
        }
    }
    let table_texts = |key: (usize, usize)| -> Vec<(usize, String)> {
        paths
            .iter()
            .filter(|p| p.len() > 2 && (p[0], p[1]) == key)
            .filter_map(|p| index::para_at(doc, p).map(|x| (p[3], normalize(&x.plain_text()))))
            .collect()
    };
    let header = tables.iter().copied().find(|&k| {
        table_texts(k)
            .iter()
            .any(|(_, t)| is_quoc_hieu(t) || is_so(t))
    });
    let signature = tables
        .iter()
        .copied()
        .filter(|&k| Some(k) != header && header.is_none_or(|h| k > h))
        .rfind(|&k| {
            let texts = table_texts(k);
            let mut cells: Vec<usize> = texts.iter().map(|(c, _)| *c).collect();
            cells.dedup();
            cells.iter().any(|&c| {
                texts
                    .iter()
                    .find(|(cc, t)| *cc == c && !t.is_empty())
                    .is_some_and(|(_, t)| is_upper(t) || t.starts_with("Nơi nhận"))
            })
        });
    let noi_nhan_cell = signature.and_then(|k| {
        table_texts(k)
            .into_iter()
            .find(|(_, t)| t.starts_with("Nơi nhận"))
            .map(|(c, _)| c)
    });
    let header_cells = header.map_or(0, |k| {
        table_texts(k)
            .iter()
            .map(|(c, _)| *c + 1)
            .max()
            .unwrap_or(0)
    });
    paths
        .iter()
        .enumerate()
        .filter_map(|(id, path)| {
            let p = index::para_at(doc, path)?;
            let key = (path[0], path[1]);
            let cell = (path.len() > 2).then(|| [path[0], path[1], path[2], path[3]]);
            let area = if path.len() > 2 && Some(key) == header && header_cells > 1 {
                if path[3] == 0 {
                    Area::HeaderLeft
                } else {
                    Area::HeaderRight
                }
            } else if path.len() > 2 && Some(key) == signature {
                if Some(path[3]) == noi_nhan_cell {
                    Area::SignatureLeft
                } else {
                    Area::Signature
                }
            } else {
                Area::Body
            };
            Some(Entry {
                id,
                path: path.clone(),
                p,
                text: normalize(&p.plain_text()),
                area,
                cell,
            })
        })
        .collect()
}

/// The component of each entry (`None`: body text or unrecognised).
fn classify(entries: &[Entry]) -> Vec<Option<&'static Component>> {
    let mut roles: Vec<Option<&'static Component>> = vec![None; entries.len()];
    let mut title_seen = false;
    let mut summary_next = false;
    let mut in_noi_nhan: Option<[usize; 4]> = None;
    for (i, e) in entries.iter().enumerate() {
        let t = e.text.as_str();
        if t.is_empty() {
            continue;
        }
        let header = matches!(e.area, Area::HeaderLeft | Area::HeaderRight);
        let role: Option<&'static Component> = if is_quoc_hieu(t) {
            Some(&nd30::QUOC_HIEU)
        } else if is_tieu_ngu(t) {
            Some(&nd30::TIEU_NGU)
        } else if is_so(t) && (header || !title_seen) {
            Some(&nd30::SO_KY_HIEU)
        } else if t.starts_with("V/v") && header {
            Some(&nd30::TRICH_YEU_CONG_VAN)
        } else if header && is_date(t) {
            Some(&nd30::DIA_DANH)
        } else if t.starts_with("Nơi nhận") {
            in_noi_nhan = e.cell;
            Some(&nd30::NOI_NHAN)
        } else if in_noi_nhan.is_some()
            && e.cell == in_noi_nhan
            && (t.starts_with('-') || t.starts_with('–'))
        {
            Some(&nd30::NOI_NHAN_LIST)
        } else if t.starts_with("Kính gửi") {
            Some(&nd30::KINH_GUI)
        } else if e.area == Area::Body && !title_seen && e.top_level() && type_name(t) {
            title_seen = true;
            summary_next = true;
            roles[i] = Some(&nd30::TEN_LOAI);
            continue;
        } else if summary_next && e.top_level() && !is_rule(e.p) {
            Some(&nd30::TRICH_YEU)
        } else {
            None
        };
        summary_next = false;
        roles[i] = role;
    }
    // Issuing body: the text lines of the left header column above the number.
    let left: Vec<usize> = (0..entries.len())
        .filter(|&i| entries[i].area == Area::HeaderLeft && has_text(&entries[i]))
        .take_while(|&i| roles[i].is_none())
        .collect();
    if let Some((&last, rest)) = left.split_last() {
        roles[last] = Some(&nd30::CO_QUAN);
        for &i in rest {
            roles[i] = Some(&nd30::CO_QUAN_CHU_QUAN);
        }
    }
    // Signer columns: leading upper-case lines = authority and title, last line = name.
    let mut cells: Vec<[usize; 4]> = entries
        .iter()
        .filter(|e| e.area == Area::Signature)
        .filter_map(|e| e.cell)
        .collect();
    cells.dedup();
    for cell in cells {
        let lines: Vec<usize> = (0..entries.len())
            .filter(|&i| entries[i].cell == Some(cell) && has_text(&entries[i]))
            .collect();
        let titles = lines
            .iter()
            .take_while(|&&i| is_upper(&entries[i].text))
            .count();
        for &i in &lines[..titles] {
            roles[i] = Some(&nd30::CHUC_VU);
        }
        if let Some(&last) = lines.last().filter(|_| titles > 0 && lines.len() > titles) {
            roles[last] = Some(&nd30::HO_TEN);
        }
    }
    roles
}

// ---- component rules ---------------------------------------------------------------

fn check_component(
    doc: &Document,
    c: &'static Component,
    e: &Entry,
    entries: &[Entry],
    out: &mut Out,
) {
    let at = At {
        para: Some(e.id),
        section: None,
        text: Some(&e.text),
    };
    let runs: Vec<bo_model::RunProps> = text_runs(e.p)
        .map(|r| effective_run(doc, e.p, &r.props))
        .collect();
    if let Some(bad) = runs
        .iter()
        .map(|r| r.size.map_or(20, |h| h.0))
        .find(|&s| !c.size.contains(s))
    {
        out.add(c.id, "size", pt_range(c.size), pt(bad), &at);
    }
    if let Some(want) = c.bold {
        if runs.iter().any(|r| r.bold.unwrap_or(false) != want) {
            out.add(c.id, "bold", yes(want), yes(!want), &at);
        }
    }
    if let Some(want) = c.italic {
        if runs.iter().any(|r| r.italic.unwrap_or(false) != want) {
            out.add(c.id, "italic", yes(want), yes(!want), &at);
        }
    }
    if c.case == Case::Upper && !is_upper(&e.text) && !runs.iter().all(|r| r.caps.unwrap_or(false))
    {
        out.add(c.id, "case", "upper".into(), "mixed".into(), &at);
    }
    if let Some(want) = c.align {
        let got = effective_para(doc, &e.p.props)
            .align
            .unwrap_or(Alignment::Left);
        if got != want {
            out.add(
                c.id,
                "align",
                align_name(want).into(),
                align_name(got).into(),
                &at,
            );
        }
    }
    if matches!(c.area, Area::HeaderLeft | Area::HeaderRight) && e.area != c.area {
        out.add(
            c.id,
            "position",
            c.area.name().into(),
            e.area.name().into(),
            &at,
        );
    }
    if c.rule_below && !rule_follows(doc, e, entries) {
        out.add(c.id, "underline", "yes".into(), "no".into(), &at);
    }
}

/// A line follows the component: its own underline / bottom border, or the next
/// paragraph of the same container is a rule.
fn rule_follows(doc: &Document, e: &Entry, entries: &[Entry]) -> bool {
    if self_underlined(doc, e.p) {
        return true;
    }
    let parent = &e.path[..e.path.len() - 1];
    entries.iter().find(|n| n.id == e.id + 1).is_some_and(|n| {
        n.path.len() == e.path.len() && &n.path[..n.path.len() - 1] == parent && is_rule(n.p)
    })
}

fn check_missing(out: &mut Out) {
    let found = out.found.clone();
    for c in nd30::COMPONENTS.iter().filter(|c| c.required) {
        if !found.contains(&c.id) {
            out.add(
                c.id,
                "missing",
                "present".into(),
                "missing".into(),
                &NOWHERE,
            );
        }
    }
    // Every document names its type, except a công văn (its "V/v" summary instead).
    if !found.contains(&nd30::TEN_LOAI.id) && !found.contains(&nd30::TRICH_YEU_CONG_VAN.id) {
        out.add(
            nd30::TEN_LOAI.id,
            "missing",
            "present".into(),
            "missing".into(),
            &NOWHERE,
        );
    }
}

// ---- body text -----------------------------------------------------------------------

fn check_body(doc: &Document, entries: &[Entry], roles: &[Option<&Component>], out: &mut Out) {
    let header = entries
        .iter()
        .find(|e| matches!(e.area, Area::HeaderLeft | Area::HeaderRight))
        .map(Entry::block);
    let signature = entries
        .iter()
        .find(|e| matches!(e.area, Area::Signature | Area::SignatureLeft))
        .map(Entry::block);
    for (e, role) in entries.iter().zip(roles) {
        if role.is_some()
            || !e.top_level()
            || e.text.is_empty()
            || is_rule(e.p)
            || header.is_some_and(|h| e.block() < h)
            || signature.is_some_and(|s| e.block() > s)
        {
            continue;
        }
        let props = effective_para(doc, &e.p.props);
        let align = props.align.unwrap_or(Alignment::Left);
        if matches!(align, Alignment::Center | Alignment::Right) {
            // Centred headings (authority, "QUYẾT ĐỊNH:", chapter titles).
            continue;
        }
        let at = At {
            para: Some(e.id),
            section: None,
            text: Some(&e.text),
        };
        if let Some(bad) = text_runs(e.p)
            .map(|r| effective_run(doc, e.p, &r.props).size.map_or(20, |h| h.0))
            .find(|&s| !BODY.size.contains(s))
        {
            out.add("body", "size", pt_range(BODY.size), pt(bad), &at);
        }
        if align != BODY.align {
            out.add(
                "body",
                "align",
                align_name(BODY.align).into(),
                align_name(align).into(),
                &at,
            );
        }
        let first = props.indent_first_line.map_or(0, |t| t.0);
        if props.num.is_none() && !BODY.first_line.contains(first) {
            out.add(
                "body",
                "indent",
                "1–1.27 cm".into(),
                format!("{:.2} cm", nd30::twips_to_mm(first) / 10.0),
                &at,
            );
        }
        let spacing = props.space_before.map_or(0, |t| t.0) + props.space_after.map_or(0, |t| t.0);
        if spacing < BODY.min_paragraph_spacing {
            out.add(
                "body",
                "spacing",
                format!("≥ {} pt", BODY.min_paragraph_spacing / 20),
                format!("{} pt", f64::from(spacing) / 20.0),
                &at,
            );
        }
        let line_ok = match props.line_spacing.unwrap_or_default() {
            LineSpacing::Auto(v) => BODY.line_auto.contains(v),
            LineSpacing::Exact(t) | LineSpacing::AtLeast(t) => BODY.line_exact.contains(t.0),
        };
        if !line_ok {
            out.add(
                "body",
                "lineSpacing",
                "1–1.5".into(),
                line_name(props.line_spacing.unwrap_or_default()),
                &at,
            );
        }
    }
}

fn line_name(l: LineSpacing) -> String {
    match l {
        LineSpacing::Auto(v) => format!("{:.2}", f64::from(v) / 240.0),
        LineSpacing::Exact(t) | LineSpacing::AtLeast(t) => format!("{} pt", f64::from(t.0) / 20.0),
    }
}

// ---- page numbers ----------------------------------------------------------------------

fn page_field(blocks: &[Block]) -> Option<&Paragraph> {
    all_paragraphs(blocks).into_iter().find(|p| {
        p.runs.iter().flat_map(|r| &r.content).any(|c| match c {
            RunContent::Field { instr, .. } => instr
                .split_whitespace()
                .next()
                .is_some_and(|w| w.eq_ignore_ascii_case("PAGE")),
            RunContent::Opaque(r) => r.xml.contains(" PAGE ") || r.xml.contains(">PAGE<"),
            _ => false,
        })
    })
}

fn check_page_number(doc: &Document, out: &mut Out) {
    let Some(s) = doc.sections.first() else {
        return;
    };
    let c = &nd30::SO_TRANG;
    let header = s.headers.default.as_deref().and_then(page_field);
    let footer = [&s.footers.default, &s.footers.first, &s.footers.even]
        .into_iter()
        .flatten()
        .find_map(|b| page_field(b));
    let Some(p) = header else {
        if footer.is_some() {
            out.add(c.id, "position", "header".into(), "footer".into(), &NOWHERE);
        } else {
            out.add(
                c.id,
                "missing",
                "present".into(),
                "missing".into(),
                &NOWHERE,
            );
        }
        return;
    };
    out.found.push(c.id);
    let at = At {
        text: None,
        ..NOWHERE
    };
    let align = effective_para(doc, &p.props)
        .align
        .unwrap_or(Alignment::Left);
    if align != Alignment::Center {
        out.add(
            c.id,
            "align",
            "center".into(),
            align_name(align).into(),
            &at,
        );
    }
    let sizes = p
        .runs
        .iter()
        .filter(|r| {
            r.content
                .iter()
                .any(|x| matches!(x, RunContent::Field { .. }))
        })
        .map(|r| effective_run(doc, p, &r.props).size.map_or(20, |h| h.0));
    if let Some(bad) = sizes.into_iter().find(|&v| !c.size.contains(v)) {
        out.add(c.id, "size", pt_range(c.size), pt(bad), &at);
    }
    let first_shows = !s.title_page || s.headers.first.as_deref().and_then(page_field).is_some();
    if first_shows {
        out.add(c.id, "firstPage", "hidden".into(), "shown".into(), &at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{HalfPoints, Twips};

    fn report(d: &Document) -> Vec<String> {
        check(d).ids().into_iter().map(str::to_owned).collect()
    }

    #[test]
    fn every_template_passes() {
        for id in super::super::TEMPLATES {
            let d = super::super::build(id).expect("template");
            let r = check(&d);
            assert!(r.issues.is_empty(), "{id}: {:#?}", r.issues);
            for c in [
                "quocHieu", "tieuNgu", "coQuan", "soKyHieu", "diaDanh", "chucVu", "hoTen",
                "soTrang",
            ] {
                assert!(r.found.iter().any(|f| f == c), "{id}: {c} not recognised");
            }
        }
    }

    fn para_mut<'d>(d: &'d mut Document, text: &str) -> &'d mut Paragraph {
        let paths = index::body_paragraphs(d);
        let path = paths
            .into_iter()
            .find(|p| index::para_at(d, p).is_some_and(|x| x.plain_text() == text))
            .expect("paragraph");
        index::para_at_mut(d, &path).expect("paragraph")
    }

    #[test]
    fn broken_variants_report_predictable_issues() {
        // Left margin 25 mm.
        let mut d = super::super::build("cong-van").expect("template");
        d.sections[0].page.margin_left = Twips(nd30::mm_to_twips(25.0));
        assert_eq!(report(&d), ["page.marginLeft"]);
        let i = &check(&d).issues[0];
        assert_eq!(
            (i.expected.as_str(), i.actual.as_str()),
            ("30–35 mm", "25.0 mm")
        );

        // Quốc hiệu in 14 pt.
        let mut d = super::super::build("quyet-dinh").expect("template");
        for r in &mut para_mut(&mut d, "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM").runs {
            r.props.size = Some(HalfPoints(28));
        }
        let r = check(&d);
        assert_eq!(r.ids(), ["quocHieu.size"]);
        assert_eq!(r.issues[0].expected, "12–13 pt");
        assert_eq!(r.issues[0].actual, "14 pt");

        // The line below the motto removed.
        let mut d = super::super::build("bao-cao").expect("template");
        let paths = index::body_paragraphs(&d);
        let motto = paths
            .iter()
            .position(|p| {
                index::para_at(&d, p)
                    .is_some_and(|x| x.plain_text() == "Độc lập - Tự do - Hạnh phúc")
            })
            .expect("motto");
        let line = paths[motto + 1].clone();
        if let Some(Block::Table(t)) = d.sections[0].blocks.get_mut(line[1]) {
            t.rows[line[2]].cells[line[3]].blocks.remove(line[4]);
        }
        assert_eq!(report(&d), ["tieuNgu.underline"]);
    }

    #[test]
    fn other_deviations() {
        let mut d = super::super::build("to-trinh").expect("template");
        d.default_run.font = Some("Arial".into());
        d.sections[0].title_page = false;
        para_mut(&mut d, "[Nội dung …]").props.indent_first_line = None;
        let r = report(&d);
        assert!(r.contains(&"font.font".to_owned()), "{r:?}");
        assert!(r.contains(&"body.indent".to_owned()), "{r:?}");
        assert!(r.contains(&"soTrang.firstPage".to_owned()), "{r:?}");
        // An empty document misses every required component.
        let r = report(&Document::default());
        assert!(r.contains(&"quocHieu.missing".to_owned()));
        assert!(r.contains(&"tenLoai.missing".to_owned()));
    }
}
