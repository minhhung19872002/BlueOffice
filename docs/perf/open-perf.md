# Mở tài liệu lớn và rò rỉ bộ nhớ (CORE-PERF-001)

Mục tiêu: `.docx` 1 000 trang mở < 8 s; `.pptx` 300 slide có video mở < 5 s; không rò rỉ bộ nhớ khi
mở/đóng 100 tài liệu. Số đo bản **release** trên máy dev (Windows 11, 16 luồng logic, 32 GB RAM, MSVC;
WASM chạy trong Node 22 / V8). Máy dùng chung với các agent khác (nhiều `rustc`/`cargo` chạy song song,
tải CPU nền dao động 30–100 %): mỗi ca chạy 3 lần, lấy **trung vị**; WASM ghi thêm thời gian CPU của
luồng đo (`openFirstPageCpuMs`), ít phụ thuộc tải nền hơn thời gian thực.

## Tệp đo (sinh khi đo, không commit)

```bash
# .docx ~1 000 trang: mỗi khối 4 đoạn căn đều + 1 chú thích cuối trang, xen kẽ bảng 3×5 có chú thích
# SEQ hoặc ảnh 8×6 cm (40 ảnh PNG khác nhau), Heading 1 mỗi ~20 trang, header + footer PAGE/NUMPAGES
bo-cli sample-big out/big-1000.docx --pages 1000 --rich        # 14,6 MB (7,1 MB XML) → 1 000 trang
# .pptx 300 slide tiêu đề + nội dung + ảnh (30 ảnh 800×600), ghi chú mỗi 10 slide,
# 6 slide có video nhúng 4 MiB (byte giả ngẫu nhiên sau hộp `ftyp`: đo mở, không giải mã)
bo-cli sample-big-deck out/deck-300.pptx                       # 40,8 MB
```

## Cách đo

```bash
# native: open (đọc + dàn trang toàn bộ) + display list trang đầu; mọi trang; xuất PDF
bo-cli bench out/big-1000.docx --iterations 3
bo-cli bench out/deck-300.pptx --iterations 3
# WASM (cùng bản build trình duyệt nạp)
pnpm build:wasm && node scripts/bench-open-wasm.mjs out/big-1000.docx out/deck-300.pptx
# rò rỉ
cargo test --release -p bo-engine --test open_close_leak -- --nocapture
node scripts/leak-wasm.mjs 100 [out/big-1000.docx out/deck-300.pptx]
pnpm --filter @blueoffice/canvas-host test      # ImageStore giải phóng ImageBitmap
```

## Kết quả mở

Đợt chính: "trước" (`main`) và "sau" chạy **xen kẽ** trên cùng máy, 3 lần mỗi bên, trung vị (máy rất bận:
5–10 `rustc` của agent khác; số "trước" của đợt này gấp ~2 lần lúc máy yên). Native = `bo-cli bench
--iterations 1` (mở = đọc gói + mô hình + dàn trang **toàn bộ** + display list trang đầu).

| Ca | Trước (3 lần → trung vị) | Sau (3 lần → trung vị) | Mục tiêu |
|---|---|---|---|
| `.docx` 1 000 trang, native: mở + trang đầu | 8,88 · 6,68 · 5,15 → **6,68 s** | 3,90 · 3,68 · 6,38 → **3,90 s** | < 8 s ✅ |
| `.docx` 1 000 trang, native: + display list cả 1 000 trang | 10,04 · 6,84 · 5,97 → 6,84 s | 4,86 · 4,36 · 6,76 → 4,86 s | — |
| `.docx` 1 000 trang, WASM: mở + trang đầu | 17,85 · 19,36 · 29,36 → **19,36 s** | 4,59 · 4,95 · 3,85 → **4,59 s** | < 8 s ✅ |
| (thời gian CPU của luồng WASM) | 20,4 · 20,4 · 19,9 → 20,4 s | 5,41 · 5,42 · 4,56 → 5,41 s | |
| `.pptx` 300 slide + 6 video, native: mở + slide đầu | 283 · 298 · 207 → **283 ms** | 381 · 256 · 363 → **363 ms** | < 5 s ✅ |
| `.pptx` 300 slide + 6 video, WASM: mở + slide đầu | 966 · 982 · 1 431 → **982 ms** | 646 · 475 · 438 → **475 ms** | < 5 s ✅ |
| (thời gian CPU của luồng WASM) | 1 453 · 1 437 · 1 718 → 1 453 ms | 813 · 844 · 657 → 813 ms | |
| Bộ nhớ WASM sau khi mở `.docx` / `.pptx` | 307 / 173 MB | 324 / 204 MB | < 1 GB ✅ |

Lúc máy yên hơn (chỉ có thay đổi 1 bên dưới, trước khi tối ưu ngắt dòng), 3 lần → trung vị: WASM `.docx`
8,49 · 8,45 · 7,91 → **8,45 s** (trượt mục tiêu) → 3,22 · 2,85 · 3,19 → **3,19 s**; WASM `.pptx`
541 · 500 · 441 → 500 ms → 283 · 318 · 326 → 318 ms; native `.docx` (trước) 3,12 s. Native `.pptx` không có
thay đổi đáng kể trên đường đi (chênh lệch là nhiễu tải nền).

