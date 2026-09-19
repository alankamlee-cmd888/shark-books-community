<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ConfirmationDialog from "../components/ConfirmationDialog.vue";
import DocumentTable from "../components/DocumentTable.vue";
import type { BooksSession } from "../lib/session";
import { errorMessage, formatMoney, readableToken } from "../lib/format";
import { UIB_BINDINGS } from "../lib/action-bindings";
import {
  attachDocument, confirmReceiptBank, listDocuments, listMoneyRecords, openDocumentView,
  ocrExtractReceipt, rejectReceiptBank, selectAndRegisterDocument, suggestReceiptBank,
  verifyDocument, type OwnerDocumentRead, type OwnerMoneyRecordRead,
  type OwnerReceiptSuggestionOutcome, type ShellOcrOutcome,
} from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();
const documents = ref<OwnerDocumentRead[]>([]);
const selectedDocument = ref<OwnerDocumentRead | null>(null);
const integrity = ref("");
const previewUrl = ref("");
const previewKind = ref<"image" | "pdf" | "unsupported" | "">("");
const ocr = ref<ShellOcrOutcome | null>(null);
const suggestion = ref<OwnerReceiptSuggestionOutcome | null>(null);
const selectedCandidateId = ref<string | null>(null);
const confirmOpen = ref(false);
const rejectOpen = ref(false);
const attachKind = ref<"moneyIn" | "moneyOut">("moneyOut");
const attachRecordId = ref("");
const moneyInRows = ref<OwnerMoneyRecordRead[]>([]);
const moneyOutRows = ref<OwnerMoneyRecordRead[]>([]);
const busy = ref(false);
const message = ref("");
const error = ref("");

const currentAttachRows = computed(() => attachKind.value === "moneyIn" ? moneyInRows.value : moneyOutRows.value);
const readySuggestion = computed(() => suggestion.value?.status === "ready" ? suggestion.value : null);

function chooseDocument(row: OwnerDocumentRead): void { selectedDocument.value = row; }

function revokePreview(): void {
  if (previewUrl.value) URL.revokeObjectURL(previewUrl.value);
  previewUrl.value = "";
  previewKind.value = "";
}

watch(selectedDocument, () => {
  revokePreview();
  integrity.value = "";
  ocr.value = null;
  suggestion.value = null;
  selectedCandidateId.value = null;
  message.value = "";
  error.value = "";
});
watch(attachKind, () => { attachRecordId.value = ""; });
onBeforeUnmount(revokePreview);

