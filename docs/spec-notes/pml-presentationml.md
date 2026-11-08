# PresentationML — ghi chú đọc spec

Nguồn: ECMA-376 Part 1 (§13.3 part, §19 PresentationML, §21.1 text/bảng DrawingML), [MS-PPTX] (phần mở rộng p14),
[MS-ODRAWXML] (drawing SmartArt). Ghi chú chỉ diễn giải và dẫn số mục, không chép nội dung spec.
Phần dùng chung DrawingML (hình, màu, fill, theme, chart) xem [`drawingml.md`](drawingml.md).

## 1. Phạm vi đã cài

- **Các part của gói** — `crates/bo-pptx/src/read.rs`, `write.rs`: part Presentation (§13.3.6), slide/layout/master/notes/
  theme theo quan hệ (§15.2), part table styles (§13.3.11). Phần tử con của `p:presentation` theo thứ tự CT_Presentation
  (§19.2.1.26): `sldMasterIdLst` §19.2.1.37, `sldIdLst` §19.2.1.34, `sldSz` §19.2.1.39, `notesSz` §19.2.1.23,
  `defaultTextStyle` §19.2.1.8.
- **Thân slide** — `crates/bo-pptx/src/pml.rs`: `cSld` §19.3.1.16, `spTree` §19.3.1.45, `sp` §19.3.1.43, `pic` §19.3.1.37,
  `grpSp` §19.3.1.22, `cxnSp` §19.3.1.19, `graphicFrame` §19.3.1.21, `nvSpPr`/`nvPr`/`cNvPr` §19.3.1.34/.33/.12,
  nền `bg`/`bgRef` §19.3.1.1/.4, `style` §19.3.1.46; CT_Slide §19.3.1.38, `sldLayout` §19.3.1.39, `sldMaster` §19.3.1.42,
  `notes`/`notesMaster` §19.3.1.26/.28; mô hình trong `crates/bo-model/src/slide.rs`.
- **Kế thừa placeholder** — `crates/bo-pptx/src/inherit.rs`: `ph` §19.3.1.36, `txStyles` §19.3.1.52, `lstStyle`
  §21.1.2.4.12, bảng màu hiệu lực `clrMap` §19.3.1.6 + `clrMapOvr` §19.3.1.7, cờ `showMasterSp` (§19.3.1.38).
  Chuỗi kế thừa: shape → placeholder layout → placeholder master; xfrm lấy từ cấp gần nhất có xfrm đầy đủ, spPr/bodyPr/
  rPr/pPr chồng theo từng thuộc tính.
- **Chuyển cảnh & hoạt hình** — `crates/bo-pptx/src/anim.rs`: `transition` §19.3.1.50 (kiểu cut/fade/push/wipe/cover/
  pull/split/random trong `TransitionKind`, `bo-model/src/slide.rs`), `timing` §19.3.1.48, cây thời gian §19.5
  (`cTn` §19.5.29, `set` §19.5.66, `animEffect` §19.5.3, `anim` §19.5.1). Thời lượng p14 đọc từ nhánh Choice của
  `mc:AlternateContent`.
- **Chữ** — `crates/bo-pptx/src/text_layout.rs`: ngắt dòng, bullet (`buAutoNum`/`buChar`/`buNone`/`buBlip` §21.1.2.4.x),
  cột, neo dọc (`bodyPr` §21.1.2.1.1), autofit `normAutofit` §21.1.2.1.3 và `spAutoFit`.
- **Bảng** — `crates/bo-pptx/src/table_style.rs`, `crates/bo-engine/src/slide_edit/table.rs`: `tbl`/`tr`/`tc`/`tcPr`/
  `gridCol`/`tblPr` §21.1.3.x; kiểu bảng `tblStyleLst` §20.1.4.2.27 / `tblStyle` §20.1.4.2.26 với các cờ banding của
  `tblPr` §21.1.3.15.
- **Render** — `crates/bo-pptx/src/render.rs` (slide → display list, point, y hướng xuống), chart qua `chart_render.rs`.
- **Chỉnh sửa** — `crates/bo-engine/src/slide_edit/` (transaction op khả nghịch, ADR-0008); connector bám điểm nối
  `stCxn` §20.1.2.2.10 (`connect.rs`).
- **SmartArt** — `crates/bo-engine/src/smartart.rs`: vẽ part drawing dựng sẵn ([MS-ODRAWXML] §2.3.2) bằng bộ vẽ shape
  của BlueSlide, part gốc giữ nguyên.

## 2. Điểm mơ hồ & cách đã chọn

