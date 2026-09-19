<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import ConfirmationDialog from "../components/ConfirmationDialog.vue";
import MoneyTable from "../components/MoneyTable.vue";
import { UIA_BINDINGS } from "../lib/action-bindings";
import {
  businessPercentToBasisPoints,
  derivedRecordId,
  errorMessage,
  EXPENSE_CATEGORIES,
  formatMoney,
  INCOME_CATEGORIES,
  parsePoundsToPence,
  penceToInput,
  SETTLEMENTS,
} from "../lib/format";
import type { BooksSession } from "../lib/session";
import {
  confirmCorrection,
  correctionHistory,
  listMoneyRecords,
  moneyRecordDetail,
  previewCorrection,
  previewMoneyIn,
  previewMoneyOut,
  saveMoneyIn,
  saveMoneyOut,
  type BusinessUse,
  type CorrectionReplacement,
  type ExpenseCategory,
  type IncomeCategory,
  type OwnerCorrectionHistoryItem,
  type OwnerCorrectionPreview,
  type OwnerCorrectionRequest,
  type OwnerMoneyInRequest,
  type OwnerMoneyOutRequest,
  type OwnerMoneyRecordDetailRead,
  type OwnerMoneyRecordRead,
  type OwnerPostingPreview,
  type SettlementAccount,
} from "../lib/tauri";

const props = defineProps<{
  session: BooksSession;
  kind: "moneyIn" | "moneyOut";
}>();

const rows = ref<OwnerMoneyRecordRead[]>([]);
const detail = ref<OwnerMoneyRecordDetailRead | null>(null);
const history = ref<OwnerCorrectionHistoryItem[]>([]);
const busy = ref(false);
const message = ref("");
const error = ref("");

const createForm = reactive({
  recordId: "",
  description: "",
  date: new Date().toISOString().slice(0, 10),
  amount: "",
  incomeCategory: "salesTrading" as IncomeCategory,
  expenseCategory: "otherBusinessExpense" as ExpenseCategory,
  settlement: "businessBank" as SettlementAccount,
  businessUse: "business" as "business" | "private" | "mixed",
  businessPercent: "100",
});

const preview = ref<OwnerPostingPreview | null>(null);
const pendingMoneyRequest = ref<OwnerMoneyInRequest | OwnerMoneyOutRequest | null>(null);
const saveDialogOpen = ref(false);

const correctionOpen = ref(false);
const correctionForm = reactive({
  removeInstead: false,
  reason: "",
  description: "",
  date: "",
  amount: "",
  incomeCategory: "salesTrading" as IncomeCategory,
  expenseCategory: "otherBusinessExpense" as ExpenseCategory,
  settlement: "businessBank" as SettlementAccount,
  businessUse: "business" as "business" | "private" | "mixed",
  businessPercent: "100",
  reversalRecordId: "",
  replacementRecordId: "",
});
const correctionPreview = ref<OwnerCorrectionPreview | null>(null);
const pendingCorrection = ref<OwnerCorrectionRequest | null>(null);
const correctionDialogOpen = ref(false);

const title = computed(() => (props.kind === "moneyIn" ? "Money in" : "Money out"));
function requireBooks() {
  return props.session.requireContext();
}

function resetCreateForm() {
  createForm.recordId = "";
  createForm.description = "";
  createForm.date = new Date().toISOString().slice(0, 10);
  createForm.amount = "";
  createForm.businessUse = "business";
  createForm.businessPercent = "100";
}

function businessUse(): BusinessUse {
  if (createForm.businessUse === "business") return { kind: "business" };
  if (createForm.businessUse === "private") return { kind: "private" };
  return {
    kind: "mixed",
    businessBasisPoints: businessPercentToBasisPoints(createForm.businessPercent),
  };
}