Mở lần lượt nhiều tệp lớn trong **một** `WasmEngine` không đóng (cách trình sửa làm: tệp mới thay tệp cũ),
`.docx` → `.pptx` → `.docx` → … : sau 2,85 / 1,20 / 2,56 / 0,43 / 2,53 / 0,46 s, bộ nhớ 324 → 402 → 700 MB rồi
đứng yên (lúc mở tệp mới, tệp cũ vẫn còn: hai tài liệu lớn cùng lúc). Bản trước mở lại cùng `.docx` nhanh
(0,7 s) nhờ bộ đệm bố cục của lần trước còn nguyên — đó chính là phần bộ nhớ không được trả ở mục 2 bên
dưới; nay mở lại cũng dàn trang lại (2,5 s).

## Điểm nóng tìm được và thay đổi

1. **WASM: tăng bộ nhớ từng chút một** (`node --cpu-prof`): 60 % thời gian mở `.docx` 1 000 trang
   (5,6 s / 9,4 s) nằm trong `dlmalloc::sys::System::alloc` = `memory.grow`. Bộ cấp phát của
   `wasm32-unknown-unknown` chỉ xin đúng phần thiếu (64 KiB) và V8 chép lại cả vùng nhớ mỗi lần lớn lên,
   nên dựng mô hình + bố cục ~300 MB theo từng cấp phát nhỏ là bậc hai. BlueSheet đã đặt trước heap
   (`bo-wasm/src/heap.rs`); nay `WasmEngine::open` cũng đặt trước theo kích thước giải nén trong central
   directory (không giải nén gì): 36 byte / byte XML + 3 byte / byte media (`reserve_for_document`,
   `bo_opc::Package::part_sizes`; trần 640 MB, dưới 16 MB không đặt). Kết quả lúc máy yên: `.docx`
   1 000 trang 8,45 s → 3,19 s, `.pptx` 300 slide 0,50 s → 0,32 s. Bộ nhớ WASM tăng nhẹ (307 → 324 MB,
   173 → 204 MB), vẫn < 1 GB (§5). Bản đầu đặt trước ở **mọi** lần mở; `leak-wasm.mjs` (chế độ tệp lớn)
   cho thấy mở xen kẽ `.docx`/`.pptx` lớn làm bộ nhớ tăng ở mỗi lần (477 → 776 MB sau 10 lần: pool đã
   phân mảnh không còn khối liền đủ lớn). Nay bỏ qua khi bộ nhớ đã ≥ 2 lần lượng cần đặt
   (`core::arch::wasm32::memory_size`): bộ nhớ dừng ở 623 MB.
2. **Ngắt dòng chép cả dòng cho mỗi từ** (`node --cpu-prof` sau bước 1: `Piece::clone` 6,5 %,
   `String::clone` 2,5 %): `bo-layout/src/paragraph.rs` đo dòng thử bằng `cur.clone()` + các mảnh của từ,
   tức O(số từ²) bản sao `Piece` (chuỗi + mảng offset) mỗi dòng. Nay chuyển các mảnh của từ vào dòng, đo,
   rồi lấy lại (`append` / `split_off`, không sao chép); bỏ thêm một bản sao khi tách mảnh theo mức bidi.
   Kết quả đầu ra giống hệt (snapshot insta và test parity xanh).
3. **Bộ đệm theo nội dung giữ lại tài liệu đã đóng**: `LayoutCache` (bố cục đoạn, trần 100 000 mục),
   bộ đệm shaping của `FontManager` (trần 200 000 mục) và bộ đệm phân tích chính tả (trần 20 000) dùng
   chung cho mọi tài liệu của một `Engine`; trường đọc đồng hồ (DATE, SAVEDATE…) đổi khoá ở mỗi lần mở,
   nên sau khi đóng, bố cục của các tài liệu trước vẫn bị giữ tới khi chạm trần (với tài liệu 1 000 trang:
   hàng chục nghìn đoạn). `Engine::load` / `load_presentation` nay xoá các bộ đệm này
   (`release_document_caches`; `FontManager::clear_caches`, `LayoutCache::clear` trả lại bộ nhớ của bảng
   băm); `load_presentation` cũng xoá bộ đệm xem trước khi in, ảnh SmartArt và trình sửa
   header/footer/chú thích/text box còn mở của tài liệu trước. Đầu ra bố cục không đổi (chỉ là bộ đệm).
4. **TS: ảnh đã giải mã** (`packages/canvas-host` `ImageStore`): `clear()` nay gọi `ImageBitmap.close()`
   (giải phóng điểm ảnh ngay, không chờ GC) và bỏ kết quả giải mã về muộn của tài liệu trước (trước đây
   một ảnh đang giải mã khi mở tài liệu mới vẫn được cất vào kho của tài liệu mới). `DocumentView.dispose`,
   BlueSlide (trình sửa, màn hình khán giả) và BlueSheet giải phóng kho ảnh khi đóng; BlueSheet xoá kho ảnh
   khi mở sổ khác (tên part media lặp giữa các tệp: trước đây có thể hiện ảnh của sổ trước).

