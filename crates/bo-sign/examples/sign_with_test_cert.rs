//! Signs a .pdf or OOXML file with a throwaway self-signed certificate:
//! `cargo run -p bo-sign --example sign_with_test_cert -- in.docx out.docx`
use std::time::SystemTime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output] = &args[..] else {
        return Err("usage: sign_with_test_cert <in> <out>".into());
    };
    let signer = bo_sign::testing::self_signed(&mut rand::thread_rng(), "BlueOffice Test");
    let bytes = std::fs::read(input)?;
    let signed = if input.to_ascii_lowercase().ends_with(".pdf") {
        bo_sign::pdf::sign_pdf(&bytes, &signer, &bo_sign::pdf::SignOptions::default())?
    } else {
        bo_sign::ooxml::sign_ooxml(&bytes, &signer, SystemTime::now())?
    };
    let count = if input.to_ascii_lowercase().ends_with(".pdf") {
        bo_sign::pdf::verify_pdf(&signed)?.len()
    } else {
        bo_sign::ooxml::verify_ooxml(&signed)?.len()
    };
    println!("{output}: {count} valid signature(s)");
    std::fs::write(output, signed)?;
    Ok(())
}
