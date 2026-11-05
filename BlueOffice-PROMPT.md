# DỰ ÁN: BlueOffice — Bộ ứng dụng văn phòng tương thích định dạng Microsoft Office

> File này là prompt gốc cho Claude Code. Đặt ở thư mục gốc repo, Claude Code đọc toàn bộ trước khi làm bất cứ việc gì.

---

## 0. Vai trò & cách làm việc

Bạn là kiến trúc sư phần mềm kiêm lead engineer của BlueOffice. Bạn xây sản phẩm từ đầu, theo từng phase. Mỗi phase phải build được, chạy được, có test tự động.

Quy trình bắt buộc cho mỗi phase:
1. Đọc `CLAUDE.md`, `docs/ARCHITECTURE.md`, `docs/PROGRESS.md`.
2. Viết kế hoạch vào `docs/plan/phase-N.md`: mục tiêu, danh sách file tạo/sửa, API giữa các module, test sẽ viết, tiêu chí hoàn thành.
3. Thực hiện theo kế hoạch, commit nhỏ (conventional commits).
4. Chạy toàn bộ test (Rust + TS + .NET), sửa đến khi xanh.
5. Cập nhật `docs/PROGRESS.md`, `CLAUDE.md` (lệnh build/test mới, quyết định kiến trúc mới).
6. Dừng lại, báo cáo kết quả, chờ duyệt trước khi sang phase tiếp theo.

Khi gặp quyết định kiến trúc quan trọng chưa có trong file này: ghi vào `docs/adr/NNNN-ten-quyet-dinh.md` (Architecture Decision Record) kèm lý do và phương án đã loại.

---

## 1. Nguyên tắc clean-room (BẮT BUỘC, không ngoại lệ)

- KHÔNG decompile, disassemble, debug-attach, hay phân tích binary/DLL của Microsoft Office dưới bất kỳ hình thức nào.
- KHÔNG sao chép mã nguồn, icon, hình ảnh, font độc quyền, template, chuỗi giao diện, tên thương hiệu (Word, Excel, PowerPoint, Office, Microsoft 365) vào sản phẩm.
- Nguồn kỹ thuật hợp lệ duy nhất:
  - ECMA-376 (Office Open XML) 5th edition / ISO/IEC 29500 (Part 1–4)
  - Microsoft Open Specifications (công bố công khai): [MS-DOCX], [MS-XLSX], [MS-PPTX], [MS-OI29500], [MS-OE376], [MS-DOC], [MS-XLS], [MS-XLSB], [MS-PPT], [MS-ODRAW], [MS-ODRAWXML], [MS-OFFCRYPTO], [MS-CFB], [MS-OLEPS], [MS-OVBA] (chỉ để giữ nguyên macro, không thực thi)
  - OASIS OpenDocument 1.3 (ODF)
  - Tài liệu công khai mô tả cú pháp & ngữ nghĩa hàm bảng tính → tự cài đặt lại
  - Quan sát hành vi bên ngoài (black-box): mở file mẫu và so sánh kết quả hiển thị/tính toán
- Thư viện bên thứ ba: chỉ license MIT / Apache-2.0 / BSD / MPL-2.0 / Zlib / OFL (font). KHÔNG dùng mã GPL / AGPL / LGPL-static. Mọi dependency ghi vào `THIRD_PARTY_NOTICES.md` (tên, phiên bản, license, mục đích). Thêm CI job `cargo deny` + `license-checker` để chặn license không hợp lệ.
- Giao diện: bố cục thanh công cụ dạng tab (ribbon-style) là được, nhưng icon, màu sắc, logo, nhận diện là của BlueOffice. Icon dùng Lucide (ISC) hoặc Fluent UI System Icons (MIT).

---

## 2. Mục tiêu sản phẩm

Ba ứng dụng dùng chung core:

| App | Định dạng | Ghi chú |
|---|---|---|
| **BlueDoc** (văn bản) | .docx đọc/ghi, .doc đọc, .odt, .rtf, .txt, .md, xuất PDF | Ưu tiên số 1 |
| **BlueSheet** (bảng tính) | .xlsx đọc/ghi, .xls & .xlsb đọc, .csv, .ods, xuất PDF | Ưu tiên số 2 |
| **BlueSlide** (trình chiếu) | .pptx đọc/ghi, .ppt đọc, .odp, xuất PDF/PNG | Ưu tiên số 3 |

### Quy tắc đặt tên sản phẩm
- Họ sản phẩm: **BlueDrive · BlueDoc · BlueSheet · BlueSlide**; giai đoạn sau: **BlueForm**, **BlueNote**. Không có chữ "s" ở cuối.
- Tên hiển thị tiếng Việt: "BlueDoc – Soạn thảo văn bản", "BlueSheet – Bảng tính", "BlueSlide – Trình chiếu".
- App id desktop: `vn.bluestar.bluedoc`, `vn.bluestar.bluesheet`, `vn.bluestar.blueslide`. Route web: `/doc`, `/sheet`, `/slide` (route cũ `/write`, `/docs`, `/sheets`, `/slides` chuyển hướng).
- KHÔNG dùng tên, nhãn hiệu, icon hay màu nhận diện của Microsoft hoặc Google (Word, Excel, PowerPoint, Office, Google Docs/Sheets/Slides/Drive) làm tên sản phẩm, tính năng hay icon; chỉ được nhắc ở dạng mô tả tương thích, ví dụ "Tương thích định dạng Microsoft Word (.docx)".
- Icon và bảng màu của từng app là thiết kế riêng của Bluestar.

