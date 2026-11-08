# Mã hoá & ký số — ghi chú đọc spec

Nguồn: [MS-OFFCRYPTO], [MS-CFB], ECMA-376 Part 1 (§18.2.29, §18.3.1.85 — thuộc tính bảo vệ) và Part 2 §12 (Digital
Signatures), W3C XML-Signature và Canonical XML 1.0, ISO 32000-1, ETSI EN 319 142-1 / EN 319 122-1 (PAdES/CAdES B-B),
RFC 5652 (CMS), RFC 5035 (ESS signing-certificate-v2). Chỉ diễn giải và dẫn số mục, không chép nội dung spec.
Quyết định kiến trúc: [ADR-0007](../adr/0007-digital-signatures.md).

## 1. Phạm vi đã cài

### `crates/bo-crypto` — tài liệu có mật khẩu
- **Vỏ CFB** — `src/cfbio.rs`: file OOXML mã hoá là Compound File chứa `EncryptionInfo` + `EncryptedPackage` ([MS-OFFCRYPTO]
  §2.3.4.4 / §2.3.4.10); khi ghi, sinh thêm storage `\x06DataSpaces` (DataSpaceVersionInfo §2.1.5, DataSpaceMap §2.1.6,
  DataSpaceDefinition §2.1.7, TransformInfoHeader §2.1.8), chuỗi UNICODE-LP-P4 §2.1.2. Đọc/ghi CFB dùng crate `cfb` 0.14.
- **EncryptionInfo** — `src/info.rs`: Version §2.3.2, EncryptionHeader/EncryptionVerifier (§2.3.2, §2.3.3), Standard §2.3.4.5,
  XML Agile §2.3.4.10 (CT_KeyData, key encryptor mật khẩu).
- **Standard Encryption** — `src/standard.rs` (§2.3.4.5–§2.3.4.9): sinh khoá §2.3.4.7 (SHA-1, 50 000 vòng cố định), kiểm
  mật khẩu §2.3.4.9, AES-ECB; giải mã và mã hoá (AES-128).
- **Agile Encryption** — `src/agile.rs` (§2.3.4.10–§2.3.4.15): sinh khoá theo block key §2.3.4.11, IV §2.3.4.12, kiểm mật khẩu
  §2.3.4.13, toàn vẹn dữ liệu HMAC §2.3.4.14, mã hoá theo đoạn 4096 byte §2.3.4.15. Hash SHA-1/256/384/512. Mặc định khi
  lưu: AES-256, SHA-512, 100 000 vòng (`src/lib.rs`).
- **Hash bảo vệ** — `src/protection.rs`: hash 16-bit cũ (§2.3.7.1; thuộc tính `password` của sheetProtection/
  workbookProtection, [MS-XLS] §2.2.9) và hash lặp kiểu ISO write protection (§2.4.2.4: algorithmName/hashValue/saltValue/
  spinCount).
- **Nối vào app**: `bo-engine/src/lib.rs` phát hiện CFB + `is_encrypted`, giải mã khi mở, mã hoá Agile khi lưu có mật khẩu;
  `bo-cli` dùng `open_package` / `save_package`.
- **Thư viện** (`crates/bo-crypto/Cargo.toml`): RustCrypto `aes` 0.8, `cbc` 0.1, `ecb` 0.1, `sha1` 0.10, `sha2` 0.10,
  `hmac` 0.12; `getrandom` 0.4 (feature `wasm_js` cho WASM), `base64`, `quick-xml`, `cfb`.

