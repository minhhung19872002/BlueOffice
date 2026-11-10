//! Boundary data of map charts (CORE-CHART-006): regions with their names and outlines, the
//! pluggable [`BoundaryProvider`]s that hold them, name matching and map projections.
//!
//! * The world's countries are bundled ([`world`]: Natural Earth Admin-0, public domain; see
//!   `assets/maps/README.md`).
//! * Other boundary sets — the Vietnamese provinces once an approved dataset is available — are
//!   loaded at run time from a dataset folder (`dataset.json` + GeoJSON / TopoJSON + name aliases,
//!   [`dataset`]) and [`register`]ed; no code knows their contents.

pub mod dataset;
pub mod projection;
pub mod world;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use unicode_normalization::UnicodeNormalization;

/// A point `(longitude, latitude)` in degrees (WGS 84).
pub type LonLat = (f64, f64);

/// What a boundary set divides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapLevel {
    /// Countries / regions of the world.
    Country,
    /// First-level subdivisions of a country (provinces, states).
    Province,
}

impl MapLevel {
    /// The level of a chartex `ST_GeoMappingLevel` value: `state`, `county` and `postalCode` are
    /// subdivisions; the others (and automatic) countries.
    #[must_use]
    pub fn of_chartex(level: Option<&str>) -> MapLevel {
        match level {
            Some("state" | "county" | "postalCode") => MapLevel::Province,
            _ => MapLevel::Country,
        }
    }
}

/// One region: identifiers, names and outline rings (outer rings and holes, filled non-zero).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Region {
    /// Codes (ISO 3166-1 alpha-2 / alpha-3, a dataset's own code); matched case-insensitively.
    pub codes: Vec<String>,
    /// Names (the first is the display name); matched without case, diacritics or punctuation.
    pub names: Vec<String>,
    /// Rings of `(lon, lat)` points.
    pub rings: Vec<Vec<LonLat>>,
}

impl Region {
    /// Bounding box `[min lon, min lat, max lon, max lat]` of the rings.
    #[must_use]
    pub fn bbox(&self) -> Option<[f64; 4]> {
        bbox_of(self.rings.iter().flatten())
    }

    /// The ring with the largest area (where a label goes).
    #[must_use]
    pub fn main_ring(&self) -> Option<&Vec<LonLat>> {
        self.rings
            .iter()
            .max_by(|a, b| ring_area(a).total_cmp(&ring_area(b)))
    }
}

/// Bounding box of points.
#[must_use]
pub fn bbox_of<'a>(pts: impl IntoIterator<Item = &'a LonLat>) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for &(x, y) in pts {
        let v = b.get_or_insert([x, y, x, y]);
        v[0] = v[0].min(x);
        v[1] = v[1].min(y);
        v[2] = v[2].max(x);
        v[3] = v[3].max(y);
    }
    b
}

/// Unsigned planar area of a ring (shoelace formula, square degrees).
#[must_use]
pub fn ring_area(r: &[LonLat]) -> f64 {
    let n = r.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let (x0, y0) = r[i];
        let (x1, y1) = r[(i + 1) % n];
        s += x0 * y1 - x1 * y0;
    }
    (s / 2.0).abs()
}

/// A source of region boundaries for map charts.
pub trait BoundaryProvider: Send + Sync {
    /// Identifier (`world`, `vn-provinces`, …).
    fn id(&self) -> &str;
    /// What the regions are.
    fn level(&self) -> MapLevel;
    /// ISO 3166-1 alpha-2 code of the country a subdivision set covers (`None` for the world).
    fn country(&self) -> Option<&str>;
    /// The regions.
    fn regions(&self) -> &[Region];
    /// Index of the region a data label names (by code, name or alias).
    fn find(&self, name: &str) -> Option<usize>;
}

/// A boundary set with a name index (the bundled world and the loaded datasets).
#[derive(Debug, Default)]
pub struct Boundaries {
    /// Identifier.
    pub id: String,
    /// Level of the regions.
    pub level: Option<MapLevel>,
    /// Country of a subdivision set (ISO alpha-2).
    pub country: Option<String>,
    /// Regions.
    pub regions: Vec<Region>,
    index: HashMap<String, usize>,
}

