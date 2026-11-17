//! The document's security for Secure Document › Security Properties (BlueEditor round 10):
//! the standard security handler's encryption dictionary (ISO 32000-1 §7.6.1 Table 20, §7.6.3
//! Table 21 and Table 22 user access permissions, §7.6.5 crypt filters; ISO 32000-2 §7.6.4.4
//! AES-256 with `/V 5 /R 6`).
use crate::PdfDocument;
use lopdf::Object;

/// How the file is encrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityInfo {
    /// Security handler (`Standard` for passwords).
    pub filter: String,
    /// "256-bit AES", "128-bit AES", "128-bit RC4", "40-bit RC4".
    pub algorithm: String,
    /// The `/P` user access permissions (bit 3 print, 4 modify, 5 copy, 6 annotate, 9 fill
    /// in forms, 11 assemble, 12 print in high quality).
    pub permissions: i32,
}

impl PdfDocument {
    /// The encryption of the file as opened, `None` when it is not encrypted.
    pub fn security_info(&self) -> Option<SecurityInfo> {
        let doc = &self.doc;
        let enc = doc.trailer.get(b"Encrypt").ok()?;
        let d = crate::resolve(doc, enc).as_dict().ok()?;
        let int = |k: &[u8]| d.get(k).ok().and_then(|o| o.as_i64().ok());
        let v = int(b"V").unwrap_or(0);
        let length = int(b"Length").unwrap_or(40);
        let cfm = d
            .get(b"CF")
            .ok()
            .map(|o| crate::resolve(doc, o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|cf| cf.get(b"StdCF").ok())
            .map(|o| crate::resolve(doc, o))
            .and_then(|o| o.as_dict().ok())
            .and_then(|s| s.get(b"CFM").and_then(Object::as_name).ok())
            .map(<[u8]>::to_vec);
        let algorithm = match (v, cfm.as_deref()) {
            (5, _) | (_, Some(b"AESV3")) => "256-bit AES".to_owned(),
            (_, Some(b"AESV2")) => "128-bit AES".to_owned(),
            (1, _) => "40-bit RC4".to_owned(),
            _ => format!("{length}-bit RC4"),
        };
        Some(SecurityInfo {
            filter: d
                .get(b"Filter")
                .and_then(Object::as_name)
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default(),
            algorithm,
            permissions: i32::try_from(int(b"P").unwrap_or(-1)).unwrap_or(-1),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{standard_fonts, PdfDocument, Permissions};

    fn pdf() -> Vec<u8> {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>",
        ];
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offs = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offs.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let x = out.len();
        out.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
        for o in offs {
            out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n").as_bytes(),
        );
        out
    }

    #[test]
    fn security_properties_read_the_encryption() {
        let mut d = PdfDocument::open(pdf(), standard_fonts()).expect("opens");
        assert_eq!(d.security_info(), None);
        let allow = Permissions {
            print: true,
            modify: false,
            copy: false,
            annotate: true,
            assemble: false,
        };
        d.set_protection("u", "o", allow, true).expect("protects");
        let bytes = d.save().expect("saves");
        let e = PdfDocument::open_with_password(bytes, standard_fonts(), "u").expect("opens");
        let info = e.security_info().expect("encrypted");
        assert_eq!(info.algorithm, "256-bit AES");
        assert_eq!(info.filter, "Standard");
        assert!(
            info.permissions & (1 << 2) != 0,
            "print {:b}",
            info.permissions
        );
        assert!(
            info.permissions & (1 << 4) == 0,
            "copy {:b}",
            info.permissions
        );
    }
}
