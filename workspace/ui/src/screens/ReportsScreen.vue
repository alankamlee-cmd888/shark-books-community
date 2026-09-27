<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { BooksSession } from "../lib/session";
import { errorMessage, formatMoney } from "../lib/format";
import { UIB_BINDINGS } from "../lib/action-bindings";
import { reportSummary, type OwnerReportSummary } from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();
const report = ref<OwnerReportSummary | null>(null);
const busy = ref(false);
const error = ref("");
const message = ref("");
async function refresh(): Promise<void> {
  if (!props.session.state.isOpen) return;
  busy.value = true; error.value = "";
  try { report.value = await reportSummary(props.session.requireContext()); message.value = "Factual books summary refreshed."; }
  catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}
onMounted(refresh);
</script>

<template>
  <div class="screen-stack">
    <section v-if="!session.state.isOpen" class="panel empty-banner">Open your books from Home before using Reports.</section>
    <template v-else>
      <section class="panel">
        <div class="panel-heading horizontal"><div><p class="eyebrow">Factual summary</p><h2>Books snapshot</h2></div><button type="button" class="secondary compact" :data-action-id="UIB_BINDINGS.reportSummary.action.actionId" :disabled="busy" @click="refresh">Refresh</button></div>
        <p class="subtle">This view reports recorded books facts only.</p>
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
