//! Blend modes inside an isolated transparency group (ISO 32000-1 §11.4.5, §11.3.5) against a
//! reference raster computed with the specification's compositing formulas.
#![cfg(windows)]

use bo_gpu_win::{Device, PageImage};
use bo_render::{BlendMode, Command, PageList, Rgba};

fn render(commands: Vec<Command>, scale: f32, region: Option<[u32; 4]>) -> PageImage {
    let device = Device::new().expect("Direct2D hardware or WARP");
    let page = PageList {
        width: 60.0,
        height: 20.0,
        commands,
    };
    device
        .render_page_region(
            &page,
            &bo_text::FontManager::new(),
            scale,
            &|_| None,
            region,
        )
        .expect("render")
}

fn rect(x: f32, w: f32, color: Rgba) -> Command {
    Command::FillRect {
        x,
        y: 0.0,
        w,
        h: 20.0,
        color,
    }
}

/// Yellow backdrop under the whole page; inside the group, pink on x 0–40 (Normal), then blue
/// with Multiply on x 20–60.
fn page(isolated: bool) -> Vec<Command> {
    vec![
        rect(0.0, 60.0, Rgba(255, 255, 0, 255)),
        Command::PushGroup {
            blend: None,
            mask: None,
            knockout: false,
            isolated,
        },
        rect(0.0, 40.0, Rgba(255, 128, 128, 255)),
        Command::PushGroup {
            blend: Some(BlendMode::Multiply),
            mask: None,
            knockout: false,
            isolated: false,
        },
        rect(20.0, 40.0, Rgba(128, 128, 255, 255)),
        Command::PopGroup,
        Command::PopGroup,
    ]
}

/// §11.3.6 / §11.4.8: the reference raster of [`page`], per 20-point column.
/// Isolated: the group starts transparent, so blue meets nothing on x 40–60 and stays blue;
/// over pink it multiplies with pink. Not isolated: blue multiplies with the page's yellow.
fn expected(isolated: bool) -> [[i32; 3]; 3] {
    let mul = |a: [i32; 3], b: [i32; 3]| [a[0] * b[0] / 255, a[1] * b[1] / 255, a[2] * b[2] / 255];
    let (yellow, pink, blue) = ([255, 255, 0], [255, 128, 128], [128, 128, 255]);
    [
        pink,
        mul(pink, blue),
        if isolated { blue } else { mul(yellow, blue) },
    ]
}

fn px(image: &PageImage, x: usize, y: usize) -> [i32; 3] {
    let o = (y * image.width as usize + x) * 4;
    [
        i32::from(image.pixels[o]),
        i32::from(image.pixels[o + 1]),
        i32::from(image.pixels[o + 2]),
    ]
}

fn close(a: [i32; 3], b: [i32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 2)
}

#[test]
fn blended_objects_in_an_isolated_group_read_the_group_backdrop() {
    for isolated in [true, false] {
        for scale in [1.0_f32, 2.0] {
            let image = render(page(isolated), scale, None);
            let want = expected(isolated);
            for (col, w) in want.iter().enumerate() {
                let x = ((col as f32 * 20.0 + 10.0) * scale) as usize;
                let got = px(&image, x, (10.0 * scale) as usize);
                assert!(
                    close(got, *w),
                    "isolated {isolated} scale {scale} column {col}: {got:?} vs {w:?}"
                );
            }
        }
    }
}

#[test]
fn a_band_of_an_isolated_group_matches_the_whole_page() {
    let full = render(page(true), 2.0, None);
    let band = render(page(true), 2.0, Some([30, 8, 60, 16]));
    for y in 0..band.height as usize {
        for x in 0..band.width as usize {
            let o = (y * band.width as usize + x) * 4;
            let f = ((y + band.top as usize) * full.width as usize + x + band.left as usize) * 4;
            assert_eq!(band.pixels[o..o + 3], full.pixels[f..f + 3], "({x}, {y})");
        }
    }
}
