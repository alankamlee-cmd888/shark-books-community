<script setup lang="ts">
import { createColumnHelper, tableFeatures, useTable } from "@tanstack/vue-table";
import type { OwnerDocumentRead } from "../lib/tauri";

const props = defineProps<{ rows: OwnerDocumentRead[]; selectedDocumentId?: string | null }>();
const emit = defineEmits<{ select: [row: OwnerDocumentRead] }>();
const features = tableFeatures({});
const columnHelper = createColumnHelper<typeof features, OwnerDocumentRead>();
const columns = columnHelper.columns([
  columnHelper.accessor("registeredAt", { header: "Added" }),
  columnHelper.accessor("originalFilename", { header: "Document" }),
  columnHelper.accessor("mediaType", { header: "Type" }),
  columnHelper.accessor("byteLen", { header: "Size" }),
]);
const table = useTable({ features, columns, get data() { return props.rows; } });
function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
</script>

<template>
  <div class="table-scroll" tabindex="0" aria-label="Receipts and documents">
    <table class="owner-table">
      <thead><tr><th>Added</th><th>Document</th><th>Type</th><th>Size</th><th><span class="visually-hidden">Open</span></th></tr></thead>
      <tbody>
        <tr v-if="table.getRowModel().rows.length === 0"><td colspan="5" class="empty-cell">No documents yet.</td></tr>
        <tr v-for="row in table.getRowModel().rows" :key="row.original.documentId" :class="{ selected: row.original.documentId === selectedDocumentId }">
          <td>{{ row.original.registeredAt }}</td>
          <td><strong>{{ row.original.originalFilename }}</strong></td>
          <td>{{ row.original.mediaType || "Unknown" }}</td>
          <td>{{ sizeLabel(row.original.byteLen) }}</td>
          <td class="action-cell"><button type="button" class="text-button" @click="emit('select', row.original)">Open</button></td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
