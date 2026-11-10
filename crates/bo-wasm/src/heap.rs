//! Heap sizing before opening big files.
//!
//! The allocator of `wasm32-unknown-unknown` (dlmalloc in `std`) grows the WebAssembly
//! memory by just what each allocation needs (64 KiB granularity). Growing a memory
//! copies it in V8 when it is bounds-checked (measured in Node and Chrome on
//! Windows: 6 400 single-page grows up to 400 MB take 4–12 s), so building a large
//! model one small allocation at a time is quadratic. Allocating one block of the
//! expected size and freeing it grows the memory once; the freed block stays in the
//! allocator's pool and serves the allocations that follow.

/// Below this, files open fast enough without reserving.
const MIN_RESERVE: usize = 16 << 20;
/// Never reserve more than this (prompt §5: WASM memory < 1 GB).
const MAX_RESERVE: usize = 640 << 20;

/// Memory needed per byte of uncompressed spreadsheet XML (1 000 000 cells:
/// 30.7 MB of XML, ~400 MB of WASM memory).
pub const SHEET_FACTOR: u64 = 12;

/// Memory needed per byte of uncompressed document / presentation XML (CORE-PERF-001,
/// measured: a 1 000-page `.docx` with 7.1 MB of XML uses ~300 MB once laid out).
pub const DOC_XML_FACTOR: u64 = 36;
/// Memory needed per byte of the other parts (media): the package bytes, the parts
/// read from them and the pictures handed to the renderer.
pub const DOC_MEDIA_FACTOR: u64 = 3;

/// Make about `bytes` of heap available with a single memory grow.
pub fn reserve(bytes: u64) {
    let n = usize::try_from(bytes)
        .unwrap_or(MAX_RESERVE)
        .min(MAX_RESERVE);
    if n < MIN_RESERVE || !cfg!(target_arch = "wasm32") {
        return;
    }
    let mut block: Vec<u8> = Vec::new();
    // Failure only means the memory could not grow that far: open anyway.
    let _ = block.try_reserve_exact(n);
}

/// [`reserve`] for an OOXML package whose XML parts total `xml` bytes: `factor`
/// bytes of memory per byte of XML.
pub fn reserve_for_package(bytes: &[u8], factor: u64) {
    if let Some(xml) = bo_opc::Package::xml_size(bytes) {
        reserve(xml.saturating_mul(factor));
    }
}

/// [`reserve`] before opening a document or presentation package (BlueDoc, BlueSlide):
/// without it, laying out a 1 000-page document spends most of its time growing the
/// memory one allocation at a time (measured: 60 % of an 8.5 s open in Node).
pub fn reserve_for_document(bytes: &[u8]) {
    if let Some((xml, other)) = bo_opc::Package::part_sizes(bytes) {
        reserve(
            xml.saturating_mul(DOC_XML_FACTOR)
                .saturating_add(other.saturating_mul(DOC_MEDIA_FACTOR)),
        );
    }
}
