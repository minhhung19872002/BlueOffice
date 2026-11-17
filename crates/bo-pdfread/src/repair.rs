//! Rebuilding a damaged cross-reference table (BlueReader round 14): files whose `startxref`
//! or xref offsets are wrong (bytes inserted or removed by a mail gateway, a truncated
//! download, a bad editor) still open, as in other readers.
//!
//! The objects are found by scanning for `N G obj` at the start of a line (ISO 32000-1 §7.3.10
//! indirect objects; §7.5.4 cross-reference table; §7.5.5 trailer); a later definition of the
//! same object wins, as with incremental updates (§7.5.6). A classic cross-reference table and
//! trailer pointing to them are appended, keeping the trailer's `/Root`, `/Info`, `/Encrypt`
//! and `/ID` (from the last `trailer` dictionary, or else from a cross-reference stream
//! dictionary, §7.5.8), or the object typed `/Catalog` when no trailer names one.

/// The file with a rebuilt cross-reference table, or `None` when no objects or no catalog were
/// found.
pub(crate) fn rebuild(bytes: &[u8]) -> Option<Vec<u8>> {
    let objects = scan_objects(bytes);
    if objects.is_empty() {
        return None;
    }
    let max = objects.iter().map(|o| o.0).max()?;
    let mut offsets: Vec<Option<(usize, u16)>> = vec![None; max as usize + 1];
    for &(num, gen, at) in &objects {
        offsets[num as usize] = Some((at, gen));
    }
    let keys = trailer_keys(bytes, &objects).or_else(|| catalog_root(bytes, &objects))?;
    let mut out = bytes.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f\r\n", max + 1).as_bytes());
    for o in offsets.iter().skip(1) {
        let line = match o {
            Some((at, gen)) => format!("{at:010} {gen:05} n\r\n"),
            None => "0000000000 00000 f\r\n".to_owned(),
        };
        out.extend_from_slice(line.as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} {keys} >>\nstartxref\n{xref_at}\n%%EOF\n",
            max + 1
        )
        .as_bytes(),
    );
    Some(out)
}

/// `(number, generation, offset)` of every `N G obj` that starts a line.
fn scan_objects(b: &[u8]) -> Vec<(u32, u16, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = find(b, i, b"obj") {
        i = k + 3;
        // `obj` followed by a delimiter (not `objstm`, `object`).
        if b.get(k + 3).is_some_and(u8::is_ascii_alphanumeric) {
            continue;
        }
        let Some((num, gen, start)) = header_before(b, k) else {
            continue;
        };
        if num > 0 && num < 10_000_000 {
            out.push((num, gen, start));
        }
    }
    out
}

/// `N G ` just before `obj` at `k`, starting a line: (N, G, offset of N).
fn header_before(b: &[u8], k: usize) -> Option<(u32, u16, usize)> {
    let mut j = k;
    let skip_ws = |j: &mut usize| {
        while *j > 0 && matches!(b[*j - 1], b' ' | b'\t') {
            *j -= 1;
        }
    };
    let digits = |j: &mut usize| {
        let end = *j;
        while *j > 0 && b[*j - 1].is_ascii_digit() {
            *j -= 1;
        }
        (*j < end).then(|| std::str::from_utf8(&b[*j..end]).ok()?.parse::<u64>().ok())?
    };
    skip_ws(&mut j);
    if j == k {
        return None;
    }
    let gen = digits(&mut j)?;
    let before = j;
    skip_ws(&mut j);
    if j == before {
        return None;
    }
    let num = digits(&mut j)?;
    // At the start of the file or of a line (or after a delimiter ending the previous object).
    if j > 0 && !matches!(b[j - 1], b'\n' | b'\r' | b'>' | b']' | b' ') {
        return None;
    }
    Some((u32::try_from(num).ok()?, u16::try_from(gen).ok()?, j))
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    b.get(from..)?
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|p| p + from)
}

fn rfind(b: &[u8], pat: &[u8]) -> Option<usize> {
    b.windows(pat.len()).rposition(|w| w == pat)
}

/// The dictionary `<< … >>` starting at or after `from` (nested dictionaries kept).
fn dict_at(b: &[u8], from: usize) -> Option<&[u8]> {
    let s = find(b, from, b"<<")?;
    let mut depth = 0_i32;
    let mut i = s;
    while i + 1 < b.len() {
        match &b[i..i + 2] {
            b"<<" => {
                depth += 1;
                i += 2;
            }
            b">>" => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return Some(&b[s..i]);
                }
            }
            _ => i += 1,
        }
    }
    None
}

