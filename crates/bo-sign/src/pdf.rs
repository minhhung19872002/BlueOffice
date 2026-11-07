//! PAdES baseline B-B PDF signatures (ETSI EN 319 142-1) written as an
//! incremental update (ISO 32000-1 §7.5.6) so earlier revisions — and
//! earlier signatures — stay byte-identical.
//!
//! Supported input: PDFs with classic cross-reference tables (ISO 32000-1
//! §7.5.4), which is what `bo-pdf` produces. Files that use cross-reference
//! streams are rejected with [`SignError::Unsupported`].

use crate::cms_sig::{sign_detached, verify_detached, VerifiedSigner};
use crate::{Result, SignError, Signer};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

/// Visible metadata of a signature (ISO 32000-1 §12.8.1, Table 252).
#[derive(Debug, Clone)]
pub struct SignOptions {
    /// `/Name`: signer name shown by viewers.
    pub name: Option<String>,
    pub reason: Option<String>,
    pub location: Option<String>,
    /// `/M`: signing time.
    pub time: SystemTime,
    /// Hex digits reserved for `/Contents` (CMS DER is half of that).
    pub reserve_hex: usize,
}

impl Default for SignOptions {
    fn default() -> Self {
        Self {
            name: None,
            reason: None,
            location: None,
            time: SystemTime::now(),
            reserve_hex: 32_768,
        }
    }
}

// ---------------------------------------------------------------- parsing

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | b'\0')
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn skip_ws(d: &[u8], mut i: usize) -> usize {
    while i < d.len() {
        if is_ws(d[i]) {
            i += 1;
        } else if d[i] == b'%' {
            while i < d.len() && d[i] != b'\n' && d[i] != b'\r' {
                i += 1;
            }
        } else {
            break;
        }
    }
    i
}

fn malformed(what: &str) -> SignError {
    SignError::Malformed(format!("PDF: {what}"))
}

