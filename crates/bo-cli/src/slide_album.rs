//! Corpus decks of SLIDE-ALBUM-001, made with the JSON commands the UI sends:
//! a photo album (pictures placed by resize / move, captions, frames, the
//! steps joined into one undo step), slides added from an outline
//! (`applyOutline`) and slides reused from other decks (`reuseOpen` +
//! `reuseSlides`, with and without their source design).

use bo_engine::Engine;

/// Names of the decks, in the order [`decks`] returns them.
pub const ALBUM_DECKS: [&str; 3] = [
    "slide-album-photos.pptx",
    "slide-album-outline.pptx",
    "slide-album-reuse.pptx",
];

fn run(e: &mut Engine, json: &str) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(&e.slide_command_json(json).ok()?).ok()?;
    v.get("error").is_none().then_some(v)
}

fn open(base: &[(&'static str, Vec<u8>)], name: &str) -> Option<Engine> {
    let (_, b) = base.iter().find(|(n, _)| *n == name)?;
    let mut e = Engine::with_bundled_fonts();
    e.open(b, name).ok()?;
    Some(e)
}

/// `(cx, cy, w, h)` of the selection's box in a command result.
fn bounds(v: &serde_json::Value) -> Option<(f64, f64, f64, f64)> {
    let b = v.get("bounds")?;
    Some((
        b.get("cx")?.as_f64()?,
        b.get("cy")?.as_f64()?,
        b.get("w")?.as_f64()?,
        b.get("h")?.as_f64()?,
    ))
}

/// Frame of an album picture.
#[derive(Clone, Copy)]
enum Frame {
    Rect,
    Rounded,
    Outline,
}

/// Height of a caption line and its gap to the picture (points), as in the UI.
const CAPTION: (f64, f64) = (30.0, 4.0);

/// Insert `png` and fit it into `cell` (`x, y, w, h` in points) keeping its
/// proportions, centred, with a caption line below when `caption` is given.
fn place(
    e: &mut Engine,
    png: &[u8],
    name: &str,
    cell: (f64, f64, f64, f64),
    caption: Option<&str>,
    frame: Frame,
) -> Option<()> {
    let r = serde_json::to_value(e.slide_insert_picture(png.to_vec(), name).ok()?).ok()?;
    let (_, _, w, h) = bounds(&r)?;
    let room = if caption.is_some() {
        cell.3 - CAPTION.0 - CAPTION.1
    } else {
        cell.3
    };
    let k = (cell.2 / w).min(room / h);
    let (tw, th) = (w * k, h * k);
    let r = run(
        e,
        &format!(
            r#"{{"cmd":"transform","transform":{{"kind":"resize","handle":"se","dx":{},"dy":{},"keepAspect":true}},"snap":false}}"#,
            tw - w,
            th - h
        ),
    )?;
    let (cx, cy, _, _) = bounds(&r)?;
    let (tx, ty) = (cell.0 + cell.2 / 2.0, cell.1 + room / 2.0);
    run(
        e,
        &format!(
            r#"{{"cmd":"transform","transform":{{"kind":"move","dx":{},"dy":{}}},"snap":false}}"#,
            tx - cx,
            ty - cy
        ),
    )?;
    match frame {
        Frame::Rect => {}
        Frame::Rounded => {
            run(e, r#"{"cmd":"pictureCropShape","preset":"roundRect"}"#)?;
        }
        Frame::Outline => {
            run(e, r#"{"cmd":"setLine","color":"404040","width":2.25}"#)?;
        }
    }
    if let Some(text) = caption {
        let y = ty + th / 2.0 + CAPTION.1;
        run(
            e,
            &format!(
                r#"{{"cmd":"insertTextBox","text":{},"rect":{{"x":{},"y":{},"w":{},"h":{}}}}}"#,
                serde_json::Value::from(text),
                cell.0,
                y,
                cell.2,
                CAPTION.0
            ),
        )?;
        run(
            e,
            r#"{"cmd":"formatText","format":{"kind":"align","align":"center"}}"#,
        )?;
    }
    Some(())
}

/// Index of the first layout of `kind` (§19.7.15 ST_SlideLayoutType).
fn layout_of(e: &Engine, kind: &str) -> Option<usize> {
    e.presentation()?
        .layouts
        .iter()
        .position(|l| l.kind.as_deref() == Some(kind))
}

/// Three pictures, two per slide, captions from the file names, one frame of
/// each kind; the album is one undo step.
fn photos(base: &[(&'static str, Vec<u8>)], png: &[u8]) -> Option<Vec<u8>> {
    let mut e = open(base, "slide-text-bullets.pptx")?;
    let (sw, sh) = {
        let s = &e.presentation()?.slide_size;
        (s.cx as f64 / 12_700.0, s.cy as f64 / 12_700.0)
    };
    let blank = layout_of(&e, "blank")?;
    let since = run(&mut e, r#"{"cmd":"historyLen"}"#)?
        .get("history")?
        .as_u64()?;
    // 5 % margin, 3 % gap, two columns (the UI's "two pictures" layout).
    let (m, gap) = (sw * 0.05, sw * 0.03);
    let cw = (sw - 2.0 * m - gap) / 2.0;
    let cells = [(m, m, cw, sh - 2.0 * m), (m + cw + gap, m, cw, sh - 2.0 * m)];
    let pictures = [
        ("Bãi biển.png", Frame::Rect),
        ("Núi rừng.png", Frame::Rounded),
        ("Phố cổ.png", Frame::Outline),
    ];
    for (i, (name, frame)) in pictures.iter().enumerate() {
        if i % 2 == 0 {
            run(&mut e, &format!(r#"{{"cmd":"addSlide","layout":{blank}}}"#))?;
        }
        let caption = name.trim_end_matches(".png");
        place(&mut e, png, name, cells[i % 2], Some(caption), *frame)?;
    }
    run(&mut e, r#"{"cmd":"select","ids":[]}"#)?;
    run(&mut e, &format!(r#"{{"cmd":"mergeHistory","since":{since}}}"#))?;
    e.save_pptx().ok()
}

/// Two title + content slides added after the first slide from an outline.
fn outline(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    let mut e = open(base, "slide-text-bullets.pptx")?;
    let now = run(&mut e, r#"{"cmd":"outline"}"#)?;
    let mut slides: Vec<serde_json::Value> = now
        .get("outline")?
        .as_array()?
        .iter()
        .enumerate()
        .map(|(i, s)| serde_json::json!({ "slide": i, "title": s["title"], "bullets": s["bullets"] }))
        .collect();
    let added = [
        serde_json::json!({ "title": "Tổng quan", "bullets": [
            { "text": "Phạm vi", "level": 0 },
            { "text": "Thuật ngữ", "level": 0 },
        ]}),
        serde_json::json!({ "title": "Thiết kế", "bullets": [
            { "text": "Mô hình", "level": 0 },
            { "text": "Chi tiết", "level": 1 },
        ]}),
    ];
    slides.splice(1..1, added);
    let cmd = serde_json::json!({ "cmd": "applyOutline", "slides": slides, "current": 1 });
    run(&mut e, &cmd.to_string())?;
    e.save_pptx().ok()
}

/// Slides reused from two decks: a pictures slide taking this deck's design,
/// and two text slides of a deck with another design keeping theirs (its layout,
/// master and theme are imported, §19.3.1.39, §19.3.1.42, §20.1.6.9).
fn reuse(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let get = |n: &str| base.iter().find(|(b, _)| *b == n).map(|(_, d)| d.clone());
    let open_src = |e: &mut Engine, bytes: &[u8]| {
        let cmd = serde_json::json!({
            "cmd": "reuseOpen",
            "name": "nguon.pptx",
            "base64": base64::engine::general_purpose::STANDARD.encode(bytes),
            "width": 96,
        });
        let v: serde_json::Value =
            serde_json::from_str(&e.slide_command_json(&cmd.to_string()).ok()?).ok()?;
        v.get("reuse")?.get("slides")?.as_array().map(Vec::len)
    };
    let mut e = open(base, "slide-shapes-presets.pptx")?;
    open_src(&mut e, &get("slide-images-crop.pptx")?)?;
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#)?;
    run(&mut e, r#"{"cmd":"reuseSlides","indices":[0]}"#)?;
    // Another design: its own theme name and colours, layouts under other names.
    let mut other = bo_pptx::read_pptx(&get("slide-text-bullets.pptx")?).ok()?;
    for t in &mut other.themes {
        t.name = Some("Nguồn".into());
        t.colors.name = "Nguồn".into();
    }
    for l in &mut other.layouts {
        let name = l.data.name.clone().unwrap_or_default();
        l.data.name = Some(format!("{name} (nguồn)"));
    }
    open_src(&mut e, &bo_pptx::write_pptx(&other).ok()?)?;
    run(
        &mut e,
        r#"{"cmd":"reuseSlides","indices":[0,1],"keepSource":true}"#,
    )?;
    run(&mut e, r#"{"cmd":"reuseClose"}"#);
    e.save_pptx().ok()
}

/// The album decks, built from the generated base decks.
pub fn decks(base: &[(&'static str, Vec<u8>)], picture: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let built = [photos(base, picture), outline(base), reuse(base)];
    ALBUM_DECKS
        .iter()
        .zip(built)
        .filter_map(|(n, d)| d.map(|d| (*n, d)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::slide::ShapeNode;

    fn built() -> Vec<(&'static str, Vec<u8>)> {
        let base = crate::slide_samples::base_decks();
        decks(&base, &crate::slide_samples::sample_png())
    }

    fn deck(all: &[(&'static str, Vec<u8>)], name: &str) -> bo_model::slide::Presentation {
        let (_, b) = all
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("{name} not built"));
        bo_pptx::read_pptx(b).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    fn pictures(s: &bo_model::slide::Slide) -> Vec<&bo_model::slide::Picture> {
        s.data
            .shapes
            .iter()
            .filter_map(|n| match n {
                ShapeNode::Picture(p) => Some(p.as_ref()),
                _ => None,
            })
            .collect()
    }

    // source: ECMA-376 Part 1, §19.3.1.37 pic, §20.1.7.6 xfrm, §20.1.9.18 prstGeom.
    #[test]
    fn album_pictures_are_embedded_and_placed_in_their_cells() {
        let all = built();
        assert_eq!(all.len(), ALBUM_DECKS.len());
        let base = crate::slide_samples::base_decks();
        let n = bo_pptx::read_pptx(
            &base
                .iter()
                .find(|(n, _)| *n == "slide-text-bullets.pptx")
                .expect("base")
                .1,
        )
        .expect("read")
        .slides
        .len();
        let p = deck(&all, ALBUM_DECKS[0]);
        // Two pictures on the first album slide, one on the second.
        assert_eq!(p.slides.len(), n + 2);
        let (first, second) = (pictures(&p.slides[1]), pictures(&p.slides[2]));
        assert_eq!((first.len(), second.len()), (2, 1));
        // Every picture has its own embedded media part.
        let media = p
            .parts
            .iter()
            .filter(|x| x.name.starts_with("/ppt/media/"))
            .count();
        assert!(media >= 3, "{media}");
        // The sample picture is 48 × 32: each cell is limited by its width. Slide 960 × 540 pt:
        // margin 48, gap 28.8, cell width 417.6 pt = 5 303 520 EMU, at x = 48 pt and 494.4 pt.
        let emu = |pt: f64| (pt * 12_700.0).round() as i64;
        for (pic, x) in first.iter().zip([48.0, 494.4]) {
            let xf = pic.props.xfrm.as_ref().expect("xfrm");
            let (off, ext) = (xf.off.expect("off"), xf.ext.expect("ext"));
            assert!((ext.0 - emu(417.6)).abs() <= 2, "{ext:?}");
            assert!((ext.1 - emu(417.6 * 32.0 / 48.0)).abs() <= 2, "{ext:?}");
            assert!((off.0 - emu(x)).abs() <= 2, "{off:?}");
        }
        // Captions: one text box per picture with the file name.
        let text = bo_engine::slide_edit::clip::plain_text(&p.slides[1].data.shapes);
        assert!(text.contains("Bãi biển") && text.contains("Núi rừng"), "{text}");
        assert!(bo_engine::slide_edit::clip::plain_text(&p.slides[2].data.shapes).contains("Phố cổ"));
    }

    #[test]
    fn album_is_one_undo_step() {
        let base = crate::slide_samples::base_decks();
        let mut e = open(&base, "slide-text-bullets.pptx").expect("open");
        let before = e.presentation().expect("deck").clone();
        // The same steps as `photos`, then one undo.
        let bytes = photos(&base, &crate::slide_samples::sample_png()).expect("album");
        assert!(bo_pptx::read_pptx(&bytes).expect("read").slides.len() > before.slides.len());
        let blank = layout_of(&e, "blank").expect("blank layout");
        let since = run(&mut e, r#"{"cmd":"historyLen"}"#).expect("len")["history"]
            .as_u64()
            .expect("history");
        run(&mut e, &format!(r#"{{"cmd":"addSlide","layout":{blank}}}"#)).expect("slide");
        place(
            &mut e,
            &crate::slide_samples::sample_png(),
            "a.png",
            (48.0, 48.0, 400.0, 400.0),
            Some("a"),
            Frame::Rounded,
        )
        .expect("place");
        run(&mut e, &format!(r#"{{"cmd":"mergeHistory","since":{since}}}"#)).expect("merge");
        let r = run(&mut e, r#"{"cmd":"undo"}"#).expect("undo");
        assert_eq!(r["canUndo"], false);
        assert_eq!(e.presentation().expect("deck"), &before);
        // Redo brings the whole album back.
        run(&mut e, r#"{"cmd":"redo"}"#).expect("redo");
        assert_eq!(e.presentation().expect("deck").slides.len(), before.slides.len() + 1);
    }

    // source: ECMA-376 Part 1, §21.1.2.2.7 pPr@lvl.
    #[test]
    fn outline_slides_have_titles_and_bullet_levels() {
        let all = built();
        let p = deck(&all, ALBUM_DECKS[1]);
        let o = bo_engine::slide_edit::outline::outline(&p);
        assert_eq!(o[1].title, "Tổng quan");
        let bullets = |i: usize| -> Vec<(String, u8)> {
            o[i].bullets.iter().map(|b| (b.text.clone(), b.level)).collect()
        };
        assert_eq!(
            bullets(1),
            vec![("Phạm vi".to_owned(), 0), ("Thuật ngữ".to_owned(), 0)]
        );
        assert_eq!(o[2].title, "Thiết kế");
        assert_eq!(
            bullets(2),
            vec![("Mô hình".to_owned(), 0), ("Chi tiết".to_owned(), 1)]
        );
        // The slides of the base deck are still there, around the new ones.
        let base = crate::slide_samples::base_decks();
        let b = bo_pptx::read_pptx(
            &base
                .iter()
                .find(|(n, _)| *n == "slide-text-bullets.pptx")
                .expect("base")
                .1,
        )
        .expect("read");
        assert_eq!(p.slides.len(), b.slides.len() + 2);
        let ob = bo_engine::slide_edit::outline::outline(&b);
        assert_eq!(o[0].title, ob[0].title);
        assert_eq!(o[3].title, ob[1].title);
    }

    // source: ECMA-376 Part 1, §19.3.1.39 sldLayout, §19.3.1.42 sldMaster, §20.1.6.9 theme.
    #[test]
    fn reused_slides_keep_or_take_the_design() {
        let all = built();
        let p = deck(&all, ALBUM_DECKS[2]);
        let base = crate::slide_samples::base_decks();
        let read = |n: &str| {
            bo_pptx::read_pptx(&base.iter().find(|(b, _)| *b == n).expect("base").1).expect("read")
        };
        let target = read("slide-shapes-presets.pptx");
        assert_eq!(p.slides.len(), target.slides.len() + 3);
        // The pictures slide took this deck's design; the two text slides brought theirs.
        assert_eq!(p.masters.len(), target.masters.len() + 1);
        assert_eq!(p.themes.len(), target.themes.len() + 1);
        assert_eq!(pictures(&p.slides[1]).len(), pictures(&read("slide-images-crop.pptx").slides[0]).len());
        assert!(p.slides[1].layout < target.layouts.len());
        for i in [2, 3] {
            let l = &p.layouts[p.slides[i].layout];
            assert!(p.slides[i].layout >= target.layouts.len());
            assert!(l.data.name.as_deref().is_some_and(|n| n.ends_with("(nguồn)")));
            assert_eq!(p.themes[p.masters[l.master].theme].name.as_deref(), Some("Nguồn"));
        }
        let src = read("slide-text-bullets.pptx");
        let text = |p: &bo_model::slide::Presentation, i: usize| {
            bo_engine::slide_edit::clip::plain_text(&p.slides[i].data.shapes)
        };
        assert_eq!(text(&p, 2), text(&src, 0));
        assert_eq!(text(&p, 3), text(&src, 1));
    }
}
