# Test vectors

| File | Source | Password | Plaintext |
|---|---|---|---|
| `msoffcrypto-agile.docx` | Encrypted with **msoffcrypto-tool 6.1.0** (MIT, Python, independent [MS-OFFCRYPTO] implementation; used only as an external tool, not a dependency): `OOXMLFile(open(src)).encrypt("Mật khẩu 123", out)` — ECMA-376 Agile Encryption (AES-256, SHA-512) | `Mật khẩu 123` | `tests/corpus/doc/smartart-03-grid.docx` |

Cross-checks (2026-11-08): `bo-crypto` decrypts this vector to the exact plaintext bytes (`tests/interop.rs`), and
msoffcrypto-tool decrypts packages produced by `bo-cli convert … --protect` (manual check, see `docs/plan/phase-5.md`).
Note: msoffcrypto-tool 6.1.0 writes an invalid compound file for packages smaller than 4096 bytes (it cannot decrypt
them itself), so the vector uses a larger plaintext.