### `crates/bo-sign` — ký số
- **Trait `Signer`** (`src/lib.rs`): chỉ đưa digest SHA-256 ra ngoài, nhận chữ ký PKCS#1 v1.5 — điểm nối USB token
  (PKCS#11 qua tiến trình trợ giúp) và ký từ xa của CA; `SoftwareSigner` cho test/máy chủ tự giữ khoá. Đọc khoá/chứng thư
  PEM (RFC 7468).
- **CMS** — `src/cms_sig.rs`: `SignedData` tách rời (RFC 5652), thuộc tính ký content-type, message-digest,
  signing-certificate-v2 (RFC 5035 §3, §4) theo CAdES/PAdES B-B (EN 319 122-1 §5.2, EN 319 142-1 §6.3); chữ ký phủ DER của
  SET OF thuộc tính (RFC 5652 §5.4).
- **PAdES** — `src/pdf.rs`: cập nhật tăng dần (ISO 32000-1 §7.5.6) trên bảng xref cổ điển (§7.5.4); trường chữ ký + widget ẩn
  trên trang 1 (§12.7.4.5, §12.5.6.19), `/AcroForm /SigFlags 3` (§12.7.2), từ điển chữ ký §12.8.1 với `SubFilter
  ETSI.CAdES.detached`, `/ByteRange` băm mọi thứ trừ `/Contents`. Chuỗi văn bản §7.9.2.2, ngày §7.9.4.
- **Chữ ký gói OOXML** — `src/ooxml.rs` (ECMA-376 Part 2 §12): part origin `/_xmlsignatures/origin.sigs` và `sigN.xml`
  với quan hệ/content type §12.2.3; một `Object` chứa `Manifest` gồm SHA-256 của mọi part, các part relationships qua
  Relationship Transform (§12.2.4.24) + C14N, thuộc tính `SignatureTime` (§12.2.4.20). Thuật toán `rsa-sha256` của
  xmldsig-more.
- **Thư viện** (`crates/bo-sign/Cargo.toml`): `rsa` 0.9 (feature sha2), `sha2`, `x509-cert` 0.2, `cms` 0.2, `der` 0.7, `spki`,
  `const-oid`, `base64`. Lý do chọn RustCrypto thay OpenSSL/ring hoặc ký trong .NET: ADR-0007.

## 2. Điểm mơ hồ & cách đã chọn

- **Kích thước verifier hash (Standard)** (`info.rs`): đọc 32 byte khi AES (hash đã đệm), 20 byte khi RC4; SaltSize khác 16 bị
  coi là hỏng.
- **Cắt/đệm khoá dẫn xuất** (`primitives.rs`): cắt bớt hoặc đệm byte 0x36 tới độ dài yêu cầu (§2.3.4.11/§2.3.4.12); test vector
  trong `lib.rs` kiểm cả trường hợp khoá 128 bit cắt từ SHA-1.
- **Chỉ key encryptor mật khẩu**: gói Agile chỉ có key encryptor chứng thư bị báo `Unsupported` (`info.rs`).
- **XML chữ ký sinh sẵn ở dạng Canonical XML 1.0** (`ooxml.rs`): không khoảng trắng thừa, phần tử rỗng mở rộng, thuộc tính
  sắp xếp — nên khi chuẩn hoá `SignedInfo`/`Object` chỉ cần chèn khai báo namespace mặc định kế thừa (trục namespace C14N
  §2.4). Hệ quả: `verify_ooxml` chỉ kiểm được chữ ký do BlueOffice sinh; chữ ký của phần mềm khác báo `Unsupported` (vẫn giữ
  nguyên khi lưu nhờ round-trip lossless).
- **Thời điểm ký PDF** đặt ở `/M` của từ điển chữ ký, không phải thuộc tính CMS (`cms_sig.rs`).
- **Phần đệm `/Contents`** (`pdf.rs`): khi xác minh, cắt số 0 đệm cuối dựa trên độ dài DER.
- **Tin cậy chứng thư**: `verify_pdf` / CMS chỉ kiểm digest + RSA; xác thực chuỗi tới Root CA là việc của bên gọi.

## 3. Quan sát black-box

- Kiểm chéo hai chiều với cài đặt độc lập msoffcrypto-tool 6.1.0 (MIT) — `crates/bo-crypto/tests/interop.rs`
  (`decrypts_a_package_encrypted_by_msoffcrypto_tool`, `package_encrypted_by_bo_crypto_round_trips`). Ghi nhận: công cụ
  này ghi ra compound file không đọc được với gói nhỏ hơn ngưỡng mini-stream, nên vector test dùng gói > 4096 byte.
- Không có ghi chép kiểm chéo chữ ký OOXML/PDF với phần mềm khác; test `crates/bo-sign/tests/sign.rs` kiểm ký–xác minh,
  ký lần hai giữ chữ ký đầu, phát hiện sửa đổi (PDF dùng `lopdf` làm dev-dependency để đọc lại).

## 4. Chưa hỗ trợ

- RC4 / CryptoAPI cho định dạng nhị phân (`.doc`, `.xls` FilePass, `.ppt`) — `crates/bo-crypto/src/lib.rs`.
- Key encryptor chứng thư (Agile) — `info.rs`.
- Hash bảo vệ (`protection.rs`) hiện chưa được crate nào khác gọi (grep `bo_crypto::protection` ngoài crate không thấy).
- PDF dùng cross-reference stream (§7.5.8) → `SignError::Unsupported`; `bo-pdf` sinh xref cổ điển nên không ảnh hưởng.
- Chưa có dấu thời gian RFC 3161 (PAdES B-T) và kiểm chuỗi CA quốc gia — `docs/PROGRESS.md` mục 8, ADR-0007.
- Chưa có `idOfficeObject` / XAdES theo [MS-OFFCRYPTO] §2.5 cho chữ ký OOXML — ADR-0007.
- Chưa có PKCS#11, client CA ký từ xa, UI ký — `docs/REQUIREMENTS.md` P6.13.
- `rsa` 0.9 có RUSTSEC-2023-0071 (rò rỉ thời gian), chỉ ảnh hưởng `SoftwareSigner` — ADR-0007.
