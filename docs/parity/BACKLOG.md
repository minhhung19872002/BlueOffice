# Parity BACKLOG (nguồn sự thật duy nhất — `BlueOffice-PARITY-PROMPT.md` §1.1)

Trạng thái: `[ ]` chưa làm · `[~]` đang làm · `[x]` xong (có test) · `[p]` một phần (ghi phần thiếu) · `[!]` bị chặn (xem
`NEEDS_HUMAN.md`). Thứ tự: P0 → P1 → P2 → P3; trong cùng mức, hạng mục mở khoá nhiều hạng mục khác trước.
Mỗi dòng: **ID** — mô tả · ưu tiên · phụ thuộc · tiêu chí hoàn thành (DoD).
Hiện trạng ban đầu lấy từ `docs/PROGRESS.md` (2026-11-08); hạng mục đã có từ giai đoạn 1 ghi `[x]`/`[p]` kèm test chứng minh.

## Vòng lặp hiện tại
- Đang làm: xem mục có `[~]`. Kết thúc phiên: cập nhật mục này + `docs/PROGRESS.md`.

---

## P0

### Phương trình (PROMPT §4.4)
- [x] **CORE-MATH-001** — Đọc/ghi OMML lossless (m:oMath, m:oMathPara giữ nguyên XML) · P0 · — · round-trip corpus `math-0*`, OpenXmlValidator. Test: `bo-docx` corpus.
- [x] **CORE-MATH-002** — Hiển thị OMML: phân số, mũ/chỉ số, căn, n-ary + cận, ngoặc co giãn, ma trận, eqArr, hàm, dấu nhấn, bar, lim, groupChr, borderBox, phant · P0 · 001 · ảnh render corpus + snapshot. _Rà soát định kỳ (2026-11-09): phần thiếu đã xong qua CORE-MATH-003 (STIX Two Math + bảng MATH, sSubSup theo SubSuperscriptGapMin) và CORE-MATH-004 (oMathPara theo jc); `m:box` dàn như nhóm (không có tham số hiển thị riêng)._
- [x] **CORE-MATH-003** — Font toán OFL (STIX Two Math hoặc Latin Modern Math) đóng gói + dùng bảng MATH (constants, variants, assembly) cho dàn trang · P0 · 002 · ngoặc/căn/tích phân dùng biến thể glyph thật; test so kích thước với giá trị MATH table.
  - Xong: STIX Two Math 2.13 (OFL-1.1, `assets/fonts/STIXTwoMath-Regular.ttf`, FaceId 16 sau các font văn bản) thay cho
    Cambria Math (`FALLBACKS`; máy có font toán cài sẵn thì dùng font đó nếu có bảng MATH). `bo-text::math`: MathConstants,
    extents, italic correction, `stretch` (biến thể kích thước → glyph assembly theo GlyphAssembly, overlap đều ≥
    MinConnectorOverlap). `bo-layout` math: chữ nghiêng bằng Mathematical Italic (U+1D434…, h → U+210E), phân số
    (shift/gap/rule theo display/text style), chỉ số trên/dưới (shift, drop, gap tối thiểu), căn (glyph √ co giãn, gap,
    rule, degree raise/kern), n-ary (DisplayOperatorMinHeight, italic correction cho cận), ngoặc đối xứng quanh trục,
    bar/limit/accent theo bảng. Sửa lỗi `MathBox::shifted` không cập nhật asc/desc. Test: `bo-layout math::tests`
    (so với giá trị đọc từ bảng MATH của chính file font: thanh phân số trên trục + độ dày, shift tử/mẫu, biến thể của "(",
    assembly ≥ 3 mảnh cho ma trận 10 hàng, tích phân display ≥ DisplayOperatorMinHeight, căn).
- [x] **CORE-MATH-004** — oMathPara: căn lề (m:jc), công thức display chiếm cả dòng · P0 · 002 · test layout vị trí x.
  - Xong: `m:oMathPara` chiếm một dòng riêng (ngắt dòng ảo trước/sau, dấu cách theo sau giữ trên dòng phương trình, không
    sinh dòng trống cuối đoạn), dòng đó căn theo `m:jc` (left/right; center và centerGroup mặc định → giữa) thay cho căn lề
    đoạn. Test: `bo-layout/tests/features.rs::display_equation_takes_its_own_aligned_line` (vị trí x cho từng jc, chữ
    quanh phương trình vẫn căn trái, `m:oMath` trong dòng không tách dòng). Chưa: căn nhóm nhiều dòng theo dấu `=` (centerGroup thật).
