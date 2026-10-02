<script setup lang="ts">
import { formatMoney, readableToken } from "../lib/format";

interface CommercialRow {
  id: string;
  number: string | null;
  kind: "invoice" | "creditNote";
  customer: string;
  state: string;
  totalPence: number;
  outstandingPence: number | null;
}

defineProps<{ rows: CommercialRow[]; selectedId?: string; busy: boolean }>();
defineEmits<{ select: [row: CommercialRow] }>();

function money(pence: number): string {
  return Number.isSafeInteger(pence) ? formatMoney(pence) : "Outside display range";
}
</script>

<template>
  <div class="table-scroll" tabindex="0" aria-label="Invoices and credit notes">
    <table class="owner-table">
      <thead>
        <tr><th>Number / draft</th><th>Type</th><th>Customer</th><th>State</th><th>Total</th><th>Outstanding</th></tr>
      </thead>
      <tbody>
        <tr v-if="!rows.length"><td colspan="6" class="empty-cell">No invoices or credit notes yet.</td></tr>
        <tr v-for="row in rows" :key="`${row.kind}:${row.id}`" :aria-selected="row.id === selectedId">
          <td><button type="button" class="text-button" :disabled="busy" @click="$emit('select', row)">{{ row.number || row.id }}</button></td>
          <td>{{ readableToken(row.kind) }}</td>
          <td>{{ row.customer }}</td>
          <td>{{ readableToken(row.state) }}</td>
          <td>{{ money(row.totalPence) }}</td>
          <td>{{ row.outstandingPence === null ? "Credit document" : money(row.outstandingPence) }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
