# DrawingML — ghi chú đọc spec

Nguồn: ECMA-376 Part 1 §20.1 (DrawingML main), §21.1 (text, bảng), §21.2 (chart), phụ lục `presetShapeDefinitions.xml`
(Annex D). Ghi chú chỉ diễn giải và dẫn số mục, không chép nội dung spec. Crate `crates/bo-drawingml` dùng chung cho
BlueSlide, BlueSheet, BlueDoc; phần đặc thù slide xem [`pml-presentationml.md`](pml-presentationml.md).

## 1. Phạm vi đã cài

- **DOM giữ phần lạ** — `src/dom.rs`: DOM nhỏ trên `quick-xml`, đi qua `bo_opc::xml::XmlReader` nên chịu giới hạn độ sâu XML;
  phần tử con chưa hiểu được giữ đúng vị trí (test `extras_keep_position`, `sp_pr_roundtrip_keeps_unknown_in_place`).
- **Màu** — `src/color.rs`: sáu mô hình màu (`srgbClr` §20.1.2.3.32, `scrgbClr` .30, `hslClr` .13, `schemeClr` .29,
  `sysClr` .33, `prstClr` .22) và các phép biến đổi §20.1.2.3.x: alpha/alphaMod/alphaOff, lum*/sat*/hue*, tint/shade,
  comp, inv, gray, gamma/invGamma, red*/green*/blue*. Ánh xạ bg1/tx1/bg2/tx2 qua `clrMap` §19.3.1.6; `phClr` lấy từ
  style reference.
- **Thuộc tính shape** — `src/props.rs`: `spPr` §20.1.2.2.35, `grpSpPr` §20.1.2.2.22, `xfrm` §20.1.7.6, `ln`
  §20.1.2.2.24 (`prstDash` §20.1.8.48, đầu/đuôi mũi tên), fill `noFill`/`solidFill`/`gradFill` (`lin`, `path`)/`pattFill`/
  `blipFill`/`grpFill` §20.1.8.x, `effectLst` §20.1.8.26 (`outerShdw`, `glow`), tham chiếu ma trận style §20.1.4.2.
- **Hình học** — `src/geom.rs`: ngôn ngữ công thức guide `gd@fmla` §20.1.9.11 (toán tử `*/`, `+-`, `+/`, `?:`, `abs`, `at2`,
  `cat2`, `sat2`, `cos`, `sin`, `tan`, `max`, `min`, `mod`, `pin`, `sqrt`, `val`), guide dựng sẵn theo kích thước
  w×h, `custGeom` §20.1.9.8, `prstGeom` §20.1.9.18, path §20.1.9.15 (hệ toạ độ riêng w/h), cung `arcTo` đổi sang Bézier
  bậc ba, chế độ tô ST_PathFillMode §20.1.10.37.
- **Preset** — `src/presets.rs` + `src/presets.dsl`: đủ 187 hình (ST_ShapeType §20.1.10.56); `crates/bo-drawingml/tests/presets.rs`
  kiểm giá trị tính tay cho khung 2000×1000 và đủ 187 preset.
- **Tay nắm & điểm nối** — `src/handles.rs` + `src/handles.dsl` (180 mục): `ahLst` §20.1.9.1 (`ahXY`, `ahPolar`),
  `cxnLst` §20.1.9.10 / `cxn` §20.1.9.9.
- **Theme** — `src/theme.rs`: `theme` §20.1.6.9, `clrScheme` §20.1.6.2, `fontScheme` §20.1.4.1.18, `fmtScheme`
  §20.1.4.1.14; giải tên font theme (`+mj-lt`, `+mn-ea`…) qua `resolve_typeface`.
- **Text** — `src/text.rs`: `txBody`, `bodyPr` §21.1.2.1.1, `noAutofit`/`normAutofit`/`spAutoFit`, `p`/`pPr`/`r`/`rPr`,
  `lnSpc`/`spcBef`/`spcAft`, `tabLst`, `buClr`/`buClrTx`/`buFont`/`buFontTx`, `lstStyle` §21.1.2.4.12.
- **Bảng** — `src/table.rs`: `tbl` §21.1.3.13, `tcPr` §21.1.3.17 (test `table_roundtrip`).
- **Chart (chỉ đọc)** — `src/chart.rs`: `chartSpace` §21.2.2.29, `barChart` (cả 3D, `barDir`, `grouping` §21.2.2.76,
  `gapWidth` §21.2.2.75), `lineChart`, `pieChart`/`doughnutChart`, `areaChart`, `ser` §21.2.2.168, `dPt` §21.2.2.52,
  `varyColors` §21.2.2.47, `title` §21.2.2.210, cache `strCache`/`numCache`. Vẽ: `crates/bo-pptx/src/chart_render.rs`.

