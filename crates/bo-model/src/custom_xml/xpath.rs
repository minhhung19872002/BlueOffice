//! The XPath 1.0 subset used by content control data binding (ECMA-376 Part 1, §17.5.2.6
//! dataBinding `w:xpath`): absolute location paths of child steps with namespace-prefixed name
//! tests (`ns0:item`, `item`, `*`, `ns0:*`), positional predicates (`[2]`), an optional final
//! attribute step (`@code`, `@ns0:code`) or `text()` step. Prefixes resolve through the
//! binding's `w:prefixMappings` (§17.5.2.27; `xmlns:ns0='uri'` declarations).
//!
//! Written from the XPath 1.0 recommendation (W3C, 1999): §2 location paths, §2.3 node tests
//! (an unprefixed name test matches names in no namespace), §2.4 predicates (a number `n` is
//! `position() = n`), §5.2 string-value of an element.

use super::dom::{XDocument, XElement, XNode};

/// Name test of a step (§2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameTest {
    /// `*`: any element (or attribute).
    Any,
    /// `prefix:*`: any name in a namespace.
    AnyIn(String),
    /// `prefix:local` or `local`.
    Name {
        /// Prefix (`None`: no namespace).
        prefix: Option<String>,
        /// Local name.
        local: String,
    },
}

/// One child step with its positional predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Name test.
    pub test: NameTest,
    /// Positional predicates in order (1-based).
    pub positions: Vec<usize>,
}

/// What the path ends with after its element steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Last {
    /// The last element step.
    Element,
    /// `@name`: an attribute of the last element.
    Attribute(NameTest),
    /// `text()`: the text of the last element.
    Text,
}

/// A parsed binding path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XPath {
    /// Element steps from the root (the first one tests the document element).
    pub steps: Vec<Step>,
    /// Final attribute / text step.
    pub last: Last,
}

/// Why a path is not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XPathError(pub String);

impl std::fmt::Display for XPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for XPathError {}

fn name_test(s: &str) -> Result<NameTest, XPathError> {
    let bad = || XPathError(format!("unsupported name test '{s}'"));
    let ok_name = |n: &str| {
        !n.is_empty()
            && n.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
            && n.chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
    };
    if s == "*" {
        return Ok(NameTest::Any);
    }
    match s.split_once(':') {
        Some((p, "*")) if ok_name(p) => Ok(NameTest::AnyIn(p.to_owned())),
        Some((p, l)) if ok_name(p) && ok_name(l) => Ok(NameTest::Name {
            prefix: Some(p.to_owned()),
            local: l.to_owned(),
        }),
        None if ok_name(s) => Ok(NameTest::Name {
            prefix: None,
            local: s.to_owned(),
        }),
        _ => Err(bad()),
    }
}

impl XPath {
    /// Parse an absolute location path of the supported subset.
    ///
    /// # Errors
    /// Relative paths, other axes, functions or non-positional predicates.
    pub fn parse(expr: &str) -> Result<XPath, XPathError> {
        let e: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
        if !e.starts_with('/') || e.starts_with("//") {
            return Err(XPathError("only absolute child paths are supported".into()));
        }
        let mut steps = Vec::new();
        let mut last = Last::Element;
        let parts: Vec<&str> = e[1..].split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            let is_last = i + 1 == parts.len();
            if part.is_empty() {
                return Err(XPathError("empty step".into()));
            }
            if let Some(a) = part.strip_prefix('@') {
                if !is_last || steps.is_empty() {
                    return Err(XPathError("an attribute step must come last".into()));
                }
                last = Last::Attribute(name_test(a)?);
                continue;
            }
            if *part == "text()" {
                if !is_last || steps.is_empty() {
                    return Err(XPathError("text() must come last".into()));
                }
                last = Last::Text;
                continue;
            }
            let (name, mut preds) = match part.find('[') {
                Some(b) => (&part[..b], &part[b..]),
                None => (*part, ""),
            };
            let name = name.strip_prefix("child::").unwrap_or(name);
            let mut positions = Vec::new();
            while !preds.is_empty() {
                let close = preds
                    .find(']')
                    .ok_or_else(|| XPathError("unterminated predicate".into()))?;
                let inner = &preds[1..close];
                let n: usize = inner
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| XPathError(format!("unsupported predicate [{inner}]")))?;
                positions.push(n);
                preds = &preds[close + 1..];
                if !preds.is_empty() && !preds.starts_with('[') {
                    return Err(XPathError("unexpected text after a predicate".into()));
                }
            }
            steps.push(Step {
                test: name_test(name)?,
                positions,
            });
        }
        if steps.is_empty() {
            return Err(XPathError("empty path".into()));
        }
        Ok(XPath { steps, last })
    }
}

