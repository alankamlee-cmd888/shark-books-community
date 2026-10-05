use std::fs;
use std::path::PathBuf;

use sbc8a4_pdfmake_rust_bridge_proof::{receipt_json, store_pdf_bytes};

const EXPECTED_SHA256: &str = "521d8d37297e1fed6c1367a4edcec413cb60c3b62d6ad058edfb09aa53fbc990";
const EXPECTED_BYTES: usize = 35287;

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

fn main() {
    let exe = std::env::current_exe().unwrap_or_else(|_| fail("CURRENT_EXE_FAILED"));
    let bundle = exe
        .parent()
        .unwrap_or_else(|| fail("BUNDLE_PARENT_MISSING"));
    let fixture = bundle.join("fixture.pdf");
    let bytes = fs::read(&fixture).unwrap_or_else(|_| fail("FIXTURE_READ_FAILED"));
    if bytes.len() != EXPECTED_BYTES {
        fail("FIXTURE_BYTES_MISMATCH");
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let documents = home.join("Documents");
    fs::create_dir_all(&documents).unwrap_or_else(|_| fail("DOCUMENTS_CREATE_FAILED"));
    let storage = documents.join("sbc8a4-pdfmake-runtime-store");
    fs::create_dir_all(&storage).unwrap_or_else(|_| fail("STORE_CREATE_FAILED"));

    let receipt = store_pdf_bytes(&storage, "invoice-proof.pdf", &bytes)
        .unwrap_or_else(|message| fail(message));
    if receipt.sha256 != EXPECTED_SHA256 {
        fail("FIXTURE_SHA_MISMATCH");
    }
    if receipt.bytes != EXPECTED_BYTES {
        fail("RECEIPT_BYTES_MISMATCH");
    }

    let result = format!(
        "{{\"schema_version\":1,\"candidate\":\"pdfmake\",\"platform\":\"ios_simulator\",\"simulator_runtime\":\"PASS\",\"fixture_sha256\":\"{}\",\"fixture_bytes\":{},\"rust_save_store\":{}}}",
        EXPECTED_SHA256,
        EXPECTED_BYTES,
        receipt_json(&receipt)
    );
    let marker = documents.join("SBC8A4_PDFMAKE_SIM_RUNTIME.json");
    fs::write(&marker, format!("{result}\n")).unwrap_or_else(|_| fail("MARKER_WRITE_FAILED"));
    println!("{result}");
}
