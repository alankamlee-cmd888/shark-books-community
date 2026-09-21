<script setup lang="ts">
import { computed, ref } from "vue";
import { commandSection, type CommandSection } from "../lib/command-routes";
import { errorMessage } from "../lib/format";
import type { BooksSession } from "../lib/session";
import {
  resolveCommandText,
  type CommandFact,
  type OwnerCommandAttention,
  type OwnerCommandTextResponse,
} from "../lib/tauri";

const props = defineProps<{ session: BooksSession }>();
const emit = defineEmits<{
  navigate: [section: CommandSection];
  attention: [attention: OwnerCommandAttention | null];
}>();

const text = ref("");
const answer = ref("");
const selectedActionId = ref<string | null>(null);
const facts = ref<CommandFact[]>([]);
const response = ref<OwnerCommandTextResponse | null>(null);
const busy = ref(false);
const error = ref("");

const route = computed(() => {
  const resolution = response.value?.resolution;
  if (!resolution || resolution.execution !== "EXECUTABLE" || !resolution.actionId) return null;
  const readyToHandOff =
    resolution.state === "KNOWN" ||
    (resolution.state === "UNKNOWN" &&
      resolution.missingOwnerSlots.length === 0 &&
      resolution.missingContextSlots.length > 0);
  return readyToHandOff ? commandSection(resolution.actionId, facts.value) : null;
});

function context() {
  const current = props.session.state.context;
  return {
    booksReference: props.session.state.isOpen ? current?.booksId ?? null : null,
    actor: current?.actor ?? "owner",
  };
}

async function resolveCurrent(candidateActionId: string | null) {
  busy.value = true;
  error.value = "";
  try {
    const ctx = context();
    const next = await resolveCommandText({
      text: text.value,
      booksReference: ctx.booksReference,
      actor: ctx.actor,
      candidateActionId,
      facts: facts.value,
    });
    response.value = next;
    emit("attention", next.resolution.attention);
  } catch (cause) {
    response.value = null;
    emit("attention", null);
    error.value = errorMessage(cause);
  } finally {
    busy.value = false;
  }
}

async function start() {
  selectedActionId.value = null;
  facts.value = [];
  answer.value = "";
  emit("attention", null);
  await resolveCurrent(null);
}

async function choose(actionId: string) {
  selectedActionId.value = actionId;
  facts.value = [];
  answer.value = "";
  await resolveCurrent(actionId);
}

async function answerPrompt() {
  const prompt = response.value?.prompt;
  const value = answer.value.trim();
  if (!prompt || !value) return;
  facts.value = [...facts.value, { slotId: prompt.slotId, value }];
  answer.value = "";
  const actionId = selectedActionId.value ?? response.value?.resolution.actionId ?? null;
  selectedActionId.value = actionId;
  await resolveCurrent(actionId);
}

function continueTo(section: CommandSection) {
  emit("navigate", section);
}
</script>

<template>
  <div class="command-panel">
    <form class="command-entry" @submit.prevent="start">
      <label>
        <span>Command</span>
        <input
          v-model="text"
          autocomplete="off"
          :disabled="busy"
          placeholder="Try “Business summary” or “Show contacts”"
          aria-describedby="command-help"
        />
      </label>
      <button type="submit" :disabled="busy || !text.trim()">
        {{ busy ? "Checking…" : "Ask Shark Books" }}
      </button>
    </form>

    <p id="command-help" class="subtle">
      Finite command mode uses the same Shark Action Registry as the manual screens. It does not
      use an AI accounting interpreter or bypass confirmations.
    </p>

    <p v-if="error" class="error-banner" role="status" aria-live="polite">{{ error }}</p>

    <div v-if="response" class="command-result" role="status" aria-live="polite">
      <p class="command-state">
        <strong>{{ response.resolution.state }}</strong>
        <span>· {{ response.resolution.execution }}</span>
      </p>

      <p v-if="response.resolution.attention" class="subtle">
        {{ response.resolution.attention.summary }}
      </p>

      <div
        v-if="response.resolution.state === 'AMBIGUOUS' && response.choices.length"
        class="command-choices"
      >
        <p><strong>Choose what you meant:</strong></p>
        <button
          v-for="choice in response.choices"
          :key="choice.actionId"
          type="button"
          class="secondary"
          :disabled="busy"
          @click="choose(choice.actionId)"
        >
          {{ choice.manualLabel }} <span class="subtle">({{ choice.family }})</span>
        </button>
      </div>

      <form v-if="response.prompt" class="command-clarification" @submit.prevent="answerPrompt">
        <label>
          <span>{{ response.prompt.label }}</span>
          <input
            v-model="answer"
            autocomplete="off"
            :disabled="busy"
            :placeholder="response.prompt.choices[0] || 'Enter the required information'"
          />
        </label>
        <p v-if="response.prompt.choices.length" class="subtle">
          {{ response.prompt.choices.join(" · ") }}
        </p>
        <button type="submit" :disabled="busy || !answer.trim()">Continue</button>
      </form>

      <button
        v-if="route"
        type="button"
        class="secondary"
        @click="continueTo(route)"
      >
        Continue in {{ route }}
      </button>

      <p
        v-else-if="
          response.resolution.state === 'KNOWN' &&
          response.resolution.execution === 'EXECUTABLE' &&
          !response.prompt
        "
        class="subtle"
      >
        The action is identified. Use the conventional Shark Books screen to complete its existing
        bounded preview/confirmation flow.
      </p>
    </div>
  </div>
</template>