Mục tiêu tương thích: mở → chỉnh sửa → lưu file mà Microsoft Office mở lại không báo lỗi, không mất dữ liệu (lossless round-trip), hiển thị sai lệch tối thiểu.

Hình thức triển khai:
- **Web** (trình duyệt) — cộng tác thời gian thực, cài đặt on-premise cho cơ quan.
- **Desktop** (Windows/macOS/Linux) — chạy offline, mở file trực tiếp từ ổ đĩa, đăng ký file association.
- **Server** — lưu trữ, phân quyền, cộng tác, chuyển đổi định dạng hàng loạt, xuất PDF headless.
- **Nhúng vào BlueDrive** (nền tảng lưu trữ & chia sẻ file của Bluestar, tích hợp ở giai đoạn sau) — người dùng BlueDrive bấm vào file .docx/.xlsx/.pptx là mở và sửa ngay trong BlueOffice, trên web lẫn desktop. Kiến trúc phải sẵn sàng cho việc này NGAY TỪ PHASE 1 (xem mục 3.4 và mục 6A).

**Yêu cầu cứng: web và desktop là MỘT sản phẩm, không phải hai.** Cùng một codebase React + cùng một core Rust; bản desktop là bản web chạy trong Tauri cộng thêm khả năng native. Mọi tính năng soạn thảo phải chạy giống hệt nhau trên cả hai; chỉ khác ở lớp nền tảng (mục 3.4).

Đặc thù Việt Nam:
- Tiếng Việt đầy đủ (Unicode NFC, dấu tổ hợp, gõ Telex/VNI qua IME hệ điều hành và IME trình duyệt — xử lý composition events đúng).
- Mẫu văn bản hành chính theo Nghị định 30/2020/NĐ-CP (quốc hiệu, tiêu ngữ, số ký hiệu, nơi nhận, khổ A4, lề chuẩn).
- Font fallback metric-compatible khi máy không có font gốc: Times New Roman → Liberation Serif/Tinos, Arial → Liberation Sans/Arimo, Calibri → Carlito, Cambria → Caladea (đều license OFL/Apache).
- Định dạng số/ngày vi-VN (dấu chấm hàng nghìn, dấu phẩy thập phân, dd/MM/yyyy, đơn vị ₫).
- Chuyển đổi bảng mã TCVN3 (ABC) / VNI-Windows → Unicode khi mở file cũ.

---

## 3. Stack công nghệ (đã chốt)

### 3.1. Tổng quan

```
┌────────────────────────────────────────────────────────────────┐
│  UI: React 18 + TypeScript + Vite                              │
│  (ribbon, dialog, panel — DOM)   +   Canvas/WebGL (vùng tài liệu)│
├────────────────────────────────────────────────────────────────┤
│  Bridge: wasm-bindgen (web)  |  Tauri commands (desktop)        │
├────────────────────────────────────────────────────────────────┤
│  CORE ENGINE: Rust (một codebase, biên dịch ra WASM + native)   │
│  document model · OOXML/binary I/O · text shaping · layout      │
│  formula engine · drawing/chart · render command list · PDF     │
├────────────────────────────────────────────────────────────────┤
│  Desktop shell: Tauri 2 (Rust native, gọi core trực tiếp)       │
│  Backend: ASP.NET Core 8 + PostgreSQL + MinIO + Redis           │
│  Worker chuyển đổi/xuất PDF: binary Rust headless (CLI/gRPC)    │
└────────────────────────────────────────────────────────────────┘
```

### 3.2. Lý do chọn

- **Rust cho core**: dàn trang văn bản, engine công thức 1 triệu ô, parse file 50MB cần hiệu năng gần C++ nhưng an toàn bộ nhớ (parse file không tin cậy từ người dùng). Một codebase chạy ở cả 3 nơi: WASM trong trình duyệt, native trong Tauri desktop, native trong worker server → hiển thị và xuất PDF nhất quán tuyệt đối.
- **React + TypeScript cho UI**: phát triển giao diện nhanh, hệ sinh thái component lớn, đội Bluestar đã quen React.
- **Canvas cho vùng tài liệu** (không dùng contentEditable): kiểm soát hoàn toàn dàn trang, phân trang, hiển thị giống nhau trên mọi trình duyệt (cách Google Docs, ONLYOFFICE đang làm). Nhập liệu qua một `<textarea>` ẩn để nhận IME tiếng Việt.
- **Tauri 2 cho desktop**: bản cài nhỏ (~10–20MB so với ~150MB của Electron), shell viết bằng Rust nên gọi core native không qua WASM.
- **ASP.NET Core cho backend**: đội Bluestar mạnh .NET; phù hợp hạ tầng Windows Server/Linux của cơ quan nhà nước; DocumentFormat.OpenXml SDK (MIT) dùng để validate file xuất ra.
- **Yrs (Rust port của Yjs, MIT) cho cộng tác**: CRDT chạy trong core, đồng bộ qua WebSocket; tương thích giao thức Yjs.

### 3.3. Thư viện cụ thể

**Rust (core)**
- `quick-xml` — parse/ghi XML streaming
- `zip` — gói OPC
- `cfb` — Compound File Binary cho .doc/.xls/.ppt
- `rustybuzz` — text shaping (port HarfBuzz)
- `ttf-parser` / `skrifa` — đọc font, metric
- `unicode-linebreak`, `unicode-bidi`, `unicode-segmentation`, `unicode-normalization`
- `hyphenation` — ngắt từ (tiếng Anh; tiếng Việt không ngắt từ)
- `tiny-skia` — rasterize headless (xuất PNG, test visual)
- `pdf-writer` — xuất PDF (tự nhúng subset font bằng `subsetter`)
- `image` — decode ảnh PNG/JPEG/GIF/BMP/TIFF; EMF/WMF tự viết bộ chuyển sang lệnh vẽ theo [MS-EMF]/[MS-WMF]
- `aes`, `sha2`, `cbc` — giải mã file có mật khẩu ([MS-OFFCRYPTO])
- `yrs` — CRDT
- `wasm-bindgen`, `serde`, `serde-wasm-bindgen`
- `chrono` — ngày giờ (serial date của bảng tính tự cài: hệ 1900 có lỗi năm nhuận 1900 và hệ 1904)
- Test: `insta` (snapshot), `proptest` (property-based), `criterion` (benchmark)

