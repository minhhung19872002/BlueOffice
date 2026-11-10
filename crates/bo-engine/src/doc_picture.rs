//! BlueDoc picture tools (CORE-PIC-001): the picture commands of
//! [`crate::picture_tools::PictureCommand`] on the document picture at the
//! selection — adjustments, crop (free, aspect ratio, shape, fill, fit), compress,
//! change and reset. Each edit is one undo step of the editing session.

use crate::edit::EditResult;
use crate::picture_tools::{self as pt, CompressReport, PictureCommand, PictureInfo, EMU_PER_PX};
use crate::{EngineError, Result};
use bo_edit::picture_tools::MediaSwap;
use bo_model::picture::{aspect_crop, aspect_ratio, fill_crop, fit_size, PictureFormat, PCT};
use bo_model::{Emu, Image};
use std::collections::BTreeMap;
use std::sync::Arc;

const EMU_PT: f64 = 12_700.0;

/// Uses of a media part: largest shown width and height (points), the crop shared
/// by every use (`None` when they differ) and the number of pictures.
type Uses = (f64, f64, Option<[i32; 4]>, usize);

fn format_of(img: &Image) -> PictureFormat {
    img.format.as_deref().cloned().unwrap_or_default()
}

fn set_format(img: &mut Image, f: PictureFormat) {
    img.format = (!f.is_plain()).then(|| Box::new(f));
}

/// Resize for a new crop so the picture keeps its scale.
fn recrop(img: &mut Image, new: Option<[i32; 4]>) {
    let [ol, ot, or, ob] = img.crop.unwrap_or([0; 4]).map(f64::from);
    let [l, t, r, b] = new.unwrap_or([0; 4]).map(f64::from);
    let uw = img.width.0 as f64 / ((PCT - ol - or) / PCT).max(1e-3);
    let uh = img.height.0 as f64 / ((PCT - ot - ob) / PCT).max(1e-3);
    img.width = Emu((uw * (PCT - l - r) / PCT).round().max(1.0) as i64);
    img.height = Emu((uh * (PCT - t - b) / PCT).round().max(1.0) as i64);
    img.crop = new.filter(|c| c.iter().any(|v| *v != 0));
}

impl crate::Engine {
    fn doc_media_pixels(&self, name: &str) -> Option<(f64, f64)> {
        let d = self.document()?.media.get(name)?;
        crate::edit::image_size(d).map(|(w, h)| (f64::from(w), f64::from(h)))
    }

