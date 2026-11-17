//! BlueSlide host side of the File screen (round 10 of the slide series): the app's own recent
//! list, saving copies in other types, exports (animated GIF, package, handouts), the file
//! location, and printing with the Print page's settings through the GDI print path.
use bo_engine::slide_export::{HandoutOptions, ImageFormat, ImageOptions, NotesOptions, Paper};
use bo_engine::slide_export::{PackageOptions, SlideExport};
use bo_engine::Engine;
use bo_render::PageList;
use bo_ui::SlidePrintJob;
use std::path::{Path, PathBuf};

fn app_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("BlueOffice"))
}

fn recent_path() -> Option<PathBuf> {
    app_dir().map(|d| d.join("slide-recent.txt"))
}

/// The app's recent presentations (path, date), most recent first.
pub fn read_recent() -> Vec<(String, String)> {
    let Some(text) = recent_path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let (p, d) = l.split_once('\t').unwrap_or((l, ""));
            (!p.is_empty()).then(|| (p.to_owned(), d.to_owned()))
        })
        .collect()
}

/// Put `path` first in the recent list (at most 30 kept) and return the list.
pub fn touch_recent(path: &Path, today: &str) -> Vec<(String, String)> {
    let p = path.to_string_lossy().into_owned();
    let mut list = read_recent();
    list.retain(|(q, _)| !q.eq_ignore_ascii_case(&p));
    list.insert(0, (p, today.to_owned()));
    list.truncate(30);
    if let (Some(dir), Some(file)) = (app_dir(), recent_path()) {
        let _ = std::fs::create_dir_all(dir);
        let text: String = list
            .iter()
            .map(|(p, d)| [p.as_str(), "\t", d, "\n"].concat())
            .collect();
        let _ = std::fs::write(file, text);
    }
    list
}

/// Today as `M/D/YYYY` in local time (`offset` minutes from UTC).
pub fn today(offset: i32) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        + i64::from(offset) * 60;
    civil(secs.div_euclid(86_400))
}

/// `M/D/YYYY` of a day counted from 1970-01-01.
fn civil(days: i64) -> String {
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{m}/{d}/{y}")
}

/// The bytes of the presentation as `ext` (`pdf`, `potx`, `ppsx`, `png` / `jpg` / `svg` of
/// the current slide, `gif` of the show) and the save dialog's filter.
pub fn export_as(
    engine: &mut Engine,
    ext: &str,
    current: usize,
) -> Result<(Vec<u8>, (&'static str, &'static str)), String> {
    let e = |x: bo_engine::EngineError| x.to_string();
    Ok(match ext {
        "pdf" => (engine.export_pdf().map_err(e)?, ("PDF (*.pdf)", "*.pdf")),
        "potx" | "ppsx" => {
            let v = bo_engine::variant::Variant::from_ext(ext).ok_or("unknown type")?;
            let bytes = engine.save_pptx().map_err(e)?;
            let c = bo_engine::variant::convert(&bytes, v).map_err(e)?;
            let filter = if ext == "potx" {
                ("Template (*.potx)", "*.potx")
            } else {
                ("Show (*.ppsx)", "*.ppsx")
            };
            (c.bytes, filter)
        }
        "png" | "jpg" | "svg" => {
            let (format, filter) = match ext {
                "png" => (ImageFormat::Png, ("PNG (*.png)", "*.png")),
                "jpg" => (ImageFormat::Jpg, ("JPEG (*.jpg)", "*.jpg")),
                _ => (ImageFormat::Svg, ("SVG (*.svg)", "*.svg")),
            };
            let opts = SlideExport::Image(ImageOptions {
                format,
                slides: Some(vec![current]),
                width: 1920,
                ..ImageOptions::default()
            });
            (engine.export_slides(&opts).map_err(e)?, filter)
        }
        "gif" => (
            engine
                .export_show_gif(bo_engine::slide_edit::video::VideoOptions {
                    width: 960,
                    fps: 10,
                    ..Default::default()
                })
                .map_err(e)?,
            ("Animated GIF (*.gif)", "*.gif"),
        ),
        "zip" => (
            engine
                .export_slides(&SlideExport::Package(PackageOptions {}))
                .map_err(e)?,
            ("Package (*.zip)", "*.zip"),
        ),
        other => return Err(format!("cannot export as {other}")),
    })
}

/// Handouts PDF with `per` slides per page.
pub fn handouts(engine: &mut Engine, per: u8) -> Result<Vec<u8>, String> {
    engine
        .export_slides(&SlideExport::Handout(HandoutOptions {
            per_page: per,
            paper: Paper::Letter,
            ..HandoutOptions::default()
        }))
        .map_err(|e| e.to_string())
}

/// Show `path` selected in the system file manager.
pub fn open_location(path: &str) {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn();
}

/// The printed pages of `job`: slides, notes pages or handout pages.
pub fn job_pages(engine: &mut Engine, job: &SlidePrintJob) -> Result<Vec<PageList>, String> {
    let e = |x: bo_engine::EngineError| x.to_string();
    let pages = if job.notes {
        engine
            .notes_pages(&NotesOptions {
                paper: Paper::Letter,
                slides: Some(job.slides.clone()),
                ..NotesOptions::default()
            })
            .map_err(e)?
            .pages
    } else if job.per_page > 0 {
        engine
            .handout_pages(&HandoutOptions {
                per_page: job.per_page,
                frame: job.frame,
                paper: Paper::Letter,
                slides: Some(job.slides.clone()),
                ..HandoutOptions::default()
            })
            .map_err(e)?
            .pages
    } else {
        let mut v = Vec::new();
        for &i in &job.slides {
            v.push(engine.slide(i).map_err(e)?.clone());
        }
        v
    };
    Ok(pages)
}

/// The order pages print in: every copy of the set (collated) or each page's copies together.
pub fn sheet_order(pages: usize, copies: u32, collated: bool) -> Vec<usize> {
    let copies = copies.max(1) as usize;
    if collated {
        (0..copies).flat_map(|_| 0..pages).collect()
    } else {
        (0..pages)
            .flat_map(|p| std::iter::repeat_n(p, copies))
            .collect()
    }
}

/// `dest` of a page of `size` points fitted and centred in the printable `area`.
pub fn fit(size: (f32, f32), area: (f32, f32)) -> [f32; 4] {
    let k = (area.0 / size.0.max(1.0)).min(area.1 / size.1.max(1.0));
    let (w, h) = (size.0 * k, size.1 * k);
    [(area.0 - w) / 2.0, (area.1 - h) / 2.0, w, h]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_and_collation() {
        assert_eq!(sheet_order(2, 2, true), vec![0, 1, 0, 1]);
        assert_eq!(sheet_order(2, 2, false), vec![0, 0, 1, 1]);
        let d = fit((960.0, 540.0), (720.0, 540.0));
        assert!((d[2] - 720.0).abs() < 0.01 && (d[1] - 67.5).abs() < 0.01);
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), "1/1/1970");
        assert_eq!(civil(59), "3/1/1970");
        assert_eq!(civil(20_774), "11/17/2026");
    }
}
