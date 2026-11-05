//! Word 97–2003 binary documents ([MS-DOC]).
//!
//! Pipeline: FIB (§2.5) → table stream → CLX / piece table (§2.9.38, §2.9.177)
//! → character positions mapped to file offsets → CHPX / PAPX through the bin
//! tables and FKPs (§2.9.31, §2.9.33, §2.9.166, §2.9.167) → stylesheet
//! (§2.9.271 STSH) and font table (§2.9.272 SttbfFfn) → sections (§2.9.255
//! PlcfSed, §2.9.252 Sepx).

use crate::bytes::{i16_at, slice, u16_at, u32_at, u8_at};
use crate::{malformed, BinaryError, Cfb, Result};
use bo_model::{
    Alignment, Block, Color, Document, HalfPoints, LineSpacing, PageSetup, ParaProps, Paragraph,
    Run, RunContent, RunProps, Section, Twips, Underline,
};
use std::collections::HashMap;

/// Options for [`read_doc_with`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocOptions {
    /// Convert runs in TCVN3 (`.Vn*`) / VNI (`VNI-*`) fonts to Unicode (prompt §2).
    pub convert_legacy_vietnamese: bool,
}

impl Default for DocOptions {
    fn default() -> Self {
        DocOptions {
            convert_legacy_vietnamese: true,
        }
    }
}

/// Maximum number of characters read from the main document.
const MAX_CHARS: u32 = 64 * 1024 * 1024;
/// Maximum depth of style inheritance.
const MAX_STYLE_DEPTH: usize = 16;
/// §2.9.255 istd value meaning "no base style".
const ISTD_NIL: u16 = 0x0FFF;

/// Read a `.doc` file with default options.
pub fn read_doc(bytes: &[u8]) -> Result<Document> {
    read_doc_with(bytes, &DocOptions::default())
}

/// Read a `.doc` file.
pub fn read_doc_with(bytes: &[u8], opts: &DocOptions) -> Result<Document> {
    let mut cfb = Cfb::open(bytes)?;
    let word = cfb.read("/WordDocument")?;
    let fib = Fib::parse(&word)?;
    let table_name = if fib.which_table { "/1Table" } else { "/0Table" };
    let table = if cfb.has(table_name) {
        cfb.read(table_name)?
    } else {
        return Err(BinaryError::Missing(table_name.into()));
    };
    let mut doc = Reader::new(&word, &table, &fib)?.document()?;
    if opts.convert_legacy_vietnamese {
        bo_encoding::convert_document(&mut doc);
    }
    Ok(doc)
}

// ---------------------------------------------------------------------------
// FIB (§2.5.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Fib {
    which_table: bool,
    ccp_text: u32,
    /// fibRgFcLcb97 pairs.
    fc_lcb: Vec<(u32, u32)>,
}

/// Indices into FibRgFcLcb97 (§2.5.6).
mod fcl {
    pub const STSHF: usize = 1;
    pub const PLCF_SED: usize = 6;
    pub const PLCF_BTE_CHPX: usize = 12;
    pub const PLCF_BTE_PAPX: usize = 13;
    pub const STTBF_FFN: usize = 15;
    pub const CLX: usize = 33;
}

impl Fib {
    fn parse(w: &[u8]) -> Result<Fib> {
        // §2.5.2 FibBase
        if u16_at(w, 0)? != 0xA5EC {
            return Err(malformed("WordDocument: bad wIdent"));
        }
        let flags = u16_at(w, 0x0A)?;
        if flags & 0x0100 != 0 {
            // fEncrypted
            return Err(BinaryError::Encrypted);
        }
        let which_table = flags & 0x0200 != 0;
        // §2.5.1: csw, fibRgW, cslw, fibRgLw, cbRgFcLcb, fibRgFcLcbBlob
        let csw = u16_at(w, 32)? as usize;
        let lw_off = 34 + csw * 2;
        let cslw = u16_at(w, lw_off)? as usize;
        let rglw = lw_off + 2;
        // §2.5.5 FibRgLw97: ccpText is the 4th 32-bit value.
        let ccp_text = u32_at(w, rglw + 12)?;
        let fc_off = rglw + cslw * 4;
        let cb = u16_at(w, fc_off)? as usize;
        let base = fc_off + 2;
        let mut fc_lcb = Vec::with_capacity(cb);
        for i in 0..cb.min(256) {
            let fc = u32_at(w, base + i * 8)?;
            let lcb = u32_at(w, base + i * 8 + 4)?;
            fc_lcb.push((fc, lcb));
        }
        Ok(Fib {
            which_table,
            ccp_text: ccp_text.min(MAX_CHARS),
            fc_lcb,
        })
    }

