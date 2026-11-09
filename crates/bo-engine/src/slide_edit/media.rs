//! Video and audio clips on slides: listing them for playback and inserting
//! new ones.
//!
//! A clip is a picture (§19.3.1.37 pic) whose `nvPr` holds §20.1.3.6
//! videoFile / §20.1.3.1 audioFile (`r:link`) and, for embedded clips, the
//! [MS-PPTX] §2.3.2 `p14:media` extension (`r:embed`); the picture's blip is
//! the poster frame shown when the clip does not play.

use super::build;
use super::geom::{place_slide, Placed};
use super::ops::{OpError, SlideOp, Tx};
use super::show::MediaOut;
use super::{insert_top, next_shape_id, slide_size, EditError, SlideSel, R};
use bo_model::slide::{Presentation, RawPart, Rel, ShapeNode, XmlElement};

/// [MS-PPTX] relationship type of `p14:media@r:embed`.
pub const REL_MEDIA: &str = "http://schemas.microsoft.com/office/2007/relationships/media";
/// [MS-PPTX] §2.3.2 extension URI of `p14:media` in `nvPr/extLst`.
const MEDIA_EXT_URI: &str = "{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}";
const NS_P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";

/// `video` / `audio` and the MIME type of a media file, by extension.
pub fn media_kind(name: &str) -> Option<(&'static str, &'static str)> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "mp4" | "m4v" => ("video", "video/mp4"),
        "webm" => ("video", "video/webm"),
        "ogv" => ("video", "video/ogg"),
        "mov" => ("video", "video/quicktime"),
        "wmv" => ("video", "video/x-ms-wmv"),
        "avi" => ("video", "video/x-msvideo"),
        "mp3" => ("audio", "audio/mpeg"),
        "m4a" => ("audio", "audio/mp4"),
        "wav" => ("audio", "audio/wav"),
        "ogg" | "oga" => ("audio", "audio/ogg"),
        "aac" => ("audio", "audio/aac"),
        "flac" => ("audio", "audio/flac"),
        "wma" => ("audio", "audio/x-ms-wma"),
        _ => return None,
    })
}

fn descendants<'a>(el: &'a XmlElement, out: &mut Vec<&'a XmlElement>) {
    for c in el.elements() {
        out.push(c);
        descendants(c, out);
    }
}

fn attr_suffix<'a>(el: &'a XmlElement, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|(k, _)| k.rsplit(':').next() == Some(local))
        .map(|(_, v)| v.as_str())
}

/// Video and audio clips of a slide (any depth).
pub fn media_out(pres: &Presentation, slide: usize) -> Vec<MediaOut> {
    let Some(s) = pres.slides.get(slide) else {
        return Vec::new();
    };
    let placed = place_slide(pres, slide);
    let mut flat: Vec<&Placed> = Vec::new();
    super::connect::walk(&placed, &mut |p| flat.push(p));
    let mut out = Vec::new();
    for p in flat {
        let Some(ShapeNode::Picture(pic)) = super::ops::node_at(pres, slide, &p.path) else {
            continue;
        };
        let mut all = Vec::new();
        for x in &pic.nv.nv_extra {
            all.push(&x.xml);
            descendants(&x.xml, &mut all);
        }
        let Some(file) = all
            .iter()
            .find(|e| matches!(e.name.as_str(), "a:videoFile" | "a:audioFile"))
        else {
            continue;
        };
        let kind = if file.name == "a:videoFile" {
            "video"
        } else {
            "audio"
        };
        let rel = |rid: &str| s.info.rels.iter().find(|r| r.id == rid);
        // Embedded bytes: p14:media@r:embed, else an internal videoFile/audioFile link.
        let embed = all
            .iter()
            .find(|e| e.local() == "media" && e.name != "a:media")
            .and_then(|e| attr_suffix(e, "embed"))
            .and_then(rel)
            .filter(|r| !r.external);
        let link = attr_suffix(file, "link").and_then(rel);
        let internal = embed.or(link.filter(|r| !r.external));
        let part = internal.map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target));
        let mime = part.as_ref().and_then(|name| {
            pres.parts
                .iter()
                .find(|q| q.name.eq_ignore_ascii_case(name))
                .and_then(|q| q.content_type.clone())
                .filter(|t| t.starts_with("video/") || t.starts_with("audio/"))
                .or_else(|| media_kind(name).map(|(_, m)| m.to_owned()))
        });
        let r = p.b.bounds();
        let playback = super::media_playback::playback_of(pres, slide, pic, p.id);
        out.push(MediaOut {
            id: p.id,
            name: pic.nv.name.clone(),
            kind,
            rect: [r.x, r.y, r.w, r.h],
            part,
            mime,
            link: link.filter(|r| r.external).map(|r| r.target.clone()),
            playback,
        });
    }
    out
}

