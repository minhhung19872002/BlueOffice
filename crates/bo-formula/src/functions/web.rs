//! Web functions ([MS-XLSX] future functions `_xlfn.ENCODEURL`, `_xlfn.WEBSERVICE`,
//! `_xlfn.FILTERXML`).
//!
//! * `ENCODEURL(text)` percent-encodes the UTF-8 bytes of the text, keeping only the RFC 3986
//!   §2.3 unreserved characters (`A-Z a-z 0-9 - _ . ~`).
//! * `FILTERXML(xml, xpath)` evaluates an XPath 1.0 subset over the XML text (see
//!   [`super::xpath`]); it is pure and always available.
//! * `WEBSERVICE(url)` asks the host's [`crate::WebProvider`]; `bo-formula` itself never
//!   touches the network, and without a provider (the default) web access is disabled.

use std::sync::Arc;

use super::{s, xpath, FnSpec};
use crate::eval::{Ctx, R};
use crate::value::{Array, ErrorKind, Value};

pub(super) const FUNCTIONS: &[FnSpec] = &[
    s("ENCODEURL", 1, 1, encodeurl),
    s("FILTERXML", 2, 2, filterxml),
    // The response can change without any precedent changing: recalculated on every refresh.
    s("WEBSERVICE", 1, 1, webservice).volatile(),
];

/// Longest text a cell holds; longer results are `#VALUE!`.
const MAX_TEXT: usize = 32_767;
/// Longest URL `WEBSERVICE` accepts.
const MAX_URL: usize = 2_048;

/// `ENCODEURL(text)`: RFC 3986 §2.1 percent-encoding (upper-case hex digits) of every UTF-8
/// byte outside the unreserved set of §2.3.
fn encodeurl(ctx: &mut Ctx, args: &[Value]) -> R {
    let text = ctx.text(&args[0])?;
    let encoded = encode_url(&text);
    if encoded.chars().count() > MAX_TEXT {
        return Err(ErrorKind::Value);
    }
    Ok(Value::Text(encoded))
}

pub(crate) fn encode_url(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(text.len());
    for &b in text.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(b >> 4)]));
            out.push(char::from(HEX[usize::from(b & 0x0F)]));
        }
    }
    out
}

/// `FILTERXML(xml, xpath)`: the string values of the selected nodes, numbers converted; one
/// node gives a single value, several a one-column array (document order). Invalid XML, an
/// invalid or non-node-set XPath, or no matching node give `#VALUE!`.
fn filterxml(ctx: &mut Ctx, args: &[Value]) -> R {
    let xml = ctx.text(&args[0])?;
    let path = ctx.text(&args[1])?;
    let values = xpath::filter_xml(&xml, &path).ok_or(ErrorKind::Value)?;
    let mut cells: Vec<Value> = values
        .into_iter()
        .map(|v| match xpath::as_number(&v) {
            Some(n) => Value::number(n),
            None => Value::Text(v),
        })
        .collect();
    if cells
        .iter()
        .any(|c| matches!(c, Value::Text(t) if t.chars().count() > MAX_TEXT))
    {
        return Err(ErrorKind::Value);
    }
    match cells.len() {
        0 => Err(ErrorKind::Value),
        1 => Ok(cells.pop().unwrap_or(Value::Error(ErrorKind::Value))),
        n => Ok(Value::Array(Arc::new(Array::new(n, 1, cells)))),
    }
}

/// `WEBSERVICE(url)`: the response body of an HTTP(S) GET, fetched by the host.
///
/// Every failure is `#VALUE!` — the error the function's published definition gives for a
/// request that cannot be completed — including "web access disabled" (no provider
/// installed), so a workbook opened where web access is off shows the same error it shows
/// offline. A URL that is not `http://` / `https://`, longer than 2048 characters, or a
/// response longer than 32767 characters is also `#VALUE!`.
fn webservice(ctx: &mut Ctx, args: &[Value]) -> R {
    let url = ctx.text(&args[0])?;
    if !is_web_url(&url) {
        return Err(ErrorKind::Value);
    }
    let provider = ctx.web.ok_or(ErrorKind::Value)?;
    let body = provider.fetch(&url).map_err(|_| ErrorKind::Value)?;
    if body.chars().count() > MAX_TEXT {
        return Err(ErrorKind::Value);
    }
    Ok(Value::Text(body))
}

/// An absolute `http` / `https` URL (scheme case-insensitive, RFC 3986 §3.1) with a host,
/// at most [`MAX_URL`] characters, without whitespace or control characters.
fn is_web_url(url: &str) -> bool {
    if url.chars().count() > MAX_URL || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"));
    rest.is_some_and(|r| {
        let host = r.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_urls() {
        assert!(is_web_url("https://example.com/a?b=1"));
        assert!(is_web_url("HTTP://example.com"));
        assert!(!is_web_url("ftp://example.com"));
        assert!(!is_web_url("file:///c:/x"));
        assert!(!is_web_url("https://"));
        assert!(!is_web_url("https://exa mple.com"));
        let long = format!("https://e.com/{}", "a".repeat(MAX_URL));
        assert!(!is_web_url(&long));
    }
}