    /// Slice of the table stream for FibRgFcLcb entry `i` (empty if absent).
    fn table_slice<'t>(&self, table: &'t [u8], i: usize) -> Result<&'t [u8]> {
        match self.fc_lcb.get(i) {
            Some(&(fc, lcb)) if lcb > 0 => slice(table, fc as usize, lcb as usize),
            _ => Ok(&[]),
        }
    }
}

// ---------------------------------------------------------------------------
// Sprm handling (§2.6)
// ---------------------------------------------------------------------------

/// Iterate (sprm, operand) pairs of a grpprl.
fn sprms(grpprl: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 <= grpprl.len() {
        let sprm = u16::from_le_bytes([grpprl[i], grpprl[i + 1]]);
        i += 2;
        // §2.2.5.1 Sprm.spra → operand size
        let size = match sprm >> 13 {
            0 | 1 => 1,
            2 | 4 | 5 => 2,
            3 => 4,
            7 => 3,
            _ => {
                // spra 6: variable length
                if sprm == 0xD608 || sprm == 0xD606 {
                    // sprmTDefTable: 2-byte cb = remaining size + 1
                    let Ok(cb) = u16_at(grpprl, i) else { break };
                    2 + (cb as usize).saturating_sub(1)
                } else if sprm == 0xC615 {
                    // sprmPChgTabs: cb 255 means a complex layout; give up on the rest.
                    let Ok(cb) = u8_at(grpprl, i) else { break };
                    if cb == 255 {
                        break;
                    }
                    1 + cb as usize
                } else {
                    let Ok(cb) = u8_at(grpprl, i) else { break };
                    1 + cb as usize
                }
            }
        };
        let Some(op) = grpprl.get(i..i + size) else {
            break;
        };
        out.push((sprm, op));
        i += size;
    }
    out
}

/// §2.9.119 Ico → RGB.
fn ico_color(ico: u8) -> Option<Color> {
    let (r, g, b) = match ico {
        1 => (0, 0, 0),
        2 => (0, 0, 255),
        3 => (0, 255, 255),
        4 => (0, 255, 0),
        5 => (255, 0, 255),
        6 => (255, 0, 0),
        7 => (255, 255, 0),
        8 => (255, 255, 255),
        9 => (0, 0, 128),
        10 => (0, 128, 128),
        11 => (0, 128, 0),
        12 => (128, 0, 128),
        13 => (128, 0, 0),
        14 => (128, 128, 0),
        15 => (128, 128, 128),
        16 => (192, 192, 192),
        _ => return None,
    };
    Some(Color { r, g, b })
}

/// Character properties being accumulated (plus flags the model lacks).
#[derive(Debug, Clone, Default, PartialEq)]
struct Chp {
    props: RunProps,
    vanish: bool,
    istd: Option<u16>,
}

/// §2.6.1 toggle operand: 0 off, 1 on, 0x80 = style value, 0x81 = negated style value.
fn toggle(op: u8, style: Option<bool>) -> Option<bool> {
    match op {
        0 => Some(false),
        1 => Some(true),
        0x80 => style,
        0x81 => Some(!style.unwrap_or(false)),
        _ => None,
    }
}

