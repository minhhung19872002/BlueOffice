//! Opening `.ppt` ([MS-PPT] via `bo-binary`) and `.odp` (ODF 1.3 via `bo-odf`)
//! decks in BlueSlide: their shapes, pictures, text and notes become a
//! PresentationML package (`bo_pptx::build`), which is then opened like any
//! `.pptx` and saved as `.pptx`.

use bo_pptx::build::{
    DeckSpec, LayoutKind, LineSpec, Paint, ParaSpec, Picture, Placeholder, RunSpec, ShapeSpec,
    SlideSpec,
};

fn rgb(c: u32) -> String {
    format!("{:06X}", c & 0x00FF_FFFF)
}

fn layout_for(shapes: &[ShapeSpec]) -> LayoutKind {
    if shapes.iter().any(|s| {
        matches!(
            s.placeholder,
            Some(Placeholder::CenterTitle | Placeholder::SubTitle)
        )
    }) {
        LayoutKind::Title
    } else if shapes.iter().any(|s| s.placeholder.is_some()) {
        LayoutKind::TitleContent
    } else {
        LayoutKind::Blank
    }
}

/// `.ppt` deck → package spec.
pub(crate) fn ppt_deck(deck: &bo_binary::PptDeck) -> DeckSpec {
    use bo_binary::{PptPlaceholder, TextKind};
    let mut out = DeckSpec {
        width: deck.slide_width,
        height: deck.slide_height,
        slides: Vec::new(),
        title: None,
    };
    for slide in &deck.slides {
        let mut shapes = Vec::new();
        for (i, s) in slide.shapes.iter().enumerate() {
            let placeholder = s.placeholder.and_then(|p| match p {
                PptPlaceholder::Title => Some(Placeholder::Title),
                PptPlaceholder::CenterTitle => Some(Placeholder::CenterTitle),
                PptPlaceholder::SubTitle => Some(Placeholder::SubTitle),
                PptPlaceholder::Body => Some(Placeholder::Body),
                PptPlaceholder::Footer | PptPlaceholder::Other => None,
            });
            let text: Vec<ParaSpec> = s
                .text
                .as_ref()
                .map(|t| {
                    t.paragraphs
                        .iter()
                        .map(|p| ParaSpec {
                            runs: p
                                .runs
                                .iter()
                                .map(|r| RunSpec {
                                    text: r.text.clone(),
                                    bold: r.bold,
                                    italic: r.italic,
                                    underline: r.underline,
                                    size: r.size.map(|v| u32::from(v) * 100),
                                    color: r.color.map(rgb),
                                    font: r.font.clone(),
                                })
                                .collect(),
                            level: p.level,
                            align: p.align.map(str::to_owned),
                            bullet: if placeholder.is_some() {
                                None
                            } else {
                                p.bullet
                            },
                        })
                        .collect()
                })
                .unwrap_or_default();
            let kind_title = s
                .text
                .as_ref()
                .is_some_and(|t| matches!(t.kind, TextKind::Title | TextKind::CenterTitle));
            let picture = s
                .picture
                .and_then(|i| deck.pictures.get(i))
                .map(|p| Picture {
                    data: p.data.clone(),
                    ext: p.ext.to_owned(),
                });
            shapes.push(ShapeSpec {
                name: format!("Shape {}", i + 1),
                x: s.x,
                y: s.y,
                cx: s.cx,
                cy: s.cy,
                rot: s.rotation,
                flip_h: s.flip_h,
                flip_v: s.flip_v,
                geometry: s.geometry.to_owned(),
                fill: if s.missing_picture {
                    Some(Paint::Solid("D9D9D9".into()))
                } else {
                    s.fill
                        .map(|c| Paint::Solid(rgb(c)))
                        .or(if placeholder.is_some() {
                            None
                        } else {
                            Some(Paint::None)
                        })
                },
                line: match s.line {
                    Some((c, w)) => Some(LineSpec {
                        color: Some(rgb(c)),
                        width: w,
                    }),
                    None if placeholder.is_none() => Some(LineSpec {
                        color: None,
                        width: 0,
                    }),
                    None => None,
                },
                placeholder: placeholder.or(kind_title.then_some(Placeholder::Title)),
                text,
                anchor: s.anchor.map(str::to_owned),
                picture,
            });
        }
        // A slide with text but no drawing (old files, fallback reads): text boxes.
        if shapes.is_empty() {
            let h = out.height.max(1);
            for (i, t) in slide.texts.iter().enumerate() {
                let title = t.kind.is_title();
                let y = if title {
                    h / 20
                } else {
                    h / 4 + i64::try_from(i).unwrap_or(0) * h / 8
                };
                shapes.push(ShapeSpec {
                    name: format!("Text {}", i + 1),
                    x: out.width / 24,
                    y: y.min(h * 7 / 8),
                    cx: out.width * 11 / 12,
                    cy: if title { h / 6 } else { h / 2 },
                    placeholder: Some(if title {
                        Placeholder::Title
                    } else {
                        Placeholder::Body
                    }),
                    text: t.paragraphs.iter().map(|p| ParaSpec::plain(p)).collect(),
                    ..ShapeSpec::default()
                });
            }
        }
        out.slides.push(SlideSpec {
            layout: layout_for(&shapes),
            shapes,
            notes: slide.notes.clone(),
            background: None,
        });
    }
    out
}