impl Boundaries {
    /// A boundary set; `aliases` maps other (old) names to a current name or code of a region.
    #[must_use]
    pub fn new(
        id: &str,
        level: MapLevel,
        country: Option<String>,
        regions: Vec<Region>,
        aliases: &[(String, String)],
    ) -> Boundaries {
        let mut index = HashMap::new();
        for (i, r) in regions.iter().enumerate() {
            for c in &r.codes {
                if !c.is_empty() {
                    index.entry(format!("#{}", c.to_uppercase())).or_insert(i);
                }
            }
            for n in &r.names {
                for k in name_keys(n) {
                    index.entry(k).or_insert(i);
                }
            }
        }
        let mut b = Boundaries {
            id: id.to_owned(),
            level: Some(level),
            country,
            regions,
            index,
        };
        for (old, new) in aliases {
            if let Some(i) = b.find(new) {
                for k in name_keys(old) {
                    b.index.entry(k).or_insert(i);
                }
            }
        }
        b
    }
}

impl BoundaryProvider for Boundaries {
    fn id(&self) -> &str {
        &self.id
    }
    fn level(&self) -> MapLevel {
        self.level.unwrap_or(MapLevel::Country)
    }
    fn country(&self) -> Option<&str> {
        self.country.as_deref()
    }
    fn regions(&self) -> &[Region] {
        &self.regions
    }
    fn find(&self, name: &str) -> Option<usize> {
        let t = name.trim();
        if t.is_empty() {
            return None;
        }
        if let Some(i) = self.index.get(&format!("#{}", t.to_uppercase())) {
            return Some(*i);
        }
        name_keys(t).iter().find_map(|k| self.index.get(k).copied())
    }
}

/// `text` folded for matching: lower case, without diacritics (`đ` → `d`), letters and digits only.
#[must_use]
pub fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'đ' => 'd',
            'ø' | 'œ' => 'o',
            'ł' => 'l',
            'æ' => 'a',
            'ß' => 's',
            _ => c,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Prefixes of administrative units dropped for matching ("Tỉnh Lào Cai" = "Lào Cai").
const UNIT_PREFIXES: [&str; 7] = [
    "thanhpho",
    "tinh",
    "tp",
    "province",
    "provinceof",
    "republicof",
    "the",
];

/// Lookup keys of a name: the folded name and, without a unit prefix, the rest.
#[must_use]
pub fn name_keys(name: &str) -> Vec<String> {
    let f = fold(name);
    if f.is_empty() {
        return Vec::new();
    }
    let mut out = vec![f.clone()];
    // Longest prefix first ("thanhpho" before "tp").
    let mut prefixes = UNIT_PREFIXES;
    prefixes.sort_by_key(|p| std::cmp::Reverse(p.len()));
    for p in prefixes {
        if let Some(rest) = f.strip_prefix(p) {
            if rest.chars().count() >= 2 && !out.iter().any(|k| k == rest) {
                out.push(rest.to_owned());
            }
        }
    }
    out
}

static REGISTRY: RwLock<Vec<Arc<dyn BoundaryProvider>>> = RwLock::new(Vec::new());

/// Registers a boundary set (replacing one with the same id).
pub fn register(p: Arc<dyn BoundaryProvider>) {
    if let Ok(mut r) = REGISTRY.write() {
        r.retain(|q| q.id() != p.id());
        r.push(p);
    }
}

/// Removes a registered boundary set.
pub fn unregister(id: &str) {
    if let Ok(mut r) = REGISTRY.write() {
        r.retain(|q| q.id() != id);
    }
}

/// The registered boundary sets (the bundled world is not among them, see [`world::world`]).
#[must_use]
pub fn registered() -> Vec<Arc<dyn BoundaryProvider>> {
    REGISTRY.read().map(|r| r.clone()).unwrap_or_default()
}

