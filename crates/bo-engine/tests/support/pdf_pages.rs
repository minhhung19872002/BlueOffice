//! Minimal reader of the PDFs `bo-pdf` writes (tests only): the page tree's
//! pages in order, each with its MediaBox and the operators of its
//! (Flate-compressed) content stream. ISO 32000-1 §7.3 (objects), §7.7.3
//! (page tree), §7.8.2 (content streams).
#![allow(dead_code, clippy::unwrap_used)]

/// One page of a PDF.
#[derive(Debug, Clone)]
pub struct PdfPage {
    /// `[x0, y0, x1, y1]`.
    pub media_box: [f32; 4],
    /// Operators with their numeric operands, in order (`("cm", [a b c d e f])`).
    pub ops: Vec<(String, Vec<f32>)>,
}

impl PdfPage {
    /// Operands of every `op`.
    pub fn all(&self, op: &str) -> Vec<Vec<f32>> {
        self.ops
            .iter()
            .filter(|(o, _)| o == op)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Fill colours (`rg`) as 0–255 integers.
    pub fn fills(&self) -> Vec<[u8; 3]> {
        self.all("rg")
            .iter()
            .map(|v| {
                let c = |x: f32| (x * 255.0).round() as u8;
                [c(v[0]), c(v[1]), c(v[2])]
            })
            .collect()
    }
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Body of object `id` (between `id 0 obj` and `endobj`).
fn object(pdf: &[u8], id: u32) -> &[u8] {
    let head = format!("\n{id} 0 obj");
    let start = find(pdf, head.as_bytes(), 0).unwrap_or_else(|| panic!("object {id}"));
    let end = find(pdf, b"endobj", start).unwrap();
    &pdf[start + head.len()..end]
}

fn refs_after(text: &str, key: &str) -> Vec<u32> {
    let at = text.find(key).unwrap_or_else(|| panic!("{key} in {text}"));
    let rest = &text[at + key.len()..];
    let open = rest.find('[').unwrap();
    let close = rest.find(']').unwrap();
    let nums: Vec<u32> = rest[open + 1..close]
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    // `a 0 R b 0 R …`: every first of three tokens.
    nums.chunks(2).map(|c| c[0]).collect()
}

fn ops_of(content: &str) -> Vec<(String, Vec<f32>)> {
    let mut out = Vec::new();
    let mut nums = Vec::new();
    for tok in content.split_whitespace() {
        if let Ok(v) = tok.parse::<f32>() {
            nums.push(v);
        } else if tok.starts_with('/')
            || tok.starts_with('<')
            || tok.starts_with('(')
            || tok.starts_with('[')
        {
            // Names, strings and arrays are operands we do not need.
        } else {
            out.push((tok.to_owned(), std::mem::take(&mut nums)));
        }
    }
    out
}

/// Pages of `pdf` in page-tree order.
pub fn pages(pdf: &[u8]) -> Vec<PdfPage> {
    let s = String::from_utf8_lossy(pdf);
    let at = s.find("/Type /Pages").expect("page tree");
    let obj_start = s[..at].rfind(" 0 obj").unwrap();
    let id_start = s[..obj_start].rfind('\n').map_or(0, |p| p + 1);
    let tree_id: u32 = s[id_start..obj_start].trim().parse().unwrap();
    let tree = String::from_utf8_lossy(object(pdf, tree_id)).into_owned();
    refs_after(&tree, "/Kids")
        .into_iter()
        .map(|id| {
            let page = String::from_utf8_lossy(object(pdf, id)).into_owned();
            let mb = &page[page.find("/MediaBox").unwrap()..];
            let v: Vec<f32> = mb[mb.find('[').unwrap() + 1..mb.find(']').unwrap()]
                .split_whitespace()
                .map(|t| t.parse().unwrap())
                .collect();
            let c = &page[page.find("/Contents").unwrap() + 9..];
            let cid: u32 = c.split_whitespace().next().unwrap().parse().unwrap();
            let body = object(pdf, cid);
            let start = find(body, b"stream", 0).unwrap() + 6;
            let start = start + usize::from(body[start] == b'\r') + 1;
            let end = find(body, b"endstream", start).unwrap();
            let mut data = &body[start..end];
            while data.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
                data = &data[..data.len() - 1];
            }
            let raw = miniz_oxide::inflate::decompress_to_vec_zlib(data).expect("flate");
            PdfPage {
                media_box: [v[0], v[1], v[2], v[3]],
                ops: ops_of(&String::from_utf8_lossy(&raw)),
            }
        })
        .collect()
}