/// Parse `w:prefixMappings` (§17.5.2.27): whitespace-separated `xmlns:prefix='uri'` (or
/// `"uri"`) declarations.
#[must_use]
pub fn prefix_mappings(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s.trim();
    while let Some(i) = rest.find("xmlns") {
        rest = &rest[i + 5..];
        let (prefix, after) = match rest.strip_prefix(':') {
            Some(r) => {
                let end = r
                    .find(|c: char| c == '=' || c.is_whitespace())
                    .unwrap_or(r.len());
                (r[..end].to_owned(), &r[end..])
            }
            None => (String::new(), rest),
        };
        let after = after
            .trim_start()
            .strip_prefix('=')
            .unwrap_or(after)
            .trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '\'' || *c == '"') else {
            rest = after;
            continue;
        };
        let body = &after[1..];
        let Some(end) = body.find(q) else { break };
        out.push((prefix, body[..end].to_owned()));
        rest = &body[end + 1..];
    }
    out
}

/// Format prefix mappings as `w:prefixMappings` expects.
#[must_use]
pub fn format_mappings(m: &[(String, String)]) -> String {
    m.iter()
        .map(|(p, u)| {
            if p.is_empty() {
                format!("xmlns='{u}'")
            } else {
                format!("xmlns:{p}='{u}'")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A selected node: child-node indices from the root to the element, and an attribute index
/// (`None`: the element itself).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Target {
    /// Indices into `children` from the document element down.
    pub path: Vec<usize>,
    /// Index into the element's `attrs`.
    pub attr: Option<usize>,
}

/// Namespace scope while walking: (prefix, uri) declarations, innermost last.
type Scope = Vec<(Option<String>, String)>;

fn push_decls(el: &XElement, scope: &mut Scope) -> usize {
    let n = scope.len();
    for (k, v) in &el.attrs {
        if k == "xmlns" {
            scope.push((None, v.clone()));
        } else if let Some(p) = k.strip_prefix("xmlns:") {
            scope.push((Some(p.to_owned()), v.clone()));
        }
    }
    n
}

fn lookup<'s>(scope: &'s Scope, prefix: Option<&str>) -> Option<&'s str> {
    scope
        .iter()
        .rev()
        .find(|(p, _)| p.as_deref() == prefix)
        .map(|(_, u)| u.as_str())
        .filter(|u| !u.is_empty() || prefix.is_none())
}

/// Namespace URI of an element name (`""`: no namespace).
fn element_ns<'s>(el: &XElement, scope: &'s Scope) -> &'s str {
    lookup(scope, el.prefix()).unwrap_or("")
}

struct Ctx<'a> {
    mappings: &'a [(String, String)],
}

