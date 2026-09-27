<script setup lang="ts">
import { createColumnHelper, tableFeatures, useTable } from "@tanstack/vue-table";
import type { OwnerBankActivityRow } from "../lib/tauri";
import { formatMoney, readableToken } from "../lib/format";

const props = defineProps<{
  rows: OwnerBankActivityRow[];
  selectedId?: number | null;
}>();

const emit = defineEmits<{
  select: [row: OwnerBankActivityRow];
}>();

const features = tableFeatures({});
const columnHelper = createColumnHelper<typeof features, OwnerBankActivityRow>();
const columns = columnHelper.columns([
  columnHelper.accessor("postedDate", { header: "Date" }),
  columnHelper.accessor("description", { header: "Description" }),
  columnHelper.accessor("signedAmountPence", { header: "Amount" }),
  columnHelper.accessor("clearanceState", { header: "State" }),
]);

const table = useTable({
  features,
  columns,
  get data() {
    return props.rows;
  },
});
</script>

<template>
  <div class="table-scroll" tabindex="0" aria-label="Bank activity">
    <table class="owner-table">
      <thead>
        <tr>
          <th scope="col">Date</th>
          <th scope="col">Description</th>
          <th scope="col">Amount</th>
          <th scope="col">Status</th>
          <th scope="col"><span class="visually-hidden">Open activity</span></th>
        </tr>
      </thead>
      <tbody>
        <tr v-if="table.getRowModel().rows.length === 0">
          <td colspan="5" class="empty-cell">No imported bank activity.</td>
        </tr>
        <tr
          v-for="row in table.getRowModel().rows"
          :key="row.original.bankActivityId"
          :class="{ selected: row.original.bankActivityId === selectedId }"
        >
          <td>{{ row.original.postedDate }}</td>
          <td>
            <strong>{{ row.original.description }}</strong>
            <span v-if="row.original.payee" class="subtle">{{ row.original.payee }}</span>
          </td>
          <td class="amount">
            {{ formatMoney(row.original.signedAmountPence, row.original.currency) }}
          </td>
          <td>
            {{
              row.original.matchedTransactionId
                ? readableToken(row.original.clearanceState ?? "matched")
                : "Not matched"
            }}
          </td>
          <td class="action-cell">
            <button type="button" class="text-button" @click="emit('select', row.original)">
              Review
            </button>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