fn apply_chp(grpprl: &[u8], chp: &mut Chp, style: &RunProps, fonts: &[String]) {
    for (sprm, op) in sprms(grpprl) {
        let b0 = op.first().copied().unwrap_or(0);
        let w = || u16_at(op, 0).ok();
        match sprm {
            // sprmCFBold, sprmCFItalic, sprmCFStrike, sprmCFDStrike
            0x0835 => chp.props.bold = toggle(b0, style.bold).or(chp.props.bold),
            0x0836 => chp.props.italic = toggle(b0, style.italic).or(chp.props.italic),
            0x0837 | 0x2A53 => chp.props.strike = toggle(b0, style.strike).or(chp.props.strike),
            // sprmCFVanish (hidden text)
            0x083C => chp.vanish = b0 & 1 != 0 && b0 != 0x80,
            // sprmCKul (§2.9.133 Kul)
            0x2A3E => {
                chp.props.underline = Some(match b0 {
                    0 => Underline::None,
                    3 => Underline::Double,
                    _ => Underline::Single,
                });
            }
            // sprmCHps
            0x4A43 => {
                if let Some(hps) = w().filter(|&h| h > 0) {
                    chp.props.size = Some(HalfPoints(u32::from(hps)));
                }
            }
            // sprmCRgFtc0 (ASCII font), sprmCFtcBi fallback not used
            0x4A4F => {
                if let Some(name) = w().and_then(|f| fonts.get(f as usize)) {
                    chp.props.font = Some(name.clone());
                }
            }
            // sprmCIco
            0x2A42 => chp.props.color = ico_color(b0).or(chp.props.color),
            // sprmCCv (§2.9.43 COLORREF; fAuto = 0xFF)
            0x6870 => {
                if op.len() >= 4 && op[3] != 0xFF {
                    chp.props.color = Some(Color {
                        r: op[0],
                        g: op[1],
                        b: op[2],
                    });
                }
            }
            // sprmCIstd (character style)
            0x4A30 => chp.istd = w(),
            _ => {}
        }
    }
}

/// Paragraph properties being accumulated.
#[derive(Debug, Clone, Default, PartialEq)]
struct Pap {
    props: ParaProps,
    istd: u16,
}

