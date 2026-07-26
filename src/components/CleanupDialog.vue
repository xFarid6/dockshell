<script setup lang="ts">
import { ref } from "vue";

const props = defineProps<{
  busy: boolean;
}>();

const emit = defineEmits<{
  prune: [categories: ("containers" | "images" | "volumes" | "networks")[]];
  close: [];
}>();

const CATEGORIES = [
  { key: "containers" as const, label: "Stopped containers" },
  { key: "images" as const, label: "Dangling images" },
  { key: "volumes" as const, label: "Unused volumes" },
  { key: "networks" as const, label: "Unused networks" },
];

const selected = ref<Record<string, boolean>>({
  containers: false,
  images: false,
  volumes: false,
  networks: false,
});
const confirming = ref(false);

function selectedCategories() {
  return CATEGORIES.filter((c) => selected.value[c.key]).map((c) => c.key);
}

function submit() {
  if (props.busy || selectedCategories().length === 0) return;
  confirming.value = true;
}

function confirmPrune() {
  emit("prune", selectedCategories());
  confirming.value = false;
}
</script>

<template>
  <div class="dialog">
    <h3>Clean up</h3>
    <template v-if="!confirming">
      <label
        v-for="c in CATEGORIES"
        :key="c.key"
      >
        <input
          v-model="selected[c.key]"
          type="checkbox"
        >
        {{ c.label }}
      </label>
      <div class="actions">
        <button
          :disabled="busy || selectedCategories().length === 0"
          @click="submit"
        >
          Clean up
        </button>
        <button
          :disabled="busy"
          @click="$emit('close')"
        >
          Cancel
        </button>
      </div>
    </template>
    <template v-else>
      <p class="warning">
        This will permanently remove: {{ selectedCategories().join(", ") }}. This cannot be undone.
      </p>
      <div class="actions">
        <button
          :disabled="busy"
          @click="confirmPrune"
        >
          Confirm removal
        </button>
        <button
          :disabled="busy"
          @click="confirming = false"
        >
          Back
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.dialog {
  border: 1px solid rgba(128, 128, 128, 0.25);
  border-radius: 8px;
  padding: 0.8rem 1rem;
  margin: 0.5rem 0.6rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.dialog h3 {
  margin: 0;
}
label {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.9rem;
}
.warning {
  font-size: 0.9rem;
}
.actions {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.3rem;
}
</style>
