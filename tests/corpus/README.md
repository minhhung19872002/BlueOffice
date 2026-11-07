# Corpus

File mẫu dùng cho round-trip, visual và E2E test. **Không** đưa file chứa dữ liệu khách hàng thật.

| File | Nguồn gốc | Nội dung |
|---|---|---|
| `doc/phase1-sample.docx` | Tự sinh: `cargo run -p bo-cli -- sample tests/corpus/doc/phase1-sample.docx` (`crates/bo-cli/src/sample.rs`) | Văn bản hành chính giả định theo bố cục NĐ 30/2020: quốc hiệu, tiêu ngữ, căn đều, thụt lề, giãn dòng, đậm/nghiêng/gạch chân/gạch kép/gạch ngang/màu, fallback Arial/Calibri, tab, ngắt dòng, phân trang tự động, ngắt trang, section ngang A4 |
| `slide/*.pptx` (25 file) | Tự sinh: `cargo run -p bo-cli -- sample-slides tests/corpus/slide` (`crates/bo-cli/src/slide_samples.rs`); test `committed_corpus_is_up_to_date` bảo đảm file khớp generator | BlueSlide: `text-*` (kế thừa placeholder, bullet nhiều cấp, đánh số, autofit fontScale/lnSpcReduction, chia cột), `shapes-*` (≈60 preset, gradient/pattern, nét đứt, mũi tên, connector, custGeom, nhóm lồng xoay/lật, đổ bóng), `images-*` (PNG, crop, alpha, ảnh theo hình, nền ảnh), `tables-*` (gộp ô, lề ô, viền), `charts-*` (cột, thanh chồng, đường, tròn), `anim-*` (fade/push/wipe/cover/random, p14 AlternateContent, appear/fade/fly-in/wipe), `media-*` (video/audio nhúng & liên kết — byte media giả), `notes-*`/`theme-*` (ghi chú, notes master, clrMapOvr, nền gradient/bgRef/pattern, ẩn slide, showMasterSp). Ảnh sinh bằng code, chữ/số giả định |

Phase 2 bổ sung ≥ 3 file cho mỗi tính năng (`doc/`), Phase 3 (`sheet/`), Phase 4 (`slide/`).
