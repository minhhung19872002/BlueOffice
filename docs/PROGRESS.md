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
| 5 | `.doc/.xls/.xlsb/.ppt` đọc, ODF r/w, mật khẩu, VBA giữ nguyên, TCVN3/VNI; track changes, comment, TOC/caption/cross-ref, mail merge, SmartArt fallback, bảng tổng hợp (pivot) | ✅ | [`plan/phase-5.md`](plan/phase-5.md) |
| 6 | Server ASP.NET Core (file, version, chia sẻ, audit, OIDC, Yjs relay, worker chuyển đổi), `bo-collab`, ký số PAdES/OOXML, desktop (deep link, nhiều cửa sổ, updater), on-prem Docker | ✅ (cộng tác trực tiếp: văn bản BlueDoc, ô BlueSheet, đối tượng BlueSlide) | [`plan/phase-6.md`](plan/phase-6.md), [`deploy/`](deploy/README.md) |
| 6A | BOHP + WOPI, BlueDrive giả lập, `editor-sdk` (postMessage), chế độ nhúng `/edit`, deep link desktop | ✅ theo mô tả prompt | [`bohp.md`](bohp.md) |
| 7 | BlueForm, BlueNote | 📝 chỉ kế hoạch | [`plan/phase-7.md`](plan/phase-7.md) |

## Kết quả kiểm thử (Windows 11, 2026-11-08, `main`)

| Bộ | Kết quả |
|---|---|
| `cargo test --workspace` | **657** test ✅ (proptest undo/redo doc + slide + track changes, proptest hội tụ cộng tác, round-trip corpus, ~920 dòng bảng công thức) |
| `cargo clippy --workspace --all-targets -D warnings` (pedantic) · `cargo fmt --check` · clippy wasm32 · `cargo deny` | ✅ |
| `pnpm test` (Vitest) | **109** test ✅ — gồm parity WASM ↔ native, 2 engine + 2 bản sao `bo-collab` thật qua relay (doc, sheet, slide) |
| `pnpm lint` · `pnpm typecheck` · `pnpm license-check` | ✅ |
| Playwright E2E (web) | **23** test ✅ — doc (gõ tiếng Việt, định dạng, font/Format Painter, bảng, header, thước kẻ, page setup, mẫu NĐ30, track changes/comment, chú thích sửa trên trang, chèn hàng có theo dõi), bảng tính (SUM, định dạng, sort, pivot, nhóm ngày/bố cục/% tổng), slide (kéo shape, định dạng một phần chữ, copy/paste, ô bảng trên canvas, dán slide sang deck khác) — đều lưu rồi mở lại |
| Playwright E2E cộng tác (server thật) | **4** test ✅ — `BlueOffice.Api` + BlueDrive giả lập + web sau proxy, 2 trình duyệt: gõ/định dạng/caret, lưu thành version (editors 2 người), offline rồi gộp lại, thu hồi quyền giữa chừng, xung đột offline; công thức BlueSheet và kéo shape BlueSlide |
| E2E desktop (Tauri WebDriver, `bluedoc.exe` thật) | **5** test — mở file từ tham số dòng lệnh, gõ chữ, công thức BlueSheet (engine native), BlueSlide, deep link `bluedoc://` → phiên server → cộng tác với người dùng web |
| `dotnet test` | **108** test ✅ (BOHP với BlueDrive giả lập, 2 người cùng phiên, thu hồi quyền, xung đột offline, WOPI, job chuyển đổi) |
| Fidelity ([`fidelity-report.md`](fidelity-report.md)) | mở + lưu 100 % corpus; OpenXmlValidator: docx 54/55, xlsx 57/60, pptx 35/35; visual diff 0 trang vượt ngưỡng so với baseline — phần còn lại là hạn chế đã biết của validator (`tests/fidelity/known-issues.txt`) |

Round-trip lossless (XML chuẩn hoá): `.docx` 52/52, `.xlsx` 54/54, `.pptx` 100 % part (corpus tự sinh, README ghi nguồn).

## Benchmark
| Phép đo | Kết quả | Mục tiêu (§5) |
|---|---|---|
| Layout nguội 300 trang (criterion) | 0,73 s | < 3 s ✅ |
| Gõ phím / Enter / Backspace / đậm ở tài liệu 308 trang (layout tăng dần) | 1,4–2,0 ms native · 2,9–3,9 ms WASM (trước: 46–79 ms) | < 16 ms ✅ |
| Bảng tính 1 triệu ô / 100k công thức: mở + tính | 0,80 s (release; trước 1,96 s) | < 5 s ✅ |
| Sửa 1 ô (~5 000 phụ thuộc) | 5,2 ms (release) | < 100 ms ✅ |
| Sửa 1 ô trong sheet 100k công thức (`LiveCalc`) | ~6 ms (debug), chỉ 6 ô tính lại | < 100 ms ✅ |
| Sửa 1 ô có 10 000 phụ thuộc VLOOKUP / chuỗi 10 000 ô | 26 ms / 27 ms (release; trước 169 / 153 ms) | < 100 ms ✅ |

