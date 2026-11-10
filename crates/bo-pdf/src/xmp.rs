//! XMP metadata packet (ISO 16684-1; Adobe XMP Specification Part 1/2) with the
//! PDF/A identification schema (ISO 19005-1 §6.7.11, ISO 19005-2 §6.6.4).
//!
//! Only predefined schemas are used (Dublin Core, XMP basic, Adobe PDF, PDF/A
//! identification), so no extension-schema description is needed (ISO 19005-1
//! §6.7.8). Every value mirrors the corresponding document information entry
//! (ISO 19005-1 §6.7.3: Title ↔ dc:title, Author ↔ dc:creator, Creator ↔
//! xmp:CreatorTool, Producer ↔ pdf:Producer, CreationDate ↔ xmp:CreateDate,
//! ModDate ↔ xmp:ModifyDate).

use crate::options::{PdfA, PdfDate};

/// Values written to both the information dictionary and the XMP packet.
pub(crate) struct DocMeta<'a> {
    pub title: Option<&'a str>,
    pub author: Option<&'a str>,
    pub creator: Option<&'a str>,
    pub producer: &'a str,
    pub lang: Option<&'a str>,
    pub date: Option<PdfDate>,
    pub pdfa: Option<PdfA>,
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // XML 1.0 §2.2: control characters other than tab/CR/LF are not allowed.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => o.push(' '),
            c => o.push(c),
        }
    }
    o
}

/// The XMP packet bytes (UTF-8, written unfiltered: ISO 19005-1 §6.7.2).
pub(crate) fn packet(m: &DocMeta<'_>) -> Vec<u8> {
    let mut x = String::new();
    // XMP Part 1 §7.3 packet wrapper (the BOM marks UTF-8).
    x.push_str("<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    x.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n");
    x.push_str("<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");
    x.push_str(concat!(
        "<rdf:Description rdf:about=\"\"",
        " xmlns:dc=\"http://purl.org/dc/elements/1.1/\"",
        " xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"",
        " xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\"",
        " xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\n"
    ));
    x.push_str("<dc:format>application/pdf</dc:format>\n");
    if let Some(t) = m.title {
        x.push_str(&format!(
            "<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>\n",
            esc(t)
        ));
    }
    if let Some(a) = m.author {
        x.push_str(&format!(
            "<dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>\n",
            esc(a)
        ));
    }
    if let Some(l) = m.lang {
        x.push_str(&format!(
            "<dc:language><rdf:Bag><rdf:li>{}</rdf:li></rdf:Bag></dc:language>\n",
            esc(l)
        ));
    }
    if let Some(c) = m.creator {
        x.push_str(&format!("<xmp:CreatorTool>{}</xmp:CreatorTool>\n", esc(c)));
    }
    if let Some(d) = m.date {
        let d = d.iso();
        x.push_str(&format!(
            "<xmp:CreateDate>{d}</xmp:CreateDate>\n<xmp:ModifyDate>{d}</xmp:ModifyDate>\n<xmp:MetadataDate>{d}</xmp:MetadataDate>\n"
        ));
    }
    x.push_str(&format!(
        "<pdf:Producer>{}</pdf:Producer>\n",
        esc(m.producer)
    ));
    if let Some(p) = m.pdfa {
        x.push_str(&format!(
            "<pdfaid:part>{}</pdfaid:part>\n<pdfaid:conformance>B</pdfaid:conformance>\n",
            p.part()
        ));
    }
    x.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n");
    // Padding lets editors update the packet in place (XMP Part 1 §7.3.3).
    for _ in 0..20 {
        x.push_str(&" ".repeat(99));
        x.push('\n');
    }
    x.push_str("<?xpacket end=\"w\"?>");
    x.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identification_and_escaping() {
        let p = packet(&DocMeta {
            title: Some("A & <B>"),
            author: Some("Lan"),
            creator: Some("BlueDoc"),
            producer: "BlueOffice",
            lang: Some("vi-VN"),
            date: None,
            pdfa: Some(PdfA::A2b),
        });
        let s = String::from_utf8(p).unwrap();
        assert!(s.contains("<pdfaid:part>2</pdfaid:part>"));
        assert!(s.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
        assert!(s.contains("<rdf:li xml:lang=\"x-default\">A &amp; &lt;B&gt;</rdf:li>"));
        assert!(s.contains("<rdf:Seq><rdf:li>Lan</rdf:li></rdf:Seq>"));
        assert!(s.starts_with("<?xpacket begin=\"\u{FEFF}\""));
        assert!(s.ends_with("<?xpacket end=\"w\"?>"));
        assert!(!s.contains("CreateDate"));
    }
}
