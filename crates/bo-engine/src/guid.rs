//! GUID-shaped ids (`{XXXXXXXX-XXXX-4XXX-8XXX-XXXXXXXXXXXX}`, [MS-XLSX] `ST_Guid`) derived
//! from a seed, so the same edit of the same document always produces the same id
//! (collaboration replays and tests stay deterministic).

use std::hash::{Hash, Hasher};

/// The `n`-th id of `seed`.
pub(crate) fn guid_nth(seed: &impl Hash, n: u64) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut h);
    n.hash(&mut h);
    let a = h.finish();
    n.hash(&mut h);
    let b = h.finish();
    format!(
        "{{{:08X}-{:04X}-4{:03X}-8{:03X}-{:012X}}}",
        a >> 32,
        (a >> 16) & 0xFFFF,
        a & 0xFFF,
        b >> 52,
        b & 0xFFFF_FFFF_FFFF
    )
}

/// The first id of `seed` that is not `taken`.
pub(crate) fn guid(seed: &impl Hash, taken: &dyn Fn(&str) -> bool) -> String {
    let mut n = 0u64;
    loop {
        let id = guid_nth(seed, n);
        if !taken(&id) {
            return id;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_braced_version_4_guids_and_stable() {
        let id = guid_nth(&("person", "An"), 0);
        assert_eq!(id.len(), 38);
        assert!(id.starts_with('{') && id.ends_with('}'));
        let parts: Vec<&str> = id[1..37].split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4') && parts[3].starts_with('8'));
        assert!(id[1..37].chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
        assert_eq!(id, guid_nth(&("person", "An"), 0));
        assert_ne!(id, guid_nth(&("person", "An"), 1));
    }

    #[test]
    fn taken_ids_are_skipped() {
        let first = guid(&7u32, &|_| false);
        let second = guid(&7u32, &|g| g == first);
        assert_eq!(first, guid_nth(&7u32, 0));
        assert_eq!(second, guid_nth(&7u32, 1));
    }
}