**TypeScript (UI)**
- React 18, Vite, Zustand (state UI), TanStack Query (gọi API)
- Radix UI primitives (MIT) + CSS Modules; icon Lucide
- `i18next` (vi-VN mặc định, en-US)
- Test: Vitest, Playwright (E2E)

**.NET (backend)**
- ASP.NET Core 8 Minimal API + SignalR hoặc WebSocket thuần cho đồng bộ Yjs
- EF Core + Npgsql (PostgreSQL), Minio .NET SDK, StackExchange.Redis
- DocumentFormat.OpenXml (validate), Serilog, OpenTelemetry
- Xác thực: OIDC (Keycloak hoặc tương thích), JWT
- Test: xUnit, Testcontainers

### 3.4. Một codebase cho web + desktop (Platform Abstraction Layer)

Toàn bộ code UI và core KHÔNG được gọi trực tiếp API trình duyệt hay API Tauri cho các việc phụ thuộc nền tảng. Mọi thứ đi qua interface trong `packages/platform`:

```ts
interface Platform {
  kind: 'web' | 'desktop';
  files: FileAccess;          // mở/lưu file: web = upload/download + File System Access API (nếu có); desktop = ổ đĩa thật qua Tauri fs
  storage: DocumentStorage;   // nguồn tài liệu: LocalStorageProvider | BlueOfficeServerProvider | BlueDriveProvider | WopiProvider
  clipboard: ClipboardAccess; // web = Async Clipboard API (HTML + text); desktop = clipboard native nhiều định dạng
  fonts: FontSource;          // web = font đóng gói + font server cấp; desktop = thêm font hệ thống
  print: PrintService;        // web = sinh PDF rồi in qua trình duyệt; desktop = hộp thoại in hệ điều hành
  engine: EngineHost;         // web = core WASM trong Web Worker; desktop = core native qua Tauri command (nhanh hơn), fallback WASM
  auth: AuthSession;          // OIDC: web = redirect; desktop = PKCE qua trình duyệt hệ thống + deep link
  window: WindowManager;      // web = tab trình duyệt; desktop = nhiều cửa sổ native
  offline: OfflineCache;      // web = IndexedDB + Service Worker (PWA); desktop = thư mục cache cục bộ
  shell: ShellIntegration;    // desktop: file association, recent files, deep link `bluedoc://` · `bluesheet://` · `blueslide://`; web: no-op
}
```

- `apps/web` khởi tạo `WebPlatform`, `apps/desktop` khởi tạo `DesktopPlatform`; phần còn lại dùng chung 100%.
- `EngineHost` có cùng một API dù chạy WASM hay native: các crate core không biết mình đang chạy ở đâu.
- Bản web chạy được dạng PWA, sửa offline, đồng bộ khi có mạng (dùng chung cơ chế CRDT ở `bo-collab`).
- Mỗi tính năng mới phải có test E2E chạy trên cả hai target (Playwright cho web, WebDriver của Tauri cho desktop) trong CI.

### 3.5. Thiết kế sẵn để nhúng (embeddable editor)

BlueOffice phải nhúng được vào ứng dụng khác (BlueDrive trước tiên) mà không sửa code lõi:
- Editor là một package độc lập `@blueoffice/editor` (React component) + bản build standalone chạy trong iframe.
- Mọi thông tin về "file đến từ đâu, ai đang mở, quyền gì, lưu về đâu" đi qua `DocumentStorage` + `AuthSession`, không hard-code vào editor.
- Branding, menu File, nút Share, nút Close… cấu hình được bởi bên nhúng (host).

---

## 4. Cấu trúc repo

```
blueoffice/
├─ CLAUDE.md
├─ THIRD_PARTY_NOTICES.md
├─ Cargo.toml                    # Rust workspace
├─ package.json / pnpm-workspace.yaml
├─ BlueOffice.sln                # .NET solution
├─ crates/
│  ├─ bo-model/        # Mô hình tài liệu: Document, Section, Paragraph, Run, Table, Drawing, Sheet, Cell, Slide...
│  ├─ bo-opc/          # Gói OPC: content types, relationships, parts, giữ opaque parts
│  ├─ bo-docx/         # WordprocessingML <-> bo-model
│  ├─ bo-xlsx/         # SpreadsheetML <-> bo-model
│  ├─ bo-pptx/         # PresentationML <-> bo-model
│  ├─ bo-drawingml/    # DrawingML: shapes, preset geometry, theme, chart
│  ├─ bo-binary/       # .doc / .xls / .xlsb / .ppt (đọc)
│  ├─ bo-odf/          # .odt / .ods / .odp
│  ├─ bo-crypto/       # [MS-OFFCRYPTO]
│  ├─ bo-text/         # shaping, font manager, fallback, line breaking
│  ├─ bo-layout/       # dàn trang văn bản → layout tree (pages, lines, boxes)
│  ├─ bo-formula/      # parser, AST, evaluator, dependency graph, hàm
│  ├─ bo-render/       # layout tree → display list (lệnh vẽ độc lập backend)
│  ├─ bo-raster/       # display list → PNG (tiny-skia)
│  ├─ bo-pdf/          # display list → PDF
│  ├─ bo-collab/       # ánh xạ bo-model <-> Yrs, operations, undo manager
│  ├─ bo-edit/         # lệnh chỉnh sửa (command), selection, transaction, undo/redo
│  ├─ bo-wasm/         # bindings cho web
│  └─ bo-cli/          # công cụ dòng lệnh: convert, render, validate, fidelity-report
├─ apps/
│  ├─ web/             # React app (3 app chung shell, route /doc /sheet /slide)
│  ├─ desktop/         # Tauri 2
│  └─ server/          # ASP.NET Core
│     ├─ BlueOffice.Api/
│     ├─ BlueOffice.Domain/
│     ├─ BlueOffice.Infrastructure/
│     └─ BlueOffice.Tests/
├─ packages/
│  ├─ platform/        # Platform Abstraction Layer (interface + WebPlatform + DesktopPlatform)
│  ├─ editor/          # @blueoffice/editor — editor nhúng được (React component + bản iframe)
│  ├─ editor-sdk/      # @blueoffice/editor-sdk — SDK cho ứng dụng host (BlueDrive) điều khiển editor qua postMessage
│  ├─ storage-providers/ # Local, BlueOfficeServer, BlueDrive, WOPI
│  ├─ ui/              # ribbon, dialog, color/font picker, panel
│  ├─ canvas-host/     # quản lý canvas, nhận display list từ WASM, vẽ, hit-test, IME textarea
│  └─ i18n/
├─ tests/
│  ├─ corpus/          # file mẫu (README ghi nguồn gốc từng file)
│  ├─ fidelity/        # ảnh tham chiếu + kết quả so sánh
│  └─ e2e/
└─ docs/
   ├─ ARCHITECTURE.md
   ├─ PROGRESS.md
   ├─ adr/
   ├─ plan/
   └─ spec-notes/      # ghi chú đọc spec theo từng phần tử