fn apply_pap(grpprl: &[u8], pap: &mut Pap) {
    for (sprm, op) in sprms(grpprl) {
        let b0 = op.first().copied().unwrap_or(0);
        let s16 = || i16_at(op, 0).ok().map(|v| Twips(i32::from(v)));
        let u16v = || u16_at(op, 0).ok().map(|v| Twips(i32::from(v)));
        match sprm {
            // sprmPIstd
            0x4600 => pap.istd = u16_at(op, 0).unwrap_or(0),
            // sprmPJc80, sprmPJc (§2.9.132 Jc)
            0x2403 | 0x2461 => {
                pap.props.align = Some(match b0 {
                    1 => Alignment::Center,
                    2 => Alignment::Right,
                    3 | 4 | 5 | 7 | 8 | 9 => Alignment::Justify,
                    _ => Alignment::Left,
                });
            }
            // sprmPDxaLeft80 / sprmPDxaLeft
            0x840F | 0x845E => pap.props.indent_left = s16().or(pap.props.indent_left),
            // sprmPDxaRight80 / sprmPDxaRight
            0x840E | 0x845D => pap.props.indent_right = s16().or(pap.props.indent_right),
            // sprmPDxaLeft180 / sprmPDxaLeft1
            0x8411 | 0x8460 => {
                pap.props.indent_first_line = s16().or(pap.props.indent_first_line);
            }
            // sprmPDyaBefore / sprmPDyaAfter
            0xA413 => pap.props.space_before = u16v().or(pap.props.space_before),
            0xA414 => pap.props.space_after = u16v().or(pap.props.space_after),
            // sprmPDyaLine (§2.9.150 LSPD)
            0x6412 => {
                if let (Ok(dya), Ok(mult)) = (i16_at(op, 0), i16_at(op, 2)) {
                    pap.props.line_spacing = Some(if mult != 0 {
                        LineSpacing::Auto(u32::from(dya.unsigned_abs()))
                    } else if dya < 0 {
                        LineSpacing::Exact(Twips(-i32::from(dya)))
                    } else {
                        LineSpacing::AtLeast(Twips(i32::from(dya)))
                    });
                }
            }
            // sprmPFPageBreakBefore
            0x2407 => pap.props.page_break_before = Some(b0 != 0),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Stylesheet (§2.9.271) and fonts (§2.9.272)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct StyleRaw {
    stk: u8,
    base: u16,
    papx: Vec<u8>,
    chpx: Vec<u8>,
}

#[derive(Debug, Default)]
struct Stylesheet {
    styles: Vec<Option<StyleRaw>>,
    default_font: Option<u16>,
    resolved_chp: HashMap<u16, RunProps>,
    resolved_pap: HashMap<u16, ParaProps>,
}

impl Stylesheet {
    fn parse(data: &[u8]) -> Stylesheet {
        Self::try_parse(data).unwrap_or_default()
    }

    fn try_parse(d: &[u8]) -> Result<Stylesheet> {
        let mut out = Stylesheet::default();
        if d.is_empty() {
            return Ok(out);
        }
        let cb_stshi = u16_at(d, 0)? as usize;
        let stshi = slice(d, 2, cb_stshi)?;
        let cstd = u16_at(stshi, 0)? as usize;
        let cb_base = u16_at(stshi, 2)? as usize;
        out.default_font = u16_at(stshi, 12).ok();
        let mut pos = 2 + cb_stshi;
        for _ in 0..cstd.min(4096) {
            let Ok(cb) = u16_at(d, pos) else { break };
            let cb = cb as usize;
            pos += 2;
            if cb == 0 {
                out.styles.push(None);
                continue;
            }
            let Ok(std) = slice(d, pos, cb) else { break };
            pos += cb;
            out.styles.push(Self::parse_std(std, cb_base).ok());
        }
        Ok(out)
    }

    /// §2.9.260 STD.
    fn parse_std(s: &[u8], cb_base: usize) -> Result<StyleRaw> {
        let w2 = u16_at(s, 2)?;
        let stk = (w2 & 0x000F) as u8;
        let base = w2 >> 4;
        let cupx = (u16_at(s, 4)? & 0x000F) as usize;
        // xstzName after StdfBase (+ StdfPost2000)
        let name_off = cb_base;
        let cch = u16_at(s, name_off)? as usize;
        let mut pos = name_off + 2 + cch * 2 + 2;
        let mut upx = Vec::new();
        for _ in 0..cupx {
            let cb = u16_at(s, pos)? as usize;
            pos += 2;
            upx.push(slice(s, pos, cb)?.to_vec());
            pos += cb + (cb & 1);
        }
        let mut raw = StyleRaw {
            stk,
            base,
            ..StyleRaw::default()
        };
        match stk {
            // paragraph style: UpxPapx (istd + grpprl), UpxChpx
            1 => {
                if let Some(p) = upx.first() {
                    raw.papx = p.get(2..).unwrap_or_default().to_vec();
                }
                if let Some(c) = upx.get(1) {
                    raw.chpx.clone_from(c);
                }
            }
            // character style: UpxChpx
            2 => {
                if let Some(c) = upx.first() {
                    raw.chpx.clone_from(c);
                }
            }
            _ => {}
        }
        Ok(raw)
    }

    fn chp(&mut self, istd: u16, fonts: &[String], depth: usize) -> RunProps {
        if let Some(r) = self.resolved_chp.get(&istd) {
            return r.clone();
        }
        let Some(Some(raw)) = self.styles.get(istd as usize).cloned() else {
            return RunProps::default();
        };
        let base = if raw.base != ISTD_NIL && raw.base != istd && depth < MAX_STYLE_DEPTH {
            self.chp(raw.base, fonts, depth + 1)
        } else {
            RunProps::default()
        };
        let mut chp = Chp {
            props: base.clone(),
            ..Chp::default()
        };
        apply_chp(&raw.chpx, &mut chp, &base, fonts);
        self.resolved_chp.insert(istd, chp.props.clone());
        chp.props
    }

    fn pap(&mut self, istd: u16, depth: usize) -> ParaProps {
        if let Some(r) = self.resolved_pap.get(&istd) {
            return *r;
        }
        let Some(Some(raw)) = self.styles.get(istd as usize).cloned() else {
            return ParaProps::default();
        };
        let base = if raw.base != ISTD_NIL && raw.base != istd && depth < MAX_STYLE_DEPTH {
            self.pap(raw.base, depth + 1)
        } else {
            ParaProps::default()
        };
        let mut pap = Pap {
            props: base,
            istd,
        };
        if raw.stk == 1 {
            apply_pap(&raw.papx, &mut pap);
        }
        self.resolved_pap.insert(istd, pap.props);
        pap.props
    }
}

/// §2.9.272 SttbfFfn: font names by index.
fn parse_fonts(d: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(first) = u16_at(d, 0) else {
        return out;
    };
    let (count, mut pos) = if first == 0xFFFF {
        (u16_at(d, 2).unwrap_or(0) as usize, 6)
    } else {
        (first as usize, 4)
    };
    for _ in 0..count.min(4096) {
        let Ok(cb) = u8_at(d, pos) else { break };
        let cb = cb as usize;
        let Ok(ffn) = slice(d, pos + 1, cb) else {
            break;
        };
        pos += 1 + cb;
        // §2.9.88 FFN: xszFfn starts at offset 39.
        let name_bytes = ffn.get(39..).unwrap_or_default();
        let units: Vec<u16> = name_bytes
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        out.push(String::from_utf16_lossy(&units));
    }
    out
}

// ---------------------------------------------------------------------------
// Property runs from FKPs
// ---------------------------------------------------------------------------

/// A run of file offsets `[start, end)` sharing one grpprl.
#[derive(Debug, Clone)]
struct PropRun {
    start: u32,
    end: u32,
    grpprl: Vec<u8>,
}

fn fkp_page(word: &[u8], pn: u32) -> Option<&[u8]> {
    let off = (pn as usize).checked_mul(512)?;
    word.get(off..off + 512)
}

/// §2.9.31 PlcBteChpx / §2.9.33 PlcBtePapx: (n+1) FCs then n PnFkp.
fn bte_pages(plc: &[u8]) -> Vec<u32> {
    if plc.len() < 4 {
        return Vec::new();
    }
    let n = (plc.len() - 4) / 8;
    (0..n)
        .filter_map(|i| u32_at(plc, 4 * (n + 1) + 4 * i).ok())
        .map(|pn| pn & 0x003F_FFFF)
        .collect()
}

/// §2.9.166 ChpxFkp.
fn chpx_runs(word: &[u8], plc: &[u8]) -> Vec<PropRun> {
    let mut out = Vec::new();
    for pn in bte_pages(plc) {
        let Some(page) = fkp_page(word, pn) else {
            continue;
        };
        let crun = page[511] as usize;
        if 4 * (crun + 1) + crun > 511 {
            continue;
        }
        for i in 0..crun {
            let (Ok(start), Ok(end)) = (u32_at(page, 4 * i), u32_at(page, 4 * (i + 1))) else {
                continue;
            };
            let b = page[4 * (crun + 1) + i] as usize * 2;
            let grpprl = if b == 0 {
                Vec::new()
            } else {
                let cb = page.get(b).copied().unwrap_or(0) as usize;
                page.get(b + 1..b + 1 + cb).unwrap_or_default().to_vec()
            };
            out.push(PropRun { start, end, grpprl });
        }
    }
    out.sort_by_key(|r| r.start);
    out
}

/// §2.9.167 PapxFkp; returned grpprl starts with the 2-byte istd.
fn papx_runs(word: &[u8], plc: &[u8]) -> Vec<PropRun> {
    let mut out = Vec::new();
    for pn in bte_pages(plc) {
        let Some(page) = fkp_page(word, pn) else {
            continue;
        };
        let cpara = page[511] as usize;
        if 4 * (cpara + 1) + 13 * cpara > 511 {
            continue;
        }
        for i in 0..cpara {
            let (Ok(start), Ok(end)) = (u32_at(page, 4 * i), u32_at(page, 4 * (i + 1))) else {
                continue;
            };
            let o = page[4 * (cpara + 1) + 13 * i] as usize * 2;
            let grpprl = if o == 0 {
                Vec::new()
            } else {
                // §2.9.175 PapxInFkp
                let cb = page.get(o).copied().unwrap_or(0) as usize;
                let (data, len) = if cb != 0 {
                    (o + 1, (2 * cb).saturating_sub(1))
                } else {
                    (o + 2, 2 * page.get(o + 1).copied().unwrap_or(0) as usize)
                };
                page.get(data..data + len).unwrap_or_default().to_vec()
            };
            out.push(PropRun { start, end, grpprl });
        }
    }
    out.sort_by_key(|r| r.start);
    out
}

fn find_run(runs: &[PropRun], fc: u32) -> Option<usize> {
    let idx = runs.partition_point(|r| r.start <= fc);
    let i = idx.checked_sub(1)?;
    (fc < runs[i].end).then_some(i)
}

// ---------------------------------------------------------------------------
// Sections (§2.9.255 PlcfSed, §2.8.? Sed, §2.9.252 Sepx)
// ---------------------------------------------------------------------------

fn sections(word: &[u8], plc: &[u8]) -> Vec<(u32, PageSetup)> {
    let mut out = Vec::new();
    if plc.len() < 4 {
        return out;
    }
    let n = (plc.len() - 4) / 16;
    for i in 0..n {
        let Ok(cp_end) = u32_at(plc, 4 * (i + 1)) else {
            break;
        };
        let sed = 4 * (n + 1) + 12 * i;
        let fc_sepx = u32_at(plc, sed + 2).unwrap_or(u32::MAX);
        // §2.6.4 SEP defaults (US Letter, 1"/1.25" margins).
        let mut page = PageSetup {
            width: Twips(12_240),
            height: Twips(15_840),
            margin_top: Twips(1440),
            margin_bottom: Twips(1440),
            margin_left: Twips(1800),
            margin_right: Twips(1800),
        };
        if fc_sepx != u32::MAX {
            if let Ok(cb) = i16_at(word, fc_sepx as usize) {
                if let Ok(grpprl) = slice(word, fc_sepx as usize + 2, cb.max(0) as usize) {
                    for (sprm, op) in sprms(grpprl) {
                        let v = u16_at(op, 0).map(|v| Twips(i32::from(v)));
                        let s = i16_at(op, 0).map(|v| Twips(i32::from(v).abs()));
                        match sprm {
                            0xB01F => page.width = v.unwrap_or(page.width),
                            0xB020 => page.height = v.unwrap_or(page.height),
                            0xB021 => page.margin_left = v.unwrap_or(page.margin_left),
                            0xB022 => page.margin_right = v.unwrap_or(page.margin_right),
                            0x9023 => page.margin_top = s.unwrap_or(page.margin_top),
                            0x9024 => page.margin_bottom = s.unwrap_or(page.margin_bottom),
                            _ => {}
                        }
                    }
                }
            }
        }
        out.push((cp_end, page));
    }
    out
}

// ---------------------------------------------------------------------------
// Text assembly
// ---------------------------------------------------------------------------

struct Reader {
    /// (character, file offset) for CP 0..ccpText.
    chars: Vec<(u16, u32)>,
    chpx: Vec<PropRun>,
    papx: Vec<PropRun>,
    fonts: Vec<String>,
    styles: Stylesheet,
    sections: Vec<(u32, PageSetup)>,
}

impl Reader {
    fn new(word: &[u8], table: &[u8], fib: &Fib) -> Result<Self> {
        let clx = fib.table_slice(table, fcl::CLX)?;
        let chars = Self::piece_text(word, clx, fib.ccp_text)?;
        let fonts = parse_fonts(fib.table_slice(table, fcl::STTBF_FFN).unwrap_or_default());
        Ok(Reader {
            chars,
            chpx: chpx_runs(
                word,
                fib.table_slice(table, fcl::PLCF_BTE_CHPX)
                    .unwrap_or_default(),
            ),
            papx: papx_runs(
                word,
                fib.table_slice(table, fcl::PLCF_BTE_PAPX)
                    .unwrap_or_default(),
            ),
            styles: Stylesheet::parse(fib.table_slice(table, fcl::STSHF).unwrap_or_default()),
            sections: sections(word, fib.table_slice(table, fcl::PLCF_SED).unwrap_or_default()),
            fonts,
        })
    }

    /// §2.9.38 Clx → §2.9.177 PlcPcd → characters with their file offsets.
    fn piece_text(word: &[u8], clx: &[u8], ccp: u32) -> Result<Vec<(u16, u32)>> {
        let mut pos = 0;
        let plc = loop {
            match clx.get(pos) {
                // Prc: clxt 0x01, cbGrpprl, grpprl
                Some(1) => {
                    let cb = i16_at(clx, pos + 1)?.max(0) as usize;
                    pos += 3 + cb;
                }
                // Pcdt: clxt 0x02, lcb, PlcPcd
                Some(2) => {
                    let lcb = u32_at(clx, pos + 1)? as usize;
                    break slice(clx, pos + 5, lcb)?;
                }
                _ => return Err(malformed("CLX without piece table")),
            }
        };
        if plc.len() < 4 {
            return Err(malformed("empty piece table"));
        }
        let n = (plc.len() - 4) / 12;
        let mut chars = Vec::with_capacity(ccp.min(1 << 20) as usize);
        for i in 0..n {
            let cp_start = u32_at(plc, 4 * i)?;
            let cp_end = u32_at(plc, 4 * (i + 1))?;
            if cp_start >= ccp {
                break;
            }
            let cp_end = cp_end.min(ccp);
            if cp_end <= cp_start {
                continue;
            }
            // §2.9.178 Pcd: fc is FcCompressed at offset 2.
            let fc_raw = u32_at(plc, 4 * (n + 1) + 8 * i + 2)?;
            let compressed = fc_raw & 0x4000_0000 != 0;
            let fc = fc_raw & 0x3FFF_FFFF;
            let count = (cp_end - cp_start) as usize;
            if compressed {
                let start = (fc / 2) as usize;
                let bytes = slice(word, start, count)?;
                for (k, &b) in bytes.iter().enumerate() {
                    // §2.4.1: compressed text is Windows-1252.
                    let c = bo_encoding::cp1252::decode(b) as u32 as u16;
                    chars.push((c, (start + k) as u32));
                }
            } else {
                let start = fc as usize;
                let bytes = slice(word, start, count * 2)?;
                for k in 0..count {
                    let c = u16::from_le_bytes([bytes[2 * k], bytes[2 * k + 1]]);
                    chars.push((c, (start + 2 * k) as u32));
                }
            }
        }
        Ok(chars)
    }

    fn para_props(&mut self, fc: u32) -> (ParaProps, RunProps) {
        let grpprl = find_run(&self.papx, fc)
            .map(|i| self.papx[i].grpprl.clone())
            .unwrap_or_default();
        let istd = u16_at(&grpprl, 0).unwrap_or(0);
        let style_pap = self.styles.pap(istd, 0);
        let style_chp = self.styles.chp(istd, &self.fonts, 0);
        let mut pap = Pap {
            props: style_pap,
            istd,
        };
        apply_pap(grpprl.get(2..).unwrap_or_default(), &mut pap);
        (pap.props, style_chp)
    }

    fn run_props(&mut self, fc: u32, para_chp: &RunProps) -> Chp {
        let grpprl = find_run(&self.chpx, fc)
            .map(|i| self.chpx[i].grpprl.clone())
            .unwrap_or_default();
        // A character style (sprmCIstd) sits between the paragraph style and direct formatting.
        let mut probe = Chp::default();
        apply_chp(&grpprl, &mut probe, para_chp, &self.fonts);
        let base = match probe.istd {
            Some(istd) => self.styles.chp(istd, &self.fonts, 0).inherit(para_chp),
            None => para_chp.clone(),
        };
        let mut chp = Chp {
            props: base.clone(),
            ..Chp::default()
        };
        apply_chp(&grpprl, &mut chp, &base, &self.fonts);
        chp
    }

    fn document(mut self) -> Result<Document> {
        let mut doc = Document {
            default_run: RunProps {
                font: self
                    .styles
                    .default_font
                    .and_then(|f| self.fonts.get(f as usize).cloned()),
                // §2.6.1 CHP default hps = 20 (10 pt)
                size: Some(HalfPoints(20)),
                ..RunProps::default()
            },
            ..Document::default()
        };
        let mut section_ends: Vec<(u32, PageSetup)> = std::mem::take(&mut self.sections);
        if section_ends.is_empty() {
            section_ends.push((u32::MAX, PageSetup::default()));
        }
        let mut sec_idx = 0;
        let mut cur_section = Section {
            page: section_ends[0].1,
            blocks: Vec::new(),
        };
        let mut runs: Vec<(Chp, RunContent)> = Vec::new();
        // Field nesting: true while inside a field code (before the separator).
        let mut fields: Vec<bool> = Vec::new();
        let chars = std::mem::take(&mut self.chars);
        let mut para_chp_cache: Option<(u32, ParaProps, RunProps)> = None;

        let mut i = 0;
        while i < chars.len() {
            let (c, fc) = chars[i];
            let cp = i as u32;
            // Paragraph properties are attached to the paragraph mark, so find
            // the end of the current paragraph lazily.
            if para_chp_cache.is_none() {
                let end = chars[i..]
                    .iter()
                    .position(|&(c, _)| matches!(c, 0x0D | 0x07 | 0x0C))
                    .map_or(chars.len() - 1, |p| i + p);
                let (pp, pchp) = self.para_props(chars[end].1);
                para_chp_cache = Some((end as u32, pp, pchp));
            }
            let Some((_, para_props, para_chp)) = para_chp_cache.clone() else {
                break;
            };
            let in_code = fields.last().copied().unwrap_or(false);
            let is_section_end = sec_idx < section_ends.len()
                && cp + 1 == section_ends[sec_idx].0
                && c == 0x0C;
            match c {
                // §2.8.25 field begin / separator / end
                0x13 => fields.push(true),
                0x14 => {
                    if let Some(f) = fields.last_mut() {
                        *f = false;
                    }
                }
                0x15 => {
                    fields.pop();
                }
                _ if in_code => {}
                0x0D | 0x07 => {
                    cur_section
                        .blocks
                        .push(Block::Paragraph(make_paragraph(para_props, &runs)));
                    runs.clear();
                    para_chp_cache = None;
                }
                0x0C if is_section_end => {
                    cur_section
                        .blocks
                        .push(Block::Paragraph(make_paragraph(para_props, &runs)));
                    runs.clear();
                    para_chp_cache = None;
                    doc.sections.push(std::mem::take(&mut cur_section));
                    sec_idx += 1;
                    cur_section.page = section_ends
                        .get(sec_idx)
                        .map_or_else(PageSetup::default, |s| s.1);
                }
                _ => {
                    let chp = self.run_props(fc, &para_chp);
                    if !chp.vanish {
                        let item = match c {
                            0x09 => Some(RunContent::Tab),
                            0x0B => Some(RunContent::LineBreak),
                            0x0C => Some(RunContent::PageBreak),
                            // non-breaking hyphen
                            0x1E => Some(RunContent::Text("\u{2011}".into())),
                            // optional hyphen, objects, footnote refs, annotations
                            0x00..=0x1F => None,
                            0xD800..=0xDBFF => {
                                // UTF-16 surrogate pair spanning two CPs.
                                let lo = chars.get(i + 1).map_or(0, |n| n.0);
                                if (0xDC00..=0xDFFF).contains(&lo) {
                                    i += 1;
                                    let cp = 0x10000
                                        + ((u32::from(c) - 0xD800) << 10)
                                        + (u32::from(lo) - 0xDC00);
                                    char::from_u32(cp).map(|ch| RunContent::Text(ch.to_string()))
                                } else {
                                    Some(RunContent::Text("\u{FFFD}".into()))
                                }
                            }
                            _ => Some(RunContent::Text(
                                char::from_u32(u32::from(c))
                                    .unwrap_or('\u{FFFD}')
                                    .to_string(),
                            )),
                        };
                        if let Some(item) = item {
                            runs.push((chp, item));
                        }
                    }
                }
            }
            i += 1;
        }
        if !runs.is_empty() {
            let props = para_chp_cache.map(|p| p.1).unwrap_or_default();
            cur_section
                .blocks
                .push(Block::Paragraph(make_paragraph(props, &runs)));
        }
        if !cur_section.blocks.is_empty() || doc.sections.is_empty() {
            doc.sections.push(cur_section);
        }
        Ok(doc)
    }
}

/// Merge consecutive items with equal properties into runs.
fn make_paragraph(props: ParaProps, items: &[(Chp, RunContent)]) -> Paragraph {
    let mut runs: Vec<Run> = Vec::new();
    for (chp, item) in items {
        match runs.last_mut() {
            Some(last) if last.props == chp.props => match (last.content.last_mut(), item) {
                (Some(RunContent::Text(prev)), RunContent::Text(t)) => prev.push_str(t),
                _ => last.content.push(item.clone()),
            },
            _ => runs.push(Run {
                props: chp.props.clone(),
                content: vec![item.clone()],
            }),
        }
    }
    Paragraph { props, runs }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprm_sizes() {
        // bold(1 byte) + hps(2 bytes) + sprmCCv (4 bytes) + variable (cb=2)
        let g = [
            0x35, 0x08, 0x01, 0x43, 0x4A, 0x18, 0x00, 0x70, 0x68, 1, 2, 3, 0, 0x00, 0xC6, 2, 9,
            9,
        ];
        let s = sprms(&g);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0], (0x0835, &[1u8][..]));
        assert_eq!(s[1], (0x4A43, &[0x18, 0][..]));
        assert_eq!(s[2].1, &[1, 2, 3, 0]);
        assert_eq!(s[3], (0xC600, &[2, 9, 9][..]));
        // truncated operand is dropped, not a panic
        assert_eq!(sprms(&[0x43, 0x4A, 0x18]).len(), 0);
    }

    #[test]
    fn toggles() {
        assert_eq!(toggle(0x81, Some(true)), Some(false));
        assert_eq!(toggle(0x80, Some(true)), Some(true));
        assert_eq!(toggle(1, None), Some(true));
    }
}