## 2. Điểm mơ hồ & cách đã chọn

- **Thứ tự biến đổi màu** (`color.rs`): áp dụng đúng thứ tự xuất hiện trong tài liệu. `lum*`/`sat*`/`hue*` tính trong HSL;
  `tint`/`shade` và `red*`/`green*`/`blue*` tính trên thành phần **tuyến tính** (sRGB ↔ linear theo IEC 61966-2-1);
  test `lum_mod_off`, `tint_shade_alpha` ghi các giá trị kỳ vọng.
- **`sysClr` thiếu `lastClr`**: dùng bảng mặc định riêng (`system_color`); `prstClr` chỉ cài một tập con bảng màu tên.
- **Cung `arcTo`** (`geom.rs`): góc bắt đầu/quét được hiểu là góc nhìn từ tâm rồi đổi sang tham số ellipse; giữ chiều và số
  vòng của góc quét (test `quarter_arc_is_exact_at_ends`).
- **Preset lạ / lỗi công thức**: vẽ thành hình chữ nhật và đẩy cảnh báo (`geom::evaluate`), không làm hỏng việc mở file.
- **Hiệu ứng**: chỉ `outerShdw` và `glow` có con là màu được mô hình hoá; mọi hiệu ứng khác giữ nguyên dạng opaque (`props.rs`
  `parse_effects`).
- **Chart**: chỉ đọc giá trị cache, không tính lại công thức bảng tính; `multiLvlStrCache` lấy cấp đầu; không tự sinh tiêu
  đề tự động (chỉ title tường minh); thanh ngang vẽ danh mục từ dưới lên theo `orientation` minMax (§21.2.2.37); bước trục
  "đẹp" 1/2/5×10ⁿ là lựa chọn của BlueOffice. Part chart giữ nguyên byte.
- **Nguồn gốc dữ liệu preset (clean-room)**: header `presets.dsl` ghi rằng file được **chuyển đổi cơ học** từ phụ lục
  `presetShapeDefinitions.xml` của ECMA-376 Part 1 (bản 5) sang định dạng dòng gọn ánh xạ 1:1 (`avLst`, `gdLst`, `rect`,
  `pathLst`), bỏ `ahLst`/`cxnLst`, và không sửa tay — sinh lại từ phụ lục. `handles.dsl` cũng chuyển cơ học từ cùng phụ lục
  cho tay nắm/điểm nối. Đây là dữ liệu đặc tả công khai (nguồn hợp lệ theo CLAUDE.md), ghi trong `THIRD_PARTY_NOTICES.md`
  mục "Dữ liệu đặc tả" (hiện chỉ nhắc `presets.dsl`, chưa nhắc `handles.dsl`). Repo không chứa script chuyển đổi.
- **Trích dẫn cần soát**: `geom.rs` ghi `arcTo` là §20.1.9.3 trong khi `handles.rs`/`handles.dsl` ghi §20.1.9.3 là `ahPolar`;
  `spAutoFit` là §21.1.2.1.4 trong `text.rs` nhưng §21.1.2.1.2 trong `bo-engine/src/slide_edit/rich.rs`.

## 3. Quan sát black-box

Code và test không ghi quan sát black-box nào cho DrawingML. Các giá trị kỳ vọng trong test (`color.rs`, `tests/presets.rs`)
được tính tay từ công thức, không đo từ phần mềm khác.

## 4. Chưa hỗ trợ

- Không có 3D (`scene3d`/`sp3d` chỉ được nhận diện để giữ vị trí trong `props.rs`/`text.rs`, không vẽ) — `docs/REQUIREMENTS.md` P4.7, `docs/PROGRESS.md` mục 5.
- Blur/glow khi xuất PDF là ảnh mặt nạ, không vector — PROGRESS mục 5.
- Hiệu ứng ngoài `outerShdw`/`glow` (innerShdw, reflection, softEdge…) chỉ được giữ, không vẽ (`props.rs`).
- Chart: chỉ bar/column/line/area/pie/doughnut; loại khác vẽ khung trống (`chart.rs` `Other`, `chart_render.rs`); chưa chèn/
  sửa chart trong BlueSlide — REQUIREMENTS P4.12.
- `prstClr` mới là tập con (`color.rs`).
