# Tiến độ

Cập nhật 2026-11-08. Theo yêu cầu của chủ dự án, các phase được làm liên tục, **không dừng chờ duyệt**
giữa các phase; ưu tiên bộ ứng dụng văn phòng (BlueDoc/BlueSheet/BlueSlide). Tích hợp BlueDrive giữ ở mức đã làm
(BOHP/WOPI + BlueDrive giả lập), không phát triển thêm cho tới khi có API thật.

## Tổng quan

| Phase | Nội dung | Trạng thái | Kế hoạch / hạn chế |
|---|---|---|---|
| 1 | Nền tảng: workspace, OPC, text, layout, render, PDF/PNG, WASM, PAL, shell UI, CI | ✅ | [`plan/phase-1.md`](plan/phase-1.md) |
| 2 | BlueDoc MVP: `.docx` lossless, layout đầy đủ, `bo-edit`, editor (IME, clipboard, tìm/thay, lưu, PDF, tự lưu) | ✅ | [`plan/phase-2.md`](plan/phase-2.md) |
| 3 | BlueSheet: `.xlsx` lossless, định dạng số, `bo-formula` (276 hàm), lưới canvas, định dạng ô, sort/filter, CF/DV, chart, tìm/thay, in | ✅ | [`plan/phase-3-xlsx.md`](plan/phase-3-xlsx.md), [`plan/phase-3-formula.md`](plan/phase-3-formula.md) |
| 4 | BlueSlide: `.pptx` lossless, 187 preset shape, render, chỉnh sửa (di chuyển/đổi cỡ/xoay/nhóm/căn/smart guide/sửa chữ), các chế độ xem, trình chiếu, presenter | ✅ | [`plan/phase-4.md`](plan/phase-4.md) |
| 5 | `.doc/.xls/.xlsb/.ppt` đọc, ODF r/w, mật khẩu, VBA giữ nguyên, TCVN3/VNI; track changes, comment, TOC/caption/cross-ref, mail merge, SmartArt fallback | ✅ (PivotTable: chưa) | [`plan/phase-5.md`](plan/phase-5.md) |
| 6 | Server ASP.NET Core (file, version, chia sẻ, audit, OIDC, Yjs relay, worker chuyển đổi), `bo-collab`, ký số PAdES/OOXML, desktop (deep link, nhiều cửa sổ, updater), on-prem Docker | ✅ (cộng tác trực tiếp: văn bản BlueDoc, ô BlueSheet, đối tượng BlueSlide) | [`plan/phase-6.md`](plan/phase-6.md), [`deploy/`](deploy/README.md) |
| 6A | BOHP + WOPI, BlueDrive giả lập, `editor-sdk` (postMessage), chế độ nhúng `/edit`, deep link desktop | ✅ theo mô tả prompt | [`bohp.md`](bohp.md) |
| 7 | BlueForm, BlueNote | 📝 chỉ kế hoạch | [`plan/phase-7.md`](plan/phase-7.md) |

## Kết quả kiểm thử (Windows 11, 2026-11-08, `main`)

| Bộ | Kết quả |
|---|---|
| `cargo test --workspace` | **556** test ✅ (proptest undo/redo doc + slide + track changes, proptest hội tụ cộng tác, round-trip corpus, ~920 dòng bảng công thức) |
| `cargo clippy --workspace --all-targets -D warnings` (pedantic) · `cargo fmt --check` · clippy wasm32 · `cargo deny` | ✅ |
| `pnpm test` (Vitest) | **84** test ✅ — gồm parity WASM ↔ native, 2 engine + 2 bản sao `bo-collab` thật qua relay (doc, sheet, slide) |
| `pnpm lint` · `pnpm typecheck` · `pnpm license-check` | ✅ |
| Playwright E2E (web) | **12** test ✅ — doc (gõ tiếng Việt, định dạng, lưu/mở lại, tìm), track changes/comment, bảng tính (SUM, định dạng, sort, lưu/mở lại), slide (kéo shape, sửa chữ, lưu/mở lại) |
| `dotnet test` | **107** test ✅ (BOHP với BlueDrive giả lập, 2 người cùng phiên, thu hồi quyền, xung đột offline, WOPI, job chuyển đổi) |
| Fidelity ([`fidelity-report.md`](fidelity-report.md)) | mở + lưu 100 % corpus; OpenXmlValidator: docx 48/49, xlsx 48/51, pptx 28/28 — phần còn lại là hạn chế đã biết của validator (`tests/fidelity/known-issues.txt`) |

