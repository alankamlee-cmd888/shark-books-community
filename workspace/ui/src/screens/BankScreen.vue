<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import BankActivityTable from "../components/BankActivityTable.vue";
import ConfirmationDialog from "../components/ConfirmationDialog.vue";
import { UIA_BINDINGS } from "../lib/action-bindings";
import { errorMessage, formatMoney, parseSignedPoundsToPence, readableToken } from "../lib/format";
import type { BooksSession } from "../lib/session";
import {
  bankActivityDetail,
  confirmBankMatch,
  confirmCsvImport,
  confirmOfxImport,
  finaliseReconciliation,
  listBankActivity,
  previewReconciliation,
  reviewCsvImport,
  reviewOfxImport,
  reviewPersistedBankActivityMatch,
  type CsvAmountMapping,
  type CsvDateFormat,
  type CsvProfile,
  type OfxFormat,
  type OwnerBankActivityDetail,
  type OwnerBankActivityRow,
  type OwnerBankImportReview,
  type OwnerBankMatchReview,
  type OwnerReconciliationPreview,
} from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();

const busy = ref(false);
const error = ref("");
const message = ref("");

const statementText = ref("");
const statementFileName = ref("");
const statementKind = ref<"csv" | "ofx" | "qfx">("csv");
const importReview = ref<OwnerBankImportReview | null>(null);
const importDialogOpen = ref(false);

const csv = reactive({
  delimiter: ",",
  dateHeader: "Date",
  valueDateHeader: "",
  descriptionHeader: "Description",
  payeeHeader: "Payee",
  referenceHeader: "Reference",
  transactionIdHeader: "TransactionId",
  currencyHeader: "",
  amountKind: "signed" as "signed" | "debitCredit",
  amountHeader: "Amount",
  debitHeader: "Debit",
  creditHeader: "Credit",
  dateFormat: "isoYmd" as CsvDateFormat,
});

const activityRows = ref<OwnerBankActivityRow[]>([]);
const selectedActivity = ref<OwnerBankActivityRow | null>(null);
const activityDetail = ref<OwnerBankActivityDetail | null>(null);

const matchReview = ref<OwnerBankMatchReview | null>(null);
const selectedMatchTransactionId = ref<number | null>(null);
const matchDialogOpen = ref(false);

const reconcile = reactive({
  statementId: "",
  statementDate: new Date().toISOString().slice(0, 10),
  openingBalance: "",
  endingBalance: "",
});
const selectedReconcileTransactions = ref<Set<number>>(new Set());
const reconciliationPreview = ref<OwnerReconciliationPreview | null>(null);
const reconciliationDialogOpen = ref(false);

function requireBooks() {
  return props.session.requireContext();
}

function csvProfile(): CsvProfile {
  let amountMapping: CsvAmountMapping;
  if (csv.amountKind === "signed") {
    amountMapping = { kind: "signed", amountHeader: csv.amountHeader.trim() };
  } else {
    amountMapping = {
      kind: "debitCredit",
      debitHeader: csv.debitHeader.trim(),
      creditHeader: csv.creditHeader.trim(),
    };
  }
  return {
    id: "uia-bank-csv",
    name: "UI-A bank import",
    delimiter: csv.delimiter,
    dateHeader: csv.dateHeader.trim(),
    valueDateHeader: csv.valueDateHeader.trim() || null,
    descriptionHeader: csv.descriptionHeader.trim(),
    payeeHeader: csv.payeeHeader.trim() || null,
    referenceHeader: csv.referenceHeader.trim() || null,
    transactionIdHeader: csv.transactionIdHeader.trim() || null,
    currencyHeader: csv.currencyHeader.trim() || null,
    amountMapping,
    dateFormat: csv.dateFormat,
  };
}

async function selectStatementFile(event: Event) {
  error.value = "";
  importReview.value = null;
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  if (file.size > 10 * 1024 * 1024) {
    error.value = "Statement file exceeds the supported 10 MiB limit.";
    input.value = "";
    return;
  }
  const extension = file.name.split(".").pop()?.toLowerCase();
  if (!extension || !["csv", "ofx", "qfx"].includes(extension)) {
    error.value = "Choose a CSV, OFX or QFX statement.";
    input.value = "";
    return;
  }
  statementKind.value = extension as "csv" | "ofx" | "qfx";
  statementFileName.value = file.name;
  statementText.value = await file.text();
  message.value = `${file.name} loaded locally for review.`;
}

