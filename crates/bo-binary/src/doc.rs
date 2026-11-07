//! Word 97–2003 binary documents ([MS-DOC]).
//!
//! Pipeline: FIB (§2.5) → table stream → CLX / piece table (§2.9.38, §2.9.177)
//! → character positions mapped to file offsets → CHPX / PAPX through the bin
//! tables and FKPs (§2.9.31, §2.9.33, §2.9.166, §2.9.167) → stylesheet
//! (§2.9.271 STSH) and font table (§2.9.272 SttbfFfn) → sections (§2.9.255
//! PlcfSed, §2.9.252 Sepx).

use crate::bytes::{i16_at, i32_at, slice, u16_at, u32_at, u8_at};
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
    let table_name = if fib.which_table {
        "/1Table"
    } else {
        "/0Table"
    };
    let table = if cfb.has(table_name) {
        cfb.read(table_name)?
    } else {
        return Err(BinaryError::Missing(table_name.into()));
    };
    let mut doc = Reader::new(&word, &table, &fib)?.document();
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
    pub const PLF_LST: usize = 73;
    pub const PLF_LFO: usize = 74;
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
    /// 1-based LFO index (0 = not in a list).
    ilfo: u16,
    ilvl: u8,
}

fn apply_pap(grpprl: &[u8], pap: &mut Pap) {
    for (sprm, op) in sprms(grpprl) {
        let b0 = op.first().copied().unwrap_or(0);
        let s16 = || i16_at(op, 0).ok().map(|v| Twips(i32::from(v)));
        let u16v = || u16_at(op, 0).ok().map(|v| Twips(i32::from(v)));
        match sprm {
            // sprmPIstd
            0x4600 => pap.istd = u16_at(op, 0).unwrap_or(0),
            // sprmPIlfo / sprmPIlvl (§2.6.2)
            0x460B => pap.ilfo = u16_at(op, 0).unwrap_or(0),
            0x260A => pap.ilvl = b0.min(8),
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
            ..Pap::default()
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
// Lists (§2.9.147 LSTF, §2.9.150 LVL, §2.9.151 LVLF, §2.9.131 LFO)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Lvl {
    start: i32,
    /// §2.9.158 MSONFC number format.
    nfc: u8,
    /// 0 = tab, 1 = space, 2 = nothing.
    follow: u8,
    papx: Vec<u8>,
    chpx: Vec<u8>,
    /// Number text template; code units < 9 are level placeholders.
    xst: Vec<u16>,
}

#[derive(Debug, Default)]
struct Lists {
    /// lsid → levels.
    by_lsid: HashMap<i32, Vec<Lvl>>,
    /// LFO lsids (index = ilfo - 1).
    lfo: Vec<i32>,
    /// Current counters per list and level.
    counters: HashMap<i32, [Option<i32>; 9]>,
}

/// Maximum number of lists read.
const MAX_LISTS: usize = 4096;

impl Lists {
    /// `plf_lst` is the table stream from fcPlfLst on (§2.9.191 PlfLst followed
    /// by the LVLs), `lcb_lst` the size of PlfLst itself.
    fn parse(plf_lst: &[u8], lcb_lst: usize, plf_lfo: &[u8]) -> Lists {
        let mut out = Lists::default();
        let Ok(c_lst) = i16_at(plf_lst, 0) else {
            return out;
        };
        let c_lst = usize::try_from(c_lst).unwrap_or(0).min(MAX_LISTS);
        let mut lvl_pos = lcb_lst.max(2 + 28 * c_lst);
        for i in 0..c_lst {
            let o = 2 + 28 * i;
            let (Ok(lsid), Ok(flags)) = (i32_at(plf_lst, o), u8_at(plf_lst, o + 26)) else {
                break;
            };
            // LSTF.fSimpleList: one level instead of nine
            let n = if flags & 1 != 0 { 1 } else { 9 };
            let mut levels = Vec::with_capacity(n);
            for _ in 0..n {
                match Self::parse_lvl(plf_lst, lvl_pos) {
                    Ok((lvl, used)) => {
                        levels.push(lvl);
                        lvl_pos += used;
                    }
                    Err(_) => break,
                }
            }
            out.by_lsid.insert(lsid, levels);
        }
        // §2.9.192 PlfLfo: lfoMac, then LFO (16 bytes, lsid first)
        if let Ok(mac) = i32_at(plf_lfo, 0) {
            for k in 0..usize::try_from(mac).unwrap_or(0).min(MAX_LISTS) {
                match i32_at(plf_lfo, 4 + 16 * k) {
                    Ok(lsid) => out.lfo.push(lsid),
                    Err(_) => break,
                }
            }
        }
        out
    }

    fn parse_lvl(d: &[u8], pos: usize) -> Result<(Lvl, usize)> {
        let lvlf = slice(d, pos, 28)?;
        let cb_chpx = lvlf[24] as usize;
        let cb_papx = lvlf[25] as usize;
        let mut p = pos + 28;
        let papx = slice(d, p, cb_papx)?.to_vec();
        p += cb_papx;
        let chpx = slice(d, p, cb_chpx)?.to_vec();
        p += cb_chpx;
        // §2.9.326 Xst
        let cch = u16_at(d, p)? as usize;
        let xst = slice(d, p + 2, cch * 2)?
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        p += 2 + cch * 2;
        Ok((
            Lvl {
                start: i32_at(lvlf, 0)?,
                nfc: lvlf[4],
                follow: lvlf[15],
                papx,
                chpx,
                xst,
            },
            p - pos,
        ))
    }

    fn level(&self, ilfo: u16, ilvl: u8) -> Option<(i32, &Lvl)> {
        let lsid = *self.lfo.get(usize::from(ilfo).checked_sub(1)?)?;
        let levels = self.by_lsid.get(&lsid)?;
        let lvl = levels.get(usize::from(ilvl)).or_else(|| levels.first())?;
        Some((lsid, lvl))
    }

    /// Advance the counter of (ilfo, ilvl) and produce (label text, follow).
    fn next_label(&mut self, ilfo: u16, ilvl: u8) -> Option<(String, u8)> {
        let (lsid, lvl) = self.level(ilfo, ilvl)?;
        let lvl = lvl.clone();
        let levels = self.by_lsid.get(&lsid)?;
        let starts: Vec<i32> = levels.iter().map(|l| l.start).collect();
        let nfcs: Vec<u8> = levels.iter().map(|l| l.nfc).collect();
        let ilvl = usize::from(ilvl).min(8);
        let ctr = self.counters.entry(lsid).or_insert([None; 9]);
        ctr[ilvl] = Some(ctr[ilvl].map_or(lvl.start, |v| v.saturating_add(1)));
        for deeper in ctr.iter_mut().skip(ilvl + 1) {
            *deeper = None;
        }
        if lvl.nfc == 255 && lvl.xst.is_empty() {
            return None;
        }
        let mut label = String::new();
        for &u in &lvl.xst {
            if u < 9 {
                let k = u as usize;
                let v = ctr[k].unwrap_or_else(|| starts.get(k).copied().unwrap_or(1));
                let nfc = if k == ilvl {
                    lvl.nfc
                } else {
                    nfcs.get(k).copied().unwrap_or(0)
                };
                label.push_str(&format_number(v, nfc));
            } else {
                label.push(bullet_char(u));
            }
        }
        Some((label, lvl.follow))
    }
}

/// Symbol-font private-use bullets → Unicode.
fn bullet_char(u: u16) -> char {
    match u {
        0xF0A7 => '\u{25AA}',
        0xF0D8 => '\u{27A2}',
        0xF076 => '\u{2756}',
        0xF0FC => '\u{2713}',
        0xF06F => 'o',
        // includes U+F0B7, the usual Symbol-font bullet
        0xF000..=0xF0FF => '\u{2022}',
        _ => char::from_u32(u32::from(u)).unwrap_or('\u{2022}'),
    }
}

fn roman(mut n: i32) -> String {
    const T: [(i32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    if !(1..=3999).contains(&n) {
        return n.to_string();
    }
    let mut s = String::new();
    for (v, r) in T {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

fn letters(n: i32) -> String {
    if n < 1 {
        return n.to_string();
    }
    // a..z, then aa, bb, … (repeated letter)
    let k = ((n - 1) % 26) as u8;
    let reps = ((n - 1) / 26 + 1).min(64) as usize;
    std::iter::repeat_n(char::from(b'a' + k), reps).collect()
}

/// §2.9.158 MSONFC (subset).
fn format_number(v: i32, nfc: u8) -> String {
    match nfc {
        1 => roman(v).to_uppercase(),
        2 => roman(v),
        3 => letters(v).to_uppercase(),
        4 => letters(v),
        5 => {
            let suffix = match (v % 10, v % 100) {
                (_, 11..=13) => "th",
                (1, _) => "st",
                (2, _) => "nd",
                (3, _) => "rd",
                _ => "th",
            };
            format!("{v}{suffix}")
        }
        22 => format!("{v:02}"),
        23 | 255 => String::new(),
        _ => v.to_string(),
    }
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

/// A materialised list label (bo-model has no numbering model yet).
#[derive(Debug, Clone)]
struct Label {
    text: String,
    props: RunProps,
    follow: u8,
}

struct Reader {
    /// (character, file offset) for CP 0..ccpText.
    chars: Vec<(u16, u32)>,
    chpx: Vec<PropRun>,
    papx: Vec<PropRun>,
    fonts: Vec<String>,
    styles: Stylesheet,
    sections: Vec<(u32, PageSetup)>,
    lists: Lists,
}

impl Reader {
    fn new(word: &[u8], table: &[u8], fib: &Fib) -> Result<Self> {
        let clx = fib.table_slice(table, fcl::CLX)?;
        let chars = Self::piece_text(word, clx, fib.ccp_text)?;
        let fonts = parse_fonts(fib.table_slice(table, fcl::STTBF_FFN).unwrap_or_default());
        let lists = match fib.fc_lcb.get(fcl::PLF_LST) {
            Some(&(fc, lcb)) if lcb > 0 => Lists::parse(
                table.get(fc as usize..).unwrap_or_default(),
                lcb as usize,
                fib.table_slice(table, fcl::PLF_LFO).unwrap_or_default(),
            ),
            _ => Lists::default(),
        };
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
            sections: sections(
                word,
                fib.table_slice(table, fcl::PLCF_SED).unwrap_or_default(),
            ),
            fonts,
            lists,
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

    /// Paragraph properties, the paragraph style's character properties and
    /// the list label if the paragraph is numbered.
    fn para_props(&mut self, fc: u32) -> (ParaProps, RunProps, Option<Label>) {
        let grpprl = find_run(&self.papx, fc)
            .map(|i| self.papx[i].grpprl.clone())
            .unwrap_or_default();
        let istd = u16_at(&grpprl, 0).unwrap_or(0);
        let direct = grpprl.get(2..).unwrap_or_default();
        let style_pap = self.styles.pap(istd, 0);
        let style_chp = self.styles.chp(istd, &self.fonts, 0);
        // List membership may come from the style or from direct formatting.
        let mut probe = Pap::default();
        if let Some(Some(raw)) = self.styles.styles.get(istd as usize) {
            if raw.stk == 1 {
                apply_pap(&raw.papx, &mut probe);
            }
        }
        apply_pap(direct, &mut probe);
        let mut pap = Pap {
            props: style_pap,
            istd,
            ..Pap::default()
        };
        let mut label = None;
        if probe.ilfo != 0 {
            if let Some((_, lvl)) = self.lists.level(probe.ilfo, probe.ilvl) {
                // §2.9.150 LVL.grpprlPapx sits between the style and direct formatting.
                let (lvl_papx, lvl_chpx) = (lvl.papx.clone(), lvl.chpx.clone());
                apply_pap(&lvl_papx, &mut pap);
                if let Some((text, follow)) = self.lists.next_label(probe.ilfo, probe.ilvl) {
                    let mut chp = Chp {
                        props: style_chp.clone(),
                        ..Chp::default()
                    };
                    apply_chp(&lvl_chpx, &mut chp, &style_chp, &self.fonts);
                    label = Some(Label {
                        text,
                        props: chp.props,
                        follow,
                    });
                }
            }
        }
        apply_pap(direct, &mut pap);
        (pap.props, style_chp, label)
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

    fn document(mut self) -> Document {
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
                let (pp, pchp, label) = self.para_props(chars[end].1);
                para_chp_cache = Some((end as u32, pp, pchp));
                if let Some(l) = label {
                    let chp = Chp {
                        props: l.props,
                        ..Chp::default()
                    };
                    if !l.text.is_empty() {
                        runs.push((chp.clone(), RunContent::Text(l.text)));
                    }
                    match l.follow {
                        0 => runs.push((chp, RunContent::Tab)),
                        1 => runs.push((chp, RunContent::Text(" ".into()))),
                        _ => {}
                    }
                }
            }
            let Some((_, para_props, para_chp)) = para_chp_cache.clone() else {
                break;
            };
            let in_code = fields.last().copied().unwrap_or(false);
            let is_section_end =
                sec_idx < section_ends.len() && cp + 1 == section_ends[sec_idx].0 && c == 0x0C;
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
        doc
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
            0x35, 0x08, 0x01, 0x43, 0x4A, 0x18, 0x00, 0x70, 0x68, 1, 2, 3, 0, 0x00, 0xC6, 2, 9, 9,
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

/// Builds minimal `.doc` files from scratch (no third-party samples).
#[cfg(test)]
pub(crate) mod build {
    fn put16(v: &mut [u8], off: usize, x: u16) {
        v[off..off + 2].copy_from_slice(&x.to_le_bytes());
    }
    fn put32(v: &mut [u8], off: usize, x: u32) {
        v[off..off + 4].copy_from_slice(&x.to_le_bytes());
    }

    /// PapxInFkp bytes for `istd` + `grpprl` (§2.9.175).
    fn papx_in_fkp(istd: u16, grpprl: &[u8]) -> Vec<u8> {
        let mut data = istd.to_le_bytes().to_vec();
        data.extend_from_slice(grpprl);
        if data.len() % 2 == 1 {
            let mut v = vec![data.len().div_ceil(2) as u8];
            v.extend(data);
            v
        } else {
            let mut v = vec![0, (data.len() / 2) as u8];
            v.extend(data);
            v
        }
    }

    fn std_entry(name: &str, stk: u16, base: u16, upx: &[Vec<u8>]) -> Vec<u8> {
        let mut s = vec![0u8; 10];
        put16(&mut s, 2, stk | (base << 4));
        put16(&mut s, 4, upx.len() as u16);
        s.extend((name.encode_utf16().count() as u16).to_le_bytes());
        for u in name.encode_utf16() {
            s.extend(u.to_le_bytes());
        }
        s.extend([0, 0]);
        for u in upx {
            s.extend((u.len() as u16).to_le_bytes());
            s.extend(u);
            if u.len() % 2 == 1 {
                s.push(0);
            }
        }
        let mut out = (s.len() as u16).to_le_bytes().to_vec();
        out.extend(s);
        out
    }

    /// Paragraph texts of [`sample_doc`] before conversion, for documentation.
    pub const TCVN3_TEXT: &str = "Việt Nam";

    /// A `.doc` with: a heading style (centred, bold), direct bold run,
    /// paragraph spacing, a PAGE field, a two-item numbered list, a TCVN3
    /// (.VnTime) run, a Unicode piece with direct font size, and A4 page setup.
    pub fn sample_doc() -> Vec<u8> {
        let tcvn = bo_encoding::unicode_to_tcvn3(TCVN3_TEXT);
        let mut piece0: Vec<u8> = Vec::new();
        piece0.extend(b"Heading\r"); // 0..8
        piece0.extend(b"Hello bold\r"); // 8..19
        piece0.extend(b"\x13 PAGE \x141\x15\r"); // 19..30
        piece0.extend(b"First\r"); // 30..36
        piece0.extend(b"Second\r"); // 36..43
        piece0.extend(&tcvn); // 43..
        piece0.push(b'\r');
        let n0 = piece0.len() as u32;
        let piece1: Vec<u16> = "Tiếng Việt\r".encode_utf16().collect();
        let n1 = piece1.len() as u32;
        let ccp = n0 + n1;

        // ---- WordDocument ----
        let mut w = vec![0u8; 3584];
        put16(&mut w, 0, 0xA5EC);
        put16(&mut w, 2, 0x00C1);
        put16(&mut w, 10, 0x0200); // fWhichTblStm
        put16(&mut w, 32, 14);
        put16(&mut w, 62, 22);
        put32(&mut w, 76, ccp);
        put16(&mut w, 152, 93);
        let fc0 = 1024u32;
        w[1024..1024 + piece0.len()].copy_from_slice(&piece0);
        let fc1 = 1400u32;
        for (k, u) in piece1.iter().enumerate() {
            put16(&mut w, 1400 + 2 * k, *u);
        }
        let end0 = fc0 + n0;
        let end1 = fc1 + 2 * n1;
        let tc = fc0 + 43; // start of the TCVN3 run

        // CHPX FKP at page 4
        let page = 2048;
        let rgfc = [fc0, fc0 + 14, fc0 + 18, tc, end0, fc1, end1];
        let crun = rgfc.len() - 1;
        for (i, fc) in rgfc.iter().enumerate() {
            put32(&mut w, page + 4 * i, *fc);
        }
        let grpprls: [&[u8]; 3] = [
            &[0x35, 0x08, 0x01],       // bold
            &[0x4F, 0x4A, 0x01, 0x00], // font 1 (.VnTime)
            &[0x43, 0x4A, 28, 0],      // 14 pt
        ];
        let mut at = 256;
        let mut offs = Vec::new();
        for g in grpprls {
            offs.push((at / 2) as u8);
            w[page + at] = g.len() as u8;
            w[page + at + 1..page + at + 1 + g.len()].copy_from_slice(g);
            at += (1 + g.len()).next_multiple_of(2);
        }
        let rgb = [0, offs[0], 0, offs[1], 0, offs[2]];
        for (i, b) in rgb.iter().enumerate() {
            w[page + 4 * (crun + 1) + i] = *b;
        }
        w[page + 511] = crun as u8;

        // PAPX FKP at page 5
        let page = 2560;
        let list = [0x0B, 0x46, 1, 0, 0x0A, 0x26, 0];
        let paras: [(u32, Option<Vec<u8>>); 8] = [
            (fc0, Some(papx_in_fkp(1, &[]))),
            (fc0 + 8, Some(papx_in_fkp(0, &[0x14, 0xA4, 240, 0]))),
            (fc0 + 19, Some(papx_in_fkp(0, &[]))),
            (fc0 + 30, Some(papx_in_fkp(0, &list))),
            (fc0 + 36, Some(papx_in_fkp(0, &list))),
            (fc0 + 43, Some(papx_in_fkp(0, &[]))),
            (end0, None),
            (fc1, Some(papx_in_fkp(0, &[0x5E, 0x84, 0xD0, 0x02]))),
        ];
        let cpara = paras.len();
        for (i, (fc, _)) in paras.iter().enumerate() {
            put32(&mut w, page + 4 * i, *fc);
        }
        put32(&mut w, page + 4 * cpara, end1);
        let mut at = 200;
        for (i, (_, papx)) in paras.iter().enumerate() {
            if let Some(p) = papx {
                w[page + 4 * (cpara + 1) + 13 * i] = (at / 2) as u8;
                w[page + at..page + at + p.len()].copy_from_slice(p);
                at += p.len() + p.len() % 2;
            }
        }
        w[page + 511] = cpara as u8;

        // Sepx at 3072: A4 portrait, margins 20/20/30/15 mm
        let sep: [(u16, u16); 6] = [
            (0xB01F, 11_906),
            (0xB020, 16_838),
            (0xB021, 1701),
            (0xB022, 850),
            (0x9023, 1134),
            (0x9024, 1134),
        ];
        put16(&mut w, 3072, (sep.len() * 4) as u16);
        for (i, (sprm, v)) in sep.iter().enumerate() {
            put16(&mut w, 3074 + 4 * i, *sprm);
            put16(&mut w, 3076 + 4 * i, *v);
        }

        // ---- 1Table ----
        let mut t: Vec<u8> = Vec::new();
        let mut fib_entries: Vec<(usize, u32, u32)> = Vec::new();
        let mut add = |t: &mut Vec<u8>, idx: usize, data: &[u8]| {
            fib_entries.push((idx, t.len() as u32, data.len() as u32));
            t.extend_from_slice(data);
        };
        // STSH
        let mut stsh = 18u16.to_le_bytes().to_vec();
        let mut stshi = vec![0u8; 18];
        put16(&mut stshi, 0, 2);
        put16(&mut stshi, 2, 10);
        stsh.extend(stshi);
        stsh.extend(std_entry(
            "Normal",
            1,
            0x0FFF,
            &[vec![0, 0], vec![0x43, 0x4A, 24, 0]],
        ));
        stsh.extend(std_entry(
            "Heading",
            1,
            0,
            &[vec![1, 0, 0x61, 0x24, 1], vec![0x35, 0x08, 0x01]],
        ));
        add(&mut t, super::fcl::STSHF, &stsh);
        // SttbfFfn
        let mut ffn = 2u16.to_le_bytes().to_vec();
        ffn.extend(0u16.to_le_bytes());
        for name in ["Times New Roman", ".VnTime"] {
            let mut f = vec![0u8; 39];
            for u in name.encode_utf16() {
                f.extend(u.to_le_bytes());
            }
            f.extend([0, 0]);
            ffn.push(f.len() as u8);
            ffn.extend(f);
        }
        add(&mut t, super::fcl::STTBF_FFN, &ffn);
        // PlcfSed
        let mut sed = 0u32.to_le_bytes().to_vec();
        sed.extend(ccp.to_le_bytes());
        sed.extend(0u16.to_le_bytes());
        sed.extend(3072u32.to_le_bytes());
        sed.extend(0u16.to_le_bytes());
        sed.extend(0xFFFF_FFFFu32.to_le_bytes());
        add(&mut t, super::fcl::PLCF_SED, &sed);
        // PlcBteChpx / PlcBtePapx
        let bte = |pn: u32| {
            let mut v = fc0.to_le_bytes().to_vec();
            v.extend(end1.to_le_bytes());
            v.extend(pn.to_le_bytes());
            v
        };
        add(&mut t, super::fcl::PLCF_BTE_CHPX, &bte(4));
        add(&mut t, super::fcl::PLCF_BTE_PAPX, &bte(5));
        // CLX with two pieces
        let mut plc = 0u32.to_le_bytes().to_vec();
        plc.extend(n0.to_le_bytes());
        plc.extend(ccp.to_le_bytes());
        for fc in [(fc0 * 2) | 0x4000_0000, fc1] {
            plc.extend(0u16.to_le_bytes());
            plc.extend(fc.to_le_bytes());
            plc.extend(0u16.to_le_bytes());
        }
        let mut clx = vec![0x02];
        clx.extend((plc.len() as u32).to_le_bytes());
        clx.extend(plc);
        add(&mut t, super::fcl::CLX, &clx);
        // PlfLst: one simple list (lsid 7), LVL "%1." decimal from 1, indent 720 / hanging 360
        let mut lst = 1i16.to_le_bytes().to_vec();
        let mut lstf = 7i32.to_le_bytes().to_vec();
        lstf.extend(0i32.to_le_bytes());
        for _ in 0..9 {
            lstf.extend(0x0FFFu16.to_le_bytes());
        }
        lstf.extend([1, 0]);
        lst.extend(lstf);
        let lcb_lst = lst.len();
        let mut lvl = 1i32.to_le_bytes().to_vec();
        lvl.extend([0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        lvl.extend(0i32.to_le_bytes());
        lvl.extend(0u32.to_le_bytes());
        let papx = [0x5E, 0x84, 0xD0, 0x02, 0x60, 0x84, 0x98, 0xFE];
        lvl.extend([0, papx.len() as u8, 0, 0]);
        lvl.extend(papx);
        lvl.extend(2u16.to_le_bytes());
        lvl.extend(0u16.to_le_bytes());
        lvl.extend(u16::from(b'.').to_le_bytes());
        let fc_lst = t.len() as u32;
        t.extend(&lst);
        t.extend(&lvl);
        fib_entries.push((super::fcl::PLF_LST, fc_lst, lcb_lst as u32));
        let mut lfo = 1i32.to_le_bytes().to_vec();
        lfo.extend(7i32.to_le_bytes());
        lfo.extend([0u8; 12]);
        lfo.extend(0xFFFF_FFFFu32.to_le_bytes());
        let fc_lfo = t.len() as u32;
        t.extend(&lfo);
        fib_entries.push((super::fcl::PLF_LFO, fc_lfo, lfo.len() as u32));

        for (idx, fc, lcb) in fib_entries {
            put32(&mut w, 154 + 8 * idx, fc);
            put32(&mut w, 154 + 8 * idx + 4, lcb);
        }
        crate::test_util::cfb_file(&[("/WordDocument", w), ("/1Table", t)], &[])
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;

    #[test]
    fn reads_built_doc() {
        let doc = read_doc(&build::sample_doc()).unwrap();
        let paras: Vec<&Paragraph> = doc.paragraphs().collect();
        let texts: Vec<String> = paras.iter().map(|p| p.plain_text()).collect();
        assert_eq!(
            texts,
            vec![
                "Heading",
                "Hello bold",
                "1",
                "1.\tFirst",
                "2.\tSecond",
                build::TCVN3_TEXT,
                "Tiếng Việt"
            ]
        );
        assert_eq!(doc.default_run.font.as_deref(), Some("Times New Roman"));
        // heading style: centred + bold, inherits 12 pt from Normal
        assert_eq!(paras[0].props.align, Some(Alignment::Center));
        assert_eq!(paras[0].runs[0].props.bold, Some(true));
        assert_eq!(paras[0].runs[0].props.size, Some(HalfPoints(24)));
        // direct bold run inside a plain paragraph
        assert_eq!(paras[1].runs.len(), 2);
        assert_eq!(paras[1].runs[1].props.bold, Some(true));
        assert_ne!(paras[1].runs[0].props.bold, Some(true));
        assert_eq!(paras[1].props.space_after, Some(Twips(240)));
        // list indentation from the LVL
        assert_eq!(paras[3].props.indent_left, Some(Twips(720)));
        assert_eq!(paras[3].props.indent_first_line, Some(Twips(-360)));
        // Unicode piece with direct size and indent
        assert_eq!(paras[6].runs[0].props.size, Some(HalfPoints(28)));
        assert_eq!(paras[6].props.indent_left, Some(Twips(720)));
        // A4 page setup from the SEPX
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].page, PageSetup::default());
    }

    #[test]
    fn legacy_conversion_can_be_disabled() {
        let doc = read_doc_with(
            &build::sample_doc(),
            &DocOptions {
                convert_legacy_vietnamese: false,
            },
        )
        .unwrap();
        let t = doc.paragraphs().nth(5).unwrap().plain_text();
        assert_ne!(t, build::TCVN3_TEXT);
    }

    #[test]
    fn number_formats() {
        assert_eq!(format_number(4, 1), "IV");
        assert_eq!(format_number(14, 2), "xiv");
        assert_eq!(format_number(28, 4), "bb");
        assert_eq!(format_number(3, 3), "C");
        assert_eq!(format_number(12, 5), "12th");
        assert_eq!(format_number(22, 5), "22nd");
        assert_eq!(format_number(7, 22), "07");
        assert_eq!(format_number(7, 23), "");
        assert_eq!(bullet_char(0xF0B7), '\u{2022}');
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
        #[test]
        fn mutated_doc_never_panics(pos in 0usize..8192, val in proptest::num::u8::ANY) {
            let bytes = build::sample_doc();
            let mut cfb = crate::Cfb::open(&bytes).unwrap();
            let mut w = cfb.read("/WordDocument").unwrap();
            let mut t = cfb.read("/1Table").unwrap();
            if pos < 4096 {
                let i = pos % w.len();
                w[i] = val;
            } else {
                let i = pos % t.len();
                t[i] = val;
            }
            if let Ok(fib) = Fib::parse(&w) {
                if let Ok(r) = Reader::new(&w, &t, &fib) {
                    let _ = r.document();
                }
            }
        }

        #[test]
        fn random_bytes_never_panic(data in proptest::collection::vec(proptest::num::u8::ANY, 0..2048)) {
            let _ = read_doc(&data);
            if let Ok(fib) = Fib::parse(&data) {
                if let Ok(r) = Reader::new(&data, &data, &fib) {
                    let _ = r.document();
                }
            }
        }
    }
}
