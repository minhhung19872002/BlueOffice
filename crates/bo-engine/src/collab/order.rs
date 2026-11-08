//! Order keys for shared sequences (table rows/columns, sheet rows/columns,
//! sheets, slides). An item's position is its key, a number, ties broken by
//! the item id; every item is a separate last-writer-wins entry, so
//! concurrent inserts and moves of *different* items all survive and every
//! replica sorts them the same way. A move or insert rewrites only the moved
//! or new items.

use std::cmp::Ordering;

/// Total order of `(key, id)` pairs.
pub(crate) fn cmp(a: (f64, &str), b: (f64, &str)) -> Ordering {
    a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1))
}

/// A key strictly between `a` and `b` (`None` = open end); `None` when the
/// float precision between them is used up.
#[allow(dead_code)]
pub(crate) fn between(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    let k = match (a, b) {
        (None, None) => 0.0,
        (Some(a), None) => a.floor() + 1.0,
        (None, Some(b)) => b.ceil() - 1.0,
        (Some(a), Some(b)) => a + (b - a) / 2.0,
    };
    let ok = k.is_finite() && a.is_none_or(|a| k > a) && b.is_none_or(|b| k < b);
    ok.then_some(k)
}

/// Keys for items in their new order: `known[i]` is the shared key of item
/// `i` (`None` for a new item) and `ids[i]` its id. The longest run of items
/// already in `(key, id)` order keeps its keys; the others get keys between
/// their kept neighbours. When precision runs out every item is renumbered.
pub(crate) fn assign(known: &[Option<f64>], ids: &[&str]) -> Vec<f64> {
    let n = known.len();
    // Longest strictly increasing subsequence of the known (key, id) pairs.
    let mut tails: Vec<usize> = Vec::new();
    let mut prev = vec![usize::MAX; n];
    for i in 0..n {
        let Some(k) = known[i] else { continue };
        let at = tails.partition_point(|&t| {
            known[t].is_some_and(|kt| cmp((kt, ids[t]), (k, ids[i])) == Ordering::Less)
        });
        if at > 0 {
            prev[i] = tails[at - 1];
        }
        if at == tails.len() {
            tails.push(i);
        } else {
            tails[at] = i;
        }
    }
    let mut keep = vec![false; n];
    let mut cur = tails.last().copied();
    while let Some(i) = cur {
        keep[i] = true;
        cur = (prev[i] != usize::MAX).then_some(prev[i]);
    }
    let mut out: Vec<f64> = vec![0.0; n];
    let mut i = 0;
    while i < n {
        if keep[i] {
            out[i] = known[i].unwrap_or(0.0);
            i += 1;
            continue;
        }
        let start = i;
        while i < n && !keep[i] {
            i += 1;
        }
        let lo = start.checked_sub(1).map(|j| out[j]);
        let hi = (i < n).then(|| known[i].unwrap_or(0.0));
        let m = (i - start) as f64;
        for (j, slot) in out.iter_mut().take(i).skip(start).enumerate() {
            let j = j as f64;
            *slot = match (lo, hi) {
                (None, None) => j,
                (Some(a), None) => a.floor() + 1.0 + j,
                (None, Some(b)) => b.ceil() - m + j,
                (Some(a), Some(b)) => a + (b - a) * (j + 1.0) / (m + 1.0),
            };
        }
    }
    // Equal neighbours can still collide with an id order that disagrees.
    let sorted = (1..n).all(|j| cmp((out[j - 1], ids[j - 1]), (out[j], ids[j])) == Ordering::Less);
    if sorted {
        out
    } else {
        (0..n).map(|j| j as f64).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_rewrite_only_the_moved_item() {
        let ids = ["c", "a", "b"];
        let k = assign(&[Some(2.0), Some(0.0), Some(1.0)], &ids);
        assert_eq!(&k[1..], &[0.0, 1.0]);
        assert!(k[0] < 0.0);
        let k = assign(&[Some(0.0), None, Some(1.0), None], &["a", "n", "b", "m"]);
        assert_eq!(k, vec![0.0, 0.5, 1.0, 2.0]);
    }

    #[test]
    fn exhausted_precision_renumbers() {
        let mut keys = vec![Some(0.0), Some(1.0)];
        let mut ids: Vec<String> = vec!["a".into(), "b".into()];
        for i in 0..200 {
            keys.insert(1, None);
            ids.insert(1, format!("n{i}"));
            let r: Vec<&str> = ids.iter().map(String::as_str).collect();
            let out = assign(&keys, &r);
            for j in 1..out.len() {
                assert_eq!(cmp((out[j - 1], r[j - 1]), (out[j], r[j])), Ordering::Less);
            }
            keys = out.into_iter().map(Some).collect();
        }
    }
}