impl Ctx<'_> {
    /// URI of an XPath prefix: the binding's mappings, else (leniently) the document's own
    /// declaration of that prefix at this point.
    fn uri<'s>(&'s self, prefix: &str, scope: &'s Scope) -> Option<&'s str> {
        self.mappings
            .iter()
            .find(|(p, _)| p == prefix)
            .map(|(_, u)| u.as_str())
            .or_else(|| lookup(scope, Some(prefix)))
    }

    fn matches(&self, t: &NameTest, ns: &str, local: &str, scope: &Scope) -> bool {
        match t {
            NameTest::Any => true,
            NameTest::AnyIn(p) => self.uri(p, scope) == Some(ns),
            NameTest::Name { prefix, local: l } => {
                l == local
                    && match prefix {
                        None => ns.is_empty(),
                        Some(p) => self.uri(p, scope) == Some(ns),
                    }
            }
        }
    }

    /// First match in document order of `steps` among `el`'s children (child axis).
    fn walk(
        &self,
        el: &XElement,
        steps: &[Step],
        last: &Last,
        scope: &mut Scope,
        path: &mut Vec<usize>,
    ) -> Option<Target> {
        let Some((step, rest)) = steps.split_first() else {
            return self.finish(el, last, scope, path);
        };
        let mut candidates: Vec<usize> = Vec::new();
        for (i, c) in el.children.iter().enumerate() {
            let XNode::Element(ch) = c else { continue };
            let mark = push_decls(ch, scope);
            if self.matches(&step.test, element_ns(ch, scope), ch.local(), scope) {
                candidates.push(i);
            }
            scope.truncate(mark);
        }
        for &p in &step.positions {
            candidates = candidates.get(p - 1).map(|i| vec![*i]).unwrap_or_default();
        }
        for i in candidates {
            let XNode::Element(ch) = &el.children[i] else {
                continue;
            };
            let mark = push_decls(ch, scope);
            path.push(i);
            let found = self.walk(ch, rest, last, scope, path);
            path.pop();
            scope.truncate(mark);
            if found.is_some() {
                return found;
            }
        }
        None
    }

    fn finish(&self, el: &XElement, last: &Last, scope: &Scope, path: &[usize]) -> Option<Target> {
        let attr = match last {
            Last::Element | Last::Text => None,
            Last::Attribute(t) => Some(el.attrs.iter().position(|(k, _)| {
                if k == "xmlns" || k.starts_with("xmlns:") {
                    return false;
                }
                // §2.3: an unprefixed attribute name is in no namespace.
                let (ns, local) = match k.split_once(':') {
                    Some((p, l)) => (lookup(scope, Some(p)).unwrap_or(""), l),
                    None => ("", k.as_str()),
                };
                self.matches(t, ns, local, scope)
            })?),
        };
        Some(Target {
            path: path.to_vec(),
            attr,
        })
    }
}

/// The first node the path selects (document order), or `None`.
#[must_use]
pub fn select(doc: &XDocument, xp: &XPath, mappings: &[(String, String)]) -> Option<Target> {
    let ctx = Ctx { mappings };
    let (first, rest) = xp.steps.split_first()?;
    let mut scope: Scope = vec![(
        Some("xml".to_owned()),
        "http://www.w3.org/XML/1998/namespace".to_owned(),
    )];
    push_decls(&doc.root, &mut scope);
    let root = &doc.root;
    let ok = ctx.matches(&first.test, element_ns(root, &scope), root.local(), &scope)
        && first.positions.iter().all(|p| *p == 1);
    if !ok {
        return None;
    }
    ctx.walk(root, rest, &xp.last, &mut scope, &mut Vec::new())
}

fn element_at<'d>(doc: &'d XDocument, path: &[usize]) -> Option<&'d XElement> {
    let mut el = &doc.root;
    for &i in path {
        match el.children.get(i)? {
            XNode::Element(e) => el = e,
            _ => return None,
        }
    }
    Some(el)
}

fn element_at_mut<'d>(doc: &'d mut XDocument, path: &[usize]) -> Option<&'d mut XElement> {
    let mut el = &mut doc.root;
    for &i in path {
        match el.children.get_mut(i)? {
            XNode::Element(e) => el = e,
            _ => return None,
        }
    }
    Some(el)
}

