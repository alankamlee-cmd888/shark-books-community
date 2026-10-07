import type {
  OwnerCreditNoteView,
  OwnerInvoiceView,
  OwnerQuoteView,
  OwnerReportSummary,
} from "./tauri";

// pdfmake 0.3.11 does not publish declarations for these browser bundle entrypoints.
// @ts-ignore admitted local pdfmake browser bundle
import pdfMakeModule from "pdfmake/build/pdfmake.js";
// @ts-ignore admitted local pdfmake VFS bundle
import vfsModule from "pdfmake/build/vfs_fonts.js";

export const MAX_GENERATED_PDF_BYTES = 25 * 1024 * 1024;

export type CommercialDocumentKind =
  | "quote"
  | "estimate"
  | "invoice"
  | "creditNote"
  | "report";

export interface CommercialDocumentParty {
  readonly displayName: string;
  readonly postalAddress: string | null;
  readonly email: string | null;
  readonly phone: string | null;
}

export interface CommercialDocumentLine {
  readonly description: string;
  readonly quantity: string;
  readonly unitPricePence: number;
  readonly totalPence: number;
}

export interface CommercialDocumentAmount {
  readonly label: string;
  readonly amountPence: number;
}

export interface CommercialDocumentDate {
  readonly label: string;
  readonly value: string;
}

export interface CommercialDocumentView {
  readonly kind: CommercialDocumentKind;
  readonly identity: string;
  readonly displayNumber: string;
  readonly state: string;
  readonly issuerName: string;
  readonly customer: CommercialDocumentParty | null;
  readonly dates: readonly CommercialDocumentDate[];
  readonly lines: readonly CommercialDocumentLine[];
  readonly amounts: readonly CommercialDocumentAmount[];
  readonly notes: readonly string[];
}

export interface RenderedCommercialPdf {
  readonly filename: string;
  readonly bytes: Uint8Array;
  readonly byteLen: number;
  readonly sha256: string;
}

interface PdfMakeDocument {
  getBuffer(): Promise<Uint8Array | ArrayBuffer>;
}

interface PdfMakeBrowserApi {
  vfs: Record<string, string>;
  createPdf(definition: unknown): PdfMakeDocument;
}

type LooseModule = Record<string, unknown> & { default?: unknown };

const pdfMakeCandidate =
  (pdfMakeModule as unknown as LooseModule).default ?? pdfMakeModule;
const pdfMake = pdfMakeCandidate as PdfMakeBrowserApi;

const vfsCandidate =
  (vfsModule as unknown as LooseModule).default ?? vfsModule;
const vfsContainer = vfsCandidate as Record<string, unknown>;
const admittedVfs =
  ((vfsContainer.pdfMake as Record<string, unknown> | undefined)?.vfs ??
    vfsContainer.vfs ??
    vfsCandidate) as Record<string, string>;

pdfMake.vfs = admittedVfs;

function assertSafePence(value: number, label: string): number {
  if (!Number.isSafeInteger(value)) {
    throw new Error(`${label} is outside the supported whole-pence range.`);
  }
  return value;
}

function pounds(value: number): string {
  const pence = assertSafePence(value, "Commercial amount");
  const sign = pence < 0 ? "-" : "";
  const absolute = Math.abs(pence);
  return `${sign}GBP ${Math.floor(absolute / 100)}.${String(absolute % 100).padStart(2, "0")}`;
}

function freezeParty(
  party: CommercialDocumentParty | null,
): CommercialDocumentParty | null {
  return party ? Object.freeze({ ...party }) : null;
}

