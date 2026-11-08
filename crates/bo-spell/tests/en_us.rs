//! The bundled English dictionary (SCOWL en_US 2020.12.07, `assets/dictionaries/en_US`) through bo-spell.
#![allow(clippy::unwrap_used)]

use bo_spell::{Checker, Dictionary};
use std::path::PathBuf;

fn en_us() -> Dictionary {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/dictionaries/en_US");
    let aff = std::fs::read(dir.join("en_US.aff")).unwrap();
    let dic = std::fs::read(dir.join("en_US.dic")).unwrap();
    Dictionary::from_aff_dic(&aff, &dic).unwrap()
}

// source: English words of common use; misspellings are standard typing errors
#[test]
fn english_words_and_misspellings() {
    let d = en_us();
    assert!(d.len() > 40_000);
    let ok = "the quick brown fox jumps over lazy dogs running ran unbelievable reorganized \
              children's studies happily organization documents spreadsheet presentations \
              Monday January English Vietnam Hanoi don't it's isn't colour's"
        .split_whitespace()
        .filter(|w| *w != "colour's");
    for w in ok {
        assert!(d.check(w), "{w}");
    }
    for w in ["teh", "recieve", "occured", "seperate", "definately", "accomodate", "goverment"] {
        assert!(!d.check(w), "{w}");
    }
}

#[test]
fn english_suggestions() {
    let d = en_us();
    let cases = [
        ("teh", "the"),
        ("recieve", "receive"),
        ("occured", "occurred"),
        ("seperate", "separate"),
        ("goverment", "government"),
        ("Accomodate", "Accommodate"),
    ];
    for (bad, good) in cases {
        let s = d.suggest(bad);
        assert!(s.iter().any(|x| x == good), "{bad}: {s:?}");
    }
}