/// String-value of the selected node (§5.2 for an element, the value for an attribute).
#[must_use]
pub fn value(doc: &XDocument, t: &Target) -> Option<String> {
    let el = element_at(doc, &t.path)?;
    match t.attr {
        Some(a) => el.attrs.get(a).map(|(_, v)| v.clone()),
        None => Some(el.string_value()),
    }
}

/// Set the value of the selected node. An element with child elements is not changed (its
/// content is structure, not a value): returns `false`.
pub fn set_value(doc: &mut XDocument, t: &Target, v: &str) -> bool {
    let Some(el) = element_at_mut(doc, &t.path) else {
        return false;
    };
    match t.attr {
        Some(a) => match el.attrs.get_mut(a) {
            Some(slot) => {
                v.clone_into(&mut slot.1);
                true
            }
            None => false,
        },
        None if el.has_elements() => false,
        None => {
            el.set_text(v);
            true
        }
    }
}

/// A node of a part as the XML mapping pane lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineNode {
    /// Qualified name as written (`@name` for an attribute).
    pub name: String,
    /// Binding path to this node (canonical: every step with its position).
    pub xpath: String,
    /// Depth (0: the document element).
    pub depth: usize,
    /// Value of a leaf element or attribute (`None` for an element with child elements).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Every element and attribute of a part (up to `limit` nodes) with canonical binding paths,
/// and the prefix mappings those paths use (`ns0`, `ns1`, … in order of first use).
#[must_use]
pub fn outline(doc: &XDocument, limit: usize) -> (Vec<OutlineNode>, Vec<(String, String)>) {
    struct W {
        nodes: Vec<OutlineNode>,
        maps: Vec<(String, String)>,
        limit: usize,
    }
    impl W {
        fn prefix_for(&mut self, uri: &str) -> Option<String> {
            if uri.is_empty() {
                return None;
            }
            if let Some((p, _)) = self.maps.iter().find(|(_, u)| u == uri) {
                return Some(p.clone());
            }
            let p = format!("ns{}", self.maps.len());
            self.maps.push((p.clone(), uri.to_owned()));
            Some(p)
        }
        fn step(&mut self, uri: &str, local: &str) -> String {
            match self.prefix_for(uri) {
                Some(p) => format!("{p}:{local}"),
                None => local.to_owned(),
            }
        }
        fn visit(&mut self, el: &XElement, xpath: &str, depth: usize, scope: &mut Scope) {
            if self.nodes.len() >= self.limit {
                return;
            }
            let leaf = !el.has_elements();
            self.nodes.push(OutlineNode {
                name: el.name.clone(),
                xpath: xpath.to_owned(),
                depth,
                value: leaf.then(|| el.string_value()),
            });
            for (k, v) in &el.attrs {
                if k == "xmlns" || k.starts_with("xmlns:") || self.nodes.len() >= self.limit {
                    continue;
                }
                let step = match k.split_once(':') {
                    Some((p, l)) => {
                        let uri = lookup(scope, Some(p)).unwrap_or("").to_owned();
                        self.step(&uri, l)
                    }
                    None => k.clone(),
                };
                self.nodes.push(OutlineNode {
                    name: format!("@{k}"),
                    xpath: format!("{xpath}/@{step}"),
                    depth: depth + 1,
                    value: Some(v.clone()),
                });
            }
            let mut seen: Vec<(String, String, usize)> = Vec::new();
            for c in &el.children {
                let XNode::Element(ch) = c else { continue };
                let mark = push_decls(ch, scope);
                let uri = element_ns(ch, scope).to_owned();
                let local = ch.local().to_owned();
                let n = if let Some(s) = seen.iter_mut().find(|(u, l, _)| *u == uri && *l == local)
                {
                    s.2 += 1;
                    s.2
                } else {
                    seen.push((uri.clone(), local.clone(), 1));
                    1
                };
                let step = self.step(&uri, &local);
                self.visit(ch, &format!("{xpath}/{step}[{n}]"), depth + 1, scope);
                scope.truncate(mark);
            }
        }
    }
    let mut w = W {
        nodes: Vec::new(),
        maps: Vec::new(),
        limit,
    };
    let mut scope: Scope = Vec::new();
    push_decls(&doc.root, &mut scope);
    let uri = element_ns(&doc.root, &scope).to_owned();
    let step = w.step(&uri, doc.root.local());
    w.visit(&doc.root, &format!("/{step}[1]"), 0, &mut scope);
    (w.nodes, w.maps)
}