/// Poster frame of a new clip: a dark frame with a play (video) or speaker
/// (audio) sign, BlueSlide's own drawing.
pub fn poster_page(kind: &str) -> bo_render::PageList {
    use bo_render::{Command, Paint, PathSeg, Rgba};
    let (w, h) = if kind == "video" {
        (320.0, 180.0)
    } else {
        (64.0, 64.0)
    };
    let fill = |c: Rgba| Some(Paint::Solid { color: c });
    let mut commands = vec![Command::FillRect {
        x: 0.0,
        y: 0.0,
        w,
        h,
        color: Rgba(31, 41, 55, 255),
    }];
    let (cx, cy, s) = (w / 2.0, h / 2.0, h.min(w) * 0.22);
    if kind == "video" {
        commands.push(Command::Path {
            segs: vec![
                PathSeg::M(cx - s * 0.7, cy - s),
                PathSeg::L(cx + s, cy),
                PathSeg::L(cx - s * 0.7, cy + s),
                PathSeg::Z,
            ],
            fill: fill(Rgba(255, 255, 255, 230)),
            stroke: None,
        });
    } else {
        // Speaker: box + cone.
        commands.push(Command::Path {
            segs: vec![
                PathSeg::M(cx - s * 1.4, cy - s * 0.5),
                PathSeg::L(cx - s * 0.6, cy - s * 0.5),
                PathSeg::L(cx + s * 0.4, cy - s * 1.3),
                PathSeg::L(cx + s * 0.4, cy + s * 1.3),
                PathSeg::L(cx - s * 0.6, cy + s * 0.5),
                PathSeg::L(cx - s * 1.4, cy + s * 0.5),
                PathSeg::Z,
            ],
            fill: fill(Rgba(255, 255, 255, 230)),
            stroke: None,
        });
        commands.push(Command::FillRect {
            x: cx + s * 0.8,
            y: cy - s * 0.6,
            w: s * 0.25,
            h: s * 1.2,
            color: Rgba(96, 165, 250, 255),
        });
    }
    bo_render::PageList {
        width: w,
        height: h,
        commands,
    }
}

fn new_rid(rels: &[Rel], taken: &[String]) -> String {
    (1..=rels.len() + taken.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| rels.iter().all(|r| &r.id != id) && !taken.contains(id))
        .unwrap_or_default()
}

/// Insert a clip (`bytes` of file `name`) with poster frame `poster` (PNG) on
/// the current slide, centred.
pub fn cmd_insert_media(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    bytes: Vec<u8>,
    name: &str,
    poster: Vec<u8>,
) -> R<()> {
    let (kind, mime) = media_kind(name).ok_or(EditError::Refused("badMedia"))?;
    if bytes.is_empty() {
        return Err(EditError::Refused("badMedia"));
    }
    let slide = sel.slide;
    let (sw, sh) = slide_size(tx.pres);
    let (w, h) = if kind == "video" {
        let w = (sw * 0.6).min(480.0);
        (w, w * 9.0 / 16.0)
    } else {
        (48.0, 48.0)
    };
    let (x, y) = if kind == "video" {
        ((sw - w) / 2.0, (sh - h) / 2.0)
    } else {
        (sw - w - 24.0, sh - h - 24.0)
    };
    let ext = name
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let media_part = build::unique_part(tx.pres, "/ppt/media/media", &ext);
    let poster_part = build::unique_part_except(
        tx.pres,
        "/ppt/media/image",
        "png",
        std::slice::from_ref(&media_part),
    );
    let file = |p: &str| format!("../media/{}", p.rsplit('/').next().unwrap_or_default());
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let mut taken = Vec::new();
    for _ in 0..3 {
        let id = new_rid(&s.info.rels, &taken);
        taken.push(id);
    }
    let [link_id, embed_id, image_id] = [taken[0].clone(), taken[1].clone(), taken[2].clone()];
    let id = next_shape_id(tx.pres, slide);
    let tag = if kind == "video" {
        "a:videoFile"
    } else {
        "a:audioFile"
    };
    let emu = |pt: f64| (pt * super::geom::EMU_PT).round() as i64;
    let xml = format!(
        concat!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{name}"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr>"#,
            r#"<p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><{tag} r:link="{link}"/>"#,
            r#"<p:extLst><p:ext uri="{uri}"><p14:media xmlns:p14="{p14}" r:embed="{embed}"/></p:ext></p:extLst></p:nvPr></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="{image}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
        ),
        id = id,
        name = build::esc(name),
        tag = tag,
        link = link_id,
        uri = MEDIA_EXT_URI,
        p14 = NS_P14,
        embed = embed_id,
        image = image_id,
        x = emu(x),
        y = emu(y),
        cx = emu(w),
        cy = emu(h),
    );
    let node = build::node(&xml);
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![
            RawPart {
                name: media_part.clone(),
                content_type: Some(mime.into()),
                data: bytes,
            },
            RawPart {
                name: poster_part.clone(),
                content_type: Some("image/png".into()),
                data: poster,
            },
        ],
    })?;
    tx.fields(slide, |f| {
        // §15.2.17 video / §15.2.2 audio relationship, the [MS-PPTX] media one, the poster.
        f.rels.push(Rel {
            id: link_id.clone(),
            rel_type: format!("{}{kind}", bo_pptx::RELATIONSHIP_NS),
            target: file(&media_part),
            external: false,
        });
        f.rels.push(Rel {
            id: embed_id.clone(),
            rel_type: REL_MEDIA.into(),
            target: file(&media_part),
            external: false,
        });
        f.rels.push(Rel {
            id: image_id.clone(),
            rel_type: format!("{}image", bo_pptx::RELATIONSHIP_NS),
            target: file(&poster_part),
            external: false,
        });
    })?;
    insert_top(tx, sel, node)
}