export function freezeCommercialDocumentView(
  view: CommercialDocumentView,
): CommercialDocumentView {
  if (!view.identity.trim() || !view.displayNumber.trim() || !view.issuerName.trim()) {
    throw new Error("Commercial document identity, number and issuer are required.");
  }
  const lines = Object.freeze(
    view.lines.map((line) =>
      Object.freeze({
        description: line.description,
        quantity: line.quantity,
        unitPricePence: assertSafePence(line.unitPricePence, "Unit price"),
        totalPence: assertSafePence(line.totalPence, "Line total"),
      }),
    ),
  );
  const amounts = Object.freeze(
    view.amounts.map((amount) =>
      Object.freeze({
        label: amount.label,
        amountPence: assertSafePence(amount.amountPence, amount.label),
      }),
    ),
  );
  const dates = Object.freeze(
    view.dates.map((date) => Object.freeze({ ...date })),
  );
  const notes = Object.freeze([...view.notes]);

  return Object.freeze({
    ...view,
    customer: freezeParty(view.customer),
    lines,
    amounts,
    dates,
    notes,
  });
}

function quoteParty(quote: OwnerQuoteView): CommercialDocumentParty {
  const party = quote.issuedSnapshot?.customer ?? quote.customer;
  return {
    displayName: party.displayName,
    postalAddress: party.postalAddress,
    email: party.email,
    phone: party.phone,
  };
}

export function quoteCommercialDocumentView(
  quote: OwnerQuoteView,
  issuerName: string,
): CommercialDocumentView {
  const snapshot = quote.issuedSnapshot;
  const sourceLines = snapshot?.lines ?? quote.lines;
  return freezeCommercialDocumentView({
    kind: quote.kind,
    identity: quote.quoteId,
    displayNumber: snapshot?.commercialNumber ?? quote.quoteId,
    state: quote.state,
    issuerName,
    customer: quoteParty(quote),
    dates: snapshot
      ? [{ label: "Issued", value: snapshot.issuedAt }]
      : [{ label: "Updated", value: quote.updatedAt }],
    lines: sourceLines.map((line) => ({
      description: line.description,
      quantity: String(line.quantitySubunits),
      unitPricePence: line.unitPricePence,
      totalPence: line.totalPence,
    })),
    amounts: [
      {
        label: "Total",
        amountPence: snapshot?.totalPence ?? quote.totalPence,
      },
    ],
    notes: snapshot
      ? ["Issued commercial snapshot. The PDF does not alter the underlying books."]
      : ["Draft commercial snapshot. Saving this PDF does not issue or post the draft."],
  });
}

function invoiceParty(invoice: OwnerInvoiceView): CommercialDocumentParty {
  const party = invoice.issued_snapshot?.customer ?? invoice.customer;
  return {
    displayName: party.display_name,
    postalAddress: party.postal_address,
    email: party.email,
    phone: party.phone,
  };
}

export function invoiceCommercialDocumentView(
  invoice: OwnerInvoiceView,
  issuerName: string,
): CommercialDocumentView {
  const snapshot = invoice.issued_snapshot;
  const sourceLines = snapshot?.lines ?? invoice.lines;
  return freezeCommercialDocumentView({
    kind: "invoice",
    identity: invoice.invoice_id,
    displayNumber: snapshot?.invoice_number ?? invoice.invoice_id,
    state: invoice.state,
    issuerName,
    customer: invoiceParty(invoice),
    dates: [
      { label: "Issue date", value: snapshot?.issue_date ?? invoice.issue_date },
      ...((snapshot?.due_date ?? invoice.due_date)
        ? [
            {
              label: "Due date",
              value: (snapshot?.due_date ?? invoice.due_date) as string,
            },
          ]
        : []),
    ],
    lines: sourceLines.map((line) => ({
      description: line.description,
      quantity: String(line.quantity_subunits),
      unitPricePence: line.unit_price_minor,
      totalPence: line.total_minor,
    })),
    amounts: [
      { label: "Total", amountPence: snapshot?.total_minor ?? invoice.total_minor },
      { label: "Manual payments", amountPence: invoice.paid_minor },
      { label: "Issued credits", amountPence: invoice.credited_minor },
      { label: "Outstanding", amountPence: invoice.outstanding_minor },
    ],
    notes: [
      ...(invoice.source_quote_id
        ? [`Source quote: ${invoice.source_quote_id}.`]
        : []),
      "Saving this PDF does not create a bank transaction or accounting posting.",
    ],
  });
}