/// `.odp` deck → package spec.
pub(crate) fn odp_deck(deck: &bo_odf::OdpDeck) -> DeckSpec {
    let mut out = DeckSpec {
        width: deck.width,
        height: deck.height,
        slides: Vec::new(),
        title: None,
    };
    for slide in &deck.slides {
        let mut shapes = Vec::new();
        for (i, s) in slide.shapes.iter().enumerate() {
            // §19.392 presentation:class
            let placeholder = match s.class.as_deref() {
                Some("title") => Some(Placeholder::Title),
                Some("subtitle") => Some(Placeholder::SubTitle),
                Some("outline" | "text") => Some(Placeholder::Body),
                _ => None,
            };
            let paint = |p: &bo_odf::Paint| match p {
                bo_odf::Paint::Inherit => None,
                bo_odf::Paint::None => Some(Paint::None),
                bo_odf::Paint::Solid(c) => Some(Paint::Solid(c.clone())),
            };
            let fill = paint(&s.fill).or_else(|| {
                // Unstyled frames and text boxes are transparent.
                (placeholder.is_none() && (s.geometry == "rect" || s.image.is_some()))
                    .then_some(Paint::None)
            });
            let line = match &s.stroke {
                bo_odf::Paint::Solid(c) => Some(LineSpec {
                    color: Some(c.clone()),
                    width: s.stroke_width.unwrap_or(9525),
                }),
                bo_odf::Paint::None => Some(LineSpec {
                    color: None,
                    width: 0,
                }),
                bo_odf::Paint::Inherit if placeholder.is_none() => Some(LineSpec {
                    color: None,
                    width: 0,
                }),
                bo_odf::Paint::Inherit => None,
            };
            shapes.push(ShapeSpec {
                name: format!("Shape {}", i + 1),
                x: s.x,
                y: s.y,
                cx: s.cx,
                cy: s.cy,
                rot: s.rotation,
                geometry: s.geometry.clone(),
                fill,
                line,
                placeholder,
                text: s
                    .paragraphs
                    .iter()
                    .map(|p| ParaSpec {
                        runs: p
                            .runs
                            .iter()
                            .map(|r| RunSpec {
                                text: r.text.clone(),
                                bold: r.bold,
                                italic: r.italic,
                                underline: r.underline,
                                size: r.size,
                                color: r.color.clone(),
                                font: r.font.clone(),
                            })
                            .collect(),
                        level: p.level,
                        align: p.align.clone(),
                        bullet: (placeholder.is_none() && p.bullet).then_some(true),
                    })
                    .collect(),
                anchor: s.anchor.clone(),
                picture: s.image.as_ref().map(|(data, ext)| Picture {
                    data: data.clone(),
                    ext: ext.clone(),
                }),
                ..ShapeSpec::default()
            });
        }
        out.slides.push(SlideSpec {
            layout: layout_for(&shapes),
            shapes,
            notes: slide.notes.clone(),
            background: slide.background.clone(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_follow_placeholders() {
        let ph = |p| ShapeSpec {
            placeholder: Some(p),
            ..ShapeSpec::default()
        };
        assert_eq!(
            layout_for(&[ph(Placeholder::CenterTitle)]),
            LayoutKind::Title
        );
        assert_eq!(
            layout_for(&[ph(Placeholder::Body)]),
            LayoutKind::TitleContent
        );
        assert_eq!(layout_for(&[]), LayoutKind::Blank);
    }
}
