<script setup lang="ts">
import { onMounted, ref } from "vue";
import QuotesTable from "../components/QuotesTable.vue";
import type { BooksSession } from "../lib/session";
import { errorMessage, formatMoney, penceToInput, readableToken } from "../lib/format";
import { CommercialDocumentRenderer, quoteCommercialDocumentView } from "../lib/commercialDocumentRenderer";
import {
  createQuoteDraft,
  issueQuote,
  listContacts,
  listQuotes,
  quoteDetail,
  removeQuoteLine,
  saveQuoteLine,
  setQuoteCustomer,
  storeCommercialPdf,
  transitionQuote,
  type OwnerContactView,
  type OwnerQuoteDetailOutcome,
  type OwnerQuoteLineView,
  type OwnerQuoteMutationOutcome,
  type OwnerQuoteView,
  type QuoteKind,
  type QuoteOutcomeState,
  type QuoteState,
} from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();
const renderer = new CommercialDocumentRenderer();
const rows = ref<OwnerQuoteView[]>([]);
const customers = ref<OwnerContactView[]>([]);
const detail = ref<OwnerQuoteDetailOutcome | null>(null);
const selectedQuoteId = ref<string | null>(null);
const filterKind = ref<"" | QuoteKind>("");
const filterState = ref<"" | QuoteState>("");
const createKind = ref<QuoteKind>("quote");
const createCustomerId = ref("");
const draftCustomerId = ref("");
const lineId = ref("");
const lineDescription = ref("");
const lineQuantity = ref("1");
const lineUnitPrice = ref("");
const commercialNumber = ref("");
const confirmIssue = ref(false);
const busy = ref(false);
const message = ref("");
const error = ref("");

