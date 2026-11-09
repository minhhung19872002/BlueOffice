//! Page range text of the print panel: `1-3, 5, 8-`, plus `p2s1-p3s2` (page
//! *as numbered* in a section, BlueDoc).
//!
//! Rules (our own choice; no public spec defines this text):
//! * items are separated by `,` or `;`, spaces are ignored;
//! * `n` is the n-th printed page (1-based position in the document), `a-b` is
//!   inclusive, `a-` runs to the last page, `-b` starts at the first page;
//! * `pNsM` is the page showing number N in section M (both 1-based); either
//!   end of a span may use it;
//! * items keep the order they are written in and may repeat (`3, 1` prints
//!   page 3 then page 1); a span written backwards (`5-3`) prints 5, 4, 3;
//! * numbers past the last page are skipped, a span is cut at the last page.

/// A page of the source document as the range text can address it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageKey {
    /// Page number as displayed (PAGE field), 1-based.
    pub number: u32,
    /// Section index, 0-based.
    pub section: usize,
}

/// One end of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    /// 1-based position.
    Pos(usize),
    /// Displayed page number in a 1-based section.
    Sec { page: u32, section: usize },
}

/// A range text that cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("bad page range '{0}'")]
pub struct RangeError(pub String);

fn parse_end(s: &str) -> Option<End> {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix('p') {
        let (p, sec) = rest.split_once('s')?;
        let page: u32 = p.parse().ok()?;
        let section: usize = sec.parse().ok()?;
        return (page >= 1 && section >= 1).then_some(End::Sec { page, section });
    }
    let n: usize = s.parse().ok()?;
    (n >= 1).then_some(End::Pos(n))
}

/// 0-based index of `end` among `keys` (`None`: no such page).
fn resolve(end: End, total: usize, keys: Option<&[PageKey]>) -> Result<Option<usize>, ()> {
    match end {
        End::Pos(n) => Ok((n <= total).then(|| n - 1)),
        End::Sec { page, section } => {
            let keys = keys.ok_or(())?;
            Ok(keys
                .iter()
                .position(|k| k.number == page && k.section + 1 == section))
        }
    }
}

/// 0-based page indices selected by `text` in a document of `total` pages.
/// `keys` (one per page) enables the `pNsM` form.
pub fn parse_ranges(
    text: &str,
    total: usize,
    keys: Option<&[PageKey]>,
) -> Result<Vec<usize>, RangeError> {
    let bad = || RangeError(text.to_owned());
    let mut out = Vec::new();
    let mut any = false;
    for item in text.split([',', ';']) {
        let item: String = item.chars().filter(|c| !c.is_whitespace()).collect();
        if item.is_empty() {
            continue;
        }
        any = true;
        let (a, b) = match item.split_once('-') {
            None => {
                let e = parse_end(&item).ok_or_else(bad)?;
                if let Some(i) = resolve(e, total, keys).map_err(|()| bad())? {
                    out.push(i);
                }
                continue;
            }
            Some((a, b)) => (a, b),
        };
        let first = if a.is_empty() {
            Some(0)
        } else {
            let e = parse_end(a).ok_or_else(bad)?;
            resolve(e, total, keys).map_err(|()| bad())?
        };
        let last = if b.is_empty() {
            total.checked_sub(1)
        } else {
            let e = parse_end(b).ok_or_else(bad)?;
            match resolve(e, total, keys).map_err(|()| bad())? {
                Some(i) => Some(i),
                // A position past the end is cut at the last page.
                None if matches!(e, End::Pos(_)) => total.checked_sub(1),
                None => None,
            }
        };
        let (Some(first), Some(last)) = (first, last) else {
            continue;
        };
        if first <= last {
            out.extend(first..=last);
        } else {
            out.extend((last..=first).rev());
        }
    }
    if any {
        Ok(out)
    } else {
        Err(bad())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(text: &str, total: usize) -> Vec<usize> {
        parse_ranges(text, total, None).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn singles_spans_and_open_ends() {
        // "1-3, 5, 8-" of 10 pages: 1 2 3 5 8 9 10 → 0-based.
        assert_eq!(r("1-3, 5, 8-", 10), vec![0, 1, 2, 4, 7, 8, 9]);
        assert_eq!(r("-2", 10), vec![0, 1]);
        assert_eq!(r(" 4 ", 10), vec![3]);
        assert_eq!(r("2;4", 5), vec![1, 3]);
    }

    #[test]
    fn order_repeats_and_backwards_spans_are_kept() {
        assert_eq!(r("3,1,3", 5), vec![2, 0, 2]);
        assert_eq!(r("5-3", 5), vec![4, 3, 2]);
    }

    #[test]
    fn pages_past_the_end_are_skipped_or_cut() {
        assert_eq!(r("4, 12", 5), vec![3]);
        assert_eq!(r("4-12", 5), vec![3, 4]);
        assert_eq!(r("9-", 5), Vec::<usize>::new());
    }

    #[test]
    fn junk_is_an_error() {
        for t in ["", " , ", "a", "1-2-3", "0", "1.5", "p1", "p0s1"] {
            assert!(parse_ranges(t, 5, None).is_err(), "{t}");
        }
        // Sections need page keys.
        assert!(parse_ranges("p1s1", 5, None).is_err());
    }

    #[test]
    fn section_pages() {
        // Section 1: pages numbered 1, 2; section 2 restarts at 1: 1, 2, 3.
        let keys = [(1, 0), (2, 0), (1, 1), (2, 1), (3, 1)]
            .map(|(number, section)| PageKey { number, section });
        let k = Some(&keys[..]);
        assert_eq!(parse_ranges("p1s2", 5, k).unwrap(), vec![2]);
        assert_eq!(parse_ranges("p2s1-p2s2", 5, k).unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_ranges("P3S2, 1", 5, k).unwrap(), vec![4, 0]);
        // A page number the section does not have selects nothing.
        assert_eq!(parse_ranges("p9s1", 5, k).unwrap(), Vec::<usize>::new());
    }
}
