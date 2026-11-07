//! Spatial index of large precedent ranges for the dependency graph.
//!
//! Formulas reading large areas (e.g. `SUM(A1:A10000)`) are not indexed cell by
//! cell. Instead each area is registered in the fixed-size grid buckets it
//! overlaps, so finding the formulas that read a given cell only scans the
//! areas registered in that cell's bucket. Areas spanning very many buckets
//! (whole columns/rows) go to a small list that is scanned linearly.

use crate::fxhash::FxHashMap;
use crate::value::Area;
use crate::workbook::CellId;

/// Rows per bucket.
const ROW_CHUNK: u32 = 128;
/// Columns per bucket.
const COL_CHUNK: u32 = 32;
/// Areas overlapping more buckets than this are kept in the linear list.
const MAX_BUCKETS: u64 = 256;

#[derive(Default)]
pub(crate) struct RangeIndex {
    /// Slot → (area, dependent formula cell); `None` for freed slots.
    entries: Vec<Option<(Area, CellId)>>,
    free: Vec<u32>,
    buckets: FxHashMap<(usize, u32, u32), Vec<u32>>,
    huge: Vec<u32>,
    by_owner: FxHashMap<CellId, Vec<u32>>,
}

fn chunks(a: &Area) -> (u32, u32, u32, u32) {
    (
        a.r1 / ROW_CHUNK,
        a.r2 / ROW_CHUNK,
        a.c1 / COL_CHUNK,
        a.c2 / COL_CHUNK,
    )
}

fn bucket_count(a: &Area) -> u64 {
    let (r1, r2, c1, c2) = chunks(a);
    u64::from(r2 - r1 + 1) * u64::from(c2 - c1 + 1)
}

impl RangeIndex {
    /// Register `area` as read by formula cell `owner`.
    pub(crate) fn insert(&mut self, area: Area, owner: CellId) {
        let slot = if let Some(s) = self.free.pop() {
            self.entries[s as usize] = Some((area, owner));
            s
        } else {
            self.entries.push(Some((area, owner)));
            (self.entries.len() - 1) as u32
        };
        if bucket_count(&area) > MAX_BUCKETS {
            self.huge.push(slot);
        } else {
            let (r1, r2, c1, c2) = chunks(&area);
            for r in r1..=r2 {
                for c in c1..=c2 {
                    self.buckets
                        .entry((area.sheet, r, c))
                        .or_default()
                        .push(slot);
                }
            }
        }
        self.by_owner.entry(owner).or_default().push(slot);
    }

    /// Remove every area registered for `owner`.
    pub(crate) fn remove_owner(&mut self, owner: CellId) {
        let Some(slots) = self.by_owner.remove(&owner) else {
            return;
        };
        for slot in slots {
            let Some((area, _)) = self.entries[slot as usize].take() else {
                continue;
            };
            if bucket_count(&area) > MAX_BUCKETS {
                self.huge.retain(|s| *s != slot);
            } else {
                let (r1, r2, c1, c2) = chunks(&area);
                for r in r1..=r2 {
                    for c in c1..=c2 {
                        let key = (area.sheet, r, c);
                        if let Some(v) = self.buckets.get_mut(&key) {
                            v.retain(|s| *s != slot);
                            if v.is_empty() {
                                self.buckets.remove(&key);
                            }
                        }
                    }
                }
            }
            self.free.push(slot);
        }
    }

    /// Visit the formula cells whose registered areas contain the cell.
    pub(crate) fn for_each_reader(&self, cell: CellId, f: &mut dyn FnMut(CellId)) {
        let key = (cell.sheet, cell.row / ROW_CHUNK, cell.col / COL_CHUNK);
        let bucket = self.buckets.get(&key).map_or(&[][..], Vec::as_slice);
        for &slot in bucket.iter().chain(self.huge.iter()) {
            if let Some((a, owner)) = &self.entries[slot as usize] {
                if a.contains(cell.sheet, cell.row, cell.col) {
                    f(*owner);
                }
            }
        }
    }

    /// Drop everything.
    pub(crate) fn clear(&mut self) {
        *self = RangeIndex::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_readers_and_removes() {
        let mut ix = RangeIndex::default();
        let f1 = CellId::new(0, 0, 100);
        let f2 = CellId::new(0, 1, 100);
        ix.insert(Area::new(0, 0, 0, 9999, 0), f1);
        ix.insert(Area::new(0, 0, 0, 1_048_575, 3), f2);
        let readers = |ix: &RangeIndex, r, c| {
            let mut v = Vec::new();
            ix.for_each_reader(CellId::new(0, r, c), &mut |o| v.push(o));
            v.sort_unstable();
            v
        };
        assert_eq!(readers(&ix, 5000, 0), vec![f1, f2]);
        assert_eq!(readers(&ix, 20000, 0), vec![f2]);
        assert_eq!(readers(&ix, 5, 4), Vec::<CellId>::new());
        ix.remove_owner(f1);
        assert_eq!(readers(&ix, 5000, 0), vec![f2]);
        ix.remove_owner(f2);
        assert_eq!(readers(&ix, 5000, 0), Vec::<CellId>::new());
    }
}
