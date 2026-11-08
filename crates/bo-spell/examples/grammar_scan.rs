//! Prints the grammar issues of each line of a text file: `cargo run -p bo-spell --example grammar_scan -- <file>`.
#![allow(clippy::unwrap_used, missing_docs)]

fn main() {
    let path = std::env::args().nth(1).expect("usage: grammar_scan <file>");
    let text = std::fs::read_to_string(path).unwrap();
    let mut count = std::collections::BTreeMap::new();
    for line in text.lines() {
        for i in bo_spell::grammar::check(line) {
            *count.entry(i.rule.id()).or_insert(0) += 1;
            println!("{:<24} {:?} in {line:?}", i.rule.id(), &line[i.range]);
        }
    }
    println!("{count:?}");
}
