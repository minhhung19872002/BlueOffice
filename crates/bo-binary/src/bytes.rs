//! Bounds-checked little-endian readers.

use crate::{malformed, Result};

/// Read `N` bytes at `off`.
pub fn arr<const N: usize>(b: &[u8], off: usize) -> Result<[u8; N]> {
    off.checked_add(N)
        .and_then(|end| b.get(off..end))
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| malformed(format!("read of {N} bytes at {off} out of bounds")))
}

pub fn u8_at(b: &[u8], off: usize) -> Result<u8> {
    b.get(off)
        .copied()
        .ok_or_else(|| malformed(format!("read at {off} out of bounds")))
}

pub fn u16_at(b: &[u8], off: usize) -> Result<u16> {
    arr::<2>(b, off).map(u16::from_le_bytes)
}

pub fn i16_at(b: &[u8], off: usize) -> Result<i16> {
    arr::<2>(b, off).map(i16::from_le_bytes)
}

pub fn u32_at(b: &[u8], off: usize) -> Result<u32> {
    arr::<4>(b, off).map(u32::from_le_bytes)
}

pub fn i32_at(b: &[u8], off: usize) -> Result<i32> {
    arr::<4>(b, off).map(i32::from_le_bytes)
}

pub fn f64_at(b: &[u8], off: usize) -> Result<f64> {
    arr::<8>(b, off).map(f64::from_le_bytes)
}

/// Sub-slice `[off, off + len)`.
pub fn slice(b: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    off.checked_add(len)
        .and_then(|end| b.get(off..end))
        .ok_or_else(|| malformed(format!("range {off}+{len} out of bounds")))
}

/// UTF-16LE bytes to a string (lossy).
pub fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// Latin-1 bytes (the "compressed" form of UTF-16 used by [MS-XLS] strings).
pub fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// A sequential reader.
#[derive(Debug, Clone)]
pub struct Rd<'a> {
    pub b: &'a [u8],
    pub pos: usize,
}

impl<'a> Rd<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Rd { b, pos: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.b.len().saturating_sub(self.pos)
    }
    pub fn u8(&mut self) -> Result<u8> {
        let v = u8_at(self.b, self.pos)?;
        self.pos += 1;
        Ok(v)
    }
    pub fn u16(&mut self) -> Result<u16> {
        let v = u16_at(self.b, self.pos)?;
        self.pos += 2;
        Ok(v)
    }
    pub fn i16(&mut self) -> Result<i16> {
        let v = i16_at(self.b, self.pos)?;
        self.pos += 2;
        Ok(v)
    }
    pub fn u32(&mut self) -> Result<u32> {
        let v = u32_at(self.b, self.pos)?;
        self.pos += 4;
        Ok(v)
    }
    pub fn f64(&mut self) -> Result<f64> {
        let v = f64_at(self.b, self.pos)?;
        self.pos += 8;
        Ok(v)
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = slice(self.b, self.pos, n)?;
        self.pos += n;
        Ok(s)
    }
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.take(n).map(|_| ())
    }
}
