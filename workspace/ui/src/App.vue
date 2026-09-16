<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import {
  createBooks,
  foundationHealth,
  openBooks,
  productionEncryptionRequired,
  verifyBooks,
} from "./lib/tauri";

const sections = [
  "Home",
  "Money in",
  "Money out",
  "Bank",
  "Receipts",
  "Contacts",
  "Reports",
  "Settings",
] as const;

type Section = (typeof sections)[number];

const activeSection = ref<Section>("Home");
const busy = ref(false);
const output = ref("Ready.");
const foundation = ref("checking");
const encryption = ref("checking");

const books = reactive({
  fileName: "my-books.sqlite",
  booksId: "my-books",
  companyName: "My business",
  actor: "owner",
});

const booksRef = computed(() => ({
  fileName: books.fileName.trim(),
  booksId: books.booksId.trim(),
  actor: books.actor.trim(),
}));

async function run(operation: () => Promise<unknown>) {
  if (busy.value) return;
  busy.value = true;
  try {
    const value = await operation();
    output.value =
      typeof value === "string" ? value : JSON.stringify(value, null, 2);
  } catch (error) {
    output.value = error instanceof Error ? error.message : String(error);
  } finally {
    busy.value = false;
  }
}

onMounted(async () => {
  try {
    const [health, required] = await Promise.all([
      foundationHealth(),
      productionEncryptionRequired(),
    ]);
    foundation.value = health;
    encryption.value = required ? "required" : "unexpectedly disabled";
  } catch (error) {
    foundation.value = "native bridge unavailable";
    encryption.value = "unknown";
    output.value = error instanceof Error ? error.message : String(error);
  }
});
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar" aria-label="Primary navigation">
      <div class="brand">
        <span class="brand-mark" aria-hidden="true">S</span>
        <div>
          <strong>Shark Books</strong>
          <span>Community</span>
        </div>
      </div>

      <nav class="nav-list">
        <button
          v-for="section in sections"
          :key="section"
          type="button"
          class="nav-item"
          :class="{ active: activeSection === section }"
          :aria-current="activeSection === section ? 'page' : undefined"
          @click="activeSection = section"
        >
          {{ section }}
        </button>
      </nav>
    </aside>

    <main class="main-content">
      <header class="topbar">
        <div>
          <p class="eyebrow">SBC-7B2 owner workspace</p>
          <h1>{{ activeSection }}</h1>
        </div>
        <dl class="runtime-status" aria-label="Runtime status">
          <div>
            <dt>Foundation</dt>
            <dd>{{ foundation }}</dd>
          </div>
          <div>
            <dt>Encryption</dt>
            <dd>{{ encryption }}</dd>
          </div>
        </dl>
      </header>

      <section v-if="activeSection === 'Home'" class="content-grid">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="eyebrow">Books</p>
              <h2>Open or create your books</h2>
            </div>
          </div>

          <form class="field-grid" @submit.prevent>
            <label>
              <span>Books file</span>
              <input v-model="books.fileName" autocomplete="off" />
            </label>
            <label>
              <span>Books ID</span>
              <input v-model="books.booksId" autocomplete="off" />
            </label>
            <label>
              <span>Business name</span>
              <input v-model="books.companyName" autocomplete="organization" />
            </label>
            <label>
              <span>Actor</span>
              <input v-model="books.actor" autocomplete="off" />
            </label>
          </form>

          <div class="button-row">
            <button
              type="button"
              :disabled="busy"
              @click="
                run(() =>
                  createBooks({
                    ...booksRef,
                    companyName: books.companyName.trim(),
                  }),
                )
              "
            >
              Create books
            </button>
            <button
              type="button"
              class="secondary"
              :disabled="busy"
              @click="run(() => openBooks(booksRef))"
            >
              Open books
            </button>
            <button
              type="button"
              class="secondary"
              :disabled="busy"
              @click="run(() => verifyBooks(booksRef))"
            >
              Check books
            </button>
          </div>
        </article>

        <article class="panel output-panel">
          <p class="eyebrow">Native result</p>
          <h2>Latest operation</h2>
          <pre aria-live="polite">{{ output }}</pre>
        </article>
      </section>

      <section v-else class="panel coming-soon" aria-live="polite">
        <p class="eyebrow">Integration slice</p>
        <h2>{{ activeSection }} is not wired yet</h2>
        <p>
          This source scaffold freezes the permanent navigation without pretending
          an unfinished owner action exists. The screen will be connected only to
          admitted Action IDs and bounded Shark operations.
        </p>
      </section>
    </main>
  </div>
</template>
