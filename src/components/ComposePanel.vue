<script setup lang="ts">
import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";

const props = defineProps<{
  /** Only the local connection can run compose (no Engine API for it). */
  isLocal: boolean;
  busy: boolean;
}>();

const emit = defineEmits<{
  up: [file: string];
  down: [file: string];
}>();

const file = ref("");

async function browse() {
  const path = await open({
    multiple: false,
    filters: [{ name: "Compose file", extensions: ["yml", "yaml"] }],
  });
  if (typeof path === "string") file.value = path;
}

function up() {
  if (!props.isLocal || props.busy || !file.value.trim()) return;
  emit("up", file.value.trim());
}

function down() {
  if (!props.isLocal || props.busy || !file.value.trim()) return;
  emit("down", file.value.trim());
}
</script>

<template>
  <div class="compose-panel">
    <p
      v-if="!isLocal"
      class="notice"
    >
      Compose requires a local connection.
    </p>
    <div class="row">
      <input
        v-model="file"
        :disabled="!isLocal"
        placeholder="path/to/docker-compose.yml"
      >
      <button
        :disabled="!isLocal || busy"
        @click="browse"
      >
        Browse…
      </button>
      <button
        :disabled="!isLocal || busy || !file.trim()"
        @click="up"
      >
        Up
      </button>
      <button
        :disabled="!isLocal || busy || !file.trim()"
        @click="down"
      >
        Down
      </button>
    </div>
  </div>
</template>

<style scoped>
.compose-panel {
  padding: 0.5rem 0.6rem;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.row {
  display: flex;
  gap: 0.4rem;
}
.row input {
  flex: 1;
}
.notice {
  font-size: 0.85rem;
  opacity: 0.7;
  margin: 0;
}
</style>
