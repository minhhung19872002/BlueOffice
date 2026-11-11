//! Timelines ([MS-XLSX] timeline parts) over the corpus pivot table with a date field: the
//! parts are added by hand (x15 timelineCacheDefinition with a selection, a timelines part and
//! the relationships), read into the model, filtered, saved and read back.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::PivotFilterType;
use bo_opc::{Limits, Package, Relationship, TargetMode};
use bo_sheet::pivot::PivotLabels;
use bo_xlsx::{read_xlsx, write_xlsx};

const X15: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";

fn with_timeline() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet/writer-pivot-dates-1.xlsx");
    let bytes = std::fs::read(path).unwrap();
    let wb = read_xlsx(&bytes).unwrap();
    let pivot = wb.sheets[1].pivots[0].name.clone();
    let mut pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let cache = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<timelineCacheDefinition xmlns="{X15}" name="NativeTimeline_Ngay" sourceName="Ngày"><pivotTables><pivotTable tabId="2" name="{pivot}"/></pivotTables><state minimalRefreshVersion="6" lastRefreshVersion="6" pivotCacheId="1" filterType="dateBetween"><selection startDate="2023-04-01T00:00:00" endDate="2023-06-30T00:00:00"/><bounds startDate="2023-01-01T00:00:00" endDate="2025-01-01T00:00:00"/></state></timelineCacheDefinition>"#
    );
    pkg.add_part(
        "/xl/timelineCaches/timelineCache1.xml",
        "application/vnd.ms-excel.timelineCache+xml",
        cache.into_bytes(),
    )
    .unwrap();
    let timelines = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<timelines xmlns="{X15}"><timeline name="Ngày" cache="NativeTimeline_Ngay" caption="Ngày" level="1" selectionLevel="1"/></timelines>"#
    );
    pkg.add_part(
        "/xl/timelines/timeline1.xml",
        "application/vnd.ms-excel.timeline+xml",
        timelines.into_bytes(),
    )
    .unwrap();
    let mut wrels = pkg.relationships("/xl/workbook.xml").unwrap();
    wrels.push(Relationship {
        id: "rId90".into(),
        rel_type: bo_xlsx::timeline::REL_TIMELINE_CACHE.into(),
        target: "timelineCaches/timelineCache1.xml".into(),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships("/xl/workbook.xml", &wrels);
    let mut srels = pkg.relationships("/xl/worksheets/sheet2.xml").unwrap();
    srels.push(Relationship {
        id: "rId90".into(),
        rel_type: bo_xlsx::timeline::REL_TIMELINE.into(),
        target: "../timelines/timeline1.xml".into(),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships("/xl/worksheets/sheet2.xml", &srels);
    pkg.write().unwrap()
}

#[test]
fn timeline_reads_filters_and_saves() {
    let mut wb = read_xlsx(&with_timeline()).unwrap();
    let t = &wb.sheets[1].timelines;
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].caption, "Ngày");
    assert_eq!(t[0].field, "Ngày");
    assert_eq!(t[0].level, 1);
    assert_eq!(t[0].start.as_deref(), Some("2023-04-01T00:00:00"));
    // Quarters from the first to the last date (45000 = 2023-03-15 … 45667 = 2025-01-10):
    // only Q2 2023 is inside the stored selection.
    let p = bo_sheet::timeline::periods(&wb, 1, 0, true);
    assert_eq!(p.first().unwrap().label, "Quý 1 2023");
    assert_eq!(p.last().unwrap().label, "Quý 1 2025");
    let chosen: Vec<&str> = p
        .iter()
        .filter(|x| x.selected)
        .map(|x| x.label.as_str())
        .collect();
    assert_eq!(chosen, ["Quý 2 2023"]);
    // Filter to 2024: the pivot table gets a dateBetween filter on the field.
    let y2024: Vec<_> = p.iter().filter(|x| x.label.ends_with("2024")).collect();
    let range = (y2024.first().unwrap().start, y2024.last().unwrap().end);
    bo_sheet::timeline::select(&mut wb, 1, 0, Some(range), &PivotLabels::default()).unwrap();
    let f = &wb.sheets[1].pivots[0].field_filters;
    assert!(f.iter().any(|f| f.kind == PivotFilterType::DateBetween));
    assert_eq!(
        wb.sheets[1].timelines[0].start.as_deref(),
        Some("2024-01-01T00:00:00")
    );
    assert_eq!(
        wb.sheets[1].timelines[0].end.as_deref(),
        Some("2024-12-31T00:00:00")
    );
    // Saved: the cache's selection is rewritten and read back.
    let saved = write_xlsx(&wb).unwrap();
    if let Ok(dir) = std::env::var("BO_SLICER_OUT") {
        std::fs::write(format!("{dir}/timeline.xlsx"), &saved).unwrap();
    }
    let back = read_xlsx(&saved).unwrap();
    let t = &back.sheets[1].timelines[0];
    assert_eq!(t.start.as_deref(), Some("2024-01-01T00:00:00"));
    // Clearing removes the filter and the selection.
    let mut back = back;
    bo_sheet::timeline::select(&mut back, 1, 0, None, &PivotLabels::default()).unwrap();
    assert!(back.sheets[1].pivots[0]
        .field_filters
        .iter()
        .all(|f| f.kind != PivotFilterType::DateBetween));
    let again = read_xlsx(&write_xlsx(&back).unwrap()).unwrap();
    assert_eq!(again.sheets[1].timelines[0].start, None);
}

fn part_text(bytes: &[u8], name: &str) -> Option<String> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.part(name)
        .map(|p| String::from_utf8(p.data.clone()).unwrap())
}

