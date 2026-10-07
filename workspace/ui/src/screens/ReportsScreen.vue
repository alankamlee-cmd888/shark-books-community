<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { BooksSession } from "../lib/session";
import { errorMessage, formatMoney } from "../lib/format";
import { UIB_BINDINGS } from "../lib/action-bindings";
import { CommercialDocumentRenderer, reportCommercialDocumentView } from "../lib/commercialDocumentRenderer";
import { reportSummary, storeCommercialPdf, type OwnerReportSummary } from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();
const renderer = new CommercialDocumentRenderer();
const report = ref<OwnerReportSummary | null>(null);
const busy = ref(false);
const error = ref("");
const message = ref("");

async function refresh(): Promise<void> {
  if (!props.session.state.isOpen) return;
  busy.value = true;
  error.value = "";
  try {
    report.value = await reportSummary(props.session.requireContext());
    message.value = "Factual books summary refreshed.";
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    busy.value = false;
  }
}

async function savePdf(): Promise<void> {
  if (!report.value) return;
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
      reportCommercialDocumentView(report.value, issuerName),
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
    <section v-if="!session.state.isOpen" class="panel empty-banner">Open your books from Home before using Reports.</section>
    <template v-else>
      <section class="panel">
        <div class="panel-heading horizontal">
          <div><p class="eyebrow">Factual summary</p><h2>Books snapshot</h2></div>
          <div class="button-row">
            <button type="button" class="secondary compact" :data-action-id="UIB_BINDINGS.reportSummary.action.actionId" :disabled="busy" @click="refresh">Refresh</button>
            <button type="button" class="secondary compact" data-action-id="COMMERCIAL.SAVE_PDF" :disabled="busy || !report" @click="savePdf">Save PDF</button>
          </div>
        </div>
        <p class="subtle">This view reports recorded books facts only.</p>
        <p v-if="!session.state.storageRootId" class="subtle">Choose a document storage folder in Settings before saving PDFs.</p>
        <div v-if="report" class="status-grid report-grid">
          <article class="metric-card"><span>Money in</span><strong>{{ formatMoney(report.moneyInMinor, report.currency) }}</strong></article>
          <article class="metric-card"><span>Money out</span><strong>{{ formatMoney(report.moneyOutMinor, report.currency) }}</strong></article>
          <article class="metric-card"><span>Business bank balance</span><strong>{{ report.businessBankBalanceMinor === null ? "Not available" : formatMoney(report.businessBankBalanceMinor, report.currency) }}</strong></article>
          <article class="metric-card"><span>Cash balance</span><strong>{{ report.cashBalanceMinor === null ? "Not available" : formatMoney(report.cashBalanceMinor, report.currency) }}</strong></article>
          <article class="metric-card"><span>Books balance check</span><strong>{{ report.booksBalanced ? "Balanced" : "Not balanced" }}</strong></article>
          <article class="metric-card"><span>Currency</span><strong>{{ report.currency }}</strong></article>
        </div>
      </section>
      <p class="operation-message" role="status" aria-live="polite">{{ error || message || (busy ? "Working…" : "") }}</p>
    </template>
  </div>
</template>
