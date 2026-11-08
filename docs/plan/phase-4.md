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
| Media | `bo_engine::slide_edit::media` | video/audio (`a:videoFile`, `p14:media`, rel media) giữ nguyên, render ảnh poster; liệt kê clip để phát, chèn clip mới (xem "Trình chiếu, hiệu ứng, biểu đồ, media") |
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
| Connector | `slide_edit/connect.rs` | `insertConnector` nối điểm nối gần nhất của hai hình (`stCxn`/`endCxn`), tự đi lại đường khi hình nối thay đổi — giữ `rot`, lật theo hướng đường mới, tính lại adjust của connector gấp khúc/cong (bentConnector3–5, curvedConnector3–5: chỗ gấp giữ vị trí trên slide, một chỗ gấp nằm giữa hai đầu), cả connector lồng trong nhóm (qua frame của nhóm); kéo đầu connector (cấp nào cũng được) để gắn/tháo khỏi điểm nối |
| Bảng | `slide_edit/table.rs`, `bo_pptx::table_cell_text_geometry` | chọn ô (click, Shift/kéo), bấm đúp ô để sửa chữ **trên canvas** như hình (caret/vùng chọn từ layout ô của renderer, IME, định dạng một phần chữ, `cellTextHit`/`cellNext`/`setText`+`textSelect` với `cell`), Tab/Shift+Tab sang ô sau/trước (chọn chữ của ô), thêm/xoá hàng/cột, gộp/tách ô, màu nền ô, tuỳ chọn hàng tiêu đề/hàng xen kẽ; tab ribbon "Bảng"; bảng mới dùng style dựng sẵn ghi vào `tableStyles.xml` |
| Dán slide sang deck khác | `slide_edit/clip.rs` (`SlideSource`), `paste_slides_from` | slide sao chép mang theo layout/master/theme và part của deck nguồn; khi dán: layout cùng tên, nếu không thì layout có kiểu placeholder phủ đủ placeholder của slide gần nhất, nếu không thì nhập layout + master + theme của nguồn (một bước undo); ảnh/media chép với tên part mới, quan hệ hợp lệ; ghi chú gắn với notes master của deck đích |
| Ungroup | `slide_edit/geom.rs` | frame theo dõi số lần lật; bỏ nhóm group bị lật chuyển phép lật xuống hình con, không xê dịch |
| Hiệu ứng | `bo-render` (`PushOpacity`/`PopOpacity`, `BlurPath` + `spread`), `bo-raster` (`blur_mask`), `bo-pdf`, `canvas-host` | bóng có blur (PDF: blur thật — mặt nạ mềm `SMask` rasterise), glow §20.1.8.32 (đường viền nở `rad/2` rồi làm mờ, raster/PDF/canvas), ảnh `alphaModFix` |
| Màn hình khán giả | `packages/slide-editor/src/audience.ts`, `AudienceView.tsx`, `Platform.channel` | "Mở màn hình khán giả": cửa sổ thứ hai (desktop `open_window`, web `window.open`) chỉ hiện slide, đồng bộ qua BroadcastChannel / sự kiện Tauri: slide hiện tại, màn hình đen, kết thúc |

## Dán slide khác deck, connector trong nhóm lật (bổ sung)
| Hạng mục | Vị trí | Ghi chú |
|---|---|---|
| Dán sang deck khác khổ | `slide_edit::SizeFit` | cách "tài liệu": vị trí và kích thước (`xfrm` off/ext, §20.1.7.6) của hình cấp cao nhất trên slide, layout và master nhập kèm nhân theo tỉ lệ nhỏ hơn của hai chiều `sldSz` (§19.2.1.39) và căn giữa; cỡ chữ, nét, toạ độ con của nhóm giữ nguyên |
| Part con của chart | `clip::slide_source`, `PartCarrier::copy_part` | part được chép kèm `.rels` của nó và các part nó trỏ tới (đệ quy): workbook nhúng (`c:externalData`), `chartColorStyle` / `chartStyle`; tên part mới, `.rels` mới trỏ tới bản sao (ECMA-376 Part 2, §9.3) |
| Siêu liên kết slide → slide | `paste_slides_from`, `clip::strip_rel_refs` | quan hệ kiểu `slide` (`ppaction://hlinksldjump`, §21.1.2.3.5) tới slide được dán cùng → trỏ tới bản sao; tới slide không được dán → bỏ quan hệ và `a:hlinkClick` / `a:hlinkHover` tham chiếu nó |
| Connector trong nhóm lật | `geom::Frame::flip`, `connect::visual` | lật của các nhóm bao ngoài (chẵn/lẻ theo trục) cộng vào lật của connector, điểm nối và handle: đầu connector, đi lại đường, adjust và `flipH/flipV` ghi trong không gian của nhóm |

