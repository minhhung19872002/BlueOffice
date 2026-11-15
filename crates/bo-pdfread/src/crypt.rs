//! Opening files protected by a user password (standard security handler, ISO 32000-1
//! §7.6.3–7.6.4; ISO 32000-2 §7.6.4.4 for AES-256).
//!
//! `lopdf` decrypts only files whose user password is empty. For other passwords the bytes are
//! loaded as if unencrypted (the trailer's `/Encrypt` and the `/ObjStm` type names are renamed
//! in place, which keeps every offset valid), then every object is decrypted with the key the
//! password gives and the object streams are unpacked.
use lopdf::encryption::{decrypt_object, PasswordAlgorithm};
use lopdf::{Document, EncryptionState, Object, ObjectStream};

const ENCRYPT: &[u8] = b"/Encrypt";
const ENCRYPT_HIDDEN: &[u8] = b"/Encryp_";
const OBJSTM: &[u8] = b"/ObjStm";
const OBJSTM_HIDDEN: &[u8] = b"/ObjSt_";

/// Rename `/Encrypt` when it is followed by a reference (the trailer key, not
/// `/EncryptMetadata`) and every `/ObjStm`.
fn disguise(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i + ENCRYPT.len() < out.len() {
        if out[i..].starts_with(ENCRYPT) {
            let next = out[i + ENCRYPT.len()];
            if next.is_ascii_whitespace() || next.is_ascii_digit() {
                out[i..i + ENCRYPT.len()].copy_from_slice(ENCRYPT_HIDDEN);
            }
            i += ENCRYPT.len();
        } else if out[i..].starts_with(OBJSTM) {
            out[i..i + OBJSTM.len()].copy_from_slice(OBJSTM_HIDDEN);
            i += OBJSTM.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Load `bytes` (starting at `%PDF-`) decrypted with `password`. The result looks like a file
/// `lopdf` opened with an empty user password: objects decrypted, `/Encrypt` kept in the
/// trailer and objects, `encryption_state` set.
pub(crate) fn load(bytes: &[u8], password: &str) -> Result<Document, String> {
    let mut doc = Document::load_mem(&disguise(bytes)).map_err(|e| format!("PDF: {e}"))?;
    let hidden = ENCRYPT_HIDDEN[1..].to_vec();
    let enc = doc
        .trailer
        .remove(&hidden)
        .ok_or("the document is not encrypted")?;
    doc.trailer.set("Encrypt", enc.clone());
    let enc_id = enc.as_reference().map_err(|e| e.to_string())?;
    let algorithm = PasswordAlgorithm::try_from(&doc).map_err(|e| e.to_string())?;
    let pw = algorithm
        .sanitize_password(password)
        .map_err(|e| e.to_string())?;
    doc.authenticate_raw_password(&pw)
        .map_err(|_| "the password is not correct".to_owned())?;
    let state = EncryptionState::decode(&doc, &pw).map_err(|e| e.to_string())?;
    let mut packed = Vec::new();
    for (&id, o) in &mut doc.objects {
        if id == enc_id {
            continue;
        }
        let _ = decrypt_object(&state, id, o);
        if let Object::Stream(s) = o {
            if s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(&OBJSTM_HIDDEN[1..]) {
                s.dict.set("Type", Object::Name(OBJSTM[1..].to_vec()));
                packed.push(id);
            }
        }
    }
    for id in packed {
        let objects = match doc.objects.get_mut(&id) {
            Some(Object::Stream(s)) => ObjectStream::new(s).map(|o| o.objects).ok(),
            _ => None,
        };
        for (k, v) in objects.into_iter().flatten() {
            doc.objects.entry(k).or_insert(v);
        }
    }
    doc.max_id = doc.objects.keys().map(|k| k.0).max().unwrap_or(0);
    doc.encryption_state = Some(state);
    Ok(doc)
}
