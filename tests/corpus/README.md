# Corpus

File mẫu dùng cho round-trip, visual và E2E test. **Không** đưa file chứa dữ liệu khách hàng thật.

| File | Nguồn gốc | Nội dung |
|---|---|---|
| `doc/phase1-sample.docx` | Tự sinh: `cargo run -p bo-cli -- sample tests/corpus/doc/phase1-sample.docx` (`crates/bo-cli/src/sample.rs`) | Văn bản hành chính giả định theo bố cục NĐ 30/2020: quốc hiệu, tiêu ngữ, căn đều, thụt lề, giãn dòng, đậm/nghiêng/gạch chân/gạch kép/gạch ngang/màu, fallback Arial/Calibri, tab, ngắt dòng, phân trang tự động, ngắt trang, section ngang A4 |

Phase 2 bổ sung ≥ 3 file cho mỗi tính năng (`doc/`), Phase 3 (`sheet/`), Phase 4 (`slide/`).
