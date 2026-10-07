import {
  CommercialDocumentRenderer,
  MAX_GENERATED_PDF_BYTES,
  buildCommercialDocumentDefinition,
  commercialPdfFilename,
  freezeCommercialDocumentView,
  type CommercialDocumentView,
} from "../commercialDocumentRenderer.ts";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

const fixture: CommercialDocumentView = freezeCommercialDocumentView({
  kind: "invoice",
  identity: "invoice-fixture-1001",
  displayNumber: "INV-1001",
  state: "issued",
  issuerName: "Example Trader",
  customer: {
    displayName: "Example Customer",
    postalAddress: "1 Test Street, Blackpool",
    email: "customer@example.invalid",
    phone: null,
  },
  dates: [
    { label: "Issue date", value: "2026-10-05" },
    { label: "Due date", value: "2026-10-19" },
  ],
  lines: [
    {
      description: "Bounded renderer fixture",
      quantity: "2",
      unitPricePence: 1250,
      totalPence: 2500,
    },
    {
      description: "Unicode fixture: café – £",
      quantity: "1",
      unitPricePence: 500,
      totalPence: 500,
    },
  ],
  amounts: [
    { label: "Total", amountPence: 3000 },
    { label: "Outstanding", amountPence: 3000 },
  ],
  notes: ["Offline deterministic renderer fixture."],
});

assert(Object.isFrozen(fixture), "commercial view must be frozen");
assert(Object.isFrozen(fixture.lines), "commercial lines must be frozen");
assert(
  commercialPdfFilename(fixture) === "invoice-INV-1001.pdf",
  "renderer must emit a portable deterministic basename",
);

const definition = buildCommercialDocumentDefinition(fixture);
const definitionText = JSON.stringify(definition, (_key, value) =>
  typeof value === "function" ? "[function]" : value,
);
assert(!definitionText.includes("http://"), "document definition must not contain remote HTTP resources");
assert(!definitionText.includes("https://"), "document definition must not contain remote HTTPS resources");

const renderer = new CommercialDocumentRenderer();
const first = await renderer.render(fixture);
const second = await renderer.render(fixture);

assert(first.byteLen > 0, "renderer must emit non-empty PDF bytes");
assert(first.byteLen <= MAX_GENERATED_PDF_BYTES, "renderer must respect the 25 MiB bound");
assert(
  first.bytes[0] === 0x25 &&
    first.bytes[1] === 0x50 &&
    first.bytes[2] === 0x44 &&
    first.bytes[3] === 0x46 &&
    first.bytes[4] === 0x2d,
  "renderer output must start with %PDF-",
);
assert(first.sha256 === second.sha256, "identical authorised input must have a stable SHA-256");
assert(first.byteLen === second.byteLen, "identical authorised input must have a stable byte length");
assert(first.filename === second.filename, "identical authorised input must have a stable filename");

console.log(`SBC8A4 renderer deterministic PASS sha256=${first.sha256} bytes=${first.byteLen}`);