async function refreshDocuments(): Promise<void> {
  if (!props.session.state.isOpen) return;
  busy.value = true; error.value = "";
  try {
    const page = await listDocuments(props.session.requireContext());
    documents.value = page.rows;
    if (selectedDocument.value) {
      selectedDocument.value = page.rows.find(row => row.documentId === selectedDocument.value?.documentId) ?? null;
    }
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

async function refreshAttachRecords(): Promise<void> {
  if (!props.session.state.isOpen) return;
  try {
    const books = props.session.requireContext();
    const [moneyIn, moneyOut] = await Promise.all([
      listMoneyRecords(books, "moneyIn", 100, 0),
      listMoneyRecords(books, "moneyOut", 100, 0),
    ]);
    moneyInRows.value = moneyIn.rows;
    moneyOutRows.value = moneyOut.rows;
  } catch (err) { error.value = errorMessage(err); }
}

async function addDocument(): Promise<void> {
  const storageRootId = props.session.state.storageRootId;
  if (!storageRootId) {
    error.value = "Choose a document storage folder in Settings before adding a document.";
    return;
  }
  busy.value = true; error.value = "";
  try {
    const outcome = await selectAndRegisterDocument(props.session.requireContext(), storageRootId);
    message.value = outcome.status === "cancelled" ? "Document selection cancelled." : "Document registered in your selected storage folder.";
    await refreshDocuments();
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

async function verifySelected(): Promise<void> {
  if (!selectedDocument.value) return;
  busy.value = true; error.value = "";
  try {
    const result = await verifyDocument(props.session.requireContext(), selectedDocument.value.documentId);
    integrity.value = result.integrity;
    message.value = result.integrity === "verified" ? "Document integrity verified." : `Document check: ${readableToken(result.integrity)}.`;
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

async function openPreview(): Promise<void> {
  if (!selectedDocument.value) return;
  busy.value = true; error.value = ""; revokePreview();
  try {
    const verified = await verifyDocument(props.session.requireContext(), selectedDocument.value.documentId);
    integrity.value = verified.integrity;
    if (verified.integrity !== "verified") throw new Error(`Document cannot be previewed: ${readableToken(verified.integrity)}.`);
    const mediaType = selectedDocument.value.mediaType || "application/octet-stream";
    previewKind.value = mediaType.startsWith("image/") && !mediaType.includes("heic") && !mediaType.includes("heif")
      ? "image" : mediaType === "application/pdf" ? "pdf" : "unsupported";
    if (previewKind.value === "unsupported") {
      message.value = "Document is verified; inline preview is not available for this type.";
      return;
    }
    const bytes = await openDocumentView(props.session.requireContext(), selectedDocument.value.documentId);
    const blob = new Blob([bytes], { type: mediaType });
    previewUrl.value = URL.createObjectURL(blob);
    message.value = "Verified preview opened.";
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

function requestId(prefix: string): string { return `${prefix}-${Date.now().toString(36)}`; }
async function runOcr(): Promise<void> {
  if (!selectedDocument.value) return;
  busy.value = true; error.value = ""; suggestion.value = null;
  try {
    ocr.value = await ocrExtractReceipt(props.session.requireContext(), requestId("ocr"), selectedDocument.value.documentId);
    message.value = ocr.value.status === "completed" ? "Factual receipt extraction completed." : ocr.value.status === "unavailable" ? `OCR unavailable: ${readableToken(ocr.value.reason)}.` : `OCR failed: ${readableToken(ocr.value.kind)}.`;
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

async function findBankSuggestions(): Promise<void> {
  if (!selectedDocument.value) return;
  busy.value = true; error.value = ""; selectedCandidateId.value = null;
  try {
    suggestion.value = await suggestReceiptBank(props.session.requireContext(), requestId("receipt"), selectedDocument.value.documentId);
    if (suggestion.value.status === "ready") {
      message.value = suggestion.value.candidates.length ? "Possible bank activity is ready for your review. Nothing has been matched." : "No bank candidate was found. The document remains unlinked.";
    } else if (suggestion.value.status === "ocrUnavailable") {
      message.value = `OCR unavailable: ${readableToken(suggestion.value.reason)}. Manual attachment remains available.`;
    } else {
      message.value = `OCR failed: ${readableToken(suggestion.value.kind)}. Manual attachment remains available.`;
    }
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

async function confirmSuggestion(): Promise<void> {
  if (!readySuggestion.value || !selectedCandidateId.value) return;
  busy.value = true; error.value = "";
  try {
    await confirmReceiptBank(props.session.requireContext(), readySuggestion.value.suggestionId, selectedCandidateId.value);
    message.value = "Receipt-to-bank suggestion decision recorded. No Bank match was created automatically.";
    suggestion.value = null; selectedCandidateId.value = null; confirmOpen.value = false;
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

async function rejectSuggestion(): Promise<void> {
  if (!readySuggestion.value) return;
  busy.value = true; error.value = "";
  try {
    await rejectReceiptBank(props.session.requireContext(), readySuggestion.value.suggestionId);
    message.value = "Receipt-to-bank suggestions rejected. The document remains available for manual attachment.";
    suggestion.value = null; selectedCandidateId.value = null; rejectOpen.value = false;
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

async function attachSelected(): Promise<void> {
  if (!selectedDocument.value || !attachRecordId.value) return;
  busy.value = true; error.value = "";
  try {
    const result = await attachDocument(props.session.requireContext(), selectedDocument.value.documentId, attachKind.value, attachRecordId.value);
    message.value = result.alreadyAttached ? "Document was already attached to that record." : "Document attached to the selected record.";
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}

onMounted(async () => { await Promise.all([refreshDocuments(), refreshAttachRecords()]); });
</script>

<template>
  <div class="screen-stack">
    <section v-if="!session.state.isOpen" class="panel empty-banner">Open your books from Home before using Receipts.</section>
    <template v-else>
      <section class="panel">
        <div class="panel-heading horizontal">
          <div><p class="eyebrow">Owner-safe documents</p><h2>Receipts & documents</h2></div>
          <div class="button-row">
            <button type="button" class="secondary compact" :data-action-id="UIB_BINDINGS.documentList.action.actionId" :disabled="busy" @click="refreshDocuments">Refresh</button>
            <button type="button" :data-action-id="UIB_BINDINGS.documentSelectRegister.action.actionId" :disabled="busy || !session.state.storageRootId" @click="addDocument">Add document</button>
          </div>
        </div>
        <p v-if="!session.state.storageRootId" class="warning-banner">Choose your document storage folder in Settings first. Shark Books never asks the web view for a native file path.</p>
        <DocumentTable :rows="documents" :selected-document-id="selectedDocument?.documentId" @select="chooseDocument" />
      </section>

      <section v-if="selectedDocument" class="content-grid receipts-grid">
        <article class="panel">
          <p class="eyebrow">Selected document</p><h2>{{ selectedDocument.originalFilename }}</h2>
          <div class="button-row">
            <button type="button" class="secondary" :data-action-id="UIB_BINDINGS.documentVerify.action.actionId" :disabled="busy" @click="verifySelected">Check integrity</button>
            <button type="button" class="secondary" :data-action-id="UIB_BINDINGS.documentOpenView.action.actionId" :disabled="busy" @click="openPreview">Open verified preview</button>
          </div>
          <p v-if="integrity" class="subtle">Integrity: {{ readableToken(integrity) }}</p>
          <div v-if="previewUrl" class="preview-shell">
            <img v-if="previewKind === 'image'" class="document-preview-image" :src="previewUrl" alt="Verified selected document preview" />
            <iframe v-else-if="previewKind === 'pdf'" class="document-preview-frame" :src="previewUrl" title="Verified selected PDF preview" />
            <p v-else class="subtle">Verified document. This file type has no inline preview.</p>
          </div>
        </article>

        <article class="panel">
          <p class="eyebrow">Factual OCR</p><h2>Extract receipt facts</h2>
          <p class="subtle">OCR is evidence only. It does not choose an accounting category, tax treatment, or Bank match.</p>
          <button type="button" :data-action-id="UIB_BINDINGS.ocrExtract.action.actionId" :disabled="busy" @click="runOcr">Extract factual receipt details</button>
          <template v-if="ocr?.status === 'completed'">
            <dl class="detail-list ocr-facts">
              <div v-if="ocr.extraction.candidates.merchantText"><dt>Merchant</dt><dd>{{ ocr.extraction.candidates.merchantText.value }}</dd></div>
              <div v-if="ocr.extraction.candidates.documentDate"><dt>Date</dt><dd>{{ ocr.extraction.candidates.documentDate.value }}</dd></div>
              <div v-if="ocr.extraction.candidates.total"><dt>Total</dt><dd>{{ formatMoney(ocr.extraction.candidates.total.totalPence, ocr.extraction.candidates.currency?.code || 'GBP') }}</dd></div>
              <div v-if="ocr.extraction.candidates.currency"><dt>Currency</dt><dd>{{ ocr.extraction.candidates.currency.code }}</dd></div>
              <div v-if="ocr.extraction.candidates.reference"><dt>Reference</dt><dd>{{ ocr.extraction.candidates.reference.value }}</dd></div>
            </dl>
            <ul v-if="ocr.extraction.warnings.length" class="error-list"><li v-for="warning in ocr.extraction.warnings" :key="warning.code">{{ readableToken(warning.code) }}<span v-if="warning.detail"> — {{ warning.detail }}</span></li></ul>
          </template>
          <p v-else-if="ocr?.status === 'unavailable'" class="warning-banner">OCR is unavailable: {{ readableToken(ocr.reason) }}. You can still attach this document manually.</p>
          <p v-else-if="ocr?.status === 'failed'" class="warning-banner">OCR failed: {{ readableToken(ocr.kind) }}. You can still attach this document manually.</p>
        </article>
      </section>

      <section v-if="selectedDocument" class="content-grid receipts-grid">
        <article class="panel">
          <p class="eyebrow">Manual fallback</p><h2>Attach to an existing record</h2>
          <p class="subtle">This manual path remains available whether OCR works or not. Leaving the document unlinked is also valid.</p>
          <div class="field-grid">
            <label><span>Record type</span><select v-model="attachKind"><option value="moneyOut">Money out</option><option value="moneyIn">Money in</option></select></label>
            <label><span>Record</span><select v-model="attachRecordId"><option value="">Choose a current record</option><option v-for="row in currentAttachRows" :key="row.recordId" :value="row.recordId">{{ row.date }} · {{ row.description }} · {{ formatMoney(row.amountPence, row.currency) }}</option></select></label>
          </div>
          <button type="button" :data-action-id="UIB_BINDINGS.documentAttach.action.actionId" :disabled="busy || !attachRecordId" @click="attachSelected">Attach selected document</button>
        </article>

        <article class="panel">
          <p class="eyebrow">Deterministic review</p><h2>Possible bank activity</h2>
          <p class="subtle">A suggestion is not a Bank match. Shark Books records only your receipt-review decision here.</p>
          <button type="button" :data-action-id="UIB_BINDINGS.receiptSuggestBank.action.actionId" :disabled="busy" @click="findBankSuggestions">Find possible bank activity</button>
          <template v-if="readySuggestion">
            <p v-if="readySuggestion.ambiguousTop" class="warning-banner">The strongest results are ambiguous. Review the facts and choose explicitly, or reject the set.</p>
            <p v-if="readySuggestion.candidates.length === 0" class="subtle">No candidate was found.</p>
            <label v-for="candidate in readySuggestion.candidates" :key="candidate.candidateId" class="candidate-card">
              <input v-model="selectedCandidateId" type="radio" :value="candidate.candidateId" :disabled="candidate.level === 'unmatched'" />
              <span><strong>{{ candidate.postedDate }} · {{ candidate.description }} · {{ formatMoney(candidate.amountPence) }}</strong><small>{{ readableToken(candidate.level) }} · score {{ candidate.score }}<template v-if="candidate.candidateId === readySuggestion.recommendedCandidateId"> · Recommended by deterministic rules</template></small><small v-if="candidate.payee">{{ candidate.payee }}</small><small v-if="candidate.reference">Reference: {{ candidate.reference }}</small><small>{{ candidate.reasons.map(readableToken).join(', ') }}</small></span>
            </label>
            <div class="button-row">
              <button type="button" :data-action-id="UIB_BINDINGS.receiptConfirmBank.action.actionId" :disabled="busy || !selectedCandidateId" @click="confirmOpen = true">Continue to confirmation</button>
              <button type="button" class="secondary" :data-action-id="UIB_BINDINGS.receiptRejectBank.action.actionId" :disabled="busy" @click="rejectOpen = true">Reject suggestions</button>
            </div>
          </template>
        </article>
      </section>

      <p class="operation-message" role="status" aria-live="polite">{{ error || message || (busy ? "Working…" : "") }}</p>
    </template>

    <ConfirmationDialog v-if="readySuggestion && selectedCandidateId" v-model:open="confirmOpen" title="Confirm receipt review" description="Record your explicit choice of this current bank-activity candidate. This does not create an automatic Bank match." confirm-label="Confirm selected suggestion" :action-id="UIB_BINDINGS.receiptConfirmBank.action.actionId" :busy="busy" @confirm="confirmSuggestion" />
    <ConfirmationDialog v-if="readySuggestion" v-model:open="rejectOpen" title="Reject receipt suggestions" description="Record that none of the current suggestions should be accepted. The document stays available for manual attachment." confirm-label="Reject suggestion set" :action-id="UIB_BINDINGS.receiptRejectBank.action.actionId" :busy="busy" @confirm="rejectSuggestion" />
  </div>
</template>