- **Khớp placeholder** (`inherit.rs` `find_placeholder`, `norm_kind`, `master_kind`): ưu tiên `idx`, sau đó `type` đã
  chuẩn hoá (`ctrTitle` coi như `title`; thiếu `type` coi như `obj`). Trên master chỉ khớp theo type; mọi type nội dung
  không có placeholder riêng (ngoài title/dt/ftr/sldNum/hdr) quy về `body`. Ở layout, placeholder `obj` của slide được
  phép ngồi lên placeholder `body` và ngược lại.
- **Lớp kiểu chữ**: font/màu từ `fontRef` của style shape đặt trên các lớp kiểu của master (`TextStyleStack` trong `inherit.rs`).
- **Chuyển cảnh trong AlternateContent** (`anim.rs`): chọn Choice trước (mang `p14:dur`), rồi Fallback; khi ghi mới, bọc
  lại AlternateContent có Choice p14 nếu có thời lượng.
- **Giữ nguyên khi chưa sửa**: transition/timing chưa đổi được ghi lại nguyên văn; khi đã sửa thì sinh lại theo cấu trúc
  tmRoot → mainSeq → nhóm click → nhóm start → node hiệu ứng. Thời lượng đọc = điểm kết thúc muộn nhất của các node hành vi.
- **Autofit co chữ** (`text_layout.rs`): bắt đầu từ `fontScale`/`lnSpcReduction` đã lưu; nếu vẫn tràn thì giảm thêm
  10 % cỡ chữ và tăng giảm-giãn-dòng 5 % mỗi bước (trần 20 %, tối đa 12 vòng) — là heuristic của BlueOffice, spec không
  quy định thuật toán. Inset mặc định của bodyPr: 0,1" trái/phải, 0,05" trên/dưới.
- **Kiểu bảng thiếu định nghĩa** (`table_style.rs`): khi deck tham chiếu một style id không có trong part table styles,
  dùng 6 kiểu dựng sẵn của BlueSlide (mỗi màu accent một kiểu, cùng schema §20.1.4.2.26, màu riêng).
- **Phần chưa mô hình hoá** (`read.rs`, `write.rs`): XML lạ giữ dạng opaque đúng vị trí; part lạ chép nguyên byte;
  quan hệ cấu trúc (layout, master, theme, notes) được trỏ lại khi ghi.
- **Trích dẫn cần soát**: trong code, §19.3.1.39 được ghi cho cả `push` (`bo-model/src/slide.rs`) và `sldLayout`
  (`read.rs`); §19.3.1.41 cho cả `random` và `sldLayoutId`; `lnSpc` được ghi §21.1.2.2.10 (`bo-model/src/slide.rs`,
  `slide_edit/text.rs`) trong khi `bo-drawingml/src/text.rs` ghi §21.1.2.2.5. Cần đối chiếu lại với bản spec đang dùng.

## 3. Quan sát black-box

Chỉ những gì code/test ghi lại:
- Mã preset hoạt hình theo cách "các phần mềm phổ biến" ghi (vào: 1 appear, 2 fly, 10 fade, 22 wipe) — `anim.rs`
  (hàm sinh timing). Timing sinh ra cho Appear là một `set` visibility 1 ms (test `generated_timing_parses_back`).
- File thực tế có `cNvPr@id` trùng nhau trên cùng slide; khoá cộng tác thêm hậu tố số lần xuất hiện —
  `crates/bo-engine/src/slide_edit/collab.rs` (`slide_shapes`).
- Không có ghi chép so sánh trực quan với phần mềm gốc; `docs/fidelity-visual.md` chỉ có baseline tự sinh.

## 4. Chưa hỗ trợ

- Khi trình chiếu chưa chạy hiệu ứng hoạt hình/chuyển cảnh (chỉ đọc/ghi model) — `docs/REQUIREMENTS.md` P4.18.
- Video/audio chỉ giữ nguyên, không phát — P4.10.
- Chưa chèn/sửa chart trong BlueSlide (chỉ đọc + vẽ từ cache) — P4.12; kiểu plot lạ chỉ vẽ khung
  (`chart_render.rs`).
- SmartArt fallback mới vẽ trong BlueDoc, BlueSlide chưa — P5.12.
- Không có 3D; blur/glow trong PDF là ảnh mặt nạ; connector trong nhóm bị lật chưa tính phép lật của nhóm; dán slide sang
  deck khác chưa co giãn theo khổ slide đích — `docs/PROGRESS.md` "Hạn chế đã biết" mục 5.
- Cộng tác master slide chưa đồng bộ — PROGRESS mục 1.
- `.ppt` và `.odp` chưa mở được trong BlueSlide — xem [`legacy-binary.md`](legacy-binary.md), REQUIREMENTS 2.10/2.11.