Round-trip lossless (XML chuẩn hoá): `.docx` 49/49, `.xlsx` 51/51, `.pptx` 100 % part (corpus tự sinh, README ghi nguồn).

## Benchmark
| Phép đo | Kết quả | Mục tiêu (§5) |
|---|---|---|
| Layout nguội 300 trang (criterion) | 0,73 s | < 3 s ✅ |
| Gõ phím ở tài liệu 300 trang | dàn lại toàn bộ tài liệu mỗi phím | < 16 ms ❌ (chưa có layout tăng dần theo transaction) |
| Bảng tính 1 triệu ô / 100k công thức: mở + tính | 3,20 s (release) | < 5 s ✅ |
| Sửa 1 ô (~5 000 phụ thuộc) | 22,8 ms (release) | < 100 ms ✅ |
| Sửa 1 ô trong sheet 100k công thức (`LiveCalc`) | ~6 ms (debug), chỉ 6 ô tính lại | < 100 ms ✅ |
| Sửa 1 ô có 10 000 phụ thuộc VLOOKUP | 331 ms | < 100 ms ❌ (chưa tính song song) |

## Ảnh chụp
- BlueDoc: [`screenshots/phase2-web-editing.png`](screenshots/phase2-web-editing.png)
- BlueSheet: [`screenshots/bluesheet-web.png`](screenshots/bluesheet-web.png)
- BlueSlide: [`screenshots/blueslide-web.png`](screenshots/blueslide-web.png), presenter [`screenshots/blueslide-presenter.png`](screenshots/blueslide-presenter.png)
- Phase 1 web/desktop/CLI: `screenshots/phase1-*.png`

## Hạn chế đã biết (ưu tiên tiếp theo)
1. **Cộng tác thời gian thực** (chế độ nhúng web; chi tiết `plan/phase-6.md`): văn bản + định dạng BlueDoc (Y.Text, con trỏ
   người khác), ô BlueSheet, đối tượng/slide BlueSlide (last-writer-wins từng đối tượng) đã đồng bộ trực tiếp. Chưa: bảng
   Word là một khối (sửa cùng bảng → người sau thắng), style/numbering/comment/footnote/master slide, định dạng và chèn-xoá
   hàng cột BlueSheet (chỉ đến người khác khi lưu version); desktop chưa có client WebSocket; chưa thử 2 trình duyệt thật
   với server đang chạy (mới kiểm bằng test 2 engine trao đổi frame thật).
2. **Hiệu năng**: layout tăng dần khi gõ ở tài liệu dài; tính song song công thức.
3. **BlueDoc**: thước kẻ/tab stop kéo thả, Format Painter, Paste Special, hộp thoại page setup, chọn font từ danh sách;
   sửa header/footer/footnote; thêm/xoá hàng cột bảng; wrap square/tight; track changes cho bảng/ảnh.
4. **BlueSheet**: kéo/đổi cỡ chart, cut/paste cập nhật tham chiếu, sort trái→phải/theo màu, vẽ chữ xoay, PivotTable.
5. **BlueSlide**: định dạng một phần đoạn chữ, copy/paste shape, chọn bằng khung kéo, adjust handle, sửa ô bảng,
   connector bám điểm nối; presenter view hiển thị trong cùng cửa sổ (chưa tách màn hình thứ hai).
6. **Desktop**: chưa có E2E Tauri WebDriver.
7. **Fidelity**: chưa có ảnh tham chiếu từ phần mềm gốc nên chưa đo visual diff (người dùng đặt ảnh vào `tests/fidelity/reference/`).
8. **Ký số**: chưa có dấu thời gian RFC 3161 (PAdES B-T) và kiểm tra chuỗi CA quốc gia (ADR-0007).
9. **Server**: một instance `api` (phòng cộng tác trong bộ nhớ); schema bằng `EnsureCreated` (chưa có migration EF).

## Lịch sử
- 2026-11-05: Phase 1 hoàn thành; đổi tên sản phẩm BlueWrite → BlueDoc (ADR-0006).
- 2026-11-07: Phase 2–6A phần cốt lõi trên các nhánh song song, hợp nhất vào `main`; `bo-sign`, `bo-collab`,
  `editor-sdk`, `bo-validate`, fidelity CI.
- 2026-11-08: BlueSheet/BlueSlide UI đầy đủ, chỉnh sửa slide, tính năng tài liệu Phase 5, chế độ nhúng, cộng tác trực tiếp
  doc/sheet/slide; build release desktop `bluedoc.exe` (33,7 MB) chạy thử mở `.docx` qua tham số dòng lệnh.
