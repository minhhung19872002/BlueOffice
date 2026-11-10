//! Boundary datasets loaded at run time (CORE-CHART-006): a folder such as
//! `assets/maps/vn-provinces/` holding
//!
//! * `dataset.json` — the manifest ([`Manifest`]): `format` `"bo-map-dataset/1"`, `id`, `level`
//!   (`"state"` for provinces, `"country"`), `country` (ISO alpha-2), `file` (the boundaries),
//!   `object` (TopoJSON object name, optional), `nameProperties` (feature properties holding names,
//!   the first is the display name), `codeProperty` (optional), `aliases` (optional file name),
//!   `expectedCount` (optional: the number of regions the file must hold), and its `source`,
//!   `version` and `license`;
//! * the boundaries as GeoJSON (RFC 7946: a `FeatureCollection` of `Polygon` / `MultiPolygon`
//!   features) or TopoJSON (a `Topology` with quantised, delta-encoded arcs);
//! * the aliases: a JSON object mapping other names (former units, other spellings) to a current
//!   name or code, e.g. `{ "Hà Tây": "Hà Nội" }`.
//!
//! Nothing is bundled: a host reads the files and calls [`build`] (or [`load_dir`] natively), then
//! [`super::register`]s the result.

use super::{Boundaries, LonLat, MapLevel, Region};
use serde::Deserialize;
use serde_json::Value;

/// Format tag of a dataset manifest.
pub const FORMAT: &str = "bo-map-dataset/1";

/// Limits against oversized files.
const MAX_REGIONS: usize = 20_000;
const MAX_POINTS: usize = 5_000_000;

/// A dataset manifest (`dataset.json`).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    /// `bo-map-dataset/1`.
    pub format: String,
    /// Identifier (registered under it).
    pub id: String,
    /// `state` (subdivisions) or `country`.
    pub level: String,
    /// ISO 3166-1 alpha-2 country of a subdivision set.
    pub country: Option<String>,
    /// Boundary file (GeoJSON or TopoJSON) in the folder.
    pub file: String,
    /// TopoJSON object to read (default: the first).
    pub object: Option<String>,
    /// Feature properties holding names; the first is the display name.
    pub name_properties: Vec<String>,
    /// Feature property holding a code.
    pub code_property: Option<String>,
    /// Alias file in the folder.
    pub aliases: Option<String>,
    /// Number of regions the file must hold.
    pub expected_count: Option<usize>,
    /// Where the data comes from.
    pub source: String,
    /// Version of the data.
    pub version: String,
    /// Licence of the data.
    pub license: String,
}

impl Manifest {
    /// Reads `dataset.json`.
    pub fn parse(json: &str) -> Result<Manifest, String> {
        let m: Manifest = serde_json::from_str(json).map_err(|e| format!("dataset.json: {e}"))?;
        if m.format != FORMAT {
            return Err(format!("dataset.json: format must be \"{FORMAT}\""));
        }
        if m.id.trim().is_empty() || m.file.trim().is_empty() {
            return Err("dataset.json: id and file are required".into());
        }
        if m.name_properties.is_empty() {
            return Err("dataset.json: nameProperties is empty".into());
        }
        // Only a plain file name: the files stay inside the dataset folder.
        for f in std::iter::once(&m.file).chain(m.aliases.iter()) {
            if f.contains(['/', '\\']) || f.contains("..") {
                return Err(format!("dataset.json: '{f}' must be a file of the folder"));
            }
        }
        Ok(m)
    }

    /// The level of the regions.
    #[must_use]
    pub fn map_level(&self) -> MapLevel {
        if self.level == "country" {
            MapLevel::Country
        } else {
            MapLevel::Province
        }
    }
}

