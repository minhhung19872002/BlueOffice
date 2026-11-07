//! Compound-file container of an encrypted package ([MS-CFB], [MS-OFFCRYPTO] §2.1, §2.3.4.4).

use crate::{CryptoError, Result, MAX_STREAM_SIZE};
use std::io::{Cursor, Read, Write};

const ENCRYPTION_INFO: &str = "/EncryptionInfo";
const ENCRYPTED_PACKAGE: &str = "/EncryptedPackage";

fn read_stream<F: Read + std::io::Seek>(
    cf: &mut cfb::CompoundFile<F>,
    path: &str,
) -> Result<Vec<u8>> {
    if !cf.is_stream(path) {
        return Err(CryptoError::NotEncrypted);
    }
    let len = cf.entry(path)?.len();
    if len > MAX_STREAM_SIZE {
        return Err(CryptoError::Malformed(format!(
            "stream {path} is {len} bytes"
        )));
    }
    let mut s = cf.open_stream(path)?;
    let mut buf = Vec::with_capacity(len.min(64 << 20) as usize);
    (&mut s).take(MAX_STREAM_SIZE).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Read (`EncryptionInfo`, `EncryptedPackage`).
pub fn read_streams(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut cf = cfb::CompoundFile::open(Cursor::new(bytes))?;
    let info = read_stream(&mut cf, ENCRYPTION_INFO)?;
    let pkg = read_stream(&mut cf, ENCRYPTED_PACKAGE)?;
    Ok((info, pkg))
}

/// UNICODE-LP-P4 ([MS-OFFCRYPTO] §2.1.2): byte length, UTF-16LE, pad to 4.
fn lp_p4(out: &mut Vec<u8>, s: &str) {
    let bytes: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
    out.extend((bytes.len() as u32).to_le_bytes());
    out.extend(&bytes);
    while out.len() & 3 != 0 {
        out.push(0);
    }
}

fn version_triplet(out: &mut Vec<u8>) {
    // ReaderVersion, UpdaterVersion, WriterVersion = 1.0 (§2.1.5 Version)
    for _ in 0..3 {
        out.extend(1u16.to_le_bytes());
        out.extend(0u16.to_le_bytes());
    }
}

/// `\x06DataSpaces` storage contents (§2.1.6 – §2.1.8, §2.3.4.4).
fn dataspaces() -> Vec<(String, Vec<u8>)> {
    // §2.1.5 DataSpaceVersionInfo
    let mut version = Vec::new();
    lp_p4(&mut version, "Microsoft.Container.DataSpaces");
    version_triplet(&mut version);

    // §2.1.6 DataSpaceMap with one DataSpaceMapEntry
    let mut entry = Vec::new();
    entry.extend(1u32.to_le_bytes()); // ReferenceComponentCount
    entry.extend(0u32.to_le_bytes()); // ReferenceComponentType: stream
    lp_p4(&mut entry, "EncryptedPackage");
    lp_p4(&mut entry, "StrongEncryptionDataSpace");
    let mut map = Vec::new();
    map.extend(8u32.to_le_bytes()); // HeaderLength
    map.extend(1u32.to_le_bytes()); // EntryCount
    map.extend((entry.len() as u32 + 4).to_le_bytes()); // Length
    map.extend(entry);

    // §2.1.7 DataSpaceDefinition
    let mut def = Vec::new();
    def.extend(8u32.to_le_bytes()); // HeaderLength
    def.extend(1u32.to_le_bytes()); // TransformReferenceCount
    lp_p4(&mut def, "StrongEncryptionTransform");

    // §2.1.8 TransformInfoHeader + §2.3.4.? EncryptionTransformInfo
    let mut id = Vec::new();
    lp_p4(&mut id, "{FF9A3F03-56EF-4613-BDD5-5A41C1D07246}");
    let mut primary = Vec::new();
    primary.extend((8 + id.len() as u32).to_le_bytes()); // TransformLength
    primary.extend(1u32.to_le_bytes()); // TransformType
    primary.extend(id);
    lp_p4(&mut primary, "Microsoft.Container.EncryptionTransform");
    version_triplet(&mut primary);
    primary.extend(0u32.to_le_bytes()); // EncryptionName (UTF-8-LP-P4, empty)
    primary.extend(0u32.to_le_bytes()); // EncryptionBlockSize
    primary.extend(0u32.to_le_bytes()); // CipherMode
    primary.extend(4u32.to_le_bytes()); // Reserved

    vec![
        ("/\u{6}DataSpaces/Version".into(), version),
        ("/\u{6}DataSpaces/DataSpaceMap".into(), map),
        (
            "/\u{6}DataSpaces/DataSpaceInfo/StrongEncryptionDataSpace".into(),
            def,
        ),
        (
            "/\u{6}DataSpaces/TransformInfo/StrongEncryptionTransform/\u{6}Primary".into(),
            primary,
        ),
    ]
}

/// Build the compound file (version 3, 512-byte sectors).
pub fn write_container(info: &[u8], package: &[u8]) -> Result<Vec<u8>> {
    let mut cf = cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new()))?;
    let mut streams = dataspaces();
    streams.push((ENCRYPTION_INFO.into(), info.to_vec()));
    streams.push((ENCRYPTED_PACKAGE.into(), package.to_vec()));
    for (path, data) in streams {
        if let Some((parent, _)) = path.rsplit_once('/') {
            if !parent.is_empty() && !cf.is_storage(parent) {
                cf.create_storage_all(parent)?;
            }
        }
        let mut s = cf.create_stream(&path)?;
        s.write_all(&data)?;
    }
    cf.flush()?;
    Ok(cf.into_inner().into_inner())
}