- [x] **CORE-MATH-005** — Nhập phương trình bằng cú pháp tuyến tính UnicodeMath → OMML · P0 · 002 · bảng test chuỗi → XML.
  - Xong: crate `bo-math` (cây `Node` theo các phần tử §22.1.2 + ghi OMML `to_omml`; `unicode_math::parse` theo Unicode
    Technical Note #28): phân số `/` và `¦` (bỏ ngoặc quanh toán hạng), `_ ^` và dấu phẩy `′`, `√ ∛ ∜ √(n&x)`, n-ary + cận +
    `▒`, ngoặc co giãn `( ) [ ] { } ⟨ ⟩ ⌊ ⌋ ⌈ ⌉ | |` với `∣`, hàm đứng (lim/max/min nhận cả hạng tử, cận dưới), `■` ma trận, `█`
    hệ phương trình, dấu nhấn tổ hợp / gạch trên, `"văn bản"`, control word `\alpha \sum \sqrt \le \to …`, `-` → `−`.
    Test: bảng chuỗi → XML (40 ca) + công thức lồng nhau + proptest không panic/XML cân; corpus `math-04-linear.docx`
    sinh từ chuỗi tuyến tính qua OpenXmlValidator (0 lỗi) + round-trip.
- [x] **CORE-MATH-006** — Nhập LaTeX (tập con phổ biến) → OMML · P0 · 005 · bảng test.
  - Xong: `bo_math::latex` (tokenizer + parser sang cùng cây `Node`): nhóm `{}`, `^ _ '`, `\frac \dfrac \tfrac \binom
    \sqrt[n]`, toán tử lớn có `\limits`, hàm (`\lim_{}` cận dưới, `\operatorname`), `\left … \right` (kể cả `.`), `( ) [ ] | |`
    ghép cặp thành ngoặc co giãn, môi trường `matrix pmatrix bmatrix Bmatrix vmatrix Vmatrix cases aligned`, dấu nhấn,
    `\overline \underline`, `\text \mathrm …` giữ dấu cách, ký hiệu Hy Lạp/toán tử, bỏ lệnh khoảng cách và chú thích `%`,
    lệnh lạ giữ nguyên dạng chữ. Test: 22 cặp LaTeX ↔ UnicodeMath cho cùng OMML, các cấu trúc riêng (binom, cases, `\left.`,
    operatorname), proptest; corpus `math-05-latex.docx` qua OpenXmlValidator (0 lỗi).
- [x] **CORE-MATH-007** — Chèn/sửa phương trình trong BlueDoc (lệnh `insert.equation`, hộp nhập tuyến tính, chuyển chuyên nghiệp ↔ tuyến tính, OMML → UnicodeMath) · P0 · 005 · E2E web: chèn, lưu, mở lại.
  - Xong: `bo-edit` `insert_equation` / `selected_equation`; `bo-engine::equation` + lệnh `insertEquation {text, latex,
    display, replace}`, `equation`, `equationFormat {linear}`; mọi `EditResult` mang `equation` (dạng tuyến tính, display,
    isLinear) khi vùng chọn là phương trình. Vùng toán chèn vào tự khai báo `xmlns:m` (tài liệu mới không khai báo ở gốc).
    `bo-math` thêm `read::from_omml` (OMML → cây) và `linear::to_linear` (cây → UnicodeMath, ngoặc khi toán hạng không đơn).
    Dạng tuyến tính = vùng toán giữ chuỗi tuyến tính trong một `m:r` (như Office Math), chuyển lại thì dựng lại.
    UI: tab Chèn → nhóm Phương trình (Chèn/sửa, Chuyên nghiệp, Tuyến tính; phím Alt+=), hộp thoại nhập UnicodeMath hoặc
    LaTeX, tuỳ chọn hiển thị riêng dòng. Test: `bo-math` round-trip tuyến tính → cây → tuyến tính (31 ca) và OMML →
    tuyến tính; `bo-engine equation::tests` (chèn, chuyển dạng, thay bằng LaTeX display, lưu/mở lại); E2E
    `specs/equation.spec.ts` (chèn, chuyển dạng, sửa, lưu, mở lại; file lưu qua OpenXmlValidator 0 lỗi).
- [ ] **CORE-MATH-008** — Thư viện phương trình mẫu (tự soạn) + đánh số phương trình · P1 · 007 · test chèn mẫu.
- [ ] **CORE-MATH-009** — Phương trình trong BlueSlide (a14:m trong txBody) và BlueSheet (shape) hiển thị · P1 · 002 · corpus + render.
- [ ] **CORE-MATH-010** — Soạn trực tiếp trên công thức (caret trong vùng toán: gõ vào tử/mẫu/chỉ số, Tab giữa các ô, tự chuyển `x^2␣` thành dựng sẵn như khi gõ tuyến tính trong vùng toán) · P2 · 007 · E2E. _(Bổ sung khi rà soát định kỳ 2026-11-09: hiện sửa qua hộp thoại tuyến tính.)_

### Soát chính tả & ngữ pháp (§4.2)
- [x] **CORE-SPELL-001** — Engine soát chính tả tương thích Hunspell (.aff/.dic: affix, compound cơ bản), crate `bo-spell` · P0 · — · test với từ điển mẫu tự viết.
  - Kế hoạch: crate mới `bo-spell` (MIT, `forbid(unsafe)`), không phụ thuộc ngoài. Đọc `.aff` (SET UTF-8, FLAG
    char/long/num/UTF-8, TRY, KEY, REP, PFX/SFX có điều kiện + strip + append + cờ tiếp, cross product, NOSUGGEST,
    KEEPCASE, FORBIDDENWORD, NEEDAFFIX, COMPOUNDFLAG/BEGIN/MIDDLE/END, COMPOUNDMIN, ONLYINCOMPOUND, WORDCHARS) và
    `.dic` (đếm dòng, từ/cờ, morph bỏ qua). `check(word)`: số/ký hiệu bỏ qua, chữ hoa đầu/toàn hoa, bóc 1 tiền tố +
    1 hậu tố (kể cả kết hợp), ghép từ. `suggest(word)`: REP, KEY (phím lân cận), TRY (chèn/thay), xoá, đảo, tách
    đôi, chữ hoa; xếp theo khoảng cách sửa + n-gram. API `Dictionary::from_aff_dic`, `Speller` gồm nhiều từ điển +
    từ điển người dùng + danh sách loại trừ; tách từ theo UAX #29. Test: từ điển mẫu tự viết theo đặc tả hunspell(5)
    (tài liệu công khai), mỗi tính năng ≥ 1 test.
  - Xong: crate `bo-spell` (`aff.rs` + `lib.rs`), 12 test (affix/điều kiện, cross product, hậu tố kép, chữ hoa, KEEPCASE/FORBIDDEN/NEEDAFFIX/NOSUGGEST, ghép từ, gợi ý REP/KEY/TRY/tách từ, ISO 8859-1, tách từ UAX #29, proptest không panic).
- [x] **CORE-SPELL-002** — Từ điển tiếng Anh license hợp lệ (SCOWL/en_US — giấy phép kiểu MIT/BSD) · P0 · 001 · ghi THIRD_PARTY_NOTICES; test từ đúng/sai.
  - Xong: SCOWL en_US 2020.12.07 trong `assets/dictionaries/en_US` (README nguyên văn, THIRD_PARTY_NOTICES); test `bo-spell/tests/en_us.rs` (từ đúng/sai, gợi ý). Chưa hỗ trợ `COMPOUNDRULE` (số thứ tự như 21st — số đã được bỏ qua khi kiểm tra) và `ICONV`.
- [x] **CORE-SPELL-003** — Từ điển tiếng Việt: tự xây từ danh sách âm tiết hợp lệ theo quy tắc chính tả (phụ âm đầu × vần × thanh, tự sinh) + nguồn mở license hợp lệ · P0 · 001 · test âm tiết đúng/sai, dấu đặt sai vị trí.
  - Kế hoạch: `bo-spell::vi::Vietnamese` (Checker) kiểm từng âm tiết theo quy tắc chính tả công khai: tách thanh
    (NFD: huyền/sắc/hỏi/ngã/nặng) và dấu mũ/trăng/móc; âm đầu (b c ch d đ g gh gi h k kh l m n ng ngh nh p ph qu r s t
    th tr v x); bảng vần đầy đủ (≈150 vần); ràng buộc c/k/q, g/gh, ng/ngh; vần tắc p/t/c/ch chỉ thanh sắc/nặng; vị trí
    dấu thanh theo kiểu cũ và kiểu mới (oa/oe/uy). Gợi ý: đổi thanh, đổi a/ă/â e/ê o/ô/ơ u/ư d/đ, đặt lại dấu, xoá/đảo
    ký tự. Test: bảng âm tiết đúng/sai.
  - Xong: `bo-spell::vi::Vietnamese` + `Speller::ignore_all_caps`; test bảng âm tiết đúng/sai, gợi ý (thanh, dấu mũ, vị trí dấu). Quét 8.369 từ tiếng Việt thật trong tài liệu repo (`cargo run -p bo-spell --example vi_scan -- <file>`): chỉ 1 từ bị từ chối (`NĐ`, chữ viết tắt — bỏ qua nhờ `ignore_all_caps`).
- [x] **CORE-SPELL-004** — Gạch đỏ trong BlueDoc (display list), gợi ý sửa (menu chuột phải), bỏ qua/bỏ qua tất cả/thêm vào từ điển người dùng · P0 · 002,003 · E2E. _Xong: lệnh `spelling/spellAt/spellIgnore/spellAddWord/setSpelling` (bo-engine `spell.rs`, cache theo đoạn), lớp phủ gạch sóng đỏ (không in), menu chuột phải, từ điển người dùng lưu localStorage, nút Soát lỗi ở tab Xem lại; en_US tải nền ở web, đóng gói ở native. Test: `bo-engine spell::tests`, E2E `specs/spelling.spec.ts`._
- [ ] **CORE-SPELL-005** — Ngôn ngữ theo đoạn/run (w:lang), kiểm tra toàn tài liệu (hộp thoại) · P1 · 004 · test.
- [ ] **CORE-SPELL-006** — Soát trong ô BlueSheet và text box BlueSlide · P1 · 004 · E2E.
- [x] **CORE-GRAM-001** — Ngữ pháp/văn phong cơ bản (khoảng trắng thừa, dấu câu, viết hoa đầu câu, lặp từ, ngoặc không đóng, dấu thanh sai vị trí kiểu cũ/mới) gạch xanh · P0 · 004 · bảng test quy tắc.
  - Xong: `bo-spell::grammar` (quy tắc tự viết theo quy ước trình bày văn bản: thừa dấu cách, dấu cách trước dấu câu,
    thiếu dấu cách sau dấu câu, viết hoa đầu câu/đoạn, lặp từ (trừ láy toàn phần thông dụng: từ từ, ngày ngày…), ngoặc/nháy
    không cân, có ngoại lệ cho viết tắt v.v./e.g., tên tệp .docx, tên mã BlueOffice.Api, ô `[ ]`, đánh số a) 1.);
    kiểu dấu thanh (`vi::tone_style`/`restyle`, oa/oe/uy) so với kiểu dùng nhiều nhất trong tài liệu. Engine: mark
    `grammar: true`, `spellAt` trả `rule`, lệnh `setGrammar`/`grammarIgnore`; UI gạch sóng xanh, menu nêu quy tắc + sửa/bỏ
    qua, nút trong nhóm Soát lỗi. Test: bảng quy tắc `grammar::tests` (+ proptest), `bo-engine spell::tests::grammar_marks_menu_and_ignore`,
    E2E `spelling.spec.ts` (gạch xanh, sửa viết hoa, bỏ qua). Quét `BlueOffice-PARITY-PROMPT.md`
    (`cargo run -p bo-spell --example grammar_scan -- <file>`) để loại báo nhầm.

### SmartArt (§4.4)
- [x] **CORE-SMART-001** — Hiển thị SmartArt bằng drawing fallback (dsp:drawing) trong Doc/Sheet/Slide · P0 · — · corpus `smartart-*`, `slide-smartart-*`. _Phần thiếu (tự dựng layout) xong ở CORE-SMART-002._
- [x] **CORE-SMART-002** — Đọc data model (dgm:dataModel: pt/cxn) + tự dựng layout cho các nhóm: danh sách, quy trình, chu trình, phân cấp, ma trận, kim tự tháp, quan hệ · P0 · — · render file không có fallback.
  - Xong: `bo-pptx::diagram_layout` — đọc data model (§21.4.3: `dgm:pt` node/asst, nối `parOf` theo `srcOrd`; bỏ qua
    pres/parTrans/sibTrans), nhóm bố cục từ `dgm:catLst/dgm:cat` của layoutDef hoặc từ `prSet@loTypeId`, bố cục tự viết:
    danh sách (lưới khối, ý con là gạch đầu dòng), quy trình (hàng khối + mũi tên), chu trình (vòng tròn đều, mũi tên
    tiếp tuyến), phân cấp (cây: lá chia đều bề rộng, cha giữa các con, đường nối gấp khúc), ma trận (lưới vuông), kim tự
    tháp (tam giác + hình thang rộng dần), quan hệ (vòng tròn chồng nhau bán trong suốt); bảng màu riêng của BlueOffice.
    Kết quả là các shape DrawingML như phần dsp:drawing nên dùng chung bộ vẽ. Dùng khi file không có dsp:drawing: BlueSlide
    (render), BlueDoc (đọc data part làm "ảnh", vẽ khi dàn trang, lưu lại nguyên vẹn), BlueSheet (overlay).
    Test: `diagram_layout::tests` (đọc cây, nhóm bố cục, từng bố cục: vị trí/thứ tự/không tràn khung), `diagram::tests`
    (data model → shape), `bo-engine smartart::tests` (docx chỉ có data model → PNG, lưu giữ nguyên data part); corpus
    `slide-smartart-data-{process,hierarchy,cycle,pyramid}.pptx`, `smartart-04-datamodel.docx` (OpenXmlValidator 0 lỗi).
- [p] **CORE-SMART-003** — Tạo SmartArt mới từ thư viện bố cục BlueOffice; ghi data model + layout/style/color + drawing fallback hợp lệ · P0 · 002 · OpenXmlValidator.
  - Xong (BlueSlide): lệnh `insertSmartArt {layout, items, colors?, style?, rect?}` ghi đủ 5 phần (data model với điểm
    doc + node + nối parOf, layoutDef `urn:bluestar.vn/blueoffice/layout/<nhóm>` có catLst, styleDef, colorsDef, dsp:drawing
    tự dựng) + 5 quan hệ từ slide và khung `dgm:relIds`; `dsp:sp@modelId` dạng GUID (ST_ModelId). Test: `bo-pptx
    diagram_layout::tests::outline_round_trip_and_parts` (ghi → đọc lại cùng cây/nhóm/màu), `bo-engine slide_show::
    smartart_inserted_edited_converted_and_saved` (chèn, lưu, mở lại: đủ phần + content type + quan hệ), E2E
    `specs/smartart.spec.ts`; file lưu từ web qua OpenXmlValidator 0 lỗi. **Thiếu**: chèn SmartArt mới trong BlueDoc /
    BlueSheet (cần mô hình phần sơ đồ + quan hệ trong docx/xlsx) → CORE-SMART-005.
- [p] **CORE-SMART-004** — Ngăn văn bản SmartArt: sửa nội dung, thêm/xoá/nâng/hạ cấp nút, đổi bố cục/màu/style, chuyển thành shapes · P0 · 003 · E2E.
  - Xong (BlueSlide): ngăn văn bản (hộp thoại dàn ý: mỗi dòng một nút, Tab/Shift+Tab hạ/nâng cấp → thêm/xoá/nâng/hạ),
    đổi bố cục (7 nhóm), màu (6 bảng màu BlueOffice) và kiểu (tô màu / viền màu) qua `setSmartArt` (ghi lại data model,
    layoutDef, dsp:drawing; id kiểu/màu nằm ở `prSet@csTypeId/qsTypeId`), `smartArtToShapes` (nhóm hình thường, giữ id,
    hoàn tác được); tab SmartArt trên ribbon, nhấp đúp mở ngăn văn bản. Test: engine (đổi bố cục/màu/kiểu, giữ dàn ý, từ
    chối giá trị lạ, chuyển thành hình + hoàn tác) và E2E `specs/smartart.spec.ts` (sửa, Tab hạ cấp, màu/kiểu, lưu, mở lại,
    chuyển thành hình). **Thiếu**: sửa SmartArt trong BlueDoc/BlueSheet (CORE-SMART-005).
- [ ] **CORE-SMART-005** — SmartArt mới và sửa SmartArt trong BlueDoc / BlueSheet (mô hình phần sơ đồ + quan hệ docx/xlsx, dùng chung `bo-pptx::diagram_layout`) · P1 · 003,004 · E2E. _(Tách ra từ 003/004 khi làm phần BlueSlide.)_

### Biểu đồ (§6.6)
- [p] **CORE-CHART-001** — Biểu đồ c: column/bar (clustered/stacked), line, area, pie, doughnut, scatter, combo: đọc/hiển thị/tạo/lưu · P0 · — · corpus `charts-*`. **Thiếu**: 100 %, 3D, bubble, radar, surface, stock, pie of pie, trendline/error bar đầy đủ.
- [p] **CORE-CHART-002** — Loại c: còn lại: 100 % stacked, bubble, radar (marker/filled), stock (HLC/OHLC/VHLC/VOHLC), surface/contour, pie of pie / bar of pie, explode · P0 · — · corpus 3 file/loại + render + round-trip.
  - Xong: một bộ vẽ biểu đồ dùng chung (bo-sheet) cho BlueSheet, BlueSlide, BlueDoc — slide/doc vẽ từ giá trị cache,
    màu theme qua callback (accent1…6), không khung vùng biểu đồ. Model đọc/ghi: màu series/điểm (srgbClr/schemeClr, dPt),
    bubbleSize, varyColors, explosion, radarStyle, ofPieType, upDownBars, wireframe; loại Stock, Surface, OfPie. Vẽ: 100 %
    stacked (cột/thanh/đường/vùng), bubble (diện tích theo kích thước), radar (standard/marker/filled), stock HLC/OHLC +
    thanh tăng/giảm, pie of pie / bar of pie (1/3 điểm cuối), bánh tách miếng, surface (nhìn từ trên như contour, dải giá
    trị trong chú giải). Tạo: 27 loại trong BlueSheet (`CHART_TYPES`) và 23 trong BlueSlide. Test: `bo-xlsx corpus::
    new_chart_types_round_trip` (18 file `writer-charts-{percent,bubble,radar,stock,surface,ofpie}-{1,2,3}.xlsx` đều qua
    OpenXmlValidator 0 lỗi, đọc lại cùng loại/kiểu/cờ/tham chiếu), `bo-sheet chart::tests` (mọi loại vẽ được),
    `bo-engine slide_show::new_chart_types_on_slides`, snapshot slide biểu đồ cập nhật. **Thiếu**: VHLC/VOHLC (khối lượng trên
    trục phụ), bề mặt 3D thật (CORE-3D-001), splitType/splitPos tuỳ chỉnh của of-pie.
- [x] **CORE-CHART-003** — Biểu đồ 3D: đọc/ghi bar3D/line3D/area3D/pie3D/surface3D + view3D (rotX/rotY/depthPercent/hPercent/rAngAx/perspective) + serAx + shape (hộp/trụ/nón/kim tự tháp); vẽ khối thật (`bo-sheet/src/chart/three_d.rs`: tường/sàn có lưới, hộp/trụ/nón, băng đường, khối vùng, tròn nghiêng có tách miếng, lưới bề mặt; loại mặt khuất + sắp mặt + chiếu sáng), dùng chung sheet/slide/doc; 18 loại 3D trong hộp thoại BlueSheet/BlueSlide · P0 · CORE-3D-001 · corpus `writer-charts-3d{bar,line,pie}-*` (OpenXmlValidator), chart::three_d::tests, `three_d_chart_types`, `every_chart_type_is_named_back`, slide_show `new_chart_types_on_slides`. Còn lại (không chặn): xoay 3D bằng chuột, định dạng sàn/tường (floor/sideWall/backWall spPr), gapDepth.
- [p] **CORE-CHART-004** — Biểu đồ mở rộng cx: waterfall, treemap, sunburst, histogram, pareto, box & whisker, funnel: đọc/ghi/hiển thị · P0 · — · corpus + validator.
  - Xong: `bo-xlsx::chartex` theo [MS-ODRAWXML] 2.24 — đọc `cx:chartSpace` (chartData: strDim nhiều cấp, numDim
    val/size; series layoutId, subtotals, binning, statistics, paretoLine), ghi chartex khi tạo mới (phần `chartEx{n}.xml`,
    content type + quan hệ chartEx, khung `mc:AlternateContent` Requires cx1/cx2 + Fallback chữ); model: 7 loại mới +
    `levels`/`subtotals`. Vẽ (bộ vẽ chung, cả BlueSheet/BlueSlide/BlueDoc): thác nước (tổng chạy, mốc tổng, nối), bản đồ
    cây (squarified theo nhóm cấp ngoài), tia nắng (vòng theo cấp), histogram (bin tự động theo quy tắc Scott / số bin /
    độ rộng; khoảng (a, b]), Pareto (sắp giảm dần + đường luỹ kế trên trục %), hộp và râu (tứ phân vị exclusive/inclusive,
    râu 1,5×IQR, ngoại lai, trung bình ×), phễu. BlueSlide vẽ lựa chọn chartex trong `mc:AlternateContent` (các
    AlternateContent khác: vẽ Fallback). Tạo mới: 7 loại trong BlueSheet. Test: `bo-xlsx corpus::new_chart_types_round_trip`
    (21 file `writer-chartex-*-{1,2,3}.xlsx`, OpenXmlValidator 0 lỗi — đã sửa binCount/axisId dạng text theo validator),
    corpus slide `slide-chartex-{waterfall,treemap,funnel}.pptx` (0 lỗi) + snapshot. **Thiếu**: tạo chartex trong
    BlueSlide/BlueDoc (đang hiển thị), regionMap (bản đồ), nhãn dữ liệu chartex.
- [x] **CORE-CHART-005** — Thành phần: trendline (mọi loại + phương trình/R²), error bars, data table, high-low lines, up/down bars, drop lines, trục log/ngày/phụ · P0 · 002 · test.
  - Xong: model + đọc + ghi + vẽ (bộ vẽ chung): trendline linear/exp/log/poly (2–6)/power/movingAvg (bình phương tối
    thiểu; R² trong không gian khớp như bảng tính; phương trình theo locale), thanh sai số fixedVal/percentage/stdDev
    (quanh trung bình chuỗi)/stdErr, both/plus/minus; dropLines, hiLowLines, upDownBars (biểu đồ đường), bảng dữ liệu
    `dTable` dưới vùng vẽ, trục giá trị log (logBase) + min/max, trục ngày (dateAx: số sê-ri → dd/mm/yyyy), trục phụ (nhóm
    trỏ tới valAx thứ hai: thang riêng, nhãn bên phải). Test: `bo-sheet chart::tests` (khớp đúng các hàm sinh dữ liệu: y = 2x
    + 1, y = 2e^0,5x, y = 3 + 2 ln x, y = 4x^1,5, đa thức bậc 2; trung bình trượt; độ lớn thanh sai số với √(32/7)), đọc
    (`bo-xlsx chart::tests::chart_components_are_read`), corpus `writer-charts-components-{1,2,3}.xlsx` round-trip +
    OpenXmlValidator 0 lỗi. Chưa: ghi trục phụ khi tạo mới (đọc/vẽ được).
- [ ] **CORE-CHART-006** — Map chart (dữ liệu ranh giới license hợp lệ, Việt Nam cấp tỉnh) · P2 · 004 · [!] dữ liệu.
- [ ] **CORE-CHART-007** — Mẫu biểu đồ (lưu/áp dụng), style & màu biểu đồ · P1 · 002 · test.

### Hiệu ứng 3D (§4.3)
- [x] **CORE-3D-001** — Hiển thị scene3d/sp3d bằng khối 3D thật (`bo-pptx/src/mesh3d.rs`): đùn đa giác, bevel trên/dưới theo profile, camera preset/rot, chiếu trực giao/xiên/phối cảnh (fov), loại mặt khuất + sắp mặt, chiếu sáng lightRig + material (Lambert/Blinn-Phong), contour; chung cho canvas/PNG/PDF · P0 · — · corpus `slide-shapes-3d`, mesh3d::tests, three_d::tests. Dựng bằng phần mềm thay WebGL (ADR-0010 §5). Còn lại (không chặn): `lightRig@rot`, nhiều nguồn sáng theo từng loại rig, chữ 3D (WordArt, thuộc CORE-SHAPE-002).

### Animation / transition / Morph (§7.2)
- [p] **SLIDE-ANIM-001** — Transition cơ bản (fade, push, wipe, cover, random…) + animation appear/fade/fly-in/wipe, đọc/ghi p:timing · P0 · — · corpus `slide-anim-*`. **Thiếu**: Morph, motion path, emphasis/exit đầy đủ, Animation Pane, trigger.
- [x] **SLIDE-ANIM-002** — Đủ họ transition: danh mục 55 hiệu ứng (`bo-pptx/src/transitions.rs`: §19.3.1 + [MS-PPTX] p14 + p15:prstTrans) với biến thể (hướng, qua màn đen, số nan, mẫu, vào/ra, nảy), đọc/ghi đúng `mc:AlternateContent` (Choice Requires p14/p15 + Fallback fade), âm thanh `sndAc` (WAV nhúng / dừng, lặp), thời lượng, áp dụng tất cả; phát lại từng họ (`slide-editor/src/transitionFx.ts`: mảnh clip-path, khối 3D CSS, preset p15) · P0 · — · corpus `slide-edit-transitions-all` (OpenXmlValidator cả nhánh Choice: `validate --choice`), transitions::tests, `every_transition_survives_saving`, test khung hình mọi hiệu ứng/biến thể, E2E slide-show-media.
- [p] **SLIDE-ANIM-003** — Morph ([MS-PPTX] `p159:morph`, option byObject/byWord/byChar, đọc/ghi + Fallback fade): khớp đối tượng (`!!tên` → cùng tên → cùng chữ → cùng hình/ảnh, gần nhất), nội suy vị trí/kích thước/xoay (đường ngắn)/màu (`slide_edit/morph.rs`), trình chiếu vẽ cặp hai lần (cũ → hộp mới, mới ← hộp cũ) hoà mờ nhau, đối tượng thừa mờ ra/vào · P0 · 002 · morph::tests (khớp + nội suy), `morph_pairs_shapes_of_consecutive_slides`, test khung hình `morphLayers`. **Thiếu**: chữ chuyển theo từ/ký tự (byWord/byChar hiện chạy như byObject), nội suy đỉnh hình dạng (hiện hoà mờ hai hình).
- [p] **SLIDE-ANIM-004** — Animation: bộ thông dịch hành vi §19.5 (`bo-pptx/src/anim_play.rs`: set, anim + công thức `#ppt_x…`, animEffect filter, animMotion, animRot, animScale, animClr; delay, repeatCount, autoRev, seq) → mọi hiệu ứng trong file đều phát; danh mục tạo mới (`anim_catalog.rs`): 25 xuất hiện (chạy ngược thành biến mất), 7 nhấn mạnh, 13 đường chuyển động (thẳng, cung, rẽ, tròn, vuông, thoi, tam giác, vòng, sóng) với biến thể; đọc lại theo presetID/presetSubtype hoặc so hành vi; ngăn hiệu ứng thêm/đổi hiệu ứng + biến thể; trình chiếu lấy mẫu track thành khung hình (clip theo filter trong hộp hình) · P0 · — · anim_play::tests (timeline), anim_catalog::tests, `entrance_emphasis_exit_and_path_effects`, `every_effect_survives_saving`, corpus `slide-edit-animations-all` (OpenXmlValidator), effectFx test, E2E. **Thiếu**: vẽ đường chuyển động tuỳ ý trên slide, hiển thị đổi màu (animClr) khi trình chiếu, hiệu ứng chữ theo từ/ký tự.
- [x] **SLIDE-ANIM-005** — Ngăn hiệu ứng: thanh thời gian từng hiệu ứng theo bước bấm, thứ tự, bắt đầu/chờ/thời lượng, lặp lại (§19.5.29 repeatCount: n lần / đến lần bấm sau), kích hoạt khi bấm vào một hình (chuỗi tương tác `interactiveSeq`, đọc/ghi; trình chiếu có vùng bấm, không chuyển bước), chép hiệu ứng (Animation Painter, đổi `spTgt` của hiệu ứng giữ nguyên XML) · P0 · 004 · `repeats_triggers_and_the_animation_painter`, test player (lặp, trigger ngoài bước bấm), E2E slide-show-media. Ghi chú: trigger theo dấu trang media chưa có.

### So sánh / Kết hợp tài liệu (§5.7)
- [x] **DOC-COMPARE-001** — So sánh với bản gốc (`bo-edit/src/compare.rs`, lệnh `compareWith`, nút Xem lại → So sánh): đoạn thêm/xoá (cả dấu đoạn §17.13.5.15/16, dấu đoạn cuối chuyển đúng chỗ), từ thay đổi trong đoạn (LCS theo từ, khoảng trắng lẻ không cắt vụn), định dạng (rPrChange/pPrChange), bảng (theo ô khi cùng lưới, khác lưới thì xoá/chèn hàng); một bước hoàn tác; đọc .docx/.odt/.doc · P0 · — · compare::tests (cặp tài liệu: chấp nhận hết = bản mới, từ chối hết = bản gốc), `compare_with_the_original_document`, E2E review.spec.
- [x] **DOC-COMPARE-002** — Kết hợp bản sửa (`compare::combine`, lệnh `combineWith`, nút Xem lại → Kết hợp): gốc = tài liệu này từ chối hết thay đổi; thay đổi của mình và của bản kia (tác giả theo revision trong file, không có thì theo tên file) trộn 3 chiều theo đoạn (thêm/xoá/sửa) và theo từ trong đoạn cả hai cùng sửa; bảng cả hai cùng sửa giữ bản của người thứ nhất · P0 · 001 · `two_reviewers_combine`, `combine_two_reviewers_copies`, E2E review.spec. Kết hợp nhiều hơn hai bản: lặp lại lệnh.
- [ ] **DOC-REVIEW-001** — Chế độ hiển thị Simple/All/No markup/Original + bong bóng lề + ngăn Reviewing · P1 · — · E2E.
- [ ] **DOC-REVIEW-002** — Theo dõi di chuyển (moveFrom/moveTo) và định dạng; khoá track changes bằng mật khẩu · P1 · — · round-trip + test.

### Field & content controls (§5.5)
- [p] **DOC-FIELD-001** — Field PAGE, NUMPAGES, SECTIONPAGES, DATE, TOC, SEQ, REF/PAGEREF, MERGEFIELD (đã có) · P0 · — · tests `bo-edit` fields. **Thiếu**: danh sách field §5.5 còn lại.
- [p] **DOC-FIELD-002** — Field: định dạng chung (`bo_model::field`: `\@` mẫu ngày giờ, `\#` mẫu số, `\*` chữ hoa/thường, La Mã, chữ cái, thứ tự, số bằng chữ Việt/Anh, hex) áp cho cả PAGE/NUMPAGES/SECTIONPAGES khi dàn trang; công thức `=` (toán tử, hàm SUM/AVERAGE/COUNT/MIN/MAX/ROUND/IF/AND/OR…, dấu trang, ô bảng A1/ABOVE/LEFT…); tính DATE, TIME, CREATEDATE, SAVEDATE, PRINTDATE, AUTHOR, TITLE, SUBJECT, KEYWORDS, COMMENTS, LASTSAVEDBY, TEMPLATE, FILENAME, FILESIZE, REVNUM, EDITTIME, NUMWORDS, NUMCHARS, USERNAME/INITIALS/ADDRESS, DOCPROPERTY, DOCVARIABLE, QUOTE, SYMBOL, SET, ASK/FILLIN (giá trị \d), IF, COMPARE, SECTION, STYLEREF, MACROBUTTON, GOTOBUTTON, AUTONUM/LISTNUM, TC/TA/XE/RD/PRINT… (không kết quả) + REF đọc SET; thuộc tính lấy từ docProps; hộp thoại Chèn → Trường · P0 · 001 · `every_field_kind` (bảng từng field), field::tests, `fields_show_their_results`, E2E fields.spec. **Thiếu**: INDEX/TOA (dựng chỉ mục), NOTEREF, INCLUDETEXT/INCLUDEPICTURE/LINK/DATABASE (giữ kết quả cũ), hỏi đáp ASK/FILLIN khi cập nhật.
- [x] **DOC-FIELD-003** — Hiện mã trường thay kết quả (Alt+F9, tuỳ chọn dàn trang `field_codes`, cả đầu/chân trang), cập nhật (F9, bỏ qua trường khoá), khoá / mở khoá (Ctrl+F11 / Ctrl+Shift+F11, §17.16.18 `w:fldLock` đọc/ghi cả fldSimple và fldChar), chuyển thành văn bản (Ctrl+Shift+F9) · P0 · 002 · `lock_unlink_and_show_field_codes`, test layout `{ PAGE }`, E2E fields.spec.
- [p] **DOC-CC-001** — Content controls (w:sdt): plain/rich text, picture, combo, dropdown, date, checkbox, repeating section: đọc/hiển thị/sửa/ghi · P0 · — · corpus + round-trip. Đã có: mô hình `ControlStart`/`ControlEnd` (cấp run và cấp khối quanh các đoạn), đọc/ghi §17.5.2.38 sdtPr (alias, tag, id, lock, showingPlcHdr, text, richText, picture, comboBox, dropDownList, date, w14:checkbox, w15:repeatingSection, group) — giữ nguyên XML khi không đổi, ghi cân bằng khi đoạn bị tách/xoá; chèn text/rich text/hộp kiểm/danh sách/hộp kết hợp/ngày (hộp thoại Chèn → Điều khiển nội dung), điền giá trị qua thanh điều khiển (bật/tắt hộp kiểm, chọn mục, nhập giá trị, chọn ngày theo dateFormat), khoá nội dung được tôn trọng; corpus `controls-01-forms.docx` qua OpenXmlValidator; test `content_controls_inline_and_block`, `insert_at_control_boundaries`, `content_controls_insert_fill_and_save`, `content_control_split_by_enter_still_saves_well_formed`, E2E content-controls.spec. **Thiếu:** tạo control ảnh / phần lặp (chỉ đọc-ghi), hộp thoại thuộc tính (tiêu đề, tag, khoá), bấm vào giữa placeholder chưa chọn cả placeholder, khung viền control khi hiển thị.
- [ ] **DOC-CC-002** — Custom XML parts + data binding cho content controls · P1 · CC-001 · test.
- [ ] **DOC-FORM-001** — Legacy form fields + chế độ chỉ điền biểu mẫu · P1 · CC-001 · test.

### Hàm bảng tính (§6.1) — chi tiết từng hàm trong `functions.md`
- [p] **SHEET-FN-000** — 450/515 hàm (Date & Time, Logical, Engineering, Statistical, Compatibility, Financial đủ) · P0 · — · `functions.md`.
- [x] **SHEET-FN-ENG** — Engineering (54 hàm: BESSEL*, chuyển cơ số, BIT*, COMPLEX/IM*, CONVERT, DELTA, GESTEP, ERF*) · P0 · — · bảng test từng hàm. `functions/engineering.rs`: đổi cơ số bù hai 10 chữ số, BIT* 48 bit, erf chuỗi Maclaurin + liên phân số erfc, Bessel J (truy hồi lùi Miller), Y (chuỗi Neumann), I (chuỗi), K (tích phân hình thang), số phức text `x+yi`/`x+yj`, CONVERT theo định nghĩa đơn vị chính xác (tiền tố SI và nhị phân, m2/m3 bình phương/lập phương tiền tố); test `fn_engineering` (183 dòng: tay + bảng Abramowitz & Stegun, kiểm lại SciPy) + đẳng thức Wronskian.
- [x] **SHEET-FN-STAT** — Statistical còn thiếu (60 hàm: phân phối BETA/BINOM/CHISQ/F/GAMMA/HYPGEOM/LOGNORM/NEGBINOM/POISSON/T/WEIBULL/Z, LINEST/LOGEST/TREND/GROWTH, FORECAST.ETS*, SKEW/KURT, FREQUENCY…) · P0 · — · test. `functions/distributions.rs`: log-gamma Lanczos, gamma/beta không đầy đủ chuẩn hoá (chuỗi + liên phân số Lentz), hàm ngược bằng chia đôi CDF; T.TEST 3 kiểu, F.TEST, CHISQ.TEST, Z.TEST; KURT/SKEW/TRIMMEAN/PERCENTRANK/MODE.MULT/FREQUENCY/PROB/STEYX; `functions/regression.rs`: LINEST/LOGEST (nhiều biến, thống kê 5 dòng, biến cộng tuyến hệ số 0), TREND/GROWTH, FORECAST.ETS* (Holt–Winters cộng, tối ưu hằng số làm trơn theo SSE, tự dò mùa bằng tự tương quan — tham số có thể khác phần mềm tham chiếu, chưa có giá trị tham chiếu); test `fn_distributions`, `fn_regression` (giá trị chuẩn tính độc lập bằng SciPy/NumPy, chuỗi chính xác cho ETS).
- [x] **SHEET-FN-FIN** — Financial còn thiếu (36 hàm: trái phiếu/chứng khoán, CUMIPMT, MIRR, VDB…) · P0 · — · test. `functions/bonds.rs`: lịch coupon (lùi từ ngày đáo hạn, giữ cuối tháng), COUP*, PRICE/YIELD/DURATION/MDURATION, ODDF*/ODDL* (kỳ lẻ theo kỳ coupon giả định), ACCRINT (Σ Aᵢ/NLᵢ)/ACCRINTM, DISC/INTRATE/RECEIVED/PRICEDISC/YIELDDISC/PRICEMAT/YIELDMAT, TBILL*, DOLLARDE/FR, FVSCHEDULE, ISPMT, MIRR, PDURATION, RRI, VDB, AMORLINC/AMORDEGRC; test `fn_securities` (tính tay theo định nghĩa + đẳng thức: kỳ đều của ODD* = PRICE, YIELD nghịch đảo PRICE, duration trái phiếu ngang giá).
- [ ] **SHEET-FN-DB** — Database (12 hàm D*) · P0 · — · test.
- [x] **SHEET-FN-COMPAT** — Compatibility còn thiếu (25 hàm) · P0 · STAT · test. 42/42: tên cũ dùng chung cài đặt với hàm mới (BETADIST, CHIDIST, TDIST hai/một đuôi, HYPGEOMDIST/NEGBINOMDIST chỉ xác suất điểm…), test cùng bảng `fn_distributions`.
- [ ] **SHEET-FN-TEXT** — Text còn thiếu (17 hàm: REGEX*, ARRAYTOTEXT, VALUETOTEXT, FIXED, DOLLAR, BAHTTEXT, ASC/DBCS, *B…) + `BO.DOCSO` (đọc số tiền tiếng Việt) · P0 · — · test.
- [ ] **SHEET-FN-LOOKUP** — Lookup còn thiếu (13 hàm: AREAS, GROUPBY, PIVOTBY, TRIMRANGE, HYPERLINK, GETPIVOTDATA, RTD, IMAGE…) · P0 · — · test.
- [ ] **SHEET-FN-MATH** — Math còn thiếu (10 hàm: SERIESSUM, MULTINOMIAL, MINVERSE/MDETERM/MUNIT, PERCENTOF…) · P0 · — · test.
- [ ] **SHEET-FN-INFO** — Information còn thiếu (INFO, CELL đầy đủ…) · P0 · — · test.
- [ ] **SHEET-FN-WEB** — ENCODEURL; WEBSERVICE/FILTERXML (mặc định tắt, quản trị bật) · P1 · — · test.
- [ ] **SHEET-FN-CUBE** — Cube: giữ công thức + giá trị cache · P2 · — · round-trip.
- [ ] **SHEET-FORMULA-002** — Ghi tiền tố `_xlfn.` / `_xlws.` cho hàm tương lai ([MS-XLSX] 2.2.2) khi lưu .xlsx (hiện ghi nguyên văn công thức người dùng nhập) · P0 · — · round-trip + OpenXmlValidator.
- [ ] **SHEET-FORMULA-001** — Toán tử @ (implicit intersection), trim ref (.:.), CSE kiểu cũ, liên kết workbook ngoài (cập nhật/sửa/ngắt) · P0 · — · test.

### Slicer / Timeline / PivotChart (§6.2)
- [p] **SHEET-PIVOT-001** — PivotTable (tạo/sửa, tổng phụ, bố cục, nhóm, trường tính toán, show values as, lọc/top N, style, refresh) · P0 · — · tests `bo-sheet` pivot. **Thiếu**: drill-down, nhiều pivot chung cache UI, đổi nguồn.
- [p] **SHEET-PIVOT-002** — PivotChart (đã có biểu đồ pivot cơ bản) · P0 · — · test. **Thiếu**: điều khiển lọc trên biểu đồ.
- [ ] **SHEET-SLICER-001** — Slicer: đọc/ghi OOXML (slicer cache, slicer parts), hiển thị, lọc pivot/bảng, điều khiển nhiều pivot · P0 · — · corpus + round-trip + E2E.
- [ ] **SHEET-TIMELINE-001** — Timeline (bộ lọc ngày) đọc/ghi/hiển thị/lọc · P0 · SLICER-001 · test.

### What-if (§6.2)
- [ ] **SHEET-WHATIF-001** — Goal Seek · P0 · — · test hội tụ.
- [ ] **SHEET-WHATIF-002** — Data Table 1 và 2 biến ({=TABLE()}) đọc/ghi/tính · P0 · — · test.
- [ ] **SHEET-WHATIF-003** — Scenario Manager + báo cáo tóm tắt · P1 · — · test.
- [ ] **SHEET-SOLVER-001** — Solver: simplex (tuyến tính), ràng buộc nguyên/nhị phân (branch & bound) · P0 · — · bài toán chuẩn có nghiệm đã biết.
- [ ] **SHEET-SOLVER-002** — Solver GRG (phi tuyến), tiến hoá, alldifferent, báo cáo answer/sensitivity/limits · P1 · 001 · test.

---

## P1
- [ ] **SHEET-PQ-001** — Lấy & biến đổi dữ liệu: kết nối CSV/JSON/XML/Excel/thư mục/web table, trình biên tập bước, lưu truy vấn, refresh; giữ định nghĩa truy vấn trong file · P1 · — · test.
- [ ] **SHEET-DM-001** — Data model: quan hệ bảng, pivot nhiều bảng, measure (ngôn ngữ biểu thức riêng) · P1 · PQ-001 · test.
- [ ] **SHEET-FLASH-001** — Flash Fill (học mẫu từ ví dụ) · P1 · — · bảng test mẫu.
- [ ] **SHEET-ATP-001** — Bộ công cụ phân tích (ANOVA, tương quan, hiệp phương sai, mô tả, làm trơn mũ, F-test, Fourier, histogram, trung bình trượt, ngẫu nhiên, xếp hạng, hồi quy, lấy mẫu, t-test, z-test) · P1 · FN-STAT · test.
- [ ] **CORE-SCRIPT-001** — BlueScript: sandbox JS (Web Worker/QuickJS license hợp lệ), API đối tượng Doc/Sheet/Slide · P1 · — · test API.
- [ ] **CORE-SCRIPT-002** — Ghi thao tác thành script (recorder), gán nút/phím tắt, chạy theo lịch (server) · P1 · 001 · E2E.
- [ ] **CORE-SCRIPT-003** — Chuyển VBA → BlueScript (best-effort) · P2 · 001 · test.
- [ ] **SLIDE-REC-001** — Ghi bài thuyết trình (âm thanh, camera, ink, timing) + rehearse timings · P1 · — · E2E.
- [ ] **SLIDE-EXPORT-001** — Xuất video MP4 (WebCodecs web; encoder license hợp lệ desktop/server) + GIF động · P1 · — · test khung hình.
- [ ] **CORE-3DMODEL-001** — Mô hình 3D glTF/GLB/OBJ/STL: chèn, xoay/zoom, đọc/ghi OOXML (am3d), trình chiếu · P1 · CORE-3D-001 · test.
- [ ] **CORE-INK-001** — Ink: bút/bút dạ/tẩy/thước, áp lực, đọc/ghi InkML trong OOXML, phát lại · P1 · — · round-trip.
- [ ] **CORE-INK-002** — Chuyển nét vẽ thành shape/văn bản/toán · P2 · 001 · test.
- [ ] **SHEET-FORMCTL-001** — Form controls (button/checkbox/combo/list/option/scroll/spinner/group/label): đọc/ghi/hoạt động; ActiveX giữ nguyên + ảnh · P1 · — · test.
- [ ] **DOC-TPL-001** — Thư viện mẫu hành chính VN (công văn, quyết định, tờ trình, biên bản, hợp đồng, báo cáo, giấy mời, kế hoạch) theo NĐ 30/2020 · P1 · — · OpenXmlValidator + kiểm tra thể thức.
- [ ] **DOC-TPL-002** — Kiểm tra thể thức văn bản hành chính tự động (font, cỡ, lề, vị trí thành phần) · P1 · 001 · bảng test.
- [ ] **CORE-LANG-001** — `TranslationProvider` (API cấu hình được + cục bộ Việt↔Anh, model license hợp lệ) · P1 · — · test với provider giả; [!] model cục bộ.
- [ ] **CORE-LANG-002** — `DictationProvider` (cục bộ, tiếng Việt, lệnh dấu câu) · P1 · — · [!] model.
- [ ] **CORE-LANG-003** — `SpeechProvider` đọc to + highlight từ · P1 · — · test với Web Speech/provider giả.
- [ ] **CORE-LANG-004** — Trình đọc tập trung (immersive reader) · P2 · — · E2E.
- [ ] **CORE-AUTOCORR-001** — AutoCorrect (danh sách thay thế, viết hoa đầu câu, hai chữ hoa, ký hiệu, nháy thông minh, gạch ngang, phân số, tự đánh số/gạch đầu dòng, link) · P1 · — · bảng test.
- [ ] **CORE-THES-001** — Từ điển đồng nghĩa (Anh: WordNet license kiểu BSD; Việt nếu có) · P2 · — · test.
- [ ] **CORE-FIND-001** — Tìm/thay nâng cao: ký tự đặc biệt (^p ^t), wildcard, highlight tất cả · P1 · — · test.
- [ ] **CORE-PASTE-001** — Dán: giữ nguồn/gộp/chỉ văn bản/ảnh/liên kết + lịch sử clipboard · P1 · — · E2E.
- [ ] **CORE-SYMBOL-001** — Bảng ký tự Unicode có tìm kiếm · P1 · — · E2E.
- [ ] **CORE-UI-001** — Tab ngữ cảnh (bảng/ảnh/biểu đồ/shape/header), ribbon đơn giản (một hàng) · P1 · — · E2E.
- [ ] **CORE-UI-002** — Backstage File đầy đủ (Mới từ mẫu, Mở gần đây/ghim, Thông tin, Lưu thành mọi định dạng, In xem trước, Xuất, Tuỳ chọn) · P1 · — · E2E.
- [ ] **CORE-UI-003** — Tìm lệnh ("Tìm kiếm thao tác"), mini toolbar, menu chuột phải theo ngữ cảnh · P1 · — · E2E.
- [ ] **CORE-UI-004** — Phím tắt Alt-key (KeyTips), tuỳ biến phím tắt, F4 lặp lệnh/đổi tham chiếu · P1 · — · E2E.
- [ ] **CORE-UI-005** — Tuỳ biến ribbon/QAT (thêm/bớt, xuất/nhập) · P2 · — · E2E.
- [ ] **CORE-UI-006** — Task pane dock/float (Styles, Navigation, Selection, Format Shape, Comments, Version History) · P1 · — · E2E.
- [ ] **CORE-UI-007** — Thanh trạng thái tuỳ biến, độ tương phản cao, đa cửa sổ/chia màn hình/cuộn đồng bộ · P2 · — · E2E.
- [ ] **CORE-PRINT-001** — In: xem trước chính xác, chọn trang/vùng, nhiều trang/tờ, co giãn, in markup, handout/notes, in tiêu đề/lưới bảng tính · P1 · — · test PDF đầu ra.
- [ ] **CORE-SEC-001** — Đánh dấu bản cuối, chỉ đọc khuyến nghị, giới hạn chỉnh sửa theo vùng/người, nhãn độ mật · P1 · — · round-trip.
- [ ] **CORE-INSPECT-001** — Kiểm tra tài liệu (xoá metadata/comment/ẩn), kiểm tra khả năng tiếp cận, kiểm tra tương thích · P1 · — · test.
- [ ] **CORE-SIGLINE-001** — Dòng chữ ký (signature line) trong văn bản, nhiều chữ ký · P1 · — · round-trip + ký.
- [ ] **CORE-FMT-001** — Mẫu/biến thể macro: .dotx/.dotm/.docm/.xltx/.xltm/.xlsm/.potx/.potm/.pptm/.ppsx/.ppsm, .thmx · P1 · — · round-trip.
- [ ] **CORE-FMT-002** — Xuất PDF/A-1b/2b/3b, PDF có thẻ (tagged), bookmark từ heading · P1 · — · validator PDF/A (veraPDF nếu license cho phép).
- [ ] **CORE-FMT-003** — HTML/MHT lưu & mở; Flat OPC XML; Excel XML 2003 đọc · P2 · — · round-trip.
- [ ] **DOC-VIEW-001** — Read Mode, Web Layout, Outline (kéo thả đề mục), Draft, Focus, Navigation pane (heading/thumbnail/kết quả tìm), ký tự ẩn ¶ · P1 · — · E2E.
- [ ] **DOC-FMT-001** — Thuộc tính ký tự còn thiếu (emboss/engrave/outline/shadow chữ, ligature/OpenType, ruby, combine, viền ký tự) · P1 · — · corpus + render.
- [ ] **DOC-LAYOUT-001** — Drop cap, cột không đều + đường kẻ, đánh số dòng, gạch nối tự động, chữ dọc · P1 · — · corpus + render.
- [ ] **DOC-PAGE-001** — Viền trang (kể cả nghệ thuật — bộ tự thiết kế), màu nền trang, watermark, theme/style set · P1 · — · corpus + render.
- [ ] **DOC-STYLE-001** — Style pane/inspector/organizer, Reveal Formatting · P2 · — · E2E.
- [ ] **DOC-LIST-001** — Định nghĩa danh sách đa cấp mới, liên kết heading, đặt giá trị số · P1 · — · test.
- [ ] **DOC-BB-001** — Building blocks / Quick Parts / AutoText + thư viện khối tự thiết kế · P2 · — · round-trip glossary part.
- [ ] **DOC-TABLE-001** — Vẽ/tẩy bảng, phân phối đều, AutoFit, sắp xếp trong bảng, công thức bảng (SUM(ABOVE)), văn bản↔bảng, bảng nổi, chữ dọc trong ô · P1 · — · test.
- [ ] **DOC-REF-001** — Mục lục hình/bảng, chỉ mục (XE/INDEX), bảng trích dẫn pháp lý (TA/TOA), chuyển footnote↔endnote · P1 · FIELD-002 · test.
- [ ] **DOC-MERGE-001** — Mail merge mở rộng: nguồn SQL/danh bạ, quy tắc IF/Skip/Ask/Fill-in, lọc/sắp xếp người nhận, trộn ra email/PDF từng người; phong bì & nhãn · P1 · — · test.
- [ ] **SHEET-DATA-001** — Sort theo màu/icon/danh sách/hàng; AutoFilter theo màu/top 10/trên dưới TB/ngày động; Advanced Filter · P1 · — · test.
- [ ] **SHEET-DATA-002** — Text to Columns, Remove Duplicates, Consolidate, Subtotal, auto Outline, khoanh dữ liệu không hợp lệ · P1 · — · test.
- [ ] **SHEET-CF-001** — CF đầy đủ: data bar âm/trục, icon set mọi bộ + tuỳ biến, Stop If True, CF cho pivot · P1 · — · corpus + render.
- [ ] **SHEET-NUMFMT-001** — Mã định dạng: lịch âm/locale, [DBNum], `*` lặp, `_`; xoay chữ mọi góc/dọc/shrink · P1 · — · bảng test.
- [ ] **SHEET-SPARK-001** — Sparklines (line/column/win-loss): đọc/ghi/hiển thị · P1 · — · corpus.
- [ ] **SHEET-CELL-001** — Checkbox trong ô, ảnh trong ô (place in cell), hàm IMAGE · P1 · FN-LOOKUP · test.
- [ ] **SHEET-TABLE-001** — Bảng: total row, cột tính toán tự lan, tự mở rộng, chuyển về range · P1 · — · test.
- [ ] **SHEET-VIEW-001** — Page Layout view, Page Break Preview (kéo ngắt), Custom Views, Show Formulas, Sheet View · P1 · — · E2E.
- [ ] **SHEET-AUDIT-001** — Trace Precedents/Dependents, Error Checking, Evaluate Formula, Watch Window, tuỳ chọn tính (thủ công, lặp, precision as displayed) · P1 · — · test.
- [ ] **SHEET-PRINT-001** — In bảng tính: fit N×M, căn giữa, header/footer mã &P &N &D &F &A, thứ tự trang, in lưới/tiêu đề/lỗi/comment · P1 · — · test PDF.
- [ ] **SHEET-PERF-001** — 1.048.576 × 16.384, pivot 1 triệu dòng < 3 s, sort/lọc 1 triệu dòng < 2 s, 10 triệu ô mở được · P1 · — · benchmark.
- [ ] **SLIDE-VIEW-001** — Outline view, Notes Page, Slide/Handout/Notes Master view, sections · P1 · — · E2E.
- [ ] **SLIDE-MASTER-001** — Sửa master/layout, thêm layout/placeholder, nhiều master, theme/variant, đổi kích thước slide (scale/ensure fit) · P1 · — · round-trip.
- [ ] **SLIDE-ZOOM-001** — Summary/Section/Slide Zoom · P2 · — · round-trip + trình chiếu.
- [ ] **SLIDE-MEDIA-001** — Audio/video: trim, fade, bookmark, phát tự động/lặp/qua nhiều slide, poster, phụ đề WebVTT · P1 · — · round-trip.
- [ ] **SLIDE-SHOW-001** — Custom show, kiosk, rehearse; Presenter: zoom vùng, bút/laser, đen/trắng màn hình, chiếu màn hình ngoài · P1 · — · E2E.
- [ ] **SLIDE-EXPORT-002** — PDF handout 1/2/3/4/6/9, ảnh PNG/JPG/SVG, gói trình chiếu · P1 · — · test.
- [ ] **CORE-SHAPE-001** — Edit Points, merge shapes (union/combine/fragment/intersect/subtract), connector bám điểm nối · P1 · — · test hình học.
- [ ] **CORE-SHAPE-002** — Hiệu ứng: reflection, soft edges, 3D rotation; WordArt (text warp prstTxWarp, outline/gradient chữ) · P1 · CORE-3D-001 · render.
- [ ] **CORE-PIC-001** — Ảnh: crop theo tỷ lệ/hình, chỉnh sáng/tương phản/nét, recolor, nén, thay ảnh, SVG → shape · P1 · — · test.
- [ ] **CORE-PIC-002** — Xoá nền ảnh (mô hình phân đoạn license hợp lệ chạy cục bộ) · P2 · — · [!] model.
- [ ] **CORE-ICON-001** — Thư viện icon/ảnh mẫu CC0/MIT có tìm kiếm · P2 · — · test.
- [ ] **CORE-EMBED-001** — Nhúng/liên kết bảng BlueSheet trong Doc/Slide, cập nhật liên kết; OLE: ảnh preview + giữ dữ liệu · P1 · — · round-trip.
- [ ] **CORE-COLLAB-001** — Cộng tác cho mọi đối tượng mới + hiển thị ai đang sửa đoạn/ô/slide · P1 · — · test hội tụ.
- [ ] **CORE-COMMENT-001** — @mention (thông báo BlueDrive/email), giao việc trong comment · P1 · — · test server.
- [ ] **CORE-VERSION-001** — So sánh phiên bản, Show Changes bảng tính · P1 · DOC-COMPARE-001 · test.
- [ ] **CORE-PERF-001** — .docx 1.000 trang < 8 s, .pptx 300 slide có video < 5 s, không rò rỉ khi mở/đóng 100 tài liệu · P1 · — · benchmark + test.

## P2
- [ ] **CORE-ADDIN-001** — Nền tảng add-in web (manifest, task pane, lệnh ribbon, quyền), API JS, marketplace nội bộ · P2 · SCRIPT-001.
- [ ] **CORE-AI-001** — BlueAI (provider LLM cấu hình, mặc định tắt, on-premise): viết lại, tóm tắt, soạn nháp, slide từ dàn ý, gợi ý công thức · P2 · —.
- [ ] **CORE-DESIGN-001** — Gợi ý thiết kế slide (rule-based), gợi ý biểu đồ, phân tích nhanh (Quick Analysis) · P2 · —.
- [ ] **CORE-CITE-001** — Trích dẫn & tài liệu tham khảo (APA, MLA, Chicago, IEEE, Harvard, ISO 690, kiểu VN), import BibTeX/RIS · P2 · FIELD-002.
- [ ] **CORE-FMT-004** — .epub (xuất), .xps (xuất), .prn/.dif/.slk, .dbf đọc, .wpd/.wps (nếu khả thi), mở PDF để sửa (best-effort) · P2 · —.
- [ ] **CORE-API-001** — API REST server tạo/sửa/chuyển đổi tài liệu (mail merge hàng loạt, báo cáo) · P2 · —.
- [ ] **SLIDE-ACTION-001** — Hyperlink & action (chuyển slide, âm thanh, hover; chương trình chỉ trong phạm vi an toàn) · P2 · —.
- [ ] **SLIDE-REMOTE-001** — Điều khiển trình chiếu từ điện thoại, chiếu trực tuyến qua link, phụ đề trực tiếp · P2 · LANG-002.
- [ ] **SLIDE-ALBUM-001** — Photo album, outline import từ BlueDoc, reuse slides UI · P2 · —.

## P3
- [ ] **CORE-MISC-001** — Phần còn lại phát hiện khi rà soát định kỳ (bổ sung tại đây).

---

## Đã xong trước giai đoạn Parity (giữ để MATRIX tham chiếu)
- [x] **CORE-COLLAB-000** — Đồng tác giả thời gian thực doc/sheet/slide, presence (test `bo-engine/tests/collab.rs`, E2E cộng tác).
- [x] **CORE-SIGN-000** — Ký số PAdES/OOXML RSA+ECDSA, B-T/B-LT/B-LTA, OCSP/CRL, PKCS#11, ký từ xa, xác minh chữ ký phần mềm khác (test `bo-sign`).
- [x] **CORE-CRYPT-000** — Mật khẩu mở/sửa (Agile/Standard), bảo vệ sheet/workbook (test `bo-crypto`, interop msoffcrypto).
- [x] **CORE-VBA-000** — VBA giữ nguyên + cảnh báo, không thực thi (— chủ ý).
- [x] **CORE-AUTOSAVE-000** — Tự lưu, khôi phục sau sự cố (E2E doc).
- [x] **DOC-TRACK-000** — Track changes ins/del/rPr/pPr/bảng/ô, accept/reject, comment luồng (test `bo-edit`, E2E).
- [x] **DOC-REF-000** — TOC, caption, cross-reference, footnote/endnote, bookmark (test `bo-edit`).
- [x] **DOC-MERGE-000** — Mail merge CSV/XLSX (test `bo-edit`).
