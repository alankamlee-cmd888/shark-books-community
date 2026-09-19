<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { BooksSession } from "../lib/session";
import { errorMessage } from "../lib/format";
import { UIB_BINDINGS } from "../lib/action-bindings";
import { booksInfo, selectStorageRoot, type OwnerSettingsBooksInfo } from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();
const info = ref<OwnerSettingsBooksInfo | null>(null);
const busy = ref(false);
const error = ref("");
const message = ref("");
async function refresh(): Promise<void> {
  if (!props.session.state.isOpen) return;
  busy.value = true; error.value = "";
  try { info.value = await booksInfo(props.session.requireContext()); message.value = "Books information refreshed."; }
  catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}
async function chooseStorageFolder(): Promise<void> {
  busy.value = true; error.value = "";
  try {
    const outcome = await selectStorageRoot(props.session.requireContext());
    if (outcome.status === "registered") {
      props.session.setStorageRoot(outcome.storageRootId, outcome.label);
      message.value = outcome.label;
    } else { message.value = "Folder selection cancelled."; }
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}
onMounted(refresh);
</script>

<template>
  <div class="screen-stack">
    <section v-if="!session.state.isOpen" class="panel empty-banner">Open your books from Home before using Settings.</section>
    <template v-else>
      <section class="content-grid settings-grid">
        <article class="panel">
          <div class="panel-heading horizontal"><div><p class="eyebrow">Books</p><h2>Books information</h2></div><button type="button" class="secondary compact" :data-action-id="UIB_BINDINGS.settingsBooksInfo.action.actionId" :disabled="busy" @click="refresh">Refresh</button></div>
          <dl v-if="info" class="detail-list">
            <div><dt>Business</dt><dd>{{ info.companyName }}</dd></div>
            <div><dt>Books update status</dt><dd>{{ info.migrationRequired ? "Update required before normal use" : "Current" }}</dd></div>
            <div><dt>Protected native storage</dt><dd>{{ info.encryptedNativeSessionActive ? "Active" : "Not active" }}</dd></div>
            <div><dt>Existing-books backup requirement</dt><dd>{{ info.backupBeforeExistingOpenRequired ? "Backup required before update/open changes" : "No additional backup step reported" }}</dd></div>
          </dl>
        </article>
        <article class="panel">
          <p class="eyebrow">Documents</p><h2>Storage folder</h2>
          <p>{{ session.state.storageRootLabel }}</p>
          <p class="subtle">Folder selection is native and session-scoped. The owner interface does not construct or display a native path.</p>
          <button type="button" :data-action-id="UIB_BINDINGS.settingsStorageRoot.action.actionId" :disabled="busy" @click="chooseStorageFolder">Choose document storage folder</button>
        </article>
      </section>
      <p class="operation-message" role="status" aria-live="polite">{{ error || message || (busy ? "Working…" : "") }}</p>
    </template>
  </div>
</template>
