<script setup lang="ts">
import { createColumnHelper, tableFeatures, useTable } from "@tanstack/vue-table";
import type { OwnerContactView } from "../lib/tauri";
import { readableToken } from "../lib/format";

const props = defineProps<{ rows: OwnerContactView[]; selectedContactId?: string | null }>();
const emit = defineEmits<{ select: [row: OwnerContactView] }>();
const features = tableFeatures({});
const columnHelper = createColumnHelper<typeof features, OwnerContactView>();
const columns = columnHelper.columns([
  columnHelper.accessor("displayName", { header: "Name" }),
  columnHelper.accessor("kind", { header: "Type" }),
  columnHelper.accessor("updatedAt", { header: "Updated" }),
]);
const table = useTable({ features, columns, get data() { return props.rows; } });
</script>

<template>
  <div class="table-scroll" tabindex="0" aria-label="Contacts">
    <table class="owner-table">
      <thead><tr><th>Name</th><th>Type</th><th>Updated</th><th><span class="visually-hidden">Edit</span></th></tr></thead>
      <tbody>
        <tr v-if="table.getRowModel().rows.length === 0"><td colspan="4" class="empty-cell">No contacts in this view.</td></tr>
        <tr v-for="row in table.getRowModel().rows" :key="row.original.contactId" :class="{ selected: row.original.contactId === selectedContactId }">
          <td><strong>{{ row.original.displayName }}</strong></td>
          <td>{{ readableToken(row.original.kind) }}</td>
          <td>{{ row.original.updatedAt }}</td>
          <td class="action-cell"><button type="button" class="text-button" @click="emit('select', row.original)">Edit</button></td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