/// Returns the end offset (exclusive) of the PDF object starting at `i`
/// (ISO 32000-1 §7.3). Indirect references `n g R` count as one value.
fn value_end(d: &[u8], i: usize, depth: usize) -> Result<usize> {
    if depth > 64 {
        return Err(malformed("nesting too deep"));
    }
    let i = skip_ws(d, i);
    let b = *d.get(i).ok_or_else(|| malformed("unexpected end"))?;
    match b {
        b'<' if d.get(i + 1) == Some(&b'<') => {
            let mut j = i + 2;
            loop {
                j = skip_ws(d, j);
                if d.get(j..j + 2) == Some(b">>") {
                    return Ok(j + 2);
                }
                j = value_end(d, j, depth + 1)?;
            }
        }
        b'<' => d[i..]
            .iter()
            .position(|&c| c == b'>')
            .map(|p| i + p + 1)
            .ok_or_else(|| malformed("unterminated hex string")),
        b'[' => {
            let mut j = i + 1;
            loop {
                j = skip_ws(d, j);
                if d.get(j) == Some(&b']') {
                    return Ok(j + 1);
                }
                j = value_end(d, j, depth + 1)?;
            }
        }
        b'(' => {
            let mut level = 0usize;
            let mut j = i;
            while j < d.len() {
                match d[j] {
                    b'\\' => j += 1,
                    b'(' => level += 1,
                    b')' => {
                        level -= 1;
                        if level == 0 {
                            return Ok(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            Err(malformed("unterminated string"))
        }
        b'/' => {
            let mut j = i + 1;
            while j < d.len() && !is_ws(d[j]) && !is_delim(d[j]) {
                j += 1;
            }
            Ok(j)
        }
        _ => {
            let token_end = |k: usize| {
                let mut k = k;
                while k < d.len() && !is_ws(d[k]) && !is_delim(d[k]) {
                    k += 1;
                }
                k
            };
            let e1 = token_end(i);
            if e1 == i {
                return Err(malformed("unexpected delimiter"));
            }
            // `n g R` reference?
            let s2 = skip_ws(d, e1);
            let e2 = token_end(s2);
            let s3 = skip_ws(d, e2);
            if d[i..e1].iter().all(u8::is_ascii_digit)
                && e2 > s2
                && d[s2..e2].iter().all(u8::is_ascii_digit)
                && d.get(s3) == Some(&b'R')
                && d.get(s3 + 1).is_none_or(|&c| is_ws(c) || is_delim(c))
            {
                return Ok(s3 + 1);
            }
            Ok(e1)
        }
    }
}

/// Top-level entries of a dictionary `<<…>>` as `(key, value text)`.
fn dict_entries(d: &[u8]) -> Result<Vec<(String, String)>> {
    let i = skip_ws(d, 0);
    if d.get(i..i + 2) != Some(b"<<") {
        return Err(malformed("expected dictionary"));
    }
    let mut j = i + 2;
    let mut out = Vec::new();
    loop {
        j = skip_ws(d, j);
        if d.get(j..j + 2) == Some(b">>") {
            return Ok(out);
        }
        if d.get(j) != Some(&b'/') {
            return Err(malformed("expected name key"));
        }
        let ke = value_end(d, j, 1)?;
        let vs = skip_ws(d, ke);
        let ve = value_end(d, vs, 1)?;
        out.push((
            String::from_utf8_lossy(&d[j + 1..ke]).into_owned(),
            String::from_utf8_lossy(&d[vs..ve]).into_owned(),
        ));
        j = ve;
    }
}

fn dict_get<'a>(entries: &'a [(String, String)], key: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn dict_to_string(entries: &[(String, String)]) -> String {
    let mut s = String::from("<<");
    for (k, v) in entries {
        let _ = write!(s, " /{k} {v}");
    }
    s.push_str(" >>");
    s
}

fn dict_set(entries: &mut Vec<(String, String)>, key: &str, value: String) {
    if let Some(e) = entries.iter_mut().find(|(k, _)| k == key) {
        e.1 = value;
    } else {
        entries.push((key.to_owned(), value));
    }
}

fn parse_ref(v: &str) -> Option<(u32, u16)> {
    let mut it = v.split_ascii_whitespace();
    let n = it.next()?.parse().ok()?;
    let g = it.next()?.parse().ok()?;
    (it.next()? == "R" && it.next().is_none()).then_some((n, g))
}

fn find_last(d: &[u8], pat: &[u8]) -> Option<usize> {
    d.windows(pat.len()).rposition(|w| w == pat)
}

struct PdfFile<'a> {
    data: &'a [u8],
    /// object number → (offset, generation), latest revision wins.
    offsets: BTreeMap<u32, (usize, u16)>,
    trailer: Vec<(String, String)>,
    startxref: usize,
}

impl<'a> PdfFile<'a> {
    fn parse(data: &'a [u8]) -> Result<Self> {
        let sx = find_last(data, b"startxref").ok_or_else(|| malformed("no startxref"))?;
        let s = skip_ws(data, sx + 9);
        let e = value_end(data, s, 0)?;
        let startxref: usize = std::str::from_utf8(&data[s..e])
            .ok()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| malformed("bad startxref"))?;
        let mut offsets = BTreeMap::new();
        let mut trailer = None;
        let mut next = Some(startxref);
        let mut guard = 0;
        while let Some(pos) = next {
            guard += 1;
            if guard > 256 {
                return Err(malformed("xref /Prev loop"));
            }
            let (t, section) = Self::xref_section(data, pos)?;
            for (n, v) in section {
                offsets.entry(n).or_insert(v);
            }
            next = dict_get(&t, "Prev").and_then(|p| p.parse().ok());
            if trailer.is_none() {
                trailer = Some(t);
            }
        }
        Ok(Self {
            data,
            offsets,
            trailer: trailer.ok_or_else(|| malformed("no trailer"))?,
            startxref,
        })
    }

    /// Parses one classic xref section + its trailer dictionary.
    #[allow(clippy::type_complexity)]
    fn xref_section(
        d: &[u8],
        pos: usize,
    ) -> Result<(Vec<(String, String)>, Vec<(u32, (usize, u16))>)> {
        let mut i = skip_ws(d, pos);
        if d.get(i..i + 4) != Some(b"xref") {
            return Err(SignError::Unsupported(
                "cross-reference streams (ISO 32000-1 §7.5.8) are not supported".into(),
            ));
        }
        i += 4;
        let mut entries = Vec::new();
        let read_int = |i: usize| -> Result<(usize, usize)> {
            let s = skip_ws(d, i);
            let mut e = s;
            while e < d.len() && d[e].is_ascii_digit() {
                e += 1;
            }
            let v = std::str::from_utf8(&d[s..e])
                .ok()
                .and_then(|t| t.parse().ok())
                .ok_or_else(|| malformed("bad xref number"))?;
            Ok((v, e))
        };
        loop {
            i = skip_ws(d, i);
            if d.get(i..i + 7) == Some(b"trailer") {
                let t = dict_entries(&d[i + 7..])?;
                return Ok((t, entries));
            }
            let (first, e) = read_int(i)?;
            let (count, e) = read_int(e)?;
            i = e;
            for k in 0..count {
                let (off, e) = read_int(i)?;
                let (gen, e) = read_int(e)?;
                let s = skip_ws(d, e);
                let kind = *d.get(s).ok_or_else(|| malformed("xref entry"))?;
                i = s + 1;
                if kind == b'n' {
                    let n = u32::try_from(first + k).map_err(|_| malformed("object number"))?;
                    let g = u16::try_from(gen).map_err(|_| malformed("generation"))?;
                    entries.push((n, (off, g)));
                }
            }
        }
    }

    /// Text of the object body (`n g obj <body> endobj`).
    fn object(&self, num: u32) -> Result<&'a [u8]> {
        let &(off, _) = self
            .offsets
            .get(&num)
            .ok_or_else(|| malformed("missing object"))?;
        let d = self.data;
        let head = d
            .get(off..)
            .and_then(|r| r.windows(3).position(|w| w == b"obj"))
            .ok_or_else(|| malformed("object header"))?;
        let s = skip_ws(d, off + head + 3);
        let e = value_end(d, s, 0)?;
        Ok(&d[s..e])
    }

    fn object_dict(&self, num: u32) -> Result<Vec<(String, String)>> {
        dict_entries(self.object(num)?)
    }

    fn size(&self) -> Result<u32> {
        dict_get(&self.trailer, "Size")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| malformed("trailer /Size"))
    }

    fn first_page(&self, pages: u32) -> Result<u32> {
        let mut node = pages;
        for _ in 0..64 {
            let dict = self.object_dict(node)?;
            if dict_get(&dict, "Type") == Some("/Page") {
                return Ok(node);
            }
            let kids = dict_get(&dict, "Kids").ok_or_else(|| malformed("/Kids"))?;
            let inner = kids.trim().trim_start_matches('[').trim_end_matches(']');
            let first = value_end(inner.as_bytes(), 0, 0)?;
            node = parse_ref(inner[..first].trim())
                .ok_or_else(|| malformed("page ref"))?
                .0;
        }
        Err(malformed("page tree too deep"))
    }
}

// ---------------------------------------------------------------- writing

/// PDF text string (ISO 32000-1 §7.9.2.2): PDFDocEncoding-safe ASCII
/// literal, otherwise UTF-16BE with BOM as a hex string.
fn text_string(s: &str) -> String {
    if s.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        let mut out = String::from("(");
        for c in s.chars() {
            if matches!(c, '(' | ')' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push(')');
        out
    } else {
        let mut out = String::from("<FEFF");
        for u in s.encode_utf16() {
            let _ = write!(out, "{u:04X}");
        }
        out.push('>');
        out
    }
}

/// UTC civil time `(year, month, day, hour, minute, second)` of a Unix time
/// (H. Hinnant's days-to-civil algorithm, public domain).
pub(crate) fn civil_from_unix(secs: u64) -> (i64, i64, i64, u64, u64, u64) {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day, rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// `D:YYYYMMDDHHmmSSZ` (ISO 32000-1 §7.9.4), UTC.
fn pdf_date(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("D:{y:04}{mo:02}{d:02}{h:02}{mi:02}{s:02}Z")
}

const BYTE_RANGE_PLACEHOLDER: &str = "[0 0000000000 0000000000 0000000000]";

/// Adds a PAdES B-B signature (invisible signature field on page 1).
pub fn sign_pdf(pdf: &[u8], signer: &dyn Signer, opts: &SignOptions) -> Result<Vec<u8>> {
    let file = PdfFile::parse(pdf)?;
    let root = dict_get(&file.trailer, "Root")
        .and_then(parse_ref)
        .ok_or_else(|| malformed("trailer /Root"))?
        .0;
    let mut catalog = file.object_dict(root)?;
    let pages = dict_get(&catalog, "Pages")
        .and_then(parse_ref)
        .ok_or_else(|| malformed("catalog /Pages"))?
        .0;
    let page = file.first_page(pages)?;
    let mut page_dict = file.object_dict(page)?;

    let size = file.size()?;
    let sig_num = size;
    let widget_num = size + 1;
    let mut next_num = size + 2;
    let mut new_objects: Vec<(u32, String)> = Vec::new();

    // Signature field + widget (ISO 32000-1 §12.7.4.5, §12.5.6.19); F 132 = Print|Locked.
    let field_count = count_sig_fields(&file, &catalog);
    let widget = format!(
        "<< /Type /Annot /Subtype /Widget /FT /Sig /F 132 /Rect [0 0 0 0] /T {} /V {sig_num} 0 R /P {page} 0 R >>",
        text_string(&format!("Signature{}", field_count + 1))
    );

    // Page /Annots += widget.
    let annots = match dict_get(&page_dict, "Annots") {
        None => String::new(),
        Some(v) => match parse_ref(v) {
            Some((n, _)) => String::from_utf8_lossy(file.object(n)?).into_owned(),
            None => v.to_owned(),
        },
    };
    let inner = annots.trim().trim_start_matches('[').trim_end_matches(']');
    dict_set(
        &mut page_dict,
        "Annots",
        format!("[{inner} {widget_num} 0 R]"),
    );

    // Catalog /AcroForm /Fields += widget, /SigFlags 3 (§12.7.2, Table 219).
    let (acro_num, mut acro) = match dict_get(&catalog, "AcroForm") {
        None => (None, Vec::new()),
        Some(v) => match parse_ref(v) {
            Some((n, _)) => (Some(n), file.object_dict(n)?),
            None => (None, dict_entries(v.as_bytes())?),
        },
    };
    let fields = match dict_get(&acro, "Fields") {
        None => String::new(),
        Some(v) => match parse_ref(v) {
            Some((n, _)) => String::from_utf8_lossy(file.object(n)?).into_owned(),
            None => v.to_owned(),
        },
    };
    let inner = fields.trim().trim_start_matches('[').trim_end_matches(']');
    dict_set(&mut acro, "Fields", format!("[{inner} {widget_num} 0 R]"));
    dict_set(&mut acro, "SigFlags", "3".into());
    if let Some(n) = acro_num {
        new_objects.push((n, dict_to_string(&acro)));
    } else {
        let n = next_num;
        next_num += 1;
        new_objects.push((n, dict_to_string(&acro)));
        dict_set(&mut catalog, "AcroForm", format!("{n} 0 R"));
    }
    new_objects.push((root, dict_to_string(&catalog)));
    new_objects.push((page, dict_to_string(&page_dict)));
    new_objects.push((widget_num, widget));

    // Signature dictionary (§12.8.1, Table 252; PAdES: SubFilter ETSI.CAdES.detached).
    let mut sig = format!(
        "<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /ETSI.CAdES.detached /ByteRange {BYTE_RANGE_PLACEHOLDER} /M {}",
        text_string(&pdf_date(opts.time))
    );
    for (k, v) in [
        ("Name", &opts.name),
        ("Reason", &opts.reason),
        ("Location", &opts.location),
    ] {
        if let Some(v) = v {
            let _ = write!(sig, " /{k} {}", text_string(v));
        }
    }
    sig.push_str(" /Contents <");
    let contents_rel = sig.len() - 1;
    sig.push_str(&"0".repeat(opts.reserve_hex));
    sig.push_str("> >>");
    new_objects.push((sig_num, sig));

    // Assemble the incremental update.
    let mut out = pdf.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    new_objects.sort_by_key(|(n, _)| *n);
    let mut xref_offsets = Vec::new();
    let mut contents_start = 0;
    let mut byte_range_pos = 0;
    for (n, body) in &new_objects {
        let off = out.len();
        let gen = file.offsets.get(n).map_or(0, |&(_, g)| g);
        let header = format!("{n} {gen} obj\n");
        if *n == sig_num {
            contents_start = off + header.len() + contents_rel;
            byte_range_pos = off
                + header.len()
                + body
                    .find(BYTE_RANGE_PLACEHOLDER)
                    .ok_or_else(|| malformed("internal: byte range"))?;
        }
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(body.as_bytes());
        out.extend_from_slice(b"\nendobj\n");
        xref_offsets.push((*n, gen, off));
    }
    let xref_pos = out.len();
    let mut xref = String::from("xref\n");
    for (n, gen, off) in &xref_offsets {
        let _ = write!(xref, "{n} 1\n{off:010} {gen:05} n\r\n");
    }
    let mut trailer = vec![
        ("Size".to_owned(), next_num.max(size).to_string()),
        ("Root".to_owned(), format!("{root} 0 R")),
        ("Prev".to_owned(), file.startxref.to_string()),
    ];
    for k in ["Info", "ID"] {
        if let Some(v) = dict_get(&file.trailer, k) {
            trailer.push((k.to_owned(), v.to_owned()));
        }
    }
    let _ = write!(
        xref,
        "trailer\n{}\nstartxref\n{xref_pos}\n%%EOF\n",
        dict_to_string(&trailer)
    );
    out.extend_from_slice(xref.as_bytes());

    // Fill /ByteRange, hash everything except /Contents <…>, sign, embed.
    let contents_end = contents_start + opts.reserve_hex + 2;
    let byte_range = format!(
        "[0 {contents_start:010} {contents_end:010} {:010}]",
        out.len() - contents_end
    );
    out[byte_range_pos..byte_range_pos + byte_range.len()].copy_from_slice(byte_range.as_bytes());
    let mut signed = out[..contents_start].to_vec();
    signed.extend_from_slice(&out[contents_end..]);
    let cms = sign_detached(&signed, signer)?;
    let hex: String = cms.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02X}");
        s
    });
    if hex.len() > opts.reserve_hex {
        return Err(SignError::Unsupported(format!(
            "signature needs {} hex digits, only {} reserved",
            hex.len(),
            opts.reserve_hex
        )));
    }
    out[contents_start + 1..contents_start + 1 + hex.len()].copy_from_slice(hex.as_bytes());
    Ok(out)
}

