//! Review › Speech › Read Aloud of BlueDoc (round 16): reads the document from the caret with
//! the platform's speech voice (the host speaks; Windows: SAPI `ISpVoice`), paragraph after
//! paragraph, selecting the word being spoken, with the reference's small bar at the top
//! right of the document — Previous paragraph, Play / Pause, Next paragraph, Settings
//! (reading speed) and Close (own drawings).
use crate::Painter;
use bo_render::{PathSeg, Rgba};

/// A Read Aloud bar button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RaButton {
    Prev,
    Play,
    Next,
    Settings,
    Close,
}

const BUTTONS: [RaButton; 5] = [
    RaButton::Prev,
    RaButton::Play,
    RaButton::Next,
    RaButton::Settings,
    RaButton::Close,
];

/// Reading speeds of Settings: (label key, English, voice rate −10…10).
pub(crate) const SPEEDS: [(&str, &str, i32); 5] = [
    ("doc16.speedSlowest", "Slowest", -6),
    ("doc16.speedSlow", "Slow", -3),
    ("doc16.speedNormal", "Normal", 0),
    ("doc16.speedFast", "Fast", 3),
    ("doc16.speedFastest", "Fastest", 6),
];

/// Read Aloud in progress.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReadAloud {
    /// Body paragraph being read and the byte offset its spoken text starts at.
    pub(crate) para: usize,
    pub(crate) from: usize,
    /// The text handed to the voice.
    pub(crate) text: String,
    pub(crate) paused: bool,
    /// Voice rate (−10…10).
    pub(crate) rate: i32,
    /// Ticks since the text was handed over without the voice speaking yet.
    pub(crate) waiting: u32,
    /// The voice was seen speaking this text.
    pub(crate) seen: bool,
    /// The word last selected (byte range in the paragraph).
    pub(crate) word: Option<(usize, usize)>,
    rect: [f32; 4],
    scale: f32,
    pub(crate) hover: Option<RaButton>,
}

/// Byte offset in `s` of UTF-16 offset `u` (the voice reports UTF-16 positions).
pub(crate) fn byte_of_utf16(s: &str, u: usize) -> usize {
    let mut n = 0;
    for (b, c) in s.char_indices() {
        if n >= u {
            return b;
        }
        n += c.len_utf16();
    }
    s.len()
}

impl ReadAloud {
    /// Place the bar at the top right of the document frame `f` (x, y, w, h).
    pub(crate) fn layout(&mut self, f: [f32; 4], scale: f32) {
        self.scale = scale;
        let b = (32.0 * scale).round();
        let w = b * 5.0 + 2.0;
        self.rect = [
            (f[0] + f[2] - w - 24.0 * scale).round(),
            (f[1] + 8.0 * scale).round(),
            w,
            b + 2.0,
        ];
    }

    fn button_rect(&self, i: usize) -> [f32; 4] {
        let b = (32.0 * self.scale).round();
        [self.rect[0] + 1.0 + b * i as f32, self.rect[1] + 1.0, b, b]
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect;
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    /// The button at (`x`, `y`).
    pub(crate) fn hit(&self, x: f32, y: f32) -> Option<RaButton> {
        (0..5).find_map(|i| {
            let r = self.button_rect(i);
            (x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]).then_some(BUTTONS[i])
        })
    }

    /// Window rectangle of the Settings button (its menu opens under it).
    pub(crate) fn settings_rect(&self) -> [f32; 4] {
        self.button_rect(3)
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.rect;
        let ink = Rgba(0x26, 0x26, 0x26, 255);
        p.fill_rect(x - 1.0, y - 1.0, w + 2.0, h + 2.0, Rgba(0, 0, 0, 24));
        p.fill_rect(x, y, w, h, Rgba(0xC6, 0xC6, 0xC6, 255));
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, Rgba(255, 255, 255, 255));
        let k = self.scale;
        let stroke = (1.2 * k).max(1.0);
        for (i, b) in BUTTONS.iter().enumerate() {
            let r = self.button_rect(i);
            if self.hover == Some(*b) {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xE5, 0xE5, 0xE5, 255));
            }
            let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            let s = 6.0 * k;
            match b {
                RaButton::Prev | RaButton::Next => {
                    // A bar and a triangle pointing away from it.
                    let d = if *b == RaButton::Next { 1.0 } else { -1.0 };
                    p.path(
                        &[
                            PathSeg::M(cx - d * s * 0.6, cy - s),
                            PathSeg::L(cx + d * s * 0.6, cy),
                            PathSeg::L(cx - d * s * 0.6, cy + s),
                            PathSeg::Z,
                        ],
                        Some(ink),
                        None,
                    );
                    p.fill_rect(
                        (cx + d * s * 0.6 - if d > 0.0 { 0.0 } else { 1.6 * k }).round(),
                        cy - s,
                        (1.6 * k).max(1.0),
                        2.0 * s,
                        ink,
                    );
                }
                RaButton::Play => {
                    if self.paused {
                        p.path(
                            &[
                                PathSeg::M(cx - s * 0.6, cy - s),
                                PathSeg::L(cx + s * 0.8, cy),
                                PathSeg::L(cx - s * 0.6, cy + s),
                                PathSeg::Z,
                            ],
                            Some(ink),
                            None,
                        );
                    } else {
                        let bw = 2.4 * k;
                        p.fill_rect(cx - s * 0.55, cy - s, bw, 2.0 * s, ink);
                        p.fill_rect(cx + s * 0.55 - bw, cy - s, bw, 2.0 * s, ink);
                    }
                }
                RaButton::Settings => {
                    // A sun-like wheel: a ring and eight short spokes.
                    let n = 16;
                    let ring: Vec<PathSeg> = (0..=n)
                        .map(|j| {
                            let t = std::f32::consts::TAU * j as f32 / n as f32;
                            let (px, py) = (cx + s * 0.55 * t.cos(), cy + s * 0.55 * t.sin());
                            if j == 0 {
                                PathSeg::M(px, py)
                            } else {
                                PathSeg::L(px, py)
                            }
                        })
                        .collect();
                    p.path(&ring, None, Some((ink, stroke)));
                    let mut spokes = Vec::new();
                    for j in 0..8 {
                        let t = std::f32::consts::TAU * j as f32 / 8.0;
                        spokes.push(PathSeg::M(cx + s * 0.75 * t.cos(), cy + s * 0.75 * t.sin()));
                        spokes.push(PathSeg::L(cx + s * t.cos(), cy + s * t.sin()));
                    }
                    p.path(&spokes, None, Some((ink, stroke * 1.4)));
                }
                RaButton::Close => {
                    let c = s * 0.75;
                    p.path(
                        &[
                            PathSeg::M(cx - c, cy - c),
                            PathSeg::L(cx + c, cy + c),
                            PathSeg::M(cx + c, cy - c),
                            PathSeg::L(cx - c, cy + c),
                        ],
                        None,
                        Some((ink, stroke)),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn utf16_offsets_map_to_bytes() {
        let s = "Tiếng Việt 😀 ok";
        assert_eq!(super::byte_of_utf16(s, 0), 0);
        assert_eq!(&s[super::byte_of_utf16(s, 6)..], "Việt 😀 ok");
        // The emoji is two UTF-16 units.
        assert_eq!(&s[super::byte_of_utf16(s, 13)..], " ok");
    }
}
