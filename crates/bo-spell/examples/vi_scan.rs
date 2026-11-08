//! Lists words of a text file that the Vietnamese checker rejects (manual check of false positives).
use bo_spell::{vi::Vietnamese, words, Checker};
fn main() {
    let path = std::env::args().nth(1).unwrap_or_default();
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut bad = std::collections::BTreeMap::<String, usize>::new();
    let mut total = 0;
    for r in words(&text) {
        let w = &text[r];
        // Only words with Vietnamese letters (diacritics or đ) — plain ASCII may be English/code.
        if !w.chars().any(|c| !c.is_ascii() && c.is_alphabetic()) {
            continue;
        }
        total += 1;
        if !Vietnamese.check(w) {
            *bad.entry(w.to_owned()).or_default() += 1;
        }
    }
    println!("{total} words, {} rejected kinds", bad.len());
    for (w, n) in bad {
        println!("{n:4} {w}");
    }
}