Kiểm thử: `slide_rich.rs::connector_inside_a_flipped_group_follows_its_shapes`,
`bo-cli slide_edits::paste_tests::pasted_slides_fit_carry_charts_and_remap_links`; corpus
`slide-edit-paste-{resized,chart,links}.pptx`, `slide-edit-connectors-flip-{h,v,hv}.pptx` (OpenXmlValidator hợp lệ);
E2E `slide-rich.spec.ts` (dán slide biểu đồ sang deck khác, lưu, mở lại).

## Trình chiếu, hiệu ứng, biểu đồ, media

| Hạng mục | Vị trí | Ghi chú |
|---|---|---|
| Chuyển trang | `slide_edit/show.rs` (`setTransition`), `bo_pptx::anim` | Đặt cho slide hiện tại / danh sách slide / mọi slide (`all`); `kind: "keep"` chỉ đổi thời gian; hướng chỉ ghi khi hợp lệ với kiểu (§19.7.50/52: push/wipe `l u r d`, cover/pull 8 hướng, split `horz/vert`); thời lượng ghi `p14:dur` trong `mc:AlternateContent`; `advClick`/`advTm`. Kiểu lạ (vd `p:dissolve`) giữ nguyên phần tử khi chỉ đổi thời gian |
| Hiệu ứng | `slide_edit/show.rs` (`addAnimation`, `setAnimation`, `moveAnimation`, `removeAnimation`) | Xuất hiện/mờ dần/bay vào/quét, lớp vào (`entr`) hoặc ra (`exit`); kích hoạt khi bấm / cùng lúc / sau hiệu ứng trước; thời lượng, độ trễ, hướng. Hiệu ứng không mô hình hoá (nhấn mạnh, đường chuyển động, filter khác) giữ `p:par` gốc (`Animation::source`), khi sinh lại `p:timing` chỉ đổi `cTn@id`, `nodeType` và `delay` |
| Dữ liệu trình chiếu | lệnh `show` → `ShowOut` | Slide tách lớp: nền + hình master/layout + các dải hình không có hiệu ứng, mỗi hình cấp cao có hiệu ứng một lớp trong suốt riêng (`bo_pptx::RenderOptions::layer`, giữ đúng thứ tự z); danh sách hiệu ứng có timeline (`Timing::timeline`), số lần bấm, chuyển trang (thời lượng mặc định: fast 500 ms, med 750 ms, slow 1000 ms — lựa chọn của BlueSlide), clip media |
| Phát | `packages/slide-editor/src/player.ts`, `ShowStage.tsx` | Logic thuần (có test): hiển thị lớp theo bước bấm, keyframe từng hiệu ứng (fade = opacity, bay vào = dịch lớp ra ngoài khung slide, quét = `clip-path: inset` theo khung hình, xuất hiện = bậc), keyframe chuyển trang (push/cover/pull = dịch, wipe/split = `clip-path`, kiểu lạ = fade); áp bằng Web Animations API. Dùng ở trình chiếu toàn màn hình, Presenter View, màn hình khán giả (đồng bộ cả bước bấm), Reading View, xem thử trong ngăn hiệu ứng; tự chuyển theo `advTm`, tắt chuyển khi bấm chuột theo `advClick` |
| UI hiệu ứng | `AnimationPane.tsx`, ribbon "Chuyển trang" / "Hiệu ứng" | Ribbon: kiểu chuyển trang, áp dụng cho mọi slide, hiệu ứng vào/ra, xem thử; ngăn bên phải: kiểu/hướng/thời lượng/tự chuyển của chuyển trang, danh sách hiệu ứng (bước bấm, kích hoạt, hướng, thời lượng, độ trễ, lên/xuống, xoá), xem thử slide |
| Biểu đồ | `slide_edit/chart.rs`, `ChartDialog.tsx` | `insertChart` (cột, thanh, đường, vùng, tròn, vành khuyên; tiêu đề, chú giải, lưới dữ liệu): part chart sinh bằng `bo_xlsx::chart_write::chart_space_xml` (§21.2), workbook nhúng `/ppt/embeddings/workbookN.xlsx` (`bo_xlsx::write_xlsx`, Sheet1: danh mục cột A, tên chuỗi hàng 1) nối qua `c:externalData` (§21.2.2.63) + quan hệ `package`; `setChart` đổi loại/tiêu đề/chú giải/dữ liệu của chart mới hoặc chart đọc từ file (`bo_xlsx::chart_write::parse_chart_part`), cập nhật workbook nhúng hoặc tạo mới; kết quả lệnh có `chart` (dữ liệu để sửa); tab ngữ cảnh "Biểu đồ", bấm đúp mở hộp thoại |
| Media | `slide_edit/media.rs`, `MediaLayer.tsx` | Kết quả lệnh có `media` (id, loại, khung, part, MIME, liên kết ngoài); `Engine::media` trả cả part media của slide; chèn: `slideInsertPicture` với phần mở rộng media → part media + ảnh đại diện PNG (tự vẽ bằng `bo-raster`) + 3 quan hệ (`video`/`audio`, media [MS-PPTX], `image`); phát bằng `<video>/<audio>` khi `canPlayType` cho phép, nếu không (hoặc lỗi giải mã) giữ ảnh đại diện kèm nhãn |
| Xuất PNG | `ExportDialog.tsx`, `zip.ts` | `file.exportPng`: slide hiện tại → `<tên>-N.png`, tất cả → `<tên>-png.zip` (ZIP lưu không nén, APPNOTE); tỉ lệ 1/2/3 px/pt; vẽ bằng chính renderer canvas của editor |
| Slide Sorter / Reading View | `BlueSlideEditor.tsx` | Kéo thả trong Slide Sorter (`moveSlide`); Reading View chạy slide như trình chiếu trong cửa sổ (nút trước/sau, phím, Esc về Thường) |
| Chỉ đọc | `packages/hosted/src/readonly.ts` | `show` là lệnh chỉ đọc (người xem vẫn trình chiếu được) |

