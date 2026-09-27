<script setup lang="ts">
import { createColumnHelper, tableFeatures, useTable } from "@tanstack/vue-table";
import type { OwnerMoneyRecordRead } from "../lib/tauri";
import { formatMoney } from "../lib/format";

const props = defineProps<{
  rows: OwnerMoneyRecordRead[];
  selectedRecordId?: string | null;
}>();

const emit = defineEmits<{
  select: [row: OwnerMoneyRecordRead];
}>();

const features = tableFeatures({});
const columnHelper = createColumnHelper<typeof features, OwnerMoneyRecordRead>();
const columns = columnHelper.columns([
  columnHelper.accessor("date", { header: "Date" }),
  columnHelper.accessor("description", { header: "Description" }),
  columnHelper.accessor("amountPence", { header: "Amount" }),
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
  <div class="table-scroll" tabindex="0" aria-label="Money records">
    <table class="owner-table">
      <thead>
        <tr>
          <th scope="col">Date</th>
          <th scope="col">Description</th>
          <th scope="col">Amount</th>
          <th scope="col"><span class="visually-hidden">Open record</span></th>
        </tr>
      </thead>
      <tbody>
        <tr v-if="table.getRowModel().rows.length === 0">
          <td colspan="4" class="empty-cell">No current records.</td>
        </tr>
        <tr
          v-for="row in table.getRowModel().rows"
          :key="row.original.recordId"
          :class="{ selected: row.original.recordId === selectedRecordId }"
        >
          <td>{{ row.original.date }}</td>
          <td>
            <strong>{{ row.original.description }}</strong>
            <span class="subtle">{{ row.original.recordId }}</span>
          </td>
          <td class="amount">
            {{ formatMoney(row.original.amountPence, row.original.currency) }}
          </td>
          <td class="action-cell">
            <button type="button" class="text-button" @click="emit('select', row.original)">
              Open
            </button>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