## Ảnh chụp
- BlueDoc: [`screenshots/phase2-web-editing.png`](screenshots/phase2-web-editing.png)
- BlueSheet: [`screenshots/bluesheet-web.png`](screenshots/bluesheet-web.png)
- BlueSlide: [`screenshots/blueslide-web.png`](screenshots/blueslide-web.png), presenter [`screenshots/blueslide-presenter.png`](screenshots/blueslide-presenter.png)
- Phase 1 web/desktop/CLI: `screenshots/phase1-*.png`

## Hạn chế đã biết (ưu tiên tiếp theo)
1. **Cộng tác thời gian thực** (web nhúng và desktop qua deep link; chi tiết `plan/phase-6.md`): văn bản + định dạng
   BlueDoc (Y.Text, con trỏ người khác), ô BlueSheet, đối tượng/slide BlueSlide (last-writer-wins từng đối tượng) đồng bộ
   trực tiếp; đã kiểm bằng 2 trình duyệt thật với server đang chạy và desktop ↔ web. Chưa: bảng Word là một khối (sửa
   cùng bảng → người sau thắng), style/numbering/comment/nội dung footnote/master slide, định dạng và chèn-xoá hàng cột
   BlueSheet (chỉ đến người khác khi lưu version); desktop chưa có ô nhập địa chỉ server trong UI (dùng
   `BLUEOFFICE_SERVER` hoặc thiết lập đã lưu); server một instance (phòng trong bộ nhớ).
2. **Hiệu năng**: layout vẫn dàn lại toàn bộ khi tài liệu có NUMPAGES trong thân bài, khi đổi style/numbering/section, khi sửa header/footer và với chú thích đánh số lại theo trang (`plan/phase-2.md`).
3. **BlueDoc**: track changes chưa ghi chèn/xoá cột-ô, gộp/tách ô, header/footer, nội dung chú thích, ảnh nổi đổi vị trí;
   chú thích dài chưa tràn sang trang sau; wrap tight dùng khung bao của đa giác.
4. **BlueSheet**: pivot chưa có biểu đồ pivot, mục tính toán (`calculatedItems`), UI nhóm thủ công, lọc theo ngày; ngày gõ
   dạng chữ chưa được nhận là ngày; chart neo tuyệt đối chưa kéo được.
5. **BlueSlide**: dán slide sang deck khác chưa co giãn theo khổ slide đích (chưa chép chart part con, liên kết
   slide→slide); connector trong nhóm bị lật chưa tính phép lật của nhóm; PDF vẽ blur/glow bằng ảnh mặt nạ (không vector), chưa có 3D.
6. **Desktop**: E2E Tauri WebDriver 5 test (mở file, gõ chữ, công thức BlueSheet, BlueSlide, deep link + cộng tác) — `pnpm test:e2e:desktop`.
7. **Fidelity**: visual diff đã chạy (`docs/fidelity-visual.md`, baseline tự sinh 134 trang); so với phần mềm gốc cần người dùng đặt ảnh vào `tests/fidelity/reference/`.
8. **Ký số**: chưa có dấu thời gian RFC 3161 (PAdES B-T) và kiểm tra chuỗi CA quốc gia (ADR-0007).
9. **Server**: một instance `api` (phòng cộng tác trong bộ nhớ); schema bằng `EnsureCreated` (chưa có migration EF).
   Npgsql dùng PostgreSQL License — chấp nhận theo [ADR-0009](adr/0009-npgsql-postgresql-license.md).
10. **Mã hoá**: Agile Encryption đã kiểm chéo hai chiều với một cài đặt độc lập (msoffcrypto-tool, `crates/bo-crypto/tests/interop.rs`).

## Lịch sử
- 2026-11-08 (khuya): pivot đầy đủ (tổng phụ, 3 bố cục, nhóm ngày/số, trường tính toán, 14 kiểu "hiển thị giá trị",
  lọc/top-N, kiểu); E2E cộng tác với server thật + desktop cộng tác qua deep link (sửa lỗi caret người vào sau nhảy cuối
  tài liệu); chú thích sửa trên trang, header/footer liên kết section trước, track changes bảng/ảnh/page setup; ô bảng
  slide trên canvas, connector giữ xoay/lật, dán slide sang deck khác, blur/glow thật trong PDF.
- 2026-11-05: Phase 1 hoàn thành; đổi tên sản phẩm BlueWrite → BlueDoc (ADR-0006).
- 2026-11-07: Phase 2–6A phần cốt lõi trên các nhánh song song, hợp nhất vào `main`; `bo-sign`, `bo-collab`,
  `editor-sdk`, `bo-validate`, fidelity CI.
- 2026-11-08 (tối): layout tăng dần (< 16 ms), công thức 10k phụ thuộc 26 ms, pivot, UI BlueDoc còn thiếu (font, Format
  Painter, thước kẻ, page setup, header/footer, bảng, mẫu NĐ30), BlueSlide (rich text, clipboard, connector, bảng,
  màn hình khán giả), visual diff, E2E desktop.
- 2026-11-08: BlueSheet/BlueSlide UI đầy đủ, chỉnh sửa slide, tính năng tài liệu Phase 5, chế độ nhúng, cộng tác trực tiếp
  doc/sheet/slide; build release desktop `bluedoc.exe` (33,7 MB) chạy thử mở `.docx` qua tham số dòng lệnh.