async function loadRows() {
  if (!props.session.state.isOpen) {
    rows.value = [];
    detail.value = null;
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    rows.value = (await listMoneyRecords(requireBooks(), props.kind, 100, 0)).rows;
    if (detail.value && !rows.value.some((row) => row.recordId === detail.value?.record.recordId)) {
      detail.value = null;
      history.value = [];
    }
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function openRecord(row: OwnerMoneyRecordRead) {
  busy.value = true;
  error.value = "";
  try {
    detail.value = (await moneyRecordDetail(requireBooks(), props.kind, row.recordId)).detail;
    history.value = (
      await correctionHistory(requireBooks(), props.kind, row.recordId, 100)
    ).items;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

function buildMoneyRequest(): OwnerMoneyInRequest | OwnerMoneyOutRequest {
  const amountPence = parsePoundsToPence(createForm.amount);
  const recordId = createForm.recordId.trim();
  const description = createForm.description.trim();
  if (!recordId) throw new Error("Enter a record reference.");
  if (!description) throw new Error("Enter a description.");
  if (!createForm.date) throw new Error("Choose a date.");

  if (props.kind === "moneyIn") {
    return {
      books: requireBooks(),
      recordId,
      description,
      date: createForm.date,
      amountPence,
      category: createForm.incomeCategory,
      settlement: createForm.settlement,
    };
  }
  return {
    books: requireBooks(),
    recordId,
    description,
    date: createForm.date,
    amountPence,
    category: createForm.expenseCategory,
    businessUse: businessUse(),
    settlement: createForm.settlement,
  };
}

async function reviewMoney() {
  busy.value = true;
  error.value = "";
  message.value = "";
  try {
    const request = buildMoneyRequest();
    pendingMoneyRequest.value = request;
    preview.value =
      props.kind === "moneyIn"
        ? await previewMoneyIn(request as OwnerMoneyInRequest)
        : await previewMoneyOut(request as OwnerMoneyOutRequest);
    saveDialogOpen.value = true;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function saveReviewedMoney() {
  if (!pendingMoneyRequest.value || !preview.value) return;
  busy.value = true;
  error.value = "";
  try {
    const receipt =
      props.kind === "moneyIn"
        ? await saveMoneyIn(pendingMoneyRequest.value as OwnerMoneyInRequest)
        : await saveMoneyOut(pendingMoneyRequest.value as OwnerMoneyOutRequest);
    saveDialogOpen.value = false;
    message.value = receipt.alreadyRecorded
      ? "That record was already present; nothing was duplicated."
      : "Record saved.";
    resetCreateForm();
    preview.value = null;
    pendingMoneyRequest.value = null;
    await loadRows();
    await props.session.refreshHome();
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

function startCorrection() {
  if (!detail.value) return;
  const row = detail.value.record;
  correctionForm.removeInstead = false;
  correctionForm.reason = "";
  correctionForm.description = row.description;
  correctionForm.date = row.date;
  correctionForm.amount = penceToInput(row.amountPence);
  correctionForm.settlement = "businessBank";
  correctionForm.businessUse = "business";
  correctionForm.businessPercent = "100";
  correctionForm.reversalRecordId = derivedRecordId("reversal", row.recordId);
  correctionForm.replacementRecordId = derivedRecordId("corrected", row.recordId);
  correctionOpen.value = true;
  correctionPreview.value = null;
}

function correctionBusinessUse(): BusinessUse {
  if (correctionForm.businessUse === "business") return { kind: "business" };
  if (correctionForm.businessUse === "private") return { kind: "private" };
  return {
    kind: "mixed",
    businessBasisPoints: businessPercentToBasisPoints(correctionForm.businessPercent),
  };
}

function buildCorrection(): OwnerCorrectionRequest {
  if (!detail.value) throw new Error("Open a current record before correcting it.");
  const original = detail.value.record;
  const reason = correctionForm.reason.trim();
  if (!reason) throw new Error("Enter the reason for the correction.");

  let replacement: CorrectionReplacement | null = null;
  if (!correctionForm.removeInstead) {
    const amountPence = parsePoundsToPence(correctionForm.amount);
    if (props.kind === "moneyIn") {
      replacement = {
        kind: "moneyIn",
        recordId: correctionForm.replacementRecordId,
        description: correctionForm.description.trim(),
        date: correctionForm.date,
        amountPence,
        category: correctionForm.incomeCategory,
        settlement: correctionForm.settlement,
      };
    } else {
      replacement = {
        kind: "moneyOut",
        recordId: correctionForm.replacementRecordId,
        description: correctionForm.description.trim(),
        date: correctionForm.date,
        amountPence,
        category: correctionForm.expenseCategory,
        businessUse: correctionBusinessUse(),
        settlement: correctionForm.settlement,
      };
    }
  }
  return {
    books: requireBooks(),
    recordKind: props.kind,
    originalRecordId: original.recordId,
    reversalRecordId: correctionForm.reversalRecordId,
    replacement,
    reason,
  };
}

async function reviewCorrection() {
  busy.value = true;
  error.value = "";
  try {
    const request = buildCorrection();
    pendingCorrection.value = request;
    correctionPreview.value = await previewCorrection(request);
    correctionDialogOpen.value = true;
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function confirmReviewedCorrection() {
  if (!pendingCorrection.value || !correctionPreview.value) return;
  busy.value = true;
  error.value = "";
  try {
    const receipt = await confirmCorrection(
      pendingCorrection.value,
      correctionPreview.value.previewFingerprint,
    );
    correctionDialogOpen.value = false;
    correctionOpen.value = false;
    message.value = receipt.alreadyApplied
      ? "This exact correction was already applied."
      : "Correction applied. The original record remains in the audit history.";
    detail.value = null;
    history.value = [];
    await loadRows();
    await props.session.refreshHome();
  } catch (cause) {
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

watch(
  [() => props.kind, () => props.session.state.isOpen],
  async () => {
    detail.value = null;
    history.value = [];
    resetCreateForm();
    await loadRows();
  },
  { immediate: true },
);
</script>

<template>
  <div class="screen-stack">
    <p v-if="!session.state.isOpen" class="empty-banner">
      Open your books from Home before using {{ title }}.
    </p>

    <template v-else>
      <section class="content-grid money-layout">
        <article class="panel">
          <div class="panel-heading horizontal">
            <div>
              <p class="eyebrow">Current records</p>
              <h2>{{ title }}</h2>
            </div>
            <button
              type="button"
              class="secondary compact"
              :data-action-id="UIA_BINDINGS.moneyRecordsList.action.actionId"
              :disabled="busy"
              @click="loadRows"
            >
              Refresh
            </button>
          </div>
          <MoneyTable
            :rows="rows"
            :selected-record-id="detail?.record.recordId"
            @select="openRecord"
          />
        </article>

        <article class="panel">
          <p class="eyebrow">Add record</p>
          <h2>Record {{ title.toLowerCase() }}</h2>
          <div class="field-grid">
            <label>
              <span>Record reference</span>
              <input v-model="createForm.recordId" autocomplete="off" />
            </label>
            <label>
              <span>Date</span>
              <input v-model="createForm.date" type="date" />
            </label>
            <label class="field-span">
              <span>Description</span>
              <input v-model="createForm.description" autocomplete="off" />
            </label>
            <label>
              <span>Amount (£)</span>
              <input v-model="createForm.amount" inputmode="decimal" placeholder="0.00" />
            </label>
            <label>
              <span>Category</span>
              <select
                v-if="kind === 'moneyIn'"
                v-model="createForm.incomeCategory"
              >
                <option v-for="[value, label] in INCOME_CATEGORIES" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
              <select v-else v-model="createForm.expenseCategory">
                <option v-for="[value, label] in EXPENSE_CATEGORIES" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
            </label>
            <label>
              <span>Received / paid via</span>
              <select v-model="createForm.settlement">
                <option v-for="[value, label] in SETTLEMENTS" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
            </label>

            <template v-if="kind === 'moneyOut'">
              <label>
                <span>Business use</span>
                <select v-model="createForm.businessUse">
                  <option value="business">Business</option>
                  <option value="private">Private</option>
                  <option value="mixed">Mixed</option>
                </select>
              </label>
              <label v-if="createForm.businessUse === 'mixed'">
                <span>Business percentage</span>
                <input
                  v-model="createForm.businessPercent"
                  inputmode="decimal"
                  placeholder="50"
                />
              </label>
            </template>
          </div>
          <button
            type="button"
            :data-action-id="
              kind === 'moneyIn'
                ? UIA_BINDINGS.moneyInPreview.action.actionId
                : UIA_BINDINGS.moneyOutPreview.action.actionId
            "
            :disabled="busy"
            @click="reviewMoney"
          >
            Review before saving
          </button>
        </article>
      </section>

      <section v-if="detail" class="content-grid detail-grid">
        <article class="panel">
          <p class="eyebrow">Record detail</p>
          <h2>{{ detail.record.description }}</h2>
          <dl class="detail-list">
            <div><dt>Date</dt><dd>{{ detail.record.date }}</dd></div>
            <div><dt>Amount</dt><dd>{{ formatMoney(detail.record.amountPence) }}</dd></div>
            <div><dt>Reference</dt><dd>{{ detail.record.recordId }}</dd></div>
            <div>
              <dt>Documents</dt>
              <dd>
                {{ detail.documents.length === 0 ? "None attached" : `${detail.documents.length} attached` }}
              </dd>
            </div>
          </dl>
          <button
            type="button"
            class="secondary"
            :data-action-id="UIA_BINDINGS.correctionPreview.action.actionId"
            @click="startCorrection"
          >
            Correct this record
          </button>
        </article>

        <article class="panel">
          <p class="eyebrow">Correction history</p>
          <h2>Immutable history</h2>
          <p v-if="history.length === 0" class="subtle">No corrections recorded.</p>
          <ul v-else class="timeline-list">
            <li v-for="item in history" :key="item.correctionId">
              <strong>{{ item.reason }}</strong>
              <span>{{ item.correctedAt }}</span>
              <span v-if="item.replacementRecordId">
                Replacement: {{ item.replacementRecordId }}
              </span>
            </li>
          </ul>
        </article>
      </section>

      <section v-if="correctionOpen && detail" class="panel">
        <p class="eyebrow">Correction</p>
        <h2>Prepare an append-only correction</h2>
        <p class="subtle">
          The original record is not edited. Shark Books previews a reversal and optional replacement.
          Re-enter the corrected category, payment route and business-use facts; they are not inferred
          from the prior accounting entries.
        </p>
        <div class="field-grid">
          <label class="field-span">
            <span>Reason</span>
            <input v-model="correctionForm.reason" autocomplete="off" />
          </label>
          <label class="checkbox-row field-span">
            <input v-model="correctionForm.removeInstead" type="checkbox" />
            <span>Remove the current record without a replacement</span>
          </label>
          <template v-if="!correctionForm.removeInstead">
            <label>
              <span>Date</span>
              <input v-model="correctionForm.date" type="date" />
            </label>
            <label>
              <span>Amount (£)</span>
              <input v-model="correctionForm.amount" inputmode="decimal" />
            </label>
            <label class="field-span">
              <span>Description</span>
              <input v-model="correctionForm.description" />
            </label>
            <label>
              <span>Category</span>
              <select
                v-if="kind === 'moneyIn'"
                v-model="correctionForm.incomeCategory"
              >
                <option v-for="[value, label] in INCOME_CATEGORIES" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
              <select v-else v-model="correctionForm.expenseCategory">
                <option v-for="[value, label] in EXPENSE_CATEGORIES" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
            </label>
            <label>
              <span>Received / paid via</span>
              <select v-model="correctionForm.settlement">
                <option v-for="[value, label] in SETTLEMENTS" :key="value" :value="value">
                  {{ label }}
                </option>
              </select>
            </label>
            <template v-if="kind === 'moneyOut'">
              <label>
                <span>Business use</span>
                <select v-model="correctionForm.businessUse">
                  <option value="business">Business</option>
                  <option value="private">Private</option>
                  <option value="mixed">Mixed</option>
                </select>
              </label>
              <label v-if="correctionForm.businessUse === 'mixed'">
                <span>Business percentage</span>
                <input v-model="correctionForm.businessPercent" inputmode="decimal" />
              </label>
            </template>
          </template>
        </div>
        <div class="button-row">
          <button
            type="button"
            :data-action-id="UIA_BINDINGS.correctionPreview.action.actionId"
            :disabled="busy"
            @click="reviewCorrection"
          >
            Review correction
          </button>
          <button type="button" class="secondary" @click="correctionOpen = false">
            Cancel
          </button>
        </div>
      </section>

      <p class="operation-message" role="status" aria-live="polite">
        {{ error || message || (busy ? "Working…" : "") }}
      </p>
    </template>

    <ConfirmationDialog
      v-if="preview"
      v-model:open="saveDialogOpen"
      :title="`Confirm ${title.toLowerCase()}`"
      description="Save exactly the reviewed owner facts. Shark Books will re-run the deterministic posting plan."
      confirm-label="Save record"
      :action-id="
        kind === 'moneyIn'
          ? UIA_BINDINGS.moneyInSave.action.actionId
          : UIA_BINDINGS.moneyOutSave.action.actionId
      "
      :busy="busy"
      @confirm="saveReviewedMoney"
    >
      <dl class="detail-list">
        <div><dt>Amount</dt><dd>{{ formatMoney(preview.amountPence) }}</dd></div>
        <div><dt>Business amount</dt><dd>{{ formatMoney(preview.businessAmountPence) }}</dd></div>
        <div><dt>Private amount</dt><dd>{{ formatMoney(preview.privateAmountPence) }}</dd></div>
      </dl>
    </ConfirmationDialog>

    <ConfirmationDialog
      v-if="correctionPreview"
      v-model:open="correctionDialogOpen"
      title="Confirm correction"
      description="This creates an append-only reversal and the reviewed replacement, if any. The original audit record remains."
      confirm-label="Apply correction"
      :action-id="UIA_BINDINGS.correctionConfirm.action.actionId"
      :busy="busy"
      @confirm="confirmReviewedCorrection"
    >
      <dl class="detail-list">
        <div>
          <dt>Original</dt>
          <dd>{{ formatMoney(correctionPreview.original.amountPence) }}</dd>
        </div>
        <div>
          <dt>Reversal</dt>
          <dd>{{ formatMoney(correctionPreview.reversal.amountPence) }}</dd>
        </div>
        <div v-if="correctionPreview.replacement">
          <dt>Replacement</dt>
          <dd>{{ formatMoney(correctionPreview.replacement.amountPence) }}</dd>
        </div>
      </dl>
    </ConfirmationDialog>
  </div>
</template>