fn prop_text(props: Option<&Value>, key: &str) -> Option<String> {
    match props?.get(key)? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn point(v: &Value) -> Option<LonLat> {
    let a = v.as_array()?;
    let x = a.first()?.as_f64()?;
    let y = a.get(1)?.as_f64()?;
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

fn ring(v: &Value, budget: &mut usize) -> Result<Vec<LonLat>, String> {
    let a = v.as_array().ok_or("a ring is not an array")?;
    if a.len() > *budget {
        return Err("too many points".into());
    }
    *budget -= a.len();
    a.iter()
        .map(|p| point(p).ok_or_else(|| "bad coordinate".to_owned()))
        .collect()
}

/// Rings of a GeoJSON `Polygon` / `MultiPolygon` geometry (RFC 7946 §3.1.6–3.1.7).
fn geojson_rings(g: &Value, budget: &mut usize) -> Result<Vec<Vec<LonLat>>, String> {
    let coords = g.get("coordinates");
    let mut out = Vec::new();
    match g.get("type").and_then(Value::as_str) {
        Some("Polygon") => {
            for r in coords
                .and_then(Value::as_array)
                .ok_or("Polygon without coordinates")?
            {
                out.push(ring(r, budget)?);
            }
        }
        Some("MultiPolygon") => {
            for poly in coords
                .and_then(Value::as_array)
                .ok_or("MultiPolygon without coordinates")?
            {
                for r in poly.as_array().ok_or("bad MultiPolygon")? {
                    out.push(ring(r, budget)?);
                }
            }
        }
        Some("GeometryCollection") => {
            for sub in g
                .get("geometries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                out.extend(geojson_rings(sub, budget)?);
            }
        }
        _ => {}
    }
    Ok(out)
}

fn region_of(props: Option<&Value>, rings: Vec<Vec<LonLat>>, m: &Manifest) -> Region {
    let mut names: Vec<String> = Vec::new();
    for k in &m.name_properties {
        if let Some(n) = prop_text(props, k) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    let codes = m
        .code_property
        .as_deref()
        .and_then(|k| prop_text(props, k))
        .into_iter()
        .collect();
    Region {
        codes,
        names,
        rings,
    }
}

/// Regions of a GeoJSON `FeatureCollection`.
pub fn parse_geojson(v: &Value, m: &Manifest) -> Result<Vec<Region>, String> {
    let features = v
        .get("features")
        .and_then(Value::as_array)
        .ok_or("GeoJSON: no features")?;
    if features.len() > MAX_REGIONS {
        return Err("GeoJSON: too many features".into());
    }
    let mut budget = MAX_POINTS;
    let mut out = Vec::new();
    for f in features {
        let Some(g) = f.get("geometry").filter(|g| !g.is_null()) else {
            continue;
        };
        let rings = geojson_rings(g, &mut budget).map_err(|e| format!("GeoJSON: {e}"))?;
        out.push(region_of(f.get("properties"), rings, m));
    }
    Ok(out)
}

/// Regions of a TopoJSON `Topology` (object `m.object`, else the first one).
pub fn parse_topojson(v: &Value, m: &Manifest) -> Result<Vec<Region>, String> {
    let objects = v
        .get("objects")
        .and_then(Value::as_object)
        .ok_or("TopoJSON: no objects")?;
    let obj = match &m.object {
        Some(name) => objects.get(name),
        None => objects.values().next(),
    }
    .ok_or("TopoJSON: object not found")?;
    // Transform: quantised, delta-encoded positions.
    let tr = v.get("transform");
    let pair = |k: &str| -> Option<(f64, f64)> { point(tr?.get(k)?) };
    let (scale, translate) = match (pair("scale"), pair("translate")) {
        (Some(s), Some(t)) => (Some(s), t),
        _ => (None, (0.0, 0.0)),
    };
    let mut budget = MAX_POINTS;
    let mut arcs: Vec<Vec<LonLat>> = Vec::new();
    for a in v
        .get("arcs")
        .and_then(Value::as_array)
        .ok_or("TopoJSON: no arcs")?
    {
        let pts = a.as_array().ok_or("TopoJSON: bad arc")?;
        if pts.len() > budget {
            return Err("TopoJSON: too many points".into());
        }
        budget -= pts.len();
        let (mut x, mut y) = (0.0, 0.0);
        let mut arc = Vec::with_capacity(pts.len());
        for p in pts {
            let (px, py) = point(p).ok_or("TopoJSON: bad position")?;
            arc.push(match scale {
                Some((sx, sy)) => {
                    x += px;
                    y += py;
                    (x * sx + translate.0, y * sy + translate.1)
                }
                None => (px, py),
            });
        }
        arcs.push(arc);
    }
    // A ring from arc indices (a negative index `i` is arc `!i` reversed).
    let stitch = |idx: &Value| -> Result<Vec<LonLat>, String> {
        let mut out: Vec<LonLat> = Vec::new();
        for i in idx.as_array().ok_or("TopoJSON: bad ring")? {
            let i = i.as_i64().ok_or("TopoJSON: bad arc index")?;
            let (k, rev) = if i < 0 {
                ((!i) as usize, true)
            } else {
                (i as usize, false)
            };
            let arc = arcs.get(k).ok_or("TopoJSON: arc index out of range")?;
            let mut pts: Vec<LonLat> = arc.clone();
            if rev {
                pts.reverse();
            }
            let skip = usize::from(!out.is_empty());
            out.extend(pts.into_iter().skip(skip));
        }
        Ok(out)
    };
    let geometries: Vec<&Value> = match obj.get("type").and_then(Value::as_str) {
        Some("GeometryCollection") => obj
            .get("geometries")
            .and_then(Value::as_array)
            .map(|g| g.iter().collect())
            .unwrap_or_default(),
        _ => vec![obj],
    };
    if geometries.len() > MAX_REGIONS {
        return Err("TopoJSON: too many geometries".into());
    }
    let mut out = Vec::new();
    for g in geometries {
        let mut rings = Vec::new();
        match g.get("type").and_then(Value::as_str) {
            Some("Polygon") => {
                for r in g
                    .get("arcs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    rings.push(stitch(r)?);
                }
            }
            Some("MultiPolygon") => {
                for poly in g
                    .get("arcs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    for r in poly.as_array().into_iter().flatten() {
                        rings.push(stitch(r)?);
                    }
                }
            }
            _ => continue,
        }
        out.push(region_of(g.get("properties"), rings, m));
    }
    Ok(out)
}

/// Aliases of an alias file: `{ "other name": "current name or code" }`.
pub fn parse_aliases(json: &str) -> Result<Vec<(String, String)>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("aliases: {e}"))?;
    let o = v.as_object().ok_or("aliases: not an object")?;
    Ok(o.iter()
        .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
        .collect())
}

