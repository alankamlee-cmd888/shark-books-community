<script setup lang="ts">
import {
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from "reka-ui";

const props = defineProps<{
  open: boolean;
  title: string;
  description: string;
  confirmLabel: string;
  actionId: string;
  busy?: boolean;
  confirmDisabled?: boolean;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  confirm: [];
}>();

function close() {
  if (!props.busy) emit("update:open", false);
}

function confirm() {
  if (!props.busy && !props.confirmDisabled) emit("confirm");
}
</script>

<template>
  <DialogRoot
    :open="open"
    @update:open="(value) => emit('update:open', value)"
  >
    <DialogPortal>
      <DialogOverlay class="dialog-overlay" />
      <DialogContent class="dialog-content">
        <DialogTitle class="dialog-title">{{ title }}</DialogTitle>
        <DialogDescription class="dialog-description">
          {{ description }}
        </DialogDescription>
        <div class="dialog-body">
          <slot />
        </div>
        <div class="dialog-actions">
          <button type="button" class="secondary" :disabled="busy" @click="close">
            Go back
          </button>
          <button
            type="button"
            :data-action-id="actionId"
            :disabled="busy || confirmDisabled"
            @click="confirm"
          >
            {{ busy ? "Working…" : confirmLabel }}
          </button>
        </div>
        <p class="visually-hidden" role="status" aria-live="polite">
          {{ busy ? "Operation in progress." : "Ready for explicit confirmation." }}
        </p>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
