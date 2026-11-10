//! Region map charts (CORE-CHART-006): boundary datasets loaded from a folder (synthetic squares,
//! clearly not real geography), province maps with and without a dataset, and the display list of a
//! world map with five countries.

use bo_model::sheet::{Cell, CellRange, CellValue, Chart, ChartKind, Workbook};
use bo_render::{Command, Paint, Rgba};
use bo_sheet::chart::{draw_chart, new_chart};
use bo_sheet::geo::{self, dataset, BoundaryProvider, MapLevel};
use bo_sheet::numfmt::Locale;
use std::path::PathBuf;
use std::sync::Arc;

fn data_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/maps")
        .join(name)
}

fn book(rows: &[(&str, CellValue)]) -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    ws.set_cell(0, 0, Cell::new(CellValue::Text("Nơi".into())));
    ws.set_cell(0, 1, Cell::new(CellValue::Text("Giá trị".into())));
    for (i, (n, v)) in rows.iter().enumerate() {
        ws.set_cell(i as u32 + 1, 0, Cell::new(CellValue::Text((*n).into())));
        ws.set_cell(i as u32 + 1, 1, Cell::new(v.clone()));
    }
    wb
}

fn map_of(wb: &Workbook, rows: usize) -> Chart {
    let range = CellRange::parse(&format!("A1:B{}", rows + 1)).expect("range");
    new_chart(wb, 0, range, "regionMap").expect("a map chart")
}

type Label = (String, [f64; 4]);

fn draw(wb: &Workbook, c: &Chart, locale: &Locale) -> (Vec<Command>, Vec<Label>) {
    let mut labels = Vec::new();
    let mut out = Vec::new();
    draw_chart(
        wb,
        0,
        c,
        360.0,
        240.0,
        locale,
        &mut |t, r, _, _, _, _| {
            let r = r.map(|v| (v * 100.0).round() / 100.0);
            labels.push((t.to_owned(), r));
        },
        &mut out,
    );
    (out, labels)
}

fn fills(out: &[Command]) -> Vec<Rgba> {
    out.iter()
        .filter_map(|c| match c {
            Command::Path {
                fill: Some(Paint::Solid { color }),
                ..
            } => Some(*color),
            _ => None,
        })
        .collect()
}

#[test]
fn geojson_dataset_loads_with_names_codes_and_aliases() {
    let b = dataset::load_dir(&data_dir("synthetic-squares"))
        .unwrap()
        .expect("dataset.json");
    assert_eq!(b.level(), MapLevel::Province);
    assert_eq!(b.country(), Some("ZZ"));
    assert_eq!(b.regions().len(), 2);
    assert_eq!(b.regions()[0].names, vec!["Ô Vuông Một", "Square One"]);
    assert_eq!(b.regions()[1].bbox(), Some([2.0, 0.0, 3.0, 1.0]));
    // With or without diacritics, a unit prefix, by code and by alias (old name → new name / code).
    assert_eq!(b.find("O VUONG MOT"), Some(0));
    assert_eq!(b.find("Tỉnh Ô Vuông Một"), Some(0));
    assert_eq!(b.find("sq2"), Some(1));
    assert_eq!(b.find("Ô Cũ"), Some(1));
    assert_eq!(b.find("o cu"), Some(1));
    assert_eq!(b.find("Old Square"), Some(0));
    assert_eq!(b.find("Hà Giang"), None);
    // A folder without dataset.json is not a dataset.
    assert!(dataset::load_dir(&data_dir("none")).unwrap().is_none());
}

#[test]
fn topojson_dataset_decodes_shared_arcs() {
    let b = dataset::load_dir(&data_dir("synthetic-topo"))
        .unwrap()
        .expect("dataset.json");
    assert_eq!(b.regions().len(), 2);
    // Quantised arcs × 0.5 + (−30, −20); the shared edge is reversed for the eastern square.
    assert_eq!(
        b.regions()[0].rings[0],
        vec![
            (-29.0, -20.0),
            (-29.0, -19.0),
            (-30.0, -19.0),
            (-30.0, -20.0),
            (-29.0, -20.0)
        ]
    );
    assert_eq!(
        b.regions()[1].rings[0],
        vec![
            (-29.0, -19.0),
            (-29.0, -20.0),
            (-28.0, -20.0),
            (-28.0, -19.0),
            (-29.0, -19.0)
        ]
    );
    assert_eq!(b.find("khoi dong"), Some(1));
    assert_eq!(b.find("WEST"), Some(0));
}