async function reviewStatement() {
  if (!statementText.value) {
    error.value = "Choose a statement first.";
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    importReview.value =
      statementKind.value === "csv"
        ? await reviewCsvImport(requireBooks(), statementText.value, csvProfile())
        : await reviewOfxImport(
            requireBooks(),
            statementText.value,
            statementKind.value as OfxFormat,
          );
    message.value = importReview.value.canConfirm
      ? "Statement review complete. Check duplicates before confirming."
      : "Statement cannot be confirmed until the reported errors are resolved.";
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

function duplicateOutcome(lineRef: string): string {
  return importReview.value?.duplicateReviews.find((row) => row.lineRef === lineRef)?.outcome ?? "new";
}

async function confirmReviewedImport() {
  if (!importReview.value) return;
  busy.value = true;
  error.value = "";
  try {
    const receipt =
      statementKind.value === "csv"
        ? await confirmCsvImport(requireBooks(), statementText.value, csvProfile(), importReview.value)
        : await confirmOfxImport(
            requireBooks(),
            statementText.value,
            statementKind.value as OfxFormat,
            importReview.value,
          );
    importDialogOpen.value = false;
    message.value = `Import complete: ${receipt.createdCount} new, ${receipt.duplicateCount} duplicate.`;
    await loadActivity();
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function loadActivity() {
  if (!props.session.state.isOpen) {
    activityRows.value = [];
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    activityRows.value = (await listBankActivity(requireBooks(), 200, 0)).rows;
    const defaultSelected = activityRows.value
      .filter((row) => row.matchedTransactionId && row.clearanceState === "cleared")
      .map((row) => row.matchedTransactionId as number);
    selectedReconcileTransactions.value = new Set(defaultSelected);
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function openActivity(row: OwnerBankActivityRow) {
  busy.value = true;
  error.value = "";
  matchReview.value = null;
  selectedMatchTransactionId.value = null;
  try {
    selectedActivity.value = row;
    activityDetail.value = await bankActivityDetail(requireBooks(), row.bankActivityId);
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function reviewMatches() {
  if (!selectedActivity.value) return;
  busy.value = true;
  error.value = "";
  try {
    matchReview.value = await reviewPersistedBankActivityMatch(
      requireBooks(),
      selectedActivity.value.bankActivityId,
    );
    selectedMatchTransactionId.value =
      matchReview.value.ambiguousTop ? null : matchReview.value.recommendedTransactionId;
    message.value =
      matchReview.value.candidates.length === 0
        ? "No bounded safe match candidate is currently available."
        : "Deterministic match review complete.";
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function confirmSelectedMatch() {
  if (!selectedActivity.value || !matchReview.value || !selectedMatchTransactionId.value) return;
  busy.value = true;
  error.value = "";
  try {
    await confirmBankMatch(
      requireBooks(),
      selectedActivity.value.bankActivityId,
      matchReview.value.candidates.map((candidate) => candidate.transactionId),
      selectedMatchTransactionId.value,
    );
    matchDialogOpen.value = false;
    message.value = "Bank match confirmed.";
    await loadActivity();
    const refreshed = activityRows.value.find(
      (row) => row.bankActivityId === selectedActivity.value?.bankActivityId,
    );
    if (refreshed) await openActivity(refreshed);
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

const reconciliableRows = computed(() =>
  activityRows.value.filter(
    (row) => row.matchedTransactionId !== null && row.clearanceState === "cleared",
  ),
);

function toggleReconcile(transactionId: number, checked: boolean) {
  const next = new Set(selectedReconcileTransactions.value);
  if (checked) next.add(transactionId);
  else next.delete(transactionId);
  selectedReconcileTransactions.value = next;
}

function handleReconcileToggle(transactionId: number, event: Event) {
  const target = event.target;
  if (!(target instanceof HTMLInputElement)) return;
  toggleReconcile(transactionId, target.checked);
}

function reconciliationTransactionIds(): number[] {
  return [...selectedReconcileTransactions.value].sort((a, b) => a - b);
}

async function reviewReconciliation() {
  busy.value = true;
  error.value = "";
  try {
    const ids = reconciliationTransactionIds();
    if (ids.length === 0) throw new Error("Select at least one cleared matched transaction.");
    reconciliationPreview.value = await previewReconciliation(
      requireBooks(),
      parseSignedPoundsToPence(reconcile.openingBalance),
      parseSignedPoundsToPence(reconcile.endingBalance),
      ids,
    );
    reconciliationDialogOpen.value = true;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function confirmReconciliation() {
  if (!reconciliationPreview.value) return;
  busy.value = true;
  error.value = "";
  try {
    const receipt = await finaliseReconciliation(
      requireBooks(),
      reconcile.statementId.trim(),
      reconcile.statementDate,
      reconciliationPreview.value.openingBalancePence,
      reconciliationPreview.value.expectedEndingBalancePence,
      reconciliationTransactionIds(),
    );
    reconciliationDialogOpen.value = false;
    message.value = receipt.alreadyFinalised
      ? "This exact statement reconciliation was already finalised."
      : "Reconciliation finalised.";
    await loadActivity();
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

watch(
  () => props.session.state.isOpen,
  async () => loadActivity(),
  { immediate: true },
);
</script>

<template>
  <div class="screen-stack">
    <p v-if="!session.state.isOpen" class="empty-banner">
      Open your books from Home before using Bank.
    </p>

    <template v-else>
      <section class="content-grid bank-import-grid">
        <article class="panel">
          <p class="eyebrow">Statement import</p>
          <h2>Review a local statement</h2>
          <p class="subtle">
            The browser reads the selected file. Shark Books receives statement text, never a native path.
          </p>
          <label>
            <span>CSV, OFX or QFX statement</span>
            <input type="file" accept=".csv,.ofx,.qfx,text/csv" @change="selectStatementFile" />
          </label>
          <p v-if="statementFileName" class="runtime-note">{{ statementFileName }}</p>

          <div v-if="statementKind === 'csv' && statementText" class="mapping-grid">
            <label>
              <span>Date column</span>
              <input v-model="csv.dateHeader" />
            </label>
            <label>
              <span>Description column</span>
              <input v-model="csv.descriptionHeader" />
            </label>
            <label>
              <span>Payee column (optional)</span>
              <input v-model="csv.payeeHeader" />
            </label>
            <label>
              <span>Reference column (optional)</span>
              <input v-model="csv.referenceHeader" />
            </label>
            <label>
              <span>Transaction ID column (optional)</span>
              <input v-model="csv.transactionIdHeader" />
            </label>
            <label>
              <span>Date format</span>
              <select v-model="csv.dateFormat">
                <option value="isoYmd">YYYY-MM-DD</option>
                <option value="dmySlash">DD/MM/YYYY</option>
              </select>
            </label>
            <label>
              <span>Amount layout</span>
              <select v-model="csv.amountKind">
                <option value="signed">One signed amount column</option>
                <option value="debitCredit">Separate money out and money in columns</option>
              </select>
            </label>
            <label v-if="csv.amountKind === 'signed'">
              <span>Amount column</span>
              <input v-model="csv.amountHeader" />
            </label>
            <template v-else>
              <label><span>Money out column</span><input v-model="csv.debitHeader" /></label>
              <label><span>Money in column</span><input v-model="csv.creditHeader" /></label>
            </template>
          </div>

          <button
            type="button"
            :data-action-id="
              statementKind === 'csv'
                ? UIA_BINDINGS.bankImportReviewCsv.action.actionId
                : UIA_BINDINGS.bankImportReviewOfxQfx.action.actionId
            "
            :disabled="busy || !statementText"
            @click="reviewStatement"
          >
            Review statement
          </button>
        </article>

        <article class="panel">
          <p class="eyebrow">Import review</p>
          <h2>Lines & duplicates</h2>
          <p v-if="!importReview" class="subtle">Choose and review a statement first.</p>
          <template v-else>
            <p>
              {{ importReview.lines.length }} line(s);
              {{ importReview.duplicateCount }} duplicate(s).
            </p>
            <ul v-if="importReview.errors.length" class="error-list">
              <li v-for="item in importReview.errors" :key="item.lineRef">
                <strong>{{ item.lineRef }}</strong> — {{ item.message }}
              </li>
            </ul>
            <div class="table-scroll" v-if="importReview.lines.length">
              <table class="owner-table">
                <thead>
                  <tr><th>Date</th><th>Description</th><th>Amount</th><th>Duplicate review</th></tr>
                </thead>
                <tbody>
                  <tr v-for="line in importReview.lines" :key="line.lineRef">
                    <td>{{ line.postedDate }}</td>
                    <td>{{ line.description }}</td>
                    <td>{{ formatMoney(line.signedAmountPence, line.currency) }}</td>
                    <td>{{ readableToken(duplicateOutcome(line.lineRef)) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <button
              type="button"
              :data-action-id="
                statementKind === 'csv'
                  ? UIA_BINDINGS.bankImportConfirmCsv.action.actionId
                  : UIA_BINDINGS.bankImportConfirmOfxQfx.action.actionId
              "
              :disabled="busy || !importReview.canConfirm"
              @click="importDialogOpen = true"
            >
              Continue to confirmation
            </button>
          </template>
        </article>
      </section>

      <section class="content-grid bank-activity-grid">
        <article class="panel">
          <div class="panel-heading horizontal">
            <div>
              <p class="eyebrow">Bank activity</p>
              <h2>Imported activity</h2>
            </div>
            <button
              type="button"
              class="secondary compact"
              :data-action-id="UIA_BINDINGS.bankActivityList.action.actionId"
              :disabled="busy"
              @click="loadActivity"
            >
              Refresh
            </button>
          </div>
          <BankActivityTable
            :rows="activityRows"
            :selected-id="selectedActivity?.bankActivityId"
            @select="openActivity"
          />
        </article>

        <article class="panel">
          <p class="eyebrow">Activity detail</p>
          <template v-if="activityDetail">
            <h2>{{ activityDetail.description }}</h2>
            <dl class="detail-list">
              <div><dt>Date</dt><dd>{{ activityDetail.postedDate }}</dd></div>
              <div><dt>Amount</dt><dd>{{ formatMoney(activityDetail.signedAmountPence) }}</dd></div>
              <div v-if="activityDetail.payee"><dt>Payee</dt><dd>{{ activityDetail.payee }}</dd></div>
              <div v-if="activityDetail.reference"><dt>Reference</dt><dd>{{ activityDetail.reference }}</dd></div>
              <div>
                <dt>Status</dt>
                <dd>
                  {{
                    activityDetail.matchedTransactionId
                      ? readableToken(activityDetail.clearanceState ?? "matched")
                      : "Not matched"
                  }}
                </dd>
              </div>
            </dl>
            <button
              v-if="!activityDetail.matchedTransactionId"
              type="button"
              :data-action-id="UIA_BINDINGS.bankMatchReview.action.actionId"
              :disabled="busy"
              @click="reviewMatches"
            >
              Review possible matches
            </button>
          </template>
          <p v-else class="subtle">Choose an imported activity row.</p>

          <div v-if="matchReview" class="match-review">
            <h3>Deterministic match review</h3>
            <p v-if="matchReview.ambiguousTop" class="warning-banner">
              The top result is ambiguous. Shark Books will not allow confirmation from this review.
            </p>
            <p v-if="matchReview.candidates.length === 0" class="subtle">
              No bounded safe candidate is available.
            </p>
            <label
              v-for="candidate in matchReview.candidates"
              :key="candidate.transactionId"
              class="candidate-card"
            >
              <input
                v-model="selectedMatchTransactionId"
                type="radio"
                :value="candidate.transactionId"
                :disabled="matchReview.ambiguousTop || candidate.level === 'unmatched'"
              />
              <span>
                <strong>
                  Transaction {{ candidate.transactionId }} · {{ readableToken(candidate.level) }}
                </strong>
                <small>Score {{ candidate.score }} · {{ candidate.reasons.map(readableToken).join(", ") }}</small>
              </span>
            </label>
            <button
              type="button"
              :data-action-id="UIA_BINDINGS.bankMatchConfirm.action.actionId"
              :disabled="
                busy ||
                matchReview.ambiguousTop ||
                selectedMatchTransactionId === null
              "
              @click="matchDialogOpen = true"
            >
              Continue to match confirmation
            </button>
          </div>
        </article>
      </section>

      <section class="panel">
        <p class="eyebrow">Reconciliation</p>
        <h2>Review a statement balance</h2>
        <p class="subtle">
          Only owner-confirmed, currently cleared Business Bank transactions are available here.
        </p>

        <div class="field-grid">
          <label>
            <span>Statement reference</span>
            <input v-model="reconcile.statementId" autocomplete="off" />
          </label>
          <label>
            <span>Statement date</span>
            <input v-model="reconcile.statementDate" type="date" />
          </label>
          <label>
            <span>Opening balance (£)</span>
            <input v-model="reconcile.openingBalance" inputmode="decimal" />
          </label>
          <label>
            <span>Ending balance (£)</span>
            <input v-model="reconcile.endingBalance" inputmode="decimal" />
          </label>
        </div>

        <fieldset class="reconcile-list">
          <legend>Cleared matched transactions</legend>
          <p v-if="reconciliableRows.length === 0" class="subtle">
            No cleared matched transactions are available.
          </p>
          <label
            v-for="row in reconciliableRows"
            :key="row.bankActivityId"
            class="checkbox-row"
          >
            <input
              type="checkbox"
              :checked="
                row.matchedTransactionId !== null &&
                selectedReconcileTransactions.has(row.matchedTransactionId)
              "
              @change="
                row.matchedTransactionId !== null &&
                handleReconcileToggle(
                  row.matchedTransactionId,
                  $event,
                )
              "
            />
            <span>
              {{ row.postedDate }} · {{ row.description }} ·
              {{ formatMoney(row.signedAmountPence, row.currency) }}
            </span>
          </label>
        </fieldset>

        <button
          type="button"
          :data-action-id="UIA_BINDINGS.bankReconcilePreview.action.actionId"
          :disabled="busy || reconciliableRows.length === 0"
          @click="reviewReconciliation"
        >
          Review reconciliation
        </button>
      </section>

      <p class="operation-message" role="status" aria-live="polite">
        {{ error || message || (busy ? "Working…" : "") }}
      </p>
    </template>

    <ConfirmationDialog
      v-if="importReview"
      v-model:open="importDialogOpen"
      title="Confirm statement import"
      description="Import exactly the statement lines and duplicate outcomes you reviewed. Shark Books will reparse and revalidate current state before saving."
      confirm-label="Import reviewed statement"
      :action-id="
        statementKind === 'csv'
          ? UIA_BINDINGS.bankImportConfirmCsv.action.actionId
          : UIA_BINDINGS.bankImportConfirmOfxQfx.action.actionId
      "
      :busy="busy"
      :confirm-disabled="!importReview.canConfirm"
      @confirm="confirmReviewedImport"
    >
      <p>{{ importReview.lines.length }} line(s), {{ importReview.duplicateCount }} duplicate(s).</p>
    </ConfirmationDialog>

    <ConfirmationDialog
      v-if="matchReview && selectedMatchTransactionId"
      v-model:open="matchDialogOpen"
      title="Confirm bank match"
      description="Bind this bank activity to the selected current bookkeeping transaction. Shark Books will rerun the matcher before recording the match."
      confirm-label="Confirm selected match"
      :action-id="UIA_BINDINGS.bankMatchConfirm.action.actionId"
      :busy="busy"
      :confirm-disabled="matchReview.ambiguousTop"
      @confirm="confirmSelectedMatch"
    >
      <p>Selected transaction: {{ selectedMatchTransactionId }}</p>
    </ConfirmationDialog>

    <ConfirmationDialog
      v-if="reconciliationPreview"
      v-model:open="reconciliationDialogOpen"
      title="Confirm reconciliation"
      description="Finalisation is permitted only if the current backend state is still exactly zero difference and every selected entry remains cleared and owner-matched."
      confirm-label="Finalise reconciliation"
      :action-id="UIA_BINDINGS.bankReconcileFinalise.action.actionId"
      :busy="busy"
      :confirm-disabled="!reconciliationPreview.canFinalise"
      @confirm="confirmReconciliation"
    >
      <dl class="detail-list">
        <div>
          <dt>Difference</dt>
          <dd>{{ formatMoney(reconciliationPreview.differencePence) }}</dd>
        </div>
        <div>
          <dt>All entries cleared</dt>
          <dd>{{ reconciliationPreview.allEntriesCleared ? "Yes" : "No" }}</dd>
        </div>
      </dl>
    </ConfirmationDialog>
  </div>
</template>
