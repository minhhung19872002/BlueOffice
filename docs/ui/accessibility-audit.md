# Kiểm tra khả năng tiếp cận giao diện BlueOffice (WCAG 2.2 AA)

Vòng rà soát cuối của giai đoạn Parity (`BlueOffice-PARITY-PROMPT.md` §9.3). Phạm vi: **giao diện của chính
BlueOffice** (khung ứng dụng, ribbon, màn hình Tệp, hộp thoại, ngăn tác vụ, thanh trạng thái) của BlueDoc, BlueSheet,
BlueSlide trên web. Nội dung tài liệu do người dùng soạn (màu chữ, tương phản trong trang) không thuộc phạm vi —
BlueDoc có trình kiểm tra trợ năng riêng cho tài liệu (Xem lại › Kiểm tra tài liệu).

> Đây là kiểm tra tự động + đi qua bằng bàn phím, **không phải** chứng nhận tuân thủ. Các mục cần người dùng trình đọc
> màn hình thật được liệt kê ở cuối.

## 1. Phương pháp và công cụ

| Công cụ | Vai trò |
|---|---|
| [axe-core](https://github.com/dequelabs/axe-core) 4.10.3 qua `@axe-core/playwright` 4.10.2 (MPL-2.0, chỉ dùng khi test) | Quét tự động với các tag `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22aa` |
| Playwright 1.64 (Chromium, 1366 × 900) | Dựng từng trạng thái giao diện, mô phỏng chủ đề, bấm phím |
| `tests/e2e/specs/a11y.spec.ts` | Bộ quét: **lỗi nào cũng làm test thất bại**; danh sách loại trừ tường minh có lý do |
| `tests/e2e/specs/a11y-keyboard.spec.ts` | Đi qua chỉ bằng bàn phím cho cả ba ứng dụng |
| Vitest: `packages/ui/test/taskPanes.test.tsx` (F6), `packages/ui/test/dialog.test.tsx` (trả focus), `packages/canvas-host/test/ime.test.ts` (ô nhập có tên) | Hồi quy cho các sửa đổi |

Chạy lại:

```bash
pnpm --filter @blueoffice/web build
cd tests/e2e && E2E_PORT=4173 pnpm exec playwright test a11y.spec.ts a11y-keyboard.spec.ts --workers=1
# A11Y_OUT=<tệp> ghi thêm danh sách vi phạm ra tệp (để soạn báo cáo)
```

Chủ đề: `packages/ui/src/theme.ts` chọn chủ đề theo hệ thống — `prefers-color-scheme: dark` → tối,
`prefers-contrast: more` / `forced-colors: active` → tương phản cao (`data-theme="hc"`). Test mô phỏng media query
(`page.emulateMedia`) và kiểm tra thuộc tính `data-theme` đã đổi trước khi quét.

### Loại trừ (một mục)

| Bộ chọn | Lý do |
|---|---|
| `canvas` | Trang tài liệu / lưới ô / slide là **điểm ảnh do engine vẽ** (ADR-0002); axe không đọc được chữ trong canvas, và màu/tương phản của nội dung là lựa chọn của tác giả tài liệu. Mọi phần tử DOM bao quanh canvas (khung trang có tên, ô nhập văn bản, ô đang nhập của BlueSheet, lớp phủ slide, ảnh có `role="img"`) **vẫn được quét**. |

Lớp tiếp cận hiện có của tài liệu (đã kiểm tra trong mã): vùng cuộn `role="document"`
(`packages/canvas-host/src/DocumentView.ts`), mỗi trang là `role="group"` tên "Trang n", ảnh / thanh cộng tác là
`role="img"` có nhãn, bàn phím và IME đi vào một `<textarea>` có tên "Nội dung tài liệu" (`packages/canvas-host/src/ime.ts`).
BlueSheet: ô đang nhập có nhãn "Ô đang nhập", thanh công thức có nhãn. **Chưa có** bản sao văn bản (mirror) của
nội dung trang cho trình đọc màn hình — xem §5.

## 2. Các trang / trạng thái đã quét

Mỗi trạng thái quét toàn trang (trừ `canvas`):

- **Màn hình bắt đầu** BlueDoc (`/doc`, chưa mở tài liệu).
- **BlueDoc** có tài liệu: toàn khung; từng thẻ ribbon (Trang đầu, Chèn, Thiết kế, Bố cục, Tham chiếu, Gửi thư, Xem lại,
  Xem); sau khi chèn bảng (thẻ ngữ cảnh Bảng); thanh trạng thái; Tệp › Thông tin, Lưu thành, Tùy chọn; hộp thoại
  Phông chữ, bảng chọn giãn dòng/khoảng cách đoạn (BlueDoc không có hộp thoại "Đoạn văn" riêng — các tuỳ chọn đoạn nằm
  ở bảng chọn này), Tìm và thay thế, Thuộc tính bảng; ngăn Kiểu, Dẫn hướng, Chú thích, BlueScript.
- **BlueSheet** có sổ tính: khung, từng thẻ ribbon, Tệp › Thông tin / Lưu thành / Tùy chọn; Định dạng ô, Chèn biểu đồ
  (bảng chọn loại), Tìm và thay thế; ngăn Chú thích, BlueScript.
- **BlueSlide** có bài trình chiếu (`tests/corpus/slide/slide-text-bullets.pptx`): khung, từng thẻ ribbon, Tệp ›
  Thông tin / Lưu thành / Tùy chọn; sau khi chèn bảng; hộp thoại Chèn biểu đồ; ngăn Vùng chọn, Hoạt hình, Chú thích,
  BlueScript.
- **Chủ đề tối và tương phản cao** cho cả ba ứng dụng: khung chính + một hộp thoại (Phông chữ / Định dạng ô / Chèn
  biểu đồ).

## 3. Kết quả — trước và sau

### 3.1 Quét axe

| # | Quy tắc (WCAG) | Ở đâu | Trước | Sửa |
|---|---|---|---|---|
| 1 | `aria-hidden-focus` (4.1.2) | Ô nhập bàn phím/IME của BlueDoc có `aria-hidden` mà vẫn nhận focus | Mọi trạng thái BlueDoc | Bỏ `aria-hidden`, đặt `aria-label` "Nội dung tài liệu" (`ime.ts`, `BlueEditor.tsx`) |
| 2 | `aria-prohibited-attr` (4.1.2) | Khung trang `.bo-page` có `aria-label` nhưng không có role; nhãn tiếng Anh cố định | Mọi trạng thái BlueDoc | `role="group"`, nhãn dịch "Trang n" (`DocumentView.ts`, khoá `a11y.page`) |
| 3 | `color-contrast` (1.4.3) | Mục đang chọn của cột điều hướng màn hình Tệp khi rê chuột: chữ tối trên nền accent 3,15:1 (BlueDoc) / 3,74:1 (BlueSheet) | Tệp › Lưu thành | Mục hiện tại giữ nền sáng khi hover (`Backstage.module.css`) |
| 4 | `aria-required-parent` (1.3.1) | Bảng chọn giãn dòng: 16 `menuitemradio` không nằm trong `menu` | BlueDoc | `role="menu"` + nhóm `role="group"` có tên (`FontControls.tsx`) |
| 5 | `target-size` (2.5.8) | Nút "Tuỳ chọn cho <kiểu>" trong ngăn Kiểu 22 × 22 px | BlueDoc | 24 × 24 px (`StylesPane.module.css`) |
| 6 | `aria-required-children` (1.3.1) | Thanh thẻ trang tính `role="tablist"` chứa nút "Thêm trang tính" | Mọi trạng thái BlueSheet | Nút ra ngoài danh sách thẻ (`BlueSheetEditor.tsx`, `.tabList`) |
| 7 | `color-contrast` (1.4.3) | Ô "Tìm lệnh" trên thanh tiêu đề: chữ trắng trên nền teal pha trắng 4,01:1 | BlueSheet | Nền pha tối + viền sáng: ≥ 6,5:1 (`TitleBar.module.css`) |
| 8 | `color-contrast` (1.4.3) | Chữ của thẻ trang tính đang chọn `#0F9D8A` trên trắng 3,38:1 | BlueSheet | Dùng token `--bo-sheet-accent-text` (5,24:1) |
| 9 | `color-contrast` (1.4.3) | Nút chính (OK) của hộp thoại BlueSheet: trắng trên `#0F9D8A` 3,38:1 | Định dạng ô, Tìm | Nền `--bo-sheet-accent-strong` (5,24:1) |
| 10 | `target-size` (2.5.8) | Ô nhập lưới dữ liệu của hộp thoại biểu đồ cao 21 px (19 ô) | BlueSlide | Cao tối thiểu 24 px (`SlideEditor.module.css`) |
| 11 | `target-size` (2.5.8) | Dòng của ngăn Vùng chọn / Hoạt hình cao 23 px | BlueSlide | Cao tối thiểu 24 px |
| 12 | `target-size` (2.5.8) | Hộp chọn chủ đề / mật độ ở Tệp › Tùy chọn cao 19 px | BlueSlide (cả ba ứng dụng dùng chung) | Điều khiển của trang Tệp cao tối thiểu 24 px (`Backstage.module.css`) |

Sau khi sửa: **0 vi phạm** trên mọi trạng thái của §2, ở cả ba chủ đề.

### 3.2 Đi qua bằng bàn phím (`a11y-keyboard.spec.ts`)

Kịch bản mỗi ứng dụng: vào tài liệu → Alt (KeyTips) → thẻ → chạy một lệnh → bàn phím ở lại trong tài liệu → F6 tới
ribbon (focus nhìn thấy được) → F6 tới thanh trạng thái → F6 về tài liệu → mở một hộp thoại bằng phím (Ctrl+H, hoặc
KeyTip) → focus vào trong hộp thoại → Esc → focus về đúng chỗ cũ.

| # | Tiêu chí | Trước | Sửa |
|---|---|---|---|
| 13 | 2.1.1 Bàn phím | F6 chỉ đi qua các ngăn tác vụ, không tới ribbon / thanh trạng thái | Ribbon và thanh trạng thái là vùng F6 (`data-f6-region`); thứ tự: tài liệu → ngăn → ribbon (thẻ đang chọn) → thanh trạng thái → tài liệu (`TaskPanes.tsx`) |
| 14 | 2.4.3 Thứ tự focus | Đóng hộp thoại BlueDoc mở bằng KeyTip (Phông chữ): focus rơi về `<body>` | `closeDialog` đặt lại focus sau khi hộp thoại đã gỡ (`BlueEditor.tsx`) |
| 15 | 2.4.3 Thứ tự focus | Đóng hộp thoại BlueSlide (Cỡ slide) bằng Esc: focus rơi về `<body>` | `Dialog` dùng chung trả focus cho phần tử đã mở nó khi focus bị mất (`packages/ui/src/Dialog.tsx`) |
| 16 | 2.4.7 Focus nhìn thấy được | Gốc BlueSheet thiếu lớp `bo-root` nên quy tắc vòng focus chung không áp dụng | Thêm `bo-root` (`BlueSheetEditor.tsx`) |
| 17 | 2.5.7 Thao tác kéo | Sắp xếp lại slide chỉ bằng kéo hình thu nhỏ | Lệnh "Chuyển slide lên / xuống" (Trang đầu › Slide, Ctrl+Shift+↑ / ↓) |
| 18 | 2.5.7 Thao tác kéo | Sắp xếp lại trang tính chỉ bằng kéo thẻ | Lệnh "Chuyển trang tính sang trái / phải" (Trang đầu › Trang tính) |

Sau khi sửa: 3/3 kịch bản đạt.

### 3.3 Rà soát các tiêu chí khác của WCAG 2.2 (trong mã)

- **2.5.7 Thao tác kéo** — đã có cách khác: chỉnh độ rộng ngăn tác vụ bằng phím mũi tên / Home / End trên thanh chia
  (`role="separator"`, test `resizes a dock with the splitter`); di chuyển ngăn nổi bằng nút neo trái / phải / nổi;
  thước kẻ: lệnh Tăng/Giảm thụt lề và Bố cục › Lề / Thiết lập trang; hình trên slide: Sắp xếp › Căn, ngăn Vùng chọn.
  **Còn thiếu**: độ rộng cột / chiều cao hàng BlueSheet chỉ có kéo hoặc "Tự khớp" (chưa có hộp thoại nhập số).
- **2.4.11 Focus không bị che** — các hộp thoại là modal (Radix) ở giữa màn hình; thanh trạng thái 24 px cố định
  không che ô nhập. Chưa có kiểm tra tự động; mục kiểm tra tay ở §5.
- **3.2.6 Trợ giúp nhất quán** — ô "Tìm lệnh" (Alt+Q) và mục Tệp › Tùy chọn ở cùng vị trí trong cả ba ứng dụng (khung
  `TitleBar` / `Backstage` dùng chung).
- **3.3.7 Nhập lại thông tin** — thuộc tính tài liệu, mật khẩu lưu, tuỳ chọn được nhớ trong phiên; không có biểu mẫu
  nhiều bước hỏi lại cùng một thông tin.
- **3.3.8 Xác thực dễ tiếp cận** — BlueOffice không có màn hình đăng nhập riêng: web dùng OIDC (trang của IdP), desktop
  dùng OAuth 2.0 + PKCE trong trình duyệt hệ thống (`packages/hosted/src/signin.ts`) — trình duyệt cho phép dán mật
  khẩu / trình quản lý mật khẩu. Không có CAPTCHA hay câu đố. Trang đăng nhập Keycloak (nếu đơn vị dùng) là trách nhiệm
  cấu hình của đơn vị.

## 4. Thay đổi mã (tóm tắt)

`packages/canvas-host/src/{ime.ts,DocumentView.ts}`, `packages/editor/src/{BlueEditor.tsx,FontControls.tsx,StylesPane.module.css}`,
`packages/sheet-editor/src/{BlueSheetEditor.tsx,SheetEditor.module.css}`, `packages/slide-editor/src/{BlueSlideEditor.tsx,SlideEditor.module.css}`,
`packages/ui/src/{Dialog.tsx,TaskPanes.tsx,Ribbon.tsx,StatusBar.tsx,Backstage.module.css,TitleBar.module.css}`,
khoá i18n mới `a11y.*`, `slideEditor.moveSlideUp/Down`, `sheetEditor.moveSheetLeft/Right` (vi-VN + en-US).

## 5. Cần người kiểm tra với trình đọc màn hình (chưa làm được tự động)

Thực hiện trên Windows 11 với **NVDA 2024+** (Chrome, Edge) và **Narrator** (Edge, ứng dụng desktop WebView2), trên
macOS với **VoiceOver** (Safari, Chrome). Ghi lại phát âm thực tế cho từng bước:

1. **Đọc nội dung tài liệu BlueDoc.** Mở `tests/corpus/doc/phase1-sample.docx`, đưa con trỏ vào văn bản, dùng phím
   mũi tên / Ctrl+mũi tên: trình đọc có đọc chữ, từ, dòng không? Hiện chỉ có ô nhập "Nội dung tài liệu" (không chứa văn
   bản trang) → dự kiến **không đọc được nội dung** — cần xác nhận và quyết định thiết kế lớp mirror văn bản
   (đề xuất: `aria-live` đọc đoạn tại con trỏ, hoặc cây DOM ẩn theo trang).
2. **Chế độ duyệt (NVDA Browse / VoiceOver VO+mũi tên)** trên trang: các trang "Trang n", ảnh có nhãn, chú thích có
   được liệt kê không; có bẫy focus nào không.
3. **BlueSheet**: di chuyển ô bằng mũi tên — trình đọc có đọc địa chỉ ô và giá trị không (ô đang nhập "Ô đang nhập" +
   hộp Tên)? Đọc công thức ở thanh công thức?
4. **BlueSlide**: Tab qua các hình trên slide, ngăn Vùng chọn — tên hình, thứ tự, trạng thái chọn (`aria-selected`)
   được đọc đúng không; nhập chữ vào hình.
5. **Ribbon**: tên thẻ, trạng thái "đã nhấn" của nút bật/tắt (`aria-pressed`), nút mở bảng chọn (`aria-haspopup`,
   `aria-expanded`), KeyTips (Alt) — có được thông báo không.
6. **Hộp thoại**: tiêu đề được đọc khi mở; focus đầu tiên hợp lý; Esc đóng và trình đọc thông báo vị trí trở về.
7. **Thông báo động**: thanh cảnh báo macro, lỗi "Không mở được tệp", kết quả Tìm (số kết quả), trạng thái lưu —
   có được đọc (vùng `role="status"` / `aria-live`) không.
8. **Tương phản cao của Windows (forced colors)**: bật chủ đề Tương phản cao của Windows, kiểm tra vòng focus, nút
   đang nhấn, thẻ đang chọn, vùng chọn trong tài liệu còn nhìn thấy (`theme.css` → `@media (forced-colors: active)`).
9. **Phóng to 200% / 400% (1.4.4, 1.4.10)**: zoom trình duyệt 200% và cửa sổ 320 px CSS — ribbon thu gọn, không mất
   chức năng, không cuộn hai chiều ở khung giao diện.
10. **2.4.11 Focus không bị che**: Tab qua các ô nhập ở cuối các ngăn tác vụ và hộp thoại dài khi cửa sổ thấp
    (≤ 600 px) — phần tử có focus không bị thanh trạng thái / ngăn nổi che hoàn toàn.
11. **Desktop (Tauri, WebView2)**: lặp lại 1, 5, 6 với Narrator trên `bluedoc.exe`.
