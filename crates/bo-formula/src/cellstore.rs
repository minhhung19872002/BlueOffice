//! Cell storage of one sheet of the formula engine: cells are grouped in blocks of
//! [`BLOCK_ROWS`] rows of one column, so the cells of a column lie next to each other in
//! memory. Recalculating a column of formulas then walks memory in order instead of
//! jumping around a hash table of every cell (a hash lookup per cell missed the cache on
//! large sheets), and the block table stays small enough to be cache resident (1 000 000
//! cells in 100 columns are ~4 000 blocks).
//!
//! A block starts sparse (a short list) and becomes a dense array once it holds
//! [`DENSE_AT`] cells, so scattered cells cost about what a hash map entry costs.

use crate::fxhash::FxHashMap;

/// Rows per block (a power of two).
const BLOCK_ROWS: u32 = 256;
const BLOCK_SHIFT: u32 = BLOCK_ROWS.trailing_zeros();
/// A sparse block becomes dense at this many cells.
const DENSE_AT: usize = 24;

#[derive(Debug)]
enum Block<T> {
    /// `(row within the block, cell)`, unordered.
    Sparse(Vec<(u16, T)>),
    /// One slot per row of the block.
    Dense { count: u32, slots: Box<[Option<T>]> },
}

impl<T> Block<T> {
    fn get(&self, off: u16) -> Option<&T> {
        match self {
            Block::Sparse(v) => v.iter().find(|(o, _)| *o == off).map(|(_, t)| t),
            Block::Dense { slots, .. } => slots[usize::from(off)].as_ref(),
        }
    }

    fn get_mut(&mut self, off: u16) -> Option<&mut T> {
        match self {
            Block::Sparse(v) => v.iter_mut().find(|(o, _)| *o == off).map(|(_, t)| t),
            Block::Dense { slots, .. } => slots[usize::from(off)].as_mut(),
        }
    }

    /// Inserts; returns the previous cell.
    fn insert(&mut self, off: u16, value: T) -> Option<T> {
        match self {
            Block::Sparse(v) => {
                if let Some((_, t)) = v.iter_mut().find(|(o, _)| *o == off) {
                    return Some(std::mem::replace(t, value));
                }
                if v.len() + 1 < DENSE_AT {
                    v.push((off, value));
                    return None;
                }
                let mut slots: Box<[Option<T>]> = (0..BLOCK_ROWS).map(|_| None).collect();
                let count = v.len() as u32 + 1;
                for (o, t) in v.drain(..) {
                    slots[usize::from(o)] = Some(t);
                }
                slots[usize::from(off)] = Some(value);
                *self = Block::Dense { count, slots };
                None
            }
            Block::Dense { count, slots } => {
                let old = slots[usize::from(off)].replace(value);
                if old.is_none() {
                    *count += 1;
                }
                old
            }
        }
    }

    /// Removes; returns the cell and whether the block is now empty.
    fn remove(&mut self, off: u16) -> (Option<T>, bool) {
        match self {
            Block::Sparse(v) => {
                let old = v
                    .iter()
                    .position(|(o, _)| *o == off)
                    .map(|i| v.swap_remove(i).1);
                (old, v.is_empty())
            }
            Block::Dense { count, slots } => {
                let old = slots[usize::from(off)].take();
                if old.is_some() {
                    *count -= 1;
                }
                (old, *count == 0)
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        match self {
            Block::Sparse(v) => v.len(),
            Block::Dense { count, .. } => *count as usize,
        }
    }
}

/// Cells of one sheet keyed by `(row, column)`.
#[derive(Debug)]
pub(crate) struct CellStore<T> {
    /// `(column, row >> BLOCK_SHIFT)` → block.
    blocks: FxHashMap<(u32, u32), Block<T>>,
    len: usize,
}

impl<T> Default for CellStore<T> {
    fn default() -> Self {
        CellStore {
            blocks: FxHashMap::default(),
            len: 0,
        }
    }
}

#[inline]
fn split(row: u32, col: u32) -> ((u32, u32), u16) {
    ((col, row >> BLOCK_SHIFT), (row & (BLOCK_ROWS - 1)) as u16)
}

// Keys by reference, like the hash map this replaced (call sites unchanged).
#[allow(clippy::trivially_copy_pass_by_ref)]
impl<T> CellStore<T> {
    /// Number of cells.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Room for `additional` more cells (the block table only).
    pub(crate) fn reserve(&mut self, additional: usize) {
        self.blocks
            .reserve(additional / BLOCK_ROWS as usize + usize::from(additional > 0));
    }

    pub(crate) fn get(&self, &(row, col): &(u32, u32)) -> Option<&T> {
        let (key, off) = split(row, col);
        self.blocks.get(&key)?.get(off)
    }

    pub(crate) fn get_mut(&mut self, &(row, col): &(u32, u32)) -> Option<&mut T> {
        let (key, off) = split(row, col);
        self.blocks.get_mut(&key)?.get_mut(off)
    }

    pub(crate) fn contains_key(&self, key: &(u32, u32)) -> bool {
        self.get(key).is_some()
    }

    /// Inserts a cell; returns the previous one.
    pub(crate) fn insert(&mut self, (row, col): (u32, u32), value: T) -> Option<T> {
        let (key, off) = split(row, col);
        let old = self
            .blocks
            .entry(key)
            .or_insert_with(|| Block::Sparse(Vec::new()))
            .insert(off, value);
        if old.is_none() {
            self.len += 1;
        }
        old
    }

    /// Removes a cell.
    pub(crate) fn remove(&mut self, &(row, col): &(u32, u32)) -> Option<T> {
        let (key, off) = split(row, col);
        let block = self.blocks.get_mut(&key)?;
        let (old, empty) = block.remove(off);
        if empty {
            self.blocks.remove(&key);
        }
        if old.is_some() {
            self.len -= 1;
        }
        old
    }

    /// Every cell with its `(row, column)`, in no particular order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = ((u32, u32), &T)> + '_ {
        self.blocks.iter().flat_map(|(&(col, b), block)| {
            let base = b << BLOCK_SHIFT;
            let it: Box<dyn Iterator<Item = ((u32, u32), &T)> + '_> = match block {
                Block::Sparse(v) => {
                    Box::new(v.iter().map(move |(o, t)| ((base + u32::from(*o), col), t)))
                }
                Block::Dense { slots, .. } => Box::new(
                    slots
                        .iter()
                        .enumerate()
                        .filter_map(move |(o, t)| t.as_ref().map(|t| ((base + o as u32, col), t))),
                ),
            };
            it
        })
    }

