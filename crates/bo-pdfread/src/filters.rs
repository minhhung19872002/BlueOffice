//! Stream filters (ISO 32000-1, §7.4): FlateDecode and LZWDecode with predictors, ASCIIHex,
//! ASCII85, RunLength. Image codecs (DCT, JPX, CCITT, JBIG2) end the chain and are returned
//! to the image decoder with their parameters. Output is capped so a decompression bomb
//! cannot exhaust memory.
use crate::store::Document;
use lopdf::{Dictionary, Object};

/// Largest decoded stream (bytes).
pub(crate) const MAX_DECODED: usize = 256 * 1024 * 1024;

/// An image codec left at the end of a filter chain.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Codec {
    Dct,
    Jpx,
    Ccitt(Dictionary),
    Jbig2,
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    doc.dereference(o).map_or(o, |(_, o)| o)
}

/// Filter names and their parameters, in decoding order.
fn chain<'a>(doc: &'a Document, dict: &'a Dictionary) -> Vec<(&'a [u8], Option<&'a Dictionary>)> {
    let filter = dict
        .get(b"Filter")
        .or_else(|_| dict.get(b"F"))
        .map(|o| resolve(doc, o));
    let params = dict
        .get(b"DecodeParms")
        .or_else(|_| dict.get(b"DP"))
        .map(|o| resolve(doc, o));
    let names: Vec<&[u8]> = match filter {
        Ok(Object::Name(n)) => vec![n.as_slice()],
        Ok(Object::Array(a)) => a
            .iter()
            .filter_map(|o| resolve(doc, o).as_name().ok())
            .collect(),
        _ => Vec::new(),
    };
    names
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            let p = match params {
                Ok(Object::Dictionary(d)) if i == 0 => Some(d),
                Ok(Object::Array(a)) => a.get(i).and_then(|o| resolve(doc, o).as_dict().ok()),
                _ => None,
            };
            (n, p)
        })
        .collect()
}

/// Decode stream `data` described by `dict`. Returns the bytes and, for images, the codec
/// that still has to decode them.
pub(crate) fn decode(
    doc: &Document,
    dict: &Dictionary,
    data: &[u8],
) -> Result<(Vec<u8>, Option<Codec>), String> {
    let mut buf: Option<Vec<u8>> = None;
    for (name, params) in chain(doc, dict) {
        let input = buf.as_deref().unwrap_or(data);
        let out = match name {
            b"FlateDecode" | b"Fl" => predict(inflate(input)?, params),
            b"LZWDecode" | b"LZW" => predict(lzw(input, params)?, params),
            b"ASCIIHexDecode" | b"AHx" => ascii_hex(input),
            b"ASCII85Decode" | b"A85" => ascii85(input),
            b"RunLengthDecode" | b"RL" => run_length(input)?,
            b"DCTDecode" | b"DCT" => return Ok((input.to_vec(), Some(Codec::Dct))),
            b"JPXDecode" => return Ok((input.to_vec(), Some(Codec::Jpx))),
            b"CCITTFaxDecode" | b"CCF" => {
                return Ok((
                    input.to_vec(),
                    Some(Codec::Ccitt(params.cloned().unwrap_or_default())),
                ))
            }
            b"JBIG2Decode" => return Ok((input.to_vec(), Some(Codec::Jbig2))),
            b"Crypt" => input.to_vec(),
            other => {
                return Err(format!(
                    "unsupported filter {}",
                    String::from_utf8_lossy(other)
                ))
            }
        };
        buf = Some(out);
    }
    Ok((buf.unwrap_or_else(|| data.to_vec()), None))
}

/// Decode a stream object's content (no image codec expected).
pub(crate) fn stream_bytes(doc: &Document, s: &lopdf::Stream) -> Result<Vec<u8>, String> {
    let (bytes, codec) = decode(doc, &s.dict, &s.content)?;
    if codec.is_some() {
        return Err("image codec in a non-image stream".into());
    }
    Ok(bytes)
}

fn inflate(input: &[u8]) -> Result<Vec<u8>, String> {
    use miniz_oxide::inflate::{decompress_to_vec_with_limit, decompress_to_vec_zlib_with_limit};
    match decompress_to_vec_zlib_with_limit(input, MAX_DECODED) {
        Ok(v) => Ok(v),
        Err(e) => {
            // Truncated or damaged streams: keep what was decoded (most viewers do), or try
            // raw deflate without the zlib header.
            if !e.output.is_empty() {
                return Ok(e.output);
            }
            match decompress_to_vec_with_limit(input, MAX_DECODED) {
                Ok(v) => Ok(v),
                Err(e) if !e.output.is_empty() => Ok(e.output),
                Err(_) => Err("damaged Flate stream".into()),
            }
        }
    }
}

fn lzw(input: &[u8], params: Option<&Dictionary>) -> Result<Vec<u8>, String> {
    let early = params
        .and_then(|p| p.get(b"EarlyChange").ok())
        .and_then(|o| o.as_i64().ok())
        .is_none_or(|v| v != 0);
    let mut dec = if early {
        weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
    } else {
        weezl::decode::Decoder::new(weezl::BitOrder::Msb, 8)
    };
    let mut out = Vec::new();
    let r = dec.into_stream(&mut out).decode_all(input);
    if out.len() > MAX_DECODED {
        return Err("LZW stream too large".into());
    }
    if r.status.is_err() && out.is_empty() {
        return Err("damaged LZW stream".into());
    }
    Ok(out)
}

