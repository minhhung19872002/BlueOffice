//! Which top-level blocks of the main story changed (incremental layout, prompt §5.4).
//!
//! Editing transactions record every block splice here; the layout re-shapes only
//! the changed blocks and the engine updates its per-block caches (word counts,
//! the `modified` flag) from the same ranges. Header/footer edits are recorded as
//! the first section whose headers or footers changed (the layout decorates the
//! pages again without re-paginating while their heights stay the same). Other
//! changes (styles, numbering, section properties…) mark everything.

/// The changed range of one section, in the current block indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionChange {
    /// Section index.
    pub section: usize,
    /// First changed block.
    pub start: usize,
    /// End (exclusive) of the changed blocks.
    pub end: usize,
    /// Current block count minus the previous one.
    pub delta: isize,
}

impl SectionChange {
    /// End (exclusive) of the replaced range in the previous block indexes.
    pub fn old_end(&self) -> usize {
        (self.end as isize - self.delta).max(self.start as isize) as usize
    }

    /// Previous index of current block `i` (`None` inside the changed range).
    pub fn old_index(&self, i: usize) -> Option<usize> {
        if i < self.start {
            Some(i)
        } else if i >= self.end {
            Some((i as isize - self.delta) as usize)
        } else {
            None
        }
    }
}

/// Accumulated block changes: everything, or one covering range per section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockChanges {
    all: bool,
    sections: Vec<SectionChange>,
    /// First section whose headers/footers (or the header selection) changed.
    headers: Option<usize>,
}

impl BlockChanges {
    /// Everything changed (a new document, styles, numbering, sections…).
    pub fn everything() -> BlockChanges {
        BlockChanges {
            all: true,
            sections: Vec::new(),
            headers: None,
        }
    }

    /// Nothing changed.
    pub fn is_empty(&self) -> bool {
        !self.all && self.sections.is_empty() && self.headers.is_none()
    }

    /// Everything must be considered changed.
    pub fn is_all(&self) -> bool {
        self.all
    }

    /// Changed ranges by section (empty when [`BlockChanges::is_all`]).
    pub fn sections(&self) -> &[SectionChange] {
        &self.sections
    }

    /// Changed range of `section`.
    pub fn section(&self, section: usize) -> Option<&SectionChange> {
        self.sections.iter().find(|c| c.section == section)
    }

    /// First section whose headers or footers changed (later sections may inherit
    /// them, ECMA-376 Part 1, §17.10.5); `None` when [`BlockChanges::is_all`].
    pub fn headers(&self) -> Option<usize> {
        self.headers
    }

    /// Mark everything changed.
    pub fn mark_all(&mut self) {
        self.all = true;
        self.sections.clear();
        self.headers = None;
    }

    /// Record that the headers/footers of `section` changed (content, title page or
    /// the even/odd setting; not the section's other properties).
    pub fn mark_headers(&mut self, section: usize) {
        if self.all {
            return;
        }
        self.headers = Some(self.headers.map_or(section, |s| s.min(section)));
    }

    /// Record that `removed` blocks at `index` of `section` were replaced by `inserted` blocks.
    pub fn splice(&mut self, section: usize, index: usize, removed: usize, inserted: usize) {
        if self.all {
            return;
        }
        let n = inserted as isize - removed as isize;
        match self.sections.iter_mut().find(|c| c.section == section) {
            None => self.sections.push(SectionChange {
                section,
                start: index,
                end: index + inserted,
                delta: n,
            }),
            Some(c) => {
                // Where the previous end lands after this splice.
                let shifted = if c.end <= index {
                    c.end
                } else if c.end >= index + removed {
                    (c.end as isize + n) as usize
                } else {
                    index + inserted
                };
                c.start = c.start.min(index);
                c.end = shifted.max(index + inserted);
                c.delta += n;
            }
        }
    }

    /// Append the changes `later`, made after these.
    pub fn extend(&mut self, later: &BlockChanges) {
        if later.all {
            self.mark_all();
            return;
        }
        if let Some(s) = later.headers {
            self.mark_headers(s);
        }
        for c in &later.sections {
            self.splice(c.section, c.start, c.old_end() - c.start, c.end - c.start);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Apply `changes` to a list of ids and check that only the covered range differs.
    fn check(ops: &[(usize, usize, usize)]) {
        let mut v: Vec<u32> = (0..20).collect();
        let original = v.clone();
        let mut next = 100;
        let mut ch = BlockChanges::default();
        for &(i, r, n) in ops {
            let ins: Vec<u32> = (0..n)
                .map(|_| {
                    next += 1;
                    next
                })
                .collect();
            v.splice(i..i + r, ins);
            ch.splice(0, i, r, n);
        }
        let c = ch.section(0).copied().expect("changed");
        assert_eq!(v.len() as isize - original.len() as isize, c.delta);
        for (i, x) in v.iter().enumerate() {
            if let Some(o) = c.old_index(i) {
                assert_eq!(*x, original[o], "{ops:?} at {i}");
            }
        }
    }

    #[test]
    fn composed_splices_cover_every_change() {
        check(&[(5, 1, 1)]);
        check(&[(5, 1, 2), (6, 1, 1)]);
        check(&[(5, 1, 2), (2, 1, 1)]);
        check(&[(5, 1, 2), (10, 2, 0)]);
        check(&[(5, 3, 0), (4, 2, 1), (12, 0, 3)]);
        check(&[(0, 0, 1), (19, 1, 1)]);
        let mut a = BlockChanges::default();
        a.splice(0, 3, 1, 2);
        let mut b = BlockChanges::default();
        b.splice(0, 8, 1, 0);
        a.extend(&b);
        assert_eq!(
            a.section(0).map(|c| (c.start, c.end, c.delta)),
            Some((3, 8, 0))
        );
    }
}