/// A boundary set from a manifest, the boundary file's text and the alias file's text.
pub fn build(m: &Manifest, boundaries: &str, aliases: Option<&str>) -> Result<Boundaries, String> {
    let v: Value = serde_json::from_str(boundaries).map_err(|e| format!("{}: {e}", m.file))?;
    let regions = match v.get("type").and_then(Value::as_str) {
        Some("Topology") => parse_topojson(&v, m)?,
        Some("FeatureCollection") => parse_geojson(&v, m)?,
        _ => return Err(format!("{}: neither GeoJSON nor TopoJSON", m.file)),
    };
    let regions: Vec<Region> = regions
        .into_iter()
        .filter(|r| !r.names.is_empty())
        .collect();
    if let Some(n) = m.expected_count {
        if regions.len() != n {
            return Err(format!(
                "{}: {} named regions, the manifest expects {n}",
                m.file,
                regions.len()
            ));
        }
    }
    let aliases = match aliases {
        Some(a) => parse_aliases(a)?,
        None => Vec::new(),
    };
    Ok(Boundaries::new(
        &m.id,
        m.map_level(),
        m.country.clone(),
        regions,
        &aliases,
    ))
}

/// Loads a dataset folder (native builds). `Ok(None)` when the folder has no `dataset.json`.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_dir(dir: &std::path::Path) -> Result<Option<Boundaries>, String> {
    let manifest = dir.join("dataset.json");
    if !manifest.is_file() {
        return Ok(None);
    }
    let read =
        |name: &str| std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"));
    let m = Manifest::parse(&read("dataset.json")?)?;
    let boundaries = read(&m.file)?;
    let aliases = match &m.aliases {
        Some(a) => Some(read(a)?),
        None => None,
    };
    build(&m, &boundaries, aliases.as_deref()).map(Some)
}

/// Registers every dataset folder under `root` (e.g. `assets/maps`) that has a `dataset.json`;
/// returns the ids registered and the errors met.
#[cfg(not(target_arch = "wasm32"))]
pub fn register_dir(root: &std::path::Path) -> (Vec<String>, Vec<String>) {
    let (mut ok, mut errors) = (Vec::new(), Vec::new());
    let Ok(entries) = std::fs::read_dir(root) else {
        return (ok, errors);
    };
    let mut dirs: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for d in dirs {
        match load_dir(&d) {
            Ok(Some(b)) => {
                ok.push(b.id.clone());
                super::register(std::sync::Arc::new(b));
            }
            Ok(None) => {}
            Err(e) => errors.push(format!("{}: {e}", d.display())),
        }
    }
    (ok, errors)
}
