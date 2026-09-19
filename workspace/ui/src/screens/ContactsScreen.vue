<script setup lang="ts">
import { onMounted, ref } from "vue";
import ContactsTable from "../components/ContactsTable.vue";
import type { BooksSession } from "../lib/session";
import { errorMessage } from "../lib/format";
import { UIB_BINDINGS } from "../lib/action-bindings";
import { listContacts, saveContact, type OwnerContactKind, type OwnerContactView } from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();
const contacts = ref<OwnerContactView[]>([]);
const filter = ref<"" | OwnerContactKind>("");
const selected = ref<OwnerContactView | null>(null);
const kind = ref<OwnerContactKind>("customer");
const displayName = ref("");
const busy = ref(false);
const message = ref("");
const error = ref("");

function newContact(): void { selected.value = null; kind.value = "customer"; displayName.value = ""; }
function editContact(contact: OwnerContactView): void { selected.value = contact; kind.value = contact.kind; displayName.value = contact.displayName; }
function generatedContactId(): string { return `contact-${kind.value}-${Date.now().toString(36)}`; }

async function refresh(): Promise<void> {
  if (!props.session.state.isOpen) return;
  busy.value = true; error.value = "";
  try { contacts.value = (await listContacts(props.session.requireContext(), filter.value || undefined)).contacts; }
  catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}
async function save(): Promise<void> {
  const name = displayName.value.trim();
  if (!name) { error.value = "Enter a contact name."; return; }
  busy.value = true; error.value = "";
  try {
    const outcome = await saveContact(props.session.requireContext(), selected.value?.contactId || generatedContactId(), kind.value, name);
    message.value = outcome.status === "created" ? "Contact created." : outcome.status === "updated" ? "Contact updated." : "Contact is already current.";
    await refresh(); newContact();
  } catch (err) { error.value = errorMessage(err); }
  finally { busy.value = false; }
}
onMounted(refresh);
</script>

<template>
  <div class="screen-stack">
    <section v-if="!session.state.isOpen" class="panel empty-banner">Open your books from Home before using Contacts.</section>
    <template v-else>
      <section class="content-grid contact-layout">
        <article class="panel">
          <div class="panel-heading horizontal"><div><p class="eyebrow">Customers & suppliers</p><h2>Contacts</h2></div><button type="button" class="secondary compact" :data-action-id="UIB_BINDINGS.contactsList.action.actionId" :disabled="busy" @click="refresh">Refresh</button></div>
          <label><span>Show</span><select v-model="filter" @change="refresh"><option value="">All contacts</option><option value="customer">Customers</option><option value="supplier">Suppliers</option></select></label>
          <ContactsTable :rows="contacts" :selected-contact-id="selected?.contactId" @select="editContact" />
        </article>
        <article class="panel">
          <p class="eyebrow">{{ selected ? "Edit contact" : "New contact" }}</p><h2>{{ selected ? selected.displayName : "Add customer or supplier" }}</h2>
          <div class="field-grid">
            <label><span>Type</span><select v-model="kind" :disabled="Boolean(selected)"><option value="customer">Customer</option><option value="supplier">Supplier</option></select></label>
            <label><span>Display name</span><input v-model="displayName" autocomplete="organization" /></label>
          </div>
          <div class="button-row"><button type="button" :data-action-id="UIB_BINDINGS.contactsSave.action.actionId" :disabled="busy || !displayName.trim()" @click="save">{{ selected ? "Save changes" : "Create contact" }}</button><button v-if="selected" type="button" class="secondary" @click="newContact">Cancel edit</button></div>
          <p class="subtle">Contacts store only the bounded customer/supplier identity used by this first owner-facing release.</p>
        </article>
      </section>
      <p class="operation-message" role="status" aria-live="polite">{{ error || message || (busy ? "Working…" : "") }}</p>
    </template>
  </div>
</template>
