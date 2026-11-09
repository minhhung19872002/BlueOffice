//! `bo-cli` — headless BlueOffice tool.
//!
//! ```text
//! bo-cli render input.docx|deck.pptx --out page-%d.png [--scale 2]
//! bo-cli convert input.docx|deck.pptx --to pdf [--out output.pdf]
//! bo-cli convert old.doc --to docx|odt|txt|pdf     # also .odt / .docx input
//! bo-cli convert book.xls --to csv|ods [--sheet 1]  # also .xlsb / .ods input
//! bo-cli convert deck.ppt --to odp|txt              # also .odp input
//! bo-cli convert deck.pptx --to gif                 # slide show as an animated GIF
//! bo-cli convert deck.pptx --to pdf --handout 6 [--vertical] [--letter] | --notes-pages
//! bo-cli convert deck.pptx --to png|jpg|svg [--width 1280] [--slide 1]   # zip of all slides without --slide
//! bo-cli convert deck.pptx --to zip                  # presentation package (deck, media, OFL fonts)
//! bo-cli convert in.docx|deck.pptx|book.xlsx --to pdf --pages "1-3, 5" [--per-sheet 4] [--paper a4] [--copies 2] [--print-options JSON]
//! bo-cli convert secret.docx --password pw --to docx --protect newpw
//! bo-cli inspect file                               # format, encryption, macros
//! bo-cli sign in.pdf --p12 me.p12 [--tsa URL] | --key k.pem --cert c.pem | --remote URL
//! bo-cli ltv in.pdf --trust root-ca.pem [--crl-fetch --ocsp-fetch] [--tsa URL] [--out out.pdf]
//! bo-cli verify in.pdf [--trust root-ca.pem] [--crl ca.crl] [--ocsp resp.der | --ocsp-fetch] [--crl-fetch] [--json]
//! bo-cli dump input.docx [--page 0]          # display list JSON (parity tests)
//! bo-cli pack <dir> <out.docx> | unpack <file> <dir>
//! bo-cli sample <out.docx>                   # generate the Phase 1 corpus sample
//! bo-cli corpus <dir>                         # generate the Phase 2 round-trip corpus
//! bo-cli roundtrip <file.docx> --out <out>    # open and save again (lossless check)
//! bo-cli fields <file.docx> --out <out.docx>  # update TOC / SEQ / REF / PAGEREF fields
//! bo-cli merge <template.docx> <data.csv|xlsx|vcf|json> --to docx|pdf|zip --out <out> [--pattern "«Field»"]  # mail merge
//! bo-cli sample-slides <dir>                 # generate the BlueSlide corpus (tests/corpus/slide)
//! bo-cli templates <dir>                     # write the BlueDoc templates + thumbnails (assets/templates/doc)
//! bo-cli sample-big <out.docx> [--pages 300]  # §5 benchmark document (text + pictures)
//! bo-cli bench input.docx [--iterations 5]
//! bo-cli validate <file-or-dir>... [--json] [--openxml auto|require|off] [--validator PATH]
//! bo-cli fidelity-report [--no-validate] [--corpus DIR] [--report FILE] [--history FILE]
//! ```
#![forbid(unsafe_code)]

