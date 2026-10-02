<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import InvoicesTable from "../components/InvoicesTable.vue";
import type { BooksSession } from "../lib/session";
import { errorMessage, formatMoney, readableToken } from "../lib/format";
import {
  listContacts,
  listQuotes,
  listInvoices,
  listCreditNotes,
  invoiceDetail,
  creditNoteDetail,
  mutateInvoice,
  type InvoiceMutation,
  type OwnerContactView,
  type OwnerQuoteView,
  type OwnerInvoiceView,
  type OwnerCreditNoteView,
} from "../lib/tauri";

interface CommercialRow {
  id: string;
  number: string | null;
  kind: "invoice" | "creditNote";
  customer: string;
  state: string;
  totalPence: number;
  outstandingPence: number | null;
}

interface HistoryRow {
  id: number;
  action: string;
  actor: string;
  occurredAt: string;
}

const props = defineProps<{ session: BooksSession }>();
const invoices = ref<OwnerInvoiceView[]>([]);
const credits = ref<OwnerCreditNoteView[]>([]);
const customers = ref<OwnerContactView[]>([]);
const quotes = ref<OwnerQuoteView[]>([]);

const selectedKind = ref<"invoice" | "creditNote" | null>(null);
const selectedInvoice = ref<OwnerInvoiceView | null>(null);
const selectedCredit = ref<OwnerCreditNoteView | null>(null);
const referenceInvoice = ref<OwnerInvoiceView | null>(null);
const history = ref<HistoryRow[]>([]);

const customerId = ref("");
const quoteId = ref("");
const number = ref("");
const confirmIssue = ref(false);
const lineId = ref("");
const sourceInvoiceLineId = ref("");
const description = ref("");
const quantity = ref("1");
const unitPence = ref("0");
const paymentPence = ref("");
const busy = ref(false);
const error = ref("");
const message = ref("");
const filter = ref("");

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

function generatedId(prefix: string): string {
  return `${prefix}-${crypto.randomUUID()}`;
}

function money(pence: number): string {
  return Number.isSafeInteger(pence) ? formatMoney(pence) : "Outside display range";
}

function integer(value: string, positive = false): number {
  if (!/^\d+$/.test(value)) throw new Error("Enter whole numbers only.");
  const amount = Number(value);
  if (!Number.isSafeInteger(amount) || amount < (positive ? 1 : 0)) {
    throw new Error("Amount or quantity is outside the supported input range.");
  }
  return amount;
}

function clearLine(): void {
  lineId.value = "";
  sourceInvoiceLineId.value = "";
  description.value = "";
  quantity.value = "1";
  unitPence.value = "0";
}

const rows = computed<CommercialRow[]>(() => {
  const invoiceRows = invoices.value.map((invoice) => ({
    id: invoice.invoice_id,
    number: invoice.issued_snapshot?.invoice_number ?? null,
    kind: "invoice" as const,
    customer: invoice.customer.display_name,
    state: invoice.state,
    totalPence: invoice.total_minor,
    outstandingPence: invoice.outstanding_minor,
  }));
  const creditRows = credits.value.map((credit) => {
    const invoice = invoices.value.find((candidate) => candidate.invoice_id === credit.invoice_id);
    return {
      id: credit.credit_note_id,
      number: credit.issued_snapshot?.credit_note_number ?? null,
      kind: "creditNote" as const,
      customer: credit.issued_snapshot?.customer.display_name ?? invoice?.customer.display_name ?? "Customer",
      state: credit.state,
      totalPence: credit.total_minor,
      outstandingPence: null,
    };
  });
  return [...invoiceRows, ...creditRows];
});

const filtered = computed(() =>
  rows.value.filter((row) => !filter.value || row.kind === filter.value || row.state === filter.value),
);

const eligibleQuotes = computed(() =>
  quotes.value.filter(
    (quote) =>
      quote.conversionEligible &&
      !invoices.value.some((invoice) => invoice.source_quote_id === quote.quoteId),
  ),
);

const selectedId = computed(() =>
  selectedKind.value === "invoice"
    ? selectedInvoice.value?.invoice_id
    : selectedCredit.value?.credit_note_id,
);

const editable = computed(() =>
  selectedKind.value === "invoice"
    ? selectedInvoice.value?.state === "draft" && !selectedInvoice.value.source_quote_id
    : selectedCredit.value?.state === "draft",
);