/// `/Root`, `/Info`, `/Encrypt`, `/ID` entries of the last trailer (or cross-reference stream
/// dictionary) that names a root object that exists.
fn trailer_keys(b: &[u8], objects: &[(u32, u16, usize)]) -> Option<String> {
    let mut dicts: Vec<&[u8]> = Vec::new();
    if let Some(t) = rfind(b, b"trailer") {
        dicts.extend(dict_at(b, t));
    }
    for &(_, _, at) in objects.iter().rev() {
        if let Some(d) = dict_at(b, at) {
            if has_name(d, b"/Type", b"/XRef") {
                dicts.push(d);
            }
        }
    }
    for d in dicts {
        let root = entry(d, b"/Root")?;
        let num: u32 = root.split_whitespace().next()?.parse().ok()?;
        if !objects.iter().any(|o| o.0 == num) {
            continue;
        }
        let mut keys = format!("/Root {root}");
        for k in [&b"/Info"[..], b"/Encrypt", b"/ID"] {
            if let Some(v) = entry(d, k) {
                keys.push_str(&format!(" {} {v}", String::from_utf8_lossy(k)));
            }
        }
        return Some(keys);
    }
    None
}

/// The last object whose dictionary says `/Type /Catalog`.
fn catalog_root(b: &[u8], objects: &[(u32, u16, usize)]) -> Option<String> {
    objects.iter().rev().find_map(|&(num, gen, at)| {
        let d = dict_at(b, at)?;
        // The dictionary must belong to this object (before its `endobj`).
        let end = find(b, at, b"endobj").unwrap_or(b.len());
        let ds = find(b, at, b"<<")?;
        (ds < end && has_name(d, b"/Type", b"/Catalog")).then(|| format!("/Root {num} {gen} R"))
    })
}

fn has_name(d: &[u8], key: &[u8], value: &[u8]) -> bool {
    entry(d, key).is_some_and(|v| v.as_bytes() == value)
}

/// Value of a top-level entry `key` (a reference `N G R`, a name, or an array `[…]`), as text.
fn entry(d: &[u8], key: &[u8]) -> Option<String> {
    // Top level only: skip nested dictionaries.
    let mut depth = 0_i32;
    let mut i = 0;
    while i < d.len() {
        if d[i..].starts_with(b"<<") {
            depth += 1;
            i += 2;
            continue;
        }
        if d[i..].starts_with(b">>") {
            depth -= 1;
            i += 2;
            continue;
        }
        if depth == 1
            && d[i..].starts_with(key)
            && !d.get(i + key.len()).is_some_and(u8::is_ascii_alphanumeric)
        {
            let rest = &d[i + key.len()..];
            let rest = &rest[rest.iter().take_while(|c| c.is_ascii_whitespace()).count()..];
            return value(rest);
        }
        i += 1;
    }
    None
}

fn value(r: &[u8]) -> Option<String> {
    let text = |s: &[u8]| String::from_utf8_lossy(s).trim().to_owned();
    match r.first()? {
        b'[' => {
            let e = r.iter().position(|&c| c == b']')?;
            Some(text(&r[..=e]))
        }
        b'/' => {
            let e = 1 + r[1..]
                .iter()
                .position(|c| c.is_ascii_whitespace() || b"/<>[]()".contains(c))
                .unwrap_or(r.len() - 1);
            Some(text(&r[..e]))
        }
        c if c.is_ascii_digit() => {
            // `N G R`
            let e = r.iter().position(|&c| c == b'R')?;
            let v = text(&r[..=e]);
            (v.split_whitespace().count() == 3).then_some(v)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>",
        ];
        let mut b = b"%PDF-1.4\n".to_vec();
        let mut offs = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offs.push(b.len());
            b.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let x = b.len();
        b.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
        for o in offs {
            b.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        b.extend_from_slice(
            format!("trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n").as_bytes(),
        );
        b
    }

    #[test]
    fn shifted_offsets_and_bad_startxref_are_rebuilt() {
        let good = sample();
        let mut bad = good.clone();
        // Junk after the header moves every object; startxref points nowhere.
        bad.splice(9..9, b"% junk ......\n".iter().copied());
        let k = rfind(&bad, b"startxref").expect("startxref");
        bad.truncate(k);
        bad.extend_from_slice(b"startxref\n999999\n%%EOF\n");
        assert!(lopdf::Document::load_mem(&bad).is_err());
        let fixed = rebuild(&bad).expect("rebuilt");
        let d = lopdf::Document::load_mem(&fixed).expect("loads");
        assert_eq!(d.get_pages().len(), 1);
    }

    #[test]
    fn a_missing_trailer_falls_back_to_the_catalog() {
        let mut b = sample();
        let t = find(
            &b, 0, b"
xref",
        )
        .expect("xref");
        b.truncate(t + 1);
        let fixed = rebuild(&b).expect("rebuilt");
        let d = lopdf::Document::load_mem(&fixed).expect("loads");
        assert_eq!(d.get_pages().len(), 1);
        assert!(rebuild(b"%PDF-1.4\nnothing here\n").is_none());
    }
}