export function creditNoteCommercialDocumentView(
  credit: OwnerCreditNoteView,
  sourceInvoice: OwnerInvoiceView,
  issuerName: string,
): CommercialDocumentView {
  const snapshot = credit.issued_snapshot;
  const sourceLines = snapshot?.lines ?? credit.lines;
  const party = snapshot?.customer ?? sourceInvoice.customer;
  return freezeCommercialDocumentView({
    kind: "creditNote",
    identity: credit.credit_note_id,
    displayNumber: snapshot?.credit_note_number ?? credit.credit_note_id,
    state: credit.state,
    issuerName,
    customer: {
      displayName: party.display_name,
      postalAddress: party.postal_address,
      email: party.email,
      phone: party.phone,
    },
    dates: snapshot ? [{ label: "Issued", value: snapshot.issued_at }] : [],
    lines: sourceLines.map((line) => ({
      description: line.description,
      quantity: String(line.quantity_subunits),
      unitPricePence: line.unit_price_minor,
      totalPence: line.total_minor,
    })),
    amounts: [{ label: "Credit total", amountPence: snapshot?.total_minor ?? credit.total_minor }],
    notes: [
      `Original invoice: ${credit.invoice_id}.`,
      "Saving this PDF does not record a payment or alter the issued credit.",
    ],
  });
}

export function reportCommercialDocumentView(
  report: OwnerReportSummary,
  issuerName: string,
): CommercialDocumentView {
  const amounts: CommercialDocumentAmount[] = [
    { label: "Money in", amountPence: report.moneyInMinor },
    { label: "Money out", amountPence: report.moneyOutMinor },
  ];
  if (report.businessBankBalanceMinor !== null) {
    amounts.push({
      label: "Business bank balance",
      amountPence: report.businessBankBalanceMinor,
    });
  }
  if (report.cashBalanceMinor !== null) {
    amounts.push({
      label: "Cash balance",
      amountPence: report.cashBalanceMinor,
    });
  }
  return freezeCommercialDocumentView({
    kind: "report",
    identity: "books-snapshot",
    displayNumber: "Books snapshot",
    state: report.booksBalanced ? "balanced" : "notBalanced",
    issuerName,
    customer: null,
    dates: [],
    lines: [],
    amounts,
    notes: [
      `Currency: ${report.currency}.`,
      `Books balance check: ${report.booksBalanced ? "Balanced" : "Not balanced"}.`,
      "This PDF is a factual presentation of already-recorded books data.",
    ],
  });
}

function safeFilenameToken(value: string): string {
  const ascii = value
    .normalize("NFKD")
    .replace(/[^\x20-\x7E]/g, "")
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .replace(/\.{2,}/g, ".");
  const trimmed = ascii.replace(/^\.+|\.+$/g, "").slice(0, 120);
  return trimmed || "commercial-document";
}

export function commercialPdfFilename(view: CommercialDocumentView): string {
  const kind = view.kind === "creditNote" ? "credit-note" : view.kind;
  return `${kind}-${safeFilenameToken(view.displayNumber || view.identity)}.pdf`;
}