    /// [`CellStore::iter`] with mutable cells.
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = ((u32, u32), &mut T)> + '_ {
        self.blocks.iter_mut().flat_map(|(&(col, b), block)| {
            let base = b << BLOCK_SHIFT;
            let it: Box<dyn Iterator<Item = ((u32, u32), &mut T)> + '_> = match block {
                Block::Sparse(v) => Box::new(
                    v.iter_mut()
                        .map(move |(o, t)| ((base + u32::from(*o), col), t)),
                ),
                Block::Dense { slots, .. } => Box::new(
                    slots
                        .iter_mut()
                        .enumerate()
                        .filter_map(move |(o, t)| t.as_mut().map(|t| ((base + o as u32, col), t))),
                ),
            };
            it
        })
    }

    /// Takes every cell out, leaving the store empty.
    pub(crate) fn drain(&mut self) -> Vec<((u32, u32), T)> {
        let mut out = Vec::with_capacity(self.len);
        self.len = 0;
        for ((col, b), block) in std::mem::take(&mut self.blocks) {
            let base = b << BLOCK_SHIFT;
            match block {
                Block::Sparse(v) => {
                    out.extend(v.into_iter().map(|(o, t)| ((base + u32::from(o), col), t)));
                }
                Block::Dense { slots, .. } => {
                    out.extend(
                        slots
                            .into_vec()
                            .into_iter()
                            .enumerate()
                            .filter_map(|(o, t)| t.map(|t| ((base + o as u32, col), t))),
                    );
                }
            }
        }
        out
    }

    /// Number of blocks (tests).
    #[cfg(test)]
    fn block_count(&self) -> usize {
        debug_assert_eq!(
            self.blocks.values().map(Block::len).sum::<usize>(),
            self.len
        );
        self.blocks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn behaves_like_a_map() {
        let mut s: CellStore<u64> = CellStore::default();
        let mut m: BTreeMap<(u32, u32), u64> = BTreeMap::new();
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        for i in 0..20_000u64 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            // Mostly a dense corner, some scattered cells.
            let (r, c) = if x.is_multiple_of(5) {
                ((x >> 8) as u32 % 1_048_576, (x >> 40) as u32 % 16_384)
            } else {
                ((x >> 8) as u32 % 2_000, (x >> 40) as u32 % 6)
            };
            if x.is_multiple_of(3) {
                assert_eq!(s.remove(&(r, c)), m.remove(&(r, c)));
            } else {
                assert_eq!(s.insert((r, c), i), m.insert((r, c), i));
            }
            assert_eq!(s.len(), m.len());
        }
        for (k, v) in &m {
            assert_eq!(s.get(k), Some(v));
        }
        let mut all: Vec<((u32, u32), u64)> = s.iter().map(|(k, v)| (k, *v)).collect();
        all.sort_unstable();
        assert_eq!(all, m.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>());
        for (_, v) in s.iter_mut() {
            *v += 1;
        }
        assert!(s.block_count() > 0);
        let mut drained = s.drain();
        drained.sort_unstable();
        assert_eq!(
            drained,
            m.iter().map(|(k, v)| (*k, *v + 1)).collect::<Vec<_>>()
        );
        assert_eq!(s.len(), 0);
        assert!(s.get(&(0, 0)).is_none());
    }

    #[test]
    fn empty_blocks_are_dropped() {
        let mut s: CellStore<u8> = CellStore::default();
        for r in 0..300 {
            s.insert((r, 2), 1);
        }
        assert_eq!(s.block_count(), 2);
        for r in 0..256 {
            s.remove(&(r, 2));
        }
        assert_eq!(s.block_count(), 1);
        assert_eq!(s.len(), 44);
    }
}