const selectedState = computed(() =>
  selectedKind.value === "invoice" ? selectedInvoice.value?.state : selectedCredit.value?.state,
);

const selectedNumber = computed(() =>
  selectedKind.value === "invoice"
    ? selectedInvoice.value?.issued_snapshot?.invoice_number ?? selectedInvoice.value?.invoice_id ?? ""
    : selectedCredit.value?.issued_snapshot?.credit_note_number ?? selectedCredit.value?.credit_note_id ?? "",
);

const selectedTotal = computed(() =>
  selectedKind.value === "invoice" ? selectedInvoice.value?.total_minor ?? 0 : selectedCredit.value?.total_minor ?? 0,
);

const displayedLines = computed(() => {
  if (selectedKind.value === "invoice" && selectedInvoice.value) {
    return selectedInvoice.value.lines.map((line) => ({
      id: line.line_id,
      sourceInvoiceLineId: line.line_id,
      description: line.description,
      quantity: line.quantity_subunits,
      unitPricePence: line.unit_price_minor,
    }));
  }
  if (selectedKind.value === "creditNote" && selectedCredit.value) {
    return selectedCredit.value.lines.map((line) => ({
      id: line.line_id,
      sourceInvoiceLineId: line.invoice_line_id,
      description: line.description,
      quantity: line.quantity_subunits,
      unitPricePence: line.unit_price_minor,
    }));
  }
  return [];
});

const creditSourceLines = computed(() =>
  referenceInvoice.value?.issued_snapshot?.lines ?? referenceInvoice.value?.lines ?? [],
);

async function reload(): Promise<void> {
  const books = props.session.requireContext();
  const [invoiceResult, creditResult, contactResult, quoteResult] = await Promise.all([
    listInvoices(books),
    listCreditNotes(books),
    listContacts(books, "customer"),
    listQuotes(books, undefined, "accepted", 500),
  ]);
  if (invoiceResult.kind !== "invoices") throw new Error("Unexpected invoice list response.");
  if (creditResult.kind !== "creditNotes") throw new Error("Unexpected credit-note list response.");
  invoices.value = invoiceResult.invoices;
  credits.value = creditResult.creditNotes;
  customers.value = contactResult.contacts;
  quotes.value = quoteResult.quotes;
  if (!customerId.value) customerId.value = customers.value[0]?.contactId ?? "";
}

async function inspectRow(row: CommercialRow): Promise<void> {
  const books = props.session.requireContext();
  clearLine();
  confirmIssue.value = false;
  number.value = "";
  if (row.kind === "invoice") {
    const result = await invoiceDetail(books, row.id);
    if (result.kind !== "invoiceDetail") throw new Error("Unexpected invoice detail response.");
    selectedKind.value = "invoice";
    selectedInvoice.value = result.invoice;
    selectedCredit.value = null;
    referenceInvoice.value = result.invoice;
    history.value = result.history.map((entry) => ({
      id: entry.mutation_id,
      action: entry.action,
      actor: entry.actor,
      occurredAt: entry.occurred_at,
    }));
    customerId.value = result.invoice.customer.customer_id;
  } else {
    const result = await creditNoteDetail(books, row.id);
    if (result.kind !== "creditNoteDetail") throw new Error("Unexpected credit-note detail response.");
    selectedKind.value = "creditNote";
    selectedCredit.value = result.creditNote;
    selectedInvoice.value = null;
    referenceInvoice.value = result.invoice;
    history.value = result.history.map((entry) => ({
      id: entry.mutation_id,
      action: entry.action,
      actor: entry.actor,
      occurredAt: entry.occurred_at,
    }));
  }
}

