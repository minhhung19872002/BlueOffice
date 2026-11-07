# Phase 4 — BlueSlide MVP (core Rust + CLI)

UI React của BlueSlide làm sau (main thread). Phần này: model, đọc/ghi `.pptx`, render, engine, CLI, corpus.

## Đã làm

| Hạng mục | Vị trí | Ghi chú |
|---|---|---|
| Model trình chiếu | `bo_model::slide` | Presentation/Slide/Layout/Master/Notes/Theme; XML không mô hình hoá giữ nguyên vị trí (`Extra` + khoá thứ tự); `Transition`, `Timing`/`Animation`, `Timing::timeline()` |
| Đọc/ghi `.pptx` | `bo-pptx::{read_pptx, write_pptx}` | presentation, master, layout, slide, notes, theme; part lạ giữ byte-for-byte; thứ tự relationship giữ nguyên (re-target tại chỗ) |
| Kế thừa placeholder | `bo_pptx::inherit` | master → layout → slide theo `idx` rồi `type`; bodyPr, lstStyle, txStyles, fontRef, clrMap/clrMapOvr |
| Text body | `bo-pptx/src/text_layout.rs` | UAX#14, bullet nhiều cấp + buAutoNum, normAutofit (fontScale, lnSpcReduction, tự co khi tràn), numCol/spcCol, anchor, căn lề |
| Hình | `bo-drawingml::{geom, presets}` | **187/187** preset từ phụ lục ECMA-376 `presetShapeDefinitions.xml` (`presets.dsl`), custGeom, fill solid/gradient (lin/path)/pattern (hatch)/picture, line dash/cap/join/mũi tên, outerShdw (xấp xỉ, không blur), group chOff/chExt + xoay/lật, connector |
| Ảnh, bảng, biểu đồ | `bo-pptx/src/render.rs`, `bo-drawingml::chart`, `bo-pptx/src/chart_render.rs` | crop srcRect, ảnh theo hình; bảng (gridSpan/rowSpan/merge, viền, lề; chưa áp table style); chart bar/column (clustered/stacked/percent), line, area, pie/doughnut từ cache |
| Media | (giữ nguyên) | video/audio (`a:videoFile`, `p14:media`, rel media) giữ nguyên, render ảnh poster |
| Transition & animation | `bo_pptx::anim` | đọc/ghi `p:transition` (kể cả `mc:AlternateContent` p14:dur): cut/fade/push/wipe/cover/pull/split/random; main sequence: appear/fade/fly-in/wipe (entr/exit), on-click/with/after-previous; ghi lại nguyên bản khi không đổi, sinh mới theo cấu trúc §19.5 khi đã sửa |
| Display list | `bo-render` | thêm `Path`, `Image`, `PushTransform`/`PopTransform`, `PushClipPath`; `bo-raster` (PNG, giải mã PNG/JPEG) và `bo-pdf` (shading, alpha, XObject ảnh, SMask) hỗ trợ đủ |
| Engine | `bo-engine` | `open()` tự nhận `.pptx`; `open_pptx`, `load_presentation`, `slide_count`, `slide`, `slide_json`, `slide_thumbnail_json`, `slide_notes`, `image_bytes`, `presentation[_mut]`, `refresh_slide`, `save_pptx`; `DocInfo.kind` = `doc`/`slide`; PDF/PNG có ảnh |
| WASM | `bo-wasm` | `slideCount`, `slideJson`, `slideThumbnailJson`, `slideNotes`, `imageBytes`, `savePptx` |
| CLI | `bo-cli` | `render deck.pptx --out slide-%d.png`, `convert deck.pptx --to pdf`, `dump`, `sample-slides <dir>` |

## Kiểm thử

- `crates/bo-pptx/tests/roundtrip.rs`: 25 deck × mọi part — **555/555 part giống hệt (100%)** sau read→write (so XML chuẩn hoá); model ổn định qua vòng thứ hai; sửa transition/animation/nhân bản slide rồi đọc lại.
- `crates/bo-engine/tests/slides.rs`: 39 snapshot insta (`tests/snapshots/`) của display list rút gọn từng slide; tất định; push/pop cân bằng; PDF/PNG/lưu lại.
- `crates/bo-drawingml/tests/presets.rs`: giá trị hình học tính tay + đủ 187 preset.

## Hạn chế đã biết

- Chưa áp table style (`tableStyleId`), chưa blur bóng, chưa `alphaModFix`/hiệu ứng ảnh, chưa 3D.
- Chart: chỉ dùng cache, không nhãn dữ liệu, không trục phụ; scatter/radar/stock vẽ khung trống.
- Animation: chỉ main sequence; hiệu ứng khác/emphasis/motion path đọc là `Other` (giữ nguyên XML khi không sửa).
- Pattern fill dạng chấm/phần trăm vẽ màu trộn.
