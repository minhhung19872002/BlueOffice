//! Optimize / compress a PDF (BlueEditor File › Optimize): pictures drawn at more than a
//! chosen resolution are downsampled (box filter) and written again — photos (JPEG sources,
//! colour) as baseline JPEG through the caller's encoder, flat pictures with Flate — then
//! the file is written with unused objects dropped, every stream compressed and objects
//! packed into object streams (ISO 32000-1 §7.5.7, §7.5.8).
//!
//! Only image XObjects drawn directly by page content with `DeviceGray` / `DeviceRGB` /
//! ICC / indexed colour and no masks are resampled; anything else is kept as it is.
use crate::color::ColorSpace;
use crate::objects::ObjectKind;
use crate::{PdfDocument, SaveOptions};
use lopdf::{Object, ObjectId};
use std::collections::HashMap;

/// What to optimize.
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizeOptions {
    /// Downsample pictures above this resolution (pixels per inch at their largest size on a
    /// page); 0 keeps every picture.
    pub max_ppi: f32,
    /// Only pictures above `max_ppi × threshold` are touched (like the usual 1.5).
    pub threshold: f32,
    /// JPEG quality 1–100 for photos.
    pub jpeg_quality: u8,
    /// Pack objects into object streams.
    pub object_streams: bool,
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self {
            max_ppi: 150.0,
            threshold: 1.5,
            jpeg_quality: 75,
            object_streams: true,
        }
    }
}

/// What optimizing did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizeReport {
    /// File size before (the bytes the document was opened or last saved with).
    pub before: usize,
    /// Size of the optimized file.
    pub after: usize,
    /// Pictures downsampled.
    pub images: usize,
    /// Objects of the opened file that nothing refers to any more (dropped).
    pub unused: usize,
}

/// A JPEG encoder: RGBA pixels, width, height, quality → JFIF bytes.
pub type JpegEncoder<'a> = &'a dyn Fn(&[u8], u32, u32, u8) -> Option<Vec<u8>>;

