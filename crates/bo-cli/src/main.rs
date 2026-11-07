//! `bo-cli` — headless BlueOffice tool.
//!
//! ```text
//! bo-cli render input.docx --out page-%d.png [--scale 2]
//! bo-cli convert input.docx --to pdf [--out output.pdf]
//! bo-cli dump input.docx [--page 0]          # display list JSON (parity tests)
//! bo-cli pack <dir> <out.docx> | unpack <file> <dir>
//! bo-cli sample <out.docx>                   # generate the Phase 1 corpus sample
//! bo-cli corpus <dir>                         # generate the Phase 2 round-trip corpus
//! bo-cli roundtrip <file.docx> --out <out>    # open and save again (lossless check)
//! bo-cli bench input.docx [--iterations 5]
//! ```
#![forbid(unsafe_code)]

mod corpus;
mod sample;

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
        #[arg(long, value_parser = ["pdf"])]
        to: String,
        #[arg(long)]
        out: Option<PathBuf>,
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
        Cmd::Convert { input, to, out } => {
            let engine = open(&input)?;
            debug_assert_eq!(to, "pdf");
            let pdf = engine.export_pdf()?;
            let out = out.unwrap_or_else(|| input.with_extension("pdf"));
            write_file(&out, &pdf)?;
            println!(
                "{} ({} pages, {} bytes)",
                out.display(),
                engine.info()?.page_count,
                pdf.len()
            );
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