```

---

## 5. Nguyên tắc thiết kế cốt lõi

1. **Lossless round-trip**: phần tử/thuộc tính XML nào chưa hiểu phải giữ nguyên (lưu dạng opaque node gắn vào node cha gần nhất, kể cả `mc:AlternateContent`, `w14:*`, `x14:*`, extension lists) và ghi lại đúng vị trí khi lưu. Phần tử bị người dùng xoá thì opaque con của nó bị xoá theo.
2. **Mô hình tài liệu bất biến + transaction**: mọi chỉnh sửa là một `Transaction` gồm các `Op` nguyên tử → undo/redo không giới hạn, ánh xạ 1-1 sang thao tác CRDT.
3. **Đơn vị**: lưu nội bộ đúng đơn vị spec (twips, EMU, half-points); chỉ quy đổi sang pixel ở tầng render, có hệ số zoom & devicePixelRatio.
4. **Layout tăng dần (incremental)**: sửa một đoạn chỉ dàn lại đoạn đó và các trang bị ảnh hưởng; cache kết quả shaping theo (font, size, text).
5. **Render qua display list**: core sinh danh sách lệnh vẽ (rect, path, glyph run, image, clip, transform); Canvas web, rasterizer PNG và PDF cùng tiêu thụ một display list → hiển thị = in = xuất PDF.
6. **Ảo hoá**: chỉ dàn trang/vẽ vùng đang nhìn thấy + đệm; bảng tính chỉ lưu ô có dữ liệu (sparse).
7. **Đa luồng**: web dùng Web Worker chạy WASM (layout, tính công thức) để UI không giật; desktop dùng thread pool (rayon).
8. **Command registry**: mọi chức năng là một command có id (vd `format.bold`, `insert.table`), ribbon/phím tắt/menu chuột phải/command palette đều gọi qua registry; trạng thái (enabled/checked) tính từ selection.
9. **An toàn khi mở file**: giới hạn kích thước giải nén (chống zip bomb), giới hạn độ sâu XML, không thực thi macro, không tải tài nguyên ngoài (external link) khi chưa được người dùng cho phép.

### Mục tiêu hiệu năng (đo bằng benchmark trong CI)
- Mở .docx 300 trang có ảnh: < 3 s (desktop), < 5 s (web)
- Gõ phím → hiển thị: < 16 ms ở tài liệu 300 trang
- Mở .xlsx 1 triệu ô có 100.000 công thức: < 5 s; sửa 1 ô → tính lại phụ thuộc: < 100 ms
- Cuộn 60 fps
- Bộ nhớ WASM < 1 GB cho các file trên

---

## 6. Lộ trình theo phase

### Phase 1 — Nền tảng
- Khởi tạo Rust workspace, pnpm workspace, .NET solution; CI GitHub Actions (build, test, lint: clippy, rustfmt, eslint, prettier, dotnet format, cargo deny).
- `bo-opc`: đọc/ghi gói OPC, giữ nguyên part không hiểu.
- `bo-text`: font manager (nạp font hệ thống + font nhúng kèm app), fallback, shaping với rustybuzz, đo metric.
- `bo-layout` bản đầu: đoạn văn nhiều run, căn trái/phải/giữa/đều, giãn dòng, thụt lề, ngắt dòng Unicode, ngắt trang đơn giản.
- `bo-render` + `bo-raster` + `bo-pdf` bản đầu.
- `bo-wasm` + `packages/canvas-host`: hiển thị một tài liệu mẫu trên canvas, zoom, cuộn.
- Shell UI: ribbon tab, thanh trạng thái, giao diện sáng/tối, đa ngôn ngữ.
- `packages/platform`: định nghĩa đủ interface ở mục 3.4, cài `WebPlatform` và `DesktopPlatform` bản đầu (files, engine, fonts, clipboard). Cùng một file mẫu mở được trên web và desktop, hiển thị giống nhau (test visual so sánh 2 target).
- `packages/editor` tách riêng khỏi `apps/web` ngay từ đầu: `apps/web` và `apps/desktop` chỉ là vỏ nhúng editor.
- Interface `DocumentStorage` + `LocalStorageProvider` (mở/lưu file cục bộ).
- `bo-cli render input.docx --out page-%d.png` và `convert --to pdf`.

### Phase 2 — BlueDoc MVP
**Đọc/ghi .docx**
- styles.xml: docDefaults, kế thừa style (basedOn), paragraph/character/table/numbering style, latentStyles giữ nguyên
- Thuộc tính run: font (ascii/hAnsi/eastAsia/cs), size, bold, italic, underline (các kiểu), strike, màu, highlight, shading, sup/sub, caps, small caps, spacing, kerning
- Thuộc tính đoạn: căn lề, thụt lề (first-line/hanging), spacing before/after, line spacing (auto/exact/atLeast), keepNext, keepLines, widowControl, pageBreakBefore, tab stops, border, shading
- numbering.xml: list nhiều cấp, abstractNum, lvlOverride, định dạng số (decimal, lowerLetter, upperRoman…), nối tiếp/khởi động lại
- Sections: khổ giấy, hướng, lề, cột, header/footer (first/even/default), section break các loại
- Bảng: grid, merge ngang/dọc, độ rộng (auto/fixed/pct), border, shading, lặp hàng tiêu đề, bảng lồng nhau, ngắt hàng qua trang
- Ảnh & drawing: inline và anchored (wrap square/tight/top-bottom/behind/in-front), crop, xoay
- Hyperlink, bookmark, footnote/endnote, field (PAGE, NUMPAGES, DATE, TOC hiển thị kết quả cache)
- Text box, shape đơn giản

**Chỉnh sửa**
- Con trỏ, chọn vùng (chuột, Shift+mũi tên, Ctrl+A, chọn từ/đoạn bằng double/triple click), gõ, xoá, IME tiếng Việt
- Copy/paste: nội bộ (giữ định dạng), HTML từ trình duyệt/ứng dụng khác, plain text; Paste Special
- Định dạng ký tự & đoạn, style gallery, Format Painter, Clear Formatting
- Find/Replace (phân biệt hoa thường, cả từ, regex, tìm theo định dạng)
- Chèn: bảng, ảnh, ngắt trang, ngắt section, ký tự đặc biệt, header/footer, số trang
- Thước kẻ ngang/dọc, tab stop kéo thả
- Undo/redo, autosave, khôi phục khi crash
- Xuất PDF, in (desktop: hộp thoại in hệ điều hành; web: in PDF)

**Tiêu chí hoàn thành**: 95% file trong `tests/corpus/doc` round-trip không mất dữ liệu; visual diff trung bình < 2% pixel.

### Phase 3 — BlueSheet MVP
**Đọc/ghi .xlsx**
- workbook, worksheets, sharedStrings (rich text), styles (numFmts, fonts, fills, borders, cellXfs, cellStyles, dxfs)
- merge cells, độ rộng cột/chiều cao hàng, ẩn hàng/cột, outline/group, freeze/split panes
- defined names (workbook & sheet scope), print area, print titles, page setup
- hyperlink, comments/notes, data validation, conditional formatting (cellIs, expression, color scale, data bar, icon set)
- Bảng (ListObject/Table) và structured references
- Chart (đọc chartSpace → mô hình chart → render)

**Định dạng số**: tự cài parser mã định dạng (4 section, điều kiện `[>100]`, màu `[Red]`, ngày giờ, phần trăm, phân số, khoa học, `@`, locale `[$-42A]`, ký hiệu tiền tệ ₫).

**Engine công thức (`bo-formula`)**
- Lexer/parser: A1 & R1C1, tham chiếu tương đối/tuyệt đối/hỗn hợp, range, union, intersection, tham chiếu sheet khác `'Sheet 1'!A1`, 3D reference, defined names, structured references, array constants
- Kiểu giá trị: number (f64), string, boolean, error (#DIV/0!, #N/A, #NAME?, #NULL!, #NUM!, #REF!, #VALUE!, #SPILL!, #CALC!), array, reference
- Ép kiểu ngầm định theo đúng ngữ nghĩa bảng tính (chuỗi số, boolean trong phép toán, ô trống)
- Dependency graph, tính lại tăng dần (dirty propagation), tính song song các nhánh độc lập, phát hiện vòng lặp + iterative calculation tùy chọn
- Hàm volatile (NOW, TODAY, RAND, OFFSET, INDIRECT)
- Dynamic arrays & spill, LET, LAMBDA
- Serial date hệ 1900 (kể cả ngày 29/02/1900 giả) và hệ 1904
- So sánh số với độ chính xác 15 chữ số có nghĩa như bảng tính thông dụng

**Hàm đợt 1 (~200 hàm)**
- Math & Trig: SUM, SUMIF, SUMIFS, SUMPRODUCT, PRODUCT, ROUND, ROUNDUP, ROUNDDOWN, MROUND, INT, TRUNC, MOD, ABS, POWER, SQRT, EXP, LN, LOG, LOG10, CEILING(.MATH), FLOOR(.MATH), RAND, RANDBETWEEN, SEQUENCE, SUBTOTAL, AGGREGATE, PI, các hàm lượng giác…
- Logical: IF, IFS, AND, OR, NOT, XOR, IFERROR, IFNA, SWITCH, TRUE, FALSE, LET, LAMBDA
- Lookup & Reference: VLOOKUP, HLOOKUP, LOOKUP, INDEX, MATCH, XLOOKUP, XMATCH, CHOOSE, OFFSET, INDIRECT, ROW(S), COLUMN(S), ADDRESS, FILTER, SORT, SORTBY, UNIQUE, TRANSPOSE
- Text: LEFT, RIGHT, MID, LEN, FIND, SEARCH, SUBSTITUTE, REPLACE, UPPER, LOWER, PROPER, TRIM, CLEAN, TEXT, VALUE, CONCAT, CONCATENATE, TEXTJOIN, TEXTSPLIT, TEXTBEFORE, TEXTAFTER, REPT, EXACT, CHAR, CODE, UNICHAR, UNICODE, NUMBERVALUE
- Date & Time: DATE, DATEVALUE, TIME, TIMEVALUE, YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, TODAY, NOW, WEEKDAY, WEEKNUM, ISOWEEKNUM, EDATE, EOMONTH, DATEDIF, DAYS, NETWORKDAYS(.INTL), WORKDAY(.INTL), YEARFRAC
- Statistical: AVERAGE, AVERAGEA, AVERAGEIF(S), COUNT, COUNTA, COUNTBLANK, COUNTIF(S), MAX, MIN, MAXIFS, MINIFS, MEDIAN, MODE.SNGL, LARGE, SMALL, RANK.EQ, RANK.AVG, STDEV.S, STDEV.P, VAR.S, VAR.P, PERCENTILE.INC, QUARTILE.INC, CORREL, FORECAST.LINEAR, NORM.DIST, NORM.INV…
- Financial: PMT, IPMT, PPMT, PV, FV, NPV, XNPV, IRR, XIRR, RATE, NPER, SLN, DB, DDB
- Information: ISBLANK, ISNUMBER, ISTEXT, ISERROR, ISERR, ISNA, ISLOGICAL, ISREF, TYPE, N, NA, CELL (cơ bản)
- Mỗi hàm: file test riêng gồm bảng (công thức, input, kết quả kỳ vọng), kể cả trường hợp lỗi và biên. Kết quả kỳ vọng do người kiểm thử cung cấp (ghi nguồn), không tự đoán.

**Giao diện & chỉnh sửa**
- Grid ảo hoá vẽ trên canvas, thanh công thức, Name Box, tab sheet (thêm/xoá/đổi tên/kéo thả/màu tab)
- Chọn vùng nhiều range (Ctrl), autofill (chuỗi số, ngày, thứ, tháng, danh sách tự định nghĩa), Flash Fill để sau
- Chèn/xoá hàng cột (cập nhật tham chiếu công thức đúng), copy/paste có điều chỉnh tham chiếu tương đối, Paste Special (values/formats/formulas/transpose)
- Sort nhiều cấp, AutoFilter, Find/Replace, Go To
- Định dạng ô (hộp thoại Format Cells), cell styles, conditional formatting, data validation (dropdown)
- Biểu đồ: cột, thanh, đường, vùng, tròn, doughnut, scatter, combo; chỉnh tiêu đề/trục/chú giải
- Xuất PDF theo page setup, in tiêu đề lặp lại

**Tiêu chí hoàn thành**: 100% test hàm đợt 1 pass; round-trip 95% file corpus; benchmark đạt mục tiêu mục 5.

### Phase 4 — BlueSlide MVP
- Đọc/ghi .pptx: presentation, slide master, slide layout, slide, notes; kế thừa placeholder master → layout → slide; theme (color scheme, font scheme, format scheme)
- Text body: autofit (normAutofit, shrink), bullet nhiều cấp, columns trong text box
- Shapes: toàn bộ preset geometry (tự cài từ định nghĩa hình học trong ECMA-376 Part 1, Annex/presetShapeDefinitions — đây là tài liệu chuẩn công khai), custom geometry, fill (solid/gradient/pattern/picture), line, shadow, group, connector
- Ảnh, video/audio (giữ nguyên, phát trên desktop/web nếu codec hỗ trợ), bảng, chart (dùng chung `bo-drawingml`)
- Chế độ: Normal (thumbnail + slide + notes), Slide Sorter, Reading View, trình chiếu toàn màn hình, Presenter View (màn hình 2: slide kế tiếp, ghi chú, đồng hồ)
- Transition & animation cơ bản (fade, push, wipe, appear, fly-in), timeline đơn giản
- Smart guides, căn chỉnh/phân bố đối tượng, nhóm/bỏ nhóm, thứ tự lớp
- Xuất PDF, PNG từng slide

### Phase 5 — Định dạng cũ & tính năng nâng cao
- `bo-binary`: đọc .doc ([MS-DOC]: FIB, piece table, CHP/PAP/SEP, stylesheet, list), .xls ([MS-XLS]: BIFF8 records, SST, formula tokens), .xlsb ([MS-XLSB]), .ppt ([MS-PPT]) → bo-model. Lưu lại dưới dạng OOXML.
- `bo-crypto`: mở/lưu file có mật khẩu (Agile Encryption, Standard Encryption), bảo vệ sheet/workbook
- Track changes (ins/del/rPrChange/pPrChange): hiển thị, accept/reject, ghi nhận thay đổi mới
- Comments có luồng trả lời
- Mục lục tự động (TOC field cập nhật), caption, cross-reference, mail merge (nguồn CSV/XLSX)
- PivotTable: đọc cache + hiển thị, tạo pivot cơ bản
- SmartArt: hiển thị bằng drawing fallback có sẵn trong file
- ODF đọc/ghi
- Bảng mã TCVN3/VNI → Unicode
- Macro VBA: giữ nguyên khi lưu, cảnh báo người dùng, KHÔNG thực thi

### Phase 6 — Cộng tác, server & triển khai
- `bo-collab`: ánh xạ mô hình sang Yrs; thao tác đồng thời trên văn bản, ô bảng tính, đối tượng slide
- Server ASP.NET Core:
  - API quản lý file/thư mục, upload/download, phiên bản (version history, khôi phục)
  - WebSocket relay đồng bộ Yjs, lưu snapshot định kỳ vào PostgreSQL/MinIO
  - Presence: con trỏ & vùng chọn của người khác, danh sách người đang xem
  - Chia sẻ & phân quyền (owner/editor/commenter/viewer), link chia sẻ có hạn
  - SSO OIDC, nhật ký truy cập (audit log)
  - Worker chuyển đổi: gọi `bo-cli` (convert, xuất PDF, sinh thumbnail) qua hàng đợi Redis
- Ký số: ký PDF (PAdES) và ký OOXML (XML-DSig theo spec), chuẩn bị điểm tích hợp USB token / ký số từ xa của CA Việt Nam
- Triển khai on-premise: Docker Compose + hướng dẫn cài đặt, sao lưu, nâng cấp; tài liệu đáp ứng yêu cầu an toàn thông tin cấp độ
- Desktop: auto-update (Tauri updater), file association, mở nhiều cửa sổ, recent files

### Phase 6A — Tích hợp BlueDrive

Mục tiêu: BlueDrive dùng BlueOffice làm trình xem/sửa mặc định cho file văn phòng, trên web và desktop, giống trải nghiệm OneDrive + Office trên web.

**1. Giao thức host ↔ editor (phía server)**
- Định nghĩa "BlueOffice Host Protocol" (BOHP) dạng REST, được BlueDrive cài đặt:
  - `GET  /files/{id}` → metadata: tên, kích thước, version, owner, quyền của người dùng hiện tại (view/comment/edit/share/download), URL chia sẻ, thông tin người dùng (id, tên, avatar)
  - `GET  /files/{id}/contents` → nội dung file
  - `POST /files/{id}/contents` (header If-Match: version) → lưu, trả version mới; 409 nếu xung đột
  - `POST /files/{id}/lock`, `/unlock`, `/refresh-lock` → khoá khi đang sửa (có hạn, gia hạn định kỳ)
  - `GET  /files/{id}/versions`, `POST /files/{id}/versions/{v}/restore`
  - `POST /files/{id}/rename`, `POST /folders/{id}/files` (Save As / tạo file mới)
- Xác thực: access token ngắn hạn (JWT ký bởi BlueDrive) truyền vào URL mở editor; BlueOffice server xác minh bằng public key (JWKS) của BlueDrive; không lưu mật khẩu người dùng.
- Ngoài BOHP, cài thêm `WopiProvider` theo đặc tả công khai [MS-WOPI] để BlueOffice cũng gắn được vào các hệ thống lưu trữ khác hỗ trợ WOPI (Nextcloud, SharePoint on-prem…) — chỉ cài đặt dựa trên tài liệu công khai.

**2. Nhúng editor trên web (`@blueoffice/editor-sdk`)**
- BlueDrive mở editor bằng iframe: `https://office.<domain>/edit?file={id}&host=bluedrive&token=...` hoặc dùng SDK:
```ts
const editor = BlueOffice.open(container, {
  fileId, accessToken, host: 'bluedrive',
  mode: 'edit' | 'view' | 'comment',
  lang: 'vi-VN', theme: 'light' | 'dark' | 'auto',
  ui: { showShare: true, showClose: true, brandName: 'BlueDrive' },
});
editor.on('saved' | 'dirty' | 'closed' | 'share-requested' | 'error' | 'collaborators-changed', handler);
editor.command('save' | 'print' | 'export-pdf' | 'close');
```
- Giao tiếp qua `postMessage` có kiểm tra `origin` (whitelist cấu hình); không lộ token ra ngoài iframe.
- Nút Share, Close, Back trong editor gửi sự kiện để BlueDrive tự xử lý bằng UI của nó.