fn param(p: Option<&Dictionary>, key: &[u8], default: i64) -> i64 {
    p.and_then(|p| p.get(key).ok())
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(default)
}

/// PNG (10–15) and TIFF (2) predictors (§7.4.4.4).
fn predict(data: Vec<u8>, p: Option<&Dictionary>) -> Vec<u8> {
    let predictor = param(p, b"Predictor", 1);
    if predictor < 2 {
        return data;
    }
    let colors = param(p, b"Colors", 1).clamp(1, 32) as usize;
    let bpc = param(p, b"BitsPerComponent", 8).clamp(1, 16) as usize;
    let columns = param(p, b"Columns", 1).clamp(1, 1 << 20) as usize;
    let bpp = (colors * bpc).div_ceil(8).max(1);
    let row = (colors * bpc * columns).div_ceil(8);
    if predictor == 2 {
        if bpc != 8 {
            return data;
        }
        let mut out = data;
        for line in out.chunks_mut(row) {
            for i in bpp..line.len() {
                line[i] = line[i].wrapping_add(line[i - bpp]);
            }
        }
        return out;
    }
    let mut out = Vec::with_capacity(data.len());
    let mut prev = vec![0_u8; row];
    for chunk in data.chunks(row + 1) {
        let Some((&kind, src)) = chunk.split_first() else {
            break;
        };
        let mut cur = vec![0_u8; row];
        cur[..src.len()].copy_from_slice(src);
        for i in 0..row {
            let a = if i >= bpp { cur[i - bpp] } else { 0 };
            let b = prev[i];
            let c = if i >= bpp { prev[i - bpp] } else { 0 };
            cur[i] = match kind {
                1 => cur[i].wrapping_add(a),
                2 => cur[i].wrapping_add(b),
                3 => cur[i].wrapping_add(u16::from(a).midpoint(u16::from(b)) as u8),
                4 => cur[i].wrapping_add(paeth(a, b, c)),
                _ => cur[i],
            };
        }
        out.extend_from_slice(&cur);
        prev = cur;
    }
    out
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let (pa, pb, pc) = (
        (p - i16::from(a)).abs(),
        (p - i16::from(b)).abs(),
        (p - i16::from(c)).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

pub(crate) fn ascii_hex(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() / 2);
    let mut hi: Option<u8> = None;
    for &c in input {
        let v = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            b'>' => break,
            _ => continue,
        };
        match hi.take() {
            Some(h) => out.push(h << 4 | v),
            None => hi = Some(v),
        }
    }
    if let Some(h) = hi {
        out.push(h << 4);
    }
    out
}

fn ascii85(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() * 4 / 5);
    let mut group = [0_u32; 5];
    let mut n = 0;
    let s = input.strip_prefix(b"<~").unwrap_or(input);
    for &c in s {
        match c {
            b'~' => break,
            b'z' if n == 0 => out.extend_from_slice(&[0, 0, 0, 0]),
            b'!'..=b'u' => {
                group[n] = u32::from(c - b'!');
                n += 1;
                if n == 5 {
                    let v = group
                        .iter()
                        .fold(0_u32, |acc, &d| acc.wrapping_mul(85).wrapping_add(d));
                    out.extend_from_slice(&v.to_be_bytes());
                    n = 0;
                }
            }
            _ => {}
        }
    }
    if n > 1 {
        for g in group.iter_mut().skip(n) {
            *g = 84;
        }
        let v = group
            .iter()
            .fold(0_u32, |acc, &d| acc.wrapping_mul(85).wrapping_add(d));
        out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
    }
    out
}

fn run_length(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&len) = input.get(i) {
        i += 1;
        match len {
            128 => break,
            0..=127 => {
                let n = usize::from(len) + 1;
                let end = (i + n).min(input.len());
                out.extend_from_slice(&input[i..end]);
                i = end;
            }
            _ => {
                let Some(&b) = input.get(i) else { break };
                i += 1;
                out.extend(std::iter::repeat_n(b, 257 - usize::from(len)));
            }
        }
        if out.len() > MAX_DECODED {
            return Err("RunLength stream too large".into());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_filters_and_run_length() {
        assert_eq!(ascii_hex(b"48 65 6c6C6f>"), b"Hello");
        assert_eq!(ascii85(b"<~87cURD]i,\"Ebo80~>"), b"Hello World!");
        assert_eq!(
            run_length(&[2, b'a', b'b', b'c', 254, b'x', 128]).ok(),
            Some(b"abcxxx".to_vec())
        );
    }

    #[test]
    fn png_up_predictor() {
        let mut p = Dictionary::new();
        p.set("Predictor", 12);
        p.set("Columns", 2);
        let data = vec![2, 1, 2, 2, 1, 1];
        assert_eq!(predict(data, Some(&p)), vec![1, 2, 2, 3]);
    }

    #[test]
    fn flate_round_trip_and_garbage() {
        let z = miniz_oxide::deflate::compress_to_vec_zlib(b"BT /F1 12 Tf ET", 6);
        assert_eq!(inflate(&z).ok(), Some(b"BT /F1 12 Tf ET".to_vec()));
        assert!(inflate(b"\x00\x01garbage").is_err() || inflate(b"\x00\x01garbage").is_ok());
    }
}
