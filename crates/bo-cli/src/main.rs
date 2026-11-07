//! `bo-cli` — headless BlueOffice tool.
//!
//! ```text
//! bo-cli render input.docx|deck.pptx --out page-%d.png [--scale 2]
//! bo-cli convert input.docx|deck.pptx --to pdf [--out output.pdf]
//! bo-cli convert old.doc --to docx|odt|txt|pdf     # also .odt / .docx input
//! bo-cli convert book.xls --to csv|ods [--sheet 1]  # also .xlsb / .ods input
//! bo-cli convert deck.ppt --to odp|txt              # also .odp input
//! bo-cli convert secret.docx --password pw --to docx --protect newpw
//! bo-cli inspect file                               # format, encryption, macros
//! bo-cli dump input.docx [--page 0]          # display list JSON (parity tests)
//! bo-cli pack <dir> <out.docx> | unpack <file> <dir>
//! bo-cli sample <out.docx>                   # generate the Phase 1 corpus sample
//! bo-cli corpus <dir>                         # generate the Phase 2 round-trip corpus
//! bo-cli roundtrip <file.docx> --out <out>    # open and save again (lossless check)
//! bo-cli fields <file.docx> --out <out.docx>  # update TOC / SEQ / REF / PAGEREF fields
//! bo-cli merge <template.docx> <data.csv|xlsx> --to docx|pdf --out <out>  # mail merge
//! bo-cli sample-slides <dir>                 # generate the BlueSlide corpus (tests/corpus/slide)
//! bo-cli bench input.docx [--iterations 5]
//! ```
#![forbid(unsafe_code)]

mod convert;
mod corpus;
mod sample;
mod slide_samples;