**3. Desktop**
- Deep link `bluedoc://open?host=bluedrive&file={id}&token=...` (tương tự `bluesheet://`, `blueslide://` theo loại file): từ BlueDrive web bấm "Mở bằng ứng dụng desktop" → app desktop mở file trực tiếp từ BlueDrive (tải về cache, lưu ngược lên qua BOHP).
- Nếu BlueDrive có client đồng bộ thư mục: file mở từ thư mục đồng bộ là file cục bộ bình thường; khi phát hiện file nằm trong thư mục BlueDrive, desktop chuyển sang chế độ cộng tác online (hỏi người dùng).
- Màn hình Start của desktop: tab "BlueDrive" duyệt file/thư mục, file gần đây, file được chia sẻ với tôi (đăng nhập SSO chung).
- Sửa offline: lưu cục bộ, đồng bộ khi có mạng; xung đột → tạo bản "(xung đột - tên người dùng - thời gian)" thay vì ghi đè.

**4. Cộng tác khi mở từ BlueDrive**
- Nhiều người mở cùng file → cùng một phiên Yjs trên BlueOffice server; khoá BOHP thuộc về phiên, không thuộc về từng người.
- Lưu về BlueDrive theo chu kỳ (autosave 30–60 s khi có thay đổi) và khi người cuối cùng rời phiên; mỗi lần lưu tạo version trong BlueDrive kèm danh sách người đã sửa.