mod convert;
mod corpus;
mod fidelity;
mod sample;
mod sign_cmd;
mod slide_edits;
mod slide_samples;
mod templates;
mod validate;

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
#[allow(clippy::large_enum_variant)] // parsed once per run
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
    /// Convert to another format. Templates / macro-enabled variants / slide shows
    /// (`dotx`, `docm`, `xltm`, `ppsx`, …) re-type the package (macros are dropped for
    /// macro-free types); `thmx` saves the file's theme as a theme file.
    Convert {
        input: PathBuf,
        #[arg(long, value_parser = ["pdf", "docx", "odt", "txt", "csv", "ods", "odp", "pptx", "xlsx", "gif", "png", "jpg", "svg", "zip",
            "docm", "dotx", "dotm", "xlsm", "xltx", "xltm", "pptm", "potx", "potm", "ppsx", "ppsm", "thmx"])]
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
        #[command(flatten)]
        deck: DeckExportArgs,
    },
    /// Digitally sign a PDF (PAdES B-B/B-T) or .docx/.xlsx/.pptx (ECMA-376 Part 2 §12,
    /// XAdES) with a PEM key + certificate, a .p12 file or a remote signing service.
    Sign {
        input: PathBuf,
        /// RSA private key (PEM); needs --cert.
        #[arg(long)]
        key: Option<PathBuf>,
        /// Certificate (PEM or DER); extra PEM blocks are the intermediate chain.
        #[arg(long)]
        cert: Option<PathBuf>,
        /// PKCS#12 file (.p12/.pfx) with key and chain.
        #[arg(long)]
        p12: Option<PathBuf>,
        /// PKCS#12 password (default: env BO_P12_PASSWORD).
        #[arg(long)]
        password: Option<String>,
        /// Remote signing service base URL (docs/deploy/remote-signing.md).
        #[arg(long)]
        remote: Option<String>,
        #[arg(long)]
        remote_credential: Option<String>,
        /// Bearer token (default: env BO_REMOTE_TOKEN).
        #[arg(long)]
        remote_token: Option<String>,
        /// RFC 3161 time-stamp authority URL (default: env BO_TSA_URL).
        #[arg(long)]
        tsa: Option<String>,
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
    /// Verify the digital signatures of a PDF or OOXML file: integrity,
    /// time-stamp and certificate path to the trust anchors.
    Verify {
        input: PathBuf,
        /// Trust anchor file (PEM bundle / DER) or directory; repeatable
        /// (default: env BO_TRUST_ANCHORS).
        #[arg(long)]
        trust: Vec<PathBuf>,
        #[command(flatten)]
        rev: sign_cmd::Revocation,
        /// Machine-readable JSON report.
        #[arg(long)]
        json: bool,
    },
    /// Add long-term validation data (PAdES B-LT DSS: certificate paths, CRLs,
    /// OCSP responses) to a signed PDF so it can be verified later offline.
    Ltv {
        input: PathBuf,
        /// Trust anchor file or directory; repeatable (default: env BO_TRUST_ANCHORS).
        #[arg(long)]
        trust: Vec<PathBuf>,
        #[command(flatten)]
        rev: sign_cmd::Revocation,
        /// Also add a document time-stamp from this RFC 3161 TSA (PAdES B-LTA).
        #[arg(long)]
        tsa: Option<String>,
        /// Output file (default: `<input>-ltv.pdf`).
        #[arg(long)]
        out: Option<PathBuf>,
    },
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
    /// Open a document, workbook or deck and save it again in its format.
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
        /// `zip`: one PDF per record, named by `--pattern`.
        #[arg(long, value_parser = ["docx", "pdf", "zip"], default_value = "docx")]
        to: String,
        #[arg(long)]
        out: PathBuf,
        /// File name pattern of `--to zip` (`«Field»` = the record's value).
        #[arg(long, default_value = "")]
        pattern: String,
    },
    /// Write the BlueSlide corpus decks into a directory.
    SampleSlides { dir: PathBuf },
    /// Write the built-in BlueDoc templates (.docx) and their thumbnails (.png).
    Templates { dir: PathBuf },
    /// Write the §5 benchmark document: about N A4 pages of text with inline pictures.
    SampleBig {
        out: PathBuf,
        #[arg(long, default_value_t = 300)]
        pages: usize,
        /// One picture after every N paragraphs (6 = about one picture per page).
        #[arg(long, default_value_t = 6)]
        image_every: usize,
        /// Number of different image parts the pictures cycle through.
        #[arg(long, default_value_t = 40)]
        distinct: usize,
    },
    /// Write the §5 benchmark workbook: 1 000 000 cells, 100 000 of them formulas.
    SampleBigSheet { out: PathBuf },
    /// Measure open + layout + display list time (.xlsx: open + calculate, edit one cell).
    Bench {
        input: PathBuf,
        #[arg(long, default_value_t = 5)]
        iterations: u32,
    },
    /// Validate OOXML packages: built-in OPC structure checks (content types,
    /// relationships, XML well-formedness, main part) plus OpenXmlValidator
    /// (tools/BlueOffice.Validate) when .NET is available. Exit code 1 on errors.
    Validate {
        /// Files or directories (searched recursively for .docx/.xlsx/.pptx/…).
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Print a JSON report instead of text.
        #[arg(long)]
        json: bool,
        /// OpenXmlValidator stage: `auto` runs it when found, `require` fails without it.
        #[arg(long, default_value = "auto", value_parser = ["auto", "require", "off"])]
        openxml: String,
        /// Validator to run: project directory, .csproj, .dll or executable
        /// (default: env BO_VALIDATE, else tools/BlueOffice.Validate of the repository).
        #[arg(long)]
        validator: Option<PathBuf>,
        /// OpenXmlValidator file format version (e.g. Office2019, Office2007).
        #[arg(long)]
        format: Option<String>,
        /// Also validate the mc:Choice branch of every mc:AlternateContent (the validator checks
        /// the Fallback only).
        #[arg(long)]
        choice: bool,
    },
    /// Round-trip the corpus and validate it (scripts/fidelity-report.mjs, run with
    /// this bo-cli): writes docs/fidelity-report.md and tests/fidelity/history.csv.
    FidelityReport {
        /// Skip OpenXmlValidator (conversion only).
        #[arg(long)]
        no_validate: bool,
        /// Corpus root with doc/, sheet/ and slide/ (default: tests/corpus).
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// Markdown report (default: docs/fidelity-report.md).
        #[arg(long)]
        report: Option<PathBuf>,
        /// History CSV appended to (default: tests/fidelity/history.csv).
        #[arg(long)]
        history: Option<PathBuf>,
        /// Directory of the round-tripped files (default: tests/fidelity/output/roundtrip).
        #[arg(long)]
        work: Option<PathBuf>,
        /// The script to run (default: scripts/fidelity-report.mjs of the repository).
        #[arg(long)]
        script: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // The recursive parsers need more than the 1 MiB main-thread stack of Windows in
    // unoptimised builds: run the command on a thread with a roomy stack.
    let worker = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || run(cli).map_err(|e| e.to_string()));
    let result = match worker {
        Ok(h) => h.join().unwrap_or_else(|_| Err("internal error".into())),
        Err(e) => Err(e.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type R = Result<(), Box<dyn std::error::Error>>;

/// Slide deck exports of `convert` (SLIDE-EXPORT-002).
#[derive(clap::Args, Debug, Default)]
struct DeckExportArgs {
    /// Slide decks, --to pdf: handouts with N slides per page (1, 2, 3, 4, 6 or 9).
    #[arg(long)]
    handout: Option<u8>,
    /// Handouts: order the slides down each column first.
    #[arg(long)]
    vertical: bool,
    /// Handouts / notes pages: US Letter paper instead of A4.
    #[arg(long)]
    letter: bool,
    /// Slide decks, --to pdf: notes pages (slide and speaker notes).
    #[arg(long)]
    notes_pages: bool,
    /// Slide decks, --to png|jpg|svg: picture width in pixels.
    #[arg(long, default_value_t = 1280)]
    width: u32,
    /// Slide decks, --to png|jpg|svg: only this slide (1-based) instead of a zip of all.
    #[arg(long)]
    slide: Option<usize>,
    #[command(flatten)]
    print: PrintArgs,
}

/// Print job options of `convert --to pdf` (CORE-PRINT-001, `bo_engine::print`).
#[derive(clap::Args, Debug, Default)]
struct PrintArgs {
    /// Pages to print: `1-3, 5, 8-` (BlueDoc also `p2s1`: page 2 of section 1).
    #[arg(long)]
    pages: Option<String>,
    /// Pages per sheet: 1, 2, 4, 6, 8 or 16.
    #[arg(long)]
    per_sheet: Option<u8>,
    /// Scale to paper: a3, a4, a5, b5, letter, legal.
    #[arg(long, value_parser = ["a3", "a4", "a5", "b5", "letter", "legal"])]
    paper: Option<String>,
    /// Copies (collated).
    #[arg(long)]
    copies: Option<u32>,
    /// Full print job as JSON (`bo_engine::print::PrintOptions`); the flags above override it.
    #[arg(long)]
    print_options: Option<String>,
}

impl PrintArgs {
    /// The print job JSON, `None` when no print option is given.
    fn json(&self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        if self.pages.is_none()
            && self.per_sheet.is_none()
            && self.paper.is_none()
            && self.copies.is_none()
            && self.print_options.is_none()
        {
            return Ok(None);
        }
        let mut v: serde_json::Value = match &self.print_options {
            Some(s) => serde_json::from_str(s)?,
            None => serde_json::json!({}),
        };
        let o = v
            .as_object_mut()
            .ok_or("--print-options must be a JSON object")?;
        if let Some(p) = &self.pages {
            o.insert("pages".into(), "range".into());
            o.insert("range".into(), p.clone().into());
        }
        if let Some(n) = self.per_sheet {
            o.insert("perSheet".into(), n.into());
        }
        if let Some(p) = &self.paper {
            o.insert("paper".into(), p.clone().into());
        }
        if let Some(c) = self.copies {
            o.insert("copies".into(), c.into());
        }
        Ok(Some(v.to_string()))
    }
}

/// Exported bytes and the output file extension.
type Exported = Option<(Vec<u8>, String)>;

/// Deck export through the BlueSlide engine: `None` for a plain conversion.
fn deck_export(
    engine: &mut Engine,
    to: &str,
    a: &DeckExportArgs,
) -> Result<Exported, Box<dyn std::error::Error>> {
    use bo_engine::slide_export::{
        handout::Order, HandoutOptions, ImageFormat, ImageOptions, NotesOptions, Paper, SlideExport,
    };
    let paper = if a.letter { Paper::Letter } else { Paper::A4 };
    let opts = match to {
        "pdf" if a.notes_pages => SlideExport::Notes(NotesOptions {
            paper,
            ..NotesOptions::default()
        }),
        "pdf" => match a.handout {
            Some(per_page) => SlideExport::Handout(HandoutOptions {
                per_page,
                order: if a.vertical {
                    Order::Vertical
                } else {
                    Order::Horizontal
                },
                paper,
                ..HandoutOptions::default()
            }),
            None => return Ok(None),
        },
        "png" | "jpg" | "svg" => SlideExport::Image(ImageOptions {
            format: match to {
                "png" => ImageFormat::Png,
                "jpg" => ImageFormat::Jpg,
                _ => ImageFormat::Svg,
            },
            slides: a.slide.map(|n| vec![n.saturating_sub(1)]),
            width: a.width,
            ..ImageOptions::default()
        }),
        "zip" => SlideExport::Package,
        _ => return Ok(None),
    };
    let one = a.slide.is_some() || engine.slide_count() == 1;
    let ext = match to {
        "png" | "jpg" | "svg" if !one => "zip",
        other => other,
    };
    Ok(Some((engine.export_slides(&opts)?, ext.to_owned())))
}

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
        Cmd::Validate {
            inputs,
            json,
            openxml,
            validator,
            format,
            choice,
        } => validate::run(&validate::Options {
            inputs,
            openxml: validate::OpenXmlMode::parse(&openxml).ok_or("bad --openxml")?,
            validator,
            file_format: format,
            choice,
            json,
        })?,
        Cmd::FidelityReport {
            no_validate,
            corpus,
            report,
            history,
            work,
            script,
        } => fidelity::run(&fidelity::Options {
            no_validate,
            corpus,
            report,
            history,
            work,
            script,
        })?,
        Cmd::Render {
            input,
            out,
            scale,
            page,
        } => {
            let lower = input.to_string_lossy().to_ascii_lowercase();
            if [".xlsx", ".xlsm", ".xlsb", ".xls", ".ods", ".csv"]
                .iter()
                .any(|e| lower.ends_with(e))
            {
                // Spreadsheets: printed pages (page setup, print area/titles).
                let mut sheet = bo_engine::SheetEngine::with_bundled_fonts();
                sheet.open(&std::fs::read(&input)?, &lower)?;
                let count = sheet.print_page_count()?;
                let pages: Vec<usize> = match page {
                    Some(p) if p >= 1 && p <= count => vec![p - 1],
                    Some(p) => return Err(format!("page {p} out of range 1..={count}").into()),
                    None => (0..count).collect(),
                };
                for i in pages {
                    let png = sheet.render_print_png(i, scale)?;
                    let path = out.replace("%d", &(i + 1).to_string());
                    write_file(Path::new(&path), &png)?;
                    println!("{path}");
                }
                return Ok(());
            }
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
            deck,
        } => run_convert(
            &input,
            &to,
            out,
            password.as_deref(),
            protect.as_deref(),
            sheet,
            &deck,
        )?,
        Cmd::Sign {
            input,
            key,
            cert,
            p12,
            password,
            remote,
            remote_credential,
            remote_token,
            tsa,
            out,
            name,
            reason,
            location,
        } => {
            let bytes = std::fs::read(&input)?;
            let file_name = input
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("document");
            let signer = sign_cmd::signer(
                &sign_cmd::SignerArgs {
                    key,
                    cert,
                    p12,
                    password,
                    remote,
                    remote_credential,
                    remote_token,
                },
                file_name,
            )?;
            let opts = bo_sign::pdf::SignOptions {
                name,
                reason,
                location,
                ..bo_sign::pdf::SignOptions::default()
            };
            let signed = sign_cmd::sign(&bytes, signer.as_ref(), &opts, tsa.as_deref())?;
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
        Cmd::Verify {
            input,
            trust,
            rev,
            json,
        } => {
            if !sign_cmd::verify(&input, &trust, &rev, json)? {
                return Err("signature verification failed".into());
            }
        }
        Cmd::Ltv {
            input,
            trust,
            rev,
            tsa,
            out,
        } => {
            let out = out.unwrap_or_else(|| {
                let stem = input
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("signed");
                input.with_file_name(format!("{stem}-ltv.pdf"))
            });
            let summary = sign_cmd::ltv(&input, &trust, &rev, tsa.as_deref(), &out)?;
            println!("{}: {summary}", out.display());
        }
        Cmd::Inspect { input, password } => {
            let bytes = std::fs::read(&input).map_err(|e| format!("{}: {e}", input.display()))?;
            let (loaded, facts) = convert::load(&bytes, password.as_deref())?;
            println!("format: {}", facts.format);
            // CORE-FMT-001: document / template / macro-enabled / slide show, by the
            // main part's content type.
            if let Some(v) = bo_engine::variant::detect(&bytes) {
                println!("variant: {}", v.ext());
            }
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
            // Workbooks and decks are saved in their own format (the `convert` path); documents
            // through the document engine.
            let ext = input
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            let app =
                bo_engine::variant::Variant::from_ext(&ext).map(bo_engine::variant::Variant::app);
            if matches!(app, Some("sheet" | "slide")) {
                // The writers keep the package variant (template, macros, slide show).
                let to = if app == Some("sheet") { "xlsx" } else { "pptx" };
                run_convert(
                    &input,
                    to,
                    Some(out),
                    None,
                    None,
                    0,
                    &DeckExportArgs::default(),
                )?;
            } else {
                let engine = open(&input)?;
                write_file(&out, &engine.save_docx()?)?;
                println!("{}", out.display());
            }
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
            pattern,
        } => {
            let mut engine = open(&template)?;
            let bytes = std::fs::read(&data).map_err(|e| format!("{}: {e}", data.display()))?;
            let name = data
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("data")
                .to_owned();
            let records = engine.set_merge_data(&bytes, &name)?;
            if to == "zip" {
                let r = engine.command(bo_engine::Command::MergeToPdfs { pattern })?;
                if let Some(e) = r.error {
                    return Err(e.into());
                }
                let out_files = r.merge_out.and_then(|m| m.zip).ok_or("no output")?;
                let zip = bo_engine::merge::base64_decode(&out_files).ok_or("base64")?;
                write_file(&out, &zip)?;
                println!("{records} records → {}", out.display());
                return Ok(());
            }
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
        Cmd::SampleBig {
            out,
            pages,
            image_every,
            distinct,
        } => {
            write_file(&out, &corpus::big_document(pages, image_every, distinct)?)?;
            println!("{}", out.display());
        }
        Cmd::SampleBigSheet { out } => {
            write_file(&out, &big_sheet()?)?;
            println!("{}", out.display());
        }
        Cmd::SampleSlides { dir } => {
            for (name, bytes) in slide_samples::decks() {
                let path = dir.join(name);
                write_file(&path, &bytes)?;
                println!("{}", path.display());
            }
        }
        Cmd::Templates { dir } => {
            for (name, bytes) in templates::files()? {
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
            if name.to_ascii_lowercase().ends_with(".xlsx") {
                return bench_sheet(&bytes, &name, iterations);
            }
            let t0 = Instant::now();
            let mut engine = Engine::with_bundled_fonts();
            let t_fonts = t0.elapsed();
            let mut times = Vec::new();
            let mut first = Vec::new();
            let mut pages = 0;
            for _ in 0..iterations.max(1) {
                let mut e = Engine::with_bundled_fonts();
                let t = Instant::now();
                pages = e.open(&bytes, &name)?.page_count;
                times.push(t.elapsed());
                // What the viewer needs before showing anything: the first page.
                std::hint::black_box(e.page_json(0)?);
                first.push(t.elapsed());
            }
            let t = Instant::now();
            let mut json_bytes = 0;
            {
                let mut e = Engine::with_bundled_fonts();
                e.open(&bytes, &name)?;
                for i in 0..pages {
                    json_bytes += e.page_json(i)?.len();
                }
            }
            let t_all = t.elapsed();
            first.sort();
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
                "open+layout+first page display list (median): {:.1} ms",
                first[first.len() / 2].as_secs_f64() * 1e3
            );
            println!(
                "open+display lists of all {pages} pages: {:.1} ms ({json_bytes} bytes JSON)",
                t_all.as_secs_f64() * 1e3
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
    deck: &DeckExportArgs,
) -> R {
    use convert::Loaded;
    let bytes = std::fs::read(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document")
        .to_owned();
    let lower = name.to_ascii_lowercase();
    if to == "pdf" && (lower.ends_with(".txt") || lower.ends_with(".md")) {
        // Plain text is not a package: the engine reads it as paragraphs.
        let data = open(input)?.export_pdf()?;
        let out = out.unwrap_or_else(|| input.with_extension("pdf"));
        write_file(&out, &data)?;
        println!("{} ({} bytes)", out.display(), data.len());
        return Ok(());
    }
    // CORE-FMT-001: a theme file from the input's theme part.
    if to == "thmx" {
        let theme = bo_engine::theme_file::theme_xml_of(&bytes)
            .ok_or("the input has no theme part (§14.2.7)")?;
        let data = bo_engine::variant::thmx_package(&theme)?;
        let out = out.unwrap_or_else(|| input.with_extension("thmx"));
        write_file(&out, &data)?;
        println!("{} ({} bytes)", out.display(), data.len());
        return Ok(());
    }
    // CORE-FMT-001: other variants are written as the base type, then re-typed.
    let variant = bo_engine::variant::Variant::from_ext(to)
        .filter(|_| !matches!(to, "docx" | "xlsx" | "pptx"));
    let to = match variant.map(bo_engine::variant::Variant::app) {
        Some("doc") => "docx",
        Some("sheet") => "xlsx",
        Some(_) => "pptx",
        None => to,
    };
    let (loaded, facts) = convert::load(&bytes, password)?;
    if facts.has_macros {
        eprintln!("note: the input contains macros; they are preserved where the output format allows and never executed");
    }
    let mut out_ext = to.to_owned();
    let job = if to == "pdf" {
        deck.print.json()?
    } else {
        None
    };
    let data: Vec<u8> = match (loaded, to) {
        (Loaded::Document { doc, .. }, "pdf") => {
            let mut engine = Engine::with_bundled_fonts();
            engine.load(doc, &name)?;
            match &job {
                Some(j) => engine.print_pdf_json(j)?,
                None => engine.export_pdf()?,
            }
        }
        // A print job on a deck or a workbook (pages, pages per sheet, copies …).
        (Loaded::Pptx(b), "pdf") if job.is_some() => {
            let mut engine = Engine::with_bundled_fonts();
            engine.open_pptx(&b, &name)?;
            engine.print_pdf_json(job.as_deref().unwrap_or("{}"))?
        }
        (Loaded::Deck(_), "pdf") if job.is_some() => {
            let mut engine = Engine::with_bundled_fonts();
            engine.open(&bytes, &name)?;
            engine.print_pdf_json(job.as_deref().unwrap_or("{}"))?
        }
        (Loaded::Xlsx(b), "pdf") if job.is_some() => {
            let mut engine = bo_engine::SheetEngine::with_bundled_fonts();
            engine.open(&b, &name)?;
            engine.print_pdf_json(job.as_deref().unwrap_or("{}"))?
        }
        (Loaded::Workbook(_) | Loaded::Spreadsheet(_), "pdf") if job.is_some() => {
            let mut engine = bo_engine::SheetEngine::with_bundled_fonts();
            engine.open(&bytes, &name)?;
            engine.print_pdf_json(job.as_deref().unwrap_or("{}"))?
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
        (Loaded::Pptx(b), "pdf" | "png" | "jpg" | "svg" | "zip") => {
            let mut engine = Engine::with_bundled_fonts();
            engine.open_pptx(&b, &name)?;
            match deck_export(&mut engine, to, deck)? {
                Some((data, ext)) => {
                    out_ext = ext;
                    data
                }
                None => engine.export_pdf()?,
            }
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
        // .ppt / .odp decks with their drawing, through the BlueSlide engine.
        (Loaded::Deck(_), "pptx" | "pdf" | "png" | "jpg" | "svg" | "zip") => {
            let mut engine = Engine::with_bundled_fonts();
            engine.open(&bytes, &name)?;
            match deck_export(&mut engine, to, deck)? {
                Some((data, ext)) => {
                    out_ext = ext;
                    data
                }
                None if to == "pdf" => engine.export_pdf()?,
                None => engine.save_pptx()?,
            }
        }
        (Loaded::Deck(p), "odp") => bo_odf::write_odp(&p)?,
        // The slide show as an animated GIF (720 px wide, 10 frames per second).
        (Loaded::Pptx(b), "gif") => {
            let mut engine = Engine::with_bundled_fonts();
            engine.open_pptx(&b, &name)?;
            show_gif(&mut engine)?
        }
        (Loaded::Deck(_), "gif") => {
            let mut engine = Engine::with_bundled_fonts();
            engine.open(&bytes, &name)?;
            show_gif(&mut engine)?
        }
        (Loaded::Deck(p), "txt") => convert::presentation_text(&p).into_bytes(),
        (_, other) => {
            return Err(format!("cannot convert {} to {other}", facts.format).into());
        }
    };
    let data = match variant {
        Some(v) => {
            let c = bo_engine::variant::convert(&data, v)?;
            if c.macros_dropped {
                eprintln!(
                    "warning: .{} cannot hold macros; the VBA project was removed",
                    v.ext()
                );
            }
            out_ext = v.ext().to_owned();
            c.bytes
        }
        None => data,
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

/// The §5 workbook ("mở .xlsx 1 triệu ô có 100.000 công thức"): 10 000 rows × 100
/// columns, 90 columns of constants and 10 columns of formulas (arithmetic,
/// SUM/AVERAGE/MAX, IF, COUNTIF, an exact VLOOKUP into a 1 000-row table, a running
/// total down the sheet) — the layout of `crates/bo-formula/benches/recalc.rs`, saved
/// as a real .xlsx so that the benchmark includes reading the file.
fn big_sheet() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const ROWS: u32 = 10_000;
    const CONSTS: u32 = 90;
    let value = |r: u32, c: u32| {
        let h =
            (r.wrapping_mul(2_654_435_761) ^ c.wrapping_mul(40_503)).wrapping_mul(2_246_822_519);
        h % 1000
    };
    let mut e = bo_engine::SheetEngine::new();
    e.new_workbook("bench.xlsx")?;
    let mut cells = Vec::with_capacity(1_000_000);
    for r in 0..ROWS {
        let n = r + 1;
        for c in 0..CONSTS {
            cells.push((r, c, value(r, c).to_string()));
        }
        let formulas = [
            format!("=A{n}*2+B{n}"),
            format!("=SUM(A{n}:J{n})"),
            format!("=CM{n}+CN{n}"),
            format!("=IF(C{n}>500,D{n},E{n})"),
            format!("=ROUND(AVERAGE(F{n}:O{n}),2)"),
            if n == 1 {
                "=CO1".to_owned()
            } else {
                format!("=CR{}+CO{n}", n - 1)
            },
            format!("=VLOOKUP({},$A$1:$B$1000,2,FALSE)", value(r, 0)),
            format!("=MAX(P{n}:Y{n})"),
            format!("=COUNTIF(Z{n}:AI{n},\">500\")"),
            format!("=CP{n}*1.1+CS{n}"),
        ];
        for (k, f) in formulas.into_iter().enumerate() {
            cells.push((r, CONSTS + k as u32, f));
        }
    }
    e.set_inputs(0, &cells)?;
    Ok(e.save("xlsx", 0)?)
}

/// `bench` for a workbook: open (read + calculate everything), then edit one input
/// read by the VLOOKUP table (10 000 dependents) and one cell of a row.
fn bench_sheet(bytes: &[u8], name: &str, iterations: u32) -> R {
    let mut opens = Vec::new();
    let mut e = bo_engine::SheetEngine::new();
    for _ in 0..iterations.max(1) {
        let mut fresh = bo_engine::SheetEngine::new();
        let t = Instant::now();
        fresh.open(bytes, name)?;
        opens.push(t.elapsed());
        e = fresh;
    }
    opens.sort();
    if bytes.starts_with(b"PK\x03\x04") {
        // Phase breakdown of the open: parse the package, then load + calculate.
        let t = Instant::now();
        let mut wb = bo_xlsx::read_xlsx(bytes)?;
        let read = t.elapsed();
        let mut full = wb.clone();
        let t = Instant::now();
        bo_sheet::live::LiveCalc::new().rebuild_from_file(&mut wb);
        let cached = t.elapsed();
        let t = Instant::now();
        bo_sheet::live::LiveCalc::new().rebuild(&mut full);
        println!(
            "  read_xlsx: {:.1} ms, load keeping cached results: {:.1} ms, full recalculation: {:.1} ms",
            read.as_secs_f64() * 1e3,
            cached.as_secs_f64() * 1e3,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
    let info = e.info()?;
    println!("sheets: {}", info.sheets.len());
    // Sanity: formulas were calculated on open (first formula cell and the last
    // cell of the running total).
    for (row, col) in [(0, 90), (9999, 95)] {
        let c = e.cell(0, row, col)?;
        println!("{} {} = {}", c.address, c.input, c.display);
    }
    println!(
        "open+calculate (median of {}): {:.1} ms",
        opens.len(),
        opens[opens.len() / 2].as_secs_f64() * 1e3
    );
    for (what, row, col) in [
        ("edit lookup table cell (10 000 dependents)", 10, 1),
        ("edit one row input", 5000, 3),
    ] {
        let mut times = Vec::new();
        for k in 0..iterations.max(1) {
            let t = Instant::now();
            e.set_input(0, row, col, &(k + 1).to_string())?;
            times.push(t.elapsed());
        }
        times.sort();
        println!(
            "{what} (median): {:.1} ms",
            times[times.len() / 2].as_secs_f64() * 1e3
        );
    }
    Ok(())
}

/// The open presentation's slide show as an animated GIF (720 px wide, 10 frames per second).
fn show_gif(engine: &mut Engine) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(
        engine.export_show_gif(bo_engine::slide_edit::video::VideoOptions {
            width: 720,
            fps: 10,
            ..Default::default()
        })?,
    )
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