fn count_sig_fields(file: &PdfFile<'_>, catalog: &[(String, String)]) -> usize {
    let Some(acro) = dict_get(catalog, "AcroForm") else {
        return 0;
    };
    let text = match parse_ref(acro) {
        Some((n, _)) => file
            .object(n)
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default(),
        None => acro.to_owned(),
    };
    text.matches(" R").count()
}

/// One verified signature of a PDF.
#[derive(Debug, Clone)]
pub struct PdfSignature {
    pub signer: VerifiedSigner,
    /// The signature covers the whole file (no later changes).
    pub covers_whole_file: bool,
}

/// Verifies every `/ByteRange` signature in `pdf` (digest + RSA; trust
/// chain validation against Vietnamese root CAs is the caller's job).
pub fn verify_pdf(pdf: &[u8]) -> Result<Vec<PdfSignature>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = pdf[from..].windows(10).position(|w| w == b"/ByteRange") {
        let at = from + p + 10;
        from = at;
        let end = value_end(pdf, at, 0)?;
        let arr = String::from_utf8_lossy(&pdf[at..end]);
        let nums: Vec<usize> = arr
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split_ascii_whitespace()
            .filter_map(|n| n.parse().ok())
            .collect();
        let [a, b, c, len] = nums[..] else {
            return Err(malformed("/ByteRange"));
        };
        if a != 0 || b > c || c.checked_add(len).is_none_or(|e| e > pdf.len()) {
            return Err(malformed("/ByteRange out of bounds"));
        }
        let hex = &pdf[b..c];
        let hex = hex
            .strip_prefix(b"<")
            .and_then(|h| h.strip_suffix(b">"))
            .ok_or_else(|| malformed("/Contents"))?;
        let mut der = Vec::with_capacity(hex.len() / 2);
        for pair in hex.chunks(2) {
            let s = std::str::from_utf8(pair).map_err(|_| malformed("hex"))?;
            der.push(u8::from_str_radix(s, 16).map_err(|_| malformed("hex"))?);
        }
        // Trailing zero padding is outside the DER value; trim via DER length.
        let der_len = der_total_len(&der).ok_or_else(|| malformed("CMS length"))?;
        der.truncate(der_len);
        let mut signed = pdf[..b].to_vec();
        signed.extend_from_slice(&pdf[c..c + len]);
        out.push(PdfSignature {
            signer: verify_detached(&der, &signed)?,
            covers_whole_file: c + len == pdf.len(),
        });
    }
    Ok(out)
}