#[test]
fn manifest_errors_are_reported() {
    let m = dataset::Manifest::parse(
        r#"{"format":"bo-map-dataset/1","id":"x","level":"state","file":"a.geojson","nameProperties":["n"],"expectedCount":3}"#,
    )
    .unwrap();
    let gj =
        std::fs::read_to_string(data_dir("synthetic-squares").join("squares.geojson")).unwrap();
    let err = dataset::build(&m, &gj, None).unwrap_err();
    assert!(err.contains("expects 3"), "{err}");
    assert!(dataset::Manifest::parse(r#"{"format":"other"}"#).is_err());
    assert!(dataset::Manifest::parse(
        r#"{"format":"bo-map-dataset/1","id":"x","file":"../x.json","nameProperties":["n"]}"#
    )
    .is_err());
    assert!(dataset::build(&m, "[1,2]", None).is_err());
}

#[test]
fn province_map_draws_from_a_registered_dataset() {
    let b = dataset::load_dir(&data_dir("synthetic-squares"))
        .unwrap()
        .unwrap();
    geo::register(Arc::new(b));
    let wb = book(&[
        ("Tỉnh Ô Vuông Một", CellValue::Number(10.0)),
        ("o vuong hai", CellValue::Number(30.0)),
    ]);
    let c = map_of(&wb, 2);
    assert_eq!(c.plots[0].kind, ChartKind::RegionMap);
    let m = c.map.as_ref().unwrap();
    assert_eq!(
        m.level.as_deref(),
        Some("state"),
        "not countries: provinces"
    );
    assert!(!m.categorical);
    let (out, labels) = draw(&wb, &c, &Locale::VI_VN);
    // Two squares: lowest value in the scale's light colour, highest in its dark colour.
    let f = fills(&out);
    assert_eq!(
        f,
        vec![Rgba(0xDC, 0xE9, 0xF5, 255), Rgba(0x1F, 0x4E, 0x89, 255)]
    );
    // Legend: highest and lowest value; labels inside the regions (they fit).
    let texts: Vec<&str> = labels.iter().map(|(t, _)| t.as_str()).collect();
    assert!(texts.contains(&"30") && texts.contains(&"10"), "{texts:?}");
    assert!(texts.contains(&"o vuong hai"), "{texts:?}");
    // The old name still finds the region.
    let wb = book(&[("Ô Cũ", CellValue::Number(5.0))]);
    let c = map_of(&wb, 1);
    assert_eq!(fills(&draw(&wb, &c, &Locale::VI_VN).0).len(), 2);
}

#[test]
fn province_map_without_a_dataset_says_so() {
    let wb = book(&[
        ("Hà Giang", CellValue::Number(1.0)),
        ("Cao Bằng", CellValue::Number(2.0)),
    ]);
    let c = map_of(&wb, 2);
    assert_eq!(c.map.as_ref().unwrap().level.as_deref(), Some("state"));
    let (out, labels) = draw(&wb, &c, &Locale::VI_VN);
    assert_eq!(fills(&out), Vec::<Rgba>::new());
    let msg = geo::needs_dataset_message(true);
    let (first, rest) = msg.split_once(" (").unwrap();
    assert_eq!(labels[0].0, first);
    assert_eq!(labels[1].0, format!("({rest}"));
    let (_, en) = draw(&wb, &c, &Locale::EN_US);
    assert!(en[0].0.starts_with("Province maps need"));
}

#[test]
fn categorical_country_map_colours_each_category() {
    let text = |s: &str| CellValue::Text(s.into());
    let wb = book(&[
        ("Việt Nam", text("ASEAN")),
        ("Thái Lan", text("ASEAN")),
        ("Japan", text("East Asia")),
    ]);
    let c = map_of(&wb, 3);
    let m = c.map.as_ref().unwrap();
    assert_eq!(m.level.as_deref(), Some("countryRegion"));
    assert!(m.categorical);
    assert_eq!(m.text_values, vec!["ASEAN", "ASEAN", "East Asia"]);
    let (out, labels) = draw(&wb, &c, &Locale::EN_US);
    let palette = bo_sheet::chart::PALETTE;
    let f = fills(&out);
    assert_eq!(f.iter().filter(|c| **c == palette[0]).count(), 2);
    assert_eq!(f.iter().filter(|c| **c == palette[1]).count(), 1);
    let texts: Vec<&str> = labels.iter().map(|(t, _)| t.as_str()).collect();
    assert!(texts.contains(&"ASEAN") && texts.contains(&"East Asia"));
}

#[test]
fn world_map_of_five_countries_display_list() {
    let wb = book(&[
        ("Việt Nam", CellValue::Number(98.2)),
        ("Thailand", CellValue::Number(71.7)),
        ("Lào", CellValue::Number(7.6)),
        ("Cambodia", CellValue::Number(17.4)),
        ("Malaysia", CellValue::Number(34.3)),
    ]);
    let mut c = map_of(&wb, 5);
    c.title = Some("Dân số (triệu)".into());
    assert_eq!(
        c.map.as_ref().unwrap().level.as_deref(),
        Some("countryRegion")
    );
    let (out, labels) = draw(&wb, &c, &Locale::VI_VN);
    // Five data countries in colours of the scale; the neighbours in the no-data grey.
    let grey = Rgba(0xE4, 0xE7, 0xEB, 255);
    let f = fills(&out);
    assert_eq!(f.iter().filter(|c| **c != grey).count(), 5, "{f:?}");
    assert!(
        f.contains(&Rgba(0x1F, 0x4E, 0x89, 255)),
        "Việt Nam has the highest value"
    );
    // One compact JSON line per command, then the labels.
    let mut lines: Vec<String> = out
        .iter()
        .map(|c| serde_json::to_string(c).unwrap())
        .collect();
    lines.extend(labels.iter().map(|l| serde_json::to_string(l).unwrap()));
    insta::assert_snapshot!("world_map_five_countries", lines.join("\n"));
    // Other projections draw the same regions.
    for p in ["mercator", "miller", "robinson", "albers"] {
        let mut c = c.clone();
        c.map.as_mut().unwrap().projection = Some(p.into());
        let f = fills(&draw(&wb, &c, &Locale::VI_VN).0);
        assert_eq!(f.iter().filter(|c| **c != grey).count(), 5, "{p}");
    }
}