    /// Run a picture tools command on the document picture at the selection.
    pub fn doc_picture_command(&mut self, cmd: PictureCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let mut previews = Vec::new();
        let mut report = None;
        let read_only = self.sec.read_only;
        let picture = self
            .session
            .as_ref()
            .and_then(bo_edit::Session::tools_picture);
        let pixels = picture
            .as_ref()
            .and_then(|p| self.doc_media_pixels(&p.media));
        let mut error: Option<String> = None;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let edits = !matches!(
            cmd,
            PictureCommand::PictureInfo | PictureCommand::PicturePreviews { .. }
        );
        let res: std::result::Result<(), String> = if read_only && edits {
            Err(crate::security::READ_ONLY.into())
        } else {
            let need = |p: Option<&Image>| p.cloned().ok_or_else(|| "noPicture".to_owned());
            let run =
                |r: bo_edit::Result<bo_edit::Selection>| r.map(|_| ()).map_err(|e| e.to_string());
            match cmd {
                PictureCommand::PictureInfo => Ok(()),
                PictureCommand::PicturePreviews { presets, size } => {
                    if let Some(img) = &picture {
                        if let Some(d) = s.doc().media.get(&img.media) {
                            previews = pt::previews(d, &presets, size.unwrap_or(96));
                        }
                    }
                    Ok(())
                }
                PictureCommand::PictureFx { fx } => need(picture.as_ref()).and_then(|_| {
                    run(s.update_picture(None, |img| {
                        let mut f = format_of(img);
                        f.fx = fx.clamped();
                        set_format(img, f);
                    }))
                }),
                PictureCommand::PictureCrop { crop } => {
                    if crop[0] + crop[2] >= PCT as i32 || crop[1] + crop[3] >= PCT as i32 {
                        Err("badCrop".into())
                    } else {
                        need(picture.as_ref()).and_then(|_| {
                            run(s.update_picture(None, |img| recrop(img, Some(crop))))
                        })
                    }
                }
                PictureCommand::PictureCropAspect { ratio } => {
                    match (aspect_ratio(&ratio), pixels) {
                        (Some((rw, rh)), Some((iw, ih))) => run(s.update_picture(None, |img| {
                            let c = aspect_crop(iw, ih, rw, rh);
                            img.crop = c.iter().any(|v| *v != 0).then_some(c);
                            img.height = Emu((img.width.0 as f64 * rh / rw).round() as i64);
                        })),
                        (None, _) => Err("badRatio".into()),
                        _ => Err("noPicture".into()),
                    }
                }
                PictureCommand::PictureCropShape { preset } => {
                    if bo_drawingml::preset(&preset).is_none() {
                        Err("badShape".into())
                    } else {
                        need(picture.as_ref()).and_then(|_| {
                            run(s.update_picture(None, |img| {
                                let mut f = format_of(img);
                                f.shape = (preset != "rect").then(|| preset.clone());
                                set_format(img, f);
                            }))
                        })
                    }
                }
                PictureCommand::PictureFill | PictureCommand::PictureFit => {
                    let fill = matches!(cmd, PictureCommand::PictureFill);
                    match pixels {
                        Some((iw, ih)) => run(s.update_picture(None, |img| {
                            let (fw, fh) = (img.width.0 as f64, img.height.0 as f64);
                            if fill {
                                let c = fill_crop(iw, ih, fw, fh);
                                img.crop = c.iter().any(|v| *v != 0).then_some(c);
                            } else {
                                let (w, h) = fit_size(iw, ih, fw, fh);
                                img.width = Emu(w.round() as i64);
                                img.height = Emu(h.round() as i64);
                                img.crop = None;
                            }
                        })),
                        None => Err("noPicture".into()),
                    }
                }
                PictureCommand::PictureCompress(spec) => {
                    let opts = spec.options();
                    let targets: Vec<Image> = if spec.all {
                        s.body_pictures()
                    } else {
                        picture.iter().cloned().collect()
                    };
                    // One new part per media part (its largest display size).
                    let mut by_media: BTreeMap<String, Uses> = BTreeMap::new();
                    for img in &targets {
                        let e = by_media
                            .entry(img.media.clone())
                            .or_insert((0.0, 0.0, img.crop, 0));
                        e.0 = e.0.max(img.width.0 as f64 / EMU_PT);
                        e.1 = e.1.max(img.height.0 as f64 / EMU_PT);
                        if e.2 != img.crop {
                            e.2 = None;
                        }
                        e.3 += 1;
                    }
                    let mut rep = CompressReport::default();
                    let mut swaps = Vec::new();
                    let mut names: Vec<String> = Vec::new();
                    for (media, (w, h, crop, uses)) in by_media {
                        let Some(data) = s.doc().media.get(&media).cloned() else {
                            continue;
                        };
                        // Cropped areas go only when every use shows the same crop.
                        let mut o = opts;
                        o.drop_cropped &= crop.is_some() || uses == 1;
                        let Some(c) = bo_raster::picfx::compress(&data, crop, w, h, &o) else {
                            continue;
                        };
                        rep.pictures += uses;
                        rep.before += data.len();
                        rep.after += c.bytes.len();
                        let ext = if c.jpeg { "jpeg" } else { "png" };
                        let mut n = s.part_number_start(s.doc().media.len() + 1);
                        let name = loop {
                            let cand = format!("/word/media/image{n}.{ext}");
                            if !s.doc().media.contains_key(&cand) && !names.contains(&cand) {
                                break cand;
                            }
                            n += 1;
                        };
                        names.push(name.clone());
                        swaps.push(MediaSwap {
                            old: media,
                            new: name,
                            bytes: Arc::new(c.bytes),
                            clear_crop: c.crop_applied,
                        });
                    }
                    report = Some(rep);
                    if swaps.is_empty() {
                        Ok(())
                    } else {
                        run(s.replace_media(swaps))
                    }
                }
                PictureCommand::PictureChange { data, name } => match (
                    picture.as_ref(),
                    pt::decode_data(&data).and_then(pt::drawable),
                ) {
                    (None, _) => Err("noPicture".into()),
                    (_, None) => Err("badImage".into()),
                    (Some(_), Some((bytes, ext, _))) => {
                        match crate::edit::image_size(&bytes) {
                            None => Err("badImage".into()),
                            Some((iw, ih)) => {
                                let media = s.new_media_name(ext);
                                run(s.update_picture(Some((media, Arc::new(bytes))), |img| {
                                    // Same box, no distortion: crop to fill it.
                                    let c = fill_crop(
                                        f64::from(iw),
                                        f64::from(ih),
                                        img.width.0 as f64,
                                        img.height.0 as f64,
                                    );
                                    img.crop = c.iter().any(|v| *v != 0).then_some(c);
                                    if !name.is_empty() {
                                        img.description.clone_from(&name);
                                    }
                                }))
                            }
                        }
                    }
                },
                PictureCommand::PictureReset { size } => need(picture.as_ref()).and_then(|_| {
                    run(s.update_picture(None, |img| {
                        if size {
                            if let Some((iw, ih)) = pixels {
                                // 96 pixels per inch.
                                img.width = Emu((iw * EMU_PER_PX).round() as i64);
                                img.height = Emu((ih * EMU_PER_PX).round() as i64);
                            }
                            img.crop = None;
                        } else {
                            recrop(img, None);
                        }
                        img.format = None;
                    }))
                }),
                PictureCommand::PictureToShapes | PictureCommand::PictureInsertSvg { .. } => {
                    Err("unsupported".into())
                }
            }
        };
        if let Err(e) = res {
            error = Some(e);
        }
        let dirty = self.relayout()?;
        let mut r = self.story_result(dirty, error)?;
        r.picture = self.doc_picture_info(previews, report).map(Box::new);
        r.collab = self.collab_after_edit();
        Ok(r)
    }

    /// Picture tools state of the document picture at the selection.
    fn doc_picture_info(
        &self,
        previews: Vec<String>,
        report: Option<CompressReport>,
    ) -> Option<PictureInfo> {
        let img = self
            .session
            .as_ref()
            .and_then(bo_edit::Session::tools_picture);
        let Some(img) = img else {
            return report.map(|r| PictureInfo {
                compressed: Some(r),
                ..PictureInfo::default()
            });
        };
        let data = self.document()?.media.get(&img.media)?;
        let f = format_of(&img);
        let mut i = pt::info(
            &img.media,
            data,
            f.fx,
            img.crop,
            f.shape.as_deref(),
            (img.width.0 as f64 / EMU_PT, img.height.0 as f64 / EMU_PT),
        );
        i.previews = previews;
        i.compressed = report;
        Some(i)
    }
}