export function buildCommercialDocumentDefinition(
  source: CommercialDocumentView,
): Record<string, unknown> {
  const view = freezeCommercialDocumentView(source);
  const customerBlocks: unknown[] = view.customer
    ? [
        { text: "Customer", style: "heading", margin: [0, 12, 0, 2] },
        { text: view.customer.displayName },
        ...(view.customer.postalAddress ? [{ text: view.customer.postalAddress }] : []),
        ...(view.customer.email ? [{ text: view.customer.email }] : []),
        ...(view.customer.phone ? [{ text: view.customer.phone }] : []),
      ]
    : [];

  const dateBlocks = view.dates.length
    ? [
        {
          margin: [0, 8, 0, 8],
          table: {
            widths: ["auto", "*"],
            body: view.dates.map((entry) => [entry.label, entry.value]),
          },
          layout: "noBorders",
        },
      ]
    : [];

  const lineBlocks: unknown[] = view.lines.length
    ? [
        { text: "Lines", style: "heading", margin: [0, 14, 0, 4] },
        {
          table: {
            headerRows: 1,
            widths: ["*", 55, 75, 75],
            body: [
              ["Description", "Qty", "Unit price", "Line total"],
              ...view.lines.map((line) => [
                line.description,
                line.quantity,
                pounds(line.unitPricePence),
                pounds(line.totalPence),
              ]),
            ],
          },
          layout: "lightHorizontalLines",
        },
      ]
    : [];

  const amountBlocks: unknown[] = view.amounts.length
    ? [
        { text: "Summary", style: "heading", margin: [0, 14, 0, 4] },
        {
          table: {
            widths: ["*", 100],
            body: view.amounts.map((amount) => [
              amount.label,
              { text: pounds(amount.amountPence), alignment: "right" },
            ]),
          },
          layout: "lightHorizontalLines",
        },
      ]
    : [];

  return {
    info: {
      title: `${view.issuerName} Â· ${view.displayNumber}`,
      author: view.issuerName,
      subject: `${view.kind} commercial document`,
      keywords: "SBC local offline commercial document",
      creationDate: new Date("2000-01-01T00:00:00.000Z"),
      modDate: new Date("2000-01-01T00:00:00.000Z"),
    },
    pageSize: "A4",
    pageMargins: [40, 58, 40, 54],
    header: (page: number, pages: number) => ({
      text: `${view.issuerName} Â· ${page}/${pages}`,
      alignment: "right",
      margin: [0, 18, 40, 0],
      fontSize: 8,
    }),
    footer: (page: number, pages: number) => ({
      text: `Page ${page} of ${pages}`,
      alignment: "center",
      margin: [0, 0, 0, 18],
      fontSize: 8,
    }),
    content: [
      { text: view.issuerName, style: "issuer" },
      { text: view.displayNumber, style: "title" },
      { text: `${view.kind} Â· ${view.state}`, style: "state" },
      ...customerBlocks,
      ...dateBlocks,
      ...lineBlocks,
      ...amountBlocks,
      ...(view.notes.length
        ? [
            { text: "Notes", style: "heading", margin: [0, 14, 0, 4] },
            ...view.notes.map((note) => ({ text: note, margin: [0, 1, 0, 1] })),
          ]
        : []),
      {
        qr: `SBC:${view.kind}:${view.identity}`,
        fit: 58,
        alignment: "right",
        margin: [0, 16, 0, 0],
      },
    ],
    styles: {
      issuer: { fontSize: 11, bold: true },
      title: { fontSize: 20, bold: true, margin: [0, 4, 0, 2] },
      state: { fontSize: 9, italics: true },
      heading: { fontSize: 11, bold: true },
    },
    defaultStyle: { font: "Roboto", fontSize: 9 },
  };
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  if (!globalThis.crypto?.subtle) {
    throw new Error("The local SHA-256 implementation is unavailable.");
  }
  const digest = await globalThis.crypto.subtle.digest(
    "SHA-256",
    Uint8Array.from(bytes),
  );
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function assertPdfSignature(bytes: Uint8Array): void {
  if (
    bytes.length < 5 ||
    bytes[0] !== 0x25 ||
    bytes[1] !== 0x50 ||
    bytes[2] !== 0x44 ||
    bytes[3] !== 0x46 ||
    bytes[4] !== 0x2d
  ) {
    throw new Error("Renderer output does not have a PDF signature.");
  }
}

export class CommercialDocumentRenderer {
  async render(source: CommercialDocumentView): Promise<RenderedCommercialPdf> {
    const view = freezeCommercialDocumentView(source);
    const raw = await pdfMake
      .createPdf(buildCommercialDocumentDefinition(view))
      .getBuffer();
    const bytes =
      raw instanceof Uint8Array
        ? Uint8Array.from(raw)
        : new Uint8Array(raw as ArrayBuffer);

    assertPdfSignature(bytes);
    if (bytes.length === 0 || bytes.length > MAX_GENERATED_PDF_BYTES) {
      throw new Error("Generated PDF exceeds the bounded 25 MiB save limit.");
    }

    return Object.freeze({
      filename: commercialPdfFilename(view),
      bytes,
      byteLen: bytes.length,
      sha256: await sha256Hex(bytes),
    });
  }
}
