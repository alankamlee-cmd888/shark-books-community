<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import AttentionPanel from "../components/AttentionPanel.vue";
import CommandPanel from "../components/CommandPanel.vue";
import { UIA_BINDINGS } from "../lib/action-bindings";
import type { CommandSection } from "../lib/command-routes";
import { errorMessage } from "../lib/format";
import type { BooksSession } from "../lib/session";
import {
  foundationHealth,
  listBankActivity,
  type OwnerBankActivityRow,
  type OwnerCommandAttention,
} from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();
const emit = defineEmits<{ navigate: [section: CommandSection] }>();

const bridgeStatus = ref("Checking native bridge…");
const bankRows = ref<OwnerBankActivityRow[]>([]);
const attentionError = ref("");
const commandAttention = ref<OwnerCommandAttention | null>(null);

async function loadAttention() {
  attentionError.value = "";
  if (!props.session.state.isOpen) {
    bankRows.value = [];
    return;
  }
  try {
    await props.session.refreshHome();
    bankRows.value = (await listBankActivity(props.session.requireContext(), 200, 0)).rows;
  } catch (error) {
    attentionError.value = errorMessage(error);
  }
}

onMounted(async () => {
  try {
    bridgeStatus.value = `Native bridge: ${await foundationHealth()}`;
  } catch {
    bridgeStatus.value = "Native bridge unavailable";
  }
  await loadAttention();
});

watch(
  () => props.session.state.isOpen,
  async () => loadAttention(),
);
</script>

<template>
  <div class="screen-stack">
    <section class="hero-grid">
      <article class="panel">
        <p class="eyebrow">Books</p>
        <h2>Start with your books</h2>
        <p class="subtle">
          Your Books name identifies this local set of books. Technical file and actor details
          stay inside the application.
        </p>

        <div class="field-grid">
          <label>
            <span>Books name</span>
            <input
              v-model="session.state.booksName"
              autocomplete="off"
              :disabled="session.state.busy"
            />
          </label>
          <label>
            <span>Business name</span>
            <input
              v-model="session.state.businessName"
              autocomplete="organization"
              :disabled="session.state.busy"
            />
          </label>
        </div>

        <div class="button-row">
          <button
            type="button"
            :data-action-id="UIA_BINDINGS.booksCreate.action.actionId"
            :disabled="session.state.busy"
            @click="session.create().then(loadAttention)"
          >
            Create books
          </button>
          <button
            type="button"
            class="secondary"
            :data-action-id="UIA_BINDINGS.booksOpen.action.actionId"
            :disabled="session.state.busy"
            @click="session.open().then(loadAttention)"
          >
            Open existing books
          </button>
          <button
            v-if="session.state.isOpen"
            type="button"
            class="secondary"
            :data-action-id="UIA_BINDINGS.booksVerify.action.actionId"
            :disabled="session.state.busy"
            @click="session.verify().then(loadAttention)"
          >
            Check books
          </button>
        </div>

        <p class="operation-message" role="status" aria-live="polite">
          {{ session.state.error || session.state.message }}
        </p>
      </article>

      <article class="panel" aria-labelledby="assistant-home-heading">
        <p class="eyebrow">Assistant home</p>
        <h2 id="assistant-home-heading">Ask Shark Books</h2>
        <CommandPanel
          :session="session"
          @navigate="emit('navigate', $event)"
          @attention="commandAttention = $event"
        />
        <p class="runtime-note">{{ bridgeStatus }}</p>
      </article>
    </section>

    <section v-if="session.state.isOpen && session.state.home" class="status-grid">
      <article class="metric-card">
        <span>Business</span>
        <strong>{{ session.state.home.companyName }}</strong>
      </article>
      <article class="metric-card">
        <span>Current records</span>
        <strong>{{ session.state.home.transactionCount }}</strong>
      </article>
      <article class="metric-card">
        <span>Books check</span>
        <strong>{{ session.state.home.booksBalanced ? "Balanced" : "Needs review" }}</strong>
      </article>
    </section>

    <p v-if="attentionError" class="error-banner" role="status" aria-live="polite">
      {{ attentionError }}
    </p>

    <AttentionPanel
      :home="session.state.home"
      :bank-rows="bankRows"
      :books-open="session.state.isOpen"
      :command-attention="commandAttention"
    />
  </div>
</template>
