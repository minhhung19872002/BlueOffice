//! Table gridlines of the view (BlueDoc round 8, Table Layout › View Gridlines): the layout
//! marks cell edges without a border with view-only rules (`bo_layout::GRID_STYLE`, never
//! rendered into pages or exports); the view draws them dashed over the page, as the
//! desktop word processor does (1 px #939393, 3 on / 1 off at 125 %).
use crate::Engine;
use bo_layout::PageItem;

impl Engine {
    /// Lay out gridline rules from now on (BlueDoc native; the next layout is complete).
    pub fn set_table_grid(&mut self, on: bool) {
        bo_layout::set_table_grid(on);
        self.cache.clear();
        self.layout.reset();
    }

    /// Gridline segments (x1, y1, x2, y2 in points) of page `index`, without those that lie
    /// on a drawn border.
    pub fn table_gridlines(&self, index: usize) -> Vec<[f32; 4]> {
        let Some(page) = self.layout().and_then(|l| l.pages.get(index)) else {
            return Vec::new();
        };
        let mut grid = Vec::new();
        let mut drawn = Vec::new();
        for it in &page.items {
            if let PageItem::Rule {
                x1,
                y1,
                x2,
                y2,
                style,
                ..
            } = it
            {
                let seg = [*x1, *y1, *x2, *y2];
                if style == bo_layout::GRID_STYLE {
                    grid.push(seg);
                } else {
                    drawn.push(seg);
                }
            }
        }
        grid.retain(|g| !drawn.iter().any(|d| covers(d, g)));
        grid.dedup();
        grid
    }
}

/// `d` lies on the same horizontal / vertical line as `g` and spans most of it.
fn covers(d: &[f32; 4], g: &[f32; 4]) -> bool {
    const TOL: f32 = 1.0;
    let horizontal = |s: &[f32; 4]| (s[1] - s[3]).abs() < 0.01;
    if horizontal(g) != horizontal(d) {
        return false;
    }
    let (a, b, c, e) = if horizontal(g) {
        if (g[1] - d[1]).abs() > TOL {
            return false;
        }
        (
            g[0].min(g[2]),
            g[0].max(g[2]),
            d[0].min(d[2]),
            d[0].max(d[2]),
        )
    } else {
        if (g[0] - d[0]).abs() > TOL {
            return false;
        }
        (
            g[1].min(g[3]),
            g[1].max(g[3]),
            d[1].min(d[3]),
            d[1].max(d[3]),
        )
    };
    let overlap = (b.min(e) - a.max(c)).max(0.0);
    overlap >= (b - a) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::Command;

    #[test]
    fn borderless_table_edges_become_gridlines_not_page_content() {
        let mut e = Engine::with_bundled_fonts();
        e.set_table_grid(true);
        e.new_document("a.docx").expect("new");
        e.command(Command::InsertTable { rows: 2, cols: 2 })
            .expect("table");
        // A new table has borders: no gridline on them.
        let with_borders = e.table_gridlines(0).len();
        e.command(Command::TableEdit {
            edit: bo_edit::TableEdit::Borders {
                preset: bo_edit::BorderPreset::None,
                pen: None,
            },
        })
        .expect("no borders");
        let grid = e.table_gridlines(0);
        bo_layout::set_table_grid(false);
        assert_eq!(with_borders, 0);
        assert!(grid.len() >= 8, "{grid:?}");
        // The page drawing never has them.
        let json = e.page_json(0).expect("page");
        assert!(!json.contains(bo_layout::GRID_STYLE));
    }
}