**5. Dịch vụ phụ trợ cho BlueDrive**
- `POST /convert` (docx↔pdf, doc→docx, xls→xlsx, ppt→pptx…) dùng worker `bo-cli`
- `POST /thumbnail` → ảnh xem trước trang/slide đầu để BlueDrive hiển thị trong danh sách file
- `POST /preview` → bản xem nhanh (PDF/ảnh từng trang) cho người chỉ có quyền xem
- `POST /extract-text` → trích văn bản thuần + metadata để BlueDrive đánh chỉ mục tìm kiếm toàn văn
- Webhook BlueOffice → BlueDrive: file đã lưu, phiên cộng tác bắt đầu/kết thúc

**6. Phân quyền & audit**
- Quyền lấy từ BlueDrive mỗi lần mở và khi refresh token; quyền thay đổi giữa chừng (bị thu hồi quyền sửa) → editor chuyển sang chỉ xem ngay.
- Ghi audit log (ai mở, ai sửa, ai xuất/in/tải về) gửi về BlueDrive.

**Tiêu chí hoàn thành**: một BlueDrive giả lập (`tests/mock-bluedrive`, cài BOHP tối thiểu) dùng để test E2E: mở file từ web, mở từ desktop qua deep link, 2 người cùng sửa, lưu tạo version, thu hồi quyền giữa chừng, sửa offline rồi đồng bộ.