/// Total length of a DER TLV starting at `d[0]`.
fn der_total_len(d: &[u8]) -> Option<usize> {
    let l0 = *d.get(1)?;
    if l0 < 0x80 {
        return Some(2 + usize::from(l0));
    }
    let n = usize::from(l0 & 0x7f);
    if n == 0 || n > 4 {
        return None;
    }
    let mut len = 0usize;
    for k in 0..n {
        len = (len << 8) | usize::from(*d.get(2 + k)?);
    }
    Some(2 + n + len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_values_and_dicts() {
        let d = b"<< /Type /Catalog /Pages 2 0 R /Names << /A [1 (x\\)y) <00>] >> /N 5 >>";
        let e = dict_entries(d).unwrap();
        assert_eq!(dict_get(&e, "Pages"), Some("2 0 R"));
        assert_eq!(dict_get(&e, "Names"), Some("<< /A [1 (x\\)y) <00>] >>"));
        assert_eq!(dict_get(&e, "N"), Some("5"));
        assert_eq!(parse_ref("12 0 R"), Some((12, 0)));
        assert_eq!(parse_ref("12 0"), None);
    }

    #[test]
    fn formats_strings_and_dates() {
        assert_eq!(text_string("a(b)"), "(a\\(b\\))");
        assert_eq!(text_string("Hà"), "<FEFF004800E0>");
        let t = UNIX_EPOCH + std::time::Duration::from_secs(1_793_000_000);
        assert_eq!(pdf_date(t), "D:20261026073320Z");
        assert_eq!(pdf_date(UNIX_EPOCH), "D:19700101000000Z");
    }

    #[test]
    fn rejects_garbage_without_panic() {
        for input in [
            &b""[..],
            b"%PDF-1.7\nstartxref\n999999\n%%EOF",
            b"startxref",
        ] {
            let s = crate::testing::NullSigner;
            assert!(sign_pdf(input, &s, &SignOptions::default()).is_err());
            assert!(verify_pdf(input).map_or(true, |v| v.is_empty()));
        }
    }
}