## Kiểm thử

- `crates/bo-pptx/tests/roundtrip.rs`: 25 deck × mọi part — **555/555 part giống hệt (100%)** sau read→write (so XML chuẩn hoá); model ổn định qua vòng thứ hai; sửa transition/animation/nhân bản slide rồi đọc lại.
- `crates/bo-engine/tests/slides.rs`: 39 snapshot insta (`tests/snapshots/`) của display list rút gọn từng slide; tất định; push/pop cân bằng; PDF/PNG/lưu lại.
- `crates/bo-drawingml/tests/presets.rs`: giá trị hình học tính tay + đủ 187 preset.
- `crates/bo-engine/tests/slide_editing.rs`: mọi lệnh sửa; lưu sau khi di chuyển chỉ đổi xfrm của hình đó (slide/part khác giống hệt); smart guides; group/ungroup giữ vị trí; notes master mới hợp lệ OPC; ảnh mới có part + content type; **proptest** (48 ca): chuỗi lệnh ngẫu nhiên rồi undo hết == ban đầu, redo hết == sau sửa. Unit test `geom`/`guides`/`text`/`build`.
- Corpus sửa qua API: `slide-edit-arrange/-text/-slides.pptx` (sinh bởi `bo-cli sample-slides`, `slide_edits.rs`); 28/28 deck qua OpenXmlValidator vào và ra (`node scripts/fidelity-report.mjs`), round-trip 100 %.
- `packages/slide-editor/test`: controller gộp preview, overlay chọn/handle, sửa text với composition, ribbon Sắp xếp/Chèn, ghi chú, kéo thả thumbnail.
- `crates/bo-engine/tests/slide_rich.rs`: định dạng một phần chữ, đoạn/mức/giãn dòng, gõ gộp undo + spAutoFit, normAutofit, clipboard hình (cùng slide, slide khác, deck khác) và slide, marquee/Tab, adjust handle, connector (gắn, đi lại đường, tháo/gắn lại, lưu), bảng, ungroup giữ lật, blur + alphaModFix qua PNG/PDF. Unit test `bo_drawingml::handles`, `bo_pptx::table_style`, `slide_edit::table`, `bo-raster` blur/opacity, `canvas-host`.
- Corpus sửa qua API thêm: `slide-edit-rich/-clipboard/-connectors/-tables.pptx`, `slide-edit-connectors-rotated.pptx` (connector gấp khúc xoay 90° + connector trong nhóm), `slide-edit-paste-other-deck.pptx`; deck mẫu `slide-shapes-glow.pptx`.
- `slide_rich.rs` thêm: sửa chữ ô bảng trên canvas + đậm một phần + Tab, connector xoay giữ `rot` khi đi lại đường, connector trong nhóm, dán slide sang deck khác (layout khớp / nhập layout+master+theme, quan hệ hợp lệ, undo), glow + blur PDF (`/SMask`).
- E2E `tests/e2e/specs/slide-rich.spec.ts`: đậm một phần chữ, copy/paste hình, sửa ô bảng trên canvas + đậm một phần chữ trong ô + Tab, dán slide từ deck khác, lưu và mở lại.
- E2E `tests/e2e/specs/slide-editing.spec.ts` (`E2E_PORT=4502`): mở deck corpus, kéo hình, sửa text tiếng Việt, lưu, mở lại và kiểm vị trí + text; chèn hình, smart guide, thêm slide, ghi chú.
- `crates/bo-engine/tests/slide_show.rs`: chuyển trang (đặt/mọi slide/chỉ thời gian/xoá, hướng không hợp lệ bị bỏ, undo), hiệu ứng (thêm/sửa/sắp xếp/xoá + timeline, lưu, mở lại, undo), lớp trình chiếu (mỗi hình có hiệu ứng một lớp, các lớp cộng lại = display list của slide), hiệu ứng lạ giữ nguyên khi sửa chuỗi, chart (chèn, sửa loại/tiêu đề/chú giải/dữ liệu, workbook nhúng đọc lại được, mở lại vẫn sửa được, undo; 4 chart corpus sửa được), media (liệt kê nhúng/liên kết, chèn video/âm thanh, quan hệ + content type). Unit test `anim` (`unknown_effects_survive_a_regenerated_sequence`, `unknown_transition_keeps_its_element`), `show`, `chart`.
- Corpus thêm (sinh bởi `bo-cli sample-slides`, qua OpenXmlValidator): `slide-anim-unknown.pptx` (nhấn mạnh, đường chuyển động, dissolve), `slide-edit-transitions/-animations/-anim-unknown/-charts/-charts-existing/-media.pptx`; mỗi nhóm tính năng ≥ 3 file (chuyển trang/hiệu ứng: `slide-anim-*` + `slide-edit-transitions/-animations/-anim-unknown`; chart: `slide-charts-*` + `slide-edit-charts*`; media: `slide-media-*` + `slide-edit-media`).
- `packages/slide-editor/test/slide.test.tsx`: `player` (hiển thị lớp, keyframe, chuyển trang, bước bấm/slide/màn kết thúc), ZIP + CRC-32, số trong lưới chart; UI: đặt chuyển trang + ngăn hiệu ứng, trình chiếu theo bước bấm, hộp thoại chart, xuất PNG, kéo thả Slide Sorter + Reading View.
- E2E `tests/e2e/specs/slide-show-media.spec.ts`: chuyển trang + hiệu ứng → lưu → mở lại → trình chiếu (lớp ẩn đến khi bấm, chuyển trang push); chèn chart → sửa dữ liệu/loại → lưu → mở lại → sửa tiếp; media (clip không giải mã được giữ ảnh đại diện, chèn WAV phát được ở chế độ Thường và trình chiếu, lưu, mở lại); xuất PNG (1 slide, `.zip` tất cả), kéo thả trong Slide Sorter, Reading View.

