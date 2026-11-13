# ADR-0017: Hướng vẽ tài liệu desktop theo số đo

- Trạng thái: Đề xuất, chưa triển khai renderer native · Ngày: 2026-11-12
- Liên quan: ADR-0003 (font và phân trang), ADR-0016 (khung cửa sổ), `docs/perf/BASELINE.md`.

## Bối cảnh và quyết định tạm thời

Mốc release cho thấy một phím mất trung vị 71,1 ms: khoảng 13,4 ms trong lệnh native,
11,4 ms từ khi lệnh trả về đến khi yêu cầu trang, 4,9 ms vẽ canvas, và 21,7 ms từ lúc vẽ
xong đến khung hình. Các con số có nhiễu do tải máy, nhưng cho thấy thay canvas một mình
không chứng minh được mục tiêu dưới 16 ms. Giữ canvas hiện tại trong khi đo lại các thay đổi
đã tích hợp; không coi canvas/WebView là renderer native hoặc là DirectWrite.

Hướng cần thử nghiệm là surface native bằng wgpu cùng pipeline chữ DirectWrite trên Windows,
Core Text trên macOS, FreeType/HarfBuzz trên Linux. Phải dùng cùng kết quả shaping, vị trí
glyph và metric của engine để không thay đổi ngắt dòng hoặc làm sai round-trip so với web;
ADR-0003 vẫn áp dụng cho lựa chọn font tài liệu. Cần prototype kiểm tra thứ tự lớp WebView,
đồng bộ DPI/di chuyển cửa sổ giữa màn hình, IME, caret, vùng chọn, khả năng truy cập, in/PDF,
và độ trễ thực tế trước khi thay renderer sản phẩm. Chưa có bằng chứng prototype nên chưa
chốt đây là phương án đạt mục tiêu.

## Phần đã có và phần còn thiếu

`bo-layout` có bộ nhớ đệm và dàn trang tăng dần tới điểm đồng bộ lại; `DocumentView.refresh`
chỉ vô hiệu hoá các trang dirty và canvas dùng `devicePixelRatio`. Đây là cập nhật theo **trang**,
không phải dirty rectangle, không chứng minh texture scrolling, hinting/ClearType hoặc độ nét
khi đổi màn hình. Chưa có wgpu surface, DirectWrite/Core Text/FreeType renderer, đo 100/125/
150/200% trên màn hình thực, hay chứng minh p95 dưới 16 ms. Các mục này phải được cài đặt và
đo, không được suy ra từ việc canvas có kích thước theo DPR.

## Hướng khảo sát bổ sung trên Windows

Ngoài wgpu, cần đánh giá Direct2D trên surface DXGI cho đường vẽ Windows: Direct2D có
`DrawGlyphRun` nhận glyph run của DirectWrite và có clip/layer; tài liệu Microsoft mô tả
[cách hai API phối hợp](https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-and-directwrite)
và [khả năng dùng DXGI surface](https://learn.microsoft.com/en-us/windows/win32/direct2d/interoperability-overview).
Không suy ra đã có GPU hoặc đã đạt chất lượng từ sự tồn tại của API: phải đo backend thực,
phương án phần mềm dự phòng và hiệu năng. Phải thử thứ tự lớp và tương tác với WebView2;
[tài liệu WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/overview-features-apis)
phân biệt windowed hosting với composition hosting. Chưa quyết định đổi kiến trúc Tauri
hay thay wgpu, vì chưa kiểm chứng được kết hợp surface với IME, truy cập và giao diện hiện có.
Thư viện `bo-dwrite` hiện chỉ trả mask CPU; `DrawGlyphRun` cần font face và glyph run,
nên còn phải thiết kế API và vòng đời tài nguyên tương ứng, không coi thư viện này là
đường vẽ Direct2D/GPU đã kết nối.
