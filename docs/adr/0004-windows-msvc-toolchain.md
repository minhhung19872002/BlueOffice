# ADR-0004: Toolchain Rust MSVC trên Windows

- Trạng thái: Chấp nhận · Ngày: 2026-11-05

## Bối cảnh
Máy phát triển ban đầu không có Visual Studio Build Tools; thử toolchain `x86_64-pc-windows-gnu` kết hợp
llvm-mingw bị xung đột runtime (`-lgcc_eh` không tìm thấy, msvcrt vs ucrt).

## Quyết định
Dùng `stable-x86_64-pc-windows-msvc` + Visual Studio 2022 Build Tools (workload VCTools) trên Windows — đúng
khuyến nghị chính thức của Tauri 2 và khớp runner `windows-latest` của GitHub Actions.

## Phương án đã loại
- `windows-gnu`: thiếu `dlltool`/`windres`, lệch với CI và với hướng dẫn Tauri.
- `windows-gnullvm`: chưa phổ biến, rủi ro với các crate `windows-sys`/WebView2.
