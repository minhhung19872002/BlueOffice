# Phase 4 — BlueSlide MVP (core Rust + CLI)

Phần này: model, đọc/ghi `.pptx`, render, engine, CLI, corpus, **chỉnh sửa slide** (API Rust + UI `packages/slide-editor`).

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

## Chỉnh sửa slide (Phase 4 — editing)

| Hạng mục | Vị trí | Ghi chú |
|---|---|---|
| API sửa | `bo_engine::slide_edit` (`Engine::slide_command[_json]`, `slide_insert_picture`) | Cầu lệnh JSON như ADR-0008: `SlideCommand` → `SlideEditResult` (vùng chọn + khung/handle, trạng thái định dạng, slide bẩn, `modified`, ghi chú, layout, preview) |
| Op khả nghịch | `slide_edit/ops.rs` | `Nodes` (splice cây hình), `Slides`, `Fields` (rels/notes/timing), `Parts`, `Pres`; mỗi op mang nội dung bị xoá, `apply` phát hiện op cũ; transaction + undo/redo không giới hạn, khôi phục cả vùng chọn |
| Hình học | `slide_edit/geom.rs` | Đặt hình giống hệt renderer (xfrm, group chOff/chExt, xoay/lật, kế thừa xfrm placeholder); hit-test theo z-order (chọn group, `deep` chọn con), 8 handle + handle xoay, resize theo trục riêng của hình (khoá tỉ lệ), xoay bước 15° |
| Smart guides | `slide_edit/guides.rs` | Bắt dính cạnh/tâm slide và cạnh/tâm hình khác trong dung sai; resize chỉ bắt cạnh đang kéo; trả đường gióng cho UI |
| Lệnh | `slide_edit/mod.rs` | move/resize/rotate (preview + commit), xoá (xoá cả animation trỏ tới hình), nhân bản, z-order, group/ungroup, căn/phân bố, chèn hộp văn bản / hình preset / ảnh (part media + rel + content type) / bảng, sửa text (giữ định dạng run quanh chỗ sửa, `\u{b}` = `a:br`), đậm/nghiêng/gạch chân/cỡ/màu/căn lề/bullet, màu tô/viền, thêm slide từ layout, xoá/nhân bản/sắp xếp slide, ghi chú (tự tạo notes master nếu deck chưa có) |
| Lossless | `bo-pptx` | Hình được sửa sinh lại từ model nên giữ `Extra`; phần khác không đổi; part mới khai báo content type (§10.1.2.2) |
| Binding | `bo-wasm` `slideCommand`/`slideInsertPicture`; Tauri `slide_cmds.rs`; `EngineHost.slideCommand`/`slideInsertPicture` (worker + native) | |
| UI | `packages/slide-editor` | Overlay SVG trên canvas (viewBox = slide pt): outline, handle, ghost khi kéo, đường gióng; Shift chọn nhiều, phím mũi tên dịch 1/10 pt, Delete, Ctrl+D, Ctrl+G/Ctrl+Shift+G, Ctrl+Z/Y; double-click sửa text bằng `<textarea>` không kiểm soát (IME tiếng Việt, chỉ commit ngoài composition); ribbon Trang đầu/Chèn/Sắp xếp qua `CommandRegistry`; panel slide: thêm/xoá/nhân bản/kéo thả sắp xếp; ghi chú sửa được; cờ “Đã sửa” + Lưu .pptx |

## Kiểm thử

- `crates/bo-pptx/tests/roundtrip.rs`: 25 deck × mọi part — **555/555 part giống hệt (100%)** sau read→write (so XML chuẩn hoá); model ổn định qua vòng thứ hai; sửa transition/animation/nhân bản slide rồi đọc lại.
- `crates/bo-engine/tests/slides.rs`: 39 snapshot insta (`tests/snapshots/`) của display list rút gọn từng slide; tất định; push/pop cân bằng; PDF/PNG/lưu lại.
- `crates/bo-drawingml/tests/presets.rs`: giá trị hình học tính tay + đủ 187 preset.
- `crates/bo-engine/tests/slide_editing.rs`: mọi lệnh sửa; lưu sau khi di chuyển chỉ đổi xfrm của hình đó (slide/part khác giống hệt); smart guides; group/ungroup giữ vị trí; notes master mới hợp lệ OPC; ảnh mới có part + content type; **proptest** (48 ca): chuỗi lệnh ngẫu nhiên rồi undo hết == ban đầu, redo hết == sau sửa. Unit test `geom`/`guides`/`text`/`build`.
- Corpus sửa qua API: `slide-edit-arrange/-text/-slides.pptx` (sinh bởi `bo-cli sample-slides`, `slide_edits.rs`); 28/28 deck qua OpenXmlValidator vào và ra (`node scripts/fidelity-report.mjs`), round-trip 100 %.
- `packages/slide-editor/test`: controller gộp preview, overlay chọn/handle, sửa text với composition, ribbon Sắp xếp/Chèn, ghi chú, kéo thả thumbnail.
- E2E `tests/e2e/specs/slide-editing.spec.ts` (`E2E_PORT=4502`): mở deck corpus, kéo hình, sửa text tiếng Việt, lưu, mở lại và kiểm vị trí + text; chèn hình, smart guide, thêm slide, ghi chú.

## Hạn chế đã biết

- Chưa áp table style (`tableStyleId`), chưa blur bóng, chưa `alphaModFix`/hiệu ứng ảnh, chưa 3D.
- Chart: chỉ dùng cache, không nhãn dữ liệu, không trục phụ; scatter/radar/stock vẽ khung trống.
- Animation: chỉ main sequence; hiệu ứng khác/emphasis/motion path đọc là `Other` (giữ nguyên XML khi không sửa).
- Pattern fill dạng chấm/phần trăm vẽ màu trộn.
- Sửa slide: định dạng ký tự áp cho toàn bộ text của hình (chưa chọn một phần text trong textarea); chưa có clipboard hình/slide, chọn bằng khung kéo (marquee), handle điều chỉnh (adjust) của preset, sửa ô bảng, connector bám điểm nối; ungroup group bị lật bỏ phép lật; text box `spAutoFit` chưa tự giãn chiều cao khi sửa; không cho nhóm placeholder; E2E desktop (Tauri) chưa có.