Các hướng đã cân nhắc, chưa cần cho mục tiêu: dàn trang lười ngoài các trang đang xem (cả hai nền tảng đã
dưới mục tiêu với dàn trang toàn bộ; `DocInfo.pageCount` cần tổng số trang), đọc zip kiểu stream, giải mã
media lười cho slide (media chỉ được chép khi slide dùng tới — video không vào `images`), parse song song.

## Rò rỉ khi mở/đóng 100 tài liệu

`cargo test --release -p bo-engine --test open_close_leak`: bộ cấp phát toàn cục có đếm byte
(`stats_alloc`, chỉ trong binary test này) đo heap đang dùng **sau khi đóng** (tài liệu trắng thay thế);
mỗi tệp corpus mở một lần (khởi động), rồi mở + vẽ trang đầu + đóng 100 tài liệu nữa trong cùng một
`Engine` / `SheetEngine`. Test đòi heap sau 100 lần không vượt mức sau khởi động quá 4 MB.

| Heap đang dùng sau khi đóng (MB) | Trắng | Trước: sau khởi động → cao nhất / cuối 100 lần | Sau: sau khởi động → cao nhất / cuối |
|---|---|---|---|
| `.docx` (86 tệp, 186 lần mở) | 17,2 | 20,6 → 20,6 / 20,6 | 18,2 → 18,2 / 18,2 |
| `.pptx` (64 tệp, 164 lần mở) | 18,9 | 19,0 → 19,0 / 19,0 | 18,9 → 18,9 / 18,9 |
| `.xlsx` (161 tệp, 261 lần mở) | 18,6 | 19,0 → 19,1 / 18,9 | 18,9 → 18,9 / 18,9 |

Không có rò rỉ tăng dần (kể cả trước khi sửa: corpus mở lại cho cùng khoá bộ đệm); phần "trước" giữ thêm
3,4 MB sau khi đóng `.docx` là bộ đệm bố cục / shaping của các tài liệu đã đóng — tỉ lệ với tài liệu lớn
nhất đã mở và chỉ dừng ở trần 100 000 đoạn / 200 000 run (mục 3 ở trên). Sau khi sửa còn 1,0 MB (bộ đệm
nạp một lần: font hệ thống, từ điển chính tả).

`node scripts/leak-wasm.mjs 100 out/big-1000.docx out/deck-300.pptx` (kích thước `WebAssembly.Memory`, chỉ
lớn lên; bản WASM sau cùng):

| Bộ nhớ WASM (MB) | Trước: sau khởi động → cuối | Sau: sau khởi động → cuối |
|---|---|---|
| corpus `.docx`, 100 lần mở/đóng | 17,3 → 17,6 (+0,4) | 15,3 → 15,3 (0) |
| corpus `.pptx` | 14,8 → 14,8 (+0,1) | 14,7 → 14,7 (0) |
| corpus `.xlsx` | 12,4 → 12,4 | 12,4 → 12,4 |
| 2 tệp lớn (1 000 trang + 300 slide), 2 lượt khởi động + 10 lần | 352,5 → 352,5 | 623,3 → 623,3 (0) |

Tệp lớn: bản sau đứng ở 623 MB (một lần đặt trước heap thứ hai, xem mục 1), bản trước ở 352 MB nhưng mỗi
lần mở `.docx` lớn đầu tiên mất 8–19 s.

TS (`pnpm --filter @blueoffice/canvas-host test`, `renderer.test.ts`): `ImageStore` với engine + bộ giải mã
giả đếm handle ảnh còn sống; 100 tài liệu (5 ảnh mỗi tài liệu, mỗi lần đóng khi một lần giải mã đang dở):
còn 0 handle, đỉnh ≤ 10. Trình sửa dùng một engine (một worker) cho mọi tài liệu và mở tệp mới thay tệp cũ,
nên không có handle engine theo tài liệu để rò; `WebEngineHost.dispose` kết thúc worker.

## Không hồi quy benchmark

| Benchmark | Trước | Sau | Ghi chú |
|---|---|---|---|
| `cargo bench -p bo-layout` `cold_300_pages` | 2,003 s [1,968 – 2,036] | 1,336 s [1,249 – 1,442] | −33 % (criterion, p < 0,05; `--save-baseline before` / `--baseline before`, chạy liền nhau) |
| `cargo bench -p bo-layout` `edit_one_paragraph_relayout` | 71,8 ms | 46,7 ms | −21 % |
| `cargo bench -p bo-formula` | — | 1,59 s / 9,4 ms / 36,3 ms / 43,9 ms | mã `bo-formula` và các crate nó dùng không đổi; số tuyệt đối gấp ~2 lần PROGRESS (0,80 s) vì tải nền |

Cả hai lần chạy `bo-layout` cùng lúc máy bận (PROGRESS ghi 0,73 s cho `cold_300_pages` lúc máy yên).