#[cfg(test)]
mod tests {
    use super::super::dom::parse;
    use super::*;

    const DOC: &str = r#"<inv:invoice xmlns:inv="urn:example:invoice" xmlns:c="urn:example:customer" id="A-17">
  <c:customer kind="org"><c:name>Bluestar</c:name><c:city>Hà Nội</c:city></c:customer>
  <inv:line><inv:item>Pen</inv:item><inv:qty>2</inv:qty></inv:line>
  <inv:line><inv:item>Ink</inv:item><inv:qty>5</inv:qty></inv:line>
  <note>plain</note>
</inv:invoice>"#;

    fn get(path: &str, maps: &str) -> Option<String> {
        let d = parse(DOC).expect("parse");
        let xp = XPath::parse(path).expect("xpath");
        let t = select(&d, &xp, &prefix_mappings(maps))?;
        value(&d, &t)
    }

    const MAPS: &str = "xmlns:ns0='urn:example:invoice' xmlns:ns1=\"urn:example:customer\"";

    #[test]
    fn selects_elements_by_namespace_and_position() {
        // The prefixes of the path need not match the document's own.
        assert_eq!(
            get("/ns0:invoice[1]/ns1:customer[1]/ns1:name[1]", MAPS).as_deref(),
            Some("Bluestar")
        );
        assert_eq!(
            get("/ns0:invoice/ns0:line[2]/ns0:item", MAPS).as_deref(),
            Some("Ink")
        );
        assert_eq!(
            get("/ns0:invoice/ns0:line/ns0:qty", MAPS).as_deref(),
            Some("2")
        );
        assert_eq!(get("/ns0:invoice/ns0:line[3]/ns0:item", MAPS), None);
        // Unprefixed name tests match elements in no namespace (XPath 1.0 §2.3).
        assert_eq!(get("/ns0:invoice/note", MAPS).as_deref(), Some("plain"));
        assert_eq!(get("/invoice", MAPS), None);
        assert_eq!(
            get("/ns0:invoice/ns0:line[2]/*[2]", MAPS).as_deref(),
            Some("5")
        );
        assert_eq!(
            get("/ns0:invoice/ns1:customer/ns1:city/text()", MAPS).as_deref(),
            Some("Hà Nội")
        );
        // The element's string-value joins its descendants' text.
        assert_eq!(
            get("/ns0:invoice/ns0:line[1]", MAPS).as_deref(),
            Some("Pen2")
        );
    }

    #[test]
    fn selects_attributes() {
        assert_eq!(get("/ns0:invoice/@id", MAPS).as_deref(), Some("A-17"));
        assert_eq!(
            get("/ns0:invoice/ns1:customer/@kind", MAPS).as_deref(),
            Some("org")
        );
        assert_eq!(get("/ns0:invoice/@missing", MAPS), None);
    }

    #[test]
    fn unknown_prefix_falls_back_to_document_declarations() {
        assert_eq!(
            get("/inv:invoice/c:customer/c:name", "").as_deref(),
            Some("Bluestar")
        );
        assert_eq!(get("/zz:invoice", ""), None);
    }

