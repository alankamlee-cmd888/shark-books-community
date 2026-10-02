<script setup lang="ts">
import { createColumnHelper, tableFeatures, useTable } from "@tanstack/vue-table";
import type { OwnerQuoteView } from "../lib/tauri";
import { formatMoney, readableToken } from "../lib/format";

const props = defineProps<{ rows: OwnerQuoteView[]; selectedQuoteId?: string | null }>();
const emit = defineEmits<{ select: [row: OwnerQuoteView] }>();
const features = tableFeatures({});
const columnHelper = createColumnHelper<typeof features, OwnerQuoteView>();
const columns = columnHelper.columns([
  columnHelper.accessor("quoteId", { header: "Document" }),
  columnHelper.accessor("customer.displayName", { header: "Customer" }),
  columnHelper.accessor("state", { header: "State" }),
  columnHelper.accessor("totalPence", { header: "Total" }),
  columnHelper.accessor("updatedAt", { header: "Updated" }),
]);
const table = useTable({ features, columns, get data() { return props.rows; } });
</script>

<template>
  <div class="table-scroll" tabindex="0" aria-label="Quotes and estimates">
    <table class="owner-table">
      <thead><tr><th>Document</th><th>Customer</th><th>State</th><th>Total</th><th>Updated</th><th><span class="visually-hidden">View</span></th></tr></thead>
      <tbody>
        <tr v-if="table.getRowModel().rows.length === 0"><td colspan="6" class="empty-cell">No quotes or estimates in this view.</td></tr>
        <tr
          v-for="row in table.getRowModel().rows"
          :key="row.original.quoteId"
          :class="{ selected: row.original.quoteId === selectedQuoteId }"
        >
          <td><strong>{{ row.original.issuedSnapshot?.commercialNumber || row.original.quoteId }}</strong><br /><small>{{ readableToken(row.original.kind) }}</small></td>
          <td>{{ row.original.customer.displayName }}</td>
          <td>{{ readableToken(row.original.state) }}</td>
          <td>{{ formatMoney(row.original.totalPence) }}</td>
          <td>{{ row.original.updatedAt }}</td>
          <td class="action-cell"><button type="button" class="text-button" @click="emit('select', row.original)">View</button></td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
