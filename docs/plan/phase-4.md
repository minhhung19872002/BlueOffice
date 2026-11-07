# Phase 4 — BlueSlide MVP (core Rust + CLI)

Phần này: model, đọc/ghi `.pptx`, render, engine, CLI, corpus, **chỉnh sửa slide** (API Rust + UI `packages/slide-editor`).

## Đã làm

| Hạng mục | Vị trí | Ghi chú |
|---|---|---|
| Model trình chiếu | `bo_model::slide` | Presentation/Slide/Layout/Master/Notes/Theme; XML không mô hình hoá giữ nguyên vị trí (`Extra` + khoá thứ tự); `Transition`, `Timing`/`Animation`, `Timing::timeline()` |
| Đọc/ghi `.pptx` | `bo-pptx::{read_pptx, write_pptx}` | presentation, master, layout, slide, notes, theme; part lạ giữ byte-for-byte; thứ tự relationship giữ nguyên (re-target tại chỗ) |
| Kế thừa placeholder | `bo_pptx::inherit` | master → layout → slide theo `idx` rồi `type`; bodyPr, lstStyle, txStyles, fontRef, clrMap/clrMapOvr |
| Text body | `bo-pptx/src/text_layout.rs` | UAX#14, bullet nhiều cấp + buAutoNum, normAutofit (fontScale, lnSpcReduction, tự co khi tràn), numCol/spcCol, anchor, căn lề |
| Hình | `bo-drawingml::{geom, presets}` | **187/187** preset từ phụ lục ECMA-376 `presetShapeDefinitions.xml` (`presets.dsl`), custGeom, fill solid/gradient (lin/path)/pattern (hatch)/picture, line dash/cap/join/mũi tên, outerShdw có blur (`BlurPath`), group chOff/chExt + xoay/lật, connector |
| Ảnh, bảng, biểu đồ | `bo-pptx/src/render.rs`, `bo-pptx/src/table_style.rs`, `bo-drawingml::chart`, `bo-pptx/src/chart_render.rs` | crop srcRect, ảnh theo hình, `alphaModFix` (độ trong suốt); bảng (gridSpan/rowSpan/merge, viền, lề, **table style** `tableStyleId` từ `tableStyles.xml` hoặc style dựng sẵn của BlueSlide, cờ firstRow/lastRow/firstCol/lastCol/bandRow/bandCol); chart bar/column (clustered/stacked/percent), line, area, pie/doughnut từ cache |
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

## Chỉnh sửa slide — phần hai

| Hạng mục | Vị trí | Ghi chú |
|---|---|---|
| Rich text trong hình | `slide_edit/rich.rs`, `slide_edit/text.rs`, `bo_pptx::shape_text_geometry` | `textSelect`/`textHit`/`textExit`; `formatText` với `range` hoặc vùng chọn chữ: đậm/nghiêng/gạch chân/gạch ngang/cỡ/màu/phông/tô sáng cho một đoạn ký tự; căn lề, bullet, đánh số, mức danh sách (Tab/Shift+Tab), giãn dòng cho đoạn; caret + vùng chọn tính từ chính layout của renderer và vẽ trên overlay; textarea vô hình nhận gõ + IME (composition); `setText` `merge` gộp lần gõ thành một bước undo; `spAutoFit` giãn chiều cao, `normAutofit` ghi `fontScale`/`lnSpcReduction` khi gõ |
| Clipboard | `slide_edit/clip.rs` | `copy`/`cut`/`paste` hình (giữ nguyên model + part ảnh; id hình và rId mới; dán cùng slide lệch 12 pt; khác deck mang part ảnh theo); PNG + text ra clipboard hệ thống qua `Platform.clipboard.writeItems` (đánh dấu để dán lại bản nội bộ); dán ảnh/text từ chương trình khác thành ảnh/hộp văn bản; `copySlides`/`cutSlides`/`paste` slide trong panel slide |
| Chọn | `slide_edit/mod.rs` | `selectRect` (khung kéo), Ctrl+A, `selectNext` (Tab/Shift+Tab) |
| Adjust handle | `bo_drawingml::handles` (`handles.dsl` sinh từ phụ lục ECMA-376 ahLst/cxnLst), `slide_edit/connect.rs` | điểm vàng theo ahXY/ahPolar, kéo → giá trị avLst trong min/max (một bước undo mỗi lần kéo) |
| Connector | `slide_edit/connect.rs` | `insertConnector` nối điểm nối gần nhất của hai hình (`stCxn`/`endCxn`), tự đi lại đường khi hình nối thay đổi; kéo đầu connector để gắn/tháo khỏi điểm nối |
| Bảng | `slide_edit/table.rs` | chọn ô (click, Shift/kéo), sửa chữ ô, thêm/xoá hàng/cột, gộp/tách ô, màu nền ô, tuỳ chọn hàng tiêu đề/hàng xen kẽ; tab ribbon "Bảng"; bảng mới dùng style dựng sẵn ghi vào `tableStyles.xml` |
| Ungroup | `slide_edit/geom.rs` | frame theo dõi số lần lật; bỏ nhóm group bị lật chuyển phép lật xuống hình con, không xê dịch |
| Hiệu ứng | `bo-render` (`PushOpacity`/`PopOpacity`, `BlurPath`), `bo-raster`, `bo-pdf`, `canvas-host` | bóng có blur, ảnh `alphaModFix` |
| Màn hình khán giả | `packages/slide-editor/src/audience.ts`, `AudienceView.tsx`, `Platform.channel` | "Mở màn hình khán giả": cửa sổ thứ hai (desktop `open_window`, web `window.open`) chỉ hiện slide, đồng bộ qua BroadcastChannel / sự kiện Tauri: slide hiện tại, màn hình đen, kết thúc |

