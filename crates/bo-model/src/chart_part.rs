//! Charts in WordprocessingML documents (CORE-CHART-007 / CORE-CHART-004 in BlueDoc).
//!
//! A chart is a `w:drawing` (ECMA-376 Part 1, §17.3.3.9) whose `a:graphicData` (§20.1.2.2.17) holds
//! a `c:chart` reference (§21.2.2.27) to a chart part (§21.2.2.29 `c:chartSpace`), or — for the
//! extended chart types — a `cx:chart` reference to a chartex part ([MS-ODRAWXML] 2.24), the frame
//! then sitting in an `mc:AlternateContent` choice (ECMA-376 Part 3, §10.2.1).
//!
//! In the document model a chart is an [`crate::Image`] whose media is its chart part: the part's
//! XML is a media part of the document, so an edit — part and frame together — is one undo step.
//! A chart BlueOffice creates or edits names its embedded workbook (§21.2.2.63 externalData →
//! §15.2.11 package relationship) by [`workbook_of`], and its chart part refers to it with the
//! relationship id [`WORKBOOK_REL_ID`]; the writer adds the chart part's relationships.

/// `graphicData@uri` of a chart frame (§21.2.2.27 chart).
pub const URI_CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
/// Chartex namespace, also the `graphicData@uri` of a chartex frame ([MS-ODRAWXML] 2.24).
pub const NS_CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
/// Content type of a chart part (§21.2.2.29 chartSpace).
pub const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
/// Content type of a chartex part ([MS-ODRAWXML]).
pub const CT_CHARTEX: &str = "application/vnd.ms-office.chartex+xml";
/// Relationship type document → chart part (ECMA-376 Part 1, Chart part).
pub const REL_CHART: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
/// Relationship type document → chartex part ([MS-ODRAWXML]).
pub const REL_CHARTEX: &str = "http://schemas.microsoft.com/office/2014/relationships/chartEx";
/// Relationship id of the embedded workbook in a chart part BlueOffice writes.
pub const WORKBOOK_REL_ID: &str = "rId1";
/// Folder of the chart parts BlueOffice writes.
pub const DIR: &str = "/word/charts";

/// Is `name` a chart or chartex part name (a `.xml` part in a `charts` folder)?
#[must_use]
pub fn is_chart_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("/charts/") && n.ends_with(".xml") && !n.contains("/_rels/")
}

/// Do these bytes hold a chart part (`c:chartSpace` or `cx:chartSpace`)?
#[must_use]
pub fn is_chart_xml(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(2048)];
    let s = String::from_utf8_lossy(head);
    s.contains("chartSpace")
}

/// Does a chart part hold a chartex chart (root `cx:chartSpace` in the chartex namespace)?
#[must_use]
pub fn is_chartex_xml(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(2048)];
    let s = String::from_utf8_lossy(head);
    // The root start tag: the first element that is not the XML declaration or a comment.
    let mut rest = s.as_ref();
    while let Some(i) = rest.find('<') {
        rest = &rest[i..];
        if rest.starts_with("<?") || rest.starts_with("<!") {
            rest = &rest[1..];
            continue;
        }
        let tag = &rest[..rest.find('>').unwrap_or(rest.len())];
        return tag.contains("chartSpace") && tag.contains(NS_CX);
    }
    false
}

/// `layoutId` of the first series of a chartex part ([MS-ODRAWXML] `cx:series@layoutId`).
#[must_use]
pub fn chartex_layout(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    let key = "layoutId=\"";
    s.find(key)
        .and_then(|i| {
            let rest = &s[i + key.len()..];
            rest.find('"').map(|j| rest[..j].to_owned())
        })
        .unwrap_or_default()
}

/// The `mc:Choice@Requires` prefix and namespace a chartex frame of layout `layout` needs
/// ([MS-ODRAWXML] namespaces: funnels came with the 2015/10/21 chartex namespace, region maps with
/// the 2016/5/10 one, the other layouts with the 2015/9/8 one).
#[must_use]
pub fn chartex_requires(layout: &str) -> (&'static str, &'static str) {
    match layout {
        "funnel" => (
            "cx2",
            "http://schemas.microsoft.com/office/drawing/2015/10/21/chartex",
        ),
        "regionMap" => (
            "cx4",
            "http://schemas.microsoft.com/office/drawing/2016/5/10/chartex",
        ),
        _ => (
            "cx1",
            "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex",
        ),
    }
}

/// Text of the `mc:Fallback` of a chartex frame (shown by applications without chartex).
pub const CHARTEX_FALLBACK: &str = "Biểu đồ mở rộng (chartex): cần ứng dụng hỗ trợ để hiển thị.";

/// Is `name` a chartex part (`chartEx<n>.xml`)?
#[must_use]
pub fn is_chartex_name(name: &str) -> bool {
    name.rsplit('/')
        .next()
        .is_some_and(|f| f.to_ascii_lowercase().starts_with("chartex"))
}

/// Part name of chart number `n`: `/word/charts/chart<n>.xml` (`chartEx<n>.xml` for chartex).
#[must_use]
pub fn chart_name(n: usize, ex: bool) -> String {
    format!("{DIR}/{}{n}.xml", if ex { "chartEx" } else { "chart" })
}

/// The embedded workbook of a chart part BlueOffice writes: `/word/charts/chart3.xml` →
/// `/word/embeddings/chart3.xlsx`.
#[must_use]
pub fn workbook_of(chart: &str) -> String {
    let file = chart.rsplit('/').next().unwrap_or(chart);
    let stem = file.strip_suffix(".xml").unwrap_or(file);
    format!("/word/embeddings/{stem}.xlsx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_chart_name("/word/charts/chart1.xml"));
        assert!(is_chart_name("/word/charts/chartEx2.xml"));
        assert!(!is_chart_name("/word/charts/_rels/chart1.xml.rels"));
        assert!(!is_chart_name("/word/diagrams/data1.xml"));
        assert!(is_chartex_name("/word/charts/chartEx2.xml"));
        assert!(!is_chartex_name("/word/charts/chart2.xml"));
        assert_eq!(chart_name(3, true), "/word/charts/chartEx3.xml");
        assert_eq!(
            workbook_of("/word/charts/chart3.xml"),
            "/word/embeddings/chart3.xlsx"
        );
        assert!(is_chart_xml(b"<?xml version=\"1.0\"?><c:chartSpace/>"));
        assert!(!is_chart_xml(b"<dgm:dataModel/>"));
        let cx = format!(
            "<?xml version=\"1.0\"?><cx:chartSpace xmlns:cx=\"{NS_CX}\"><cx:series layoutId=\"funnel\"/></cx:chartSpace>"
        );
        assert!(is_chartex_xml(cx.as_bytes()));
        assert!(!is_chartex_xml(b"<c:chartSpace xmlns:c=\"x\"/>"));
        assert_eq!(chartex_layout(cx.as_bytes()), "funnel");
        assert_eq!(chartex_requires("funnel").0, "cx2");
        assert_eq!(chartex_requires("waterfall").0, "cx1");
    }
}