/// The boundary set for a map of `names` at `level`: the world for countries; for subdivisions the
/// registered set of that level matching the most names (`None` when no registered set matches).
#[must_use]
pub fn provider_for(level: MapLevel, names: &[String]) -> Option<Arc<dyn BoundaryProvider>> {
    match level {
        MapLevel::Country => Some(world::world()),
        MapLevel::Province => registered()
            .into_iter()
            .filter(|p| p.level() == MapLevel::Province)
            .map(|p| {
                let hits = names.iter().filter(|n| p.find(n).is_some()).count();
                (hits, p)
            })
            .filter(|(hits, _)| *hits > 0)
            // The first registered set wins a tie.
            .reduce(|best, x| if x.0 > best.0 { x } else { best })
            .map(|(_, p)| p),
    }
}

/// The level a new map of these location names gets: countries when at least half of the names
/// are countries of the world, else subdivisions (provinces).
#[must_use]
pub fn guess_level(names: &[String]) -> MapLevel {
    let w = world::world();
    let named: Vec<&String> = names.iter().filter(|n| !n.trim().is_empty()).collect();
    if named.is_empty() {
        return MapLevel::Country;
    }
    let hits = named.iter().filter(|n| w.find(n).is_some()).count();
    if hits * 2 >= named.len() {
        MapLevel::Country
    } else {
        MapLevel::Province
    }
}

/// The message shown when a province-level map has no boundary dataset (Vietnamese or English).
#[must_use]
pub fn needs_dataset_message(vi: bool) -> &'static str {
    if vi {
        "Bản đồ cấp tỉnh cần bộ dữ liệu ranh giới tỉnh/thành được phê duyệt (assets/maps/vn-provinces)."
    } else {
        "Province maps need an approved province boundary dataset (assets/maps/vn-provinces)."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Vec<LonLat> {
        vec![(x, y), (x + s, y), (x + s, y + s), (x, y + s)]
    }

    #[test]
    fn fold_drops_case_diacritics_and_punctuation() {
        assert_eq!(fold("Việt Nam"), "vietnam");
        assert_eq!(fold("VIET NAM"), "vietnam");
        assert_eq!(fold("Đà Nẵng"), "danang");
        assert_eq!(fold("Bà Rịa – Vũng Tàu"), "bariavungtau");
        assert_eq!(fold("Côte d'Ivoire"), "cotedivoire");
    }

    #[test]
    fn name_keys_strip_unit_prefixes() {
        assert_eq!(name_keys("Tỉnh Lào Cai"), vec!["tinhlaocai", "laocai"]);
        assert_eq!(name_keys("Thành phố Huế"), vec!["thanhphohue", "hue"]);
        assert_eq!(
            name_keys("TP. Hồ Chí Minh"),
            vec!["tphochiminh", "hochiminh"]
        );
    }

    #[test]
    fn boundaries_match_codes_names_and_aliases() {
        let regions = vec![
            Region {
                codes: vec!["A1".into()],
                names: vec!["Ô Vuông Một".into(), "Square One".into()],
                rings: vec![square(0.0, 0.0, 1.0)],
            },
            Region {
                codes: vec!["B2".into()],
                names: vec!["Ô Vuông Hai".into()],
                rings: vec![square(2.0, 0.0, 1.0)],
            },
        ];
        let b = Boundaries::new(
            "test",
            MapLevel::Province,
            Some("ZZ".into()),
            regions,
            &[("Ô Cũ".into(), "Ô Vuông Hai".into())],
        );
        assert_eq!(b.find("o vuong mot"), Some(0));
        assert_eq!(b.find("SQUARE ONE"), Some(0));
        assert_eq!(b.find("a1"), Some(0));
        assert_eq!(b.find("Tỉnh Ô Vuông Hai"), Some(1));
        assert_eq!(b.find("o cu"), Some(1));
        assert_eq!(b.find("Ô Cũ"), Some(1));
        assert_eq!(b.find("Ô Ba"), None);
        assert_eq!(b.find(""), None);
    }

    #[test]
    fn region_geometry() {
        let r = Region {
            rings: vec![square(0.0, 0.0, 1.0), square(5.0, 5.0, 3.0)],
            ..Region::default()
        };
        assert_eq!(r.bbox(), Some([0.0, 0.0, 8.0, 8.0]));
        assert_eq!(ring_area(r.main_ring().unwrap()), 9.0);
    }
}
