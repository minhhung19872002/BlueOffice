# BlueOffice

Bộ ứng dụng văn phòng của Bluestar, tương thích định dạng Office Open XML:

| Ứng dụng | Mô tả | Route web |
|---|---|---|
| **BlueDoc** – Soạn thảo văn bản | Tương thích định dạng Microsoft Word (.docx) | `/doc` |
| **BlueSheet** – Bảng tính | Tương thích định dạng Microsoft Excel (.xlsx) | `/sheet` |
| **BlueSlide** – Trình chiếu | Tương thích định dạng Microsoft PowerPoint (.pptx) | `/slide` |

Tích hợp với **BlueDrive** (lưu trữ & chia sẻ). Giai đoạn sau: **BlueForm**, **BlueNote**.

- Web + desktop (Tauri) dùng chung một codebase React và một core Rust.
- Hướng dẫn phát triển: [`CLAUDE.md`](CLAUDE.md) · Kiến trúc: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · Tiến độ: [`docs/PROGRESS.md`](docs/PROGRESS.md)
- Giấy phép bên thứ ba: [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)