## Kiểm thử

- `crates/bo-pptx/tests/roundtrip.rs`: 25 deck × mọi part — **555/555 part giống hệt (100%)** sau read→write (so XML chuẩn hoá); model ổn định qua vòng thứ hai; sửa transition/animation/nhân bản slide rồi đọc lại.
- `crates/bo-engine/tests/slides.rs`: 39 snapshot insta (`tests/snapshots/`) của display list rút gọn từng slide; tất định; push/pop cân bằng; PDF/PNG/lưu lại.
- `crates/bo-drawingml/tests/presets.rs`: giá trị hình học tính tay + đủ 187 preset.
- `crates/bo-engine/tests/slide_editing.rs`: mọi lệnh sửa; lưu sau khi di chuyển chỉ đổi xfrm của hình đó (slide/part khác giống hệt); smart guides; group/ungroup giữ vị trí; notes master mới hợp lệ OPC; ảnh mới có part + content type; **proptest** (48 ca): chuỗi lệnh ngẫu nhiên rồi undo hết == ban đầu, redo hết == sau sửa. Unit test `geom`/`guides`/`text`/`build`.
- Corpus sửa qua API: `slide-edit-arrange/-text/-slides.pptx` (sinh bởi `bo-cli sample-slides`, `slide_edits.rs`); 28/28 deck qua OpenXmlValidator vào và ra (`node scripts/fidelity-report.mjs`), round-trip 100 %.
- `packages/slide-editor/test`: controller gộp preview, overlay chọn/handle, sửa text với composition, ribbon Sắp xếp/Chèn, ghi chú, kéo thả thumbnail.
- `crates/bo-engine/tests/slide_rich.rs`: định dạng một phần chữ, đoạn/mức/giãn dòng, gõ gộp undo + spAutoFit, normAutofit, clipboard hình (cùng slide, slide khác, deck khác) và slide, marquee/Tab, adjust handle, connector (gắn, đi lại đường, tháo/gắn lại, lưu), bảng, ungroup giữ lật, blur + alphaModFix qua PNG/PDF. Unit test `bo_drawingml::handles`, `bo_pptx::table_style`, `slide_edit::table`, `bo-raster` blur/opacity, `canvas-host`.
- Corpus sửa qua API thêm: `slide-edit-rich/-clipboard/-connectors/-tables.pptx`.
- E2E `tests/e2e/specs/slide-rich.spec.ts`: đậm một phần chữ, copy/paste hình, sửa ô bảng, lưu và mở lại.
- E2E `tests/e2e/specs/slide-editing.spec.ts` (`E2E_PORT=4502`): mở deck corpus, kéo hình, sửa text tiếng Việt, lưu, mở lại và kiểm vị trí + text; chèn hình, smart guide, thêm slide, ghi chú.

## Hạn chế đã biết

- Chưa 3D, glow/soft edge/reflection; hiệu ứng ảnh khác `alphaModFix` (duotone, độ sáng…) chưa vẽ. PDF không có blur thật (viền mờ xấp xỉ).
- Chart: chỉ dùng cache, không nhãn dữ liệu, không trục phụ; scatter/radar/stock vẽ khung trống.
- Animation: chỉ main sequence; hiệu ứng khác/emphasis/motion path đọc là `Other` (giữ nguyên XML khi không sửa).
- Pattern fill dạng chấm/phần trăm vẽ màu trộn.
- Sửa slide: connector đi lại đường bỏ `rot` (connector gấp khúc xoay 90° trở thành ngang trước); chỉ connector cấp cao nhất; sửa chữ trong ô bảng dùng ô nhập riêng (chưa có caret trên canvas như hình); định dạng một phần chữ trong ô bảng chưa có; dán slide sang deck khác bị từ chối (bố cục khác deck); không cho nhóm placeholder.