fn part_names(bytes: &[u8]) -> Vec<String> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.parts().iter().map(|p| p.name.clone()).collect()
}

/// A timeline inserted over the date field of the corpus pivot table: written as a timeline
/// cache (pivot table, pivot cache id, bounds), a timelines part and a `tsle:timeslicer`
/// frame; the selection, the level and the deletion survive saving.
#[test]
fn timeline_is_created_saved_edited_and_deleted() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet/writer-pivot-dates-1.xlsx");
    let mut wb = read_xlsx(&std::fs::read(path).unwrap()).unwrap();
    let labels = PivotLabels::default();
    let pivot = wb.sheets[1].pivots[0].clone();
    let cache_part = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == pivot.cache_id)
        .unwrap()
        .part
        .clone();
    assert_eq!(bo_sheet::timeline::date_fields(&wb, 1, 0), [2]);
    let i = bo_sheet::timeline::insert_timeline(
        &mut wb,
        1,
        1,
        0,
        2,
        bo_model::sheet::CellRef::new(1, 4),
        &labels,
    )
    .unwrap();
    let bytes = write_xlsx(&wb).unwrap();
    // The pivot cache gets both extensions; the timeline cache names it and the pivot table.
    let def = part_text(&bytes, &cache_part).unwrap();
    assert!(
        def.contains(r#"<x14:pivotCacheDefinition pivotCacheId="1"/>"#),
        "{def}"
    );
    assert!(
        def.contains(r#"<x15:timelinePivotCacheDefinition timelineData="1"/>"#),
        "{def}"
    );
    let names = part_names(&bytes);
    let cache = names
        .iter()
        .find(|n| n.starts_with("/xl/timelineCaches/"))
        .unwrap();
    let xml = part_text(&bytes, cache).unwrap();
    let tab = wb.sheets[1].sheet_id;
    // The dates run from 2023-03-15 (45000) in steps of 23 days to 2025-01-10 (45667): the
    // bounds are the first day and the day after the last.
    assert!(
        xml.contains(&format!(
            r#"name="NativeTimeline_Ngày" sourceName="Ngày"><pivotTables><pivotTable tabId="{tab}" name="{}"/></pivotTables><state minimalRefreshVersion="6" lastRefreshVersion="6" pivotCacheId="1" filterType="unknown"><bounds startDate="2023-03-15T00:00:00" endDate="2025-01-11T00:00:00"/></state>"#,
            pivot.name
        )),
        "{xml}"
    );
    let workbook = part_text(&bytes, "/xl/workbook.xml").unwrap();
    assert!(workbook.contains("<x15:timelineCacheRefs><x15:timelineCacheRef r:id="));
    let sheet = part_text(&bytes, "/xl/worksheets/sheet2.xml").unwrap();
    assert!(sheet.contains("<x15:timelineRefs><x15:timelineRef r:id="));
    let drawing = names
        .iter()
        .find(|n| n.starts_with("/xl/drawings/") && n.ends_with(".xml"))
        .unwrap();
    assert!(part_text(&bytes, drawing)
        .unwrap()
        .contains(r#"<tsle:timeslicer name="Ngày"/>"#));

    let mut back = read_xlsx(&bytes).unwrap();
    let t = &back.sheets[1].timelines[i];
    assert_eq!((t.name.as_str(), t.field.as_str()), ("Ngày", "Ngày"));
    assert_eq!(t.pivots, std::slice::from_ref(&pivot.name));
    assert_eq!((t.level, t.start.clone()), (2, None));
    assert_eq!(t.style.as_deref(), Some("TimeSlicerStyleLight1"));
    assert_eq!(t.from, bo_model::sheet::CellRef::new(1, 4));
    let c = back
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == pivot.cache_id)
        .unwrap();
    assert_eq!((c.ext_id, c.timeline_data), (Some(1), true));

    // Filter the file timeline to 2024, show years, save.
    let p = bo_sheet::timeline::periods(&back, 1, i, false);
    let y2024: Vec<_> = p.iter().filter(|x| x.label.ends_with("2024")).collect();
    let range = (y2024.first().unwrap().start, y2024.last().unwrap().end);
    bo_sheet::timeline::select(&mut back, 1, i, Some(range), &labels).unwrap();
    bo_sheet::timeline::set_options(&mut back, 1, i, Some(0), Some("Năm & ngày".into()), None)
        .unwrap();
    bo_xlsx::timeline::patch_timeline(&mut back, 1, i);
    let bytes = write_xlsx(&back).unwrap();
    // `BO_SLICER_OUT=<dir>` keeps the files for the OpenXmlValidator (`bo-cli validate`).
    if let Ok(dir) = std::env::var("BO_SLICER_OUT") {
        std::fs::write(format!("{dir}/timeline-edited.xlsx"), &bytes).unwrap();
    }
    // The state says what kind of filter the selection is.
    let names = part_names(&bytes);
    let cache = names
        .iter()
        .find(|n| n.starts_with("/xl/timelineCaches/"))
        .unwrap();
    assert!(part_text(&bytes, cache)
        .unwrap()
        .contains(r#"filterType="dateBetween"><selection startDate="2024-01-01T00:00:00""#));
    let mut again = read_xlsx(&bytes).unwrap();
    let t = &again.sheets[1].timelines[i];
    assert_eq!(t.level, 0);
    assert_eq!(t.caption, "Năm & ngày");
    assert_eq!(t.start.as_deref(), Some("2024-01-01T00:00:00"));
    assert_eq!(t.end.as_deref(), Some("2024-12-31T00:00:00"));
    assert!(again.sheets[1].pivots[0]
        .field_filters
        .iter()
        .any(|f| f.kind == PivotFilterType::DateBetween));

    // Deleting it removes the frame, the parts, their relationships and extension entries.
    assert!(bo_xlsx::timeline::remove_timeline(&mut again, 1, i));
    let bytes = write_xlsx(&again).unwrap();
    if let Ok(dir) = std::env::var("BO_SLICER_OUT") {
        std::fs::write(format!("{dir}/timeline-deleted.xlsx"), &bytes).unwrap();
    }
    let names = part_names(&bytes);
    assert!(
        !names.iter().any(|n| n.to_lowercase().contains("timeline")),
        "{names:?}"
    );
    let workbook = part_text(&bytes, "/xl/workbook.xml").unwrap();
    assert!(!workbook.contains("timelineCacheRef"), "{workbook}");
    assert!(!workbook.contains("NativeTimeline_"), "{workbook}");
    let sheet = part_text(&bytes, "/xl/worksheets/sheet2.xml").unwrap();
    assert!(!sheet.contains("timelineRef"), "{sheet}");
    for n in names.iter().filter(|n| n.starts_with("/xl/drawings/")) {
        assert!(!part_text(&bytes, n).unwrap().contains("timeslicer"));
    }
    let last = read_xlsx(&bytes).unwrap();
    assert!(last.sheets.iter().all(|s| s.timelines.is_empty()));
    assert_eq!(last.sheets[1].pivots.len(), 1);
}
