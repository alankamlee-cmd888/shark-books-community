<script setup lang="ts">
import type {
  OwnerBankActivityRow,
  OwnerCommandAttention,
  OwnerHomeStatus,
} from "../lib/tauri";

const props = defineProps<{
  home: OwnerHomeStatus | null;
  bankRows: OwnerBankActivityRow[];
  booksOpen: boolean;
  commandAttention: OwnerCommandAttention | null;
}>();

function unmatchedCount(): number {
  return props.bankRows.filter((row) => row.matchedTransactionId === null).length;
}
</script>

<template>
  <section class="panel attention-panel" aria-labelledby="attention-heading">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">Attention</p>
        <h2 id="attention-heading">What needs a look</h2>
      </div>
    </div>

    <ul
      v-if="commandAttention || (booksOpen && home)"
      class="attention-list"
    >
      <li v-if="commandAttention">
        <strong>{{ commandAttention.title }}</strong>
        <span>{{ commandAttention.summary }}</span>
      </li>

      <template v-if="booksOpen && home">
        <li v-if="!home.booksBalanced">
          <strong>Books check</strong>
          <span>The current books status is not balanced. Use Check books before continuing.</span>
        </li>
        <li v-if="unmatchedCount() > 0">
          <strong>Bank activity</strong>
          <span>{{ unmatchedCount() }} imported bank item(s) are not matched yet.</span>
        </li>
        <li v-if="home.transactionCount === 0">
          <strong>No records yet</strong>
          <span>Add Money in or Money out when you are ready.</span>
        </li>
        <li
          v-if="
            !commandAttention &&
            home.booksBalanced &&
            unmatchedCount() === 0 &&
            home.transactionCount > 0
          "
        >
          <strong>No current exceptions</strong>
          <span>The factual checks available in the owner UI do not show an item needing attention.</span>
        </li>
      </template>
    </ul>

    <p v-else class="subtle">
      Open your books or type a finite command to see factual Attention items.
    </p>
  </section>
</template>