async function run(task: () => Promise<void>): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  message.value = "";
  try {
    await task();
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function mutate(request: InvoiceMutation): Promise<void> {
  const outcome = await mutateInvoice(props.session.requireContext(), request);
  await reload();
  if (outcome.kind === "invoice") {
    await inspectRow({
      id: outcome.invoice.invoice_id,
      number: outcome.invoice.issued_snapshot?.invoice_number ?? null,
      kind: "invoice",
      customer: outcome.invoice.customer.display_name,
      state: outcome.invoice.state,
      totalPence: outcome.invoice.total_minor,
      outstandingPence: outcome.invoice.outstanding_minor,
    });
  } else {
    await inspectRow({
      id: outcome.creditNote.credit_note_id,
      number: outcome.creditNote.issued_snapshot?.credit_note_number ?? null,
      kind: "creditNote",
      customer: outcome.creditNote.issued_snapshot?.customer.display_name ?? outcome.invoice.customer.display_name,
      state: outcome.creditNote.state,
      totalPence: outcome.creditNote.total_minor,
      outstandingPence: null,
    });
  }
  message.value = "Saved. Mutation history updated.";
}

function createInvoice(): void {
  if (!customerId.value) return;
  void run(() =>
    mutate({
      operation: "createInvoiceDraft",
      invoiceId: generatedId("invoice"),
      customerId: customerId.value,
      issueDate: today(),
      dueDate: null,
    }),
  );
}

function convertQuote(): void {
  if (!quoteId.value) return;
  if (!window.confirm("Create one invoice draft from this accepted quote's immutable issued snapshot? This conversion can happen only once.")) return;
  void run(() =>
    mutate({
      operation: "convertAcceptedQuote",
      quoteId: quoteId.value,
      invoiceId: generatedId("invoice"),
      issueDate: today(),
      dueDate: null,
    }),
  );
}

function createCredit(): void {
  if (!selectedInvoice.value) return;
  void run(() =>
    mutate({
      operation: "createCreditNoteDraft",
      creditNoteId: generatedId("credit"),
      invoiceId: selectedInvoice.value!.invoice_id,
    }),
  );
}

function editLine(line: { id: string; sourceInvoiceLineId: string; description: string; quantity: number; unitPricePence: number }): void {
  lineId.value = line.id;
  sourceInvoiceLineId.value = line.sourceInvoiceLineId;
  description.value = line.description;
  quantity.value = String(line.quantity);
  unitPence.value = String(line.unitPricePence);
}

function selectCreditSource(): void {
  const line = creditSourceLines.value.find((candidate) => candidate.line_id === sourceInvoiceLineId.value);
  if (!line) return;
  lineId.value = "";
  description.value = line.description;
  quantity.value = "1";
  unitPence.value = String(line.unit_price_minor);
}

function saveLine(): void {
  void run(async () => {
    if (selectedKind.value === "invoice" && selectedInvoice.value) {
      await mutate({
        operation: "saveInvoiceLine",
        invoiceId: selectedInvoice.value.invoice_id,
        lineId: lineId.value || generatedId("line"),
        description: description.value,
        quantitySubunits: integer(quantity.value, true),
        unitPricePence: integer(unitPence.value),
      });
      return;
    }
    if (selectedKind.value === "creditNote" && selectedCredit.value) {
      if (!sourceInvoiceLineId.value) throw new Error("Choose an original invoice line.");
      await mutate({
        operation: "saveCreditNoteLine",
        creditNoteId: selectedCredit.value.credit_note_id,
        lineId: lineId.value || generatedId("credit-line"),
        invoiceLineId: sourceInvoiceLineId.value,
        quantitySubunits: integer(quantity.value, true),
      });
    }
  });
}

function removeLine(id: string): void {
  void run(async () => {
    if (selectedKind.value === "invoice" && selectedInvoice.value) {
      await mutate({ operation: "removeInvoiceLine", invoiceId: selectedInvoice.value.invoice_id, lineId: id });
    } else if (selectedKind.value === "creditNote" && selectedCredit.value) {
      await mutate({ operation: "removeCreditNoteLine", creditNoteId: selectedCredit.value.credit_note_id, lineId: id });
    }
  });
}

function updateCustomer(): void {
  if (!selectedInvoice.value || !customerId.value) return;
  void run(() =>
    mutate({
      operation: "setInvoiceCustomer",
      invoiceId: selectedInvoice.value!.invoice_id,
      customerId: customerId.value,
    }),
  );
}

function issue(): void {
  if (!confirmIssue.value || !number.value.trim()) return;
  if (selectedKind.value === "invoice" && selectedInvoice.value) {
    void run(() =>
      mutate({
        operation: "issueInvoice",
        invoiceId: selectedInvoice.value!.invoice_id,
        invoiceNumber: number.value.trim(),
      }),
    );
  } else if (selectedKind.value === "creditNote" && selectedCredit.value) {
    void run(() =>
      mutate({
        operation: "issueCreditNote",
        creditNoteId: selectedCredit.value!.credit_note_id,
        creditNoteNumber: number.value.trim(),
      }),
    );
  }
}

function cancel(): void {
  if (!window.confirm("Cancel this document?")) return;
  if (selectedKind.value === "invoice" && selectedInvoice.value) {
    void run(() => mutate({ operation: "cancelInvoice", invoiceId: selectedInvoice.value!.invoice_id }));
  } else if (selectedKind.value === "creditNote" && selectedCredit.value) {
    void run(() => mutate({ operation: "cancelCreditNote", creditNoteId: selectedCredit.value!.credit_note_id }));
  }
}

function pay(): void {
  if (!selectedInvoice.value) return;
  if (!window.confirm("Record an explicit manual payment amount? This updates the commercial balance and history only; it creates no bank transaction.")) return;
  void run(async () => {
    await mutate({
      operation: "recordManualPayment",
      invoiceId: selectedInvoice.value!.invoice_id,
      amountPence: integer(paymentPence.value, true),
    });
    paymentPence.value = "";
  });
}

onMounted(() => {
  if (props.session.state.isOpen) void run(reload);
});
</script>

<template>
  <div class="screen-stack">
    <section v-if="!session.state.isOpen" class="panel empty-banner">Open your books from Home before using Invoices & credit notes.</section>
    <template v-else>
      <section class="panel">
        <div class="panel-heading horizontal">
          <h2>Invoices & credit notes</h2>
          <button class="secondary" :disabled="busy" @click="run(reload)">Refresh</button>
        </div>
        <p class="subtle">Commercial balances and manual payment records. Credit notes reduce the balance without recording a bank payment.</p>
        <label>
          <span>Filter</span>
          <select v-model="filter">
            <option value="">All documents</option>
            <option v-for="state in ['invoice','creditNote','draft','issued','part_paid','paid','cancelled']" :key="state" :value="state">{{ readableToken(state) }}</option>
          </select>
        </label>
        <InvoicesTable :rows="filtered" :selected-id="selectedId" :busy="busy" @select="(row) => run(() => inspectRow(row))" />
        <div class="field-grid">
          <label>
            <span>Customer for new invoice</span>
            <select v-model="customerId" :disabled="busy">
              <option value="">Choose customer</option>
              <option v-for="c in customers" :key="c.contactId" :value="c.contactId">{{ c.displayName }}</option>
            </select>
          </label>
          <button :disabled="busy || !customerId" @click="createInvoice">Create invoice draft</button>
          <label>
            <span>Accepted quote / estimate</span>
            <select v-model="quoteId" :disabled="busy">
              <option value="">Choose accepted quote</option>
              <option v-for="q in eligibleQuotes" :key="q.quoteId" :value="q.quoteId">{{ q.issuedSnapshot?.commercialNumber || q.quoteId }} — {{ q.customer.displayName }}</option>
            </select>
          </label>
          <button :disabled="busy || !quoteId" @click="convertQuote">Convert accepted quote explicitly</button>
        </div>
      </section>

      <section v-if="selectedKind && (selectedInvoice || selectedCredit)" class="panel">
        <p class="eyebrow">{{ readableToken(selectedKind) }} · {{ readableToken(selectedState || '') }}</p>
        <h2>{{ selectedNumber }}</h2>

        <template v-if="selectedInvoice">
          <p>{{ selectedInvoice.customer.display_name }}<span v-if="selectedInvoice.customer.postal_address"> · {{ selectedInvoice.customer.postal_address }}</span></p>
          <p v-if="selectedInvoice.source_quote_id">Source quote: {{ selectedInvoice.source_quote_id }}. Conversion is immutable and one-to-one.</p>
          <dl class="summary-list">
            <div><dt>Total</dt><dd>{{ money(selectedInvoice.total_minor) }}</dd></div>
            <div><dt>Manual payments</dt><dd>{{ money(selectedInvoice.paid_minor) }}</dd></div>
            <div><dt>Issued credits</dt><dd>{{ money(selectedInvoice.credited_minor) }}</dd></div>
            <div><dt>Outstanding</dt><dd>{{ money(selectedInvoice.outstanding_minor) }}</dd></div>
          </dl>
        </template>

        <template v-else-if="selectedCredit">
          <p>Original invoice: {{ selectedCredit.invoice_id }}</p>
          <dl class="summary-list"><div><dt>Credit total</dt><dd>{{ money(selectedTotal) }}</dd></div></dl>
        </template>

        <div class="table-scroll" tabindex="0" aria-label="Commercial document lines">
          <table class="owner-table">
            <thead><tr><th>Description</th><th>Quantity</th><th>Unit price</th><th v-if="editable">Actions</th></tr></thead>
            <tbody>
              <tr v-for="line in displayedLines" :key="line.id">
                <td>{{ line.description }}</td>
                <td>{{ line.quantity }}</td>
                <td>{{ money(line.unitPricePence) }}</td>
                <td v-if="editable">
                  <button :disabled="busy" @click="editLine(line)">Edit</button>
                  <button :disabled="busy" @click="removeLine(line.id)">Remove</button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <template v-if="editable">
          <h3>Edit draft</h3>
          <div v-if="selectedInvoice" class="button-row">
            <select v-model="customerId" aria-label="Draft customer" :disabled="busy">
              <option v-for="c in customers" :key="c.contactId" :value="c.contactId">{{ c.displayName }}</option>
            </select>
            <button :disabled="busy || !customerId" @click="updateCustomer">Update customer</button>
          </div>
          <label v-else>
            <span>Original invoice line</span>
            <select v-model="sourceInvoiceLineId" :disabled="busy" @change="selectCreditSource">
              <option value="">Choose original line</option>
              <option v-for="line in creditSourceLines" :key="line.line_id" :value="line.line_id">{{ line.description }}</option>
            </select>
          </label>
          <div class="field-grid">
            <label><span>Description</span><input v-model="description" maxlength="500" :disabled="busy || selectedKind === 'creditNote'" /></label>
            <label><span>Quantity (whole units)</span><input v-model="quantity" inputmode="numeric" :disabled="busy" /></label>
            <label><span>Unit price (whole pence)</span><input v-model="unitPence" inputmode="numeric" :disabled="busy || selectedKind === 'creditNote'" /></label>
          </div>
          <div class="button-row">
            <button :disabled="busy || !description" @click="saveLine">Save line</button>
            <button class="secondary" :disabled="busy" @click="clearLine">Clear edit</button>
          </div>
        </template>

        <template v-if="selectedState === 'draft'">
          <h3>Issue and freeze</h3>
          <label><span>Document number</span><input v-model="number" maxlength="64" :disabled="busy" /></label>
          <label><input v-model="confirmIssue" type="checkbox" :disabled="busy" /> I confirm this number and commercial snapshot will be immutable.</label>
          <button :disabled="busy || !confirmIssue || !number.trim() || !displayedLines.length" @click="issue">Issue {{ selectedKind === 'invoice' ? 'invoice' : 'credit note' }}</button>
        </template>

        <div v-if="selectedInvoice && ['issued','part_paid','paid'].includes(selectedInvoice.state)" class="button-row">
          <button :disabled="busy" @click="createCredit">Create credit note</button>
          <label><span>Manual payment (whole pence)</span><input v-model="paymentPence" inputmode="numeric" :disabled="busy || selectedInvoice.outstanding_minor <= 0" /></label>
          <button :disabled="busy || !paymentPence || selectedInvoice.outstanding_minor <= 0" @click="pay">Record manual payment</button>
        </div>

        <button v-if="selectedState === 'draft'" class="secondary" :disabled="busy" @click="cancel">Cancel document</button>

        <h3>Mutation history</h3>
        <div class="table-scroll" tabindex="0" aria-label="Commercial mutation history">
          <table class="owner-table">
            <thead><tr><th>When</th><th>Actor</th><th>Action</th></tr></thead>
            <tbody><tr v-for="entry in history" :key="entry.id"><td>{{ entry.occurredAt }}</td><td>{{ entry.actor }}</td><td>{{ readableToken(entry.action) }}</td></tr></tbody>
          </table>
        </div>
      </section>

      <p role="status" aria-live="polite" class="operation-message">{{ error || message || (busy ? "Working…" : "") }}</p>
    </template>
  </div>
</template>
