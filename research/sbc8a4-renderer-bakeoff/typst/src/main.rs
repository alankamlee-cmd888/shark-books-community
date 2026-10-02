use std::fs;

fn main() {
    let first = sbc8a4_typst_bakeoff::render_pdf()
        .expect("first embedded Typst render failed");
    let second = sbc8a4_typst_bakeoff::render_pdf()
        .expect("second embedded Typst render failed");
    assert_eq!(first, second, "Typst PDF bytes were not deterministic");
    fs::write("fixture-a.pdf", &first).expect("write fixture-a failed");
    fs::write("fixture-b.pdf", &second).expect("write fixture-b failed");
    println!("PASS_TYPST_EMBEDDED bytes={} deterministic=true", first.len());
}
