<script setup lang="ts">
import { ref } from "vue";
import type { Settings } from "../api";

const props = defineProps<{
  settings: Settings;
}>();

const emit = defineEmits<{
  save: [settings: Settings];
  close: [];
}>();

const theme = ref(props.settings.theme);
const refreshIntervalSecs = ref(props.settings.refreshIntervalSecs);

function submit() {
  emit("save", {
    theme: theme.value,
    refreshIntervalSecs: Math.max(1, refreshIntervalSecs.value || 1),
  });
}
</script>

<template>
  <div class="overlay">
    <div class="dialog">
      <h3>Settings</h3>
      <label>
        Theme
        <select v-model="theme">
          <option value="system">
            System
          </option>
          <option value="dark">
            Dark
          </option>
          <option value="light">
            Light
          </option>
        </select>
      </label>
      <label>
        Refresh interval (seconds)
        <input
          v-model.number="refreshIntervalSecs"
          type="number"
          min="1"
        >
      </label>
      <div class="actions">
        <button @click="submit">
          Save
        </button>
        <button @click="$emit('close')">
          Cancel
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
}
.dialog {
  background-color: var(--color-bg);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 1rem 1.2rem;
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  min-width: 260px;
}
.dialog h3 {
  margin: 0;
}
label {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  font-size: 0.85rem;
}
select {
  border-radius: 6px;
  border: 1px solid var(--color-input-border);
  padding: 0.4em 0.6em;
  font-family: inherit;
  color: inherit;
  background-color: var(--color-input-bg);
}
.actions {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.3rem;
}
</style>
