//! Hashed index of a lookup vector for exact-match lookups.
//!
//! `VLOOKUP`/`HLOOKUP`/`MATCH`/`XLOOKUP`/`XMATCH` with an exact match compare
//! the lookup value against every key with spreadsheet equality (ECMA-376
//! Part 1, §18.17.7: same type, numbers equal at 15 significant digits, text
//! case-insensitive). That relation is an equivalence, so the keys can be
//! normalized and hashed once per recalculation pass: thousands of lookups into
//! the same table then cost one hash probe each instead of a linear scan.

use crate::fxhash::FxHashMap;
use crate::value::{round_sig15, Value};

/// Normalized form of a scalar under exact-match equality.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Num(u64),
    Text(String),
    Bool(bool),
    Empty,
}

fn key(v: &Value) -> Option<Key> {
    match v {
        Value::Number(n) => {
            if n.is_nan() {
                return None;
            }
            // Integers below 1e15 are exact at 15 significant digits.
            let r = if n.fract() == 0.0 && n.abs() < 1e15 {
                *n
            } else {
                round_sig15(*n)
            };
            // -0 and +0 are equal.
            let r = if r == 0.0 { 0.0 } else { r };
            Some(Key::Num(r.to_bits()))
        }
        Value::Text(t) => Some(Key::Text(t.to_lowercase())),
        Value::Bool(b) => Some(Key::Bool(*b)),
        Value::Empty => Some(Key::Empty),
        _ => None,
    }
}

/// Positions of every distinct key of a lookup vector.
#[derive(Debug, Default)]
pub struct ExactIndex {
    /// Key → (first position, last position).
    map: FxHashMap<Key, (usize, usize)>,
}

impl ExactIndex {
    /// Index the values of a vector in order.
    pub fn build<'a>(values: impl IntoIterator<Item = &'a Value>) -> ExactIndex {
        let mut map: FxHashMap<Key, (usize, usize)> = FxHashMap::default();
        for (i, v) in values.into_iter().enumerate() {
            if let Some(k) = key(v) {
                map.entry(k).and_modify(|e| e.1 = i).or_insert((i, i));
            }
        }
        ExactIndex { map }
    }

    /// Position of the first (or last, when `reverse`) key equal to `needle`.
    ///
    /// Wildcard patterns are not handled here; callers fall back to a scan.
    pub fn find(&self, needle: &Value, reverse: bool) -> Option<usize> {
        let k = key(needle)?;
        self.map
            .get(&k)
            .map(|&(first, last)| if reverse { last } else { first })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_keys() {
        let vals = [
            Value::Number(1.0),
            Value::Text("Abc".into()),
            Value::Number(0.1 + 0.2),
            Value::Bool(true),
            Value::Empty,
            Value::Text("abc".into()),
            Value::Number(-0.0),
        ];
        let ix = ExactIndex::build(vals.iter());
        assert_eq!(ix.find(&Value::Number(0.3), false), Some(2));
        assert_eq!(ix.find(&Value::Text("ABC".into()), false), Some(1));
        assert_eq!(ix.find(&Value::Text("ABC".into()), true), Some(5));
        assert_eq!(ix.find(&Value::Bool(true), false), Some(3));
        assert_eq!(ix.find(&Value::Bool(false), false), None);
        assert_eq!(ix.find(&Value::Empty, false), Some(4));
        assert_eq!(ix.find(&Value::Number(0.0), false), Some(6));
        assert_eq!(ix.find(&Value::Text("1".into()), false), None);
    }
}