    #[test]
    fn rejects_unsupported_syntax() {
        for bad in [
            "a/b",
            "//a",
            "/a[@x='1']",
            "/a[0]",
            "/@x",
            "/a/@x/b",
            "/a/count(b)",
            "/",
            "/a//b",
        ] {
            assert!(XPath::parse(bad).is_err(), "{bad}");
        }
        let xp = XPath::parse(" /ns0:a [ 2 ] / child::b[1][1] ").expect("ok");
        assert_eq!(xp.steps.len(), 2);
        assert_eq!(xp.steps[0].positions, vec![2]);
        assert_eq!(xp.steps[1].positions, vec![1, 1]);
    }

    #[test]
    fn set_value_changes_leaves_only() {
        let mut d = parse(DOC).expect("parse");
        let m = prefix_mappings(MAPS);
        let t = select(
            &d,
            &XPath::parse("/ns0:invoice/ns0:line[2]/ns0:qty").expect("x"),
            &m,
        )
        .expect("node");
        assert!(set_value(&mut d, &t, "7 & more"));
        assert_eq!(value(&d, &t).as_deref(), Some("7 & more"));
        let line = select(
            &d,
            &XPath::parse("/ns0:invoice/ns0:line[2]").expect("x"),
            &m,
        )
        .expect("node");
        assert!(!set_value(&mut d, &line, "x"));
        let a = select(&d, &XPath::parse("/ns0:invoice/@id").expect("x"), &m).expect("node");
        assert!(set_value(&mut d, &a, "B-1"));
        let out = super::super::dom::write(&d);
        assert!(out.contains("id=\"B-1\""));
        assert!(out.contains("<inv:qty>7 &amp; more</inv:qty>"));
    }

    #[test]
    fn prefix_mapping_forms() {
        assert_eq!(
            prefix_mappings("xmlns:ns0='urn:a'  xmlns:ns1=\"urn:b\" xmlns='urn:d'"),
            vec![
                ("ns0".to_owned(), "urn:a".to_owned()),
                ("ns1".to_owned(), "urn:b".to_owned()),
                (String::new(), "urn:d".to_owned())
            ]
        );
        assert_eq!(
            format_mappings(&[("ns0".into(), "urn:a".into())]),
            "xmlns:ns0='urn:a'"
        );
    }

    #[test]
    fn outline_paths_select_their_nodes() {
        let d = parse(DOC).expect("parse");
        let (nodes, maps) = outline(&d, 100);
        let paths: Vec<&str> = nodes.iter().map(|n| n.xpath.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/ns0:invoice[1]",
                "/ns0:invoice[1]/@id",
                "/ns0:invoice[1]/ns1:customer[1]",
                "/ns0:invoice[1]/ns1:customer[1]/@kind",
                "/ns0:invoice[1]/ns1:customer[1]/ns1:name[1]",
                "/ns0:invoice[1]/ns1:customer[1]/ns1:city[1]",
                "/ns0:invoice[1]/ns0:line[1]",
                "/ns0:invoice[1]/ns0:line[1]/ns0:item[1]",
                "/ns0:invoice[1]/ns0:line[1]/ns0:qty[1]",
                "/ns0:invoice[1]/ns0:line[2]",
                "/ns0:invoice[1]/ns0:line[2]/ns0:item[1]",
                "/ns0:invoice[1]/ns0:line[2]/ns0:qty[1]",
                "/ns0:invoice[1]/note[1]",
            ]
        );
        assert_eq!(
            maps[0],
            ("ns0".to_owned(), "urn:example:invoice".to_owned())
        );
        assert_eq!(
            maps[1],
            ("ns1".to_owned(), "urn:example:customer".to_owned())
        );
        assert_eq!(nodes[4].value.as_deref(), Some("Bluestar"));
        assert_eq!(nodes[2].value, None);
        for n in &nodes {
            let t = select(&d, &XPath::parse(&n.xpath).expect("x"), &maps).expect("selects");
            if let Some(v) = &n.value {
                assert_eq!(value(&d, &t).as_ref(), Some(v), "{}", n.xpath);
            }
        }
        assert_eq!(outline(&d, 3).0.len(), 3);
    }
}