## Hạn chế đã biết

- Chưa 3D, soft edge/reflection/innerShdw; hiệu ứng ảnh khác `alphaModFix` (duotone, độ sáng…) chưa vẽ. Blur/glow trong PDF là ảnh mặt nạ 2 px/pt (không vector).
- Chart: chỉ dùng cache, không nhãn dữ liệu, không trục phụ; scatter/radar/stock vẽ khung trống.
- Animation: chỉ main sequence; hiệu ứng nhấn mạnh/đường chuyển động/filter khác giữ nguyên XML (cả khi sửa hiệu ứng khác) nhưng khi chiếu chỉ nhấp nháy mờ; hiệu ứng theo đoạn văn (`p:txEl`) chạy cho cả hình; hiệu ứng nhắm hình trong nhóm chạy cho cả nhóm cấp cao; `p:bldLst` không được sinh lại khi chuỗi hiệu ứng được sinh lại.
- Chuyển trang: chỉ cut/fade/push/wipe/cover/pull/split có chuyển động riêng; kiểu khác (kể cả random) phát như fade; âm thanh chuyển trang (`p:sndAc`) không phát.
- Chart: chart đọc từ file khi sửa được sinh lại từ tóm tắt (mất định dạng riêng của chuỗi/trục, như BlueSheet); combo chart gộp thành một kiểu.
- Media: clip liên kết ngoài chỉ hiện ảnh đại diện; không áp `p14:trim`, không có nút media trong `p:timing` (clip không tự phát khi vào slide); ảnh đại diện của clip mới là hình vẽ sẵn (không lấy khung hình đầu).
- Pattern fill dạng chấm/phần trăm vẽ màu trộn.
- Sửa slide: không cho nhóm placeholder. Dán slide khác khổ: chữ, độ dày nét và toạ độ con của nhóm không co giãn (chỉ vị trí/kích thước hình cấp cao nhất); placeholder không có `xfrm` lấy vị trí từ layout của deck đích. 3D (`a:scene3d` / `a:sp3d`) chưa vẽ (kể cả gần đúng).
