//! Spec units.
//!
//! ECMA-376 Part 1, §22.9.2.14 `ST_TwipsMeasure` — 1/20 of a point.
//! ECMA-376 Part 1, §17.18.40 `ST_HpsMeasure` — half-points.
//! ECMA-376 Part 1, §20.1.10.16 `ST_Coordinate` — English Metric Units (914 400 per inch).

use serde::{Deserialize, Serialize};

/// Twentieths of a point.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Twips(pub i32);

/// Half-points (font sizes in WordprocessingML).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct HalfPoints(pub u32);

/// English Metric Units.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Emu(pub i64);

impl Twips {
    /// Value in points.
    pub fn to_pt(self) -> f32 {
        self.0 as f32 / 20.0
    }
    /// Construct from points (rounded to the nearest twip).
    pub fn from_pt(pt: f32) -> Self {
        Twips((pt * 20.0).round() as i32)
    }
}

impl HalfPoints {
    /// Value in points.
    pub fn to_pt(self) -> f32 {
        self.0 as f32 / 2.0
    }
}

impl Emu {
    /// EMU per point (12 700).
    pub const PER_PT: i64 = 12_700;
    /// Value in points.
    pub fn to_pt(self) -> f32 {
        self.0 as f32 / Self::PER_PT as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(Twips(1440).to_pt(), 72.0);
        assert_eq!(Twips::from_pt(72.0), Twips(1440));
        assert_eq!(HalfPoints(24).to_pt(), 12.0);
        assert_eq!(Emu(914_400).to_pt(), 72.0);
    }
}
