//! Property: any set of parts survives write → read unchanged.

use bo_opc::{Limits, Package};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn arbitrary_parts_roundtrip(parts in proptest::collection::btree_map("[a-z]{1,8}/[a-z0-9]{1,8}\\.bin", proptest::collection::vec(any::<u8>(), 0..512), 0..8)) {
        let mut p = Package::new();
        for (name, data) in &parts {
            p.put_part(&format!("/{name}"), data.clone());
        }
        let q = Package::read(&p.write().unwrap(), &Limits::default()).unwrap();
        for (name, data) in &parts {
            prop_assert_eq!(&q.part(&format!("/{name}")).unwrap().data, data);
        }
    }

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = Package::read(&bytes, &Limits::default());
    }
}