> Khi bắt đầu Phase 6A, hỏi tôi về stack và API hiện có của BlueDrive để điều chỉnh BOHP cho khớp trước khi code.

### Phase 7 — BlueForm & BlueNote (chỉ kế hoạch)
- **BlueForm**: biểu mẫu/khảo sát; kết quả trả lời đổ vào một workbook BlueSheet.
- **BlueNote**: sổ ghi chú dùng lại engine BlueDoc (bo-model/bo-layout/bo-edit) và cộng tác bo-collab.
- Kế hoạch chi tiết: `docs/plan/phase-7.md`. Chưa code.

---

## 7. Kiểm thử độ trung thực (fidelity)

- `tests/corpus/{write,sheet,slide}`: bộ file mẫu tự tạo cho từng tính năng (mỗi tính năng ≥ 3 file), file `README.md` ghi nguồn gốc từng file. Không đưa file chứa dữ liệu khách hàng thật.
- **Round-trip test**: mở → lưu → so sánh XML đã chuẩn hoá (canonical: sắp xếp thuộc tính, bỏ khoảng trắng không ý nghĩa). Phần không bị chỉnh sửa phải giống hệt.
- **Validation**: mọi file xuất ra phải qua OpenXmlValidator (.NET, chạy trong CI) không lỗi.
- **Visual regression**: `bo-cli render` ra PNG từng trang, so pixel-diff với ảnh tham chiếu trong `tests/fidelity/reference/` (ảnh do người dùng cung cấp từ phần mềm có bản quyền của họ), ngưỡng cấu hình theo từng file.
- **Fuzzing**: `cargo fuzz` cho các parser (OPC, XML, CFB, BIFF, formula) — không được panic, không được treo.
- **Property-based test** cho engine công thức và undo/redo (thực hiện chuỗi thao tác ngẫu nhiên rồi undo hết phải về trạng thái ban đầu).
- **E2E** (Playwright): các luồng chính — tạo văn bản hành chính, nhập bảng tính có công thức, tạo slide, cộng tác 2 người.
- Báo cáo fidelity tự động sau mỗi lần CI (`docs/fidelity-report.md`): % pass theo từng nhóm tính năng, xu hướng theo thời gian.