/// Box-filter `rgba` (`w` × `h`, 4 bytes a pixel) down to `nw` × `nh`.
fn downsample(rgba: &[u8], w: usize, h: usize, nw: usize, nh: usize) -> Vec<u8> {
    let mut out = vec![0_u8; nw * nh * 4];
    for y in 0..nh {
        let y0 = y * h / nh;
        let y1 = ((y + 1) * h / nh).max(y0 + 1).min(h);
        for x in 0..nw {
            let x0 = x * w / nw;
            let x1 = ((x + 1) * w / nw).max(x0 + 1).min(w);
            let mut acc = [0_u32; 4];
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = (yy * w + xx) * 4;
                    for c in 0..4 {
                        acc[c] += u32::from(rgba[i + c]);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (y * nw + x) * 4;
            for c in 0..4 {
                out[o + c] = (acc[c] / n.max(1)) as u8;
            }
        }
    }
    out
}

impl PdfDocument {
    /// Largest size (points) each image XObject is drawn at by page content.
    fn drawn_sizes(&mut self) -> HashMap<ObjectId, (f32, f32)> {
        let mut out: HashMap<ObjectId, (f32, f32)> = HashMap::new();
        for page in 0..self.pages.len() {
            let Ok(objs) = self.page_objects(page) else {
                continue;
            };
            let Some(pid) = self.pages.get(page).map(|p| p.id) else {
                continue;
            };
            for o in objs.iter().filter(|o| o.kind == ObjectKind::Image) {
                if o.rec.op != b"Do" {
                    continue;
                }
                let Some(Ok(name)) = o.rec.args.first().map(Object::as_name) else {
                    continue;
                };
                let id = self
                    .doc
                    .get_dictionary(pid)
                    .ok()
                    .and_then(|p| crate::inherited(&self.doc, p, b"Resources"))
                    .and_then(|r| r.as_dict().ok())
                    .and_then(|r| r.get(b"XObject").ok())
                    .and_then(|x| crate::resolve(&self.doc, x).as_dict().ok())
                    .and_then(|x| x.get(name).ok())
                    .and_then(|r| r.as_reference().ok());
                if let Some(id) = id {
                    let w = (o.bbox[2] - o.bbox[0]).abs();
                    let h = (o.bbox[3] - o.bbox[1]).abs();
                    let e = out.entry(id).or_insert((0.0, 0.0));
                    *e = (e.0.max(w), e.1.max(h));
                }
            }
        }
        out
    }

    /// Downsample the pictures drawn above `o.max_ppi` (one undo step); returns how many.
    pub fn downsample_images(&mut self, o: &OptimizeOptions, jpeg: JpegEncoder<'_>) -> usize {
        if o.max_ppi <= 0.0 {
            return 0;
        }
        let sizes = self.drawn_sizes();
        let mut count = 0;
        self.begin("optimize");
        for (id, (pw, ph)) in sizes {
            if pw < 1.0 || ph < 1.0 {
                continue;
            }
            let Ok(Object::Stream(s)) = self.doc.get_object(id) else {
                continue;
            };
            let d = &s.dict;
            if d.has(b"SMask") || d.has(b"Mask") || d.has(b"Decode") {
                continue;
            }
            if d.get(b"ImageMask")
                .and_then(Object::as_bool)
                .unwrap_or(false)
            {
                continue;
            }
            let w = d.get(b"Width").and_then(Object::as_i64).unwrap_or(0);
            let h = d.get(b"Height").and_then(Object::as_i64).unwrap_or(0);
            let (Ok(w), Ok(h)) = (usize::try_from(w), usize::try_from(h)) else {
                continue;
            };
            // Target size at max_ppi.
            let tw = (pw / 72.0 * o.max_ppi).ceil().max(1.0) as usize;
            let th = (ph / 72.0 * o.max_ppi).ceil().max(1.0) as usize;
            if (w as f32) <= tw as f32 * o.threshold && (h as f32) <= th as f32 * o.threshold {
                continue;
            }
            let k = (tw as f32 / w as f32).max(th as f32 / h as f32).min(1.0);
            let (nw, nh) = (
                ((w as f32 * k).round() as usize).max(1),
                ((h as f32 * k).round() as usize).max(1),
            );
            let none = |_: &[u8]| None;
            let space = d
                .get(b"ColorSpace")
                .ok()
                .and_then(|c| ColorSpace::parse(&self.doc, c, &none, 0));
            let simple = matches!(
                space,
                Some(ColorSpace::Gray | ColorSpace::Rgb | ColorSpace::Indexed { .. })
            );
            let filter_is = |n: &[u8]| match d.get(b"Filter") {
                Ok(Object::Name(f)) => f == n,
                Ok(Object::Array(a)) => a.iter().any(|f| f.as_name().ok() == Some(n)),
                _ => false,
            };
            let was_jpeg = filter_is(b"DCTDecode");
            if !simple && !was_jpeg {
                continue;
            }
            let gray = matches!(space, Some(ColorSpace::Gray));
            let Some(img) = crate::image::decode(&crate::image::Params {
                doc: &self.doc,
                dict: d,
                data: &s.content,
                space,
                fill: [0.0; 3],
            }) else {
                continue;
            };
            if img.width as usize != w || img.height as usize != h {
                continue;
            }
            let small = downsample(&img.pixels, w, h, nw, nh);
            let old_len = s.content.len();
            let mut nd = lopdf::Dictionary::new();
            nd.set("Type", Object::Name(b"XObject".to_vec()));
            nd.set("Subtype", Object::Name(b"Image".to_vec()));
            nd.set("Width", nw as i64);
            nd.set("Height", nh as i64);
            nd.set("BitsPerComponent", 8);
            if let Ok(i) = d.get(b"Interpolate") {
                nd.set("Interpolate", i.clone());
            }
            let photo = was_jpeg && !gray;
            let data = if photo {
                let Some(j) = jpeg(&small, nw as u32, nh as u32, o.jpeg_quality) else {
                    continue;
                };
                nd.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
                nd.set("Filter", Object::Name(b"DCTDecode".to_vec()));
                j
            } else {
                let samples: Vec<u8> = if gray {
                    small.chunks_exact(4).map(|p| p[0]).collect()
                } else {
                    small
                        .chunks_exact(4)
                        .flat_map(|p| [p[0], p[1], p[2]])
                        .collect()
                };
                nd.set(
                    "ColorSpace",
                    Object::Name(if gray {
                        b"DeviceGray".to_vec()
                    } else {
                        b"DeviceRGB".to_vec()
                    }),
                );
                nd.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                miniz_oxide::deflate::compress_to_vec_zlib(&samples, 9)
            };
            if data.len() >= old_len {
                continue;
            }
            self.touch(id);
            if let Some(obj) = self.doc.objects.get_mut(&id) {
                *obj = Object::Stream(lopdf::Stream::new(nd, data));
                count += 1;
            }
        }
        self.commit();
        count
    }

    /// Optimize: downsample pictures (undoable), then write the whole file compressed with
    /// unused objects dropped. Returns the new file and what changed.
    pub fn optimize(
        &mut self,
        o: &OptimizeOptions,
        jpeg: JpegEncoder<'_>,
    ) -> Result<(Vec<u8>, OptimizeReport), String> {
        let before = self.bytes().len();
        let objects_before = self.doc.objects.len();
        let images = self.downsample_images(o, jpeg);
        let bytes = self.save_full(SaveOptions {
            compress: true,
            object_streams: o.object_streams,
            prune: true,
        })?;
        let kept = lopdf::Document::load_mem(&bytes).map_or(objects_before, |d| {
            // Object streams themselves are new objects.
            d.objects
                .values()
                .filter(|o| {
                    !matches!(o, Object::Stream(s)
                        if s.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"ObjStm"))
                })
                .count()
        });
        Ok((
            bytes.clone(),
            OptimizeReport {
                before,
                after: bytes.len(),
                images,
                unused: objects_before.saturating_sub(kept),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standard_fonts;
    use lopdf::dictionary;

    /// A page drawing a 1200 × 1200 RGB picture (noise) in a 144 pt square.
    fn doc() -> PdfDocument {
        let n = 1200_usize;
        let mut px = Vec::with_capacity(n * n * 3);
        let mut s = 7_u32;
        for _ in 0..n * n * 3 {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            px.push((s >> 24) as u8);
        }
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&px, 6);
        let mut d = lopdf::Document::with_version("1.7");
        let mut idict = lopdf::Dictionary::new();
        idict.set("Type", Object::Name(b"XObject".to_vec()));
        idict.set("Subtype", Object::Name(b"Image".to_vec()));
        idict.set("Width", n as i64);
        idict.set("Height", n as i64);
        idict.set("BitsPerComponent", 8);
        idict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
        idict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        let im = d.add_object(Object::Stream(lopdf::Stream::new(idict, z)));
        // An object nothing refers to.
        d.add_object(Object::String(
            vec![b'x'; 5000],
            lopdf::StringFormat::Literal,
        ));
        let content = d.add_object(Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"q 144 0 0 144 10 10 cm /Im1 Do Q".to_vec(),
        )));
        let mut xo = lopdf::Dictionary::new();
        xo.set("Im1", im);
        let mut res = lopdf::Dictionary::new();
        res.set("XObject", xo);
        let pages_id = d.new_object_id();
        let page = d.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 300.into(), 300.into()],
            "Resources" => res,
            "Contents" => content,
        });
        d.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
            }),
        );
        let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        d.trailer.set("Root", cat);
        let mut out = Vec::new();
        d.save_to(&mut out).expect("saves");
        PdfDocument::open(out, standard_fonts()).expect("opens")
    }

    #[test]
    fn pictures_are_downsampled_and_the_file_shrinks() {
        let mut d = doc();
        let (bytes, r) = d
            .optimize(&OptimizeOptions::default(), &|_, _, _, _| None)
            .expect("optimizes");
        // 144 pt = 2 in → 300 px at 150 ppi; flat (Flate) source stays Flate.
        assert_eq!(r.images, 1, "{r:?}");
        assert!(r.after * 4 < r.before, "{r:?}");
        assert!(r.unused >= 1, "{r:?}");
        let mut again = PdfDocument::open(bytes, standard_fonts()).expect("opens");
        let page = again
            .render(0, crate::RenderOptions::default())
            .expect("renders");
        let img = page.images.values().next().expect("picture");
        assert_eq!((img.width, img.height), (300, 300));
        // Undo puts the full picture back.
        assert!(d.undo());
        let page = d
            .render(0, crate::RenderOptions::default())
            .expect("renders");
        assert_eq!(page.images.values().next().expect("picture").width, 1200);
    }

    #[test]
    fn box_filter_averages() {
        let px = [
            0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255,
        ];
        assert_eq!(downsample(&px, 2, 2, 1, 1), vec![127, 127, 127, 255]);
    }
}