use bo_engine::Engine;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "bo-cli", version, about = "BlueOffice command-line tool")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Render pages to PNG. `%d` in --out is replaced by the 1-based page number.
    Render {
        input: PathBuf,
        #[arg(long, default_value = "page-%d.png")]
        out: String,
        /// Pixels per point (1 = 72 dpi, 1.333 = 96 dpi, 2 = 144 dpi).
        #[arg(long, default_value_t = 2.0)]
        scale: f32,
        /// Only this page (1-based).
        #[arg(long)]
        page: Option<usize>,
    },
    /// Convert to another format.
    Convert {
        input: PathBuf,
        #[arg(long, value_parser = ["pdf", "docx", "odt", "txt", "csv", "ods", "odp", "pptx", "xlsx"])]
        to: String,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Password of an encrypted input.
        #[arg(long)]
        password: Option<String>,
        /// Encrypt a .docx output with this password (Agile Encryption).
        #[arg(long)]
        protect: Option<String>,
        /// Sheet for CSV output (1-based).
        #[arg(long, default_value_t = 1)]
        sheet: usize,
    },
    /// Digitally sign a PDF (PAdES B-B) or .docx/.xlsx/.pptx (ECMA-376 Part 2 §12)
    /// with an RSA key (PEM) and its certificate (PEM or DER).
    Sign {
        input: PathBuf,
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        cert: PathBuf,
        /// Output file (default: `<input>-signed.<ext>`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// PDF only: signer name, reason and location shown by viewers.
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        location: Option<String>,
    },
    /// Verify the digital signatures of a PDF or OOXML file.
    Verify { input: PathBuf },
    /// Report the detected format, encryption and macro presence (macros are never run).
    Inspect {
        input: PathBuf,
        #[arg(long)]
        password: Option<String>,
    },
    /// Print the display list of a page as JSON.
    Dump {
        input: PathBuf,
        #[arg(long, default_value_t = 0)]
        page: usize,
    },
    /// Zip a directory into an OPC package.
    Pack { dir: PathBuf, out: PathBuf },
    /// Extract all parts of a package into a directory.
    Unpack { input: PathBuf, dir: PathBuf },
    /// Write the Phase 1 sample document.
    Sample { out: PathBuf },
    /// Write the Phase 2 feature corpus (prints the README table).
    Corpus { dir: PathBuf },
    /// Open a document and save it again.
    Roundtrip {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Update fields (TOC, SEQ, REF, PAGEREF) and save as .docx.
    Fields {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Mail merge: one section per data record, saved as .docx or PDF.
    Merge {
        template: PathBuf,
        data: PathBuf,
        #[arg(long, value_parser = ["docx", "pdf"], default_value = "docx")]
        to: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Write the BlueSlide corpus decks into a directory.
    SampleSlides { dir: PathBuf },
    /// Measure open + layout + display list time.
    Bench {
        input: PathBuf,
        #[arg(long, default_value_t = 5)]
        iterations: u32,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type R = Result<(), Box<dyn std::error::Error>>;

fn open(input: &Path) -> Result<Engine, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let mut engine = Engine::with_bundled_fonts();
    let name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");
    engine.open(&bytes, name)?;
    Ok(engine)
}

fn run(cli: Cli) -> R {
    match cli.cmd {
        Cmd::Render {
            input,
            out,
            scale,
            page,
        } => {
            let engine = open(&input)?;
            let info = engine.info()?;
            let pages: Vec<usize> = match page {
                Some(p) if p >= 1 && p <= info.page_count => vec![p - 1],
                Some(p) => {
                    return Err(format!("page {p} out of range 1..={}", info.page_count).into())
                }
                None => (0..info.page_count).collect(),
            };
            for i in pages {
                let png = engine.render_png(i, scale)?;
                let path = out.replace("%d", &(i + 1).to_string());
                write_file(Path::new(&path), &png)?;
                println!("{path}");
            }
        }
        Cmd::Convert {
            input,
            to,
            out,
            password,
            protect,
            sheet,
        } => run_convert(
            &input,
            &to,
            out,
            password.as_deref(),
            protect.as_deref(),
            sheet,
        )?,
        Cmd::Sign {
            input,
            key,
            cert,
            out,
            name,
            reason,
            location,
        } => {
            let bytes = std::fs::read(&input)?;
            let signer = bo_sign::SoftwareSigner::from_pem(
                &std::fs::read_to_string(&key)?,
                &std::fs::read(&cert)?,
            )?;
            let is_pdf = bytes.starts_with(b"%PDF");
            let signed = if is_pdf {
                let opts = bo_sign::pdf::SignOptions {
                    name,
                    reason,
                    location,
                    ..bo_sign::pdf::SignOptions::default()
                };
                bo_sign::pdf::sign_pdf(&bytes, &signer, &opts)?
            } else {
                bo_sign::ooxml::sign_ooxml(&bytes, &signer, std::time::SystemTime::now())?
            };
            let out = out.unwrap_or_else(|| {
                let stem = input
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("signed");
                let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("bin");
                input.with_file_name(format!("{stem}-signed.{ext}"))
            });
            std::fs::write(&out, signed)?;
            println!("{}", out.display());
        }
        Cmd::Verify { input } => {
            let bytes = std::fs::read(&input)?;
            if bytes.starts_with(b"%PDF") {
                let sigs = bo_sign::pdf::verify_pdf(&bytes)?;
                for (i, s) in sigs.iter().enumerate() {
                    println!(
                        "signature {}: valid, signer {}{}",
                        i + 1,
                        s.signer.subject,
                        if s.covers_whole_file {
                            ""
                        } else {
                            " (document changed after this signature)"
                        }
                    );
                }
                if sigs.is_empty() {
                    println!("no signatures");
                }
            } else {
                let sigs = bo_sign::ooxml::verify_ooxml(&bytes)?;
                for s in &sigs {
                    println!(
                        "{}: valid, signed {}",
                        s.part,
                        s.signing_time.as_deref().unwrap_or("?")
                    );
                }
                if sigs.is_empty() {
                    println!("no signatures");
                }
            }
        }
        Cmd::Inspect { input, password } => {
            let bytes = std::fs::read(&input).map_err(|e| format!("{}: {e}", input.display()))?;
            let (loaded, facts) = convert::load(&bytes, password.as_deref())?;
            println!("format: {}", facts.format);
            println!("encrypted: {}", facts.encrypted);
            println!("macros: {}", facts.has_macros);
            match loaded {
                convert::Loaded::Document { doc, .. } => {
                    println!("paragraphs: {}", doc.paragraphs().count());
                }
                convert::Loaded::Workbook(wb) => {
                    for s in &wb.sheets {
                        println!("sheet: {} ({} cells)", s.name, s.cells.len());
                    }
                }
                convert::Loaded::Spreadsheet(d) => {
                    for s in &d.sheets {
                        println!("sheet: {} ({} cells)", s.name, s.cells.len());
                    }
                }
                convert::Loaded::Deck(p) => println!("slides: {}", p.slides.len()),
                convert::Loaded::Pptx(b) => {
                    let mut engine = Engine::with_bundled_fonts();
                    println!("slides: {}", engine.open_pptx(&b, "deck.pptx")?.page_count);
                }
                convert::Loaded::Xlsx(b) => {
                    let mut engine = bo_engine::SheetEngine::new();
                    for s in engine.open(&b, "book.xlsx")?.sheets {
                        println!("sheet: {}", s.name);
                    }
                }
            }
        }
        Cmd::Dump { input, page } => {
            let engine = open(&input)?;
            println!("{}", engine.page_json(page)?);
        }
        Cmd::Pack { dir, out } => {
            let mut pkg = bo_opc::Package::new();
            pkg.remove_part("/[Content_Types].xml");
            let mut files = Vec::new();
            collect(&dir, &dir, &mut files)?;
            files.sort();
            for (name, path) in files {
                pkg.put_part(&name, std::fs::read(path)?);
            }
            write_file(&out, &pkg.write()?)?;
        }
        Cmd::Unpack { input, dir } => {
            let pkg = bo_opc::Package::read(&std::fs::read(&input)?, &bo_opc::Limits::default())?;
            for p in pkg.parts() {
                let path = dir.join(p.name.trim_start_matches('/'));
                write_file(&path, &p.data)?;
                println!("{}", path.display());
            }
        }
        Cmd::Sample { out } => {
            write_file(&out, &bo_docx::write_docx(&sample::document())?)?;
            println!("{}", out.display());
        }
        Cmd::Corpus { dir } => {
            print!("{}", corpus::generate(&dir)?);
        }
        Cmd::Roundtrip { input, out } => {
            let engine = open(&input)?;
            write_file(&out, &engine.save_docx()?)?;
            println!("{}", out.display());
        }
        Cmd::Fields { input, out } => {
            let mut engine = open(&input)?;
            let r = engine.command(bo_engine::Command::UpdateFields)?;
            if let Some(e) = r.error {
                return Err(e.into());
            }
            write_file(&out, &engine.save_docx()?)?;
            println!(
                "{} fields updated → {}",
                r.count.unwrap_or(0),
                out.display()
            );
        }
        Cmd::Merge {
            template,
            data,
            to,
            out,
        } => {
            let mut engine = open(&template)?;
            let bytes = std::fs::read(&data).map_err(|e| format!("{}: {e}", data.display()))?;
            let name = data
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("data")
                .to_owned();
            let records = engine.set_merge_data(&bytes, &name)?;
            let r = engine.command(bo_engine::Command::MergeToDocument)?;
            if let Some(e) = r.error {
                return Err(e.into());
            }
            let bytes = if to == "pdf" {
                engine.export_pdf()?
            } else {
                engine.save_docx()?
            };
            write_file(&out, &bytes)?;
            println!("{records} records → {}", out.display());
        }
        Cmd::SampleSlides { dir } => {
            for (name, bytes) in slide_samples::decks() {
                let path = dir.join(name);
                write_file(&path, &bytes)?;
                println!("{}", path.display());
            }
        }
        Cmd::Bench { input, iterations } => {
            let bytes = std::fs::read(&input)?;
            let name = input
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("doc")
                .to_owned();
            let t0 = Instant::now();
            let mut engine = Engine::with_bundled_fonts();
            let t_fonts = t0.elapsed();
            let mut times = Vec::new();
            let mut pages = 0;
            for _ in 0..iterations.max(1) {
                let mut e = Engine::with_bundled_fonts();
                let t = Instant::now();
                pages = e.open(&bytes, &name)?.page_count;
                times.push(t.elapsed());
            }
            let t = Instant::now();
            let pdf = {
                engine.open(&bytes, &name)?;
                engine.export_pdf()?
            };
            let t_pdf = t.elapsed();
            times.sort();
            println!("pages: {pages}");
            println!("font load: {:.1} ms", t_fonts.as_secs_f64() * 1e3);
            println!(
                "open+layout+display list (median of {}): {:.1} ms",
                times.len(),
                times[times.len() / 2].as_secs_f64() * 1e3
            );
            println!(
                "open+export pdf: {:.1} ms ({} bytes)",
                t_pdf.as_secs_f64() * 1e3,
                pdf.len()
            );
        }
    }
    Ok(())
}

fn run_convert(
    input: &Path,
    to: &str,
    out: Option<PathBuf>,
    password: Option<&str>,
    protect: Option<&str>,
    sheet: usize,
) -> R {
    use convert::Loaded;
    let bytes = std::fs::read(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document")
        .to_owned();
    let (loaded, facts) = convert::load(&bytes, password)?;
    if facts.has_macros {
        eprintln!("note: the input contains macros; they are preserved where the output format allows and never executed");
    }
    let mut out_ext = to.to_owned();
    let data: Vec<u8> = match (loaded, to) {
        (Loaded::Document { doc, .. }, "pdf") => {
            let mut engine = Engine::with_bundled_fonts();
            engine.load(doc, &name)?;
            engine.export_pdf()?
        }
        (Loaded::Document { doc, source }, "docx") => {
            let (docx, carried) = convert::document_to_docx(&doc, source.as_ref())?;
            if carried {
                out_ext = "docm".into();
            }
            docx
        }
        (Loaded::Document { doc, .. }, "odt") => bo_odf::write_odt(&doc)?,
        (Loaded::Document { doc, .. }, "txt") => doc.plain_text().into_bytes(),
        (Loaded::Workbook(wb), "csv") => {
            let s = wb
                .sheets
                .get(sheet.saturating_sub(1))
                .ok_or_else(|| format!("sheet {sheet} out of range 1..={}", wb.sheets.len()))?;
            s.to_csv().into_bytes()
        }
        (Loaded::Workbook(wb), "ods") => bo_odf::write_ods(&convert::workbook_to_ods(&wb))?,
        (Loaded::Spreadsheet(d), "csv") => {
            let s = d
                .sheets
                .get(sheet.saturating_sub(1))
                .ok_or_else(|| format!("sheet {sheet} out of range 1..={}", d.sheets.len()))?;
            convert::ods_sheet_to_worksheet(s).to_csv().into_bytes()
        }
        (Loaded::Spreadsheet(d), "ods") => bo_odf::write_ods(&d)?,
        (Loaded::Pptx(b), "pdf") => {
            let mut engine = Engine::with_bundled_fonts();
            engine.open_pptx(&b, &name)?;
            engine.export_pdf()?
        }
        (Loaded::Pptx(b), "pptx") => {
            let mut engine = Engine::with_bundled_fonts();
            engine.open_pptx(&b, &name)?;
            engine.save_pptx()?
        }
        // Spreadsheets through the BlueSheet engine (xls/xlsb/ods are converted).
        (Loaded::Xlsx(b), "xlsx" | "csv" | "pdf") => sheet_convert(&b, &name, to, sheet)?,
        (Loaded::Xlsx(b), "ods") => {
            let mut engine = bo_engine::SheetEngine::new();
            engine.open(&b, &name)?;
            let wb = engine.workbook().ok_or("no workbook")?;
            bo_odf::write_ods(&convert::model_to_ods(wb))?
        }
        (Loaded::Workbook(_) | Loaded::Spreadsheet(_), "xlsx" | "pdf") => {
            sheet_convert(&bytes, &name, to, sheet)?
        }
        (Loaded::Deck(p), "odp") => bo_odf::write_odp(&p)?,
        (Loaded::Deck(p), "txt") => convert::presentation_text(&p).into_bytes(),
        (_, other) => {
            return Err(format!("cannot convert {} to {other}", facts.format).into());
        }
    };
    let data = if let Some(pw) = protect {
        if !matches!(to, "docx") {
            return Err("--protect is only supported for docx output".into());
        }
        bo_crypto::save_package(&data, Some(pw))?
    } else {
        data
    };
    let out = out.unwrap_or_else(|| input.with_extension(&out_ext));
    if out == input {
        return Err("output would overwrite the input; pass --out".into());
    }
    write_file(&out, &data)?;
    println!("{} ({} bytes)", out.display(), data.len());
    Ok(())
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            collect(root, &p, out)?;
        } else {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((format!("/{rel}"), p));
        }
    }
    Ok(())
}

fn write_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, data)
}

/// Spreadsheet output via `SheetEngine` (`sheet` is 1-based for CSV).
fn sheet_convert(
    bytes: &[u8],
    name: &str,
    to: &str,
    sheet: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut engine = bo_engine::SheetEngine::with_bundled_fonts();
    engine.open(bytes, name)?;
    Ok(match to {
        "pdf" => engine.export_pdf()?,
        fmt => engine.save(fmt, sheet.saturating_sub(1))?,
    })
}
