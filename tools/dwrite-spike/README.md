# DirectWrite experiment

This standalone Windows-only experiment loads a TTF from memory, shapes Vietnamese text with
`bo-text`, and passes the resulting glyph IDs, advances and offsets to DirectWrite for a
ClearType alpha masks for DPR 1, 1.25, 1.5, 1.75 and 2. Run from the repository root:

```powershell
cargo run --manifest-path tools/dwrite-spike/Cargo.toml
```

An optional argument supplies another TTF; a second argument supplies text. The default is the
bundled Tinos Regular and “Việt Nam”. This is a diagnostic, not a document renderer: it
does not perform font fallback, handle RTL or nonzero collection indices, composite the mask,
draw into a native surface, handle IME, test multi-monitor DPI, or establish text quality or
performance.
The memory loader copies the bytes and is unregistered after the face and analysis are gone.
