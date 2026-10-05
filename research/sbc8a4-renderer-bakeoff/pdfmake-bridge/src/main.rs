use std::env;
use std::io::{self, Read};
use std::path::PathBuf;

use sbc8a4_pdfmake_rust_bridge_proof::{MAX_PDF_BYTES, receipt_json, store_pdf_bytes};

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        fail("USAGE: bridge <storage-root> <filename>");
    }

    let root = PathBuf::from(&args[1]);
    let mut bytes = Vec::new();
    io::stdin()
        .take(MAX_PDF_BYTES + 1)
        .read_to_end(&mut bytes)
        .unwrap_or_else(|_| fail("STDIN_READ_FAILED"));

    match store_pdf_bytes(&root, &args[2], &bytes) {
        Ok(receipt) => println!("{}", receipt_json(&receipt)),
        Err(message) => fail(message),
    }
}
