<script setup lang="ts">
import { computed, ref } from "vue";
import HomeScreen from "./screens/HomeScreen.vue";
import MoneyScreen from "./screens/MoneyScreen.vue";
import BankScreen from "./screens/BankScreen.vue";
import ReceiptsScreen from "./screens/ReceiptsScreen.vue";
import ContactsScreen from "./screens/ContactsScreen.vue";
import ReportsScreen from "./screens/ReportsScreen.vue";
import SettingsScreen from "./screens/SettingsScreen.vue";
import { createBooksSession } from "./lib/session";

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
const session = createBooksSession();
const subtitle = computed(() =>
  session.state.isOpen && session.state.home
    ? session.state.home.companyName
    : "Local owner workspace",
);
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar" aria-label="Primary navigation">
      <div class="brand">
        <span class="brand-mark" aria-hidden="true">S</span>
        <div><strong>Shark Books</strong><span>Community</span></div>
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
        >{{ section }}</button>
      </nav>
    </aside>

    <main class="main-content">
      <header class="topbar">
        <div>
          <p class="eyebrow">SBC-7B2 owner workspace</p>
          <h1>{{ activeSection }}</h1>
          <p class="topbar-subtitle">{{ subtitle }}</p>
        </div>
        <span class="session-chip" :class="{ open: session.state.isOpen }">
          {{ session.state.isOpen ? "Books open" : "Books closed" }}
        </span>
      </header>

      <HomeScreen v-if="activeSection === 'Home'" :session="session" />
      <MoneyScreen v-else-if="activeSection === 'Money in'" :session="session" kind="moneyIn" />
      <MoneyScreen v-else-if="activeSection === 'Money out'" :session="session" kind="moneyOut" />
      <BankScreen v-else-if="activeSection === 'Bank'" :session="session" />
      <ReceiptsScreen v-else-if="activeSection === 'Receipts'" :session="session" />
      <ContactsScreen v-else-if="activeSection === 'Contacts'" :session="session" />
      <ReportsScreen v-else-if="activeSection === 'Reports'" :session="session" />
      <SettingsScreen v-else :session="session" />
    </main>
  </div>
</template>
