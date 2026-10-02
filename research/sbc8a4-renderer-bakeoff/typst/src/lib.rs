use typst_as_lib::{typst_kit_options::TypstKitFontOptions, TypstEngine};

pub fn fixture_source() -> String {
    let mut source = String::from(
        r#"#set document(
  title: [SBC8A4 deterministic fixture],
  author: ("MTD Shark",),
  date: datetime(year: 2026, month: 10, day: 2),
)
#set page(
  paper: "a4",
  margin: 14mm,
  header: align(right)[MTD Shark invoice fixture],
  numbering: "1",
  number-align: center + bottom,
)
#set text(size: 9pt)
= Invoice INV-2026-0001
Unicode fixture: Café naïve — £ € — Ελληνικά — Привет

#pagebreak()
== Long table
#table(
  columns: (auto, 1fr, auto),
  inset: 4pt,
  table.header([*Item*], [*Description*], [*Amount*]),
"#,
    );
    for i in 1..=140 {
        let pounds = i * 123 / 100;
        let pence = i * 123 % 100;
        source.push_str(&format!(
            "[{i}], [Café naïve — £ € — Ελληνικά — Привет {i}], [£{pounds}.{pence:02}],\n"
        ));
    }
    source.push_str(")\n");
    source
}

pub fn render_pdf() -> Result<Vec<u8>, String> {
    let source = fixture_source();
    let engine = TypstEngine::builder()
        .main_file(source)
        .search_fonts_with(
            TypstKitFontOptions::default()
                .include_system_fonts(false)
                .include_embedded_fonts(true),
        )
        .build();
    let compiled = engine.compile();
    let document = compiled
        .output
        .map_err(|errors| format!("typst compile failed: {errors:?}"))?;
    typst_pdf::pdf(&document, &Default::default())
        .map_err(|errors| format!("typst pdf export failed: {errors:?}"))
}