---

## 8. Quy ước code

- Rust: edition 2021, `#![forbid(unsafe_code)]` ở mọi crate trừ khi có ADR giải thích; không `unwrap()` trên dữ liệu từ file (dùng `Result` + lỗi có ngữ cảnh); clippy pedantic ở mức warn.
- TypeScript: `strict: true`, không `any` trừ khi có comment giải thích.
- C#: nullable enabled, `TreatWarningsAsErrors`.
- Mỗi phần tử/thuộc tính định dạng file được cài đặt phải có comment tham chiếu mục spec, ví dụ:
  `// ECMA-376 Part 1, §17.3.1.26 pPr (Paragraph Properties)`
- Không hard-code chuỗi UI — dùng i18n key (vi-VN mặc định, en-US).
- Mỗi PR/commit chức năng phải kèm test.
- Tài liệu public API của mỗi crate bằng rustdoc.

---

## 9. CLAUDE.md cần tạo ngay (nội dung tối thiểu)

- Tóm tắt dự án + đường dẫn tới file prompt này
- Nguyên tắc clean-room (mục 1) — nhắc lại ngắn gọn
- Lệnh build/test/run cho Rust, web, desktop, server
- Sơ đồ crate và phụ thuộc
- Phase hiện tại & việc tiếp theo
- Danh sách quyết định kiến trúc (link tới docs/adr)

---

## 10. Việc cần làm ngay

1. Tạo cấu trúc repo như mục 4 cùng `CLAUDE.md`, `docs/ARCHITECTURE.md`, `docs/PROGRESS.md`, `THIRD_PARTY_NOTICES.md`.
2. Viết `docs/plan/phase-1.md` chi tiết.
3. Thực hiện Phase 1 đến khi CI xanh và mở/hiển thị được một file .docx mẫu đơn giản trên web lẫn desktop (cùng `packages/editor`, cùng kết quả hiển thị), xuất được PDF.
4. Dừng lại, báo cáo kết quả (ảnh chụp, số liệu benchmark), chờ duyệt trước khi sang Phase 2.