function generatedId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}`;
}

function parseNonNegativePoundsToPence(value: string): number {
  const normalized = value.trim().replace(/,/g, "");
  if (!/^\d+(?:\.\d{1,2})?$/.test(normalized)) {
    throw new Error("Enter a GBP unit price with no more than two decimal places.");
  }
  const [whole, fraction = ""] = normalized.split(".");
  const pence = Number(whole) * 100 + Number((fraction + "00").slice(0, 2));
  if (!Number.isSafeInteger(pence) || pence < 0) {
    throw new Error("Unit price is outside the supported whole-pence range.");
  }
  return pence;
}

async function refresh(): Promise<void> {
  if (!props.session.state.isOpen) return;
  busy.value = true;
  error.value = "";
  try {
    const books = props.session.requireContext();
    const [quoteResult, customerResult] = await Promise.all([
      listQuotes(books, filterKind.value || undefined, filterState.value || undefined),
      listContacts(books, "customer"),
    ]);
    rows.value = quoteResult.quotes;
    customers.value = customerResult.contacts;
    if (!createCustomerId.value && customers.value.length > 0) {
      createCustomerId.value = customers.value[0].contactId;
    }
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function loadDetail(rowOrId: OwnerQuoteView | string): Promise<void> {
  const quoteId = typeof rowOrId === "string" ? rowOrId : rowOrId.quoteId;
  selectedQuoteId.value = quoteId;
  busy.value = true;
  error.value = "";
  try {
    detail.value = await quoteDetail(props.session.requireContext(), quoteId);
    draftCustomerId.value = detail.value.quote.customer.customerId;
    confirmIssue.value = false;
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function finishMutation(outcome: OwnerQuoteMutationOutcome, successMessage: string): Promise<void> {
  if (!outcome.nonPosting || outcome.invoiceCreated || outcome.requiresFurtherAutomaticAction) {
    throw new Error("The quote operation returned an unsafe side-effect contract.");
  }
  message.value = successMessage;
  await refresh();
  await loadDetail(outcome.quote.quoteId);
}

async function createDraft(): Promise<void> {
  if (!createCustomerId.value) {
    error.value = "Choose an existing customer.";
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    const outcome = await createQuoteDraft(
      props.session.requireContext(),
      generatedId(createKind.value),
      createKind.value,
      createCustomerId.value,
    );
    await finishMutation(outcome, `${readableToken(createKind.value)} draft created.`);
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function changeCustomer(): Promise<void> {
  if (!detail.value || !draftCustomerId.value) return;
  busy.value = true;
  error.value = "";
  try {
    const outcome = await setQuoteCustomer(
      props.session.requireContext(),
      detail.value.quote.quoteId,
      draftCustomerId.value,
    );
    await finishMutation(outcome, "Draft customer updated.");
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

function editLine(line: OwnerQuoteLineView): void {
  lineId.value = line.lineId;
  lineDescription.value = line.description;
  lineQuantity.value = String(line.quantitySubunits);
  lineUnitPrice.value = penceToInput(line.unitPricePence);
}

function clearLine(): void {
  lineId.value = "";
  lineDescription.value = "";
  lineQuantity.value = "1";
  lineUnitPrice.value = "";
}

async function saveLine(): Promise<void> {
  if (!detail.value) return;
  const quantity = Number(lineQuantity.value);
  if (!Number.isSafeInteger(quantity) || quantity <= 0) {
    error.value = "Quantity must be a positive whole number.";
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    const editing = Boolean(lineId.value);
    const outcome = await saveQuoteLine(
      props.session.requireContext(),
      detail.value.quote.quoteId,
      lineId.value || generatedId("line"),
      lineDescription.value,
      quantity,
      parseNonNegativePoundsToPence(lineUnitPrice.value),
    );
    clearLine();
    await finishMutation(outcome, editing ? "Draft line updated." : "Draft line added.");
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function removeLine(line: OwnerQuoteLineView): Promise<void> {
  if (!detail.value || !window.confirm(`Remove “${line.description}” from this draft?`)) return;
  busy.value = true;
  error.value = "";
  try {
    const outcome = await removeQuoteLine(
      props.session.requireContext(),
      detail.value.quote.quoteId,
      line.lineId,
    );
    clearLine();
    await finishMutation(outcome, "Draft line removed.");
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function issue(): Promise<void> {
  if (!detail.value || !confirmIssue.value) return;
  busy.value = true;
  error.value = "";
  try {
    const outcome = await issueQuote(
      props.session.requireContext(),
      detail.value.quote.quoteId,
      commercialNumber.value,
    );
    commercialNumber.value = "";
    confirmIssue.value = false;
    await finishMutation(outcome, "Commercial snapshot issued and frozen.");
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function transition(target: QuoteOutcomeState): Promise<void> {
  if (!detail.value) return;
  const consequence = target === "accepted"
    ? "This records eligibility for later 8A3 conversion only. It will not create an invoice."
    : `This will mark the document ${target}.`;
  if (!window.confirm(`${consequence}\n\nContinue?`)) return;
  busy.value = true;
  error.value = "";
  try {
    const outcome = await transitionQuote(
      props.session.requireContext(),
      detail.value.quote.quoteId,
      target,
    );
    await finishMutation(outcome, target === "accepted"
      ? "Accepted. Eligible for later 8A3 conversion; no invoice was created."
      : `Document marked ${target}.`);
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function savePdf(): Promise<void> {
  if (!detail.value) return;
  const storageRootId = props.session.state.storageRootId;
  if (!storageRootId) {
    error.value = "Select a document storage folder in Settings before saving PDFs.";
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    const issuerName = props.session.state.home?.companyName;
    if (!issuerName) throw new Error("The books company name is unavailable.");
    const rendered = await renderer.render(
      quoteCommercialDocumentView(detail.value.quote, issuerName),
    );
    const receipt = await storeCommercialPdf(
      storageRootId,
      rendered.filename,
      rendered.bytes,
      rendered.sha256,
    );
    if (receipt.sha256 !== rendered.sha256 || receipt.byteLen !== rendered.byteLen) {
      throw new Error("The stored PDF integrity receipt does not match the rendered bytes.");
    }
    message.value = receipt.created
      ? `PDF saved: ${receipt.relativePath}`
      : `PDF already saved with identical bytes: ${receipt.relativePath}`;
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

onMounted(refresh);
</script>

<template>
  <div class="screen-stack">
    <section v-if="!session.state.isOpen" class="panel empty-banner">Open your books from Home before using Quotes & estimates.</section>
    <template v-else>
      <section class="content-grid contact-layout">
        <article class="panel">
          <div class="panel-heading horizontal">
            <div><p class="eyebrow">Non-posting commercial documents</p><h2>Quotes & estimates</h2></div>
            <button type="button" class="secondary compact" :disabled="busy" @click="refresh">Refresh</button>
          </div>
          <div class="field-grid">
            <label><span>Type</span><select v-model="filterKind" @change="refresh"><option value="">All types</option><option value="quote">Quotes</option><option value="estimate">Estimates</option></select></label>
            <label><span>State</span><select v-model="filterState" @change="refresh"><option value="">All states</option><option v-for="state in ['draft','issued','accepted','rejected','expired','cancelled']" :key="state" :value="state">{{ readableToken(state) }}</option></select></label>
          </div>
          <QuotesTable :rows="rows" :selected-quote-id="selectedQuoteId" @select="loadDetail" />
          <hr />
          <p class="eyebrow">New draft</p>
          <div class="field-grid">
            <label><span>Type</span><select v-model="createKind"><option value="quote">Quote</option><option value="estimate">Estimate</option></select></label>
            <label><span>Customer</span><select v-model="createCustomerId"><option value="">Choose customer</option><option v-for="customer in customers" :key="customer.contactId" :value="customer.contactId">{{ customer.displayName }}</option></select></label>
          </div>
          <div class="button-row"><button type="button" data-action-id="QUOTE.CREATE_DRAFT" :disabled="busy || !createCustomerId" @click="createDraft">Create draft</button></div>
          <p class="subtle">Drafts and every later state change remain outside the ledger. No accounting transaction is posted.</p>
        </article>

        <article v-if="detail" class="panel">
          <p class="eyebrow">{{ readableToken(detail.quote.kind) }} · {{ readableToken(detail.quote.state) }}</p>
          <h2>{{ detail.quote.issuedSnapshot?.commercialNumber || detail.quote.quoteId }}</h2>
          <dl class="summary-list">
            <div><dt>Customer snapshot</dt><dd>{{ detail.quote.issuedSnapshot?.customer.displayName || detail.quote.customer.displayName }}</dd></div>
            <div><dt>Total</dt><dd>{{ formatMoney(detail.quote.totalPence) }}</dd></div>
            <div><dt>Posting</dt><dd>None</dd></div>
          </dl>
          <div class="button-row"><button type="button" class="secondary" data-action-id="COMMERCIAL.SAVE_PDF" :disabled="busy" @click="savePdf">Save PDF</button></div>
          <p v-if="!session.state.storageRootId" class="subtle">Choose a document storage folder in Settings before saving PDFs.</p>

          <template v-if="detail.quote.state === 'draft'">
            <h3>Edit draft</h3>
            <div class="field-grid">
              <label class="field-span"><span>Customer</span><select v-model="draftCustomerId"><option v-for="customer in customers" :key="customer.contactId" :value="customer.contactId">{{ customer.displayName }}</option></select></label>
            </div>
            <div class="button-row"><button type="button" class="secondary" data-action-id="QUOTE.SET_CUSTOMER" :disabled="busy || !draftCustomerId" @click="changeCustomer">Update customer</button></div>
            <div class="table-scroll" tabindex="0" aria-label="Draft commercial lines">
              <table class="owner-table">
                <thead><tr><th>Description</th><th>Quantity</th><th>Unit price</th><th>Total</th><th>Actions</th></tr></thead>
                <tbody>
                  <tr v-if="detail.quote.lines.length === 0"><td colspan="5" class="empty-cell">Add at least one line before issue.</td></tr>
                  <tr v-for="line in detail.quote.lines" :key="line.lineId">
                    <td>{{ line.description }}</td><td>{{ line.quantitySubunits }}</td><td>{{ formatMoney(line.unitPricePence) }}</td><td>{{ formatMoney(line.totalPence) }}</td>
                    <td class="action-cell"><button type="button" class="text-button" @click="editLine(line)">Edit</button><button type="button" class="text-button" @click="removeLine(line)">Remove</button></td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div class="field-grid">
              <label class="field-span"><span>Description</span><input v-model="lineDescription" maxlength="500" /></label>
              <label><span>Quantity</span><input v-model="lineQuantity" inputmode="numeric" /></label>
              <label><span>Unit price (£)</span><input v-model="lineUnitPrice" inputmode="decimal" placeholder="0.00" /></label>
            </div>
            <div class="button-row"><button type="button" :data-action-id="lineId ? 'QUOTE.REPLACE_LINE' : 'QUOTE.ADD_LINE'" :disabled="busy || !lineDescription.trim() || !lineUnitPrice.trim()" @click="saveLine">{{ lineId ? "Update line" : "Add line" }}</button><button v-if="lineId" type="button" class="secondary" @click="clearLine">Cancel edit</button></div>

            <h3>Issue and freeze</h3>
            <label><span>Commercial number</span><input v-model="commercialNumber" maxlength="64" placeholder="Q-0001" /></label>
            <label><input v-model="confirmIssue" type="checkbox" /> I confirm the customer and line snapshot will become immutable.</label>
            <div class="button-row"><button type="button" data-action-id="QUOTE.ISSUE" :disabled="busy || !commercialNumber.trim() || detail.quote.lines.length === 0 || !confirmIssue" @click="issue">Issue</button></div>
          </template>

          <template v-else-if="detail.quote.state === 'issued'">
            <h3>Record outcome</h3>
            <div class="button-row">
              <button type="button" data-action-id="QUOTE.ACCEPT" :disabled="busy" @click="transition('accepted')">Accept</button>
              <button type="button" class="secondary" data-action-id="QUOTE.REJECT" :disabled="busy" @click="transition('rejected')">Reject</button>
              <button type="button" class="secondary" data-action-id="QUOTE.EXPIRE" :disabled="busy" @click="transition('expired')">Expire</button>
              <button type="button" class="secondary" data-action-id="QUOTE.CANCEL" :disabled="busy" @click="transition('cancelled')">Cancel</button>
            </div>
          </template>

          <p v-if="detail.quote.conversionEligible" class="empty-banner">Accepted and explicitly eligible for later 8A3 conversion. No invoice exists and no invoice was created by this action.</p>
          <p v-else-if="detail.quote.state !== 'draft'" class="subtle">The issued customer, lines, number and total are immutable.</p>

          <h3>Mutation history</h3>
          <div class="table-scroll" tabindex="0" aria-label="Quote mutation history">
            <table class="owner-table">
              <thead><tr><th>When</th><th>Action</th><th>Actor</th><th>State change</th></tr></thead>
              <tbody>
                <tr v-for="entry in detail.history" :key="entry.mutationId">
                  <td>{{ entry.occurredAt }}</td><td>{{ readableToken(entry.action) }}</td><td>{{ entry.actor }}</td><td>{{ entry.before?.state || '—' }} → {{ entry.after?.state || '—' }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </article>
        <article v-else class="panel empty-banner">Select a quote or estimate to inspect its commercial snapshot and mutation history.</article>
      </section>
      <p class="operation-message" role="status" aria-live="polite">{{ error || message || (busy ? "Working…" : "") }}</p>
    </template>
  </div>
</template>
